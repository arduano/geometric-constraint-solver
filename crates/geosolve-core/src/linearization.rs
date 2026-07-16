use std::ops::Range;

use nalgebra::{DMatrix, DVector};

use crate::analysis::{EliminationPlan, SolveComponent};
use crate::problem::VariableState;
use crate::residual::{JacobianCoordinates, LinearizationStorage, LocalJacobianStorage};
use crate::solver::{RankDiagnostics, rank_diagnostics};
use crate::{
    ComponentSolveReport, CoreError, EvaluationError, HardValidity, LocalJacobian, PackedState,
    Problem, ResidualBlock, ResidualCategory, ResidualId, ResidualRowRef, SensitivityError,
    SessionRevisions, SolveReport, SolverConfig, SourceConstraintId, VariableId, VariableKind,
    VariableValue,
};

/// One active reduced tangent block in deterministic component-column order.
#[derive(Clone, Debug, PartialEq)]
pub struct ReducedTangentBlock {
    pub root: VariableId,
    /// Exact aliases sharing the root tangent. The root itself is not repeated.
    pub alias_members: Vec<VariableId>,
    pub kind: VariableKind,
    pub tangent_range: Range<usize>,
    pub step_scales: Vec<f64>,
}

/// One active hard scalar row in deterministic component-row order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReducedHardRow {
    pub row: ResidualRowRef,
    pub residual_scale: f64,
}

/// One raw right/body-local tangent block returned by a sensitivity solve.
#[derive(Clone, Debug, PartialEq)]
pub struct RawTangentBlock {
    pub root: VariableId,
    pub alias_members: Vec<VariableId>,
    pub kind: VariableKind,
    /// Raw local tangent values after applying the block's characteristic step scales.
    pub values: DVector<f64>,
}

/// Classification of `J * delta + normalized_residual_rate = 0`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SensitivityStatus {
    Unique,
    UnderdeterminedMinimumNorm,
    Inconsistent,
}

impl SensitivityStatus {
    /// Whether the independently checked differentiated equation is satisfied.
    #[must_use]
    pub const fn is_success_like(self) -> bool {
        matches!(self, Self::Unique | Self::UnderdeterminedMinimumNorm)
    }
}

/// Accepted-threshold SVD sensitivity result in normalized and raw local coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct SensitivitySolution {
    /// Accepted session revisions inherited from the detached component snapshot.
    pub revisions: SessionRevisions,
    pub status: SensitivityStatus,
    pub normalized_tangent: DVector<f64>,
    pub raw_tangent_blocks: Vec<RawTangentBlock>,
    pub equation_residual_max: f64,
    pub equation_residual_l2: f64,
}

/// Immutable accepted-state reduced hard linearization for one component.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedHardComponentLinearization {
    revisions: SessionRevisions,
    component_index: usize,
    pattern_signature: u64,
    tangent_blocks: Vec<ReducedTangentBlock>,
    hard_rows: Vec<ReducedHardRow>,
    normalized_residual: DVector<f64>,
    normalized_jacobian: DMatrix<f64>,
    rank: usize,
    left_nullity: usize,
    right_nullity: usize,
    rank_threshold: f64,
    singular_values: Vec<f64>,
    normalized_residual_tolerance: f64,
}

impl AcceptedHardComponentLinearization {
    #[must_use]
    pub const fn revisions(&self) -> SessionRevisions {
        self.revisions
    }

    #[must_use]
    pub const fn component_index(&self) -> usize {
        self.component_index
    }

    #[must_use]
    pub const fn pattern_signature(&self) -> u64 {
        self.pattern_signature
    }

    #[must_use]
    pub fn tangent_blocks(&self) -> &[ReducedTangentBlock] {
        &self.tangent_blocks
    }

    #[must_use]
    pub fn hard_rows(&self) -> &[ReducedHardRow] {
        &self.hard_rows
    }

    #[must_use]
    pub const fn normalized_residual(&self) -> &DVector<f64> {
        &self.normalized_residual
    }

    #[must_use]
    pub const fn normalized_jacobian(&self) -> &DMatrix<f64> {
        &self.normalized_jacobian
    }

