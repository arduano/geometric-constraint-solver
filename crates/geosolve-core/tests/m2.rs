use approx::assert_relative_eq;
use geosolve_core::{
    AuditBinding, EvaluationError, LocalJacobian, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SolveTermination, SolverConfig, SourceConstraint,
    SourceConstraintId, VariableBlock, VariableId, VariableValue,
};

const FD_STEP: f64 = 1.0e-5;
const FD_TOLERANCE: f64 = 1.0e-6;
const CHARACTERISTIC_SCALES: [f64; 3] = [1.0e-6, 1.0, 1.0e6];

fn audit_row(template: &str, binding: &str, unit: &str) -> ResidualRowAudit {
    ResidualRowAudit::new(
        template,
        vec![AuditBinding::new(binding, "synthetic variable")],
        unit,
    )
}

fn add_source(problem: &mut Problem, label: &str) -> SourceConstraintId {
    problem.add_source(SourceConstraint::new(label).unwrap())
}

fn scalar_value(problem: &Problem, variable: VariableId) -> f64 {
    let VariableValue::Scalar(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected scalar variable")
    };
    value
}

fn vec2_value(problem: &Problem, variable: VariableId) -> [f64; 2] {
    let VariableValue::Vec2(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected Vec2 variable")
    };
    value
}

fn assert_jacobians(problem: &Problem) {
    let report = problem.check_jacobians(FD_STEP).unwrap();
    assert!(report.all_within(FD_TOLERANCE), "{report:#?}");
}

#[derive(Clone, Debug)]
struct Affine2 {
    matrix: [[f64; 2]; 2],
    target: [f64; 2],
}

impl ResidualEvaluator for Affine2 {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![
            self.matrix[0][0] * value[0] + self.matrix[0][1] * value[1] - self.target[0],
            self.matrix[1][0] * value[0] + self.matrix[1][1] * value[1] - self.target[1],
        ])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(
            2,
            2,
            vec![
                self.matrix[0][0],
                self.matrix[0][1],
                self.matrix[1][0],
                self.matrix[1][1],
            ],
        )])
    }
}

#[derive(Clone, Debug)]
struct LinearRow {
    coefficients: [f64; 2],
    target: f64,
}

impl ResidualEvaluator for LinearRow {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![
            self.coefficients[0] * value[0] + self.coefficients[1] * value[1] - self.target,
        ])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 2, self.coefficients.to_vec())])
    }
}

#[derive(Clone, Debug)]
struct CircleDistance {
    center: [f64; 2],
    radius: f64,
}

impl ResidualEvaluator for CircleDistance {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![
            (point[0] - self.center[0]).hypot(point[1] - self.center[1]) - self.radius,
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        let delta = [point[0] - self.center[0], point[1] - self.center[1]];
        let distance = delta[0].hypot(delta[1]);
        if distance <= 1.0e-14 {
            return Err(EvaluationError::invalid_geometry(
                "circle-distance derivative is undefined at the center",
            ));
        }
        Ok(vec![LocalJacobian::new(
            1,
            2,
            vec![delta[0] / distance, delta[1] / distance],
        )])
    }
}

#[derive(Clone, Debug)]
struct CircleIntersection {
    first_center: [f64; 2],
    first_radius: f64,
    second_center: [f64; 2],
    second_radius: f64,
}

impl CircleIntersection {
    fn row(point: [f64; 2], center: [f64; 2], radius: f64) -> (f64, [f64; 2]) {
        let delta = [point[0] - center[0], point[1] - center[1]];
        let distance = delta[0].hypot(delta[1]);
        (
            distance - radius,
            [delta[0] / distance, delta[1] / distance],
        )
    }
}

impl ResidualEvaluator for CircleIntersection {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        let first = (point[0] - self.first_center[0]).hypot(point[1] - self.first_center[1])
            - self.first_radius;
        let second = (point[0] - self.second_center[0]).hypot(point[1] - self.second_center[1])
            - self.second_radius;
        Ok(vec![first, second])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        let first_distance =
            (point[0] - self.first_center[0]).hypot(point[1] - self.first_center[1]);
        let second_distance =
            (point[0] - self.second_center[0]).hypot(point[1] - self.second_center[1]);
        if first_distance <= 1.0e-14 || second_distance <= 1.0e-14 {
            return Err(EvaluationError::invalid_geometry(
                "circle-distance derivative is undefined at a center",
            ));
        }
        let (_, first) = Self::row(*point, self.first_center, self.first_radius);
        let (_, second) = Self::row(*point, self.second_center, self.second_radius);
        Ok(vec![LocalJacobian::new(
            2,
            2,
            vec![first[0], first[1], second[0], second[1]],
        )])
    }
}

