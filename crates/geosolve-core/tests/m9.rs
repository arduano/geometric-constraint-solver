use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, CoreError, EvaluationError, EvaluationErrorCategory,
    HardValidity, LinearizationStorage, LocalJacobian, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SecondaryStatus, SolveTermination, SolverConfig,
    SourceConstraint, VariableBlock, VariableId, VariableValue,
};

fn rows(count: usize) -> Vec<ResidualRowAudit> {
    (0..count)
        .map(|index| {
            ResidualRowAudit::new(
                format!("M9 synthetic row {index}"),
                vec![AuditBinding::new("variables", "M9 fixture")],
                "model unit",
            )
        })
        .collect()
}

fn source(problem: &mut Problem, label: &str) -> geosolve_core::SourceConstraintId {
    problem.add_source(SourceConstraint::new(label).unwrap())
}

#[derive(Clone, Copy, Debug)]
struct MixedLegacy;

impl MixedLegacy {
    fn values(variables: &[VariableValue]) -> Result<[f64; 2], EvaluationError> {
        let [
            VariableValue::Scalar(scalar),
            VariableValue::Vec2(vector),
            VariableValue::Pose2(pose),
        ] = variables
        else {
            return Err(EvaluationError::invalid_geometry(
                "mixed fixture expected Scalar, Vec2, and Pose2",
            ));
        };
        Ok([
            scalar + vector[0] + pose[0],
            scalar * vector[1] + pose[2].sin(),
        ])
    }

    fn blocks(variables: &[VariableValue]) -> Result<[Vec<f64>; 3], EvaluationError> {
        let [
            VariableValue::Scalar(scalar),
            VariableValue::Vec2(vector),
            VariableValue::Pose2(pose),
        ] = variables
        else {
            return Err(EvaluationError::invalid_geometry(
                "mixed fixture expected Scalar, Vec2, and Pose2",
            ));
        };
        Ok([
            vec![1.0, vector[1]],
            vec![1.0, 0.0, 0.0, *scalar],
            vec![1.0, 0.0, 0.0, 0.0, 0.0, pose[2].cos()],
        ])
    }
}

