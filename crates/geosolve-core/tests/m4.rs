use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, CoreError, EvaluationError, LocalJacobian, Problem,
    RedundancyKind, ResidualBlock, ResidualCategory, ResidualEvaluator, ResidualId,
    ResidualRowAudit, SolveTermination, SolverConfig, SourceConstraint, SourceConstraintId,
    VariableBlock, VariableId, VariableKind, VariableValue,
};

fn audit_rows(count: usize, label: &str) -> Vec<ResidualRowAudit> {
    (0..count)
        .map(|row| {
            ResidualRowAudit::new(
                format!("{label} row {row}"),
                vec![AuditBinding::new("variables", "M4 synthetic fixture")],
                "model unit",
            )
        })
        .collect()
}

fn source(problem: &mut Problem, label: &str) -> SourceConstraintId {
    problem.add_source(SourceConstraint::new(label).unwrap())
}

fn scalar(problem: &Problem, variable: VariableId) -> f64 {
    let VariableValue::Scalar(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected scalar")
    };
    value
}

fn vec2(problem: &Problem, variable: VariableId) -> [f64; 2] {
    let VariableValue::Vec2(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected Vec2")
    };
    value
}

#[derive(Clone, Debug)]
struct ScalarRows(Vec<f64>);

impl ResidualEvaluator for ScalarRows {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(self.0.iter().map(|target| value - target).collect())
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(
            self.0.len(),
            1,
            vec![1.0; self.0.len()],
        )])
    }
}

#[derive(Clone, Copy, Debug)]
struct Vec2Target([f64; 2]);

impl ResidualEvaluator for Vec2Target {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![value[0] - self.0[0], value[1] - self.0[1]])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarDifference;

impl ResidualEvaluator for ScalarDifference {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![first - second])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![
            LocalJacobian::new(1, 1, vec![1.0]),
            LocalJacobian::new(1, 1, vec![-1.0]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
struct ConstantZero;

impl ResidualEvaluator for ConstantZero {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if !variables.is_empty() {
            return Err(EvaluationError::invalid_geometry("expected no variables"));
        }
        Ok(vec![0.0])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        if !variables.is_empty() {
            return Err(EvaluationError::invalid_geometry("expected no variables"));
        }
        Ok(Vec::new())
    }
}

#[derive(Clone, Copy, Debug)]
struct InvalidEvaluator(bool);

impl ResidualEvaluator for InvalidEvaluator {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if self.0 {
            Err(EvaluationError::invalid_geometry(
                "disconnected invalid component",
            ))
        } else {
            Ok(vec![f64::NAN])
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
enum AuxiliaryFailureMode {
    InvalidGeometry,
    NanResidual,
    InfiniteResidual,
    NanJacobian,
    InfiniteJacobian,
}

#[derive(Clone, Copy, Debug)]
struct AuxiliaryFailureEvaluator(AuxiliaryFailureMode);

impl ResidualEvaluator for AuxiliaryFailureEvaluator {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        match self.0 {
            AuxiliaryFailureMode::InvalidGeometry => Err(EvaluationError::invalid_geometry(
                "invalid auxiliary geometry",
            )),
            AuxiliaryFailureMode::NanResidual => Ok(vec![f64::NAN]),
            AuxiliaryFailureMode::InfiniteResidual => Ok(vec![f64::INFINITY]),
            AuxiliaryFailureMode::NanJacobian | AuxiliaryFailureMode::InfiniteJacobian => {
                Ok(vec![0.0])
            }
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let value = match self.0 {
            AuxiliaryFailureMode::NanJacobian => f64::NAN,
            AuxiliaryFailureMode::InfiniteJacobian => f64::INFINITY,
            _ => 1.0,
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![value])])
    }
}

#[derive(Clone, Copy, Debug)]
struct HugeFiniteResidual;

impl ResidualEvaluator for HugeFiniteResidual {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Ok(vec![f64::MAX, f64::MAX])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(2, 1, vec![0.0, 0.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct BinaryAffine {
    coefficients: [f64; 2],
    target: f64,
}

impl ResidualEvaluator for BinaryAffine {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![
            self.coefficients[0] * first + self.coefficients[1] * second - self.target,
        ])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![
            LocalJacobian::new(1, 1, vec![self.coefficients[0]]),
            LocalJacobian::new(1, 1, vec![self.coefficients[1]]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
struct LinearCoefficient {
    coefficient: f64,
    target: f64,
}

impl ResidualEvaluator for LinearCoefficient {
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
struct DependentX;

impl ResidualEvaluator for DependentX {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![value[0], 2.0 * value[0]])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(2, 2, vec![1.0, 0.0, 2.0, 0.0])])
    }
}

fn add_scalar_rows(
    problem: &mut Problem,
    source_id: SourceConstraintId,
    variable: VariableId,
    targets: &[f64],
) -> ResidualId {
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                targets.len(),
                vec![1.0; targets.len()],
                audit_rows(targets.len(), "scalar target"),
                ScalarRows(targets.to_vec()),
            )
            .unwrap(),
        )
        .unwrap()
}

fn add_scaled_scalar_rows(
    problem: &mut Problem,
    source_id: SourceConstraintId,
    variable: VariableId,
    targets: &[f64],
    scale: f64,
) -> ResidualId {
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                targets.len(),
                vec![scale; targets.len()],
                audit_rows(targets.len(), "scaled scalar target"),
                ScalarRows(targets.to_vec()),
            )
            .unwrap(),
        )
        .unwrap()
}

fn add_vec2_target(
    problem: &mut Problem,
    source_id: SourceConstraintId,
    variable: VariableId,
    target: [f64; 2],
) -> ResidualId {
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![1.0, 1.0],
                audit_rows(2, "Vec2 target"),
                Vec2Target(target),
            )
            .unwrap(),
        )
        .unwrap()
}