#[derive(Clone, Debug)]
struct ScalarTarget(f64);

impl ResidualEvaluator for ScalarTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![value - self.0])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Debug)]
struct Product;

impl ResidualEvaluator for Product {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![value[0] * value[1]])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![LocalJacobian::new(1, 2, vec![value[1], value[0]])])
    }
}

#[derive(Clone, Debug)]
struct ConstantResidual(f64);

impl ResidualEvaluator for ConstantResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(_)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![self.0])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![0.0])])
    }
}

#[derive(Clone, Debug)]
struct Quadratic(f64);

impl ResidualEvaluator for Quadratic {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![value * value - self.0])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![2.0 * value])])
    }
}

#[derive(Clone, Copy, Debug)]
enum SolveFailure {
    InvalidGeometry,
    NonFiniteJacobian,
}

#[derive(Clone, Debug)]
struct FailingEvaluator(SolveFailure);

impl ResidualEvaluator for FailingEvaluator {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        match self.0 {
            SolveFailure::InvalidGeometry => {
                Err(EvaluationError::invalid_geometry("synthetic degeneracy"))
            }
            SolveFailure::NonFiniteJacobian => Ok(vec![1.0]),
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![f64::NAN])])
    }
}

fn add_scalar_residual(
    problem: &mut Problem,
    source: SourceConstraintId,
    variable: VariableId,
    category: ResidualCategory,
    target: f64,
    scale: f64,
) {
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                category,
                vec![variable],
                1,
                vec![scale],
                vec![audit_row("(x - target) / scale", "x", "m")],
                ScalarTarget(target),
            )
            .unwrap(),
        )
        .unwrap();
}

fn circle_problem(characteristic_scale: f64, initial: [f64; 2]) -> (Problem, VariableId) {
    let mut problem = Problem::new();
    let point = problem.add_variable(
        VariableBlock::vec2(initial, [characteristic_scale, characteristic_scale]).unwrap(),
    );
    let source = add_source(&mut problem, "two-circle intersection");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![point],
                2,
                vec![characteristic_scale, characteristic_scale],
                vec![
                    audit_row("(distance(P, C0) - r0) / scale", "P", "m"),
                    audit_row("(distance(P, C1) - r1) / scale", "P", "m"),
                ],
                CircleIntersection {
                    first_center: [0.0, 0.0],
                    first_radius: 5.0 * characteristic_scale,
                    second_center: [4.0 * characteristic_scale, 0.0],
                    second_radius: 3.0 * characteristic_scale,
                },
            )
            .unwrap(),
        )
        .unwrap();
    (problem, point)
}

fn exact_problem(scale: f64) -> (Problem, VariableId) {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, 0.0], [scale, scale]).unwrap());
    let source = add_source(&mut problem, "linear pair");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![scale, scale],
                vec![
                    audit_row("x + 2y - 5", "x,y", "m"),
                    audit_row("3x - y - 4", "x,y", "m"),
                ],
                Affine2 {
                    matrix: [[1.0, 2.0], [3.0, -1.0]],
                    target: [5.0 * scale, 4.0 * scale],
                },
            )
            .unwrap(),
        )
        .unwrap();
    (problem, variable)
}

fn underdetermined_problem(scale: f64) -> (Problem, VariableId) {
    let mut problem = Problem::new();
    let variable = problem.add_variable(
        VariableBlock::vec2([3.5 * scale, 3.0 * scale], [5.0 * scale, 5.0 * scale]).unwrap(),
    );
    let source = add_source(&mut problem, "point on radius-five circle");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![5.0 * scale],
                vec![audit_row("(distance(P, C) - radius) / scale", "P", "m")],
                CircleDistance {
                    center: [0.0, 0.0],
                    radius: 5.0 * scale,
                },
            )
            .unwrap(),
        )
        .unwrap();
    (problem, variable)
}