    #[must_use]
    pub const fn rank(&self) -> usize {
        self.rank
    }

    #[must_use]
    pub const fn left_nullity(&self) -> usize {
        self.left_nullity
    }

    #[must_use]
    pub const fn right_nullity(&self) -> usize {
        self.right_nullity
    }

    #[must_use]
    pub const fn rank_threshold(&self) -> f64 {
        self.rank_threshold
    }

    #[must_use]
    pub fn singular_values(&self) -> &[f64] {
        &self.singular_values
    }

    /// Solves `J * delta + normalized_residual_rate = 0` with the accepted
    /// component rank threshold. Rank-deficient consistent systems return the
    /// SVD minimum-norm tangent; inconsistent least squares is never success-like.
    ///
    /// # Errors
    ///
    /// Rejects a wrong-sized or non-finite rate, any non-finite SVD output, or
    /// normalized-to-raw scaling that cannot round-trip without material loss.
    pub fn solve_sensitivity(
        &self,
        normalized_residual_rate: &DVector<f64>,
    ) -> Result<SensitivitySolution, SensitivityError> {
        if normalized_residual_rate.len() != self.normalized_jacobian.nrows() {
            return Err(SensitivityError::DimensionMismatch {
                expected: self.normalized_jacobian.nrows(),
                actual: normalized_residual_rate.len(),
            });
        }
        for (index, &value) in normalized_residual_rate.iter().enumerate() {
            if !value.is_finite() {
                return Err(SensitivityError::NonFiniteRightHandSide { index, value });
            }
        }

        let rows = self.normalized_jacobian.nrows();
        let columns = self.normalized_jacobian.ncols();
        let normalized_tangent = if rows == 0 || columns == 0 {
            DVector::zeros(columns)
        } else {
            let decomposition = self.normalized_jacobian.clone().svd(true, true);
            if decomposition
                .singular_values
                .iter()
                .any(|value| !value.is_finite())
                || decomposition
                    .singular_values
                    .iter()
                    .filter(|&&value| value > self.rank_threshold)
                    .count()
                    != self.rank
            {
                return Err(SensitivityError::NumericalFailure {
                    context: "SVD rank does not match the accepted component rank",
                });
            }
            decomposition
                .solve(&(-normalized_residual_rate), self.rank_threshold)
                .map_err(|_| SensitivityError::NumericalFailure {
                    context: "SVD minimum-norm solve",
                })?
        };
        if normalized_tangent.len() != columns
            || normalized_tangent.iter().any(|value| !value.is_finite())
        {
            return Err(SensitivityError::NumericalFailure {
                context: "non-finite normalized tangent",
            });
        }

        let mathematical_residual =
            &self.normalized_jacobian * &normalized_tangent + normalized_residual_rate;
        let (mathematical_residual_max, _) = finite_norms(mathematical_residual.iter().copied())
            .ok_or(SensitivityError::NumericalFailure {
                context: "non-finite mathematical differentiated-equation residual",
            })?;
        let mathematically_consistent =
            mathematical_residual_max <= self.normalized_residual_tolerance;

        let raw_tangent_blocks = self.raw_tangent_blocks(&normalized_tangent)?;
        let recovered_tangent = self.recover_normalized_tangent(&raw_tangent_blocks)?;
        let equation_residual =
            &self.normalized_jacobian * recovered_tangent + normalized_residual_rate;
        let (equation_residual_max, equation_residual_l2) = finite_norms(
            equation_residual.iter().copied(),
        )
        .ok_or(SensitivityError::NumericalFailure {
            context: "non-finite recoverable differentiated-equation residual",
        })?;
        if mathematically_consistent && equation_residual_max > self.normalized_residual_tolerance {
            return Err(SensitivityError::NumericalFailure {
                context: "recoverable raw tangent violates differentiated equations",
            });
        }
        let status = if !mathematically_consistent {
            SensitivityStatus::Inconsistent
        } else if self.rank == columns {
            SensitivityStatus::Unique
        } else {
            SensitivityStatus::UnderdeterminedMinimumNorm
        };

        debug_assert!(
            !status.is_success_like()
                || equation_residual_max <= self.normalized_residual_tolerance
        );
        Ok(SensitivitySolution {
            revisions: self.revisions,
            status,
            normalized_tangent,
            raw_tangent_blocks,
            equation_residual_max,
            equation_residual_l2,
        })
    }

