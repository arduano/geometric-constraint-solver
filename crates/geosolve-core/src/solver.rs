use nalgebra::{DMatrix, DVector};

use crate::problem::VariableState;
use crate::{
    AuditSnapshot, CoreError, DenseAssembly, PackedLayout, PackedState, Problem, ResidualCategory,
    ResidualId, SourceConstraintId,
};

/// Why nonlinear iteration stopped. Constraint-system diagnostics are kept
/// separately because a converged solution may still be underconstrained,
/// redundant, or singular.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveTermination {
    Converged,
    Stalled,
    IterationLimit,
    InvalidGeometry,
    NumericalFailure,
}

/// One deterministic attempted LM step.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveTraceRecord {
    pub iteration: usize,
    pub accepted: bool,
    pub trial_valid: bool,
    pub cost_before: f64,
    pub trial_cost: f64,
    /// Cost of the accepted state after this record.
    pub cost: f64,
    pub damping: f64,
    pub actual_reduction: f64,
    pub predicted_reduction: f64,
    pub reduction_ratio: f64,
    pub normalized_step_max: f64,
}

/// Attempted steps in deterministic execution order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolveTrace {
    pub records: Vec<SolveTraceRecord>,
}

/// Numerical and structural facts evaluated at the returned accepted state.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveReport {
    pub termination: SolveTermination,
    pub iterations: usize,
    pub accepted_state: PackedState,
    pub hard_residuals_validated: bool,
    pub hard_residual_max: f64,
    pub hard_residual_l2: f64,
    pub rank_is_valid: bool,
    pub rank: usize,
    pub local_degrees_of_freedom: usize,
    pub is_singular: bool,
    pub rank_relative_tolerance: f64,
    pub rank_threshold: f64,
    pub singular_values: Vec<f64>,
    pub conflicting_sources: Vec<SourceConstraintId>,
    pub redundant_sources: Vec<SourceConstraintId>,
    pub trace: SolveTrace,
    pub audit: AuditSnapshot,
}

/// Centralized normalized tolerances, damping policy, and iteration limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolverConfig {
    pub normalized_residual_tolerance: f64,
    pub normalized_step_tolerance: f64,
    pub rank_relative_tolerance: f64,
    pub max_iterations: usize,
    pub initial_damping: f64,
    pub minimum_damping: f64,
    pub maximum_damping: f64,
    pub damping_increase_factor: f64,
    pub damping_decrease_factor: f64,
    pub step_acceptance_ratio: f64,
    pub max_block_normalized_step: f64,
}

impl SolverConfig {
    /// Validates every numerical policy value before solving mutates a problem.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSolverConfig`] for an invalid field.
    pub fn validate(&self) -> Result<(), CoreError> {
        positive_finite(
            self.normalized_residual_tolerance,
            "normalized_residual_tolerance",
        )?;
        positive_finite(self.normalized_step_tolerance, "normalized_step_tolerance")?;
        positive_finite(self.rank_relative_tolerance, "rank_relative_tolerance")?;
        if self.rank_relative_tolerance > 1.0 {
            return invalid_config(
                "rank_relative_tolerance",
                "must be less than or equal to one",
            );
        }
        positive_finite(self.initial_damping, "initial_damping")?;
        positive_finite(self.minimum_damping, "minimum_damping")?;
        positive_finite(self.maximum_damping, "maximum_damping")?;
        if self.minimum_damping > self.initial_damping {
            return invalid_config("minimum_damping", "must not exceed initial_damping");
        }
        if self.initial_damping > self.maximum_damping {
            return invalid_config("maximum_damping", "must not be less than initial_damping");
        }
        if !self.damping_increase_factor.is_finite() || self.damping_increase_factor <= 1.0 {
            return invalid_config(
                "damping_increase_factor",
                "must be finite and greater than one",
            );
        }
        if !self.damping_decrease_factor.is_finite()
            || self.damping_decrease_factor <= 0.0
            || self.damping_decrease_factor >= 1.0
        {
            return invalid_config(
                "damping_decrease_factor",
                "must be finite and strictly between zero and one",
            );
        }
        if !self.step_acceptance_ratio.is_finite()
            || self.step_acceptance_ratio < 0.0
            || self.step_acceptance_ratio >= 1.0
        {
            return invalid_config(
                "step_acceptance_ratio",
                "must be finite, non-negative, and less than one",
            );
        }
        positive_finite(self.max_block_normalized_step, "max_block_normalized_step")
    }
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            normalized_residual_tolerance: 1.0e-9,
            normalized_step_tolerance: 1.0e-10,
            rank_relative_tolerance: 1.0e-10,
            max_iterations: 80,
            initial_damping: 1.0e-3,
            minimum_damping: 1.0e-15,
            maximum_damping: 1.0e15,
            damping_increase_factor: 10.0,
            damping_decrease_factor: 0.25,
            step_acceptance_ratio: 1.0e-4,
            max_block_normalized_step: 1.0,
        }
    }
}