impl ResidualEvaluator for MixedLegacy {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Ok(Self::values(variables)?.to_vec())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let blocks = Self::blocks(variables)?;
        Ok(vec![
            LocalJacobian::new(2, 1, blocks[0].clone()),
            LocalJacobian::new(2, 2, blocks[1].clone()),
            LocalJacobian::new(2, 3, blocks[2].clone()),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
struct MixedFused;

impl ResidualEvaluator for MixedFused {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        MixedLegacy.evaluate(variables)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        MixedLegacy.jacobian(variables)
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        Some((|| {
            storage
                .residuals_mut()
                .copy_from_slice(&MixedLegacy::values(variables)?);
            let blocks = MixedLegacy::blocks(variables)?;
            for (index, values) in blocks.iter().enumerate() {
                storage
                    .jacobian_block_mut(index)
                    .expect("declared mixed incidence")
                    .values_mut()
                    .copy_from_slice(values);
            }
            Ok(())
        })())
    }
}

fn mixed_problem(fused: bool) -> Problem {
    let mut problem = Problem::new();
    let scalar = problem.add_variable(VariableBlock::scalar(2.0, 0.5).unwrap());
    let vector = problem.add_variable(VariableBlock::vec2([3.0, 5.0], [2.0, 4.0]).unwrap());
    let pose =
        problem.add_variable(VariableBlock::pose2([7.0, 11.0, 0.4], [5.0, 6.0, 0.2]).unwrap());
    for category in [
        ResidualCategory::Hard,
        ResidualCategory::Temporary,
        ResidualCategory::Preference,
    ] {
        let source_id = source(&mut problem, &format!("{category:?} mixed"));
        if fused {
            problem
                .add_residual(
                    ResidualBlock::new(
                        source_id,
                        category,
                        vec![scalar, vector, pose],
                        2,
                        vec![10.0, 20.0],
                        rows(2),
                        MixedFused,
                    )
                    .unwrap(),
                )
                .unwrap();
        } else {
            problem
                .add_residual(
                    ResidualBlock::new(
                        source_id,
                        category,
                        vec![scalar, vector, pose],
                        2,
                        vec![10.0, 20.0],
                        rows(2),
                        MixedLegacy,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
    }
    problem
}

#[test]
fn fused_and_legacy_dense_jacobian_and_audit_paths_are_identical() {
    let mut legacy = mixed_problem(false);
    let mut fused = mixed_problem(true);
    let legacy_dense = legacy.assemble_dense().unwrap();
    let fused_dense = fused.assemble_dense().unwrap();
    assert_eq!(legacy_dense.residuals(), fused_dense.residuals());
    assert_eq!(legacy_dense.jacobian(), fused_dense.jacobian());
    assert!(legacy.check_jacobians(1.0e-6).unwrap().all_within(1.0e-6));
    assert!(fused.check_jacobians(1.0e-6).unwrap().all_within(1.0e-6));
    let legacy_audit = legacy.audit_snapshot().unwrap();
    let fused_audit = fused.audit_snapshot().unwrap();
    for (legacy_source, fused_source) in legacy_audit.sources.iter().zip(&fused_audit.sources) {
        assert_eq!(legacy_source.source_label, fused_source.source_label);
        for (legacy_row, fused_row) in legacy_source.rows.iter().zip(&fused_source.rows) {
            assert_eq!(
                legacy_row.raw_residual.to_bits(),
                fused_row.raw_residual.to_bits()
            );
            assert_eq!(
                legacy_row.normalized_residual.to_bits(),
                fused_row.normalized_residual.to_bits()
            );
            assert_eq!(legacy_row.category, fused_row.category);
        }
    }
    let legacy_report = legacy.solve(SolverConfig::default()).unwrap();
    let fused_report = fused.solve(SolverConfig::default()).unwrap();
    assert_eq!(legacy_report.termination, fused_report.termination);
    assert_eq!(legacy_report.hard_validity, fused_report.hard_validity);
    assert_eq!(
        legacy_report.hard_termination,
        fused_report.hard_termination
    );
    assert_eq!(
        legacy_report.temporary_status,
        fused_report.temporary_status
    );
    assert_eq!(
        legacy_report.preference_status,
        fused_report.preference_status
    );
    assert_eq!(legacy_report.rank, fused_report.rank);
    assert_eq!(legacy_report.right_nullity, fused_report.right_nullity);
    assert_eq!(
        legacy_report.accepted_state.ambient(),
        fused_report.accepted_state.ambient()
    );
    for (legacy_row, fused_row) in legacy_report
        .audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .zip(
            fused_report
                .audit
                .sources
                .iter()
                .flat_map(|source| &source.rows),
        )
    {
        assert_eq!(legacy_row.category, fused_row.category);
        assert_eq!(
            legacy_row.normalized_residual.to_bits(),
            fused_row.normalized_residual.to_bits()
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct IncompleteFused;

impl ResidualEvaluator for IncompleteFused {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Ok(vec![0.0, 0.0])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(2, 1, vec![1.0, 1.0])])
    }

    fn linearize(
        &self,
        _variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        storage.residuals_mut()[0] = 0.0;
        storage
            .jacobian_block_mut(0)
            .expect("one incidence")
            .values_mut()[0] = 1.0;
        Some(Ok(()))
    }
}

#[derive(Clone, Copy, Debug)]
struct MalformedLegacy;

impl ResidualEvaluator for MalformedLegacy {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Ok(vec![0.0])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 2, vec![1.0, 1.0])])
    }
}

fn one_residual_problem(evaluator: impl ResidualEvaluator + 'static, output: usize) -> Problem {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "one residual");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                output,
                vec![1.0; output],
                rows(output),
                evaluator,
            )
            .unwrap(),
        )
        .unwrap();
    problem
}

#[test]
fn fused_unwritten_slots_and_legacy_malformed_shapes_keep_error_classification() {
    assert!(matches!(
        one_residual_problem(IncompleteFused, 2).assemble_dense(),
        Err(CoreError::NonFiniteValue { .. })
    ));
    assert!(matches!(
        one_residual_problem(MalformedLegacy, 1).assemble_dense(),
        Err(CoreError::DimensionMismatch {
            context: "local Jacobian columns",
            ..
        })
    ));
}

#[derive(Clone, Copy, Debug)]
struct CategorizedJacobianFailure {
    fused: bool,
}