    fn recover_normalized_tangent(
        &self,
        raw_tangent_blocks: &[RawTangentBlock],
    ) -> Result<DVector<f64>, SensitivityError> {
        if raw_tangent_blocks.len() != self.tangent_blocks.len() {
            return Err(SensitivityError::NumericalFailure {
                context: "raw tangent block count changed during recovery",
            });
        }
        let mut recovered = DVector::zeros(self.normalized_jacobian.ncols());
        for (mapping, raw) in self.tangent_blocks.iter().zip(raw_tangent_blocks) {
            if raw.root != mapping.root
                || raw.alias_members != mapping.alias_members
                || raw.kind != mapping.kind
                || raw.values.len() != mapping.step_scales.len()
            {
                return Err(SensitivityError::NumericalFailure {
                    context: "raw tangent block mapping changed during recovery",
                });
            }
            for (coordinate, (&raw_value, &scale)) in
                raw.values.iter().zip(&mapping.step_scales).enumerate()
            {
                let value = raw_value / scale;
                if !value.is_finite() {
                    return Err(SensitivityError::NumericalFailure {
                        context: "raw tangent recovery produced a non-finite coordinate",
                    });
                }
                recovered[mapping.tangent_range.start + coordinate] = value;
            }
        }
        if recovered.iter().any(|value| !value.is_finite()) {
            return Err(SensitivityError::NumericalFailure {
                context: "raw tangent recovery produced a non-finite vector",
            });
        }
        Ok(recovered)
    }

    fn raw_tangent_blocks(
        &self,
        normalized_tangent: &DVector<f64>,
    ) -> Result<Vec<RawTangentBlock>, SensitivityError> {
        self.tangent_blocks
            .iter()
            .map(|block| {
                let mut values = DVector::zeros(block.tangent_range.len());
                for (coordinate, &scale) in block.step_scales.iter().enumerate() {
                    let normalized = normalized_tangent[block.tangent_range.start + coordinate];
                    let raw = normalized * scale;
                    let recovered = raw / scale;
                    let round_trip_error = (recovered - normalized).abs();
                    let round_trip_tolerance = 64.0 * f64::EPSILON * normalized.abs();
                    if !raw.is_finite()
                        || !recovered.is_finite()
                        || round_trip_error > round_trip_tolerance
                    {
                        return Err(SensitivityError::NumericalFailure {
                            context: "raw tangent scaling loses material precision",
                        });
                    }
                    values[coordinate] = raw;
                }
                Ok(RawTangentBlock {
                    root: block.root,
                    alias_members: block.alias_members.clone(),
                    kind: block.kind,
                    values,
                })
            })
            .collect()
    }
}

/// Revision-stamped immutable snapshot of one session's accepted hard system.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedHardLinearization {
    revisions: SessionRevisions,
    accepted_state: PackedState,
    components: Vec<AcceptedHardComponentLinearization>,
}

impl AcceptedHardLinearization {
    #[must_use]
    pub const fn revisions(&self) -> SessionRevisions {
        self.revisions
    }

    #[must_use]
    pub const fn accepted_state(&self) -> &PackedState {
        &self.accepted_state
    }

    #[must_use]
    pub fn components(&self) -> &[AcceptedHardComponentLinearization] {
        &self.components
    }

    #[must_use]
    pub fn component(&self, component_index: usize) -> Option<&AcceptedHardComponentLinearization> {
        self.components
            .iter()
            .find(|component| component.component_index == component_index)
    }
}

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