impl Problem {
    /// Solves hard residual rows and commits only accepted finite states.
    ///
    /// Temporary and preference rows are validated and included in the audit,
    /// but are intentionally excluded from the M2 objective. This avoids an
    /// undocumented weighted substitute for a future hierarchical/null-space
    /// policy. `Converged` is possible only after a fresh hard-row evaluation.
    ///
    /// # Errors
    ///
    /// Returns an error only for invalid solver configuration or invalid static
    /// problem storage. Evaluator and numerical failures are represented by
    /// [`SolveTermination`] while preserving the last accepted finite state.
    #[allow(clippy::too_many_lines)]
    pub fn solve(&mut self, config: SolverConfig) -> Result<SolveReport, CoreError> {
        config.validate()?;
        let layout = self.packed_layout()?;
        let mut state = self.variable_state();
        let mut trace = SolveTrace::default();
        let mut damping = config.initial_damping;

        let full_assembly = match self.assemble_dense_for_state(&state) {
            Ok(assembly) => assembly,
            Err(error) => {
                return self.finish_solve(error_termination(&error), config, trace);
            }
        };
        let mut current_hard = extract_hard_system(self, &full_assembly)?;
        let Some(mut cost) = residual_cost(&current_hard.residuals) else {
            return self.finish_solve(SolveTermination::NumericalFailure, config, trace);
        };
        let mut termination =
            if residual_max(&current_hard.residuals) <= config.normalized_residual_tolerance {
                SolveTermination::Converged
            } else {
                SolveTermination::IterationLimit
            };

        if termination != SolveTermination::Converged {
            for iteration in 1..=config.max_iterations {
                let Some(mut step) =
                    lm_step(&current_hard.jacobian, &current_hard.residuals, damping)
                else {
                    termination = SolveTermination::NumericalFailure;
                    break;
                };
                let Some(normalized_step_max) =
                    limit_block_steps(&mut step, &layout, config.max_block_normalized_step)
                else {
                    termination = SolveTermination::NumericalFailure;
                    break;
                };
                let Some(predicted_reduction) = predicted_reduction(&current_hard, &step, cost)
                else {
                    termination = SolveTermination::NumericalFailure;
                    break;
                };

                if normalized_step_max <= config.normalized_step_tolerance {
                    trace.records.push(rejected_record(
                        iteration,
                        cost,
                        damping,
                        predicted_reduction,
                        normalized_step_max,
                        false,
                    ));
                    termination = SolveTermination::Stalled;
                    break;
                }

                let mut trial_state = state.clone();
                if apply_normalized_step(&mut trial_state, &layout, &step).is_err() {
                    trace.records.push(rejected_record(
                        iteration,
                        cost,
                        damping,
                        predicted_reduction,
                        normalized_step_max,
                        false,
                    ));
                    if !increase_damping(&mut damping, &config) {
                        termination = SolveTermination::Stalled;
                        break;
                    }
                    continue;
                }

                let trial_assembly = match self.assemble_dense_for_state(&trial_state) {
                    Ok(assembly) => assembly,
                    Err(error) if recoverable_trial_error(&error) => {
                        trace.records.push(rejected_record(
                            iteration,
                            cost,
                            damping,
                            predicted_reduction,
                            normalized_step_max,
                            false,
                        ));
                        if !increase_damping(&mut damping, &config) {
                            termination = SolveTermination::Stalled;
                            break;
                        }
                        continue;
                    }
                    Err(_) => {
                        termination = SolveTermination::NumericalFailure;
                        break;
                    }
                };
                let trial_hard_system = extract_hard_system(self, &trial_assembly)?;
                let Some(trial_cost) = residual_cost(&trial_hard_system.residuals) else {
                    trace.records.push(rejected_record(
                        iteration,
                        cost,
                        damping,
                        predicted_reduction,
                        normalized_step_max,
                        false,
                    ));
                    if !increase_damping(&mut damping, &config) {
                        termination = SolveTermination::Stalled;
                        break;
                    }
                    continue;
                };
                let actual_reduction = cost - trial_cost;
                let reduction_ratio = if predicted_reduction > 0.0 {
                    actual_reduction / predicted_reduction
                } else {
                    0.0
                };
                if !actual_reduction.is_finite() || !reduction_ratio.is_finite() {
                    termination = SolveTermination::NumericalFailure;
                    break;
                }
                let accepted = actual_reduction > 0.0
                    && predicted_reduction > 0.0
                    && reduction_ratio >= config.step_acceptance_ratio;
                let accepted_cost = if accepted { trial_cost } else { cost };
                trace.records.push(SolveTraceRecord {
                    iteration,
                    accepted,
                    trial_valid: true,
                    cost_before: cost,
                    trial_cost,
                    cost: accepted_cost,
                    damping,
                    actual_reduction,
                    predicted_reduction,
                    reduction_ratio,
                    normalized_step_max,
                });

                if accepted {
                    self.replace_variable_state(&trial_state)?;
                    state = trial_state;
                    current_hard = trial_hard_system;
                    cost = trial_cost;
                    update_accepted_damping(&mut damping, reduction_ratio, &config);
                    let independently_evaluated =
                        match self.normalized_category_values(&state, ResidualCategory::Hard) {
                            Ok(values) => values,
                            Err(error) => {
                                termination = error_termination(&error);
                                break;
                            }
                        };
                    if independently_evaluated
                        .iter()
                        .map(|item| item.3.abs())
                        .fold(0.0, f64::max)
                        <= config.normalized_residual_tolerance
                    {
                        termination = SolveTermination::Converged;
                        break;
                    }
                } else if !increase_damping(&mut damping, &config) {
                    termination = SolveTermination::Stalled;
                    break;
                }
            }
        }

        self.finish_solve(termination, config, trace)
    }