fn add_alias(
    problem: &mut Problem,
    source_id: SourceConstraintId,
    alias: VariableId,
    representative: VariableId,
) -> ResidualId {
    problem
        .add_residual(
            ResidualBlock::exact_alias(
                source_id,
                alias,
                representative,
                VariableKind::Scalar,
                vec![1.0],
                audit_rows(1, "exact scalar equality"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn add_fixed_scalar(
    problem: &mut Problem,
    source_id: SourceConstraintId,
    variable: VariableId,
    value: f64,
) -> ResidualId {
    problem
        .add_residual(
            ResidualBlock::fixed_variable(
                source_id,
                variable,
                VariableValue::Scalar(value),
                vec![1.0],
                audit_rows(1, "fixed scalar"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn add_fixed_vec2(
    problem: &mut Problem,
    source_id: SourceConstraintId,
    variable: VariableId,
    value: [f64; 2],
) -> ResidualId {
    problem
        .add_residual(
            ResidualBlock::fixed_variable(
                source_id,
                variable,
                VariableValue::Vec2(value),
                vec![1.0, 1.0],
                audit_rows(2, "fixed Vec2"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn add_binary_residual(
    problem: &mut Problem,
    source_id: SourceConstraintId,
    first: VariableId,
    second: VariableId,
    target: f64,
) -> ResidualId {
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![first, second],
                1,
                vec![1.0],
                audit_rows(1, "binary difference target"),
                BinaryAffine {
                    coefficients: [1.0, -1.0],
                    target,
                },
            )
            .unwrap(),
        )
        .unwrap()
}

#[test]
fn incidence_graph_components_include_isolated_variables_and_residuals() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(7.0, 1.0).unwrap());
    let isolated = problem.add_variable(VariableBlock::scalar(11.0, 1.0).unwrap());
    let x_source = source(&mut problem, "x target");
    let y_source = source(&mut problem, "y target");
    let constant_source = source(&mut problem, "isolated zero row");
    let x_row = add_scalar_rows(&mut problem, x_source, x, &[1.0]);
    let y_row = add_scalar_rows(&mut problem, y_source, y, &[2.0]);
    let constant_row = problem
        .add_residual(
            ResidualBlock::new(
                constant_source,
                ResidualCategory::Hard,
                Vec::new(),
                1,
                vec![1.0],
                audit_rows(1, "constant zero"),
                ConstantZero,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

    let analysis = problem.analyze_incidence();
    assert_eq!(analysis.variable_ids, vec![x, y, isolated]);
    assert_eq!(analysis.residual_ids, vec![x_row, y_row, constant_row]);
    assert_eq!(analysis.edges.len(), 2);
    assert_eq!(analysis.components.len(), 4);
    assert!(analysis.components.iter().any(|component| {
        component.variable_ids == vec![isolated] && component.residual_ids.is_empty()
    }));
    assert!(analysis.components.iter().any(|component| {
        component.variable_ids.is_empty() && component.residual_ids == vec![constant_row]
    }));

    let report = problem
        .solve_decomposed(SolverConfig::default(), &[])
        .unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!((scalar(&problem, x) - 1.0).abs() <= 1.0e-9);
    assert!((scalar(&problem, y) - 2.0).abs() <= 1.0e-9);
    assert_eq!(scalar(&problem, isolated).to_bits(), 11.0_f64.to_bits());
    assert_eq!(report.structural.components, 4);
}

#[test]
fn incremental_solve_reuses_unedited_component_bitwise() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(-4.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(9.0, 1.0).unwrap());
    let x_source = source(&mut problem, "x equals one");
    let y_source = source(&mut problem, "y equals two");
    add_scalar_rows(&mut problem, x_source, x, &[1.0]);
    add_scalar_rows(&mut problem, y_source, y, &[2.0]);

    let first = problem
        .solve_decomposed(SolverConfig::default(), &[])
        .unwrap();
    assert_eq!(first.termination, SolveTermination::Converged);
    assert!(
        first
            .component_solves
            .iter()
            .all(|component| !component.reused)
    );
    let y_accepted = scalar(&problem, y);
    let analysis = problem.analyze_incidence();
    let x_component = analysis
        .components
        .iter()
        .find(|component| component.variable_ids.contains(&x))
        .unwrap()
        .index;
    let y_component = analysis
        .components
        .iter()
        .find(|component| component.variable_ids.contains(&y))
        .unwrap()
        .index;

    problem
        .set_variable_value(x, VariableValue::Scalar(-12.0))
        .unwrap();
    let second = problem
        .solve_decomposed(SolverConfig::default(), &[x])
        .unwrap();
    assert_eq!(second.termination, SolveTermination::Converged);
    assert!(!second.component_solves[x_component].reused);
    assert!(second.component_solves[x_component].iterations > 0);
    assert_eq!(
        second.component_solves[x_component].trace.records.len(),
        second.component_solves[x_component].iterations
    );
    assert!(
        second.component_solves[x_component]
            .trace
            .records
            .iter()
            .all(|record| record.component_index == Some(x_component))
    );
    assert!(second.component_solves[y_component].reused);
    assert_eq!(second.component_solves[y_component].iterations, 0);
    assert!(
        second.component_solves[y_component]
            .trace
            .records
            .is_empty()
    );
    assert!(
        second
            .trace
            .records
            .iter()
            .all(|record| record.component_index.is_some())
    );
    assert!((scalar(&problem, x) - 1.0).abs() <= 1.0e-9);
    assert_eq!(scalar(&problem, y).to_bits(), y_accepted.to_bits());
    assert!((scalar(&problem, y) - y_accepted).abs() <= 1.0e-12);
}

#[test]
fn scalar_and_vec2_fixed_variables_remove_all_active_coordinates() {
    let mut problem = Problem::new();
    let scalar_variable = problem.add_variable(VariableBlock::scalar(-5.0, 1.0).unwrap());
    let vector_variable =
        problem.add_variable(VariableBlock::vec2([8.0, 9.0], [1.0, 1.0]).unwrap());
    let scalar_source = source(&mut problem, "fixed scalar");
    let vector_source = source(&mut problem, "fixed Vec2");
    let scalar_residual = add_fixed_scalar(&mut problem, scalar_source, scalar_variable, 3.0);
    let vector_residual = add_fixed_vec2(&mut problem, vector_source, vector_variable, [1.5, -2.0]);

    problem
        .declare_fixed_variable(scalar_variable, VariableValue::Scalar(3.0), scalar_residual)
        .unwrap();
    problem
        .declare_fixed_variable(
            vector_variable,
            VariableValue::Vec2([1.5, -2.0]),
            vector_residual,
        )
        .unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(
        scalar(&problem, scalar_variable).to_bits(),
        3.0_f64.to_bits()
    );
    assert_eq!(
        vec2(&problem, vector_variable).map(f64::to_bits),
        [1.5_f64.to_bits(), (-2.0_f64).to_bits()]
    );
    assert_eq!(report.structural.tangent_dimensions, 3);
    assert_eq!(report.structural.fixed_eliminated_coordinates, 3);
    assert_eq!(report.structural.active_tangent_dimensions, 0);
    assert_eq!(report.structural.eliminated_rows, 3);
    assert_eq!(report.rank, 0);
    assert_eq!(report.local_degrees_of_freedom, 0);
    assert!(report.audit.sources.iter().all(|item| {
        item.annotations.eliminated
            && item.annotations.suppressed
            && item.rows.iter().all(|row| {
                row.annotations.eliminated
                    && row.annotations.suppressed
                    && row.normalized_residual.abs() <= 1.0e-12
            })
    }));
}

#[test]
fn exact_alias_chain_coalesces_columns_and_synchronizes_eliminated_rows() {
    let mut problem = Problem::new();
    let a = problem.add_variable(VariableBlock::scalar(-3.0, 1.0).unwrap());
    let b = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let c = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let ab_source = source(&mut problem, "a equals b");
    let bc_source = source(&mut problem, "b equals c");
    let target_source = source(&mut problem, "a equals four");
    let ab = add_alias(&mut problem, ab_source, a, b);
    let bc = add_alias(&mut problem, bc_source, b, c);
    add_scalar_rows(&mut problem, target_source, a, &[4.0]);
    problem.declare_exact_alias(a, b, ab).unwrap();
    problem.declare_exact_alias(b, c, bc).unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!((scalar(&problem, a) - 4.0).abs() <= 1.0e-9);
    assert_eq!(scalar(&problem, a).to_bits(), scalar(&problem, b).to_bits());
    assert_eq!(scalar(&problem, b).to_bits(), scalar(&problem, c).to_bits());
    assert_eq!(report.structural.tangent_dimensions, 3);
    assert_eq!(report.structural.aliased_eliminated_coordinates, 2);
    assert_eq!(report.structural.active_tangent_dimensions, 1);
    assert_eq!(report.structural.eliminated_rows, 2);
    assert_eq!((report.rank, report.local_degrees_of_freedom), (1, 0));
    for source_id in [ab_source, bc_source] {
        let audit = report
            .audit
            .sources
            .iter()
            .find(|item| item.source_id == source_id)
            .unwrap();
        assert!(audit.annotations.eliminated);
        assert_eq!(audit.rows.len(), 1);
        assert!(audit.rows[0].annotations.suppressed);
        assert!(audit.rows[0].normalized_residual.abs() <= 1.0e-12);
    }
}

#[test]
fn duplicate_rows_distinguish_same_source_from_separate_sources() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(-1.0, 1.0).unwrap());
    let multi_source = source(&mut problem, "two rows from one source");
    let separate_source = source(&mut problem, "separate duplicate source");
    let multi_residual = add_scalar_rows(&mut problem, multi_source, variable, &[2.0, 2.0]);
    let separate_residual = add_scalar_rows(&mut problem, separate_source, variable, &[2.0]);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.redundant_sources, vec![separate_source]);
    assert_eq!(
        report.sources_containing_redundant_rows,
        vec![multi_source, separate_source]
    );
    assert_eq!(report.redundant_rows.len(), 2);
    assert_eq!(report.redundant_rows[0].row.residual_id, multi_residual);
    assert_eq!(report.redundant_rows[0].row.row_in_block, 1);
    assert_eq!(report.redundant_rows[0].kind, RedundancyKind::WithinSource);
    assert_eq!(report.redundant_rows[1].row.residual_id, separate_residual);
    assert_eq!(
        report.redundant_rows[1].kind,
        RedundancyKind::SeparateSource
    );
    let first_audit = &report.audit.sources[0];
    assert!(first_audit.annotations.redundant);
    assert!(!first_audit.rows[0].annotations.redundant);
    assert!(first_audit.rows[1].annotations.redundant);
    assert!(report.audit.sources[1].rows[0].annotations.redundant);
}

#[test]
fn contradictory_scalar_sources_name_only_restoring_sources() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.2, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(-3.0, 1.0).unwrap());
    let zero = source(&mut problem, "x equals zero");
    let one = source(&mut problem, "x equals one");
    let unrelated = source(&mut problem, "y equals two");
    add_scalar_rows(&mut problem, zero, x, &[0.0]);
    add_scalar_rows(&mut problem, one, x, &[1.0]);
    add_scalar_rows(&mut problem, unrelated, y, &[2.0]);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_ne!(report.termination, SolveTermination::Converged);
    assert_eq!(report.conflicting_sources, vec![zero, one]);
    assert!(!report.conflicting_sources.contains(&unrelated));
    assert!(report.hard_residual_max >= 0.49);
    let expected_l2 = scalar(&problem, x)
        .hypot(scalar(&problem, x) - 1.0)
        .hypot(scalar(&problem, y) - 2.0);
    assert!((report.hard_residual_l2 - expected_l2).abs() <= 1.0e-12);
    assert!(report.hard_residual_l2 > 0.7);
    assert!(report.audit.sources[0].annotations.conflicting);
    assert!(report.audit.sources[1].annotations.conflicting);
    assert!(!report.audit.sources[2].annotations.conflicting);
}

#[test]
fn huge_finite_hard_residual_norm_failure_is_numerical_and_finite() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let huge_source = source(&mut problem, "huge finite hard rows");
    problem
        .add_residual(
            ResidualBlock::new(
                huge_source,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![1.0, 1.0],
                audit_rows(2, "huge finite"),
                HugeFiniteResidual,
            )
            .unwrap(),
        )
        .unwrap();
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::NumericalFailure);
    assert!(!report.hard_residuals_validated);
    assert!(!report.component_solves[0].hard_residuals_validated);
    assert_eq!(
        report.component_solves[0].termination,
        SolveTermination::NumericalFailure
    );
    assert!(report.hard_residual_max.is_finite());
    assert!(report.hard_residual_l2.is_finite());
    assert!(
        report.audit.sources[0]
            .rows
            .iter()
            .all(|row| row.raw_residual.is_finite()
                && row.normalized_residual.is_finite()
                && row.evaluation_status == AuditEvaluationStatus::Evaluated)
    );
}

#[test]
fn contradictory_vector_sources_are_diagnosed_once_per_source() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.25, 0.75], [1.0, 1.0]).unwrap());
    let zero = source(&mut problem, "vector equals zero");
    let one = source(&mut problem, "vector equals one");
    let zero_residual = add_vec2_target(&mut problem, zero, variable, [0.0, 0.0]);
    let one_residual = add_vec2_target(&mut problem, one, variable, [1.0, 1.0]);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_ne!(report.termination, SolveTermination::Converged);
    assert_eq!(report.conflicting_sources, vec![zero, one]);
    assert_eq!(report.audit.sources.len(), 2);
    assert!(report.audit.sources.iter().all(|item| {
        item.annotations.conflicting
            && item.rows.len() == 2
            && item.rows.iter().all(|row| row.annotations.conflicting)
    }));
    let flat = problem.audit_rows().unwrap();
    assert_eq!(flat.len(), 4);
    assert_eq!(flat[0].residual_id, zero_residual);
    assert_eq!(flat[0].row_in_block, 0);
    assert_eq!(flat[1].residual_id, zero_residual);
    assert_eq!(flat[1].row_in_block, 1);
    assert_eq!(flat[2].residual_id, one_residual);
    assert_eq!(flat[2].row_in_block, 0);
    assert_eq!(flat[3].residual_id, one_residual);
    assert_eq!(flat[3].row_in_block, 1);
}

