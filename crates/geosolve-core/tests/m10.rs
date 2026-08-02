use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use geosolve_core::{
    AcceptedAuditPatch, AcceptedStatePatch, AuditBinding, AuditEvaluationStatus, BoundStatus,
    CoordinateBound, CoreError, DiagnosticBudget, DiagnosticIncompleteReason, DiagnosticStatus,
    EvaluationError, HardValidity, LocalJacobian, OneSidedMobility, OperationControl,
    OperationController, OperationLimits, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SecondaryStatus, SessionCoreRejection,
    SessionDomainRejection, SessionError, SessionPatch, SessionTransactionRejection, SolveSession,
    SolveTermination, SolverConfig, SourceConstraint, VariableBlock, VariableId, VariableKind,
    VariableValue, cancellation_pair,
};

fn row(label: &str) -> ResidualRowAudit {
    ResidualRowAudit::new(
        label,
        vec![AuditBinding::new("variable", "M10 synthetic")],
        "model-unit",
    )
}

#[derive(Clone, Copy, Debug)]
struct ScalarTarget(f64);

impl ResidualEvaluator for ScalarTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![value - self.0])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct Vec2Target([f64; 2]);

impl ResidualEvaluator for Vec2Target {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected Vec2"));
        };
        Ok(vec![value[0] - self.0[0], value[1] - self.0[1]])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct DifferenceTarget(f64);

impl ResidualEvaluator for DifferenceTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![first - second - self.0])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![
            LocalJacobian::new(1, 1, vec![1.0]),
            LocalJacobian::new(1, 1, vec![-1.0]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
struct UnitCircleRows;

impl ResidualEvaluator for UnitCircleRows {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![value.cos(), value.sin()])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![LocalJacobian::new(
            2,
            1,
            vec![-value.sin(), value.cos()],
        )])
    }
}

#[derive(Clone, Debug)]
struct DomainCheckedTarget {
    target: f64,
    lower: f64,
    upper: f64,
    outside: Arc<AtomicUsize>,
}

impl ResidualEvaluator for DomainCheckedTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        if !(self.lower..=self.upper).contains(value) {
            self.outside.fetch_add(1, Ordering::Relaxed);
            return Err(EvaluationError::out_of_domain("bound trial escaped"));
        }
        Ok(vec![value - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        self.evaluate(variables)?;
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Debug)]
struct DenseVec3Rows {
    coefficients: Vec<[f64; 3]>,
    targets: Vec<f64>,
}

#[derive(Clone, Copy, Debug)]
struct Vec2LinearRow {
    coefficients: [f64; 2],
    target: f64,
}

#[derive(Clone, Copy, Debug)]
struct ScalarPairTarget([f64; 2]);

impl ResidualEvaluator for ScalarPairTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![first - self.0[0], second - self.0[1]])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![
            LocalJacobian::new(2, 1, vec![1.0, 0.0]),
            LocalJacobian::new(2, 1, vec![0.0, 1.0]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
enum SaddleBoundSide {
    Lower,
    Upper,
}

#[derive(Clone, Debug)]
struct BoundedScalarSaddle {
    scale: f64,
    side: SaddleBoundSide,
    outside: Arc<AtomicUsize>,
}

impl ResidualEvaluator for BoundedScalarSaddle {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        let outside = match self.side {
            SaddleBoundSide::Lower => *value < 0.0,
            SaddleBoundSide::Upper => *value > 0.0,
        };
        if outside {
            self.outside.fetch_add(1, Ordering::Relaxed);
            return Err(EvaluationError::out_of_domain(
                "one-sided saddle sampled outside its bound",
            ));
        }
        let normalized = value / self.scale;
        Ok(vec![1.0 - normalized * normalized])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        self.evaluate(variables)?;
        Ok(vec![LocalJacobian::new(
            1,
            1,
            vec![-2.0 * value / (self.scale * self.scale)],
        )])
    }
}

#[derive(Clone, Debug)]
struct BoundedMixedSaddle {
    outside: Arc<AtomicUsize>,
}

#[derive(Clone, Copy, Debug)]
struct MaskedBoundSaddle;

impl ResidualEvaluator for MaskedBoundSaddle {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![1.0 - value * value + 1.0e6 * value.powi(4)])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![LocalJacobian::new(
            1,
            1,
            vec![-2.0 * value + 4.0e6 * value.powi(3)],
        )])
    }
}

#[derive(Clone, Copy, Debug)]
struct SmoothSampleMaskedMaximum;

impl SmoothSampleMaskedMaximum {
    const RADII: [f64; 3] = [1.0e-3, 5.0e-4, 2.5e-4];

    fn value_and_derivative(value: f64) -> (f64, f64) {
        let mut factors = [0.0; 3];
        let mut derivatives = [0.0; 3];
        for (index, radius) in Self::RADII.into_iter().enumerate() {
            let difference = value * value - radius * radius;
            let radius_fourth = radius.powi(4);
            factors[index] = difference * difference / radius_fourth;
            derivatives[index] = 4.0 * value * difference / radius_fourth;
        }
        let product = factors.iter().product::<f64>();
        let product_derivative = (0..3)
            .map(|index| {
                derivatives[index]
                    * factors
                        .iter()
                        .enumerate()
                        .filter_map(|(other, factor)| (other != index).then_some(*factor))
                        .product::<f64>()
            })
            .sum::<f64>();
        (
            1.0 - value * value * product,
            -2.0 * value * product - value * value * product_derivative,
        )
    }
}

impl ResidualEvaluator for SmoothSampleMaskedMaximum {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![Self::value_and_derivative(*value).0])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![LocalJacobian::new(
            1,
            1,
            vec![Self::value_and_derivative(*value).1],
        )])
    }
}

#[derive(Clone, Copy, Debug)]
struct ConstantSecondary(f64);

impl ResidualEvaluator for ConstantSecondary {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if !variables.is_empty() {
            return Err(EvaluationError::invalid_geometry("expected no variables"));
        }
        Ok(vec![self.0])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        if !variables.is_empty() {
            return Err(EvaluationError::invalid_geometry("expected no variables"));
        }
        Ok(Vec::new())
    }
}

#[derive(Clone, Copy, Debug)]
struct ConstantSecondaryWithInvalidDerivative;

impl ResidualEvaluator for ConstantSecondaryWithInvalidDerivative {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if !variables.is_empty() {
            return Err(EvaluationError::invalid_geometry("expected no variables"));
        }
        Ok(vec![1.0])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Err(EvaluationError::nondifferentiable(
            "constant secondary derivative intentionally unavailable",
        ))
    }
}

impl ResidualEvaluator for BoundedMixedSaddle {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2([x, y])] = variables else {
            return Err(EvaluationError::invalid_geometry("expected Vec2"));
        };
        if *x < 0.0 || *y < 0.0 {
            self.outside.fetch_add(1, Ordering::Relaxed);
            return Err(EvaluationError::out_of_domain(
                "mixed saddle sampled outside its bounds",
            ));
        }
        Ok(vec![1.0 + x * x + y * y - 4.0 * x * y])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2([x, y])] = variables else {
            return Err(EvaluationError::invalid_geometry("expected Vec2"));
        };
        self.evaluate(variables)?;
        Ok(vec![LocalJacobian::new(
            1,
            2,
            vec![2.0 * x - 4.0 * y, 2.0 * y - 4.0 * x],
        )])
    }
}

impl ResidualEvaluator for Vec2LinearRow {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected Vec2"));
        };
        Ok(vec![
            self.coefficients[0] * value[0] + self.coefficients[1] * value[1] - self.target,
        ])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 2, self.coefficients.to_vec())])
    }
}

impl ResidualEvaluator for DenseVec3Rows {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec3(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected Vec3"));
        };
        Ok(self
            .coefficients
            .iter()
            .zip(&self.targets)
            .map(|(row, target)| row[0] * value[0] + row[1] * value[1] + row[2] * value[2] - target)
            .collect())
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(
            self.coefficients.len(),
            3,
            self.coefficients
                .iter()
                .flat_map(|row| row.iter().copied())
                .collect(),
        )])
    }
}

#[derive(Clone, Debug)]
struct CountingScalarTarget {
    target: f64,
    calls: Arc<AtomicUsize>,
}

impl ResidualEvaluator for CountingScalarTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![value - self.target])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Debug)]
struct IsolatedStatefulTarget {
    target: f64,
    calls: Arc<AtomicUsize>,
    instances: Arc<Mutex<Vec<Arc<AtomicUsize>>>>,
}

impl IsolatedStatefulTarget {
    fn new(target: f64, instances: Arc<Mutex<Vec<Arc<AtomicUsize>>>>) -> Self {
        let calls = Arc::new(AtomicUsize::new(0));
        instances.lock().unwrap().push(Arc::clone(&calls));
        Self {
            target,
            calls,
            instances,
        }
    }
}

