use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use geosolve_core::{
    AdaptiveStepController, AdaptiveStepDecision, AdaptiveStepPolicy, AuditBinding,
    AuditEvaluationStatus, ContinuationError, ContinuationTangentOrientation, CoordinateBound,
    DiagnosticBudget, EvaluationError, HardValidity, InitialParameterDirection, LinearSolveBackend,
    LinearSolveBackendPolicy, LocalJacobian, PrioritySolveBackend, Problem,
    PseudoArclengthVariable, ResidualBlock, ResidualCategory, ResidualEvaluator, ResidualRowAudit,
    SecondaryStatus, SessionPatch, SolveSession, SolveTermination, SolverConfig, SourceConstraint,
    SparseFallbackReason, StructuralClassification, VariableBlock, VariableId, VariableKind,
    VariableValue,
};
use geosolve_geometry::{Pose2 as GeometryPose2, Pose3 as GeometryPose3};
use nalgebra::DVector;

fn rows(count: usize) -> Vec<ResidualRowAudit> {
    (0..count)
        .map(|row| {
            ResidualRowAudit::new(
                format!("M16 structural row {row}"),
                vec![AuditBinding::new("variables", "M16 fixture")],
                "model unit",
            )
        })
        .collect()
}

#[derive(Clone, Debug)]
struct AffineScalars {
    matrix: Vec<Vec<f64>>,
    target: Vec<f64>,
}

impl ResidualEvaluator for AffineScalars {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let values = variables
            .iter()
            .map(|value| match value {
                VariableValue::Scalar(value) => Ok(*value),
                _ => Err(EvaluationError::invalid_geometry("expected scalar blocks")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.matrix.len() != self.target.len()
            || self.matrix.iter().any(|row| row.len() != values.len())
        {
            return Err(EvaluationError::invalid_geometry(
                "affine dimensions do not match incidence",
            ));
        }
        Ok(self
            .matrix
            .iter()
            .zip(&self.target)
            .map(|(row, target)| {
                row.iter()
                    .zip(&values)
                    .map(|(coefficient, value)| coefficient * value)
                    .sum::<f64>()
                    - target
            })
            .collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        if self.matrix.iter().any(|row| row.len() != variables.len()) {
            return Err(EvaluationError::invalid_geometry(
                "affine dimensions do not match incidence",
            ));
        }
        Ok((0..variables.len())
            .map(|column| {
                LocalJacobian::new(
                    self.matrix.len(),
                    1,
                    self.matrix.iter().map(|row| row[column]).collect(),
                )
            })
            .collect())
    }
}

#[derive(Clone, Debug)]
struct BoundCheckedTargets {
    targets: Vec<f64>,
    lower: f64,
    upper: f64,
    outside: Arc<AtomicUsize>,
}

impl ResidualEvaluator for BoundCheckedTargets {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if variables.len() != self.targets.len() {
            return Err(EvaluationError::invalid_geometry(
                "bound-checked target dimensions do not match incidence",
            ));
        }
        variables
            .iter()
            .zip(&self.targets)
            .map(|(value, target)| {
                let VariableValue::Scalar(value) = value else {
                    return Err(EvaluationError::invalid_geometry("expected scalar blocks"));
                };
                if !(self.lower..=self.upper).contains(value) {
                    self.outside.fetch_add(1, Ordering::Relaxed);
                    return Err(EvaluationError::out_of_domain(
                        "large bounded operator evaluated outside its bounds",
                    ));
                }
                Ok(value - target)
            })
            .collect()
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        self.evaluate(variables)?;
        Ok((0..variables.len())
            .map(|column| {
                LocalJacobian::new(
                    variables.len(),
                    1,
                    (0..variables.len())
                        .map(|row| if row == column { 1.0 } else { 0.0 })
                        .collect(),
                )
            })
            .collect())
    }
}

#[derive(Clone, Copy, Debug)]
struct CoupledMaximum;

impl ResidualEvaluator for CoupledMaximum {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "coupled maximum expected two scalars",
            ));
        };
        let difference = first - second;
        Ok(vec![1.0 - difference * difference])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "coupled maximum expected two scalars",
            ));
        };
        let difference = first - second;
        Ok(vec![
            LocalJacobian::new(1, 1, vec![-2.0 * difference]),
            LocalJacobian::new(1, 1, vec![2.0 * difference]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
struct CoupledMaskedMaximum;

impl ResidualEvaluator for CoupledMaskedMaximum {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "coupled masked maximum expected two scalars",
            ));
        };
        let difference = first - second;
        Ok(vec![
            1.0 - difference * difference + 1.0e6 * difference.powi(4),
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "coupled masked maximum expected two scalars",
            ));
        };
        let difference = first - second;
        let derivative = -2.0 * difference + 4.0e6 * difference.powi(3);
        Ok(vec![
            LocalJacobian::new(1, 1, vec![derivative]),
            LocalJacobian::new(1, 1, vec![-derivative]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarSquare {
    target: f64,
}

impl ResidualEvaluator for ScalarSquare {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "scalar square expected one scalar",
            ));
        };
        Ok(vec![value * value - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "scalar square expected one scalar",
            ));
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![2.0 * value])])
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarFold;

impl ResidualEvaluator for ScalarFold {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [
            VariableValue::Scalar(state),
            VariableValue::Scalar(parameter),
        ] = variables
        else {
            return Err(EvaluationError::invalid_geometry(
                "scalar fold expected state and parameter scalars",
            ));
        };
        Ok(vec![state * state - parameter])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(state), VariableValue::Scalar(_)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "scalar fold expected state and parameter scalars",
            ));
        };
        Ok(vec![
            LocalJacobian::new(1, 1, vec![2.0 * state]),
            LocalJacobian::new(1, 1, vec![-1.0]),
        ])
    }
}

#[derive(Clone, Debug)]
struct PosePairLinear {
    matrix: Vec<Vec<f64>>,
}

impl ResidualEvaluator for PosePairLinear {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Pose2(first), VariableValue::Pose2(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "pose-pair linear fixture expected two Pose2 blocks",
            ));
        };
        let values = [
            first[0], first[1], first[2], second[0], second[1], second[2],
        ];
        Ok(self
            .matrix
            .iter()
            .map(|row| {
                row.iter()
                    .zip(values)
                    .map(|(coefficient, value)| coefficient * value)
                    .sum()
            })
            .collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Pose2(first), VariableValue::Pose2(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "pose-pair linear fixture expected two Pose2 blocks",
            ));
        };
        if first[2].to_bits() != 0.0_f64.to_bits() || second[2].to_bits() != 0.0_f64.to_bits() {
            return Err(EvaluationError::invalid_geometry(
                "pose-pair linear fixture is defined at identity orientation",
            ));
        }
        Ok(vec![
            LocalJacobian::new(
                self.matrix.len(),
                3,
                self.matrix
                    .iter()
                    .flat_map(|row| row[..3].iter().copied())
                    .collect(),
            ),
            LocalJacobian::new(
                self.matrix.len(),
                3,
                self.matrix
                    .iter()
                    .flat_map(|row| row[3..].iter().copied())
                    .collect(),
            ),
        ])
    }
}

fn add_affine(
    problem: &mut Problem,
    label: &str,
    variables: Vec<VariableId>,
    matrix: Vec<Vec<f64>>,
    scale: f64,
) -> geosolve_core::ResidualId {
    let target = vec![0.0; matrix.len()];
    add_affine_target(problem, label, variables, matrix, target, scale).1
}

fn add_affine_target(
    problem: &mut Problem,
    label: &str,
    variables: Vec<VariableId>,
    matrix: Vec<Vec<f64>>,
    target: Vec<f64>,
    scale: f64,
) -> (geosolve_core::SourceConstraintId, geosolve_core::ResidualId) {
    let source = problem.add_source(SourceConstraint::new(label).unwrap());
    let row_count = matrix.len();
    let residual = problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                variables,
                row_count,
                vec![scale; row_count],
                rows(row_count),
                AffineScalars { matrix, target },
            )
            .unwrap(),
        )
        .unwrap();
    (source, residual)
}

fn add_secondary_affine_target(
    problem: &mut Problem,
    label: &str,
    category: ResidualCategory,
    variables: Vec<VariableId>,
    matrix: Vec<Vec<f64>>,
    target: Vec<f64>,
) -> (geosolve_core::SourceConstraintId, geosolve_core::ResidualId) {
    let source = problem.add_source(SourceConstraint::new(label).unwrap());
    let row_count = matrix.len();
    let residual = problem
        .add_residual(
            ResidualBlock::new(
                source,
                category,
                variables,
                row_count,
                vec![1.0; row_count],
                rows(row_count),
                AffineScalars { matrix, target },
            )
            .unwrap(),
        )
        .unwrap();
    (source, residual)
}

fn diagonal_matrix(size: usize) -> Vec<Vec<f64>> {
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| if row == column { 1.0 } else { 0.0 })
                .collect()
        })
        .collect()
}

fn bounded_oracle_fixture(size: usize) -> (Problem, Vec<VariableId>) {
    let mut problem = Problem::new();
    let variables = (0..size)
        .map(|index| {
            let value = match index {
                0 => 0.5,
                1 => -0.5,
                _ => 0.0,
            };
            problem.add_variable(VariableBlock::scalar(value, 1.0).unwrap())
        })
        .collect::<Vec<_>>();
    problem
        .add_bound(
            CoordinateBound::new(variables[0], 0, Some(0.0), Some(2.0), "oracle lower").unwrap(),
        )
        .unwrap();
    problem
        .add_bound(
            CoordinateBound::new(variables[1], 0, Some(-2.0), Some(0.0), "oracle upper").unwrap(),
        )
        .unwrap();
    problem
        .add_bound(
            CoordinateBound::new(variables[2], 0, Some(0.0), Some(2.0), "oracle release").unwrap(),
        )
        .unwrap();
    let mut targets = vec![0.0; size];
    targets[0] = -1.0;
    targets[1] = 1.0;
    targets[2] = 1.0;
    add_secondary_affine_target(
        &mut problem,
        "bounded dense/operator oracle",
        ResidualCategory::Temporary,
        variables.clone(),
        diagonal_matrix(size),
        targets,
    );
    (problem, variables)
}

fn scalar(problem: &Problem, variable: VariableId) -> f64 {
    let VariableValue::Scalar(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected scalar")
    };
    value
}