#[allow(clippy::too_many_lines)]
pub(crate) fn build_accepted_hard_linearization(
    problem: &Problem,
    plan: &EliminationPlan,
    report: &SolveReport,
    revisions: SessionRevisions,
    config: SolverConfig,
) -> Result<AcceptedHardLinearization, CoreError> {
    config.validate()?;
    if report.hard_validity != HardValidity::Valid
        || !report.hard_residuals_validated
        || !report.rank_is_valid
        || !report.hard_residual_max.is_finite()
        || report.hard_residual_max > config.normalized_residual_tolerance
    {
        return invalid_accepted("retained report is not finite, hard-valid, and rank-valid");
    }
    let accepted_state = problem.packed_state()?;
    if accepted_state != report.accepted_state {
        return invalid_accepted("retained problem state does not match the accepted report state");
    }
    if report.structural != plan.structural {
        return invalid_accepted("retained elimination plan does not match the accepted report");
    }
    if plan.components.len() != plan.component_layouts.len()
        || plan.components.len() != plan.structural.component_summaries.len()
        || plan.components.len() != report.component_solves.len()
    {
        return invalid_accepted("component counts do not match retained session data");
    }

    let state = problem.variable_state();
    let mut components = Vec::with_capacity(plan.components.len());
    let mut aggregate_rank = 0usize;
    let mut aggregate_left_nullity = 0usize;
    let mut aggregate_right_nullity = 0usize;
    let mut aggregate_machine_tolerance = 0.0_f64;
    let mut aggregate_rank_threshold = 0.0_f64;
    let mut aggregate_is_singular = false;
    let mut aggregate_near_singular = false;
    let mut aggregate_singular_values = Vec::new();
    let mut fresh_hard_max = 0.0_f64;
    let mut fresh_hard_l2 = 0.0_f64;
    for component in &plan.components {
        let summary = &plan.structural.component_summaries[component.index];
        let component_report = &report.component_solves[component.index];
        if summary.component_index != component.index
            || component_report.component_index != component.index
            || component_report.pattern_signature != summary.pattern_signature
            || component_report.hard_validity != HardValidity::Valid
            || !component_report.hard_residuals_validated
            || !component_report.rank_is_valid
            || !component_report.hard_residual_max.is_finite()
            || !component_report.hard_residual_l2.is_finite()
            || component_report.hard_residual_max > config.normalized_residual_tolerance
        {
            return invalid_accepted("component report identity or accepted validity is invalid");
        }

        // Include eliminated hard blocks in fresh canonical evaluation, then
        // project only active rows into the public reduced matrix.
        let linearization =
            problem.linearize_component(plan, component, &state, &component.residual_ids)?;
        if linearization.numeric.blocks.len() != component.residual_ids.len()
            || linearization
                .numeric
                .blocks
                .iter()
                .zip(&component.residual_ids)
                .any(|(block, residual_id)| {
                    block.residual_id != *residual_id
                        || block.category != ResidualCategory::Hard
                        || plan.source_is_suppressed(block.source_id)
                })
        {
            return invalid_accepted("canonical component hard-block identity is invalid");
        }
        let (component_hard_max, component_hard_l2) = finite_norms(
            linearization
                .numeric
                .blocks
                .iter()
                .flat_map(|block| block.normalized_residuals.iter().copied()),
        )
        .ok_or(CoreError::InvalidAcceptedLinearization {
            context: "canonical component hard residual is non-finite",
        })?;
        if component_hard_max > config.normalized_residual_tolerance {
            return invalid_accepted("fresh accepted hard residual exceeds configured tolerance");
        }
        fresh_hard_max = fresh_hard_max.max(component_hard_max);
        fresh_hard_l2 = fresh_hard_l2.hypot(component_hard_l2);
        if !fresh_hard_l2.is_finite() {
            return invalid_accepted("aggregate fresh accepted hard residual is non-finite");
        }

        let tangent_blocks = reduced_tangent_blocks(problem, plan, &linearization.layout)?;
        let dense = linearization.project_dense(plan, ResidualCategory::Hard)?;
        if dense.residuals.len() != dense.jacobian.nrows()
            || dense.rows.len() != dense.jacobian.nrows()
            || dense.jacobian.nrows() != summary.active_hard_rows
            || dense.jacobian.ncols() != summary.active_tangent_dimensions
            || tangent_blocks
                .last()
                .map_or(0, |block| block.tangent_range.end)
                != dense.jacobian.ncols()
        {
            return invalid_accepted("reduced component row or column dimensions are invalid");
        }
        let hard_rows = reduced_hard_rows(problem, plan, component, &dense.rows)?;
        let fresh_rank = rank_diagnostics(&dense.jacobian, config.rank_relative_tolerance).ok_or(
            CoreError::InvalidAcceptedLinearization {
                context: "fresh component rank policy evaluation failed",
            },
        )?;
        validate_accepted_rank(component_report, &dense.jacobian, &fresh_rank, config)?;

        aggregate_rank = aggregate_rank.saturating_add(component_report.rank);
        aggregate_left_nullity =
            aggregate_left_nullity.saturating_add(component_report.left_nullity);
        aggregate_right_nullity =
            aggregate_right_nullity.saturating_add(component_report.right_nullity);
        aggregate_machine_tolerance = aggregate_machine_tolerance.max(fresh_rank.machine_tolerance);
        aggregate_rank_threshold = aggregate_rank_threshold.max(fresh_rank.threshold);
        aggregate_is_singular |=
            fresh_rank.rank < dense.jacobian.nrows().min(dense.jacobian.ncols());
        aggregate_near_singular |= fresh_rank.near_singular;
        aggregate_singular_values.extend(fresh_rank.singular_values.iter().copied());
        components.push(AcceptedHardComponentLinearization {
            revisions,
            component_index: component.index,
            pattern_signature: summary.pattern_signature,
            tangent_blocks,
            hard_rows,
            normalized_residual: dense.residuals,
            normalized_jacobian: dense.jacobian,
            rank: component_report.rank,
            left_nullity: component_report.left_nullity,
            right_nullity: component_report.right_nullity,
            rank_threshold: component_report.rank_threshold,
            singular_values: component_report.singular_values.clone(),
            normalized_residual_tolerance: config.normalized_residual_tolerance,
        });
    }
    if !fresh_hard_max.is_finite()
        || fresh_hard_max > config.normalized_residual_tolerance
        || aggregate_rank != report.rank
        || aggregate_left_nullity != report.left_nullity
        || aggregate_right_nullity != report.right_nullity
        || report.local_degrees_of_freedom != aggregate_right_nullity
        || report.is_singular != aggregate_is_singular
        || report.near_singular != aggregate_near_singular
        || !same_finite_value(
            report.rank_relative_tolerance,
            config.rank_relative_tolerance,
        )
        || !same_finite_value(report.rank_machine_tolerance, aggregate_machine_tolerance)
        || !same_finite_value(report.rank_threshold, aggregate_rank_threshold)
        || report.singular_values.len() != aggregate_singular_values.len()
        || report
            .singular_values
            .iter()
            .zip(&aggregate_singular_values)
            .any(|(&accepted, &fresh)| !same_finite_value(accepted, fresh))
    {
        return invalid_accepted("aggregate component numerics do not match the accepted report");
    }

    Ok(AcceptedHardLinearization {
        revisions,
        accepted_state,
        components,
    })
}