    fn finish_solve(
        &self,
        mut termination: SolveTermination,
        config: SolverConfig,
        trace: SolveTrace,
    ) -> Result<SolveReport, CoreError> {
        let state = self.variable_state();
        let accepted_state = self.packed_state()?;
        let tangent_dimension = accepted_state.layout().tangent_dimension();
        let mut hard_residuals_validated = false;
        let mut hard_residual_max = 0.0;
        let mut hard_residual_l2 = 0.0;
        let mut validated_rows = Vec::new();

        match self.normalized_category_values(&state, ResidualCategory::Hard) {
            Ok(values) => {
                if let Some((maximum, l2)) = residual_norms(values.iter().map(|item| item.3)) {
                    hard_residuals_validated = true;
                    hard_residual_max = maximum;
                    hard_residual_l2 = l2;
                    validated_rows = values;
                } else {
                    termination = SolveTermination::NumericalFailure;
                }
            }
            Err(error) => termination = error_termination(&error),
        }

        let mut rank_is_valid = false;
        let mut rank = 0;
        let mut local_degrees_of_freedom = tangent_dimension;
        let mut is_singular = false;
        let mut rank_threshold = 0.0;
        let mut singular_values = Vec::new();
        let mut redundant_sources = Vec::new();

        match self.assemble_dense_for_state(&state) {
            Ok(assembly) => {
                let hard = extract_hard_system(self, &assembly)?;
                if let Some(diagnostics) =
                    rank_diagnostics(&hard.jacobian, config.rank_relative_tolerance)
                {
                    rank_is_valid = true;
                    rank = diagnostics.rank;
                    local_degrees_of_freedom = tangent_dimension.saturating_sub(rank);
                    is_singular = rank < hard.jacobian.nrows().min(hard.jacobian.ncols());
                    rank_threshold = diagnostics.threshold;
                    singular_values = diagnostics.singular_values;
                    if hard_residuals_validated {
                        redundant_sources = find_redundant_sources(
                            &hard,
                            &validated_rows,
                            rank_threshold,
                            config.normalized_residual_tolerance,
                        );
                    }
                } else {
                    termination = SolveTermination::NumericalFailure;
                }
            }
            Err(error) => termination = error_termination(&error),
        }

        let audit = match self.audit_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                termination = error_termination(&error);
                AuditSnapshot::default()
            }
        };

        if hard_residuals_validated
            && hard_residual_max <= config.normalized_residual_tolerance
            && !matches!(
                termination,
                SolveTermination::InvalidGeometry | SolveTermination::NumericalFailure
            )
        {
            termination = SolveTermination::Converged;
        } else if termination == SolveTermination::Converged {
            termination = SolveTermination::NumericalFailure;
        }

        Ok(SolveReport {
            termination,
            iterations: trace.records.len(),
            accepted_state,
            hard_residuals_validated,
            hard_residual_max,
            hard_residual_l2,
            rank_is_valid,
            rank,
            local_degrees_of_freedom,
            is_singular,
            rank_relative_tolerance: config.rank_relative_tolerance,
            rank_threshold,
            singular_values,
            conflicting_sources: Vec::new(),
            redundant_sources,
            trace,
            audit,
        })
    }
}