impl ResidualEvaluator for CategorizedJacobianFailure {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![value + 3.0])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Err(EvaluationError::nondifferentiable(
            "synthetic legacy Jacobian cusp",
        ))
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        if !self.fused {
            return None;
        }
        storage.residuals_mut()[0] = self.evaluate(variables).unwrap()[0];
        Some(Err(EvaluationError::ambiguous(
            "synthetic fused branch ambiguity",
        )))
    }
}

#[test]
fn audit_requires_jacobian_success_but_retains_fresh_values_and_fused_category() {
    for (fused, category, message) in [
        (
            false,
            EvaluationErrorCategory::Nondifferentiable,
            "legacy Jacobian cusp",
        ),
        (
            true,
            EvaluationErrorCategory::Ambiguous,
            "fused branch ambiguity",
        ),
    ] {
        let problem = one_residual_problem(CategorizedJacobianFailure { fused }, 1);
        assert!(matches!(
            problem.audit_snapshot(),
            Err(CoreError::CategorizedEvaluation {
                category: actual,
                ..
            }) if actual == category
        ));
        let partial = problem.audit_snapshot_partial();
        let row = &partial.sources[0].rows[0];
        assert_eq!(row.raw_residual.to_bits(), 3.0_f64.to_bits());
        assert_eq!(row.normalized_residual.to_bits(), 3.0_f64.to_bits());
        assert_eq!(row.evaluation_status, AuditEvaluationStatus::Failed);
        assert_eq!(row.evaluation_error_category, Some(category));
        assert!(row.evaluation_error.as_deref().unwrap().contains(message));
    }
}

#[derive(Clone, Copy, Debug)]
enum Failure {
    Structured(EvaluationErrorCategory),
    NonFinite,
}

#[derive(Clone, Copy, Debug)]
struct Failing(Failure);

impl ResidualEvaluator for Failing {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        match self.0 {
            Failure::Structured(EvaluationErrorCategory::Degenerate) => {
                Err(EvaluationError::degenerate("synthetic degeneracy"))
            }
            Failure::Structured(EvaluationErrorCategory::OutOfDomain) => {
                Err(EvaluationError::out_of_domain("synthetic domain escape"))
            }
            Failure::Structured(EvaluationErrorCategory::Nondifferentiable) => {
                Err(EvaluationError::nondifferentiable("synthetic corner"))
            }
            Failure::Structured(EvaluationErrorCategory::Ambiguous) => {
                Err(EvaluationError::ambiguous("synthetic branch ambiguity"))
            }
            Failure::Structured(_) => unreachable!("fixture enumerates every M9 category"),
            Failure::NonFinite => Ok(vec![f64::NAN]),
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[test]
fn structured_errors_are_invalid_geometry_with_identity_and_nonfinite_stays_numerical() {
    for category in [
        EvaluationErrorCategory::Degenerate,
        EvaluationErrorCategory::OutOfDomain,
        EvaluationErrorCategory::Nondifferentiable,
        EvaluationErrorCategory::Ambiguous,
    ] {
        let error = match category {
            EvaluationErrorCategory::Degenerate => EvaluationError::degenerate("message"),
            EvaluationErrorCategory::OutOfDomain => EvaluationError::out_of_domain("message"),
            EvaluationErrorCategory::Nondifferentiable => {
                EvaluationError::nondifferentiable("message")
            }
            EvaluationErrorCategory::Ambiguous => EvaluationError::ambiguous("message"),
            _ => unreachable!("fixture enumerates every M9 category"),
        };
        assert_eq!(error.category(), Some(category));
        assert_eq!(error.message(), "message");

        let mut problem = one_residual_problem(Failing(Failure::Structured(category)), 1);
        assert!(matches!(
            problem.assemble_dense(),
            Err(CoreError::CategorizedEvaluation {
                category: actual,
                message,
                ..
            }) if actual == category && !message.is_empty()
        ));
        let initial = problem.packed_state().unwrap();
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::InvalidGeometry);
        assert_eq!(report.hard_validity, HardValidity::Invalid);
        assert!(!report.hard_residuals_validated);
        assert_eq!(report.accepted_state, initial);
        let row = &report.audit.sources[0].rows[0];
        assert_eq!(row.evaluation_status, AuditEvaluationStatus::Failed);
        assert_eq!(row.evaluation_error_category, Some(category));
        assert!(
            row.evaluation_error
                .as_deref()
                .unwrap()
                .contains("residual")
        );
    }