fn reduced_tangent_blocks(
    problem: &Problem,
    plan: &EliminationPlan,
    layout: &ComponentTangentLayout,
) -> Result<Vec<ReducedTangentBlock>, CoreError> {
    let mut expected_start = 0usize;
    layout
        .blocks
        .iter()
        .map(|block| {
            let root = problem
                .variable(block.root)
                .ok_or(CoreError::UnknownVariable(block.root))?;
            let kind = root.kind();
            if block.tangent_range.start != expected_start
                || block.tangent_range.len() != kind.tangent_dimension()
                || block.step_scales != root.step_scales()
                || block
                    .members
                    .iter()
                    .filter(|&&member| member == block.root)
                    .count()
                    != 1
            {
                return invalid_accepted("reduced tangent block layout is invalid");
            }
            for &member in &block.members {
                let variable = problem
                    .variable(member)
                    .ok_or(CoreError::UnknownVariable(member))?;
                if plan.root(member) != Some(block.root)
                    || variable.kind() != kind
                    || variable.step_scales() != block.step_scales
                {
                    return invalid_accepted("reduced alias tangent mapping is invalid");
                }
            }
            expected_start = block.tangent_range.end;
            Ok(ReducedTangentBlock {
                root: block.root,
                alias_members: block
                    .members
                    .iter()
                    .copied()
                    .filter(|&member| member != block.root)
                    .collect(),
                kind,
                tangent_range: block.tangent_range.clone(),
                step_scales: block.step_scales.clone(),
            })
        })
        .collect()
}