fn component_with_residual(
    report: &geosolve_core::SolveReport,
    residual: geosolve_core::ResidualId,
) -> &geosolve_core::ComponentStructuralSummary {
    report
        .structural
        .component_summaries
        .iter()
        .find(|component| component.residual_ids.contains(&residual))
        .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn under_well_over_and_aggregate_structural_totals_are_reported() {
    let mut problem = Problem::new();
    let under_x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let under_y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let well = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let over = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let under_residual = add_affine(
        &mut problem,
        "under",
        vec![under_x, under_y],
        vec![vec![1.0, 1.0]],
        1.0,
    );
    let well_residual = add_affine(&mut problem, "well", vec![well], vec![vec![1.0]], 1.0);
    let over_residual = add_affine(
        &mut problem,
        "over",
        vec![over],
        vec![vec![1.0], vec![2.0]],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    let under = component_with_residual(&report, under_residual);
    assert_eq!(
        under.structural_classification,
        StructuralClassification::Under
    );
    assert_eq!(
        (
            under.structural_rank,
            under.structural_left_nullity,
            under.structural_right_nullity,
        ),
        (1, 0, 1)
    );
    assert_eq!(under.dm_partitions.under.rows.len(), 1);
    assert_eq!(under.dm_partitions.under.tangent_coordinates.len(), 2);
    assert!(under.dm_partitions.well.rows.is_empty());
    assert!(under.dm_partitions.over.rows.is_empty());

    let well = component_with_residual(&report, well_residual);
    assert_eq!(
        well.structural_classification,
        StructuralClassification::Well
    );
    assert_eq!(
        (
            well.structural_rank,
            well.structural_left_nullity,
            well.structural_right_nullity,
        ),
        (1, 0, 0)
    );
    assert_eq!(well.dm_partitions.well.rows.len(), 1);
    assert_eq!(well.dm_partitions.well.tangent_coordinates.len(), 1);

    let over = component_with_residual(&report, over_residual);
    assert_eq!(
        over.structural_classification,
        StructuralClassification::Over
    );
    assert_eq!(
        (
            over.structural_rank,
            over.structural_left_nullity,
            over.structural_right_nullity,
        ),
        (1, 1, 0)
    );
    assert_eq!(over.dm_partitions.over.rows.len(), 2);
    assert_eq!(over.dm_partitions.over.tangent_coordinates.len(), 1);

    let structural = &report.structural;
    assert_eq!(
        structural.structural_classification,
        StructuralClassification::Mixed
    );
    assert_eq!(
        (
            structural.structural_rank,
            structural.structural_left_nullity,
            structural.structural_right_nullity,
        ),
        (3, 1, 1)
    );
    assert_eq!(
        structural.structural_rank,
        structural
            .component_summaries
            .iter()
            .map(|component| component.structural_rank)
            .sum::<usize>()
    );
    assert_eq!(
        structural.structural_left_nullity,
        structural
            .component_summaries
            .iter()
            .map(|component| component.structural_left_nullity)
            .sum::<usize>()
    );
    assert_eq!(
        structural.structural_right_nullity,
        structural
            .component_summaries
            .iter()
            .map(|component| component.structural_right_nullity)
            .sum::<usize>()
    );
    assert_eq!(structural.dm_partitions.under.rows.len(), 1);
    assert_eq!(structural.dm_partitions.under.tangent_coordinates.len(), 2);
    assert_eq!(structural.dm_partitions.well.rows.len(), 1);
    assert_eq!(structural.dm_partitions.well.tangent_coordinates.len(), 1);
    assert_eq!(structural.dm_partitions.over.rows.len(), 2);
    assert_eq!(structural.dm_partitions.over.tangent_coordinates.len(), 1);
    for component in &structural.component_summaries {
        assert_eq!(
            report.component_solves[component.component_index].sparsity_signature,
            component.sparsity_signature
        );
    }
    assert_eq!(report.rank, 3);
    assert_eq!((report.left_nullity, report.right_nullity), (1, 1));
}

fn connected_mixed_problem(reverse_dense_incidence: bool) -> Problem {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let z = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_affine(&mut problem, "x row zero", vec![x], vec![vec![1.0]], 1.0);
    add_affine(&mut problem, "x row one", vec![x], vec![vec![2.0]], 1.0);
    if reverse_dense_incidence {
        add_affine(
            &mut problem,
            "connecting row",
            vec![z, y, x],
            vec![vec![3.0, 2.0, 1.0]],
            1.0,
        );
    } else {
        add_affine(
            &mut problem,
            "connecting row",
            vec![x, y, z],
            vec![vec![1.0, 2.0, 3.0]],
            1.0,
        );
    }
    problem
}

#[test]
fn connected_count_square_hall_deficiency_is_mixed_with_deterministic_dm_parts() {
    let mut problem = connected_mixed_problem(false);
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.structural.component_summaries.len(), 1);
    let component = &report.structural.component_summaries[0];
    assert_eq!(
        (
            component.active_hard_rows,
            component.active_tangent_dimensions
        ),
        (3, 3)
    );
    assert_eq!(
        component.structural_classification,
        StructuralClassification::Mixed
    );
    assert_eq!(
        (
            component.structural_rank,
            component.structural_left_nullity,
            component.structural_right_nullity,
        ),
        (2, 1, 1)
    );
    assert_eq!(component.dm_partitions.under.rows.len(), 1);
    assert_eq!(component.dm_partitions.under.tangent_coordinates.len(), 2);
    assert!(component.dm_partitions.well.rows.is_empty());
    assert!(component.dm_partitions.well.tangent_coordinates.is_empty());
    assert_eq!(component.dm_partitions.over.rows.len(), 2);
    assert_eq!(component.dm_partitions.over.tangent_coordinates.len(), 1);

    let baseline = component.clone();
    let mut reversed = connected_mixed_problem(true);
    let reversed = reversed.solve(SolverConfig::default()).unwrap();
    let reversed = &reversed.structural.component_summaries[0];
    assert_ne!(baseline.pattern_signature, reversed.pattern_signature);
    assert_eq!(baseline.sparsity_signature, reversed.sparsity_signature);
    assert_eq!(baseline.structural_rank, reversed.structural_rank);
    assert_eq!(baseline.dm_partitions, reversed.dm_partitions);
}

#[derive(Clone, Copy, Debug)]
struct Product;

impl ResidualEvaluator for Product {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2 block"));
        };
        Ok(vec![value[0] * value[1]])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one Vec2 block"));
        };
        Ok(vec![LocalJacobian::new(1, 2, vec![value[1], value[0]])])
    }
}

#[test]
fn numerical_rank_drop_does_not_change_structural_classification_or_sparsity() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, 1.0], [1.0, 1.0]).unwrap());
    let source = problem.add_source(SourceConstraint::new("product").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                rows(1),
                Product,
            )
            .unwrap(),
        )
        .unwrap();

    let regular = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(regular.rank, 1);
    let regular_structural = regular.structural.component_summaries[0].clone();
    assert_eq!(regular_structural.structural_rank, 1);
    assert_eq!(
        regular_structural.structural_classification,
        StructuralClassification::Under
    );

    problem
        .set_variable_value(variable, VariableValue::Vec2([0.0, 0.0]))
        .unwrap();
    let singular = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(singular.rank, 0);
    assert_eq!(
        singular.structural.component_summaries[0],
        regular_structural
    );
}

fn scaled_pattern(scale: f64) -> geosolve_core::ComponentStructuralSummary {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
    add_affine(
        &mut problem,
        "scaled row",
        vec![variable],
        vec![vec![1.0]],
        scale,
    );
    problem
        .structural_summary()
        .unwrap()
        .component_summaries
        .remove(0)
}

#[test]
fn sparsity_signature_is_scale_independent_without_changing_pattern_signature() {
    let small = scaled_pattern(1.0e-6);
    let unit = scaled_pattern(1.0);
    let large = scaled_pattern(1.0e6);
    assert_eq!(small.sparsity_signature, unit.sparsity_signature);
    assert_eq!(unit.sparsity_signature, large.sparsity_signature);
    assert_ne!(small.pattern_signature, unit.pattern_signature);
    assert_ne!(unit.pattern_signature, large.pattern_signature);
}