    let mut problem = one_residual_problem(Failing(Failure::NonFinite), 1);
    let initial = problem.packed_state().unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::NumericalFailure);
    assert_eq!(report.hard_validity, HardValidity::NotEvaluated);
    assert_eq!(report.accepted_state, initial);
}

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
struct BranchLimitedTarget;

impl ResidualEvaluator for BranchLimitedTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        if *value == 0.0 {
            Ok(vec![-1.0])
        } else {
            Err(EvaluationError::out_of_domain("trial left fixed branch"))
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

fn add_scalar(
    problem: &mut Problem,
    variable: VariableId,
    category: ResidualCategory,
    evaluator: impl ResidualEvaluator + 'static,
) {
    let source_id = source(problem, &format!("{category:?} scalar"));
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                category,
                vec![variable],
                1,
                vec![1.0],
                rows(1),
                evaluator,
            )
            .unwrap(),
        )
        .unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn hard_and_secondary_statuses_are_orthogonal_for_all_core_outcomes() {
    let mut no_priorities = Problem::new();
    let variable = no_priorities.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_scalar(
        &mut no_priorities,
        variable,
        ResidualCategory::Hard,
        ScalarTarget(0.0),
    );
    let report = no_priorities.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.hard_termination, SolveTermination::Converged);
    assert_eq!(report.temporary_status, SecondaryStatus::NotRequested);
    assert_eq!(report.preference_status, SecondaryStatus::NotRequested);

    let mut fixed_only = Problem::new();
    let variable = fixed_only.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let fixed_source = source(&mut fixed_only, "fixed scalar");
    let fixed = fixed_only
        .add_residual(
            ResidualBlock::fixed_variable(
                fixed_source,
                variable,
                VariableValue::Scalar(2.0),
                vec![1.0],
                rows(1),
            )
            .unwrap(),
        )
        .unwrap();
    fixed_only
        .declare_fixed_variable(variable, VariableValue::Scalar(2.0), fixed)
        .unwrap();
    add_scalar(
        &mut fixed_only,
        variable,
        ResidualCategory::Preference,
        ScalarTarget(10.0),
    );
    let report = fixed_only.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.preference_status, SecondaryStatus::Acceptable);
    let zero_by_zero = report
        .component_solves
        .iter()
        .find(|component| {
            report.structural.component_summaries[component.component_index]
                .active_tangent_dimensions
                == 0
        })
        .unwrap();
    assert_eq!(
        (zero_by_zero.left_nullity, zero_by_zero.right_nullity),
        (0, 0)
    );
    assert_eq!(
        zero_by_zero.rank_threshold.to_bits(),
        f64::EPSILON.to_bits()
    );

    let mut optimal = Problem::new();
    let variable = optimal.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_scalar(
        &mut optimal,
        variable,
        ResidualCategory::Temporary,
        ScalarTarget(1.0),
    );
    let report = optimal.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.temporary_status, SecondaryStatus::Optimal);

    let mut limited = Problem::new();
    let variable = limited.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_scalar(
        &mut limited,
        variable,
        ResidualCategory::Temporary,
        ScalarTarget(1.0),
    );
    let report = limited
        .solve(SolverConfig {
            max_iterations: 1,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.temporary_status, SecondaryStatus::IterationLimit);
    assert_ne!(report.termination, SolveTermination::Converged);

    let mut stalled = Problem::new();
    let variable = stalled.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_scalar(
        &mut stalled,
        variable,
        ResidualCategory::Temporary,
        BranchLimitedTarget,
    );
    let report = stalled.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.temporary_status, SecondaryStatus::Stalled);

    let mut evaluation_failure = Problem::new();
    let variable = evaluation_failure.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_scalar(
        &mut evaluation_failure,
        variable,
        ResidualCategory::Preference,
        Failing(Failure::Structured(EvaluationErrorCategory::Degenerate)),
    );
    let report = evaluation_failure.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.hard_termination, SolveTermination::Converged);
    assert_eq!(report.preference_status, SecondaryStatus::EvaluationFailure);
    assert_eq!(report.termination, SolveTermination::InvalidGeometry);
}