fn duplicate_problem(scale: f64) -> (Problem, VariableId, SourceConstraintId, SourceConstraintId) {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
    let first = add_source(&mut problem, "first x equals 2");
    let duplicate = add_source(&mut problem, "duplicate x equals 2");
    add_scalar_residual(
        &mut problem,
        first,
        variable,
        ResidualCategory::Hard,
        2.0 * scale,
        scale,
    );
    add_scalar_residual(
        &mut problem,
        duplicate,
        variable,
        ResidualCategory::Hard,
        2.0 * scale,
        scale,
    );
    (problem, variable, first, duplicate)
}

fn contradictory_problem(
    scale: f64,
) -> (Problem, VariableId, SourceConstraintId, SourceConstraintId) {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.2 * scale, scale).unwrap());
    let zero = add_source(&mut problem, "x equals zero");
    let one = add_source(&mut problem, "x equals one");
    add_scalar_residual(
        &mut problem,
        zero,
        variable,
        ResidualCategory::Hard,
        0.0,
        scale,
    );
    add_scalar_residual(
        &mut problem,
        one,
        variable,
        ResidualCategory::Hard,
        scale,
        scale,
    );
    (problem, variable, zero, one)
}

fn rank_drop_problem(scale: f64, normalized_initial: [f64; 2]) -> (Problem, VariableId) {
    let mut problem = Problem::new();
    let variable = problem.add_variable(
        VariableBlock::vec2(
            [normalized_initial[0] * scale, normalized_initial[1] * scale],
            [scale, scale],
        )
        .unwrap(),
    );
    let source = add_source(&mut problem, "product equals zero");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![scale * scale],
                vec![audit_row("x * y", "x,y", "m^2")],
                Product,
            )
            .unwrap(),
        )
        .unwrap();
    (problem, variable)
}

#[test]
fn exactly_determined_linear_system_converges() {
    let (mut problem, variable) = exact_problem(1.0);
    assert_jacobians(&problem);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert_eq!(report.rank, 2);
    assert_eq!(report.local_degrees_of_freedom, 0);
    assert!(!report.is_singular);
    let value = vec2_value(&problem, variable);
    assert_relative_eq!(value[0], 13.0 / 7.0, epsilon = 1.0e-9);
    assert_relative_eq!(value[1], 11.0 / 7.0, epsilon = 1.0e-9);
}

#[test]
fn nonlinear_circle_system_converges_from_three_nearby_guesses() {
    let documented_guesses = [[3.8, 2.7], [4.2, 3.2], [3.6, 3.4]];
    for initial in documented_guesses {
        let (mut problem, point) = circle_problem(1.0, initial);
        assert_jacobians(&problem);
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(
            report.termination,
            SolveTermination::Converged,
            "{initial:?}"
        );
        assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
        assert_eq!(report.rank, 2);
        let value = vec2_value(&problem, point);
        assert_relative_eq!(value[0], 4.0, epsilon = 2.0e-9);
        assert_relative_eq!(value[1], 3.0, epsilon = 2.0e-9);
    }
}

#[test]
fn one_equation_two_variables_converges_with_one_dof() {
    let (mut problem, variable) = underdetermined_problem(1.0);
    assert_jacobians(&problem);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.rank, 1);
    assert_eq!(report.local_degrees_of_freedom, 1);
    assert!(!report.is_singular);
    let value = vec2_value(&problem, variable);
    assert_relative_eq!(value[0].hypot(value[1]), 5.0, epsilon = 5.0e-9);
}

#[test]
fn duplicate_rows_converge_and_name_the_later_source_as_redundant() {
    let (mut problem, variable, _first, duplicate) = duplicate_problem(1.0);
    assert_jacobians(&problem);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_relative_eq!(scalar_value(&problem, variable), 2.0, epsilon = 1.0e-9);
    assert_eq!(report.rank, 1);
    assert!(!report.is_singular);
    assert_eq!(report.redundant_sources, vec![duplicate]);
    assert!(report.conflicting_sources.is_empty());
}