impl Clone for IsolatedStatefulTarget {
    fn clone(&self) -> Self {
        let calls = Arc::new(AtomicUsize::new(self.calls.load(Ordering::Relaxed)));
        self.instances.lock().unwrap().push(Arc::clone(&calls));
        Self {
            target: self.target,
            calls,
            instances: Arc::clone(&self.instances),
        }
    }
}

impl ResidualEvaluator for IsolatedStatefulTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(vec![value - self.target])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct InvalidEvaluation;

impl ResidualEvaluator for InvalidEvaluation {
    fn evaluate(&self, _: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Err(EvaluationError::invalid_geometry("diagnostic fixture"))
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Err(EvaluationError::invalid_geometry("diagnostic fixture"))
    }
}

#[derive(Clone, Copy, Debug)]
struct InvalidRank;

impl ResidualEvaluator for InvalidRank {
    fn evaluate(&self, _: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Ok(vec![0.0])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![f64::NAN])])
    }
}

fn source(problem: &mut Problem, label: &str) -> geosolve_core::SourceConstraintId {
    problem.add_source(SourceConstraint::new(label).unwrap())
}

fn add_scalar_target(
    problem: &mut Problem,
    variable: VariableId,
    category: ResidualCategory,
    target: f64,
    label: &str,
) -> (geosolve_core::SourceConstraintId, geosolve_core::ResidualId) {
    let source_id = source(problem, label);
    let residual = problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                category,
                vec![variable],
                1,
                vec![1.0],
                vec![row(label)],
                ScalarTarget(target),
            )
            .unwrap(),
        )
        .unwrap();
    (source_id, residual)
}

fn add_vec3_rows(
    problem: &mut Problem,
    variable: VariableId,
    category: ResidualCategory,
    coefficients: Vec<[f64; 3]>,
    targets: Vec<f64>,
    label: &str,
) {
    let source_id = source(problem, label);
    let rows = targets
        .iter()
        .enumerate()
        .map(|(index, _)| row(&format!("{label} row {index}")))
        .collect::<Vec<_>>();
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                category,
                vec![variable],
                targets.len(),
                vec![1.0; targets.len()],
                rows,
                DenseVec3Rows {
                    coefficients,
                    targets,
                },
            )
            .unwrap(),
        )
        .unwrap();
}

fn scalar(problem: &Problem, variable: VariableId) -> f64 {
    let VariableValue::Scalar(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected scalar")
    };
    value
}

fn invalid_diagnostic_report(
    evaluator: impl ResidualEvaluator + 'static,
) -> geosolve_core::SolveReport {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "invalid diagnostic");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![row("invalid diagnostic")],
                evaluator,
            )
            .unwrap(),
        )
        .unwrap();
    problem.solve(SolverConfig::default()).unwrap()
}

#[test]
fn lower_upper_fixed_and_vec2_bounds_are_active_during_solving() {
    for (initial, target, lower, upper, status, expected) in [
        (
            -2.0,
            -1.0,
            Some(0.0),
            None,
            BoundStatus::ActiveLower,
            0.0_f64,
        ),
        (3.0, 2.0, None, Some(1.0), BoundStatus::ActiveUpper, 1.0),
        (9.0, 4.0, Some(0.5), Some(0.5), BoundStatus::Fixed, 0.5),
    ] {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(initial, 1.0).unwrap());
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Temporary,
            target,
            "bounded target",
        );
        problem
            .add_bound(CoordinateBound::new(variable, 0, lower, upper, "scalar box").unwrap())
            .unwrap();
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(scalar(&problem, variable).to_bits(), expected.to_bits());
        assert_eq!(report.bounds[0].status, status);
        assert_eq!(report.hard_validity, HardValidity::Valid);
    }

    let mut vector = Problem::new();
    let variable = vector.add_variable(VariableBlock::vec2([-4.0, 7.0], [1.0, 1.0]).unwrap());
    let source_id = source(&mut vector, "Vec2 temporary target");
    vector
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                2,
                vec![1.0, 1.0],
                vec![row("x target"), row("y target")],
                Vec2Target([-2.0, 4.0]),
            )
            .unwrap(),
        )
        .unwrap();
    vector
        .add_bound(CoordinateBound::new(variable, 0, Some(-1.0), None, "x lower").unwrap())
        .unwrap();
    vector
        .add_bound(CoordinateBound::new(variable, 1, None, Some(2.0), "y upper").unwrap())
        .unwrap();
    let report = vector.solve(SolverConfig::default()).unwrap();
    let VariableValue::Vec2(value) = vector.variable(variable).unwrap().value() else {
        panic!("expected Vec2")
    };
    assert_eq!(
        value.map(f64::to_bits),
        [(-1.0_f64).to_bits(), 2.0_f64.to_bits()]
    );
    assert_eq!(
        report
            .bounds
            .iter()
            .map(|bound| bound.status)
            .collect::<Vec<_>>(),
        vec![BoundStatus::ActiveLower, BoundStatus::ActiveUpper]
    );
}

#[test]
fn trials_never_reach_evaluators_outside_bounds_and_outside_hard_target_is_invalid() {
    let outside = Arc::new(AtomicUsize::new(0));
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.25, 1.0).unwrap());
    let source_id = source(&mut problem, "outside hard target");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![row("x - 2")],
                DomainCheckedTarget {
                    target: 2.0,
                    lower: 0.0,
                    upper: 1.0,
                    outside: Arc::clone(&outside),
                },
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), Some(1.0), "unit").unwrap())
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(outside.load(Ordering::Relaxed), 0);
    assert_ne!(report.termination, SolveTermination::Converged);
    assert_eq!(report.hard_validity, HardValidity::Invalid);
    assert_eq!(scalar(&problem, variable).to_bits(), 1.0_f64.to_bits());
    assert_eq!(report.bounds[0].status, BoundStatus::ActiveUpper);
}

#[test]
fn equality_nullity_bidirectional_dof_and_one_sided_motion_are_distinct() {
    let mut lower = Problem::new();
    let x = lower.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    lower
        .add_bound(CoordinateBound::new(x, 0, Some(0.0), None, "lower").unwrap())
        .unwrap();
    let report = lower.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.right_nullity, 1);
    assert_eq!(report.bidirectional_degrees_of_freedom, 0);
    assert_eq!(report.one_sided_mobility, OneSidedMobility::Exists);

    let mut fixed = Problem::new();
    let x = fixed.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    fixed
        .add_bound(CoordinateBound::new(x, 0, Some(0.0), Some(0.0), "fixed").unwrap())
        .unwrap();
    let report = fixed.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.right_nullity, 1);
    assert_eq!(report.bidirectional_degrees_of_freedom, 0);
    assert_eq!(report.one_sided_mobility, OneSidedMobility::None);
}

#[test]
fn duplicate_active_alias_normals_do_not_overcount_mobility() {
    let mut problem = Problem::new();
    let alias = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let root = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "alias");
    let residual = problem
        .add_residual(
            ResidualBlock::exact_alias(
                source_id,
                alias,
                root,
                VariableKind::Scalar,
                vec![1.0],
                vec![row("alias")],
            )
            .unwrap(),
        )
        .unwrap();
    problem.declare_exact_alias(alias, root, residual).unwrap();
    problem
        .add_bound(CoordinateBound::new(alias, 0, Some(0.0), None, "alias lower").unwrap())
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(root, 0, Some(0.0), None, "root lower").unwrap())
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.right_nullity, 1);
    assert_eq!(report.bidirectional_degrees_of_freedom, 0);
    assert_eq!(report.one_sided_mobility, OneSidedMobility::Exists);
    assert_eq!(report.bounds.len(), 2);
}

#[test]
fn temporary_bound_optimum_dominates_preference_and_inward_motion_releases() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.25, 1.0).unwrap());
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        2.0,
        "temporary outside upper",
    );
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        0.0,
        "preference inward",
    );
    problem
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), Some(1.0), "unit").unwrap())
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(scalar(&problem, variable).to_bits(), 1.0_f64.to_bits());
    assert_eq!(report.bounds[0].status, BoundStatus::ActiveUpper);
    assert_eq!(report.temporary_status, SecondaryStatus::Acceptable);

    let mut inward = Problem::new();
    let variable = inward.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    add_scalar_target(
        &mut inward,
        variable,
        ResidualCategory::Temporary,
        0.25,
        "inward target",
    );
    inward
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), Some(1.0), "unit").unwrap())
        .unwrap();
    let report = inward.solve(SolverConfig::default()).unwrap();
    assert!((scalar(&inward, variable) - 0.25).abs() <= 1.0e-12);
    assert_eq!(report.bounds[0].status, BoundStatus::Inactive);
}