#[test]
fn fixed_and_alias_coordinates_use_only_the_unfixed_root_in_dm_output() {
    let mut problem = Problem::new();
    let alias = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let root = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let fixed = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());

    let alias_source = problem.add_source(SourceConstraint::new("alias").unwrap());
    let alias_residual = problem
        .add_residual(
            ResidualBlock::exact_alias(
                alias_source,
                alias,
                root,
                VariableKind::Scalar,
                vec![1.0],
                rows(1),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_exact_alias(alias, root, alias_residual)
        .unwrap();

    let fixed_source = problem.add_source(SourceConstraint::new("fixed").unwrap());
    let fixed_residual = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                fixed_source,
                fixed,
                VariableValue::Scalar(0.0),
                vec![1.0],
                rows(1),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_fixed_variable(fixed, VariableValue::Scalar(0.0), fixed_residual)
        .unwrap();

    let active = add_affine(
        &mut problem,
        "active alias row",
        vec![alias, root, fixed],
        vec![vec![2.0, 3.0, 0.0]],
        1.0,
    );
    let report = problem.solve(SolverConfig::default()).unwrap();
    let component = component_with_residual(&report, active);
    assert_eq!(component.active_tangent_dimensions, 1);
    assert_eq!(component.aliased_eliminated_coordinates, 1);
    assert_eq!(component.structural_rank, 1);
    assert_eq!(
        component.structural_classification,
        StructuralClassification::Well
    );
    assert_eq!(component.dm_partitions.well.tangent_coordinates.len(), 1);
    assert_eq!(
        component.dm_partitions.well.tangent_coordinates[0].root,
        root
    );
    assert_eq!(
        component.dm_partitions.well.tangent_coordinates[0].coordinate_in_block,
        0
    );
    assert!(
        component
            .dm_partitions
            .well
            .tangent_coordinates
            .iter()
            .all(|coordinate| coordinate.root != alias && coordinate.root != fixed)
    );
    assert_eq!(report.structural.fixed_eliminated_coordinates, 1);
    assert_eq!(report.structural.aliased_eliminated_coordinates, 1);
}

fn backend_config(policy: LinearSolveBackendPolicy) -> SolverConfig {
    SolverConfig {
        initial_damping: 1.0e-15,
        minimum_damping: 1.0e-15,
        linear_solve_backend: policy,
        ..SolverConfig::default()
    }
}

fn assert_audit_parity(
    dense: &geosolve_core::AuditSnapshot,
    sparse: &geosolve_core::AuditSnapshot,
) {
    assert_eq!(dense.sources.len(), sparse.sources.len());
    for (dense_source, sparse_source) in dense.sources.iter().zip(&sparse.sources) {
        assert_eq!(dense_source.source_id, sparse_source.source_id);
        assert_eq!(dense_source.source_label, sparse_source.source_label);
        assert_eq!(dense_source.annotations, sparse_source.annotations);
        assert_eq!(dense_source.active_bounds, sparse_source.active_bounds);
        assert_eq!(dense_source.rows.len(), sparse_source.rows.len());
        for (dense_row, sparse_row) in dense_source.rows.iter().zip(&sparse_source.rows) {
            assert_eq!(dense_row.residual_id, sparse_row.residual_id);
            assert_eq!(dense_row.category, sparse_row.category);
            assert_eq!(dense_row.row_in_block, sparse_row.row_in_block);
            assert_eq!(dense_row.template, sparse_row.template);
            assert_eq!(dense_row.bindings, sparse_row.bindings);
            assert_eq!(dense_row.unit, sparse_row.unit);
            assert!((dense_row.scale - sparse_row.scale).abs() <= f64::EPSILON);
            assert!((dense_row.raw_residual - sparse_row.raw_residual).abs() <= 1.0e-9);
            assert!(
                (dense_row.normalized_residual - sparse_row.normalized_residual).abs() <= 1.0e-9
            );
            assert_eq!(dense_row.evaluation_status, sparse_row.evaluation_status);
            assert_eq!(
                dense_row.evaluation_error_category,
                sparse_row.evaluation_error_category
            );
            assert_eq!(dense_row.evaluation_error, sparse_row.evaluation_error);
            assert_eq!(dense_row.annotations, sparse_row.annotations);
            assert_eq!(dense_row.active_bounds, sparse_row.active_bounds);
            assert_eq!(
                dense_row.incident_variables.len(),
                sparse_row.incident_variables.len()
            );
            for (dense_variable, sparse_variable) in dense_row
                .incident_variables
                .iter()
                .zip(&sparse_row.incident_variables)
            {
                assert_eq!(dense_variable.variable_id, sparse_variable.variable_id);
                assert_eq!(
                    dense_variable.value.ambient_values().len(),
                    sparse_variable.value.ambient_values().len()
                );
                for (&dense_value, &sparse_value) in dense_variable
                    .value
                    .ambient_values()
                    .iter()
                    .zip(sparse_variable.value.ambient_values())
                {
                    assert!((dense_value - sparse_value).abs() <= 1.0e-9);
                }
            }
        }
    }
}

fn assert_priority_parity(
    dense: &[geosolve_core::PrioritySolveReport],
    sparse: &[geosolve_core::PrioritySolveReport],
) {
    assert_eq!(dense.len(), sparse.len());
    for (dense, sparse) in dense.iter().zip(sparse) {
        assert_eq!(dense.group_index, sparse.group_index);
        assert_eq!(dense.component_index, sparse.component_index);
        assert_eq!(dense.component_indices, sparse.component_indices);
        assert_eq!(dense.scope, sparse.scope);
        assert_eq!(dense.backend, sparse.backend);
        assert_eq!(
            dense.largest_explicit_nullspace_block_rows,
            sparse.largest_explicit_nullspace_block_rows
        );
        assert_eq!(dense.protected_temporary, sparse.protected_temporary);
        assert_eq!(dense.category, sparse.category);
        assert_eq!(dense.iterations, sparse.iterations);
        for (dense_cost, sparse_cost) in [
            (dense.initial_cost, sparse.initial_cost),
            (dense.final_cost, sparse.final_cost),
            (
                dense.attained_temporary_cost,
                sparse.attained_temporary_cost,
            ),
        ] {
            match (dense_cost, sparse_cost) {
                (Some(dense), Some(sparse)) => assert!((dense - sparse).abs() <= 1.0e-20),
                (None, None) => {}
                _ => panic!("priority cost availability differs"),
            }
        }
        assert_eq!(dense.termination, sparse.termination);
        assert_eq!(dense.status, sparse.status);
    }
}

fn assert_component_parity(
    dense: &[geosolve_core::ComponentSolveReport],
    sparse: &[geosolve_core::ComponentSolveReport],
) {
    assert_eq!(dense.len(), sparse.len());
    for (dense, sparse) in dense.iter().zip(sparse) {
        assert_eq!(dense.component_index, sparse.component_index);
        assert_eq!(dense.pattern_signature, sparse.pattern_signature);
        assert_eq!(dense.sparsity_signature, sparse.sparsity_signature);
        assert_eq!(dense.structural_nnz, sparse.structural_nnz);
        assert_eq!(dense.reused, sparse.reused);
        assert_eq!(dense.secondary_participated, sparse.secondary_participated);
        assert_eq!(
            dense.state_changed_by_secondary,
            sparse.state_changed_by_secondary
        );
        assert_eq!(dense.termination, sparse.termination);
        assert_eq!(dense.hard_termination, sparse.hard_termination);
        assert_eq!(dense.hard_validity, sparse.hard_validity);
        assert_eq!(
            dense.hard_residuals_validated,
            sparse.hard_residuals_validated
        );
        assert!((dense.hard_residual_max - sparse.hard_residual_max).abs() <= 1.0e-9);
        assert!((dense.hard_residual_l2 - sparse.hard_residual_l2).abs() <= 1.0e-9);
        assert_eq!(dense.rank_is_valid, sparse.rank_is_valid);
        assert_eq!(dense.rank, sparse.rank);
        assert_eq!(dense.left_nullity, sparse.left_nullity);
        assert_eq!(dense.right_nullity, sparse.right_nullity);
        assert_eq!(
            dense.local_degrees_of_freedom,
            sparse.local_degrees_of_freedom
        );
        assert_eq!(
            dense.bidirectional_degrees_of_freedom,
            sparse.bidirectional_degrees_of_freedom
        );
        assert_eq!(dense.one_sided_mobility, sparse.one_sided_mobility);
        assert_eq!(dense.active_bounds, sparse.active_bounds);
        assert_eq!(dense.is_singular, sparse.is_singular);
        assert_eq!(
            dense.rank_relative_tolerance.to_bits(),
            sparse.rank_relative_tolerance.to_bits()
        );
        assert!((dense.rank_machine_tolerance - sparse.rank_machine_tolerance).abs() <= 1.0e-12);
        assert!((dense.rank_threshold - sparse.rank_threshold).abs() <= 1.0e-12);
        assert!((dense.sigma_max - sparse.sigma_max).abs() <= 1.0e-9);
        match (
            dense.smallest_retained_singular_value,
            sparse.smallest_retained_singular_value,
        ) {
            (Some(dense), Some(sparse)) => assert!((dense - sparse).abs() <= 1.0e-9),
            (None, None) => {}
            _ => panic!("smallest retained singular value availability differs"),
        }
        assert_eq!(
            dense.near_singular_factor.to_bits(),
            sparse.near_singular_factor.to_bits()
        );
        match (dense.near_singular_ratio, sparse.near_singular_ratio) {
            (Some(dense), Some(sparse)) => {
                assert!((dense - sparse).abs() <= 1.0e-6 * dense.abs().max(1.0));
            }
            (None, None) => {}
            _ => panic!("near-singular ratio availability differs"),
        }
        assert_eq!(dense.near_singular, sparse.near_singular);
        assert_eq!(dense.singular_values.len(), sparse.singular_values.len());
        for (&dense, &sparse) in dense.singular_values.iter().zip(&sparse.singular_values) {
            assert!((dense - sparse).abs() <= 1.0e-9);
        }
    }
}

fn assert_dense_sparse_parity(problem: Problem, expected_rank: usize) {
    let mut dense_problem = problem.clone();
    let dense = dense_problem
        .solve(backend_config(LinearSolveBackendPolicy::DenseOnly))
        .unwrap();
    let mut sparse_problem = problem;
    let sparse = sparse_problem
        .solve(backend_config(LinearSolveBackendPolicy::SparsePreferred))
        .unwrap();
    for report in [&dense, &sparse] {
        assert_eq!(report.hard_validity, HardValidity::Valid);
        assert!(report.hard_residuals_validated);
        assert!(report.hard_residual_max <= 1.0e-9);
        assert!(report.rank_is_valid);
        assert_eq!(report.rank, expected_rank);
        assert_eq!(report.structural_nnz, report.structural.structural_nnz);
    }
    assert_eq!(dense.actual_backend, Some(LinearSolveBackend::Dense));
    assert_eq!(sparse.actual_backend, Some(LinearSolveBackend::SparseQr));
    assert_eq!(sparse.sparse_fallback_reason, None);
    assert_eq!(dense.rank, sparse.rank);
    assert_eq!(dense.left_nullity, sparse.left_nullity);
    assert_eq!(dense.right_nullity, sparse.right_nullity);
    assert_eq!(
        dense.local_degrees_of_freedom,
        sparse.local_degrees_of_freedom
    );
    assert_eq!(
        dense.bidirectional_degrees_of_freedom,
        sparse.bidirectional_degrees_of_freedom
    );
    assert_eq!(dense.one_sided_mobility, sparse.one_sided_mobility);
    assert_eq!(dense.hard_termination, sparse.hard_termination);
    assert_eq!(dense.hard_validity, sparse.hard_validity);
    assert_eq!(dense.rank_is_valid, sparse.rank_is_valid);
    assert_eq!(dense.is_singular, sparse.is_singular);
    assert_eq!(dense.near_singular, sparse.near_singular);
    assert_eq!(
        dense.rank_relative_tolerance.to_bits(),
        sparse.rank_relative_tolerance.to_bits()
    );
    assert!((dense.rank_machine_tolerance - sparse.rank_machine_tolerance).abs() <= 1.0e-12);
    assert!((dense.rank_threshold - sparse.rank_threshold).abs() <= 1.0e-12);
    assert_eq!(dense.singular_values.len(), sparse.singular_values.len());
    for (&dense_value, &sparse_value) in dense.singular_values.iter().zip(&sparse.singular_values) {
        assert!((dense_value - sparse_value).abs() <= 1.0e-9);
    }
    assert_eq!(dense.bounds, sparse.bounds);
    assert_eq!(dense.temporary_status, sparse.temporary_status);
    assert_eq!(dense.preference_status, sparse.preference_status);
    assert_priority_parity(&dense.priority_solves, &sparse.priority_solves);
    assert_eq!(dense.conflicting_sources, sparse.conflicting_sources);
    assert_eq!(dense.redundant_sources, sparse.redundant_sources);
    assert_eq!(
        dense.sources_containing_redundant_rows,
        sparse.sources_containing_redundant_rows
    );
    assert_eq!(dense.redundant_rows, sparse.redundant_rows);
    assert_eq!(dense.redundancy_diagnostics, sparse.redundancy_diagnostics);
    assert_eq!(dense.conflict_diagnostics, sparse.conflict_diagnostics);
    assert_eq!(dense.singular_rows, sparse.singular_rows);
    assert_eq!(dense.structural, sparse.structural);
    assert_component_parity(&dense.component_solves, &sparse.component_solves);
    assert_eq!(
        dense.accepted_state.layout(),
        sparse.accepted_state.layout()
    );
    for (&dense_value, &sparse_value) in dense
        .accepted_state
        .ambient()
        .iter()
        .zip(sparse.accepted_state.ambient())
    {
        assert!((dense_value - sparse_value).abs() <= 1.0e-9);
    }
    assert_audit_parity(&dense.audit, &sparse.audit);
}

#[test]
fn forced_dense_and_sparse_preferred_full_rank_parity() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(3.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(-4.0, 1.0).unwrap());
    add_affine_target(
        &mut problem,
        "full rank",
        vec![x, y],
        vec![vec![1.0, 0.0], vec![1.0, 1.0]],
        vec![1.0, 3.0],
        1.0,
    );
    assert_dense_sparse_parity(problem, 2);
}

#[test]
fn forced_dense_and_sparse_preferred_rank_deficient_damped_parity() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_affine_target(
        &mut problem,
        "dependent rows",
        vec![x, y],
        vec![vec![1.0, 1.0], vec![2.0, 2.0]],
        vec![3.0, 6.0],
        1.0,
    );
    assert_dense_sparse_parity(problem, 1);
}

#[test]
fn forced_dense_and_sparse_preferred_alias_and_explicit_zero_parity() {
    let mut problem = Problem::new();
    let alias = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let root = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let alias_source = problem.add_source(SourceConstraint::new("alias").unwrap());
    let alias_residual = problem
        .add_residual(
            ResidualBlock::exact_alias(
                alias_source,
                alias,
                root,
                VariableKind::Scalar,
                vec![1.0],
                rows(1),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_exact_alias(alias, root, alias_residual)
        .unwrap();
    add_affine_target(
        &mut problem,
        "canonical accumulation",
        vec![alias, root, y],
        vec![vec![1.0, -1.0, 1.0], vec![2.0, 0.0, 0.0]],
        vec![2.0, 2.0],
        1.0,
    );
    assert_dense_sparse_parity(problem, 2);
}

#[test]
fn forced_dense_and_sparse_preferred_bound_release_parity() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_affine_target(
        &mut problem,
        "inward target",
        vec![x],
        vec![vec![1.0]],
        vec![1.0],
        1.0,
    );
    problem
        .add_bound(CoordinateBound::new(x, 0, Some(0.0), Some(2.0), "x interval").unwrap())
        .unwrap();
    assert_dense_sparse_parity(problem, 1);
}

#[test]
fn forced_dense_and_sparse_preferred_scale_extreme_parity() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut problem = Problem::new();
        let x = problem.add_variable(VariableBlock::scalar(-2.0 * scale, scale).unwrap());
        add_affine_target(
            &mut problem,
            "scaled target",
            vec![x],
            vec![vec![1.0]],
            vec![3.0 * scale],
            scale,
        );
        assert_dense_sparse_parity(problem, 1);
    }
}

#[test]
fn forced_dense_and_sparse_near_rank_threshold_parity() {
    for (coefficient, expected_rank) in [(0.99e-10, 1), (1.01e-10, 2)] {
        let mut problem = Problem::new();
        let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        add_affine_target(
            &mut problem,
            "near rank threshold",
            vec![x, y],
            vec![vec![1.0, 0.0], vec![0.0, coefficient]],
            vec![1.0, coefficient],
            1.0,
        );
        assert_dense_sparse_parity(problem, expected_rank);
    }
}

#[test]
fn forced_dense_and_sparse_secondary_audit_and_status_parity() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(-3.0, 1.0).unwrap());
    add_affine_target(
        &mut problem,
        "secondary parity hard",
        vec![x, y],
        vec![vec![1.0, 0.0]],
        vec![0.0],
        1.0,
    );
    add_secondary_affine_target(
        &mut problem,
        "secondary parity temporary",
        ResidualCategory::Temporary,
        vec![x, y],
        vec![vec![0.0, 1.0]],
        vec![1.0],
    );
    assert_dense_sparse_parity(problem, 1);
}

#[derive(Clone, Copy, Debug)]
struct NonFiniteResidual;

impl ResidualEvaluator for NonFiniteResidual {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Ok(vec![f64::NAN])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[test]
fn forced_dense_and_sparse_preferred_both_reject_nonfinite_before_linear_solve() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source = problem.add_source(SourceConstraint::new("nonfinite").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![x],
                1,
                vec![1.0],
                rows(1),
                NonFiniteResidual,
            )
            .unwrap(),
        )
        .unwrap();
    for policy in [
        LinearSolveBackendPolicy::DenseOnly,
        LinearSolveBackendPolicy::SparsePreferred,
    ] {
        let mut candidate = problem.clone();
        let report = candidate.solve(backend_config(policy)).unwrap();
        assert_ne!(report.hard_validity, HardValidity::Valid);
        assert!(!report.hard_residuals_validated);
        assert_eq!(report.actual_backend, None);
    }
}

#[test]
fn auto_keeps_tiny_components_dense_and_zero_iteration_evidence_is_empty() {
    let mut active = Problem::new();
    let x = active.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_affine_target(
        &mut active,
        "tiny auto",
        vec![x],
        vec![vec![1.0]],
        vec![1.0],
        1.0,
    );
    let report = active.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.actual_backend, Some(LinearSolveBackend::Dense));

    let mut exact = Problem::new();
    let x = exact.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    add_affine_target(
        &mut exact,
        "exact sparse request",
        vec![x],
        vec![vec![1.0]],
        vec![1.0],
        1.0,
    );
    let report = exact
        .solve(backend_config(LinearSolveBackendPolicy::SparsePreferred))
        .unwrap();
    assert_eq!(report.iterations, 0);
    assert_eq!(report.actual_backend, None);
    assert!(!report.symbolic_analysis_reused);
    assert_eq!(report.symbolic_analysis_reuse_count, 0);
}