fn reduced_hard_rows(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    rows: &[RowIdentity],
) -> Result<Vec<ReducedHardRow>, CoreError> {
    rows.iter()
        .map(|row| {
            let residual = problem
                .residual(row.residual_id)
                .ok_or(CoreError::UnknownResidual(row.residual_id))?;
            if residual.category() != ResidualCategory::Hard
                || !component.active_residual_ids.contains(&row.residual_id)
                || plan.is_eliminated(row.residual_id)
                || plan.source_is_suppressed(row.source_id)
                || residual.source() != row.source_id
                || row.row_in_block >= residual.output_dimension()
            {
                return invalid_accepted("public reduced hard-row mapping is invalid");
            }
            let residual_scale = residual.scales()[row.row_in_block];
            if !residual_scale.is_finite() || residual_scale <= 0.0 {
                return invalid_accepted("public reduced hard-row scale is invalid");
            }
            Ok(ReducedHardRow {
                row: ResidualRowRef {
                    residual_id: row.residual_id,
                    row_in_block: row.row_in_block,
                    source_id: row.source_id,
                },
                residual_scale,
            })
        })
        .collect()
}

fn validate_accepted_rank(
    report: &ComponentSolveReport,
    jacobian: &DMatrix<f64>,
    fresh: &RankDiagnostics,
    config: SolverConfig,
) -> Result<(), CoreError> {
    let is_singular = fresh.rank < jacobian.nrows().min(jacobian.ncols());
    if !fresh.relative_threshold.is_finite()
        || !same_finite_value(fresh.relative_tolerance, config.rank_relative_tolerance)
        || !same_finite_value(report.rank_relative_tolerance, fresh.relative_tolerance)
        || !same_finite_value(report.rank_machine_tolerance, fresh.machine_tolerance)
        || !same_finite_value(report.rank_threshold, fresh.threshold)
        || !same_finite_value(report.sigma_max, fresh.sigma_max)
        || !same_optional_finite_value(
            report.smallest_retained_singular_value,
            fresh.smallest_retained,
        )
        || !same_finite_value(report.near_singular_factor, fresh.near_singular_factor)
        || !same_optional_finite_value(report.near_singular_ratio, fresh.near_singular_ratio)
        || report.near_singular != fresh.near_singular
        || report.is_singular != is_singular
        || report.singular_values.len() != fresh.singular_values.len()
        || report
            .singular_values
            .iter()
            .zip(&fresh.singular_values)
            .any(|(&accepted, &fresh)| !same_finite_value(accepted, fresh))
        || report.rank != fresh.rank
        || report.rank > jacobian.nrows().min(jacobian.ncols())
        || report.left_nullity != jacobian.nrows() - report.rank
        || report.right_nullity != jacobian.ncols() - report.rank
        || report.local_degrees_of_freedom != report.right_nullity
    {
        return invalid_accepted("fresh component SVD does not match accepted rank data");
    }
    Ok(())
}

fn same_optional_finite_value(first: Option<f64>, second: Option<f64>) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => same_finite_value(first, second),
        (None, None) => true,
        _ => false,
    }
}

fn same_finite_value(first: f64, second: f64) -> bool {
    first.is_finite()
        && second.is_finite()
        && (first - second).abs() <= 1024.0 * f64::EPSILON * first.abs().max(second.abs()).max(1.0)
}

fn finite_norms(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    let mut maximum = 0.0_f64;
    let mut l2 = 0.0_f64;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        maximum = maximum.max(value.abs());
        l2 = l2.hypot(value);
        if !l2.is_finite() {
            return None;
        }
    }
    Some((maximum, l2))
}

fn invalid_accepted<T>(context: &'static str) -> Result<T, CoreError> {
    Err(CoreError::InvalidAcceptedLinearization { context })
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