#[test]
fn underconstraint_redundancy_and_singularity_coexist() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([4.0, 9.0], [1.0, 1.0]).unwrap());
    let first = source(&mut problem, "x equals one");
    let duplicate = source(&mut problem, "duplicate x equals one");
    for source_id in [first, duplicate] {
        problem
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    audit_rows(1, "x target"),
                    XTarget(1.0),
                )
                .unwrap(),
            )
            .unwrap();
    }

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.rank, 1);
    assert_eq!(report.local_degrees_of_freedom, 1);
    assert!(report.is_singular);
    assert_eq!(report.redundant_sources, vec![duplicate]);
}

#[derive(Clone, Copy, Debug)]
struct XTarget(f64);

impl ResidualEvaluator for XTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2"));
        };
        Ok(vec![value[0] - self.0])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 2, vec![1.0, 0.0])])
    }
}

#[test]
fn numerical_singularity_changes_without_structural_signature_change() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, 1.0], [1.0, 1.0]).unwrap());
    let product_source = source(&mut problem, "x times y equals zero");
    problem
        .add_residual(
            ResidualBlock::new(
                product_source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                audit_rows(1, "product"),
                Product,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

    let regular = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(regular.rank, 1);
    assert!(!regular.is_singular);
    let signature = regular.structural.component_summaries[0].pattern_signature;
    problem
        .set_variable_value(variable, VariableValue::Vec2([0.0, 0.0]))
        .unwrap();
    let singular = problem
        .solve(SolverConfig {
            rank_relative_tolerance: 1.0e-8,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(singular.rank, 0);
    assert!(singular.is_singular);
    assert_eq!(
        singular.structural.component_summaries[0].pattern_signature,
        signature
    );
    assert_eq!(singular.singular_rows.len(), 1);
    assert!(singular.audit.sources[0].annotations.singular);
    assert!(singular.audit.sources[0].rows[0].annotations.singular);
}

#[test]
fn disconnected_invalid_component_does_not_block_healthy_component() {
    for invalid_geometry in [true, false] {
        let mut problem = Problem::new();
        let healthy = problem.add_variable(VariableBlock::scalar(-4.0, 1.0).unwrap());
        let invalid = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let healthy_source = source(&mut problem, "healthy target");
        let invalid_source = source(&mut problem, "invalid disconnected evaluator");
        add_scalar_rows(&mut problem, healthy_source, healthy, &[3.0]);
        problem
            .add_residual(
                ResidualBlock::new(
                    invalid_source,
                    ResidualCategory::Hard,
                    vec![invalid],
                    1,
                    vec![1.0],
                    audit_rows(1, "invalid component"),
                    InvalidEvaluator(invalid_geometry),
                )
                .unwrap(),
            )
            .unwrap();

        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(
            report.termination,
            if invalid_geometry {
                SolveTermination::InvalidGeometry
            } else {
                SolveTermination::NumericalFailure
            }
        );
        assert!((scalar(&problem, healthy) - 3.0).abs() <= 1.0e-9);
        let healthy_component = report
            .structural
            .component_summaries
            .iter()
            .find(|component| component.variable_ids.contains(&healthy))
            .unwrap()
            .component_index;
        let invalid_component = report
            .structural
            .component_summaries
            .iter()
            .find(|component| component.variable_ids.contains(&invalid))
            .unwrap()
            .component_index;
        assert_eq!(
            report.component_solves[healthy_component].termination,
            SolveTermination::Converged
        );
        assert!(report.component_solves[healthy_component].hard_residuals_validated);
        assert!(!report.component_solves[invalid_component].hard_residuals_validated);
        let healthy_audit = report
            .audit
            .sources
            .iter()
            .find(|item| item.source_id == healthy_source)
            .unwrap();
        assert_eq!(healthy_audit.rows.len(), 1);
        assert!(healthy_audit.rows[0].normalized_residual.abs() <= 1.0e-9);
        assert_eq!(
            healthy_audit.rows[0].evaluation_status,
            AuditEvaluationStatus::Evaluated
        );
        assert!(healthy_audit.rows[0].evaluation_error.is_none());
        let invalid_audit: Vec<_> = report
            .audit
            .sources
            .iter()
            .filter(|item| item.source_id == invalid_source)
            .collect();
        assert_eq!(invalid_audit.len(), 1);
        assert_eq!(invalid_audit[0].rows.len(), 1);
        assert_eq!(
            invalid_audit[0].rows[0].evaluation_status,
            AuditEvaluationStatus::Failed
        );
        assert!(invalid_audit[0].rows[0].evaluation_error.is_some());
        assert_eq!(invalid_audit[0].rows[0].incident_variables.len(), 1);
        assert!(invalid_audit[0].rows[0].raw_residual.is_finite());
        assert!(invalid_audit[0].rows[0].normalized_residual.is_finite());
    }
}

#[test]
fn returned_state_validates_all_auxiliary_values_and_jacobians() {
    let categories = [ResidualCategory::Temporary, ResidualCategory::Preference];
    let modes = [
        AuxiliaryFailureMode::InvalidGeometry,
        AuxiliaryFailureMode::NanResidual,
        AuxiliaryFailureMode::InfiniteResidual,
        AuxiliaryFailureMode::NanJacobian,
        AuxiliaryFailureMode::InfiniteJacobian,
    ];
    for category in categories {
        for mode in modes {
            let mut problem = Problem::new();
            let variable = problem.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap());
            let hard_source = source(&mut problem, "hard target");
            let auxiliary_source = source(&mut problem, "failing auxiliary row");
            add_scalar_rows(&mut problem, hard_source, variable, &[1.0]);
            problem
                .add_residual(
                    ResidualBlock::new(
                        auxiliary_source,
                        category,
                        vec![variable],
                        1,
                        vec![1.0],
                        audit_rows(1, "failing auxiliary"),
                        AuxiliaryFailureEvaluator(mode),
                    )
                    .unwrap(),
                )
                .unwrap();

            let report = problem.solve(SolverConfig::default()).unwrap();
            assert_eq!(
                report.termination,
                if matches!(mode, AuxiliaryFailureMode::InvalidGeometry) {
                    SolveTermination::InvalidGeometry
                } else {
                    SolveTermination::NumericalFailure
                },
                "{category:?} {mode:?}"
            );
            assert!((scalar(&problem, variable) - 1.0).abs() <= 1.0e-9);
            assert!(report.hard_residuals_validated);
            let row = &report
                .audit
                .sources
                .iter()
                .find(|item| item.source_id == auxiliary_source)
                .unwrap()
                .rows[0];
            assert_eq!(row.evaluation_status, AuditEvaluationStatus::Failed);
            assert!(
                row.evaluation_error
                    .as_deref()
                    .is_some_and(|error| !error.is_empty())
            );
            assert!(row.raw_residual.is_finite());
            assert!(row.normalized_residual.is_finite());
        }
    }

    let mut finite = Problem::new();
    let variable = finite.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let hard_source = source(&mut finite, "finite hard target");
    add_scalar_rows(&mut finite, hard_source, variable, &[1.0]);
    for (category, target) in [
        (ResidualCategory::Temporary, 100.0),
        (ResidualCategory::Preference, -100.0),
    ] {
        let source_id = source(&mut finite, &format!("finite {category:?}"));
        finite
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    category,
                    vec![variable],
                    1,
                    vec![1.0],
                    audit_rows(1, "finite nonzero auxiliary"),
                    ScalarRows(vec![target]),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let report = finite.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    for source in &report.audit.sources {
        for row in &source.rows {
            assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
            assert!(row.evaluation_error.is_none());
        }
    }
    assert!(report.audit.sources[1].rows[0].normalized_residual.abs() > 90.0);
    assert!(report.audit.sources[2].rows[0].normalized_residual.abs() > 90.0);
}

#[test]
fn conflict_deletion_suppresses_fixed_and_alias_semantics() {
    let mut fixed_problem = Problem::new();
    let x = fixed_problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let fixed_source = source(&mut fixed_problem, "x fixed at zero");
    let free_target = source(&mut fixed_problem, "x otherwise equals one");
    let fixed_row = add_fixed_scalar(&mut fixed_problem, fixed_source, x, 0.0);
    add_scalar_rows(&mut fixed_problem, free_target, x, &[1.0]);
    fixed_problem
        .declare_fixed_variable(x, VariableValue::Scalar(0.0), fixed_row)
        .unwrap();
    let fixed_report = fixed_problem.solve(SolverConfig::default()).unwrap();
    assert_ne!(fixed_report.termination, SolveTermination::Converged);
    assert_eq!(
        fixed_report.conflicting_sources,
        vec![fixed_source, free_target]
    );

    let mut alias_problem = Problem::new();
    let a = alias_problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let b = alias_problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let alias_source = source(&mut alias_problem, "a aliases b");
    let a_target = source(&mut alias_problem, "a equals zero");
    let b_target = source(&mut alias_problem, "b equals one");
    let alias_row = add_alias(&mut alias_problem, alias_source, a, b);
    add_scalar_rows(&mut alias_problem, a_target, a, &[0.0]);
    add_scalar_rows(&mut alias_problem, b_target, b, &[1.0]);
    alias_problem.declare_exact_alias(a, b, alias_row).unwrap();
    let alias_report = alias_problem.solve(SolverConfig::default()).unwrap();
    assert_ne!(alias_report.termination, SolveTermination::Converged);
    assert_eq!(
        alias_report.conflicting_sources,
        vec![alias_source, a_target, b_target]
    );
}

#[test]
fn conflicts_are_bounded_and_diagnosed_per_failed_component() {
    let mut disconnected = Problem::new();
    let x = disconnected.add_variable(VariableBlock::scalar(0.25, 1.0).unwrap());
    let y = disconnected.add_variable(VariableBlock::scalar(2.25, 1.0).unwrap());
    let sources: Vec<_> = ["x zero", "x one", "y two", "y three"]
        .into_iter()
        .map(|label| source(&mut disconnected, label))
        .collect();
    add_scalar_rows(&mut disconnected, sources[0], x, &[0.0]);
    add_scalar_rows(&mut disconnected, sources[1], x, &[1.0]);
    add_scalar_rows(&mut disconnected, sources[2], y, &[2.0]);
    add_scalar_rows(&mut disconnected, sources[3], y, &[3.0]);
    let report = disconnected.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.conflicting_sources, sources);

    let mut bounded = Problem::new();
    let oversized = bounded.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let small = bounded.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    for index in 0..13 {
        let source_id = source(&mut bounded, &format!("oversized contradiction {index}"));
        add_scalar_rows(&mut bounded, source_id, oversized, &[f64::from(index % 2)]);
    }
    let small_zero = source(&mut bounded, "small zero");
    let small_one = source(&mut bounded, "small one");
    add_scalar_rows(&mut bounded, small_zero, small, &[0.0]);
    add_scalar_rows(&mut bounded, small_one, small, &[1.0]);
    let bounded_report = bounded.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        bounded_report.conflicting_sources,
        vec![small_zero, small_one]
    );
}

#[test]
fn shared_fixed_connector_does_not_expand_ordinary_conflict_sources() {
    let mut problem = Problem::new();
    let connector = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let fixed_source = source(&mut problem, "shared fixed connector");
    let fixed_row = add_fixed_scalar(&mut problem, fixed_source, connector, 0.0);
    problem
        .declare_fixed_variable(connector, VariableValue::Scalar(0.0), fixed_row)
        .unwrap();

    let failed_branch = problem.add_variable(VariableBlock::scalar(0.2, 1.0).unwrap());
    let failed_zero = source(&mut problem, "failed branch zero");
    let failed_one = source(&mut problem, "failed branch one");
    for (source_id, target) in [(failed_zero, 0.0), (failed_one, 1.0)] {
        add_binary_residual(&mut problem, source_id, failed_branch, connector, target);
    }
    for index in 0..12 {
        let branch = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let source_id = source(&mut problem, &format!("unrelated connector branch {index}"));
        add_binary_residual(&mut problem, source_id, branch, connector, 0.0);
    }

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.conflicting_sources, vec![failed_zero, failed_one]);
}