#[test]
fn session_derives_dirty_components_preserves_ids_and_rejects_stale_or_failed_patches() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(8.0, 1.0).unwrap());
    let (_, x_residual) =
        add_scalar_target(&mut problem, x, ResidualCategory::Hard, 1.0, "x target");
    add_scalar_target(&mut problem, y, ResidualCategory::Hard, 2.0, "y target");
    problem
        .add_bound(CoordinateBound::new(x, 0, Some(0.0), Some(3.0), "x unit-ish").unwrap())
        .unwrap();
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let layout = session.problem().packed_layout().unwrap();
    let ids = session.problem().analyze_incidence();
    let accepted_report = session.report().clone();
    let accepted_revisions = session.revisions();

    let mut patch = SessionPatch::new(accepted_revisions);
    patch.set_variable_value(x, VariableValue::Scalar(0.25));
    let transaction = session.apply(patch).unwrap();
    assert!(transaction.committed());
    let x_component = transaction
        .report
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&x))
        .unwrap()
        .component_index;
    let y_component = transaction
        .report
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&y))
        .unwrap()
        .component_index;
    assert!(!transaction.report.component_solves[x_component].reused);
    assert!(transaction.report.component_solves[y_component].reused);
    assert_eq!(session.problem().packed_layout().unwrap(), layout);
    assert_eq!(session.problem().analyze_incidence(), ids);

    let stale = SessionPatch::new(accepted_revisions);
    assert!(matches!(
        session.apply(stale),
        Err(SessionError::StalePatch { .. })
    ));

    let before_problem = session.problem().packed_state().unwrap();
    let before_report = session.report().clone();
    let before_revisions = session.revisions();
    let before_stamps = session.component_dependency_stamps().to_vec();
    let source_id = session.problem().residual(x_residual).unwrap().source();
    let replacement = ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        vec![x],
        1,
        vec![1.0],
        vec![row("outside replacement")],
        ScalarTarget(5.0),
    )
    .unwrap();
    let mut failing = SessionPatch::new(before_revisions);
    failing.replace_residual(x_residual, replacement);
    let rejected = session.apply(failing).unwrap();
    assert!(!rejected.committed());
    assert!(matches!(
        rejected.rejection,
        Some(SessionTransactionRejection::Core(_))
    ));
    assert_eq!(session.problem().packed_state().unwrap(), before_problem);
    assert_eq!(session.report(), &before_report);
    assert_eq!(session.revisions(), before_revisions);
    assert_eq!(session.component_dependency_stamps(), before_stamps);
    assert_ne!(session.report(), &accepted_report);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one transaction lifecycle proves exact secondary preservation, provenance, row-level rejection, and rollback"
)]
fn exact_state_synchronization_preserves_secondary_rows_and_rejects_equal_cost_row_changes() {
    let mut problem = Problem::new();
    let patched_hard = problem.add_variable(VariableBlock::scalar(0.25, 1.0).unwrap());
    let secondary = problem.add_variable(VariableBlock::scalar(-0.25, 1.0).unwrap());
    let rotating = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let preference_rotating = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let movable_first = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let movable_second = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    for (variable, label) in [
        (patched_hard, "patchable hard coordinate"),
        (secondary, "secondary hard coordinate"),
        (rotating, "rotating hard coordinate"),
        (preference_rotating, "preference-rotating hard coordinate"),
    ] {
        add_scalar_target(&mut problem, variable, ResidualCategory::Hard, 0.0, label);
    }
    add_scalar_target(
        &mut problem,
        secondary,
        ResidualCategory::Temporary,
        1.0,
        "positive attained Temporary row",
    );
    add_scalar_target(
        &mut problem,
        secondary,
        ResidualCategory::Preference,
        -1.0,
        "positive attained Preference row",
    );
    let movable_hard_source = source(&mut problem, "movable secondary equality");
    problem
        .add_residual(
            ResidualBlock::new(
                movable_hard_source,
                ResidualCategory::Hard,
                vec![movable_first, movable_second],
                1,
                vec![1.0],
                vec![row("movable_first - movable_second")],
                DifferenceTarget(0.0),
            )
            .unwrap(),
        )
        .unwrap();
    let preference_rotating_source = source(&mut problem, "equal-cost rotating Preference rows");
    problem
        .add_residual(
            ResidualBlock::new(
                preference_rotating_source,
                ResidualCategory::Preference,
                vec![preference_rotating],
                2,
                vec![1.0, 1.0],
                vec![row("cos(angle)"), row("sin(angle)")],
                UnitCircleRows,
            )
            .unwrap(),
        )
        .unwrap();
    for (variable, target, label) in [
        (movable_first, 1.0, "movable positive Temporary first"),
        (movable_second, -1.0, "movable positive Temporary second"),
    ] {
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Temporary,
            target,
            label,
        );
    }
    for (variable, target, label) in [
        (movable_first, 2.0, "movable positive Preference first"),
        (movable_second, -2.0, "movable positive Preference second"),
    ] {
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Preference,
            target,
            label,
        );
    }
    let rotating_source = source(&mut problem, "equal-cost rotating Temporary rows");
    problem
        .add_residual(
            ResidualBlock::new(
                rotating_source,
                ResidualCategory::Temporary,
                vec![rotating],
                2,
                vec![1.0, 1.0],
                vec![row("cos(angle)"), row("sin(angle)")],
                UnitCircleRows,
            )
            .unwrap(),
        )
        .unwrap();
    let jacobian = problem.check_jacobians(1.0e-5).unwrap();
    assert!(jacobian.all_within(1.0e-8), "{jacobian:#?}");

    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let before = session.report().clone();
    assert!(
        before.iterations > 0,
        "the retained solve must carry real work"
    );
    assert!(before.priority_solves.iter().any(|priority| {
        priority.category == ResidualCategory::Temporary
            && priority.final_cost.is_some_and(|cost| cost > 0.0)
    }));
    assert!(before.priority_solves.iter().any(|priority| {
        priority.category == ResidualCategory::Preference
            && priority.final_cost.is_some_and(|cost| cost > 0.0)
    }));
    let secondary_rows = |report: &geosolve_core::SolveReport| {
        report
            .audit
            .sources
            .iter()
            .flat_map(|source| &source.rows)
            .filter(|row| {
                matches!(
                    row.category,
                    ResidualCategory::Temporary | ResidualCategory::Preference
                )
            })
            .map(|row| {
                (
                    row.residual_id,
                    row.row_in_block,
                    row.normalized_residual.to_bits(),
                )
            })
            .collect::<Vec<_>>()
    };
    let retained_secondary_rows = secondary_rows(&before);
    let retained_component_provenance = before
        .component_solves
        .iter()
        .map(|component| {
            (
                component.iterations,
                component.reused,
                component.actual_backend,
                component.symbolic_analysis_reused,
                component.symbolic_analysis_reuse_count,
                component.sparse_fallback_reason,
                component.trace.clone(),
            )
        })
        .collect::<Vec<_>>();
    let retained_priority_provenance = before
        .priority_solves
        .iter()
        .map(|priority| {
            (
                priority.group_index,
                priority.category,
                priority.backend,
                priority.largest_explicit_nullspace_block_rows,
                priority.iterations,
            )
        })
        .collect::<Vec<_>>();

    let before_allowlist_problem = session.problem().packed_state().unwrap();
    let before_allowlist_revisions = session.revisions();
    let before_allowlist_stamps = session.component_dependency_stamps().to_vec();
    let mut outside_allowlist =
        AcceptedStatePatch::new(before_allowlist_revisions, vec![patched_hard]);
    outside_allowlist.set_variable_value(secondary, VariableValue::Scalar(5.0e-10));
    assert!(matches!(
        session.synchronize_accepted_state(outside_allowlist),
        Err(SessionError::AcceptedStateVariableNotAllowed { variable }) if variable == secondary
    ));
    assert_eq!(
        session.problem().packed_state().unwrap(),
        before_allowlist_problem
    );
    assert_eq!(session.report(), &before);
    assert_eq!(session.revisions(), before_allowlist_revisions);
    assert_eq!(
        session.component_dependency_stamps(),
        before_allowlist_stamps
    );

    let secondary_before = secondary_rows(&before);
    let movable_after = 5.0e-11;
    let mut exact_patch = AcceptedStatePatch::new(
        session.revisions(),
        vec![patched_hard, movable_first, movable_second],
    );
    exact_patch.set_variable_value(patched_hard, VariableValue::Scalar(5.0e-10));
    exact_patch.set_variable_value(movable_first, VariableValue::Scalar(movable_after));
    exact_patch.set_variable_value(movable_second, VariableValue::Scalar(movable_after));
    let committed = session.synchronize_accepted_state(exact_patch).unwrap();
    assert!(committed.committed(), "{:#?}", committed.rejection);
    let secondary_after = secondary_rows(&committed.report);
    assert_ne!(
        secondary_after, secondary_before,
        "the accepted path must exercise changed secondary rows"
    );
    assert_eq!(secondary_after.len(), secondary_before.len());
    assert!(
        secondary_before
            .iter()
            .zip(&secondary_after)
            .all(|(before, after)| {
                before.0 == after.0
                    && before.1 == after.1
                    && (f64::from_bits(before.2) - f64::from_bits(after.2)).abs() <= 1.0e-10
            })
    );
    assert_eq!(committed.report.iterations, before.iterations);
    assert_eq!(
        committed
            .report
            .component_solves
            .iter()
            .map(|component| (
                component.iterations,
                component.reused,
                component.actual_backend,
                component.symbolic_analysis_reused,
                component.symbolic_analysis_reuse_count,
                component.sparse_fallback_reason,
                component.trace.clone()
            ))
            .collect::<Vec<_>>(),
        retained_component_provenance,
        "fresh certification must retain the execution provenance of the solve it certifies"
    );
    assert_eq!(
        committed
            .report
            .priority_solves
            .iter()
            .map(|priority| (
                priority.group_index,
                priority.category,
                priority.backend,
                priority.largest_explicit_nullspace_block_rows,
                priority.iterations,
            ))
            .collect::<Vec<_>>(),
        retained_priority_provenance
    );
    assert_ne!(secondary_after, retained_secondary_rows);

    let stopped_problem = session.problem().packed_state().unwrap();
    let stopped_report = session.report().clone();
    let stopped_revisions = session.revisions();
    let stopped_stamps = session.component_dependency_stamps().to_vec();
    let baseline = session.clone();
    let (cancel, token) = cancellation_pair();
    cancel.cancel();
    let mut controller =
        OperationController::new(OperationControl::new(token, OperationLimits::unlimited()));
    let mut stopped_patch = AcceptedStatePatch::new(stopped_revisions, vec![patched_hard]);
    stopped_patch.set_variable_value(patched_hard, VariableValue::Scalar(7.5e-10));
    assert!(
        session
            .synchronize_accepted_state_with_controller(stopped_patch, &mut controller)
            .unwrap()
            .is_none()
    );
    assert_eq!(session.problem().packed_state().unwrap(), stopped_problem);
    assert_eq!(session.report(), &stopped_report);
    assert_eq!(session.revisions(), stopped_revisions);
    assert_eq!(session.component_dependency_stamps(), stopped_stamps);
    let mut baseline = baseline;
    let baseline_probe = baseline
        .apply(SessionPatch::new(baseline.revisions()))
        .unwrap();
    let stopped_probe = session
        .apply(SessionPatch::new(session.revisions()))
        .unwrap();
    assert_eq!(stopped_probe.report, baseline_probe.report);

    let retained_problem = session.problem().packed_state().unwrap();
    let retained_report = session.report().clone();
    let retained_revisions = session.revisions();
    let rotating_before = scalar(session.problem(), rotating);
    let rotating_after = 5.0e-10_f64;
    let before_cost = 0.5 * (rotating_before.cos().powi(2) + rotating_before.sin().powi(2));
    let after_cost = 0.5 * (rotating_after.cos().powi(2) + rotating_after.sin().powi(2));
    assert_eq!(before_cost.to_bits(), after_cost.to_bits());

    let mut changed_rows = AcceptedStatePatch::new(retained_revisions, vec![rotating]);
    changed_rows.set_variable_value(rotating, VariableValue::Scalar(rotating_after));
    let rejected = session.synchronize_accepted_state(changed_rows).unwrap();
    assert!(!rejected.committed());
    assert!(
        matches!(
            rejected.rejection,
            Some(SessionTransactionRejection::Core(
                SessionCoreRejection::TemporaryResidualChanged { .. }
            ))
        ),
        "{:#?}",
        rejected.rejection
    );
    assert_eq!(session.problem().packed_state().unwrap(), retained_problem);
    assert_eq!(session.report(), &retained_report);
    assert_eq!(session.revisions(), retained_revisions);

    let preference_rotating_after = 5.0e-10_f64;
    let mut changed_preference_rows =
        AcceptedStatePatch::new(retained_revisions, vec![preference_rotating]);
    changed_preference_rows.set_variable_value(
        preference_rotating,
        VariableValue::Scalar(preference_rotating_after),
    );
    let rejected = session
        .synchronize_accepted_state(changed_preference_rows)
        .unwrap();
    assert!(!rejected.committed());
    assert!(
        matches!(
            rejected.rejection,
            Some(SessionTransactionRejection::Core(
                SessionCoreRejection::PreferenceResidualChanged { .. }
            ))
        ),
        "{:#?}",
        rejected.rejection
    );
    assert_eq!(session.problem().packed_state().unwrap(), retained_problem);
    assert_eq!(session.report(), &retained_report);
    assert_eq!(session.revisions(), retained_revisions);
}