#[derive(Debug)]
struct HardSystem {
    residuals: DVector<f64>,
    jacobian: DMatrix<f64>,
    rows: Vec<(ResidualId, usize, SourceConstraintId)>,
}

fn extract_hard_system(
    problem: &Problem,
    assembly: &DenseAssembly,
) -> Result<HardSystem, CoreError> {
    let row_count = assembly
        .residual_layout()
        .iter()
        .filter_map(|layout| {
            problem
                .residual(layout.residual_id)
                .filter(|residual| residual.category() == ResidualCategory::Hard)
                .map(|_| layout.row_range.len())
        })
        .sum();
    let column_count = assembly.variable_layout().tangent_dimension();
    let mut residuals = DVector::zeros(row_count);
    let mut jacobian = DMatrix::zeros(row_count, column_count);
    let mut rows = Vec::with_capacity(row_count);
    let mut target_row = 0;

    for layout in assembly.residual_layout() {
        let residual = problem
            .residual(layout.residual_id)
            .ok_or(CoreError::UnknownResidual(layout.residual_id))?;
        if residual.category() != ResidualCategory::Hard {
            continue;
        }
        for (row_in_block, source_row) in layout.row_range.clone().enumerate() {
            residuals[target_row] = assembly.residuals()[source_row];
            jacobian
                .row_mut(target_row)
                .copy_from(&assembly.jacobian().row(source_row));
            rows.push((layout.residual_id, row_in_block, residual.source()));
            target_row += 1;
        }
    }
    Ok(HardSystem {
        residuals,
        jacobian,
        rows,
    })
}

fn lm_step(
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    damping: f64,
) -> Option<DVector<f64>> {
    let rows = jacobian.nrows();
    let columns = jacobian.ncols();
    if columns == 0 {
        return Some(DVector::zeros(0));
    }
    let mut augmented = DMatrix::zeros(rows + columns, columns);
    augmented
        .view_mut((0, 0), (rows, columns))
        .copy_from(jacobian);
    let sqrt_damping = damping.sqrt();
    if !sqrt_damping.is_finite() {
        return None;
    }
    for index in 0..columns {
        augmented[(rows + index, index)] = sqrt_damping;
    }
    let mut right_hand_side = DVector::zeros(rows + columns);
    right_hand_side.rows_mut(0, rows).copy_from(&(-residuals));

    let qr = augmented.clone().qr();
    let mut transformed = right_hand_side.clone();
    qr.q_tr_mul(&mut transformed);
    let triangular = qr.r();
    let transformed_top = transformed.rows(0, columns).into_owned();
    if let Some(step) = triangular.solve_upper_triangular(&transformed_top)
        && step.iter().all(|value| value.is_finite())
    {
        return Some(step);
    }

    let svd = augmented.svd(true, true);
    let largest = svd.singular_values.iter().copied().fold(0.0_f64, f64::max);
    let dimension = u32::try_from((rows + columns).max(columns)).ok()?;
    let epsilon = f64::EPSILON * f64::from(dimension) * largest;
    let step = svd.solve(&right_hand_side, epsilon).ok()?;
    step.iter().all(|value| value.is_finite()).then_some(step)
}