#[test]
fn cached_component_is_revalidated_against_requested_tolerance() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(5.0e-5, 1.0).unwrap());
    let target = source(&mut problem, "strict cache target");
    add_scalar_rows(&mut problem, target, variable, &[0.0]);
    let loose = problem
        .solve_decomposed(
            SolverConfig {
                normalized_residual_tolerance: 1.0e-4,
                ..SolverConfig::default()
            },
            &[],
        )
        .unwrap();
    assert_eq!(loose.component_solves[0].iterations, 0);
    assert!(!loose.component_solves[0].reused);

    let strict = problem
        .solve_decomposed(SolverConfig::default(), &[])
        .unwrap();
    assert!(!strict.component_solves[0].reused);
    assert!(strict.component_solves[0].iterations > 0);
    assert!(scalar(&problem, variable).abs() <= 1.0e-9);
}

#[test]
fn fixed_connector_splits_reduced_components_for_incremental_reuse() {
    let mut problem = Problem::new();
    let left = problem.add_variable(VariableBlock::scalar(-3.0, 1.0).unwrap());
    let connector = problem.add_variable(VariableBlock::scalar(4.0, 1.0).unwrap());
    let right = problem.add_variable(VariableBlock::scalar(8.0, 1.0).unwrap());
    let fixed_source = source(&mut problem, "fixed connector");
    let left_source = source(&mut problem, "left through connector");
    let right_source = source(&mut problem, "right through connector");
    let fixed_row = add_fixed_scalar(&mut problem, fixed_source, connector, 0.0);
    for (source_id, variables, target) in [
        (left_source, vec![left, connector], 1.0),
        (right_source, vec![right, connector], 2.0),
    ] {
        problem
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    variables,
                    1,
                    vec![1.0],
                    audit_rows(1, "fixed/free coupling"),
                    BinaryAffine {
                        coefficients: [1.0, -1.0],
                        target,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    problem
        .declare_fixed_variable(connector, VariableValue::Scalar(0.0), fixed_row)
        .unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));
    assert_eq!(problem.analyze_incidence().components.len(), 1);
    let first = problem
        .solve_decomposed(SolverConfig::default(), &[])
        .unwrap();
    assert_eq!(first.structural.components, 3);
    let left_component = first
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&left))
        .unwrap()
        .component_index;
    let right_component = first
        .structural
        .component_summaries
        .iter()
        .find(|component| component.variable_ids.contains(&right))
        .unwrap()
        .component_index;
    let right_value = scalar(&problem, right);
    problem
        .set_variable_value(left, VariableValue::Scalar(10.0))
        .unwrap();
    let second = problem
        .solve_decomposed(SolverConfig::default(), &[left])
        .unwrap();
    assert!(!second.component_solves[left_component].reused);
    assert!(second.component_solves[left_component].iterations > 0);
    assert!(second.component_solves[right_component].reused);
    assert!(
        second.component_solves[right_component]
            .trace
            .records
            .is_empty()
    );
    assert_eq!(scalar(&problem, right).to_bits(), right_value.to_bits());
}