#[test]
fn session_rejects_failure_termination_or_failed_audit_despite_success_evidence() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Hard,
        1.0,
        "accepted target",
    );
    let config = SolverConfig::default();
    let accepted = problem.solve(config).unwrap();
    assert_eq!(accepted.termination, SolveTermination::Converged);
    assert_eq!(accepted.hard_validity, HardValidity::Valid);
    assert!(accepted.hard_residuals_validated);
    assert!(accepted.rank_is_valid);
    assert!(accepted.audit.sources.iter().all(|source| {
        source
            .rows
            .iter()
            .all(|row| row.evaluation_status == AuditEvaluationStatus::Evaluated)
    }));

    for termination in [
        SolveTermination::InvalidGeometry,
        SolveTermination::NumericalFailure,
    ] {
        let mut report = accepted.clone();
        report.termination = termination;
        let error =
            SolveSession::from_accepted_report(problem.clone(), config, report).unwrap_err();
        assert!(matches!(
            error,
            SessionError::InitialRejected(geosolve_core::SessionCoreRejection::EvaluationFailure)
        ));
    }

    let mut report = accepted;
    report.audit.sources[0].rows[0].evaluation_status = AuditEvaluationStatus::Failed;
    let error = SolveSession::from_accepted_report(problem, config, report).unwrap_err();
    assert!(matches!(
        error,
        SessionError::InitialRejected(geosolve_core::SessionCoreRejection::EvaluationFailure)
    ));
}

#[test]
fn source_parameter_and_fixed_connector_replacements_dirty_all_dependencies() {
    let mut problem = Problem::new();
    let left = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let connector = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let right = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let fixed_source = source(&mut problem, "fixed connector");
    let fixed_residual = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                fixed_source,
                connector,
                VariableValue::Scalar(0.0),
                vec![1.0],
                vec![row("fixed connector")],
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_fixed_variable(connector, VariableValue::Scalar(0.0), fixed_residual)
        .unwrap();
    for (variable, target, label) in [(left, 1.0, "left"), (right, 2.0, "right")] {
        let source_id = source(&mut problem, label);
        problem
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    vec![variable, connector],
                    1,
                    vec![1.0],
                    vec![row(label)],
                    DifferenceTarget(target),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let replacement = ResidualBlock::fixed_variable(
        fixed_source,
        connector,
        VariableValue::Scalar(1.0),
        vec![1.0],
        vec![row("fixed connector replacement")],
    )
    .unwrap();
    let mut patch = SessionPatch::new(session.revisions());
    patch.replace_residual(fixed_residual, replacement);
    let result = session.apply(patch).unwrap();
    assert!(result.committed(), "{:#?}", result.rejection);
    for variable in [left, right] {
        let component = result
            .report
            .structural
            .component_summaries
            .iter()
            .find(|component| component.variable_ids.contains(&variable))
            .unwrap();
        assert!(!result.report.component_solves[component.component_index].reused);
    }
    assert!((scalar(session.problem(), left) - 2.0).abs() <= 1.0e-9);
    assert!((scalar(session.problem(), right) - 3.0).abs() <= 1.0e-9);
}

#[test]
fn successful_source_parameter_replacement_dirties_only_its_owner_component() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let (x_source, x_residual) = add_scalar_target(
        &mut problem,
        x,
        ResidualCategory::Hard,
        1.0,
        "x source parameter",
    );
    add_scalar_target(
        &mut problem,
        y,
        ResidualCategory::Hard,
        2.0,
        "y source parameter",
    );
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let before = session.revisions();
    let before_signature = session
        .report()
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&x))
        .unwrap()
        .pattern_signature;
    let retained_y = scalar(session.problem(), y);
    let replacement = ResidualBlock::new(
        x_source,
        ResidualCategory::Hard,
        vec![x],
        1,
        vec![2.0],
        vec![row("updated target and scale")],
        ScalarTarget(3.0),
    )
    .unwrap();
    let mut patch = SessionPatch::new(before);
    patch.replace_residual(x_residual, replacement);
    let result = session.apply(patch).unwrap();
    assert!(result.committed(), "{:#?}", result.rejection);
    let x_component = result
        .report
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&x))
        .unwrap()
        .component_index;
    let y_component = result
        .report
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&y))
        .unwrap()
        .component_index;
    assert!(!result.report.component_solves[x_component].reused);
    assert!(result.report.component_solves[y_component].reused);
    assert!((scalar(session.problem(), x) - 3.0).abs() <= 1.0e-9);
    assert_eq!(scalar(session.problem(), y).to_bits(), retained_y.to_bits());
    assert_eq!(result.revisions.source, before.source + 1);
    assert_eq!(result.revisions.state, before.state + 1);
    assert_ne!(
        result.report.component_solves[x_component].pattern_signature,
        before_signature
    );
    let accepted = session.accepted_hard_linearization().unwrap();
    let accepted_x = accepted.component(x_component).unwrap();
    assert_eq!(
        accepted_x.pattern_signature(),
        result.report.component_solves[x_component].pattern_signature
    );
    assert_eq!(
        accepted_x.hard_rows()[0].residual_scale.to_bits(),
        2.0_f64.to_bits()
    );
}