fn limit_block_steps(step: &mut DVector<f64>, layout: &PackedLayout, limit: f64) -> Option<f64> {
    let mut maximum = 0.0_f64;
    for block in layout.blocks() {
        let range = block.tangent_range.clone();
        let norm = stable_norm(step.rows(range.start, range.len()).iter().copied())?;
        if norm > limit {
            let factor = limit / norm;
            step.rows_mut(range.start, range.len()).scale_mut(factor);
        }
        maximum = maximum.max(norm.min(limit));
    }
    step.iter()
        .all(|value| value.is_finite())
        .then_some(maximum)
}

fn apply_normalized_step(
    state: &mut VariableState,
    layout: &PackedLayout,
    step: &DVector<f64>,
) -> Result<(), CoreError> {
    for ((id, value), block) in state.values.iter_mut().zip(layout.blocks()) {
        if *id != block.variable_id {
            return Err(CoreError::UnknownVariable(*id));
        }
        let mut raw_delta = Vec::with_capacity(block.tangent_range.len());
        for (column, &scale) in block.step_scales.iter().enumerate() {
            raw_delta.push(step[block.tangent_range.start + column] * scale);
        }
        value.plus(&raw_delta)?;
    }
    Ok(())
}

fn predicted_reduction(system: &HardSystem, step: &DVector<f64>, cost: f64) -> Option<f64> {
    let linearized = &system.residuals + &system.jacobian * step;
    let model_cost = residual_cost(&linearized)?;
    let reduction = cost - model_cost;
    reduction.is_finite().then_some(reduction)
}

fn residual_cost(residuals: &DVector<f64>) -> Option<f64> {
    let squared = residuals.dot(residuals);
    let cost = 0.5 * squared;
    cost.is_finite().then_some(cost)
}

fn residual_max(residuals: &DVector<f64>) -> f64 {
    residuals
        .iter()
        .map(|value| value.abs())
        .fold(0.0, f64::max)
}

fn residual_norms(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
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

fn stable_norm(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut norm = 0.0_f64;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        norm = norm.hypot(value);
        if !norm.is_finite() {
            return None;
        }
    }
    Some(norm)
}

#[derive(Debug)]
struct RankDiagnostics {
    rank: usize,
    threshold: f64,
    singular_values: Vec<f64>,
}

fn rank_diagnostics(jacobian: &DMatrix<f64>, relative_tolerance: f64) -> Option<RankDiagnostics> {
    if jacobian.nrows() == 0 || jacobian.ncols() == 0 {
        return Some(RankDiagnostics {
            rank: 0,
            threshold: 0.0,
            singular_values: Vec::new(),
        });
    }
    let singular_values: Vec<_> = jacobian
        .clone()
        .svd(false, false)
        .singular_values
        .iter()
        .copied()
        .collect();
    if singular_values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let largest = singular_values.iter().copied().fold(0.0, f64::max);
    let threshold = largest * relative_tolerance;
    if !threshold.is_finite() {
        return None;
    }
    let rank = singular_values
        .iter()
        .filter(|&&value| value > threshold)
        .count();
    Some(RankDiagnostics {
        rank,
        threshold,
        singular_values,
    })
}