#[test]
fn redundancy_uses_complete_nonzero_source_groups() {
    let mut grouped = Problem::new();
    let variable = grouped.add_variable(VariableBlock::vec2([1.0, 1.0], [1.0, 1.0]).unwrap());
    let first = add_source(&mut grouped, "x equals zero");
    grouped
        .add_residual(
            ResidualBlock::new(
                first,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![audit_row("x", "x,y", "1")],
                LinearRow {
                    coefficients: [1.0, 0.0],
                    target: 0.0,
                },
            )
            .unwrap(),
        )
        .unwrap();
    let multi_row = add_source(&mut grouped, "x and y equal zero");
    grouped
        .add_residual(
            ResidualBlock::new(
                multi_row,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![1.0, 1.0],
                vec![audit_row("x", "x,y", "1"), audit_row("y", "x,y", "1")],
                Affine2 {
                    matrix: [[1.0, 0.0], [0.0, 1.0]],
                    target: [0.0, 0.0],
                },
            )
            .unwrap(),
        )
        .unwrap();
    assert_jacobians(&grouped);
    let grouped_report = grouped.solve(SolverConfig::default()).unwrap();
    assert_eq!(grouped_report.termination, SolveTermination::Converged);
    assert_eq!(grouped_report.rank, 2);
    assert!(!grouped_report.redundant_sources.contains(&multi_row));
    assert!(grouped_report.redundant_sources.is_empty());

    let mut near_zero = Problem::new();
    let variable = near_zero.add_variable(VariableBlock::vec2([1.0, 0.0], [1.0, 1.0]).unwrap());
    let first = add_source(&mut near_zero, "ordinary x row");
    let tiny = add_source(&mut near_zero, "near-zero x row");
    for (source, coefficient) in [(first, 1.0), (tiny, 1.0e-14)] {
        near_zero
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![audit_row("coefficient * x", "x,y", "1")],
                    LinearRow {
                        coefficients: [coefficient, 0.0],
                        target: 0.0,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    assert_jacobians(&near_zero);
    let near_zero_report = near_zero.solve(SolverConfig::default()).unwrap();
    assert_eq!(near_zero_report.termination, SolveTermination::Converged);
    assert!(!near_zero_report.redundant_sources.contains(&tiny));
    assert!(near_zero_report.redundant_sources.is_empty());
}

#[test]
fn contradictory_rows_do_not_converge_or_commit_non_finite_state() {
    let (mut problem, variable, zero, one) = contradictory_problem(1.0);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_ne!(report.termination, SolveTermination::Converged);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max >= 0.49);
    assert!(scalar_value(&problem, variable).is_finite());
    assert!(
        report
            .accepted_state
            .ambient()
            .iter()
            .all(|value| value.is_finite())
    );
    assert!(report.redundant_sources.is_empty());
    assert_eq!(report.conflicting_sources, vec![zero, one]);
}

#[test]
fn configuration_dependent_rank_drop_is_separate_from_termination() {
    for (initial, expected_rank, expected_dof, expected_singular) in
        [([0.0, 0.0], 0, 2, true), ([0.0, 1.0], 1, 1, false)]
    {
        let (mut problem, _variable) = rank_drop_problem(1.0, initial);
        assert_jacobians(&problem);

        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged);
        assert_eq!(report.rank, expected_rank);
        assert_eq!(report.local_degrees_of_freedom, expected_dof);
        assert_eq!(report.is_singular, expected_singular);
        assert!(report.redundant_sources.is_empty());
        assert_relative_eq!(
            report.rank_relative_tolerance,
            1.0e-10,
            epsilon = f64::EPSILON
        );
    }
}

#[test]
fn iteration_limit_and_stagnation_keep_the_last_finite_accepted_state() {
    let mut limited = Problem::new();
    let limited_variable = limited.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let source = add_source(&mut limited, "quadratic equals two");
    limited
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![limited_variable],
                1,
                vec![1.0],
                vec![audit_row("x^2 - 2", "x", "1")],
                Quadratic(2.0),
            )
            .unwrap(),
        )
        .unwrap();
    assert_jacobians(&limited);
    let limited_report = limited
        .solve(SolverConfig {
            max_iterations: 1,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(limited_report.termination, SolveTermination::IterationLimit);
    assert_eq!(limited_report.iterations, 1);
    assert!(limited_report.trace.records[0].accepted);
    assert!(scalar_value(&limited, limited_variable).is_finite());
    assert_eq!(
        limited_report.accepted_state.ambient().as_slice(),
        limited.packed_state().unwrap().ambient().as_slice()
    );

    let mut stalled = Problem::new();
    let stalled_variable = stalled.add_variable(VariableBlock::scalar(7.0, 1.0).unwrap());
    let source = add_source(&mut stalled, "constant impossible row");
    stalled
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![stalled_variable],
                1,
                vec![1.0],
                vec![audit_row("1", "x", "1")],
                ConstantResidual(1.0),
            )
            .unwrap(),
        )
        .unwrap();
    assert_jacobians(&stalled);
    let stalled_report = stalled.solve(SolverConfig::default()).unwrap();
    assert_eq!(stalled_report.termination, SolveTermination::Stalled);
    assert_relative_eq!(
        scalar_value(&stalled, stalled_variable),
        7.0,
        epsilon = f64::EPSILON
    );
    assert_eq!(stalled_report.iterations, 1);
    assert!(!stalled_report.trace.records[0].accepted);
    assert!(stalled_report.hard_residual_max > 0.0);
}