#[test]
fn compatible_bound_replacement_has_its_own_revision_and_stable_id() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        2.0,
        "temporary bound target",
    );
    let bound = problem
        .add_bound(
            CoordinateBound::new(variable, 0, Some(0.0), Some(1.0), "initial unit box").unwrap(),
        )
        .unwrap();
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    assert_eq!(
        scalar(session.problem(), variable).to_bits(),
        1.0_f64.to_bits()
    );
    let before = session.revisions();
    let mut patch = SessionPatch::new(before);
    patch.replace_bound(
        bound,
        CoordinateBound::new(variable, 0, Some(0.0), Some(3.0), "expanded box").unwrap(),
    );
    let result = session.apply(patch).unwrap();
    assert!(result.committed(), "{:#?}", result.rejection);
    assert!((scalar(session.problem(), variable) - 2.0).abs() <= 1.0e-12);
    assert_eq!(result.report.bounds[0].bound_id, bound);
    assert_eq!(result.revisions.bound, before.bound + 1);
    assert_eq!(result.revisions.source, before.source);
    assert_eq!(result.revisions.state, before.state + 1);
}

#[test]
#[allow(clippy::too_many_lines)]
fn diagnostics_report_complete_truncated_and_skipped_budgets_in_source_order() {
    let mut redundant = Problem::new();
    let x = redundant.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let first = add_scalar_target(&mut redundant, x, ResidualCategory::Hard, 0.0, "first").0;
    let second = add_scalar_target(&mut redundant, x, ResidualCategory::Hard, 0.0, "second").0;
    let complete = redundant.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        complete.redundancy_diagnostics.status,
        DiagnosticStatus::Complete
    );
    assert_eq!(complete.redundant_sources, vec![second]);
    assert_eq!(
        complete.conflict_diagnostics.status,
        DiagnosticStatus::Skipped
    );
    assert_eq!(
        complete.conflict_diagnostics.reason,
        Some(DiagnosticIncompleteReason::HardConstraintsValid)
    );
    assert_ne!(first, second);

    let mut conflict = Problem::new();
    let x = conflict.add_variable(VariableBlock::scalar(0.5, 1.0).unwrap());
    let zero = add_scalar_target(&mut conflict, x, ResidualCategory::Hard, 0.0, "zero").0;
    let one = add_scalar_target(&mut conflict, x, ResidualCategory::Hard, 1.0, "one").0;
    let truncated = conflict
        .solve(SolverConfig {
            conflict_diagnostic_budget: DiagnosticBudget {
                max_trials: 1,
                ..DiagnosticBudget::unlimited()
            },
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(
        truncated.conflict_diagnostics.status,
        DiagnosticStatus::Truncated
    );
    assert_eq!(truncated.conflict_diagnostics.consumed.trials, 1);
    assert_eq!(
        truncated.conflict_diagnostics.reason,
        Some(DiagnosticIncompleteReason::TrialBudget)
    );
    assert!(
        truncated.conflicting_sources == vec![zero] || truncated.conflicting_sources.is_empty()
    );
    assert!(!truncated.conflicting_sources.contains(&one));

    let mut complete_conflict = Problem::new();
    let x = complete_conflict.add_variable(VariableBlock::scalar(0.5, 1.0).unwrap());
    let zero = add_scalar_target(
        &mut complete_conflict,
        x,
        ResidualCategory::Hard,
        0.0,
        "complete zero",
    )
    .0;
    let one = add_scalar_target(
        &mut complete_conflict,
        x,
        ResidualCategory::Hard,
        1.0,
        "complete one",
    )
    .0;
    let complete = complete_conflict.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        complete.conflict_diagnostics.status,
        DiagnosticStatus::Complete
    );
    assert_eq!(complete.conflicting_sources, vec![zero, one]);

    let skipped = conflict
        .solve(SolverConfig {
            conflict_diagnostic_budget: DiagnosticBudget {
                enabled: false,
                ..DiagnosticBudget::unlimited()
            },
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(
        skipped.conflict_diagnostics.status,
        DiagnosticStatus::Skipped
    );
    assert_eq!(
        skipped.conflict_diagnostics.reason,
        Some(DiagnosticIncompleteReason::Disabled)
    );
    assert!(skipped.conflicting_sources.is_empty());

    let mut two_components = Problem::new();
    for (name, target) in [("first", 0.0), ("second", 2.0)] {
        let variable = two_components.add_variable(VariableBlock::scalar(target, 1.0).unwrap());
        add_scalar_target(
            &mut two_components,
            variable,
            ResidualCategory::Hard,
            target,
            &format!("{name} basis"),
        );
        add_scalar_target(
            &mut two_components,
            variable,
            ResidualCategory::Hard,
            target,
            &format!("{name} duplicate"),
        );
    }
    let redundancy_truncated = two_components
        .solve(SolverConfig {
            redundancy_diagnostic_budget: DiagnosticBudget {
                max_trials: 2,
                ..DiagnosticBudget::unlimited()
            },
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(
        redundancy_truncated.redundancy_diagnostics.status,
        DiagnosticStatus::Truncated
    );
    assert_eq!(
        redundancy_truncated
            .redundancy_diagnostics
            .consumed
            .components,
        1
    );
    assert_eq!(
        redundancy_truncated.redundancy_diagnostics.reason,
        Some(DiagnosticIncompleteReason::TrialBudget)
    );
}

#[test]
fn invalid_bounds_reject_without_entering_a_problem() {
    let mut problem = Problem::new();
    let scalar = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    assert!(CoordinateBound::new(scalar, 0, None, None, "empty").is_err());
    assert!(CoordinateBound::new(scalar, 0, Some(f64::NAN), None, "nan").is_err());
    assert!(CoordinateBound::new(scalar, 0, Some(2.0), Some(1.0), "reverse").is_err());
    assert!(CoordinateBound::new(scalar, 0, Some(0.0), None, " ").is_err());
    assert!(
        problem
            .add_bound(CoordinateBound::new(scalar, 1, Some(0.0), None, "bad coordinate").unwrap())
            .is_err()
    );
    problem
        .add_bound(CoordinateBound::new(scalar, 0, Some(0.0), None, "first").unwrap())
        .unwrap();
    assert!(
        problem
            .add_bound(CoordinateBound::new(scalar, 0, None, Some(1.0), "duplicate").unwrap())
            .is_err()
    );
}

#[test]
fn hard_bvls_releases_corner_coordinates_when_an_interior_witness_exists() {
    let mut problem = Problem::new();
    let variable =
        problem.add_variable(VariableBlock::vec3([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]).unwrap());
    add_vec3_rows(
        &mut problem,
        variable,
        ResidualCategory::Hard,
        vec![[1.0, -1.0, 0.0], [1.0, 0.0, 1.0]],
        vec![-4.0, 1.0],
        "external 2x3 corner",
    );
    problem
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), None, "x lower").unwrap())
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(variable, 2, None, Some(0.0), "z upper").unwrap())
        .unwrap();

    // [2, 6, -1] is a strictly interior feasible witness for both equations.
    let witness: [f64; 3] = [2.0, 6.0, -1.0];
    assert!((witness[0] - witness[1] + 4.0).abs() <= f64::EPSILON);
    assert!((witness[0] + witness[2] - 1.0).abs() <= f64::EPSILON);
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    let VariableValue::Vec3(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected Vec3")
    };
    assert!((value[0] - 1.0).abs() <= 1.0e-8, "{value:?}");
    assert!((value[1] - 5.0).abs() <= 1.0e-8, "{value:?}");
    assert!(value[2].abs() <= 1.0e-8, "{value:?}");
}