fn find_redundant_sources(
    hard: &HardSystem,
    validated_rows: &[(ResidualId, usize, SourceConstraintId, f64)],
    threshold: f64,
    residual_tolerance: f64,
) -> Vec<SourceConstraintId> {
    let mut candidates = Vec::new();
    let mut groups: Vec<(SourceConstraintId, Vec<usize>)> = Vec::new();
    for (row_index, row) in hard.rows.iter().enumerate() {
        if let Some((_, indices)) = groups.iter_mut().find(|group| group.0 == row.2) {
            indices.push(row_index);
        } else {
            groups.push((row.2, vec![row_index]));
        }
    }

    let mut prior_rows = Vec::new();
    for (source, source_rows) in groups {
        let prior_rank = selected_row_rank(&hard.jacobian, &prior_rows, threshold);
        let all_rows_nonzero = !source_rows.is_empty()
            && source_rows.iter().all(|&row| {
                stable_norm(hard.jacobian.row(row).iter().copied())
                    .is_some_and(|norm| norm > threshold)
            });
        let all_rows_satisfied = source_rows.iter().all(|&row_index| {
            let row = hard.rows[row_index];
            validated_rows
                .iter()
                .find(|item| item.0 == row.0 && item.1 == row.1)
                .is_some_and(|item| item.3.abs() <= residual_tolerance)
        });
        let mut combined_rows = prior_rows.clone();
        combined_rows.extend_from_slice(&source_rows);
        let combined_rank = selected_row_rank(&hard.jacobian, &combined_rows, threshold);
        if prior_rank > 0 && all_rows_nonzero && all_rows_satisfied && combined_rank == prior_rank {
            candidates.push(source);
        }
        prior_rows = combined_rows;
    }
    candidates
}

fn selected_row_rank(matrix: &DMatrix<f64>, rows: &[usize], threshold: f64) -> usize {
    if rows.is_empty() || matrix.ncols() == 0 {
        return 0;
    }
    let selected = DMatrix::from_fn(rows.len(), matrix.ncols(), |row, column| {
        matrix[(rows[row], column)]
    });
    selected
        .svd(false, false)
        .singular_values
        .iter()
        .filter(|&&value| value > threshold)
        .count()
}

fn rejected_record(
    iteration: usize,
    cost: f64,
    damping: f64,
    predicted_reduction: f64,
    normalized_step_max: f64,
    trial_valid: bool,
) -> SolveTraceRecord {
    SolveTraceRecord {
        iteration,
        accepted: false,
        trial_valid,
        cost_before: cost,
        trial_cost: cost,
        cost,
        damping,
        actual_reduction: 0.0,
        predicted_reduction,
        reduction_ratio: 0.0,
        normalized_step_max,
    }
}

fn increase_damping(damping: &mut f64, config: &SolverConfig) -> bool {
    if *damping >= config.maximum_damping {
        return false;
    }
    *damping = (*damping * config.damping_increase_factor).min(config.maximum_damping);
    true
}

fn update_accepted_damping(damping: &mut f64, ratio: f64, config: &SolverConfig) {
    if ratio > 0.75 {
        *damping = (*damping * config.damping_decrease_factor).max(config.minimum_damping);
    } else if ratio < 0.25 {
        *damping = (*damping * config.damping_increase_factor).min(config.maximum_damping);
    }
}

fn recoverable_trial_error(error: &CoreError) -> bool {
    matches!(
        error,
        CoreError::InvalidGeometry { .. } | CoreError::NonFiniteValue { .. }
    )
}

fn error_termination(error: &CoreError) -> SolveTermination {
    if matches!(error, CoreError::InvalidGeometry { .. }) {
        SolveTermination::InvalidGeometry
    } else {
        SolveTermination::NumericalFailure
    }
}

fn positive_finite(value: f64, field: &'static str) -> Result<(), CoreError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        invalid_config(field, "must be positive and finite")
    }
}

fn invalid_config<T>(field: &'static str, message: &'static str) -> Result<T, CoreError> {
    Err(CoreError::InvalidSolverConfig { field, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_strict_but_finite() {
        let config = SolverConfig::default();
        config.validate().unwrap();
        assert!(config.normalized_residual_tolerance.is_finite());
        assert!(config.normalized_residual_tolerance > 0.0);
        assert!(config.max_iterations > 0);
    }
}