#[derive(Clone, Copy, Debug)]
struct UnitCircle;

impl ResidualEvaluator for UnitCircle {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(x), VariableValue::Scalar(y)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![x * x + y * y - 1.0])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(x), VariableValue::Scalar(y)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![
            LocalJacobian::new(1, 1, vec![2.0 * x]),
            LocalJacobian::new(1, 1, vec![2.0 * y]),
        ])
    }
}

#[test]
fn priority_hard_reprojection_is_included_in_backend_evidence() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let hard_source = problem.add_source(SourceConstraint::new("unit circle").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                hard_source,
                ResidualCategory::Hard,
                vec![x, y],
                1,
                vec![1.0],
                rows(1),
                UnitCircle,
            )
            .unwrap(),
        )
        .unwrap();
    let temporary_source = problem.add_source(SourceConstraint::new("move y").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                temporary_source,
                ResidualCategory::Temporary,
                vec![y],
                1,
                vec![1.0],
                rows(1),
                AffineScalars {
                    matrix: vec![vec![1.0]],
                    target: vec![0.5],
                },
            )
            .unwrap(),
        )
        .unwrap();

    let report = problem
        .solve(backend_config(LinearSolveBackendPolicy::SparsePreferred))
        .unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.component_solves[0].iterations, 0);
    assert_eq!(report.actual_backend, Some(LinearSolveBackend::SparseQr));
    assert!(report.symbolic_analysis_reused);
    assert!(report.symbolic_analysis_reuse_count > 0);
}

fn session_residual(
    source: geosolve_core::SourceConstraintId,
    variable: VariableId,
    target: f64,
    scale: f64,
) -> ResidualBlock {
    ResidualBlock::new(
        source,
        ResidualCategory::Hard,
        vec![variable],
        1,
        vec![scale],
        rows(1),
        AffineScalars {
            matrix: vec![vec![1.0]],
            target: vec![target],
        },
    )
    .unwrap()
}

#[test]
fn session_reuses_symbolic_across_scale_edits_and_rejected_clone_retains_cache() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source = problem.add_source(SourceConstraint::new("session target").unwrap());
    let residual = problem
        .add_residual(session_residual(source, x, 0.5, 1.0))
        .unwrap();
    problem
        .add_bound(CoordinateBound::new(x, 0, Some(0.0), Some(1.0), "session bound").unwrap())
        .unwrap();
    let config = backend_config(LinearSolveBackendPolicy::SparsePreferred);
    let mut session = SolveSession::new(problem, config).unwrap();
    assert_eq!(
        session.report().actual_backend,
        Some(LinearSolveBackend::SparseQr)
    );
    assert!(!session.report().symbolic_analysis_reused);

    let reused = session
        .apply(SessionPatch::new(session.revisions()))
        .unwrap();
    assert!(reused.committed());
    assert!(reused.report.component_solves[0].reused);
    assert_eq!(reused.report.component_solves[0].iterations, 0);
    assert_eq!(reused.report.component_solves[0].actual_backend, None);
    assert!(!reused.report.component_solves[0].symbolic_analysis_reused);

    let mut scale_edit = SessionPatch::new(session.revisions());
    scale_edit.replace_residual(residual, session_residual(source, x, 0.25, 2.0));
    let accepted = session.apply(scale_edit).unwrap();
    assert!(accepted.committed());
    assert!(accepted.report.symbolic_analysis_reused);
    assert!(accepted.report.symbolic_analysis_reuse_count >= 1);

    let retained_report = session.report().clone();
    let retained_revisions = session.revisions();
    let mut rejected_edit = SessionPatch::new(retained_revisions);
    rejected_edit.replace_residual(residual, session_residual(source, x, 2.0, 2.0));
    let rejected = session.apply(rejected_edit).unwrap();
    assert!(!rejected.committed());
    assert_eq!(session.report(), &retained_report);
    assert_eq!(session.revisions(), retained_revisions);

    let mut next_edit = SessionPatch::new(retained_revisions);
    next_edit.replace_residual(residual, session_residual(source, x, 0.75, 4.0));
    let next = session.apply(next_edit).unwrap();
    assert!(next.committed());
    assert!(next.report.symbolic_analysis_reused);
    assert!(next.report.symbolic_analysis_reuse_count >= 1);
}