#[test]
fn rank_and_redundancy_use_component_local_scale_thresholds() {
    let mut problem = Problem::new();
    let large = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let small = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let large_source = source(&mut problem, "large Jacobian");
    problem
        .add_residual(
            ResidualBlock::new(
                large_source,
                ResidualCategory::Hard,
                vec![large],
                1,
                vec![1.0],
                audit_rows(1, "large coefficient"),
                LinearCoefficient {
                    coefficient: 1.0e12,
                    target: 1.0e12,
                },
            )
            .unwrap(),
        )
        .unwrap();
    let small_first = source(&mut problem, "small Jacobian");
    let small_duplicate = source(&mut problem, "small duplicate");
    add_scalar_rows(&mut problem, small_first, small, &[2.0]);
    add_scalar_rows(&mut problem, small_duplicate, small, &[2.0]);
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.rank, 2);
    assert_eq!(
        report.rank,
        report.component_solves.iter().map(|item| item.rank).sum()
    );
    assert_eq!(
        report.local_degrees_of_freedom,
        report
            .component_solves
            .iter()
            .map(|item| item.local_degrees_of_freedom)
            .sum()
    );
    assert_eq!(report.redundant_sources, vec![small_duplicate]);
}

#[test]
fn partial_multi_row_redundancy_marks_only_the_dependent_row() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let prior = source(&mut problem, "prior x row");
    let partial = source(&mut problem, "partially redundant x and y");
    problem
        .add_residual(
            ResidualBlock::new(
                prior,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                audit_rows(1, "prior x"),
                XTarget(0.0),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .add_residual(
            ResidualBlock::new(
                partial,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![1.0, 1.0],
                audit_rows(2, "x and y"),
                Vec2Target([0.0, 0.0]),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert!(report.redundant_sources.is_empty());
    assert_eq!(report.sources_containing_redundant_rows, vec![partial]);
    assert_eq!(report.redundant_rows.len(), 1);
    assert_eq!(report.redundant_rows[0].row.source_id, partial);
    assert_eq!(report.redundant_rows[0].row.row_in_block, 0);
    assert_eq!(
        report.redundant_rows[0].kind,
        RedundancyKind::SeparateSource
    );
    let audit = report
        .audit
        .sources
        .iter()
        .find(|item| item.source_id == partial)
        .unwrap();
    assert!(audit.annotations.redundant);
    assert!(audit.rows[0].annotations.redundant);
    assert!(!audit.rows[1].annotations.redundant);
}

#[test]
fn fully_redundant_source_requires_all_rows_across_components() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let prior = source(&mut problem, "prior x source");
    let shared = source(&mut problem, "shared cross-component source");
    add_scalar_rows(&mut problem, prior, x, &[0.0]);
    let duplicate_row = add_scalar_rows(&mut problem, shared, x, &[0.0]);
    let independent_row = add_scalar_rows(&mut problem, shared, y, &[0.0]);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.structural.components, 2);
    assert!(report.redundant_sources.is_empty());
    assert_eq!(report.sources_containing_redundant_rows, vec![shared]);
    assert_eq!(report.redundant_rows.len(), 1);
    assert_eq!(report.redundant_rows[0].row.residual_id, duplicate_row);
    let audit = report
        .audit
        .sources
        .iter()
        .find(|item| item.source_id == shared)
        .unwrap();
    assert_eq!(audit.rows.len(), 2);
    assert!(
        audit
            .rows
            .iter()
            .find(|row| row.residual_id == duplicate_row)
            .unwrap()
            .annotations
            .redundant
    );
    assert!(
        !audit
            .rows
            .iter()
            .find(|row| row.residual_id == independent_row)
            .unwrap()
            .annotations
            .redundant
    );
}

#[test]
fn dependent_nonzero_rows_receive_conservative_singular_annotations() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, 4.0], [1.0, 1.0]).unwrap());
    let dependent = source(&mut problem, "dependent nonzero rows");
    problem
        .add_residual(
            ResidualBlock::new(
                dependent,
                ResidualCategory::Hard,
                vec![variable],
                2,
                vec![1.0, 1.0],
                audit_rows(2, "dependent rows"),
                DependentX,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert!(report.is_singular);
    assert_eq!(report.singular_rows.len(), 2);
    assert!(report.audit.sources[0].annotations.singular);
    assert!(
        report.audit.sources[0]
            .rows
            .iter()
            .all(|row| row.annotations.singular)
    );
}

#[test]
fn active_residual_sums_columns_from_multiple_alias_members() {
    let mut problem = Problem::new();
    let a = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let b = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let alias_source = source(&mut problem, "a equals b");
    let sum_source = source(&mut problem, "a plus b equals six");
    let alias_row = add_alias(&mut problem, alias_source, a, b);
    problem
        .add_residual(
            ResidualBlock::new(
                sum_source,
                ResidualCategory::Hard,
                vec![a, b],
                1,
                vec![1.0],
                audit_rows(1, "aliased sum"),
                BinaryAffine {
                    coefficients: [1.0, 1.0],
                    target: 6.0,
                },
            )
            .unwrap(),
        )
        .unwrap();
    problem.declare_exact_alias(a, b, alias_row).unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));
    let report = problem
        .solve(SolverConfig {
            initial_damping: 1.0e-12,
            minimum_damping: 1.0e-15,
            max_block_normalized_step: 10.0,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!((scalar(&problem, a) - 3.0).abs() <= 1.0e-9);
    assert_eq!(scalar(&problem, a).to_bits(), scalar(&problem, b).to_bits());
    assert!(report.component_solves[0].iterations <= 2);
}

#[test]
fn m4_elimination_redundancy_and_conflict_are_scale_invariant() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut eliminated = Problem::new();
        let a = eliminated.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let b = eliminated.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let alias_source = source(&mut eliminated, "scaled alias");
        let target_source = source(&mut eliminated, "scaled alias target");
        let alias_row = eliminated
            .add_residual(
                ResidualBlock::exact_alias(
                    alias_source,
                    a,
                    b,
                    VariableKind::Scalar,
                    vec![scale],
                    audit_rows(1, "scaled alias"),
                )
                .unwrap(),
            )
            .unwrap();
        add_scaled_scalar_rows(&mut eliminated, target_source, a, &[2.0 * scale], scale);
        eliminated.declare_exact_alias(a, b, alias_row).unwrap();
        let report = eliminated.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged, "{scale}");
        assert_eq!(report.structural.active_tangent_dimensions, 1);
        assert!((scalar(&eliminated, a) / scale - 2.0).abs() <= 1.0e-9);

        let mut redundant = Problem::new();
        let variable = redundant.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let first = source(&mut redundant, "scaled first");
        let duplicate = source(&mut redundant, "scaled duplicate");
        add_scaled_scalar_rows(&mut redundant, first, variable, &[scale], scale);
        add_scaled_scalar_rows(&mut redundant, duplicate, variable, &[scale], scale);
        let report = redundant.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.redundant_sources, vec![duplicate]);

        let mut conflict = Problem::new();
        let variable = conflict.add_variable(VariableBlock::scalar(0.2 * scale, scale).unwrap());
        let zero = source(&mut conflict, "scaled zero");
        let one = source(&mut conflict, "scaled one");
        add_scaled_scalar_rows(&mut conflict, zero, variable, &[0.0], scale);
        add_scaled_scalar_rows(&mut conflict, one, variable, &[scale], scale);
        let report = conflict.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.conflicting_sources, vec![zero, one]);
    }
}