#[derive(Clone, Debug)]
struct LinearRows {
    matrix: Vec<Vec<f64>>,
}

impl ResidualEvaluator for LinearRows {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(self
            .matrix
            .iter()
            .map(|row| row[0] * value[0] + row[1] * value[1])
            .collect())
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(
            self.matrix.len(),
            2,
            self.matrix.iter().flatten().copied().collect(),
        )])
    }
}

#[test]
fn rank_policy_reports_machine_floor_nullities_near_band_and_empty_shapes() {
    let mut floor = Problem::new();
    let variable = floor.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut floor, "machine floor row");
    floor
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                rows(1),
                ScalarCoefficient(1.0e-20),
            )
            .unwrap(),
        )
        .unwrap();
    let report = floor.solve(SolverConfig::default()).unwrap();
    let component = &report.component_solves[0];
    assert_eq!(component.rank, 0);
    assert_eq!((component.left_nullity, component.right_nullity), (1, 1));
    assert_eq!(
        component.rank_machine_tolerance.to_bits(),
        f64::EPSILON.to_bits()
    );
    assert_eq!(component.rank_threshold.to_bits(), f64::EPSILON.to_bits());
    assert_eq!(component.sigma_max.to_bits(), 1.0e-20_f64.to_bits());

    let mut near = Problem::new();
    let variable = near.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let source_id = source(&mut near, "near singular diagonal");
    near.add_residual(
        ResidualBlock::new(
            source_id,
            ResidualCategory::Hard,
            vec![variable],
            2,
            vec![1.0, 1.0],
            rows(2),
            LinearRows {
                matrix: vec![vec![1.0, 0.0], vec![0.0, 5.0e-9]],
            },
        )
        .unwrap(),
    )
    .unwrap();
    let report = near.solve(SolverConfig::default()).unwrap();
    let component = &report.component_solves[0];
    assert_eq!(component.rank, 2);
    assert_eq!((component.left_nullity, component.right_nullity), (0, 0));
    assert!(component.near_singular);
    assert!(component.near_singular_ratio.unwrap() <= component.near_singular_factor);
    assert!(report.near_singular);

    let mut empty = Problem::new();
    empty.add_variable(VariableBlock::scalar(3.0, 1.0).unwrap());
    let constant_source = source(&mut empty, "constant row");
    empty
        .add_residual(
            ResidualBlock::new(
                constant_source,
                ResidualCategory::Hard,
                Vec::new(),
                1,
                vec![1.0],
                rows(1),
                ConstantZero,
            )
            .unwrap(),
        )
        .unwrap();
    let report = empty.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.rank, 0);
    assert_eq!((report.left_nullity, report.right_nullity), (1, 1));
    assert_eq!(report.local_degrees_of_freedom, report.right_nullity);
    assert!(
        report
            .component_solves
            .iter()
            .all(|component| component.rank_threshold >= f64::EPSILON)
    );
}