#[test]
fn preference_group_protects_each_connected_temporary_group_independently() {
    let mut problem = Problem::new();
    let first = [
        problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap()),
        problem.add_variable(VariableBlock::scalar(3.0, 1.0).unwrap()),
        problem.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap()),
    ];
    let second = [
        problem.add_variable(VariableBlock::scalar(-1.0, 1.0).unwrap()),
        problem.add_variable(VariableBlock::scalar(-4.0, 1.0).unwrap()),
        problem.add_variable(VariableBlock::scalar(3.0, 1.0).unwrap()),
    ];
    for (index, variables) in [first, second].into_iter().enumerate() {
        add_affine_target(
            &mut problem,
            &format!("hard component {index}"),
            variables.to_vec(),
            vec![vec![1.0, 0.0, 0.0]],
            vec![0.0],
            1.0,
        );
        add_secondary_affine_target(
            &mut problem,
            &format!("temporary component {index}"),
            ResidualCategory::Temporary,
            variables.to_vec(),
            vec![vec![0.0, 1.0, 0.0]],
            vec![0.0],
        );
    }
    add_secondary_affine_target(
        &mut problem,
        "cross preference",
        ResidualCategory::Preference,
        vec![first[2], second[2]],
        vec![vec![1.0, -1.0]],
        vec![0.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert!(scalar(&problem, first[0]).abs() <= 1.0e-12);
    assert!(scalar(&problem, first[1]).abs() <= 1.0e-12);
    assert!(scalar(&problem, second[0]).abs() <= 1.0e-12);
    assert!(scalar(&problem, second[1]).abs() <= 1.0e-12);
    assert!((scalar(&problem, first[2]) - 0.5).abs() <= 1.0e-9);
    assert!((scalar(&problem, second[2]) - 0.5).abs() <= 1.0e-9);
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(preference.component_indices.len(), 2);
    assert_eq!(preference.protected_temporary.len(), 2);
    assert!(
        preference.protected_temporary.iter().all(|protected| {
            protected.preserved
                && protected
                    .final_cost
                    .is_some_and(|cost| cost <= protected.preservation_tolerance)
        }),
        "{preference:#?}"
    );
    assert!(
        report
            .component_solves
            .iter()
            .all(|component| component.secondary_participated)
    );
}

#[test]
fn large_unbounded_cross_component_group_uses_validated_projected_cgls() {
    const COMPONENTS: usize = 128;
    let mut problem = Problem::new();
    let mut variables = Vec::new();
    for index in 0..COMPONENTS {
        let anchor = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
        let free = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
        add_affine_target(
            &mut problem,
            &format!("projected hard {index}"),
            vec![anchor, free],
            vec![vec![1.0, 0.0]],
            vec![0.0],
            1.0,
        );
        add_secondary_affine_target(
            &mut problem,
            &format!("projected temporary {index}"),
            ResidualCategory::Temporary,
            vec![anchor, free],
            vec![vec![1.0, 0.0]],
            vec![0.0],
        );
        variables.push(free);
    }
    add_secondary_affine_target(
        &mut problem,
        "large coupled preference",
        ResidualCategory::Preference,
        variables.clone(),
        vec![vec![1.0; COMPONENTS]],
        vec![0.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    let priority = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(priority.component_indices.len(), COMPONENTS);
    assert_eq!(priority.backend, Some(PrioritySolveBackend::ProjectedCgls));
    assert_eq!(priority.largest_explicit_nullspace_block_rows, 2);
    assert_eq!(priority.protected_temporary.len(), COMPONENTS);
    assert!(
        priority
            .protected_temporary
            .iter()
            .all(|protected| protected.preserved)
    );
    assert!(priority.final_cost.unwrap() <= 1.0e-24);
    assert!(
        variables
            .iter()
            .all(|&variable| scalar(&problem, variable).abs() <= 1.0e-12)
    );
}

#[test]
fn coupled_dense_working_set_preserves_bounds_without_freezing_other_components() {
    let mut problem = Problem::new();
    let fixed_by_bound = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let movable = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    problem
        .add_bound(
            CoordinateBound::new(
                fixed_by_bound,
                0,
                None,
                Some(0.0),
                "cross-group upper bound",
            )
            .unwrap(),
        )
        .unwrap();
    add_secondary_affine_target(
        &mut problem,
        "bounded coupled temporary",
        ResidualCategory::Temporary,
        vec![fixed_by_bound, movable],
        vec![vec![1.0, -1.0]],
        vec![0.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!(scalar(&problem, fixed_by_bound).abs() <= f64::EPSILON);
    assert!(scalar(&problem, movable).abs() <= 1.0e-12);
    assert_eq!(
        report.priority_solves[0].backend,
        Some(PrioritySolveBackend::DenseBlockNullspace)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn hierarchy_only_session_edit_reuses_hard_components_and_reruns_coupled_group() {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let second = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let bound = problem
        .add_bound(
            CoordinateBound::new(first, 0, Some(-10.0), Some(10.0), "session broad bound").unwrap(),
        )
        .unwrap();
    let (source, residual) = add_secondary_affine_target(
        &mut problem,
        "session coupled temporary",
        ResidualCategory::Temporary,
        vec![first, second],
        vec![vec![1.0, -1.0]],
        vec![0.0],
    );
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let initial_stamps = session.component_dependency_stamps().to_vec();
    let replacement = ResidualBlock::new(
        source,
        ResidualCategory::Temporary,
        vec![first, second],
        1,
        vec![1.0],
        rows(1),
        AffineScalars {
            matrix: vec![vec![1.0, -1.0]],
            target: vec![2.0],
        },
    )
    .unwrap();
    let mut patch = SessionPatch::new(session.revisions());
    patch.replace_residual(residual, replacement);
    let transaction = session.apply(patch).unwrap();

    assert!(transaction.committed(), "{transaction:#?}");
    assert!(transaction.report.component_solves.iter().all(|component| {
        component.reused && component.iterations == 0 && component.secondary_participated
    }));
    assert!(
        transaction
            .report
            .component_solves
            .iter()
            .all(|component| component.state_changed_by_secondary)
    );
    assert_eq!(transaction.report.priority_solves[0].iterations, 1);
    assert!((scalar(session.problem(), first) - 1.0).abs() <= 1.0e-9);
    assert!((scalar(session.problem(), second) + 1.0).abs() <= 1.0e-9);
    for (before, after) in initial_stamps
        .iter()
        .zip(session.component_dependency_stamps())
    {
        assert!(after.state_revision > before.state_revision);
        assert!(after.source_revision > before.source_revision);
        assert_eq!(after.state_revision, transaction.revisions.state);
        assert_eq!(after.source_revision, transaction.revisions.source);
    }

    let before_source_stamps = session.component_dependency_stamps().to_vec();
    let mut source_patch = SessionPatch::new(session.revisions());
    source_patch.replace_source(
        source,
        SourceConstraint::new("renamed coupled temporary").unwrap(),
    );
    let source_transaction = session.apply(source_patch).unwrap();
    assert!(source_transaction.committed());
    assert!(
        source_transaction
            .report
            .component_solves
            .iter()
            .all(|component| {
                component.reused && component.iterations == 0 && component.secondary_participated
            })
    );
    assert!(
        source_transaction
            .report
            .component_solves
            .iter()
            .all(|component| !component.state_changed_by_secondary)
    );
    for (before, after) in before_source_stamps
        .iter()
        .zip(session.component_dependency_stamps())
    {
        assert!(after.state_revision > before.state_revision);
        assert!(after.source_revision > before.source_revision);
        assert_eq!(after.state_revision, source_transaction.revisions.state);
        assert_eq!(after.source_revision, source_transaction.revisions.source);
    }

    let mut variable_patch = SessionPatch::new(session.revisions());
    variable_patch.set_variable_value(first, VariableValue::Scalar(0.0));
    let variable_transaction = session.apply(variable_patch).unwrap();
    assert!(variable_transaction.committed());
    assert!(!variable_transaction.report.component_solves[0].reused);
    assert!(variable_transaction.report.component_solves[1].reused);
    assert!(
        variable_transaction
            .report
            .component_solves
            .iter()
            .all(|component| component.secondary_participated)
    );

    let mut bound_patch = SessionPatch::new(session.revisions());
    bound_patch.replace_bound(
        bound,
        CoordinateBound::new(
            first,
            0,
            Some(-10.0),
            Some(0.0),
            "session active upper bound",
        )
        .unwrap(),
    );
    let bound_transaction = session.apply(bound_patch).unwrap();
    assert!(bound_transaction.committed(), "{bound_transaction:#?}");
    assert!(!bound_transaction.report.component_solves[0].reused);
    assert!(bound_transaction.report.component_solves[1].reused);
    assert!(scalar(session.problem(), first) <= 0.0);
    assert!(
        (scalar(session.problem(), first) - scalar(session.problem(), second) - 2.0).abs()
            <= 1.0e-9
    );
}

#[test]
fn coupled_positive_cost_affine_minimum_receives_curvature_certification() {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap());
    let second = problem.add_variable(VariableBlock::scalar(3.0, 1.0).unwrap());
    let hard_fixed = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_affine_target(
        &mut problem,
        "hard-fixed bounded coordinate",
        vec![hard_fixed],
        vec![vec![1.0]],
        vec![0.0],
        1.0,
    );
    problem
        .add_bound(
            CoordinateBound::new(first, 0, Some(-10.0), Some(10.0), "inactive broad bound")
                .unwrap(),
        )
        .unwrap();
    problem
        .add_bound(
            CoordinateBound::new(
                hard_fixed,
                0,
                Some(-1.0e-6),
                Some(1.0e-6),
                "narrow bound on immobile coordinate",
            )
            .unwrap(),
        )
        .unwrap();
    add_secondary_affine_target(
        &mut problem,
        "inconsistent coupled temporary",
        ResidualCategory::Temporary,
        vec![first, second, hard_fixed],
        vec![vec![1.0, -1.0, 0.0], vec![1.0, -1.0, 0.0]],
        vec![0.0, 1.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert!(
        (scalar(&problem, first) - scalar(&problem, second) - 0.5).abs() <= 1.0e-9,
        "first={}, second={}, report={report:#?}",
        scalar(&problem, first),
        scalar(&problem, second)
    );
    assert!((report.priority_solves[0].final_cost.unwrap() - 0.25).abs() <= 1.0e-12);
    assert_eq!(
        report.priority_solves[0].status,
        SecondaryStatus::Acceptable
    );
    assert_eq!(scalar(&problem, hard_fixed).to_bits(), 0.0_f64.to_bits());
}

#[test]
fn coupled_stationary_maximum_escapes_only_after_negative_curvature_is_found() {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let second = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source = problem.add_source(SourceConstraint::new("coupled maximum").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Temporary,
                vec![first, second],
                1,
                vec![1.0],
                rows(1),
                CoupledMaximum,
            )
            .unwrap(),
        )
        .unwrap();
    let jacobians = problem.check_jacobians(1.0e-6).unwrap();
    assert!(jacobians.all_within(1.0e-8), "{jacobians:#?}");

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!((scalar(&problem, first) - scalar(&problem, second)).abs() >= 1.0 - 1.0e-9);
    assert!(
        report.priority_solves[0].final_cost.unwrap() <= 1.0e-20,
        "first={}, second={}, report={report:#?}",
        scalar(&problem, first),
        scalar(&problem, second)
    );
}

#[test]
fn preference_only_edit_refreshes_reused_temporary_cost_and_audit_at_moved_state() {
    let mut problem = Problem::new();
    let anchor = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let free = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_affine_target(
        &mut problem,
        "audit hard manifold",
        vec![anchor, free],
        vec![vec![1.0, 0.0]],
        vec![0.0],
        1.0,
    );
    let (_, temporary) = add_secondary_affine_target(
        &mut problem,
        "audit temporary manifold",
        ResidualCategory::Temporary,
        vec![anchor, free],
        vec![vec![1.0, 0.0]],
        vec![0.0],
    );
    let (preference_source, preference) = add_secondary_affine_target(
        &mut problem,
        "editable preference",
        ResidualCategory::Preference,
        vec![anchor, free],
        vec![vec![0.0, 1.0]],
        vec![0.0],
    );
    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let replacement = ResidualBlock::new(
        preference_source,
        ResidualCategory::Preference,
        vec![anchor, free],
        1,
        vec![1.0],
        rows(1),
        AffineScalars {
            matrix: vec![vec![0.0, 1.0]],
            target: vec![2.0],
        },
    )
    .unwrap();
    let mut patch = SessionPatch::new(session.revisions());
    patch.replace_residual(preference, replacement);
    let transaction = session.apply(patch).unwrap();

    assert!(transaction.committed(), "{transaction:#?}");
    assert!((scalar(session.problem(), free) - 2.0).abs() <= 1.0e-9);
    assert!(transaction.report.component_solves[0].reused);
    let temporary_report = transaction
        .report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Temporary)
        .unwrap();
    assert_eq!(temporary_report.iterations, 0);
    assert_eq!(temporary_report.final_cost, Some(0.0));
    let preference_report = transaction
        .report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(preference_report.status, SecondaryStatus::Optimal);
    assert_eq!(preference_report.protected_temporary.len(), 1);
    assert!(preference_report.protected_temporary[0].preserved);
    assert_eq!(
        preference_report.protected_temporary[0].final_cost,
        Some(0.0)
    );

    let temporary_audit = transaction
        .report
        .audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .find(|row| row.residual_id == temporary)
        .unwrap();
    assert_eq!(
        temporary_audit.evaluation_status,
        AuditEvaluationStatus::Evaluated
    );
    assert!(temporary_audit.normalized_residual.abs() <= f64::EPSILON);
    let free_snapshot = temporary_audit
        .incident_variables
        .iter()
        .find(|snapshot| snapshot.variable_id == free)
        .unwrap();
    assert_eq!(free_snapshot.value, VariableValue::Scalar(2.0));
}

#[test]
fn large_bounded_interior_optimum_uses_operator_without_global_dense_nullspace() {
    const COMPONENTS: usize = 128;
    let mut problem = Problem::new();
    let variables = (0..COMPONENTS)
        .map(|_| problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap()))
        .collect::<Vec<_>>();
    problem
        .add_bound(
            CoordinateBound::new(
                variables[0],
                0,
                Some(-2.0),
                Some(2.0),
                "large coupled bound",
            )
            .unwrap(),
        )
        .unwrap();
    add_secondary_affine_target(
        &mut problem,
        "large bounded temporary",
        ResidualCategory::Temporary,
        variables.clone(),
        vec![vec![1.0; COMPONENTS]],
        vec![0.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
    let priority = &report.priority_solves[0];
    assert_eq!(priority.backend, Some(PrioritySolveBackend::ProjectedCgls));
    assert_eq!(priority.component_indices.len(), COMPONENTS);
    assert_eq!(priority.largest_explicit_nullspace_block_rows, 1);
    assert_eq!(priority.iterations, 1);
    assert!(
        variables
            .iter()
            .all(|&variable| scalar(&problem, variable).abs() <= 1.0e-12)
    );
}

#[test]
fn large_bounded_lower_and_upper_optima_reach_endpoints_without_false_optimality() {
    const COMPONENTS: usize = 128;
    let mut problem = Problem::new();
    let variables = (0..COMPONENTS)
        .map(|index| {
            let value = match index {
                0 => 0.5,
                1 => -0.5,
                _ => 0.0,
            };
            problem.add_variable(VariableBlock::scalar(value, 1.0).unwrap())
        })
        .collect::<Vec<_>>();
    problem
        .add_bound(
            CoordinateBound::new(variables[0], 0, Some(0.0), Some(2.0), "active lower").unwrap(),
        )
        .unwrap();
    problem
        .add_bound(
            CoordinateBound::new(variables[1], 0, Some(-2.0), Some(0.0), "active upper").unwrap(),
        )
        .unwrap();
    let mut targets = vec![0.0; COMPONENTS];
    targets[0] = -1.0;
    targets[1] = 1.0;
    add_secondary_affine_target(
        &mut problem,
        "large endpoint temporary",
        ResidualCategory::Temporary,
        variables.clone(),
        diagonal_matrix(COMPONENTS),
        targets,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Stalled, "{report:#?}");
    assert_eq!(report.temporary_status, SecondaryStatus::Stalled);
    assert_ne!(report.temporary_status, SecondaryStatus::Optimal);
    assert!(scalar(&problem, variables[0]).abs() <= f64::EPSILON);
    assert!(scalar(&problem, variables[1]).abs() <= f64::EPSILON);
    let priority = &report.priority_solves[0];
    assert_eq!(priority.backend, Some(PrioritySolveBackend::ProjectedCgls));
    assert_eq!(priority.largest_explicit_nullspace_block_rows, 1);
    assert!((priority.final_cost.unwrap() - 1.0).abs() <= 1.0e-12);
}

#[test]
fn large_bounded_operator_releases_an_initially_active_lower_bound() {
    const COMPONENTS: usize = 128;
    let mut problem = Problem::new();
    let variables = (0..COMPONENTS)
        .map(|_| problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()))
        .collect::<Vec<_>>();
    problem
        .add_bound(
            CoordinateBound::new(variables[0], 0, Some(0.0), Some(2.0), "released lower").unwrap(),
        )
        .unwrap();
    let mut targets = vec![0.0; COMPONENTS];
    targets[0] = 1.0;
    add_secondary_affine_target(
        &mut problem,
        "large released-bound temporary",
        ResidualCategory::Temporary,
        variables.clone(),
        diagonal_matrix(COMPONENTS),
        targets,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
    assert!((scalar(&problem, variables[0]) - 1.0).abs() <= 1.0e-12);
    assert_eq!(
        report.priority_solves[0].backend,
        Some(PrioritySolveBackend::ProjectedCgls)
    );
}

#[test]
fn large_bounded_operator_deduplicates_alias_bound_normals() {
    const ACTIVE_COORDINATES: usize = 128;
    let mut problem = Problem::new();
    let alias = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let root = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let alias_source = problem.add_source(SourceConstraint::new("bounded alias").unwrap());
    let alias_residual = problem
        .add_residual(
            ResidualBlock::exact_alias(
                alias_source,
                alias,
                root,
                VariableKind::Scalar,
                vec![1.0],
                rows(1),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_exact_alias(alias, root, alias_residual)
        .unwrap();
    let mut variables = vec![alias];
    variables.extend(
        (1..ACTIVE_COORDINATES)
            .map(|_| problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap())),
    );
    for (variable, label) in [(alias, "alias lower"), (root, "root lower")] {
        problem
            .add_bound(CoordinateBound::new(variable, 0, Some(0.0), Some(2.0), label).unwrap())
            .unwrap();
    }
    let mut targets = vec![0.0; ACTIVE_COORDINATES];
    targets[0] = 1.0;
    add_secondary_affine_target(
        &mut problem,
        "alias-normal temporary",
        ResidualCategory::Temporary,
        variables,
        diagonal_matrix(ACTIVE_COORDINATES),
        targets,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
    assert!((scalar(&problem, alias) - 1.0).abs() <= 1.0e-12);
    assert!((scalar(&problem, root) - 1.0).abs() <= 1.0e-12);
    assert_eq!(
        report.priority_solves[0].backend,
        Some(PrioritySolveBackend::ProjectedCgls)
    );
}

#[test]
fn large_bounded_preference_preserves_zero_cost_temporary_rows() {
    const COMPONENTS: usize = 128;
    let mut problem = Problem::new();
    let variables = (0..COMPONENTS)
        .map(|_| problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()))
        .collect::<Vec<_>>();
    problem
        .add_bound(
            CoordinateBound::new(
                variables[COMPONENTS - 1],
                0,
                Some(-2.0),
                Some(2.0),
                "large protected broad bound",
            )
            .unwrap(),
        )
        .unwrap();
    add_secondary_affine_target(
        &mut problem,
        "protected first coordinate",
        ResidualCategory::Temporary,
        vec![variables[0]],
        vec![vec![1.0]],
        vec![0.0],
    );
    let mut targets = vec![1.0; COMPONENTS];
    targets[0] = 0.0;
    add_secondary_affine_target(
        &mut problem,
        "large protected preference",
        ResidualCategory::Preference,
        variables.clone(),
        diagonal_matrix(COMPONENTS),
        targets,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.preference_status, SecondaryStatus::Optimal);
    assert!(scalar(&problem, variables[0]).abs() <= f64::EPSILON);
    assert!(
        variables[1..]
            .iter()
            .all(|&variable| (scalar(&problem, variable) - 1.0).abs() <= 1.0e-12)
    );
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(
        preference.backend,
        Some(PrioritySolveBackend::ProjectedCgls)
    );
    assert_eq!(preference.protected_temporary.len(), 1);
    assert!(preference.protected_temporary[0].preserved);
    assert!(preference.protected_temporary[0].final_cost.unwrap() <= 1.0e-24);
}

#[test]
fn large_bounded_operator_matches_the_small_dense_active_set_oracle() {
    let (mut dense, dense_variables) = bounded_oracle_fixture(8);
    let dense_report = dense.solve(SolverConfig::default()).unwrap();
    let (mut operator, operator_variables) = bounded_oracle_fixture(128);
    let operator_report = operator.solve(SolverConfig::default()).unwrap();

    assert_eq!(dense_report.termination, SolveTermination::Stalled);
    assert_eq!(operator_report.termination, SolveTermination::Stalled);
    assert_eq!(
        dense_report.priority_solves[0].backend,
        Some(PrioritySolveBackend::DenseBlockNullspace)
    );
    assert_eq!(
        operator_report.priority_solves[0].backend,
        Some(PrioritySolveBackend::ProjectedCgls)
    );
    for index in 0..dense_variables.len() {
        assert!(
            (scalar(&dense, dense_variables[index]) - scalar(&operator, operator_variables[index]))
                .abs()
                <= 1.0e-9
        );
    }
    assert!(
        (dense_report.priority_solves[0].final_cost.unwrap()
            - operator_report.priority_solves[0].final_cost.unwrap())
        .abs()
            <= 1.0e-12
    );
}

#[test]
fn large_bounded_operator_never_evaluates_outside_coordinate_bounds() {
    const COMPONENTS: usize = 128;
    let outside = Arc::new(AtomicUsize::new(0));
    let mut oracle = Problem::new();
    let oracle_variables = (0..COMPONENTS)
        .map(|_| oracle.add_variable(VariableBlock::scalar(0.5, 1.0).unwrap()))
        .collect::<Vec<_>>();
    let oracle_source = oracle.add_source(SourceConstraint::new("bound-check oracle").unwrap());
    oracle
        .add_residual(
            ResidualBlock::new(
                oracle_source,
                ResidualCategory::Temporary,
                oracle_variables,
                COMPONENTS,
                vec![1.0; COMPONENTS],
                rows(COMPONENTS),
                BoundCheckedTargets {
                    targets: vec![0.5; COMPONENTS],
                    lower: 0.0,
                    upper: 1.0,
                    outside: Arc::clone(&outside),
                },
            )
            .unwrap(),
        )
        .unwrap();
    let jacobians = oracle.check_jacobians(1.0e-7).unwrap();
    assert!(jacobians.all_within(1.0e-6), "{jacobians:#?}");

    let mut problem = Problem::new();
    let variables = (0..COMPONENTS)
        .map(|_| problem.add_variable(VariableBlock::scalar(0.5, 1.0).unwrap()))
        .collect::<Vec<_>>();
    problem
        .add_bound(
            CoordinateBound::new(variables[0], 0, Some(0.0), Some(1.0), "checked lower").unwrap(),
        )
        .unwrap();
    let mut targets = vec![0.5; COMPONENTS];
    targets[0] = -1.0;
    let source = problem.add_source(SourceConstraint::new("bound-checked temporary").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Temporary,
                variables.clone(),
                COMPONENTS,
                vec![1.0; COMPONENTS],
                rows(COMPONENTS),
                BoundCheckedTargets {
                    targets,
                    lower: 0.0,
                    upper: 1.0,
                    outside: Arc::clone(&outside),
                },
            )
            .unwrap(),
        )
        .unwrap();

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Stalled, "{report:#?}");
    assert!(scalar(&problem, variables[0]).abs() <= f64::EPSILON);
    assert_eq!(outside.load(Ordering::Relaxed), 0);
    assert_eq!(
        report.priority_solves[0].backend,
        Some(PrioritySolveBackend::ProjectedCgls)
    );
}

#[test]
fn large_unbounded_positive_cost_group_stalls_without_curvature_basis() {
    const COMPONENTS: usize = 128;
    let mut problem = Problem::new();
    let variables = (0..COMPONENTS)
        .map(|_| problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()))
        .collect::<Vec<_>>();
    add_secondary_affine_target(
        &mut problem,
        "large inconsistent temporary",
        ResidualCategory::Temporary,
        variables.clone(),
        vec![vec![1.0; COMPONENTS], vec![1.0; COMPONENTS]],
        vec![0.0, 1.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Stalled);
    assert_eq!(report.temporary_status, SecondaryStatus::Stalled);
    let priority = &report.priority_solves[0];
    assert_eq!(priority.backend, Some(PrioritySolveBackend::ProjectedCgls));
    assert_eq!(priority.component_indices.len(), COMPONENTS);
    assert_eq!(priority.largest_explicit_nullspace_block_rows, 1);
    assert!((priority.final_cost.unwrap() - 0.25).abs() <= 1.0e-12);
    let sum = variables
        .iter()
        .map(|&variable| scalar(&problem, variable))
        .sum::<f64>();
    assert!((sum - 0.5).abs() <= 1.0e-9);
}

#[test]
fn multiscale_coupled_curvature_does_not_mask_a_cross_component_saddle() {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let second = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source = problem.add_source(SourceConstraint::new("coupled masked maximum").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Temporary,
                vec![first, second],
                1,
                vec![1.0],
                rows(1),
                CoupledMaskedMaximum,
            )
            .unwrap(),
        )
        .unwrap();
    let jacobians = problem.check_jacobians(1.0e-7).unwrap();
    assert!(jacobians.all_within(1.0e-6), "{jacobians:#?}");
    let report = problem
        .solve(SolverConfig {
            max_iterations: 1,
            ..SolverConfig::default()
        })
        .unwrap();

    assert_eq!(report.termination, SolveTermination::Stalled);
    assert_eq!(report.temporary_status, SecondaryStatus::Stalled);
    assert_ne!(report.temporary_status, SecondaryStatus::Optimal);
}

#[test]
fn zero_cost_large_bounded_group_is_optimal_without_selecting_a_basis_backend() {
    const COMPONENTS: usize = 128;
    let mut problem = Problem::new();
    let variables = (0..COMPONENTS)
        .map(|_| problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()))
        .collect::<Vec<_>>();
    problem
        .add_bound(
            CoordinateBound::new(
                variables[0],
                0,
                Some(-1.0),
                Some(1.0),
                "zero-cost large bound",
            )
            .unwrap(),
        )
        .unwrap();
    add_secondary_affine_target(
        &mut problem,
        "zero-cost large temporary",
        ResidualCategory::Temporary,
        variables,
        vec![vec![1.0; COMPONENTS]],
        vec![0.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
    let priority = &report.priority_solves[0];
    assert_eq!(priority.backend, None);
    assert_eq!(priority.iterations, 0);
    assert_eq!(priority.final_cost, Some(0.0));
    assert_eq!(priority.largest_explicit_nullspace_block_rows, 0);
}

#[test]
fn preference_cannot_worsen_two_independent_temporary_optima() {
    let mut problem = Problem::new();
    let first = [
        problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()),
        problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()),
    ];
    let second = [
        problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()),
        problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()),
    ];
    let mut temporary_ids = Vec::new();
    for (index, (variables, target)) in [(first, 1.0), (second, -1.0)].into_iter().enumerate() {
        add_affine_target(
            &mut problem,
            &format!("strict priority hard {index}"),
            variables.to_vec(),
            vec![vec![1.0, 0.0]],
            vec![0.0],
            1.0,
        );
        temporary_ids.push(
            add_secondary_affine_target(
                &mut problem,
                &format!("strict priority temporary {index}"),
                ResidualCategory::Temporary,
                variables.to_vec(),
                vec![vec![0.0, 1.0]],
                vec![target],
            )
            .1,
        );
    }
    let (_, preference_id) = add_secondary_affine_target(
        &mut problem,
        "directly conflicting preference",
        ResidualCategory::Preference,
        vec![first[1], second[1]],
        vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        vec![0.0, 0.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert!((scalar(&problem, first[1]) - 1.0).abs() <= 1.0e-12);
    assert!((scalar(&problem, second[1]) + 1.0).abs() <= 1.0e-12);
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(preference.status, SecondaryStatus::Acceptable);
    assert!((preference.initial_cost.unwrap() - 1.0).abs() <= 1.0e-12);
    assert!((preference.final_cost.unwrap() - 1.0).abs() <= 1.0e-12);
    assert_eq!(preference.protected_temporary.len(), 2);
    for protected in &preference.protected_temporary {
        assert!(protected.preserved);
        let final_cost = protected.final_cost.unwrap();
        assert!(final_cost <= protected.preservation_tolerance);
        assert!(final_cost <= protected.attained_cost + protected.preservation_tolerance);
    }
    for temporary_id in temporary_ids {
        let row = report
            .audit
            .sources
            .iter()
            .flat_map(|source| &source.rows)
            .find(|row| row.residual_id == temporary_id)
            .unwrap();
        assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
        assert!(row.normalized_residual.abs() <= 1.0e-12);
    }
    let preference_rows = report
        .audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .filter(|row| row.residual_id == preference_id)
        .collect::<Vec<_>>();
    assert_eq!(preference_rows.len(), 2);
    assert!(preference_rows[0].normalized_residual.abs() >= 1.0 - 1.0e-12);
    assert!(preference_rows[1].normalized_residual.abs() >= 1.0 - 1.0e-12);
}

#[test]
fn constrained_large_singleton_uses_implicit_hard_projector_without_nullspace_basis() {
    const VARIABLES: usize = 128;
    let mut problem = Problem::new();
    let variables = (0..VARIABLES)
        .map(|_| problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()))
        .collect::<Vec<_>>();
    for index in 1..VARIABLES {
        add_affine_target(
            &mut problem,
            &format!("large singleton connector {index}"),
            vec![variables[index - 1], variables[index]],
            vec![vec![-1.0, 1.0]],
            vec![0.0],
            1.0,
        );
    }
    add_secondary_affine_target(
        &mut problem,
        "large singleton temporary",
        ResidualCategory::Temporary,
        vec![variables[0]],
        vec![vec![1.0]],
        vec![1.0],
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
    let priority = &report.priority_solves[0];
    assert_eq!(priority.component_indices, vec![0]);
    assert_eq!(priority.backend, Some(PrioritySolveBackend::ProjectedCgls));
    assert_eq!(priority.iterations, 1);
    assert_eq!(priority.largest_explicit_nullspace_block_rows, 0);
    assert!(
        variables
            .iter()
            .all(|&variable| (scalar(&problem, variable) - 1.0).abs() <= 1.0e-9)
    );
}

fn sparse_ambiguous_chain(anchor_coefficient: f64) -> Problem {
    const VARIABLES: usize = 256;
    let mut problem = Problem::new();
    let variables = (0..VARIABLES)
        .map(|index| {
            let value = if index.is_multiple_of(2) { 0.1 } else { -0.1 };
            problem.add_variable(VariableBlock::scalar(value, 1.0).unwrap())
        })
        .collect::<Vec<_>>();
    add_affine_target(
        &mut problem,
        "ambiguous sparse anchor",
        vec![variables[0]],
        vec![vec![anchor_coefficient]],
        vec![0.0],
        1.0,
    );
    for index in 1..VARIABLES {
        add_affine_target(
            &mut problem,
            &format!("ambiguous sparse edge {index}"),
            vec![variables[index - 1], variables[index]],
            vec![vec![-1.0, 1.0]],
            vec![0.0],
            1.0,
        );
    }
    problem
}

#[test]
fn auto_uses_dense_with_rank_ambiguous_evidence_for_large_sparse_shapes() {
    for (anchor_coefficient, expected_rank, expected_near_singular) in
        [(0.0, 255, false), (1.0e-7, 256, true)]
    {
        let mut problem = sparse_ambiguous_chain(anchor_coefficient);
        let report = problem
            .solve(SolverConfig {
                max_iterations: 1,
                initial_damping: 1.0e-20,
                minimum_damping: 1.0e-20,
                redundancy_diagnostic_budget: DiagnosticBudget {
                    enabled: false,
                    ..DiagnosticBudget::unlimited()
                },
                ..SolverConfig::default()
            })
            .unwrap();
        assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
        assert_eq!(report.rank, expected_rank);
        assert_eq!(report.near_singular, expected_near_singular);
        assert_eq!(report.requested_backend, LinearSolveBackendPolicy::Auto);
        assert_eq!(report.actual_backend, Some(LinearSolveBackend::Dense));
        assert_eq!(
            report.sparse_fallback_reason,
            Some(SparseFallbackReason::RankAmbiguous)
        );
        assert_eq!(report.symbolic_analysis_reuse_count, 0);
    }
}

#[test]
fn auto_routes_a_large_full_rank_sparse_shape_to_sparse_qr() {
    let mut problem = sparse_ambiguous_chain(1.0);
    let report = problem
        .solve(SolverConfig {
            max_iterations: 1,
            initial_damping: 1.0e-15,
            minimum_damping: 1.0e-15,
            redundancy_diagnostic_budget: DiagnosticBudget {
                enabled: false,
                ..DiagnosticBudget::unlimited()
            },
            ..SolverConfig::default()
        })
        .unwrap();

    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert_eq!(report.rank, 256);
    assert!(!report.near_singular);
    assert_eq!(report.requested_backend, LinearSolveBackendPolicy::Auto);
    assert_eq!(report.actual_backend, Some(LinearSolveBackend::SparseQr));
    assert_eq!(report.sparse_fallback_reason, None);
}

fn scalar_square_session(value: f64) -> SolveSession {
    let mut problem = Problem::new();
    let state = problem.add_variable(VariableBlock::scalar(value, 1.0).unwrap());
    let source = problem.add_source(SourceConstraint::new("scalar fold section").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![state],
                1,
                vec![1.0],
                rows(1),
                ScalarSquare {
                    target: value * value,
                },
            )
            .unwrap(),
        )
        .unwrap();
    SolveSession::new(
        problem,
        SolverConfig {
            normalized_residual_tolerance: 1.0e-12,
            ..SolverConfig::default()
        },
    )
    .unwrap()
}

#[test]
fn augmented_tangent_crosses_a_scalar_fold_with_explicit_orientation() {
    let regular = scalar_square_session(0.1);
    let regular_linearization = regular.accepted_hard_linearization().unwrap();
    let regular_component = &regular_linearization.components()[0];
    let parameter_column = DVector::from_vec(vec![-1.0]);
    let increasing = regular_component
        .augmented_unit_null_tangent(
            &parameter_column,
            &ContinuationTangentOrientation::Initial(InitialParameterDirection::Increasing),
        )
        .unwrap();
    assert!(increasing.normalized_state()[0] > 0.0);
    assert!(increasing.parameter_component() > 0.0);
    assert!(increasing.equation_residual_max() <= regular.config().normalized_residual_tolerance);
    assert_eq!(increasing.augmented_rank(), 1);
    assert_eq!(
        increasing.rank_threshold().to_bits(),
        regular_component.rank_threshold().to_bits()
    );

    let decreasing = regular_component
        .augmented_unit_null_tangent(
            &parameter_column,
            &ContinuationTangentOrientation::Initial(InitialParameterDirection::Decreasing),
        )
        .unwrap();
    assert!(decreasing.normalized_state()[0] < 0.0);
    assert!(decreasing.parameter_component() < 0.0);

    let fold = scalar_square_session(0.0);
    let fold_linearization = fold.accepted_hard_linearization().unwrap();
    let fold_component = &fold_linearization.components()[0];
    let forward = fold_component
        .augmented_unit_null_tangent(
            &parameter_column,
            &ContinuationTangentOrientation::Previous(increasing),
        )
        .unwrap();
    assert!((forward.normalized_state()[0] - 1.0).abs() <= 1.0e-14);
    assert!(forward.parameter_component().abs() <= 1.0e-14);
    let backward = fold_component
        .augmented_unit_null_tangent(
            &parameter_column,
            &ContinuationTangentOrientation::Previous(decreasing),
        )
        .unwrap();
    assert!((backward.normalized_state()[0] + 1.0).abs() <= 1.0e-14);
    assert!(backward.parameter_component().abs() <= 1.0e-14);

    assert_eq!(
        fold_component
            .augmented_unit_null_tangent(
                &parameter_column,
                &ContinuationTangentOrientation::Initial(InitialParameterDirection::Increasing,),
            )
            .unwrap_err(),
        ContinuationError::AmbiguousOrientation
    );
}

#[test]
fn augmented_tangent_rejects_bad_columns_and_nonunique_nullspaces() {
    let regular = scalar_square_session(0.1);
    let accepted = regular.accepted_hard_linearization().unwrap();
    let component = &accepted.components()[0];
    let orientation =
        ContinuationTangentOrientation::Initial(InitialParameterDirection::Increasing);
    assert_eq!(
        component
            .augmented_unit_null_tangent(&DVector::zeros(0), &orientation)
            .unwrap_err(),
        ContinuationError::DimensionMismatch {
            context: "augmented continuation parameter column",
            expected: 1,
            actual: 0,
        }
    );
    assert!(matches!(
        component.augmented_unit_null_tangent(&DVector::from_vec(vec![f64::NAN]), &orientation),
        Err(ContinuationError::NonFiniteValue {
            context: "augmented continuation parameter column",
            index: 0,
            value,
        }) if value.is_nan()
    ));

    let mut under = Problem::new();
    let first = under.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let second = under.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    add_affine(
        &mut under,
        "two-dimensional augmented nullspace",
        vec![first, second],
        vec![vec![1.0, 0.0]],
        1.0,
    );
    let session = SolveSession::new(under, SolverConfig::default()).unwrap();
    let accepted = session.accepted_hard_linearization().unwrap();
    let error = accepted.components()[0]
        .augmented_unit_null_tangent(&DVector::from_vec(vec![0.0]), &orientation)
        .unwrap_err();
    assert!(matches!(
        error,
        ContinuationError::UnexpectedAugmentedNullity {
            right_nullity: 2,
            ..
        }
    ));
}

fn pose_pair_linearization(matrix: Vec<Vec<f64>>) -> SolveSession {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::pose2([0.0; 3], [1.0; 3]).unwrap());
    let second = problem.add_variable(VariableBlock::pose2([0.0; 3], [1.0; 3]).unwrap());
    let source = problem.add_source(SourceConstraint::new("multi-body tangent fixture").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![first, second],
                matrix.len(),
                vec![1.0; matrix.len()],
                rows(matrix.len()),
                PosePairLinear { matrix },
            )
            .unwrap(),
        )
        .unwrap();
    SolveSession::new(problem, SolverConfig::default()).unwrap()
}

#[test]
fn augmented_pose2_tangent_uses_strict_threshold_and_previous_orientation() {
    let identity = (0..6)
        .map(|row| {
            (0..6)
                .map(|column| if row == column { 1.0 } else { 0.0 })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let full_session = pose_pair_linearization(identity.clone());
    let full = full_session.accepted_hard_linearization().unwrap();
    let full_component = &full.components()[0];
    assert_eq!(full_component.tangent_blocks().len(), 2);
    assert!(
        full_component
            .tangent_blocks()
            .iter()
            .all(|block| block.kind == VariableKind::Pose2)
    );
    let mut regular_parameter = DVector::zeros(6);
    regular_parameter[5] = -1.0;
    let increasing = full_component
        .augmented_unit_null_tangent(
            &regular_parameter,
            &ContinuationTangentOrientation::Initial(InitialParameterDirection::Increasing),
        )
        .unwrap();
    let decreasing = full_component
        .augmented_unit_null_tangent(
            &regular_parameter,
            &ContinuationTangentOrientation::Initial(InitialParameterDirection::Decreasing),
        )
        .unwrap();
    assert!(increasing.normalized_state()[5] > 0.0);
    assert!(increasing.parameter_component() > 0.0);
    assert!(decreasing.normalized_state()[5] < 0.0);
    assert!(decreasing.parameter_component() < 0.0);

    let mut deficient = identity;
    deficient[5].fill(0.0);
    let deficient_session = pose_pair_linearization(deficient);
    let deficient = deficient_session.accepted_hard_linearization().unwrap();
    let component = &deficient.components()[0];
    let threshold = component.rank_threshold();
    let mut below = DVector::zeros(6);
    below[5] = threshold * (1.0 - 1.0e-6);
    assert!(matches!(
        component.augmented_unit_null_tangent(
            &below,
            &ContinuationTangentOrientation::Previous(increasing.clone()),
        ),
        Err(ContinuationError::UnexpectedAugmentedNullity {
            right_nullity: 2,
            ..
        })
    ));

    let mut above = DVector::zeros(6);
    above[5] = threshold * (1.0 + 1.0e-6);
    let forward = component
        .augmented_unit_null_tangent(
            &above,
            &ContinuationTangentOrientation::Previous(increasing),
        )
        .unwrap();
    let backward = component
        .augmented_unit_null_tangent(
            &above,
            &ContinuationTangentOrientation::Previous(decreasing),
        )
        .unwrap();
    assert!(forward.normalized_state()[5] > 0.0);
    assert!(forward.parameter_component().abs() <= 1.0e-12);
    assert!(backward.normalized_state()[5] < 0.0);
    assert!(backward.parameter_component().abs() <= 1.0e-12);
    assert_eq!(forward.augmented_rank(), 6);
}

#[test]
fn adaptive_controller_retries_at_the_exact_minimum_and_enforces_budgets() {
    let policy = AdaptiveStepPolicy {
        initial_step: 0.3,
        minimum_step: 0.1,
        maximum_step: 0.6,
        growth_factor: 2.0,
        shrink_factor: 0.5,
        fast_iterations: 2,
        slow_iterations: 5,
        small_correction: 0.1,
        large_correction: 0.5,
        maximum_correction: 0.25,
        maximum_correction_step_ratio: 1.0,
        max_retries: 4,
        max_samples: 2,
    };
    let mut controller = AdaptiveStepController::new(policy).unwrap();
    assert!((controller.correction_limit(0.1).unwrap() - 0.1).abs() <= f64::EPSILON);
    assert!((controller.correction_limit(1.0).unwrap() - 0.25).abs() <= f64::EPSILON);
    assert_eq!(
        controller.correction_limit(0.0).unwrap_err(),
        ContinuationError::InvalidPathStep { value: 0.0 }
    );
    assert_eq!(controller.reject(), AdaptiveStepDecision::Retry);
    assert!((controller.current_step() - 0.15).abs() <= f64::EPSILON);
    assert_eq!(controller.reject(), AdaptiveStepDecision::Retry);
    assert!((controller.current_step() - 0.1).abs() <= f64::EPSILON);
    assert_eq!(controller.reject(), AdaptiveStepDecision::MinimumStep);
    assert_eq!(
        controller.current_step().to_bits(),
        policy.minimum_step.to_bits()
    );

    controller.accept(2, 0.1).unwrap();
    assert_eq!(controller.retries(), 0);
    assert_eq!(controller.accepted_samples(), 1);
    assert!((controller.current_step() - 0.2).abs() <= f64::EPSILON);
    controller.accept(5, 0.5).unwrap();
    assert_eq!(controller.accepted_samples(), 2);
    assert!(controller.sample_limit_reached());
    assert!((controller.current_step() - 0.1).abs() <= f64::EPSILON);
    assert!(matches!(
        controller.accept(0, -1.0),
        Err(ContinuationError::InvalidCorrectionNorm { value: -1.0 })
    ));
    assert!(matches!(
        controller.accept(0, f64::NAN),
        Err(ContinuationError::InvalidCorrectionNorm { value }) if value.is_nan()
    ));

    let mut retry_limited = AdaptiveStepController::new(AdaptiveStepPolicy {
        max_retries: 1,
        ..policy
    })
    .unwrap();
    assert_eq!(retry_limited.reject(), AdaptiveStepDecision::Retry);
    assert_eq!(retry_limited.reject(), AdaptiveStepDecision::RetryLimit);

    assert!(
        AdaptiveStepController::new(AdaptiveStepPolicy {
            shrink_factor: 1.0,
            ..policy
        })
        .is_err()
    );
    assert!(
        AdaptiveStepController::new(AdaptiveStepPolicy {
            maximum_correction_step_ratio: 0.0,
            ..policy
        })
        .is_err()
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn pseudo_arclength_row_matches_manifold_finite_differences_at_all_scales() {
    for model_scale in [1.0e-6, 1.0, 1.0e6] {
        let mut problem = Problem::new();
        let mut variables = Vec::new();
        let mut signed_distance = 0.0;
        let mut add_block = |variable_id,
                             reference,
                             step_scales: Vec<f64>,
                             tangent: Vec<f64>,
                             difference: &[f64]| {
            signed_distance += tangent
                .iter()
                .zip(difference)
                .zip(&step_scales)
                .map(|((&tangent, &difference), &scale)| tangent * difference / scale)
                .sum::<f64>();
            variables.push(PseudoArclengthVariable::new(variable_id, reference, tangent).unwrap());
        };

        let scalar_reference = 1.2 * model_scale;
        let scalar_difference = [0.17 * model_scale];
        let scalar = problem.add_variable(
            VariableBlock::scalar(scalar_reference + scalar_difference[0], 0.8 * model_scale)
                .unwrap(),
        );
        add_block(
            scalar,
            VariableValue::Scalar(scalar_reference),
            vec![0.8 * model_scale],
            vec![0.11],
            &scalar_difference,
        );

        let vec2_reference = [-0.7 * model_scale, 0.9 * model_scale];
        let vec2_difference = [0.13 * model_scale, -0.19 * model_scale];
        let vector2 = problem.add_variable(
            VariableBlock::vec2(
                [
                    vec2_reference[0] + vec2_difference[0],
                    vec2_reference[1] + vec2_difference[1],
                ],
                [0.7 * model_scale, 1.1 * model_scale],
            )
            .unwrap(),
        );
        add_block(
            vector2,
            VariableValue::Vec2(vec2_reference),
            vec![0.7 * model_scale, 1.1 * model_scale],
            vec![-0.07, 0.09],
            &vec2_difference,
        );

        let vec3_reference = [0.4 * model_scale, -1.3 * model_scale, 0.2 * model_scale];
        let vec3_difference = [-0.08 * model_scale, 0.16 * model_scale, 0.21 * model_scale];
        let vector3 = problem.add_variable(
            VariableBlock::vec3(
                [
                    vec3_reference[0] + vec3_difference[0],
                    vec3_reference[1] + vec3_difference[1],
                    vec3_reference[2] + vec3_difference[2],
                ],
                [0.9 * model_scale, 1.2 * model_scale, 0.6 * model_scale],
            )
            .unwrap(),
        );
        add_block(
            vector3,
            VariableValue::Vec3(vec3_reference),
            vec![0.9 * model_scale, 1.2 * model_scale, 0.6 * model_scale],
            vec![0.05, -0.08, 0.06],
            &vec3_difference,
        );

        let pose2_reference =
            GeometryPose2::from_ambient([0.3 * model_scale, -0.6 * model_scale, 0.4]).unwrap();
        let pose2_difference = [0.14 * model_scale, -0.12 * model_scale, 0.23];
        let pose2_value = pose2_reference.retract(pose2_difference).unwrap();
        let pose2 = problem.add_variable(
            VariableBlock::pose2(
                pose2_value.ambient(),
                [0.75 * model_scale, 1.3 * model_scale, 0.8],
            )
            .unwrap(),
        );
        add_block(
            pose2,
            VariableValue::Pose2(pose2_reference.ambient()),
            vec![0.75 * model_scale, 1.3 * model_scale, 0.8],
            vec![-0.04, 0.07, 0.1],
            &pose2_difference,
        );

        let pose3_reference = GeometryPose3::exp([
            -0.2 * model_scale,
            0.5 * model_scale,
            0.8 * model_scale,
            0.2,
            -0.3,
            0.1,
        ])
        .unwrap();
        let pose3_difference = [
            0.11 * model_scale,
            -0.09 * model_scale,
            0.15 * model_scale,
            -0.17,
            0.12,
            0.08,
        ];
        let pose3_value = pose3_reference.retract(pose3_difference).unwrap();
        let pose3 = problem.add_variable(
            VariableBlock::pose3(
                pose3_value.ambient(),
                [
                    0.8 * model_scale,
                    1.1 * model_scale,
                    1.4 * model_scale,
                    0.7,
                    0.9,
                    1.2,
                ],
            )
            .unwrap(),
        );
        add_block(
            pose3,
            VariableValue::Pose3(pose3_reference.ambient()),
            vec![
                0.8 * model_scale,
                1.1 * model_scale,
                1.4 * model_scale,
                0.7,
                0.9,
                1.2,
            ],
            vec![0.03, -0.04, 0.06, 0.08, -0.05, 0.07],
            &pose3_difference,
        );

        let source = problem.add_source(SourceConstraint::new("pseudo-arclength control").unwrap());
        let residual = problem
            .add_pseudo_arclength(source, &variables, signed_distance)
            .unwrap();
        let assembly = problem.assemble_dense().unwrap();
        assert!(
            assembly.residuals()[0].abs() <= 2.0e-9,
            "scale={model_scale:e}, assembly={assembly:#?}"
        );
        let jacobians = problem.check_jacobians(1.0e-6).unwrap();
        assert!(
            jacobians.all_within(1.0e-6),
            "scale={model_scale:e}, jacobians={jacobians:#?}"
        );
        let audit = problem.audit_rows().unwrap();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].residual_id, residual);
        assert_eq!(audit[0].category, ResidualCategory::Hard);
        assert!(audit[0].template.contains("normalized_local_difference"));
        assert_eq!(audit[0].unit, "normalized arclength");
    }
}

#[test]
fn pseudo_arclength_projects_alias_incidence_to_the_reduced_root() {
    let mut problem = Problem::new();
    let alias = problem.add_variable(VariableBlock::scalar(1.4, 2.0).unwrap());
    let root = problem.add_variable(VariableBlock::scalar(1.4, 2.0).unwrap());
    let alias_source = problem.add_source(SourceConstraint::new("continuation alias").unwrap());
    let alias_residual = problem
        .add_residual(
            ResidualBlock::exact_alias(
                alias_source,
                alias,
                root,
                VariableKind::Scalar,
                vec![1.0],
                rows(1),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_exact_alias(alias, root, alias_residual)
        .unwrap();
    let control_source = problem.add_source(SourceConstraint::new("alias path control").unwrap());
    problem
        .add_pseudo_arclength(
            control_source,
            &[
                PseudoArclengthVariable::new(alias, VariableValue::Scalar(1.0), vec![0.75])
                    .unwrap(),
            ],
            0.15,
        )
        .unwrap();

    let session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let accepted = session.accepted_hard_linearization().unwrap();
    assert_eq!(accepted.components().len(), 1);
    let component = &accepted.components()[0];
    assert_eq!(component.tangent_blocks().len(), 1);
    assert_eq!(component.tangent_blocks()[0].root, root);
    assert_eq!(component.tangent_blocks()[0].alias_members, vec![alias]);
    assert_eq!(component.normalized_jacobian().shape(), (1, 1));
    assert!((component.normalized_jacobian()[(0, 0)] - 0.75).abs() <= f64::EPSILON);
    assert!(component.normalized_residual()[0].abs() <= f64::EPSILON);
}

#[test]
fn pseudo_arclength_rejects_invalid_coefficients_and_pose_log_cuts() {
    let smallest_subnormal = f64::from_bits(1);
    assert!(
        PseudoArclengthVariable::new(
            VariableId::default(),
            VariableValue::Scalar(0.0),
            Vec::new(),
        )
        .is_err()
    );

    let mut accessor_problem = Problem::new();
    let accessor_id = accessor_problem.add_variable(VariableBlock::scalar(0.0, 2.0).unwrap());
    let accessor =
        PseudoArclengthVariable::new(accessor_id, VariableValue::Scalar(1.0), vec![0.5]).unwrap();
    assert_eq!(accessor.variable_id(), accessor_id);
    assert_eq!(accessor.reference(), VariableValue::Scalar(1.0));
    assert_eq!(accessor.normalized_tangent(), &[0.5]);
    let accessor_source =
        accessor_problem.add_source(SourceConstraint::new("sealed pseudo block").unwrap());
    accessor_problem
        .add_pseudo_arclength(accessor_source, &[accessor], 0.0)
        .unwrap();
    let accessor_assembly = accessor_problem.assemble_dense().unwrap();
    assert!((accessor_assembly.residuals()[0] + 0.25).abs() <= f64::EPSILON);
    assert!((accessor_assembly.jacobian()[(0, 0)] - 0.5).abs() <= f64::EPSILON);

    let mut mismatched = Problem::new();
    let pose = mismatched.add_variable(VariableBlock::pose2([0.0; 3], [1.0; 3]).unwrap());
    let source = mismatched.add_source(SourceConstraint::new("mismatched pseudo block").unwrap());
    let mismatched_variable =
        PseudoArclengthVariable::new(pose, VariableValue::Scalar(0.0), vec![1.0]).unwrap();
    assert!(
        mismatched
            .add_pseudo_arclength(source, &[mismatched_variable], 0.0)
            .is_err()
    );

    let mut overflowing = Problem::new();
    let tiny_scale =
        overflowing.add_variable(VariableBlock::scalar(0.0, smallest_subnormal).unwrap());
    let source = overflowing.add_source(SourceConstraint::new("overflowing pseudo scale").unwrap());
    let overflowing_variable =
        PseudoArclengthVariable::new(tiny_scale, VariableValue::Scalar(0.0), vec![1.0]).unwrap();
    assert!(
        overflowing
            .add_pseudo_arclength(source, &[overflowing_variable], 0.0)
            .is_err()
    );

    let pose3_cut = GeometryPose3::exp([0.0, 0.0, 0.0, std::f64::consts::PI, 0.0, 0.0])
        .unwrap()
        .ambient();
    for (reference, value, scales, tangent) in [
        (
            VariableValue::Pose2([0.0, 0.0, 0.0]),
            VariableValue::Pose2([0.0, 0.0, std::f64::consts::PI]),
            vec![1.0; 3],
            vec![1.0, 0.0, 0.0],
        ),
        (
            VariableValue::Pose3(GeometryPose3::identity().ambient()),
            VariableValue::Pose3(pose3_cut),
            vec![1.0; 6],
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ),
    ] {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::new(value, scales.clone()).unwrap());
        let source = problem.add_source(SourceConstraint::new("pseudo log cut").unwrap());
        problem
            .add_pseudo_arclength(
                source,
                &[PseudoArclengthVariable::new(variable, reference, tangent).unwrap()],
                0.0,
            )
            .unwrap();
        assert!(problem.assemble_dense().is_err());
    }
}

fn pseudo_fold_problem() -> Problem {
    let reference_state = 0.2;
    let reference_parameter = reference_state * reference_state;
    let state_tangent = -1.0_f64 / 1.16_f64.sqrt();
    let parameter_tangent = 0.4 * state_tangent;
    let signed_distance = 0.25;
    let mut problem = Problem::new();
    let state = problem.add_variable(
        VariableBlock::scalar(reference_state + signed_distance * state_tangent, 1.0).unwrap(),
    );
    let parameter = problem.add_variable(
        VariableBlock::scalar(
            reference_parameter + signed_distance * parameter_tangent,
            1.0,
        )
        .unwrap(),
    );
    let fold_source = problem.add_source(SourceConstraint::new("scalar fold").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                fold_source,
                ResidualCategory::Hard,
                vec![state, parameter],
                1,
                vec![1.0],
                rows(1),
                ScalarFold,
            )
            .unwrap(),
        )
        .unwrap();
    let control_source = problem.add_source(SourceConstraint::new("fold pseudo control").unwrap());
    problem
        .add_pseudo_arclength(
            control_source,
            &[
                PseudoArclengthVariable::new(
                    state,
                    VariableValue::Scalar(reference_state),
                    vec![state_tangent],
                )
                .unwrap(),
                PseudoArclengthVariable::new(
                    parameter,
                    VariableValue::Scalar(reference_parameter),
                    vec![parameter_tangent],
                )
                .unwrap(),
            ],
            signed_distance,
        )
        .unwrap();
    problem
}

#[test]
fn pseudo_arclength_corrector_crosses_fold_with_dense_sparse_parity() {
    let problem = pseudo_fold_problem();
    let jacobians = problem.check_jacobians(1.0e-6).unwrap();
    assert!(jacobians.all_within(1.0e-6), "{jacobians:#?}");

    let mut crossed = problem.clone();
    let report = crossed
        .solve(backend_config(LinearSolveBackendPolicy::DenseOnly))
        .unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    let layout = report.accepted_state.layout().blocks();
    let state_id = layout[0].variable_id;
    let parameter_id = layout[1].variable_id;
    let state = scalar(&crossed, state_id);
    let parameter = scalar(&crossed, parameter_id);
    assert!(state < 0.0, "state={state}, parameter={parameter}");
    assert!(parameter > 0.0, "state={state}, parameter={parameter}");
    assert!((state * state - parameter).abs() <= 1.0e-9);

    assert_dense_sparse_parity(problem, 2);
}