#[test]
fn secondary_working_set_reaches_feasible_zero_cost_in_three_variables() {
    let mut problem = Problem::new();
    let variable =
        problem.add_variable(VariableBlock::vec3([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]).unwrap());
    add_vec3_rows(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        vec![[1.0, -1.0, 0.0], [1.0, 0.0, 1.0]],
        vec![-4.0, 1.0],
        "bounded secondary 2x3",
    );
    problem
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), None, "x lower").unwrap())
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(variable, 2, None, Some(0.0), "z upper").unwrap())
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert!(
        report.priority_solves[0].final_cost.unwrap() <= 1.0e-18,
        "{report:#?}"
    );
    let VariableValue::Vec3(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected Vec3")
    };
    assert!(value[0] >= 0.0 && value[2] <= 0.0, "{value:?}");
}

#[test]
fn dependent_projected_lower_bounds_release_to_the_convex_zero_cost_target() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut problem = Problem::new();
        let x = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let y = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let equality_source = source(&mut problem, "dependent-bound equality");
        problem
            .add_residual(
                ResidualBlock::new(
                    equality_source,
                    ResidualCategory::Hard,
                    vec![x, y],
                    1,
                    vec![scale],
                    vec![row("x - y")],
                    DifferenceTarget(0.0),
                )
                .unwrap(),
            )
            .unwrap();
        let target_source = source(&mut problem, "dependent-bound target");
        problem
            .add_residual(
                ResidualBlock::new(
                    target_source,
                    ResidualCategory::Temporary,
                    vec![x, y],
                    2,
                    vec![scale, scale],
                    vec![row("x - target"), row("y - target")],
                    ScalarPairTarget([scale, scale]),
                )
                .unwrap(),
            )
            .unwrap();
        problem
            .add_bound(CoordinateBound::new(x, 0, Some(0.0), None, "x lower").unwrap())
            .unwrap();
        problem
            .add_bound(CoordinateBound::new(y, 0, Some(0.0), None, "y lower").unwrap())
            .unwrap();

        assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.hard_validity, HardValidity::Valid, "scale={scale:e}");
        assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
        assert!(
            report.priority_solves[0].final_cost.unwrap() <= 1.0e-18,
            "scale={scale:e}: {report:#?}"
        );
        assert!((scalar(&problem, x) / scale - 1.0).abs() <= 1.0e-9);
        assert!((scalar(&problem, y) / scale - 1.0).abs() <= 1.0e-9);
        assert!(
            report
                .bounds
                .iter()
                .all(|bound| bound.status == BoundStatus::Inactive)
        );
    }
}

#[test]
fn zero_multiplier_lower_and_upper_saddles_escape_only_inward() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for (side, expected_sign) in [
            (SaddleBoundSide::Lower, 1.0),
            (SaddleBoundSide::Upper, -1.0),
        ] {
            let outside = Arc::new(AtomicUsize::new(0));
            let evaluator = BoundedScalarSaddle {
                scale,
                side,
                outside: Arc::clone(&outside),
            };

            let mut oracle = Problem::new();
            let variable = oracle
                .add_variable(VariableBlock::scalar(expected_sign * 0.25 * scale, scale).unwrap());
            let source_id = source(&mut oracle, "one-sided saddle oracle");
            oracle
                .add_residual(
                    ResidualBlock::new(
                        source_id,
                        ResidualCategory::Temporary,
                        vec![variable],
                        1,
                        vec![1.0],
                        vec![row("1 - (x / scale)^2")],
                        evaluator.clone(),
                    )
                    .unwrap(),
                )
                .unwrap();
            assert!(oracle.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

            let mut problem = Problem::new();
            let variable = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
            let source_id = source(&mut problem, "one-sided saddle");
            problem
                .add_residual(
                    ResidualBlock::new(
                        source_id,
                        ResidualCategory::Temporary,
                        vec![variable],
                        1,
                        vec![1.0],
                        vec![row("1 - (x / scale)^2")],
                        evaluator,
                    )
                    .unwrap(),
                )
                .unwrap();
            let (lower, upper) = match side {
                SaddleBoundSide::Lower => (Some(0.0), None),
                SaddleBoundSide::Upper => (None, Some(0.0)),
            };
            problem
                .add_bound(
                    CoordinateBound::new(variable, 0, lower, upper, "one-sided saddle bound")
                        .unwrap(),
                )
                .unwrap();

            let report = problem.solve(SolverConfig::default()).unwrap();
            assert_eq!(
                report.termination,
                SolveTermination::Converged,
                "{report:#?}"
            );
            assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
            assert!(report.priority_solves[0].final_cost.unwrap() <= 1.0e-18);
            assert!(
                (scalar(&problem, variable) / scale - expected_sign).abs() <= 1.0e-9,
                "scale={scale:e}, side={side:?}: {report:#?}"
            );
            assert_eq!(outside.load(Ordering::Relaxed), 0);
        }
    }
}

#[test]
fn incomplete_multidimensional_critical_cone_never_reports_optimal() {
    let outside = Arc::new(AtomicUsize::new(0));
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let source_id = source(&mut problem, "bounded mixed saddle");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                vec![row("1 + x^2 + y^2 - 4*x*y")],
                BoundedMixedSaddle {
                    outside: Arc::clone(&outside),
                },
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), None, "x lower").unwrap())
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(variable, 1, Some(0.0), None, "y lower").unwrap())
        .unwrap();

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Stalled, "{report:#?}");
    assert_eq!(report.temporary_status, SecondaryStatus::Stalled);
    assert_eq!(outside.load(Ordering::Relaxed), 0);

    let session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    assert_eq!(session.report().termination, SolveTermination::Stalled);
    assert_eq!(session.report().temporary_status, SecondaryStatus::Stalled);
}

#[test]
fn finite_one_sided_stencil_cannot_certify_weak_bound_optimality() {
    let mut oracle = Problem::new();
    let variable = oracle.add_variable(VariableBlock::scalar(1.0e-3, 1.0).unwrap());
    let source_id = source(&mut oracle, "masked saddle oracle");
    oracle
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                vec![row("1 - x^2 + 1e6*x^4")],
                MaskedBoundSaddle,
            )
            .unwrap(),
        )
        .unwrap();
    let jacobian = oracle.check_jacobians(1.0e-7).unwrap();
    assert!(jacobian.all_within(1.0e-6), "{jacobian:#?}");

    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "masked one-sided saddle");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                vec![row("1 - x^2 + 1e6*x^4")],
                MaskedBoundSaddle,
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), None, "lower").unwrap())
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Stalled, "{report:#?}");
    assert_eq!(report.temporary_status, SecondaryStatus::Stalled);
}

#[test]
fn multiscale_unbounded_curvature_does_not_mask_a_singleton_saddle() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "masked unbounded saddle");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                vec![row("1 - x^2 + 1e6*x^4")],
                MaskedBoundSaddle,
            )
            .unwrap(),
        )
        .unwrap();
    let report = problem
        .solve(SolverConfig {
            max_iterations: 1,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.termination, SolveTermination::IterationLimit);
    assert_eq!(report.temporary_status, SecondaryStatus::IterationLimit);
    assert!(scalar(&problem, variable).abs() > 1.0e-6, "{report:#?}");
}

#[test]
fn smooth_maximum_masked_at_every_sampled_radius_is_acceptable_not_optimal() {
    let mut oracle = Problem::new();
    let variable = oracle.add_variable(VariableBlock::scalar(1.0e-4, 1.0).unwrap());
    let source_id = source(&mut oracle, "smooth sampled-radius mask oracle");
    oracle
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                vec![row("analytic sampled-radius masked maximum")],
                SmoothSampleMaskedMaximum,
            )
            .unwrap(),
        )
        .unwrap();
    let jacobian = oracle.check_jacobians(1.0e-7).unwrap();
    assert!(jacobian.all_within(1.0e-5), "{jacobian:#?}");

    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "smooth sampled-radius mask");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                vec![row("analytic sampled-radius masked maximum")],
                SmoothSampleMaskedMaximum,
            )
            .unwrap(),
        )
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.temporary_status, SecondaryStatus::Acceptable);
    assert_eq!(
        report.priority_solves[0].status,
        SecondaryStatus::Acceptable
    );
    assert_ne!(report.priority_solves[0].status, SecondaryStatus::Optimal);
    assert!(scalar(&problem, variable).abs() <= f64::EPSILON);
    assert!((report.priority_solves[0].final_cost.unwrap() - 0.5).abs() <= f64::EPSILON);
}