#[test]
fn characteristic_scales_preserve_classification_and_normalized_accuracy() {
    for scale in CHARACTERISTIC_SCALES {
        let (mut problem, point) = circle_problem(scale, [3.8 * scale, 2.7 * scale]);
        assert_jacobians(&problem);
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged, "{scale}");
        assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
        assert_eq!(report.rank, 2);
        assert_eq!(report.local_degrees_of_freedom, 0);
        assert!(!report.is_singular);
        assert!(report.redundant_sources.is_empty());
        let value = vec2_value(&problem, point);
        assert_relative_eq!(value[0] / scale, 4.0, epsilon = 2.0e-9);
        assert_relative_eq!(value[1] / scale, 3.0, epsilon = 2.0e-9);

        let (mut problem, variable) = exact_problem(scale);
        assert_jacobians(&problem);
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged, "{scale}");
        assert!(report.hard_residual_max <= 1.0e-9);
        assert_eq!((report.rank, report.local_degrees_of_freedom), (2, 0));
        assert!(!report.is_singular);
        assert!(report.redundant_sources.is_empty());
        let value = vec2_value(&problem, variable);
        assert_relative_eq!(value[0] / scale, 13.0 / 7.0, epsilon = 1.0e-9);
        assert_relative_eq!(value[1] / scale, 11.0 / 7.0, epsilon = 1.0e-9);

        let (mut problem, variable) = underdetermined_problem(scale);
        assert_jacobians(&problem);
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged, "{scale}");
        assert!(report.hard_residual_max <= 1.0e-9);
        assert_eq!((report.rank, report.local_degrees_of_freedom), (1, 1));
        assert!(!report.is_singular);
        assert!(report.redundant_sources.is_empty());
        let value = vec2_value(&problem, variable);
        assert_relative_eq!(value[0].hypot(value[1]) / scale, 5.0, epsilon = 5.0e-9);

        let (mut problem, variable, first, duplicate) = duplicate_problem(scale);
        assert_jacobians(&problem);
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged, "{scale}");
        assert!(report.hard_residual_max <= 1.0e-9);
        assert_eq!((report.rank, report.local_degrees_of_freedom), (1, 0));
        assert!(!report.is_singular);
        assert_eq!(report.redundant_sources, vec![duplicate]);
        assert_eq!(
            report
                .audit
                .sources
                .iter()
                .map(|source| source.source_id)
                .collect::<Vec<_>>(),
            vec![first, duplicate]
        );
        assert_relative_eq!(
            scalar_value(&problem, variable) / scale,
            2.0,
            epsilon = 1.0e-9
        );

        let (mut problem, variable, zero, one) = contradictory_problem(scale);
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Stalled, "{scale}");
        assert!(report.hard_residuals_validated);
        assert!(report.hard_residual_max >= 0.49);
        assert_eq!((report.rank, report.local_degrees_of_freedom), (1, 0));
        assert!(!report.is_singular);
        assert!(report.redundant_sources.is_empty());
        assert_eq!(
            report
                .audit
                .sources
                .iter()
                .map(|source| source.source_id)
                .collect::<Vec<_>>(),
            vec![zero, one]
        );
        assert!(scalar_value(&problem, variable).is_finite());

        for (initial, expected_rank, expected_dof, expected_singular) in
            [([0.0, 0.0], 0, 2, true), ([0.0, 1.0], 1, 1, false)]
        {
            let (mut problem, _variable) = rank_drop_problem(scale, initial);
            assert_jacobians(&problem);
            let report = problem.solve(SolverConfig::default()).unwrap();
            assert_eq!(report.termination, SolveTermination::Converged, "{scale}");
            assert!(report.hard_residual_max <= 1.0e-9);
            assert_eq!(report.rank, expected_rank);
            assert_eq!(report.local_degrees_of_freedom, expected_dof);
            assert_eq!(report.is_singular, expected_singular);
            assert!(report.redundant_sources.is_empty());
        }
    }
}