#[test]
fn m4_source_diagnostics_follow_source_store_order_not_residual_order() {
    let mut conflict = Problem::new();
    let variable = conflict.add_variable(VariableBlock::scalar(0.5, 1.0).unwrap());
    let zero = source(&mut conflict, "stored zero first");
    let one = source(&mut conflict, "stored one second");
    add_scalar_rows(&mut conflict, one, variable, &[1.0]);
    add_scalar_rows(&mut conflict, zero, variable, &[0.0]);
    let report = conflict.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.conflicting_sources, vec![zero, one]);

    let mut redundant = Problem::new();
    let variable = redundant.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let primary = source(&mut redundant, "stored primary first");
    let duplicate = source(&mut redundant, "stored duplicate second");
    add_scalar_rows(&mut redundant, duplicate, variable, &[2.0]);
    add_scalar_rows(&mut redundant, primary, variable, &[2.0]);
    let report = redundant.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.redundant_sources, vec![duplicate]);
    assert_eq!(report.redundant_rows[0].row.source_id, duplicate);
}

#[test]
#[allow(clippy::too_many_lines)]
fn stale_invalid_cyclic_and_conflicting_eliminations_are_rejected() {
    let mut stale = Problem::new();
    let variable = stale.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let fixed_source = source(&mut stale, "fixed");
    let fixed_row = add_fixed_scalar(&mut stale, fixed_source, variable, 1.0);
    stale
        .declare_fixed_variable(variable, VariableValue::Scalar(1.0), fixed_row)
        .unwrap();
    stale.remove_residual(fixed_row).unwrap();
    assert!(matches!(
        stale.structural_summary(),
        Err(CoreError::UnknownResidual(id)) if id == fixed_row
    ));

    let mut stale_variable_problem = Problem::new();
    let stale_variable =
        stale_variable_problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    stale_variable_problem
        .remove_variable(stale_variable)
        .unwrap();
    let active = stale_variable_problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let active_source = source(&mut stale_variable_problem, "active fixed row");
    let active_row = add_scalar_rows(&mut stale_variable_problem, active_source, active, &[0.0]);
    assert!(matches!(
        stale_variable_problem.declare_fixed_variable(
            stale_variable,
            VariableValue::Scalar(0.0),
            active_row
        ),
        Err(CoreError::UnknownVariable(id)) if id == stale_variable
    ));

    let mut scale_mismatch = Problem::new();
    let first = scale_mismatch.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let second = scale_mismatch.add_variable(VariableBlock::scalar(0.0, 2.0).unwrap());
    let alias_source = source(&mut scale_mismatch, "scale-mismatched alias");
    let alias_row = add_alias(&mut scale_mismatch, alias_source, first, second);
    assert!(matches!(
        scale_mismatch.declare_exact_alias(first, second, alias_row),
        Err(CoreError::AliasScaleMismatch { .. })
    ));

    let mut invalid_kind = Problem::new();
    let scalar_variable = invalid_kind.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let vector_variable =
        invalid_kind.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let invalid_source = source(&mut invalid_kind, "invalid alias kind");
    let scalar_row = add_scalar_rows(&mut invalid_kind, invalid_source, scalar_variable, &[0.0]);
    assert!(matches!(
        invalid_kind.declare_exact_alias(scalar_variable, vector_variable, scalar_row),
        Err(CoreError::VariableKindMismatch {
            expected: VariableKind::Vec2,
            actual: VariableKind::Scalar
        })
    ));

    let mut conflicting = Problem::new();
    let fixed = conflicting.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let first_source = source(&mut conflicting, "first fixed declaration");
    let second_source = source(&mut conflicting, "second fixed declaration");
    let first_row = add_fixed_scalar(&mut conflicting, first_source, fixed, 1.0);
    let second_row = add_fixed_scalar(&mut conflicting, second_source, fixed, 1.0);
    conflicting
        .declare_fixed_variable(fixed, VariableValue::Scalar(1.0), first_row)
        .unwrap();
    assert!(matches!(
        conflicting.declare_fixed_variable(fixed, VariableValue::Scalar(1.0), second_row),
        Err(CoreError::ConflictingElimination { variable, .. }) if variable == fixed
    ));

    let mut cyclic = Problem::new();
    let a = cyclic.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let b = cyclic.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let c = cyclic.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let ab_source = source(&mut cyclic, "a to b");
    let bc_source = source(&mut cyclic, "b to c");
    let ca_source = source(&mut cyclic, "c to a");
    let ab = add_alias(&mut cyclic, ab_source, a, b);
    let bc = add_alias(&mut cyclic, bc_source, b, c);
    let ca = add_alias(&mut cyclic, ca_source, c, a);
    cyclic.declare_exact_alias(a, b, ab).unwrap();
    cyclic.declare_exact_alias(b, c, bc).unwrap();
    assert!(matches!(
        cyclic.declare_exact_alias(c, a, ca),
        Err(CoreError::AliasCycle { .. })
    ));

    let mut fake_alias = Problem::new();
    let alias = fake_alias.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let representative = fake_alias.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let fake_source = source(&mut fake_alias, "opaque same-shape alias");
    let fake_row = fake_alias
        .add_residual(
            ResidualBlock::new(
                fake_source,
                ResidualCategory::Hard,
                vec![alias, representative],
                1,
                vec![1.0],
                audit_rows(1, "fake alias"),
                ScalarDifference,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(
        fake_alias
            .check_jacobians(1.0e-5)
            .unwrap()
            .all_within(1.0e-6)
    );
    assert!(matches!(
        fake_alias.declare_exact_alias(alias, representative, fake_row),
        Err(CoreError::InvalidEliminationResidual { .. })
    ));

    let mut mismatched_fixed = Problem::new();
    let variable = mismatched_fixed.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let fixed_source = source(&mut mismatched_fixed, "trusted fixed zero");
    let fixed_row = add_fixed_scalar(&mut mismatched_fixed, fixed_source, variable, 0.0);
    assert!(matches!(
        mismatched_fixed.declare_fixed_variable(variable, VariableValue::Scalar(1.0), fixed_row),
        Err(CoreError::InvalidEliminationResidual { .. })
    ));
    let report = mismatched_fixed.solve(SolverConfig::default()).unwrap();
    assert!((scalar(&mismatched_fixed, variable) - 1.0).abs() > 0.5);
    assert_eq!(report.structural.eliminated_rows, 0);

    let mut invalid_residual = Problem::new();
    let variable = invalid_residual.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let temporary_source = source(&mut invalid_residual, "temporary is not exact hard");
    let temporary = invalid_residual
        .add_residual(
            ResidualBlock::new(
                temporary_source,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                audit_rows(1, "temporary"),
                ScalarRows(vec![0.0]),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        invalid_residual.declare_fixed_variable(variable, VariableValue::Scalar(0.0), temporary),
        Err(CoreError::InvalidEliminationResidual { .. })
    ));
}