#[test]
fn mobility_uses_the_authoritative_weak_equality_nullspace_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut problem = Problem::new();
        let variable =
            problem.add_variable(VariableBlock::vec2([0.0, 0.0], [scale, scale]).unwrap());
        let source_id = source(&mut problem, "weak retained equality");
        problem
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![scale],
                    vec![row("[0, 5e-11] weak equality")],
                    Vec2LinearRow {
                        coefficients: [0.0, 5.0e-11],
                        target: 0.0,
                    },
                )
                .unwrap(),
            )
            .unwrap();
        problem
            .add_bound(
                CoordinateBound::new(variable, 0, Some(0.0), Some(0.0), "fixed x normal").unwrap(),
            )
            .unwrap();
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.rank, 1, "scale={scale:e}: {report:#?}");
        assert_eq!(report.right_nullity, 1, "scale={scale:e}");
        assert_eq!(
            report.bidirectional_degrees_of_freedom, 0,
            "scale={scale:e}"
        );
        assert_eq!(report.one_sided_mobility, OneSidedMobility::None);
    }
}

#[test]
fn diagnostic_trial_budget_never_omits_component_reports_and_counts_dimensions() {
    let mut problem = Problem::new();
    for target in [0.0, 2.0] {
        let variable = problem.add_variable(VariableBlock::scalar(target, 1.0).unwrap());
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Hard,
            target,
            "basis",
        );
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Hard,
            target,
            "duplicate",
        );
    }
    let report = problem
        .solve(SolverConfig {
            redundancy_diagnostic_budget: DiagnosticBudget {
                max_trials: 2,
                ..DiagnosticBudget::unlimited()
            },
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.component_solves.len(), 2);
    assert_eq!(
        report.redundancy_diagnostics.status,
        DiagnosticStatus::Truncated
    );
    assert_eq!(report.redundancy_diagnostics.consumed.components, 1);
    assert_eq!(report.redundancy_diagnostics.consumed.tangent_dimensions, 1);
    assert_eq!(report.redundancy_diagnostics.consumed.scalar_rows, 2);
    assert_eq!(report.redundancy_diagnostics.consumed.trials, 2);
    assert_eq!(
        report.redundancy_diagnostics.reason,
        Some(DiagnosticIncompleteReason::TrialBudget)
    );
}

#[test]
fn clean_session_components_freshly_validate_all_rows_without_rerunning_secondary_optimization() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let x_hard = Arc::new(AtomicUsize::new(0));
    let x_secondary = Arc::new(AtomicUsize::new(0));
    let y_hard = Arc::new(AtomicUsize::new(0));
    let y_secondary = Arc::new(AtomicUsize::new(0));
    for (variable, category, target, calls, label) in [
        (
            x,
            ResidualCategory::Hard,
            1.0,
            Arc::clone(&x_hard),
            "x hard",
        ),
        (
            x,
            ResidualCategory::Temporary,
            1.0,
            Arc::clone(&x_secondary),
            "x temporary",
        ),
        (
            y,
            ResidualCategory::Hard,
            2.0,
            Arc::clone(&y_hard),
            "y hard",
        ),
        (
            y,
            ResidualCategory::Temporary,
            2.0,
            Arc::clone(&y_secondary),
            "y temporary",
        ),
    ] {
        let source_id = source(&mut problem, label);
        problem
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    category,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row(label)],
                    CountingScalarTarget { target, calls },
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    for counter in [&x_hard, &x_secondary, &y_hard, &y_secondary] {
        counter.store(0, Ordering::Relaxed);
    }
    let mut patch = SessionPatch::new(session.revisions());
    patch.set_variable_value(x, VariableValue::Scalar(0.5));
    let transaction = session.apply(patch).unwrap();
    assert!(transaction.committed(), "{transaction:#?}");
    assert!(x_hard.load(Ordering::Relaxed) > 0);
    assert!(x_secondary.load(Ordering::Relaxed) > 0);
    assert!(y_hard.load(Ordering::Relaxed) > 0);
    assert!(y_secondary.load(Ordering::Relaxed) > 0);
    let y_component = transaction
        .report
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&y))
        .unwrap()
        .component_index;
    assert!(transaction.report.component_solves[y_component].reused);
    assert_eq!(
        transaction.report.component_solves[y_component].iterations,
        0
    );
    let reused_priority = transaction
        .report
        .priority_solves
        .iter()
        .find(|priority| {
            priority.component_index == Some(y_component)
                && priority.category == ResidualCategory::Temporary
        })
        .unwrap();
    assert_eq!(reused_priority.iterations, 0);
}

#[test]
fn explicitly_deep_cloned_evaluator_telemetry_isolated_from_rejected_candidate() {
    let instances = Arc::new(Mutex::new(Vec::new()));
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "stateful target");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![row("stateful target")],
                IsolatedStatefulTarget::new(1.0, Arc::clone(&instances)),
            )
            .unwrap(),
        )
        .unwrap();
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let accepted_counter = Arc::clone(&instances.lock().unwrap()[0]);
    let accepted_calls = accepted_counter.load(Ordering::Relaxed);
    let mut patch = SessionPatch::new(session.revisions());
    patch.set_variable_value(variable, VariableValue::Scalar(0.5));
    let (transaction, output) = session
        .apply_with_output(patch, |_, _| {
            Err::<(), _>(SessionDomainRejection::invalid("reject candidate"))
        })
        .unwrap();
    assert!(!transaction.committed());
    assert!(output.is_none());
    assert_eq!(accepted_counter.load(Ordering::Relaxed), accepted_calls);
    let counters = instances.lock().unwrap();
    assert!(counters.len() >= 2);
    assert!(
        counters
            .iter()
            .skip(1)
            .any(|counter| { counter.load(Ordering::Relaxed) > accepted_calls })
    );
}

#[test]
fn failed_bound_replacement_is_atomic_and_initial_guesses_are_projected() {
    let mut projected = Problem::new();
    let variable = projected.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap());
    projected
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), Some(1.0), "unit").unwrap())
        .unwrap();
    let report = projected.solve(SolverConfig::default()).unwrap();
    assert_eq!(scalar(&projected, variable).to_bits(), 0.0_f64.to_bits());
    assert_eq!(report.bounds[0].status, BoundStatus::ActiveLower);

    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Hard,
        1.0,
        "hard one",
    );
    let bound = problem
        .add_bound(CoordinateBound::new(variable, 0, Some(0.0), Some(2.0), "wide").unwrap())
        .unwrap();
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let before_state = session.problem().packed_state().unwrap();
    let before_report = session.report().clone();
    let before_revisions = session.revisions();
    let mut patch = SessionPatch::new(before_revisions);
    patch.replace_bound(
        bound,
        CoordinateBound::new(variable, 0, Some(0.0), Some(0.0), "incompatible fixed").unwrap(),
    );
    let rejected = session.apply(patch).unwrap();
    assert!(!rejected.committed());
    assert_eq!(session.problem().packed_state().unwrap(), before_state);
    assert_eq!(session.report(), &before_report);
    assert_eq!(session.revisions(), before_revisions);
    assert_eq!(session.problem().bound(bound).unwrap().upper(), Some(2.0));
}