#[test]
fn rank_boundaries_are_strict_component_local_and_rectangular() {
    let mut at_threshold = Problem::new();
    let variable = at_threshold.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let source_id = source(&mut at_threshold, "sigma equals tau");
    at_threshold
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![1.0, 1.0],
                rows(2),
                LinearRows {
                    matrix: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let report = at_threshold
        .solve(SolverConfig {
            rank_relative_tolerance: 1.0,
            ..SolverConfig::default()
        })
        .unwrap();
    let component = &report.component_solves[0];
    assert_eq!(component.rank_threshold.to_bits(), 1.0_f64.to_bits());
    assert!(
        component
            .singular_values
            .iter()
            .all(|value| value.to_bits() == 1.0_f64.to_bits())
    );
    assert_eq!(component.rank, 0, "sigma == tau must be excluded");
    assert_eq!((component.left_nullity, component.right_nullity), (2, 2));

    let mut at_near_boundary = Problem::new();
    let variable =
        at_near_boundary.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let source_id = source(&mut at_near_boundary, "exact near band boundary");
    at_near_boundary
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![1.0, 1.0],
                rows(2),
                LinearRows {
                    matrix: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let report = at_near_boundary
        .solve(SolverConfig {
            rank_relative_tolerance: 0.01,
            ..SolverConfig::default()
        })
        .unwrap();
    let component = &report.component_solves[0];
    assert_eq!(component.rank, 2);
    assert_eq!(component.near_singular_ratio, Some(100.0));
    assert!(
        component.near_singular,
        "the factor-100 boundary is inclusive"
    );

    let mut rectangular = Problem::new();
    let variable = rectangular.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let source_id = source(&mut rectangular, "machine-floor rectangular");
    rectangular
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                3,
                vec![1.0, 1.0, 1.0],
                rows(3),
                LinearRows {
                    matrix: vec![vec![1.0e-20, 0.0], vec![0.0, 1.0e-20], vec![0.0, 0.0]],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let report = rectangular.solve(SolverConfig::default()).unwrap();
    let component = &report.component_solves[0];
    assert_eq!(component.rank, 0);
    assert_eq!((component.left_nullity, component.right_nullity), (3, 2));
    assert_eq!(
        component.rank_machine_tolerance.to_bits(),
        (3.0 * f64::EPSILON).to_bits()
    );
    assert_eq!(
        component.rank_threshold.to_bits(),
        component.rank_machine_tolerance.to_bits()
    );
}

#[test]
fn disconnected_component_permutation_keeps_each_components_rank_policy() {
    for reverse in [false, true] {
        let mut problem = Problem::new();
        let add_near = |problem: &mut Problem| {
            let variable =
                problem.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
            let source_id = source(problem, "near component");
            problem
                .add_residual(
                    ResidualBlock::new(
                        source_id,
                        ResidualCategory::Hard,
                        vec![variable],
                        2,
                        vec![1.0, 1.0],
                        rows(2),
                        LinearRows {
                            matrix: vec![vec![1.0, 0.0], vec![0.0, 5.0e-9]],
                        },
                    )
                    .unwrap(),
                )
                .unwrap();
        };
        let add_large = |problem: &mut Problem| {
            let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
            let source_id = source(problem, "large component");
            problem
                .add_residual(
                    ResidualBlock::new(
                        source_id,
                        ResidualCategory::Hard,
                        vec![variable],
                        1,
                        vec![1.0],
                        rows(1),
                        ScalarCoefficient(1.0e12),
                    )
                    .unwrap(),
                )
                .unwrap();
        };
        if reverse {
            add_large(&mut problem);
            add_near(&mut problem);
        } else {
            add_near(&mut problem);
            add_large(&mut problem);
        }

        let report = problem.solve(SolverConfig::default()).unwrap();
        let near = report
            .component_solves
            .iter()
            .find(|component| component.singular_values.len() == 2)
            .unwrap();
        let large = report
            .component_solves
            .iter()
            .find(|component| component.singular_values.len() == 1)
            .unwrap();
        assert_eq!(near.rank, 2);
        assert!(near.near_singular);
        assert_eq!(near.sigma_max.to_bits(), 1.0_f64.to_bits());
        assert_eq!(large.rank, 1);
        assert_eq!(large.sigma_max.to_bits(), 1.0e12_f64.to_bits());
        assert_eq!(report.rank, 3);
    }
}

#[test]
fn normalized_rank_policy_is_scale_permutation_and_component_isolation_invariant() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for matrix in [
            vec![vec![1.0, 0.0], vec![0.0, 5.0e-9]],
            vec![vec![0.0, 5.0e-9], vec![1.0, 0.0]],
        ] {
            let mut problem = Problem::new();
            let near_variable =
                problem.add_variable(VariableBlock::vec2([0.0, 0.0], [scale, scale]).unwrap());
            let near_source = source(&mut problem, "normalized near component");
            problem
                .add_residual(
                    ResidualBlock::new(
                        near_source,
                        ResidualCategory::Hard,
                        vec![near_variable],
                        2,
                        vec![scale, scale],
                        rows(2),
                        LinearRows { matrix },
                    )
                    .unwrap(),
                )
                .unwrap();
            let large_variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
            let large_source = source(&mut problem, "disconnected large component");
            problem
                .add_residual(
                    ResidualBlock::new(
                        large_source,
                        ResidualCategory::Hard,
                        vec![large_variable],
                        1,
                        vec![1.0],
                        rows(1),
                        ScalarCoefficient(1.0e12),
                    )
                    .unwrap(),
                )
                .unwrap();
            let report = problem.solve(SolverConfig::default()).unwrap();
            let near_component = report
                .structural
                .component_summaries
                .iter()
                .find(|component| component.variable_ids.contains(&near_variable))
                .unwrap()
                .component_index;
            let near = &report.component_solves[near_component];
            assert_eq!(near.rank, 2, "scale={scale:e}");
            assert!(near.near_singular, "scale={scale:e}");
            assert!((near.sigma_max - 1.0).abs() <= 2.0e-15);
            assert!((near.rank_threshold - 1.0e-10).abs() <= 2.0e-25);
            assert!(near.near_singular_ratio.unwrap() <= 100.0);
            assert_eq!(report.rank, 3);
        }
    }
}

#[test]
fn reused_components_receive_fresh_hard_validity_and_rank_policy() {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::scalar(-1.0, 1.0).unwrap());
    let second = problem.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap());
    add_scalar(
        &mut problem,
        first,
        ResidualCategory::Hard,
        ScalarTarget(1.0),
    );
    add_scalar(
        &mut problem,
        second,
        ResidualCategory::Hard,
        ScalarTarget(2.0),
    );
    problem
        .solve_decomposed(SolverConfig::default(), &[])
        .unwrap();
    problem
        .set_variable_value(first, VariableValue::Scalar(-3.0))
        .unwrap();
    let report = problem
        .solve_decomposed(SolverConfig::default(), &[first])
        .unwrap();
    let reused = report
        .component_solves
        .iter()
        .find(|component| component.reused)
        .unwrap();
    assert_eq!(reused.hard_validity, HardValidity::Valid);
    assert_eq!(reused.hard_termination, SolveTermination::Converged);
    assert!(reused.hard_residuals_validated);
    assert!(reused.rank_is_valid);
    assert!(reused.rank_machine_tolerance > 0.0);
}

