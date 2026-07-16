use std::ops::Range;

use nalgebra::{DMatrix, DVector};

use crate::analysis::{EliminationPlan, SolveComponent};
use crate::problem::VariableState;
use crate::residual::{JacobianCoordinates, LinearizationStorage, LocalJacobianStorage};
use crate::{
    CoreError, EvaluationError, LocalJacobian, Problem, ResidualBlock, ResidualCategory,
    ResidualId, SourceConstraintId, VariableId, VariableValue,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EvaluationStatus {
    Evaluated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowIdentity {
    pub(crate) residual_id: ResidualId,
    pub(crate) source_id: SourceConstraintId,
    pub(crate) row_in_block: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct LinearizedJacobianBlock {
    pub(crate) variable_id: VariableId,
    pub(crate) rows: usize,
    pub(crate) columns: usize,
    pub(crate) normalized_values: Vec<f64>,
    pub(crate) status: EvaluationStatus,
}

#[derive(Clone, Debug)]
pub(crate) struct LinearizedResidualBlock {
    pub(crate) residual_id: ResidualId,
    pub(crate) source_id: SourceConstraintId,
    pub(crate) category: ResidualCategory,
    pub(crate) normalized_residuals: Vec<f64>,
    pub(crate) rows: Vec<RowIdentity>,
    pub(crate) jacobian_blocks: Vec<LinearizedJacobianBlock>,
    pub(crate) status: EvaluationStatus,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BlockLinearization {
    pub(crate) blocks: Vec<LinearizedResidualBlock>,
    pub(crate) scalar_rows: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveTangentBlock {
    pub(crate) root: VariableId,
    pub(crate) members: Vec<VariableId>,
    pub(crate) tangent_range: Range<usize>,
    pub(crate) step_scales: Vec<f64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ComponentTangentLayout {
    pub(crate) blocks: Vec<ActiveTangentBlock>,
    pub(crate) tangent_dimension: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ComponentLinearization {
    pub(crate) layout: ComponentTangentLayout,
    pub(crate) numeric: BlockLinearization,
}

#[derive(Debug)]
pub(crate) struct ComponentDenseSystem {
    pub(crate) residuals: DVector<f64>,
    pub(crate) jacobian: DMatrix<f64>,
    pub(crate) rows: Vec<RowIdentity>,
}

impl Problem {
    pub(crate) fn linearize_blocks_for_state(
        &self,
        state: &VariableState,
        residual_filter: Option<&[ResidualId]>,
    ) -> Result<BlockLinearization, CoreError> {
        validate_state(self, state)?;
        let mut result = BlockLinearization::default();
        for (residual_id, residual) in self.residuals.iter() {
            if residual_filter.is_some_and(|filter| !filter.contains(&residual_id)) {
                continue;
            }
            let variables = incident_values(residual, state)?;
            let block = evaluate_block(self, residual_id, residual, &variables)?;
            result.scalar_rows = result
                .scalar_rows
                .checked_add(block.normalized_residuals.len())
                .ok_or(CoreError::DimensionOverflow {
                    context: "packed residual",
                })?;
            result.blocks.push(block);
        }
        Ok(result)
    }

    pub(crate) fn linearize_component(
        &self,
        plan: &EliminationPlan,
        component: &SolveComponent,
        state: &VariableState,
        residual_filter: &[ResidualId],
    ) -> Result<ComponentLinearization, CoreError> {
        let layout = plan.component_layouts.get(component.index).cloned().ok_or(
            CoreError::DimensionMismatch {
                context: "cached component tangent layout",
                expected: plan.components.len(),
                actual: component.index,
            },
        )?;
        Ok(ComponentLinearization {
            layout,
            numeric: self.linearize_blocks_for_state(state, Some(residual_filter))?,
        })
    }

    pub(crate) fn validate_residual_linearization(
        &self,
        state: &VariableState,
        residual_id: ResidualId,
    ) -> Result<(), CoreError> {
        let linearization = self.linearize_blocks_for_state(state, Some(&[residual_id]))?;
        if linearization.blocks.len() == 1 {
            Ok(())
        } else {
            Err(CoreError::UnknownResidual(residual_id))
        }
    }
}

impl ComponentLinearization {
    pub(crate) fn project_dense(
        &self,
        plan: &EliminationPlan,
        category: ResidualCategory,
    ) -> Result<ComponentDenseSystem, CoreError> {
        let selected_rows = self
            .numeric
            .blocks
            .iter()
            .filter(|block| {
                block.category == category
                    && (category != ResidualCategory::Hard
                        || (!plan.is_eliminated(block.residual_id)
                            && !plan.source_is_suppressed(block.source_id)))
            })
            .try_fold(0usize, |rows, block| {
                rows.checked_add(block.normalized_residuals.len())
            })
            .ok_or(CoreError::DimensionOverflow {
                context: "component dense residual",
            })?;
        selected_rows
            .checked_mul(self.layout.tangent_dimension)
            .ok_or(CoreError::DimensionOverflow {
                context: "component dense Jacobian",
            })?;
        let mut residuals = DVector::zeros(selected_rows);
        let mut jacobian = DMatrix::zeros(selected_rows, self.layout.tangent_dimension);
        let mut rows = Vec::with_capacity(selected_rows);
        let mut target_row = 0;

        for block in &self.numeric.blocks {
            if block.category != category
                || (category == ResidualCategory::Hard
                    && (plan.is_eliminated(block.residual_id)
                        || plan.source_is_suppressed(block.source_id)))
            {
                continue;
            }
            debug_assert_eq!(block.status, EvaluationStatus::Evaluated);
            for (local_row, &value) in block.normalized_residuals.iter().enumerate() {
                residuals[target_row + local_row] = value;
                rows.push(block.rows[local_row]);
            }
            for local in &block.jacobian_blocks {
                debug_assert_eq!(local.status, EvaluationStatus::Evaluated);
                let Some(active) = self
                    .layout
                    .blocks
                    .iter()
                    .find(|active| active.members.contains(&local.variable_id))
                else {
                    // Fixed coordinates are intentionally validated in the block IR
                    // but have no materialized component column.
                    continue;
                };
                if local.columns != active.tangent_range.len() {
                    return Err(CoreError::DimensionMismatch {
                        context: "component local Jacobian columns",
                        expected: active.tangent_range.len(),
                        actual: local.columns,
                    });
                }
                for local_row in 0..local.rows {
                    for local_column in 0..local.columns {
                        jacobian[(
                            target_row + local_row,
                            active.tangent_range.start + local_column,
                        )] += local.normalized_values[local_row * local.columns + local_column];
                    }
                }
            }
            target_row += block.normalized_residuals.len();
        }
        if residuals
            .iter()
            .chain(jacobian.iter())
            .any(|value| !value.is_finite())
        {
            return Err(CoreError::NonFiniteValue {
                context: "component dense projection",
                index: 0,
                value: f64::NAN,
            });
        }
        Ok(ComponentDenseSystem {
            residuals,
            jacobian,
            rows,
        })
    }
}

pub(crate) fn component_tangent_layout(
    plan: &EliminationPlan,
    component_index: usize,
) -> ComponentTangentLayout {
    let mut layout = ComponentTangentLayout::default();
    for group in &plan.active_groups {
        if group.component_index != component_index {
            continue;
        }
        let start = layout.tangent_dimension;
        let end = start + group.kind.tangent_dimension();
        layout.blocks.push(ActiveTangentBlock {
            root: group.root,
            members: group.members.clone(),
            tangent_range: start..end,
            step_scales: group.step_scales.clone(),
        });
        layout.tangent_dimension = end;
    }
    layout
}

pub(crate) fn evaluate_values(
    residual_id: ResidualId,
    residual: &ResidualBlock,
    variables: &[VariableValue],
) -> Result<Vec<f64>, CoreError> {
    let values = residual
        .evaluator()
        .evaluate(variables)
        .map_err(|error| evaluator_error(residual_id, error))?;
    if values.len() != residual.output_dimension() {
        return Err(CoreError::DimensionMismatch {
            context: "evaluator residual output",
            expected: residual.output_dimension(),
            actual: values.len(),
        });
    }
    validate_finite(&values, "evaluator residual output")?;
    Ok(values)
}

pub(crate) fn normalize_residuals(
    residual: &ResidualBlock,
    values: &[f64],
) -> Result<Vec<f64>, CoreError> {
    values
        .iter()
        .zip(residual.scales())
        .enumerate()
        .map(|(index, (&value, &scale))| {
            let normalized = value / scale;
            if normalized.is_finite() {
                Ok(normalized)
            } else {
                Err(CoreError::NonFiniteValue {
                    context: "normalized residual",
                    index,
                    value: normalized,
                })
            }
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn evaluate_block(
    problem: &Problem,
    residual_id: ResidualId,
    residual: &ResidualBlock,
    variables: &[VariableValue],
) -> Result<LinearizedResidualBlock, CoreError> {
    let mut raw_residuals = vec![f64::NAN; residual.output_dimension()];
    let mut raw_jacobians = residual
        .incident_variables()
        .iter()
        .map(|&variable_id| {
            let variable = problem
                .variables
                .get(variable_id)
                .ok_or(CoreError::UnknownVariable(variable_id))?;
            let values = residual
                .output_dimension()
                .checked_mul(variable.kind().tangent_dimension())
                .ok_or(CoreError::DimensionOverflow {
                    context: "local Jacobian",
                })?;
            Ok(vec![f64::NAN; values])
        })
        .collect::<Result<Vec<_>, CoreError>>()?;

    let (fused_result, jacobian_coordinates) = {
        let mut storage_blocks = residual
            .incident_variables()
            .iter()
            .zip(raw_jacobians.iter_mut())
            .map(|(&variable_id, values)| {
                let variable = problem
                    .variables
                    .get(variable_id)
                    .ok_or(CoreError::UnknownVariable(variable_id))?;
                Ok(LocalJacobianStorage::new(
                    residual.output_dimension(),
                    variable.kind().tangent_dimension(),
                    variable.step_scales(),
                    values,
                ))
            })
            .collect::<Result<Vec<_>, CoreError>>()?;
        let mut storage = LinearizationStorage::new(&mut raw_residuals, &mut storage_blocks);
        let result = residual.evaluator().linearize(variables, &mut storage);
        (result, storage.jacobian_coordinates())
    };

    match fused_result {
        Some(Ok(())) => {
            validate_finite(&raw_residuals, "evaluator residual output")?;
            for values in &raw_jacobians {
                validate_finite(values, "evaluator Jacobian")?;
            }
        }
        None => {
            raw_residuals = evaluate_values(residual_id, residual, variables)?;
            raw_jacobians = evaluate_legacy_jacobians(problem, residual_id, residual, variables)?
                .into_iter()
                .map(|block| block.values().to_vec())
                .collect();
        }
        Some(Err(error)) => return Err(evaluator_error(residual_id, error)),
    }

    let normalized_residuals = normalize_residuals(residual, &raw_residuals)?;
    let jacobian_blocks = residual
        .incident_variables()
        .iter()
        .zip(raw_jacobians)
        .map(|(&variable_id, raw_values)| {
            let variable = problem
                .variables
                .get(variable_id)
                .ok_or(CoreError::UnknownVariable(variable_id))?;
            let columns = variable.kind().tangent_dimension();
            let mut normalized_values = Vec::with_capacity(raw_values.len());
            for row in 0..residual.output_dimension() {
                for column in 0..columns {
                    let variable_normalized = match jacobian_coordinates {
                        JacobianCoordinates::RawTangent => {
                            raw_values[row * columns + column] * variable.step_scales()[column]
                        }
                        JacobianCoordinates::NormalizedTangent => {
                            raw_values[row * columns + column]
                        }
                    };
                    let normalized = variable_normalized / residual.scales()[row];
                    if !normalized.is_finite() {
                        return Err(CoreError::NonFiniteValue {
                            context: "normalized Jacobian",
                            index: row * columns + column,
                            value: normalized,
                        });
                    }
                    normalized_values.push(normalized);
                }
            }
            Ok(LinearizedJacobianBlock {
                variable_id,
                rows: residual.output_dimension(),
                columns,
                normalized_values,
                status: EvaluationStatus::Evaluated,
            })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    let rows = (0..residual.output_dimension())
        .map(|row_in_block| RowIdentity {
            residual_id,
            source_id: residual.source(),
            row_in_block,
        })
        .collect();
    Ok(LinearizedResidualBlock {
        residual_id,
        source_id: residual.source(),
        category: residual.category(),
        normalized_residuals,
        rows,
        jacobian_blocks,
        status: EvaluationStatus::Evaluated,
    })
}

fn evaluate_legacy_jacobians(
    problem: &Problem,
    residual_id: ResidualId,
    residual: &ResidualBlock,
    variables: &[VariableValue],
) -> Result<Vec<LocalJacobian>, CoreError> {
    let jacobians = residual
        .evaluator()
        .jacobian(variables)
        .map_err(|error| evaluator_error(residual_id, error))?;
    if jacobians.len() != residual.incident_variables().len() {
        return Err(CoreError::DimensionMismatch {
            context: "evaluator Jacobian block count",
            expected: residual.incident_variables().len(),
            actual: jacobians.len(),
        });
    }
    for (&variable_id, jacobian) in residual.incident_variables().iter().zip(&jacobians) {
        let variable = problem
            .variables
            .get(variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))?;
        if jacobian.rows() != residual.output_dimension() {
            return Err(CoreError::DimensionMismatch {
                context: "local Jacobian rows",
                expected: residual.output_dimension(),
                actual: jacobian.rows(),
            });
        }
        let columns = variable.kind().tangent_dimension();
        if jacobian.columns() != columns {
            return Err(CoreError::DimensionMismatch {
                context: "local Jacobian columns",
                expected: columns,
                actual: jacobian.columns(),
            });
        }
        let expected_values =
            jacobian
                .rows()
                .checked_mul(columns)
                .ok_or(CoreError::DimensionOverflow {
                    context: "local Jacobian",
                })?;
        if jacobian.values().len() != expected_values {
            return Err(CoreError::DimensionMismatch {
                context: "local Jacobian values",
                expected: expected_values,
                actual: jacobian.values().len(),
            });
        }
        validate_finite(jacobian.values(), "evaluator Jacobian")?;
    }
    Ok(jacobians)
}

fn incident_values(
    residual: &ResidualBlock,
    state: &VariableState,
) -> Result<Vec<VariableValue>, CoreError> {
    residual
        .incident_variables()
        .iter()
        .map(|&variable_id| {
            state
                .values
                .iter()
                .find_map(|&(id, value)| (id == variable_id).then_some(value))
                .ok_or(CoreError::UnknownVariable(variable_id))
        })
        .collect()
}

fn validate_state(problem: &Problem, state: &VariableState) -> Result<(), CoreError> {
    let expected = problem.variables.iter().count();
    if state.values.len() != expected {
        return Err(CoreError::DimensionMismatch {
            context: "solver variable state",
            expected,
            actual: state.values.len(),
        });
    }
    for ((expected_id, variable), &(actual_id, value)) in
        problem.variables.iter().zip(&state.values)
    {
        if actual_id != expected_id {
            return Err(CoreError::UnknownVariable(actual_id));
        }
        if value.kind() != variable.kind() {
            return Err(CoreError::VariableKindMismatch {
                expected: variable.kind(),
                actual: value.kind(),
            });
        }
        value.validate_finite()?;
    }
    Ok(())
}

fn validate_finite(values: &[f64], context: &'static str) -> Result<(), CoreError> {
    for (index, &value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(CoreError::NonFiniteValue {
                context,
                index,
                value,
            });
        }
    }
    Ok(())
}

pub(crate) fn evaluator_error(residual: ResidualId, error: EvaluationError) -> CoreError {
    match error {
        EvaluationError::InvalidGeometry(message) => {
            CoreError::InvalidGeometry { residual, message }
        }
        EvaluationError::Categorized { category, message } => CoreError::CategorizedEvaluation {
            residual,
            category,
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuditBinding, ResidualEvaluator, ResidualRowAudit, SourceConstraint, VariableBlock,
        VariableKind,
    };

    #[derive(Clone, Copy, Debug)]
    struct ScalarLinear {
        coefficients: [f64; 2],
        fail_first_derivative: bool,
    }

    impl ResidualEvaluator for ScalarLinear {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
                return Err(EvaluationError::invalid_geometry("expected two scalars"));
            };
            Ok(vec![
                self.coefficients[0] * first + self.coefficients[1] * second,
            ])
        }

        fn jacobian(
            &self,
            _variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            Ok(vec![
                LocalJacobian::new(
                    1,
                    1,
                    vec![if self.fail_first_derivative {
                        f64::NAN
                    } else {
                        self.coefficients[0]
                    }],
                ),
                LocalJacobian::new(1, 1, vec![self.coefficients[1]]),
            ])
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ScalarTarget;

    impl ResidualEvaluator for ScalarTarget {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let [VariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry("expected one scalar"));
            };
            Ok(vec![*value])
        }

        fn jacobian(
            &self,
            _variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct MustNotEvaluate;

    impl ResidualEvaluator for MustNotEvaluate {
        fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            Err(EvaluationError::ambiguous(
                "excluded residual was evaluated",
            ))
        }

        fn jacobian(
            &self,
            _variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            Err(EvaluationError::ambiguous(
                "excluded residual was evaluated",
            ))
        }
    }

    fn row() -> ResidualRowAudit {
        ResidualRowAudit::new(
            "M9 private row",
            vec![AuditBinding::new("variables", "private fixture")],
            "1",
        )
    }

    fn source(problem: &mut Problem, label: &str) -> SourceConstraintId {
        problem.add_source(SourceConstraint::new(label).unwrap())
    }

    fn add_scalar_target(problem: &mut Problem, variable: VariableId, label: &str) -> ResidualId {
        let source_id = source(problem, label);
        problem
            .add_residual(
                ResidualBlock::new(
                    source_id,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row()],
                    ScalarTarget,
                )
                .unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn component_matrix_width_is_independent_of_disconnected_global_columns() {
        let mut measurements = Vec::new();
        for count in [8, 128] {
            let mut problem = Problem::new();
            for index in 0..count {
                let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
                add_scalar_target(&mut problem, variable, &format!("target {index}"));
            }
            let plan = EliminationPlan::new(&problem).unwrap();
            let component = &plan.components[0];
            let linearization = problem
                .linearize_component(
                    &plan,
                    component,
                    &problem.variable_state(),
                    &component.active_residual_ids,
                )
                .unwrap();
            let dense = linearization
                .project_dense(&plan, ResidualCategory::Hard)
                .unwrap();
            measurements.push((
                problem.packed_layout().unwrap().tangent_dimension(),
                dense.jacobian.shape(),
                dense.jacobian.len(),
            ));
        }
        assert_eq!(measurements, vec![(8, (1, 1), 1), (128, (1, 1), 1)]);
    }

    #[test]
    fn fixed_incidence_blocks_are_validated_but_not_materialized() {
        for fail_first_derivative in [false, true] {
            let mut problem = Problem::new();
            let fixed = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
            let active = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
            let fixed_source = source(&mut problem, "fixed");
            let fixed_residual = problem
                .add_residual(
                    ResidualBlock::fixed_variable(
                        fixed_source,
                        fixed,
                        VariableValue::Scalar(0.0),
                        vec![1.0],
                        vec![row()],
                    )
                    .unwrap(),
                )
                .unwrap();
            let coupling_source = source(&mut problem, "fixed-active coupling");
            let coupling = problem
                .add_residual(
                    ResidualBlock::new(
                        coupling_source,
                        ResidualCategory::Hard,
                        vec![fixed, active],
                        1,
                        vec![1.0],
                        vec![row()],
                        ScalarLinear {
                            coefficients: [2.0, 3.0],
                            fail_first_derivative,
                        },
                    )
                    .unwrap(),
                )
                .unwrap();
            problem
                .declare_fixed_variable(fixed, VariableValue::Scalar(0.0), fixed_residual)
                .unwrap();
            let plan = EliminationPlan::new(&problem).unwrap();
            let component = plan
                .components
                .iter()
                .find(|component| component.active_residual_ids.contains(&coupling))
                .unwrap();
            let result = problem.linearize_component(
                &plan,
                component,
                &problem.variable_state(),
                &component.active_residual_ids,
            );
            if fail_first_derivative {
                assert!(matches!(result, Err(CoreError::NonFiniteValue { .. })));
            } else {
                let linearization = result.unwrap();
                assert_eq!(linearization.numeric.blocks[0].jacobian_blocks.len(), 2);
                let dense = linearization
                    .project_dense(&plan, ResidualCategory::Hard)
                    .unwrap();
                assert_eq!(dense.jacobian.shape(), (1, 1));
                assert!((dense.jacobian[(0, 0)] - 3.0).abs() <= f64::EPSILON);
            }
        }
    }

    #[test]
    fn alias_incidence_blocks_remain_ordered_and_sum_into_the_root_column() {
        let mut problem = Problem::new();
        let alias = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let root = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let alias_source = source(&mut problem, "alias");
        let alias_residual = problem
            .add_residual(
                ResidualBlock::exact_alias(
                    alias_source,
                    alias,
                    root,
                    VariableKind::Scalar,
                    vec![1.0],
                    vec![row()],
                )
                .unwrap(),
            )
            .unwrap();
        let active_source = source(&mut problem, "alias members in one row");
        let active_residual = problem
            .add_residual(
                ResidualBlock::new(
                    active_source,
                    ResidualCategory::Hard,
                    vec![alias, root],
                    1,
                    vec![1.0],
                    vec![row()],
                    ScalarLinear {
                        coefficients: [2.0, 3.0],
                        fail_first_derivative: false,
                    },
                )
                .unwrap(),
            )
            .unwrap();
        problem
            .declare_exact_alias(alias, root, alias_residual)
            .unwrap();
        let plan = EliminationPlan::new(&problem).unwrap();
        let component = plan
            .components
            .iter()
            .find(|component| component.active_residual_ids.contains(&active_residual))
            .unwrap();
        let linearization = problem
            .linearize_component(
                &plan,
                component,
                &problem.variable_state(),
                &component.active_residual_ids,
            )
            .unwrap();
        assert_eq!(
            linearization.numeric.blocks[0]
                .jacobian_blocks
                .iter()
                .map(|block| block.variable_id)
                .collect::<Vec<_>>(),
            vec![alias, root]
        );
        let dense = linearization
            .project_dense(&plan, ResidualCategory::Hard)
            .unwrap();
        assert_eq!(dense.jacobian.shape(), (1, 1));
        assert!((dense.jacobian[(0, 0)] - 5.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn residual_filters_preserve_store_order_and_do_not_evaluate_excluded_rows() {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let first = add_scalar_target(&mut problem, variable, "first");
        let excluded_source = source(&mut problem, "excluded failure");
        let excluded = problem
            .add_residual(
                ResidualBlock::new(
                    excluded_source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row()],
                    MustNotEvaluate,
                )
                .unwrap(),
            )
            .unwrap();
        let second = add_scalar_target(&mut problem, variable, "second");
        let linearization = problem
            .linearize_blocks_for_state(&problem.variable_state(), Some(&[second, first]))
            .unwrap();
        assert_eq!(
            linearization
                .blocks
                .iter()
                .map(|block| block.residual_id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        assert!(
            !linearization
                .blocks
                .iter()
                .any(|block| block.residual_id == excluded)
        );
    }
}