#[test]
fn tiny_representable_interiors_are_not_snapped_to_bound_endpoints() {
    let mut problem = Problem::new();
    let variable =
        problem.add_variable(VariableBlock::scalar(f64::from_bits(2), f64::MIN_POSITIVE).unwrap());
    problem
        .add_bound(
            CoordinateBound::new(
                variable,
                0,
                Some(f64::from_bits(1)),
                None,
                "smallest positive",
            )
            .unwrap(),
        )
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(scalar(&problem, variable).to_bits(), 2);
    assert_eq!(report.bounds[0].status, BoundStatus::Inactive);
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_diagnostic_completeness_reason_is_distinct_and_dimensioned() {
    let base = |evaluator: ScalarTarget, budget: DiagnosticBudget| {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let source_id = source(&mut problem, "budget fixture");
        problem
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row("budget fixture")],
                    evaluator,
                )
                .unwrap(),
            )
            .unwrap();
        problem
            .solve(SolverConfig {
                redundancy_diagnostic_budget: budget,
                ..SolverConfig::default()
            })
            .unwrap()
    };
    for (budget, reason) in [
        (
            DiagnosticBudget {
                max_component_tangent_dimension: 0,
                ..DiagnosticBudget::unlimited()
            },
            DiagnosticIncompleteReason::ComponentTangentBudget,
        ),
        (
            DiagnosticBudget {
                max_component_scalar_rows: 0,
                ..DiagnosticBudget::unlimited()
            },
            DiagnosticIncompleteReason::ComponentRowBudget,
        ),
        (
            DiagnosticBudget {
                max_candidate_sources: 0,
                ..DiagnosticBudget::unlimited()
            },
            DiagnosticIncompleteReason::CandidateSourceBudget,
        ),
        (
            DiagnosticBudget {
                max_trials: 0,
                ..DiagnosticBudget::unlimited()
            },
            DiagnosticIncompleteReason::TrialBudget,
        ),
    ] {
        let report = base(ScalarTarget(0.0), budget);
        assert_eq!(
            report.redundancy_diagnostics.status,
            DiagnosticStatus::Skipped
        );
        assert_eq!(report.redundancy_diagnostics.reason, Some(reason));
    }

    let mut hard_invalid = Problem::new();
    let variable = hard_invalid.add_variable(VariableBlock::scalar(0.5, 1.0).unwrap());
    add_scalar_target(
        &mut hard_invalid,
        variable,
        ResidualCategory::Hard,
        0.0,
        "zero",
    );
    add_scalar_target(
        &mut hard_invalid,
        variable,
        ResidualCategory::Hard,
        1.0,
        "one",
    );
    let report = hard_invalid.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.redundancy_diagnostics.reason,
        Some(DiagnosticIncompleteReason::HardInvalid)
    );

    let invalid_evaluation = invalid_diagnostic_report(InvalidEvaluation);
    assert_eq!(
        invalid_evaluation.redundancy_diagnostics.reason,
        Some(DiagnosticIncompleteReason::InvalidEvaluation)
    );
    let invalid_rank = invalid_diagnostic_report(InvalidRank);
    assert_eq!(
        invalid_rank.redundancy_diagnostics.reason,
        Some(DiagnosticIncompleteReason::InvalidRank)
    );
}

#[test]
fn reused_components_preserve_truncated_diagnostic_evidence() {
    let mut problem = Problem::new();
    for target in [0.0, 2.0] {
        let variable = problem.add_variable(VariableBlock::scalar(target, 1.0).unwrap());
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Hard,
            target,
            "basis",
        );
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Hard,
            target,
            "duplicate",
        );
    }
    let config = SolverConfig {
        redundancy_diagnostic_budget: DiagnosticBudget {
            max_trials: 2,
            ..DiagnosticBudget::unlimited()
        },
        ..SolverConfig::default()
    };
    let mut session = SolveSession::new(problem, config).unwrap();
    let initial = session.report().redundancy_diagnostics;
    assert_eq!(initial.status, DiagnosticStatus::Truncated);
    let result = session
        .apply(SessionPatch::new(session.revisions()))
        .unwrap();
    assert!(result.committed());
    assert!(
        result
            .report
            .component_solves
            .iter()
            .all(|item| item.reused)
    );
    assert_eq!(result.report.redundancy_diagnostics, initial);
}

#[test]
fn accepted_audit_refresh_cannot_change_equations_and_advances_source_revision() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let (source_id, residual_id) = add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Hard,
        1.0,
        "original source",
    );
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let before = session.revisions();
    let state = session.report().accepted_state.clone();
    let mut refresh = AcceptedAuditPatch::new(before);
    refresh.replace_source(
        source_id,
        SourceConstraint::new("refreshed source").unwrap(),
    );
    refresh.replace_residual_rows(residual_id, vec![row("refreshed equation description")]);
    session.refresh_accepted_audit(refresh).unwrap();
    assert_eq!(session.revisions().source, before.source + 1);
    assert_eq!(session.revisions().state, before.state);
    assert_eq!(session.report().accepted_state, state);
    let audit = session
        .report()
        .audit
        .sources
        .iter()
        .find(|source| source.source_id == source_id)
        .unwrap();
    assert_eq!(audit.source_label, "refreshed source");
    assert_eq!(audit.rows[0].template, "refreshed equation description");

    let retained_report = session.report().clone();
    let retained_revisions = session.revisions();
    let mut invalid = AcceptedAuditPatch::new(retained_revisions);
    invalid.replace_residual_rows(
        residual_id,
        vec![ResidualRowAudit::new(
            " ",
            vec![AuditBinding::new("variable", "invalid")],
            "model-unit",
        )],
    );
    assert!(session.refresh_accepted_audit(invalid).is_err());
    assert_eq!(session.report(), &retained_report);
    assert_eq!(session.revisions(), retained_revisions);
}

#[test]
fn componentless_secondary_sources_support_normal_and_audit_session_patches() {
    let mut problem = Problem::new();
    let source_id = source(&mut problem, "constant secondary");
    let residual_id = problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Preference,
                Vec::new(),
                1,
                vec![1.0],
                vec![row("constant one")],
                ConstantSecondary(1.0),
            )
            .unwrap(),
        )
        .unwrap();
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();

    let mut audit = AcceptedAuditPatch::new(session.revisions());
    audit.replace_residual_rows(residual_id, vec![row("constant one refreshed")]);
    session.refresh_accepted_audit(audit).unwrap();
    assert_eq!(
        session.report().audit.sources[0].rows[0].template,
        "constant one refreshed"
    );

    let replacement = ResidualBlock::new(
        source_id,
        ResidualCategory::Preference,
        Vec::new(),
        1,
        vec![1.0],
        vec![row("constant two")],
        ConstantSecondary(2.0),
    )
    .unwrap();
    let mut patch = SessionPatch::new(session.revisions());
    patch.replace_residual(residual_id, replacement);
    let transaction = session.apply(patch).unwrap();
    assert!(transaction.committed(), "{transaction:#?}");
    assert_eq!(
        transaction.report.preference_status,
        SecondaryStatus::Acceptable
    );
    assert!((transaction.report.audit.sources[0].rows[0].raw_residual - 2.0).abs() <= f64::EPSILON);
}

#[test]
fn componentless_secondary_derivative_failure_cannot_enter_a_session_as_acceptable() {
    let mut problem = Problem::new();
    let source_id = source(&mut problem, "invalid constant secondary derivative");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Preference,
                Vec::new(),
                1,
                vec![1.0],
                vec![row("invalid constant secondary derivative")],
                ConstantSecondaryWithInvalidDerivative,
            )
            .unwrap(),
        )
        .unwrap();

    let error = SolveSession::new(problem, SolverConfig::default()).unwrap_err();
    assert!(matches!(
        error,
        SessionError::InitialRejected(geosolve_core::SessionCoreRejection::EvaluationFailure)
    ));
}

#[test]
fn exact_state_certification_rejects_elimination_canonicalization_without_mutation() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "fixed exact state");
    let residual_id = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                source_id,
                variable,
                VariableValue::Scalar(0.0),
                vec![1.0],
                vec![row("fixed exact state")],
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_fixed_variable(variable, VariableValue::Scalar(0.0), residual_id)
        .unwrap();
    problem
        .set_variable_value(variable, VariableValue::Scalar(2.0))
        .unwrap();
    let before = problem.packed_state().unwrap();
    let mut controller = OperationController::new(OperationControl::unlimited());

    let error = problem
        .certify_current_state_with_controller(SolverConfig::default(), &mut controller)
        .unwrap_err();

    assert!(matches!(
        error,
        CoreError::InvalidAcceptedLinearization {
            context: "materialized state requires fixed, alias, or bound canonicalization"
        }
    ));
    assert_eq!(problem.packed_state().unwrap(), before);
    assert_eq!(scalar(&problem, variable).to_bits(), 2.0_f64.to_bits());
}

#[test]
fn exact_state_certification_reports_invalid_geometry_without_mutation_or_publication() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "invalid exact state");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![row("invalid exact state")],
                InvalidEvaluation,
            )
            .unwrap(),
        )
        .unwrap();
    let before = problem.packed_state().unwrap();
    let mut controller = OperationController::new(OperationControl::unlimited());

    let report = problem
        .certify_current_state_with_controller(SolverConfig::default(), &mut controller)
        .unwrap()
        .unwrap();

    assert_eq!(report.termination, SolveTermination::InvalidGeometry);
    assert_eq!(report.hard_validity, HardValidity::Invalid);
    assert_eq!(
        report.audit.sources[0].rows[0].evaluation_status,
        AuditEvaluationStatus::Failed
    );
    assert_eq!(problem.packed_state().unwrap(), before);
    let error =
        SolveSession::from_accepted_report(problem, SolverConfig::default(), report).unwrap_err();
    assert!(matches!(
        error,
        SessionError::InitialRejected(geosolve_core::SessionCoreRejection::EvaluationFailure)
    ));
}