#[test]
fn redundancy_and_conflict_diagnostics_respect_machine_floor_rank() {
    let mut redundant = Problem::new();
    let variable = redundant.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    for label in ["machine-floor basis row", "machine-floor duplicate row"] {
        let source_id = source(&mut redundant, label);
        redundant
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    rows(1),
                    ScalarCoefficient(1.0e-15),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let redundant_source = redundant.audit_rows().unwrap()[1].source_id;
    let report = redundant
        .solve(SolverConfig {
            rank_relative_tolerance: 1.0e-30,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.rank, 1);
    assert_eq!(
        report.component_solves[0].rank_threshold.to_bits(),
        report.component_solves[0].rank_machine_tolerance.to_bits()
    );
    assert!(report.redundant_sources.contains(&redundant_source));
    assert!(
        report
            .sources_containing_redundant_rows
            .contains(&redundant_source)
    );

    let mut conflicting = Problem::new();
    let variable = conflicting.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let conflicting_source = source(&mut conflicting, "below-floor conflicting row");
    conflicting
        .add_residual(
            ResidualBlock::new(
                conflicting_source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                rows(1),
                ScalarAffine {
                    coefficient: 1.0e-20,
                    target: 1.0,
                },
            )
            .unwrap(),
        )
        .unwrap();
    let report = conflicting.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.rank, 0);
    assert_eq!(report.hard_validity, HardValidity::Invalid);
    assert!(report.conflicting_sources.contains(&conflicting_source));
}

#[derive(Clone, Copy, Debug)]
struct ScalarCoefficient(f64);

impl ResidualEvaluator for ScalarCoefficient {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![self.0 * value])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![self.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarAffine {
    coefficient: f64,
    target: f64,
}

impl ResidualEvaluator for ScalarAffine {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![self.coefficient * value - self.target])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![self.coefficient])])
    }
}

#[derive(Clone, Copy, Debug)]
struct ConstantZero;

impl ResidualEvaluator for ConstantZero {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if variables.is_empty() {
            Ok(vec![0.0])
        } else {
            Err(EvaluationError::invalid_geometry("expected no variables"))
        }
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        if variables.is_empty() {
            Ok(Vec::new())
        } else {
            Err(EvaluationError::invalid_geometry("expected no variables"))
        }
    }
}