#[test]
fn trace_bookkeeping_and_audit_match_the_returned_accepted_state() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.1, 1.0).unwrap());
    let hard_source = add_source(&mut problem, "quadratic unit root");
    problem
        .add_residual(
            ResidualBlock::new(
                hard_source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![audit_row("x^2 - 1", "x", "1")],
                Quadratic(1.0),
            )
            .unwrap(),
        )
        .unwrap();
    let temporary_source = add_source(&mut problem, "unused temporary target");
    add_scalar_residual(
        &mut problem,
        temporary_source,
        variable,
        ResidualCategory::Temporary,
        100.0,
        1.0,
    );
    assert_jacobians(&problem);

    let report = problem
        .solve(SolverConfig {
            initial_damping: 1.0e-8,
            max_block_normalized_step: 10.0,
            max_iterations: 100,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!(report.trace.records.iter().any(|record| record.accepted));
    assert!(report.trace.records.iter().any(|record| !record.accepted));
    for (index, record) in report.trace.records.iter().enumerate() {
        assert_eq!(record.iteration, index + 1);
        assert!(record.cost.is_finite());
        assert!(record.damping.is_finite());
        assert!(record.actual_reduction.is_finite());
        assert!(record.predicted_reduction.is_finite());
        assert!(record.reduction_ratio.is_finite());
        if record.accepted {
            assert!(record.cost <= record.cost_before);
            assert_relative_eq!(record.cost, record.trial_cost, epsilon = f64::EPSILON);
        } else {
            assert_relative_eq!(record.cost, record.cost_before, epsilon = f64::EPSILON);
        }
    }

    assert_eq!(
        report.accepted_state.ambient().as_slice(),
        problem.packed_state().unwrap().ambient().as_slice()
    );
    let mut unannotated_report_audit = report.audit.clone();
    for source in &mut unannotated_report_audit.sources {
        source.annotations = geosolve_core::AuditAnnotations::default();
        source.active_bounds.clear();
        for row in &mut source.rows {
            row.annotations = geosolve_core::AuditAnnotations::default();
            row.active_bounds.clear();
        }
    }
    assert_eq!(unannotated_report_audit, problem.audit_snapshot().unwrap());
    assert_eq!(report.audit.sources.len(), 2);
    for source in &report.audit.sources {
        for row in &source.rows {
            assert_relative_eq!(
                row.normalized_residual,
                row.raw_residual / row.scale,
                epsilon = f64::EPSILON
            );
        }
    }
    let hard_row = &report.audit.sources[0].rows[0];
    assert_eq!(hard_row.bindings[0].value, "synthetic variable");
    assert_eq!(hard_row.incident_variables.len(), 1);
    assert_eq!(hard_row.incident_variables[0].variable_id, variable);
    let VariableValue::Scalar(audited_value) = hard_row.incident_variables[0].value else {
        panic!("expected audited scalar value")
    };
    assert_relative_eq!(
        audited_value,
        scalar_value(&problem, variable),
        epsilon = f64::EPSILON
    );
    assert!((audited_value - 0.1).abs() > 0.5);
    assert_relative_eq!(
        scalar_value(&problem, variable).abs(),
        1.0,
        epsilon = 1.0e-9
    );
    let temporary_row = &report.audit.sources[1].rows[0];
    assert_eq!(temporary_row.category, ResidualCategory::Temporary);
    assert!(temporary_row.normalized_residual.abs() > 90.0);
    assert!(report.hard_residual_max <= 1.0e-9);
}

#[test]
fn invalid_geometry_and_numerical_failure_preserve_the_initial_finite_state() {
    for (mode, expected) in [
        (
            SolveFailure::InvalidGeometry,
            SolveTermination::InvalidGeometry,
        ),
        (
            SolveFailure::NonFiniteJacobian,
            SolveTermination::NumericalFailure,
        ),
    ] {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
        let source = add_source(&mut problem, "failing evaluator");
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![audit_row("failure", "x", "1")],
                    FailingEvaluator(mode),
                )
                .unwrap(),
            )
            .unwrap();

        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, expected);
        assert!(report.trace.records.is_empty());
        assert_relative_eq!(
            scalar_value(&problem, variable),
            2.0,
            epsilon = f64::EPSILON
        );
        assert!(
            report
                .accepted_state
                .ambient()
                .iter()
                .all(|value| value.is_finite())
        );
    }
}
