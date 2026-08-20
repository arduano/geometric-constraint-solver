use super::{
    AUTO_SPARSE_MAX_DENSITY_DENOMINATOR, AUTO_SPARSE_MIN_COLUMNS, AUTO_SPARSE_MIN_NNZ,
    AUTO_SPARSE_MIN_ROWS, ActiveLayout, CachedComponent, ComponentIndexedSystem,
    ComponentSolveReport, CoreError, DMatrix, DVector, EliminationPlan, HardSystem, HardValidity,
    LinearSolveBackend, LinearSolveBackendPolicy, Matrix2, NEAR_SINGULAR_FACTOR,
    OperationCheckpoint, OperationController, OperationWorkCounter, PrioritySolveReport, Problem,
    ResidualCategory, ResidualId, SecondaryStatus, SolveComponent, SolveTermination, SolveTrace,
    SolveTraceRecord, SolverConfig, SparseFallbackReason, VariableId, VariableState, Vector2,
    active_layout, apply_normalized_step, at_bound_endpoint, bound_column,
    controlled_dense_factorization, controlled_factorization, controlled_rank_diagnostics,
    linearized_component_objective, numerical_nullspace, objective_decreases,
    restricted_structural_nnz, set_state_value, solve_damped_least_squares, validate_component,
};

#[derive(Clone, Copy)]
pub(super) enum DirtyRequest<'a> {
    All,
    Variables(&'a [VariableId]),
    Components(&'a [usize]),
}

#[derive(Clone, Debug)]
pub(super) struct ComponentExecution {
    pub(super) component_index: usize,
    pub(super) reused: bool,
    pub(super) termination: SolveTermination,
    pub(super) trace: SolveTrace,
}

#[derive(Debug)]
pub(super) struct IterationOutcome {
    pub(super) termination: SolveTermination,
    pub(super) state: VariableState,
    pub(super) trace: SolveTrace,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BackendEvidence {
    pub(super) requested: LinearSolveBackendPolicy,
    pub(super) actual: Option<LinearSolveBackend>,
    pub(super) symbolic_reuse_count: usize,
    pub(super) fallback_reason: Option<SparseFallbackReason>,
}

impl BackendEvidence {
    pub(super) const fn new(requested: LinearSolveBackendPolicy) -> Self {
        Self {
            requested,
            actual: None,
            symbolic_reuse_count: 0,
            fallback_reason: None,
        }
    }

    fn record_backend(&mut self, backend: LinearSolveBackend) {
        self.actual = merge_actual_backend(self.actual, Some(backend));
    }

    fn record_symbolic_reuse(&mut self, reused: bool) {
        self.symbolic_reuse_count += usize::from(reused);
    }

    fn record_fallback(&mut self, reason: SparseFallbackReason) {
        self.fallback_reason.get_or_insert(reason);
    }

    fn merge(&mut self, other: Self) {
        debug_assert_eq!(self.requested, other.requested);
        self.actual = merge_actual_backend(self.actual, other.actual);
        self.symbolic_reuse_count = self
            .symbolic_reuse_count
            .saturating_add(other.symbolic_reuse_count);
        if self.fallback_reason.is_none() {
            self.fallback_reason = other.fallback_reason;
        }
    }
}

pub(super) fn record_backend_evidence(
    problem: &Problem,
    component_index: usize,
    evidence: BackendEvidence,
) {
    let mut aggregate = problem.solve_backend_evidence.borrow_mut();
    // Conflict diagnostics may solve a temporary suppression plan after the
    // accepted-plan component reports have already captured their evidence.
    if let Some(component) = aggregate.get_mut(component_index) {
        component.merge(evidence);
    }
}

pub(super) const fn merge_actual_backend(
    first: Option<LinearSolveBackend>,
    second: Option<LinearSolveBackend>,
) -> Option<LinearSolveBackend> {
    match (first, second) {
        (None, backend) | (backend, None) => backend,
        (Some(LinearSolveBackend::Dense), Some(LinearSolveBackend::Dense)) => {
            Some(LinearSolveBackend::Dense)
        }
        (Some(LinearSolveBackend::SparseQr), Some(LinearSolveBackend::SparseQr)) => {
            Some(LinearSolveBackend::SparseQr)
        }
        _ => Some(LinearSolveBackend::Mixed),
    }
}

pub(super) fn cache_matches(
    cached: &CachedComponent,
    plan: &EliminationPlan,
    component: &SolveComponent,
) -> bool {
    let summary = &plan.structural.component_summaries[component.index];
    cached.pattern_signature == summary.pattern_signature
        && cached.variable_ids == component.variable_ids
        && cached.residual_ids == component.residual_ids
        && cached.values.len() == component.variable_ids.len()
}

pub(super) fn validated_cached_state(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    current_state: &VariableState,
    cached: &CachedComponent,
    config: SolverConfig,
) -> Option<VariableState> {
    let mut candidate = current_state.clone();
    for (&variable_id, &value) in component.variable_ids.iter().zip(&cached.values) {
        set_state_value(&mut candidate, variable_id, value).ok()?;
    }
    plan.synchronize_state(problem, &mut candidate).ok()?;
    enforce_state_bounds(problem, plan, &mut candidate).ok()?;
    validate_component(problem, component, &candidate, config)
        .valid
        .then_some(candidate)
}

pub(super) fn trusted_cached_state(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    current_state: &VariableState,
    cached: &CachedComponent,
) -> Option<VariableState> {
    let mut candidate = current_state.clone();
    for (&variable_id, &value) in component.variable_ids.iter().zip(&cached.values) {
        set_state_value(&mut candidate, variable_id, value).ok()?;
    }
    plan.synchronize_state(problem, &mut candidate).ok()?;
    enforce_state_bounds(problem, plan, &mut candidate).ok()?;
    Some(candidate)
}

#[allow(clippy::too_many_lines)]
pub(super) fn iterate_component(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: VariableState,
    config: SolverConfig,
    control: Option<&mut OperationController>,
) -> Option<IterationOutcome> {
    iterate_component_objective(
        problem,
        plan,
        component,
        state,
        config,
        ComponentIterationObjective::Hard,
        control,
    )
}

#[derive(Clone, Copy)]
pub(super) enum ComponentIterationObjective<'a> {
    Hard,
    HardAndPriority {
        category: ResidualCategory,
        residual_ids: &'a [ResidualId],
    },
    HardAndPriorityResidualTarget {
        category: ResidualCategory,
        residual_ids: &'a [ResidualId],
        target: &'a DVector<f64>,
    },
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn iterate_component_objective(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    mut state: VariableState,
    config: SolverConfig,
    objective: ComponentIterationObjective<'_>,
    control: Option<&mut OperationController>,
) -> Option<IterationOutcome> {
    let mut trace = SolveTrace::default();
    let mut backend = BackendEvidence::new(config.linear_solve_backend);
    let mut damping = config.initial_damping;
    let layout = active_layout(plan, component.index);
    if plan.synchronize_state(problem, &mut state).is_err()
        || enforce_state_bounds(problem, plan, &mut state).is_err()
    {
        return Some(IterationOutcome {
            termination: SolveTermination::NumericalFailure,
            state,
            trace,
        });
    }
    let mut current_hard =
        match linearized_component_objective(problem, plan, component, &state, objective) {
            Ok(system) => system,
            Err(error) => {
                return Some(IterationOutcome {
                    termination: error_termination(&error),
                    state,
                    trace,
                });
            }
        };
    let Some(mut cost) = residual_cost(&current_hard.residuals) else {
        return Some(IterationOutcome {
            termination: SolveTermination::NumericalFailure,
            state,
            trace,
        });
    };
    let mut termination =
        if residual_max(&current_hard.residuals) <= config.normalized_residual_tolerance {
            SolveTermination::Converged
        } else {
            SolveTermination::IterationLimit
        };

    let mut operation = control;
    if termination != SolveTermination::Converged {
        for iteration in 1..=config.max_iterations {
            let mut control = match operation.as_deref_mut() {
                Some(controller) => Some(
                    controller
                        .charged_boundary(
                            OperationWorkCounter::NonlinearIterations,
                            1,
                            OperationCheckpoint::BeforeNonlinearIteration,
                            OperationCheckpoint::AfterNonlinearIteration,
                        )
                        .ok()?,
                ),
                None => None,
            };
            let Some(mut step) = bounded_lm_step(
                problem,
                &state,
                &layout,
                &current_hard.jacobian,
                &current_hard.residuals,
                current_hard.indexed.as_ref(),
                damping,
                config.normalized_step_tolerance,
                config.rank_relative_tolerance,
                config.linear_solve_backend,
                &mut backend,
                control.as_deref_mut(),
            ) else {
                termination = SolveTermination::NumericalFailure;
                break;
            };
            if control
                .as_ref()
                .is_some_and(|controller| controller.is_stopped())
            {
                return None;
            }
            if limit_block_steps(&mut step, &layout, config.max_block_normalized_step).is_none()
                || limit_step_to_bound_events(problem, &state, &layout, &mut step).is_none()
            {
                termination = SolveTermination::NumericalFailure;
                break;
            }
            let Some(normalized_step_max) = maximum_block_step(&step, &layout) else {
                termination = SolveTermination::NumericalFailure;
                break;
            };
            let Some(predicted_reduction) = predicted_reduction(&current_hard, &step, cost) else {
                termination = SolveTermination::NumericalFailure;
                break;
            };
            let mut control = match control.as_deref_mut() {
                Some(controller) => Some(
                    controller
                        .boundary(
                            OperationCheckpoint::BeforeTrialBoundary,
                            OperationCheckpoint::AfterTrialBoundary,
                        )
                        .ok()?,
                ),
                None => None,
            };
            if normalized_step_max <= config.normalized_step_tolerance {
                if charge_rejected_trial(&mut control) {
                    return None;
                }
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
            if apply_normalized_step(problem, plan, &mut trial_state, &layout, &step).is_err() {
                if charge_rejected_trial(&mut control) {
                    return None;
                }
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
            let trial_hard = match linearized_component_objective(
                problem,
                plan,
                component,
                &trial_state,
                objective,
            ) {
                Ok(system) => system,
                Err(error) if recoverable_trial_error(&error) => {
                    if charge_rejected_trial(&mut control) {
                        return None;
                    }
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
            let Some(trial_cost) = residual_cost(&trial_hard.residuals) else {
                if charge_rejected_trial(&mut control) {
                    return None;
                }
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
            if !accepted && charge_rejected_trial(&mut control) {
                return None;
            }
            trace.records.push(SolveTraceRecord {
                component_index: None,
                iteration,
                accepted,
                trial_valid: true,
                cost_before: cost,
                trial_cost,
                cost: if accepted { trial_cost } else { cost },
                damping,
                actual_reduction,
                predicted_reduction,
                reduction_ratio,
                normalized_step_max,
            });
            if accepted {
                state = trial_state;
                current_hard = trial_hard;
                cost = trial_cost;
                update_accepted_damping(&mut damping, reduction_ratio, &config);
                if residual_max(&current_hard.residuals) <= config.normalized_residual_tolerance {
                    termination = SolveTermination::Converged;
                    break;
                }
            } else if !increase_damping(&mut damping, &config) {
                termination = SolveTermination::Stalled;
                break;
            }
        }
    }
    if operation
        .as_ref()
        .is_some_and(|controller| controller.is_stopped())
    {
        return None;
    }
    record_backend_evidence(problem, component.index, backend);
    Some(IterationOutcome {
        termination,
        state,
        trace,
    })
}

pub(super) fn charge_rejected_trial<C>(control: &mut Option<C>) -> bool
where
    C: std::ops::DerefMut<Target = OperationController>,
{
    control.as_deref_mut().is_some_and(|controller| {
        controller
            .charge(
                OperationWorkCounter::RejectedTrials,
                1,
                OperationCheckpoint::BeforeTrialBoundary,
            )
            .is_err()
    })
}

pub(super) fn lm_step(
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    damping: f64,
    control: Option<&mut OperationController>,
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
    solve_dense_least_squares(&augmented, &right_hand_side, control).map(|(solution, _)| solution)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn bounded_lm_step(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    indexed: Option<&ComponentIndexedSystem>,
    damping: f64,
    normalized_step_tolerance: f64,
    rank_relative_tolerance: f64,
    policy: LinearSolveBackendPolicy,
    backend: &mut BackendEvidence,
    mut control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    let bounds = normalized_step_bounds(problem, state, layout, jacobian.ncols())?;
    let mut step = DVector::zeros(jacobian.ncols());
    let mut working = bounds
        .iter()
        .map(|bound| {
            if bound.lower <= bound.upper && bound.upper <= bound.lower {
                WorkingBound::Fixed
            } else if bound.lower == 0.0 {
                WorkingBound::Lower
            } else if bound.upper == 0.0 {
                WorkingBound::Upper
            } else {
                WorkingBound::Free
            }
        })
        .collect::<Vec<_>>();
    for (column, status) in working.iter().enumerate() {
        step[column] = match status {
            WorkingBound::Lower | WorkingBound::Fixed => bounds[column].lower,
            WorkingBound::Upper => bounds[column].upper,
            WorkingBound::Free => 0.0,
        };
    }

    let maximum_iterations = 8usize.saturating_mul(jacobian.ncols().saturating_add(1));
    for _ in 0..maximum_iterations {
        let free = working
            .iter()
            .enumerate()
            .filter_map(|(column, status)| (*status == WorkingBound::Free).then_some(column))
            .collect::<Vec<_>>();
        let mut candidate = step.clone();
        if !free.is_empty() {
            let active_model = jacobian * &step + residuals;
            let free_contribution = DVector::from_iterator(
                jacobian.nrows(),
                (0..jacobian.nrows()).map(|row| {
                    free.iter()
                        .map(|&column| jacobian[(row, column)] * step[column])
                        .sum::<f64>()
                }),
            );
            let effective_residual = active_model - free_contribution;
            let reduced_step = lm_step_with_backend(
                problem,
                indexed,
                jacobian,
                &effective_residual,
                &free,
                damping,
                normalized_step_tolerance,
                rank_relative_tolerance,
                policy,
                backend,
                control.as_deref_mut(),
            )?;
            for (reduced_column, &column) in free.iter().enumerate() {
                candidate[column] = reduced_step[reduced_column];
            }
        }

        if let Some((alpha, column, side)) =
            first_step_bound_event(&step, &candidate, &bounds, &free)
        {
            step += (candidate - &step) * alpha;
            match side {
                WorkingBound::Lower => {
                    step[column] = bounds[column].lower;
                    working[column] = WorkingBound::Lower;
                }
                WorkingBound::Upper => {
                    step[column] = bounds[column].upper;
                    working[column] = WorkingBound::Upper;
                }
                WorkingBound::Free | WorkingBound::Fixed => return None,
            }
            continue;
        }
        step = candidate;

        let model_residuals = jacobian * &step + residuals;
        let gradient = jacobian.transpose() * model_residuals + &step * damping;
        if gradient.iter().any(|value| !value.is_finite()) {
            return None;
        }
        let tolerance = kkt_gradient_tolerance(
            jacobian,
            residuals,
            &step,
            damping,
            normalized_step_tolerance,
        )?;
        let mut release: Option<(usize, f64)> = None;
        for (column, status) in working.iter().copied().enumerate() {
            let violation = match status {
                WorkingBound::Lower if gradient[column] < -tolerance => -gradient[column],
                WorkingBound::Upper if gradient[column] > tolerance => gradient[column],
                WorkingBound::Free if gradient[column].abs() > tolerance => return None,
                WorkingBound::Free
                | WorkingBound::Lower
                | WorkingBound::Upper
                | WorkingBound::Fixed => 0.0,
            };
            if violation > 0.0
                && release
                    .as_ref()
                    .is_none_or(|(_, current)| violation > *current)
            {
                release = Some((column, violation));
            }
        }
        if let Some((column, _)) = release {
            working[column] = WorkingBound::Free;
            continue;
        }
        return step.iter().all(|value| value.is_finite()).then_some(step);
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lm_step_with_backend(
    problem: &Problem,
    indexed: Option<&ComponentIndexedSystem>,
    jacobian: &DMatrix<f64>,
    effective_residual: &DVector<f64>,
    free_columns: &[usize],
    damping: f64,
    normalized_step_tolerance: f64,
    rank_relative_tolerance: f64,
    policy: LinearSolveBackendPolicy,
    backend: &mut BackendEvidence,
    mut control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    let reduced = DMatrix::from_fn(jacobian.nrows(), free_columns.len(), |row, column| {
        jacobian[(row, free_columns[column])]
    });
    let sparse_requested = match policy {
        LinearSolveBackendPolicy::DenseOnly => false,
        LinearSolveBackendPolicy::SparsePreferred => true,
        LinearSolveBackendPolicy::Auto => indexed.is_some_and(|indexed| {
            if !auto_prefers_sparse(indexed, free_columns) {
                return false;
            }
            let rank = controlled_rank_diagnostics(
                &reduced,
                rank_relative_tolerance,
                control.as_deref_mut(),
            );
            if rank.is_some_and(|rank| {
                rank.rank == reduced.nrows().min(reduced.ncols()) && !rank.near_singular
            }) {
                true
            } else {
                backend.record_fallback(SparseFallbackReason::RankAmbiguous);
                false
            }
        }),
    };
    if sparse_requested {
        let sparse = controlled_factorization(control.as_deref_mut(), || {
            Some(
                indexed
                    .ok_or(SparseFallbackReason::ConstructionFailure)
                    .and_then(|indexed| {
                        solve_damped_least_squares(
                            indexed,
                            effective_residual,
                            free_columns,
                            damping,
                            normalized_step_tolerance,
                            &mut problem.sparse_symbolic_cache.borrow_mut(),
                        )
                        .map_err(|failure| failure.reason)
                    }),
            )
        })?;
        match sparse {
            Ok(outcome) => {
                backend.record_symbolic_reuse(outcome.symbolic_reused);
                backend.record_backend(LinearSolveBackend::SparseQr);
                return Some(outcome.step);
            }
            Err(reason) => backend.record_fallback(reason),
        }
    }

    let step = lm_step(&reduced, effective_residual, damping, control)?;
    backend.record_backend(LinearSolveBackend::Dense);
    Some(step)
}

pub(super) fn auto_prefers_sparse(system: &ComponentIndexedSystem, free_columns: &[usize]) -> bool {
    if system.row_count < AUTO_SPARSE_MIN_ROWS || free_columns.len() < AUTO_SPARSE_MIN_COLUMNS {
        return false;
    }
    let Some(nnz) = restricted_structural_nnz(system, free_columns) else {
        return false;
    };
    let Some(capacity) = system.row_count.checked_mul(free_columns.len()) else {
        return true;
    };
    if nnz < AUTO_SPARSE_MIN_NNZ || capacity == 0 {
        return false;
    }
    // AUTO_SPARSE_MAX_DENSITY is exactly 1/128; integer comparison avoids
    // architecture-dependent precision loss for large dimensions.
    nnz <= capacity / AUTO_SPARSE_MAX_DENSITY_DENOMINATOR
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NormalizedStepBound {
    pub(super) lower: f64,
    pub(super) upper: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WorkingBound {
    Free,
    Lower,
    Upper,
    Fixed,
}

pub(super) fn normalized_step_bounds(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    dimension: usize,
) -> Option<Vec<NormalizedStepBound>> {
    let mut intervals = vec![
        NormalizedStepBound {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        };
        dimension
    ];
    for (_, bound) in problem.bounds.iter() {
        let Some(column) = bound_column(layout, bound.variable_id(), bound.coordinate()) else {
            continue;
        };
        let block = layout
            .blocks
            .iter()
            .find(|block| block.tangent_range.contains(&column))?;
        let local = column - block.tangent_range.start;
        let scale = block.step_scales[local];
        let value = crate::bounds::coordinate_value(
            crate::analysis::state_value(state, bound.variable_id())?,
            bound.coordinate(),
        );
        if let Some(lower) = bound.lower() {
            intervals[column].lower = intervals[column].lower.max((lower - value) / scale);
        }
        if let Some(upper) = bound.upper() {
            intervals[column].upper = intervals[column].upper.min((upper - value) / scale);
        }
    }
    intervals
        .iter()
        .all(|bound| {
            !bound.lower.is_nan()
                && !bound.upper.is_nan()
                && bound.lower <= 0.0
                && bound.upper >= 0.0
                && bound.lower <= bound.upper
        })
        .then_some(intervals)
}

pub(super) fn first_step_bound_event(
    current: &DVector<f64>,
    candidate: &DVector<f64>,
    bounds: &[NormalizedStepBound],
    free: &[usize],
) -> Option<(f64, usize, WorkingBound)> {
    let mut event: Option<(f64, usize, WorkingBound)> = None;
    for &column in free {
        let direction = candidate[column] - current[column];
        let candidate_event = if candidate[column] < bounds[column].lower && direction < 0.0 {
            Some((
                (bounds[column].lower - current[column]) / direction,
                WorkingBound::Lower,
            ))
        } else if candidate[column] > bounds[column].upper && direction > 0.0 {
            Some((
                (bounds[column].upper - current[column]) / direction,
                WorkingBound::Upper,
            ))
        } else {
            None
        };
        let Some((alpha, side)) = candidate_event else {
            continue;
        };
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return None;
        }
        if event
            .as_ref()
            .is_none_or(|(current_alpha, current_column, _)| {
                alpha < *current_alpha
                    || alpha.total_cmp(current_alpha).is_eq() && column < *current_column
            })
        {
            event = Some((alpha, column, side));
        }
    }
    event
}

pub(super) fn kkt_gradient_tolerance(
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    step: &DVector<f64>,
    damping: f64,
    normalized_step_tolerance: f64,
) -> Option<f64> {
    let jacobian_norm = stable_norm(jacobian.iter().copied())?;
    let residual_norm = stable_norm(residuals.iter().copied())?;
    let step_norm = stable_norm(step.iter().copied())?;
    let scale = jacobian_norm * residual_norm + damping * step_norm;
    let roundoff = 64.0 * f64::EPSILON * scale;
    let step_resolution = normalized_step_tolerance * (jacobian_norm * jacobian_norm + damping);
    let tolerance = roundoff.max(step_resolution);
    tolerance.is_finite().then_some(tolerance)
}

#[derive(Clone, Debug)]
pub(super) struct ReducedStepBound {
    pub(super) normal: DVector<f64>,
    pub(super) lower: f64,
    pub(super) upper: f64,
}

pub(super) struct ConstrainedNullspaceStep {
    pub(super) step: DVector<f64>,
    pub(super) stationary: bool,
    pub(super) critical_cone: Option<ReducedCriticalCone>,
}

pub(super) struct ReducedCriticalCone {
    /// Basis in the protected-nullspace coordinates.
    pub(super) span: DMatrix<f64>,
    /// Signed inward weak-active normals in `span` coordinates.
    pub(super) inequalities: DMatrix<f64>,
}

pub(super) struct WorkingSetKkt {
    pub(super) release: Option<usize>,
    pub(super) multipliers: Vec<f64>,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn constrained_nullspace_step(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    nullspace: &DMatrix<f64>,
    reduced_jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    relative_tolerance: f64,
    normalized_step_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<ConstrainedNullspaceStep> {
    let full_bounds = normalized_step_bounds(problem, state, layout, nullspace.nrows())?;
    let mut constraints = Vec::new();
    for (column, bound) in full_bounds.iter().enumerate() {
        if !bound.lower.is_finite() && !bound.upper.is_finite() {
            continue;
        }
        let normal = nullspace.row(column).transpose().into_owned();
        let norm = stable_norm(normal.iter().copied())?;
        if norm == 0.0 {
            continue;
        }
        constraints.push(ReducedStepBound {
            normal,
            lower: bound.lower,
            upper: bound.upper,
        });
    }
    let mut reduced_step = DVector::zeros(nullspace.ncols());
    let desired_working = constraints
        .iter()
        .map(|constraint| {
            if constraint.lower <= constraint.upper && constraint.upper <= constraint.lower {
                WorkingBound::Fixed
            } else if constraint.lower == 0.0 {
                WorkingBound::Lower
            } else if constraint.upper == 0.0 {
                WorkingBound::Upper
            } else {
                WorkingBound::Free
            }
        })
        .collect::<Vec<_>>();
    let mut working = independent_initial_working_set(
        &constraints,
        &desired_working,
        relative_tolerance,
        control.as_deref_mut(),
    )?;
    let maximum_iterations = 8usize.saturating_mul(constraints.len().saturating_add(1));
    for _ in 0..maximum_iterations {
        let candidate = solve_active_reduced_least_squares(
            reduced_jacobian,
            residuals,
            &constraints,
            &working,
            relative_tolerance,
            control.as_deref_mut(),
        )?;
        if let Some((alpha, constraint, side)) =
            first_linear_bound_event(&reduced_step, &candidate, &constraints, &working)
        {
            reduced_step += (candidate - &reduced_step) * alpha;
            if working_constraint_is_independent(
                &constraints,
                &working,
                constraint,
                relative_tolerance,
                control.as_deref_mut(),
            )? {
                working[constraint] = side;
            } else if !constraint_satisfied(&constraints[constraint], &reduced_step) {
                return None;
            }
            continue;
        }
        reduced_step = candidate;

        let model_residuals = reduced_jacobian * &reduced_step + residuals;
        let gradient = reduced_jacobian.transpose() * &model_residuals;
        if gradient.iter().any(|value| !value.is_finite()) {
            return None;
        }
        let tolerance = kkt_gradient_tolerance(
            reduced_jacobian,
            residuals,
            &reduced_step,
            0.0,
            normalized_step_tolerance,
        )?;
        let kkt = working_set_kkt(
            &gradient,
            &constraints,
            &working,
            tolerance,
            control.as_deref_mut(),
        )
        .ok()?;
        if let Some(constraint) = kkt.release {
            working[constraint] = WorkingBound::Free;
            continue;
        }
        let mut step = nullspace * &reduced_step;
        snap_constrained_roundoff(&mut step, &full_bounds)?;
        let current_cost = residual_cost(residuals)?;
        let model_cost = residual_cost(&model_residuals)?;
        let stationary = stable_norm(step.iter().copied())? <= normalized_step_tolerance
            || !objective_decreases(current_cost, model_cost);
        let has_active_bound = constraints
            .iter()
            .any(|constraint| constraint.lower == 0.0 || constraint.upper == 0.0);
        let critical_cone = if stationary || !has_active_bound {
            Some(reduced_critical_cone(
                nullspace.ncols(),
                &constraints,
                &working,
                &kkt.multipliers,
                relative_tolerance,
                tolerance,
                control.as_deref_mut(),
            )?)
        } else {
            None
        };
        return step
            .iter()
            .all(|value| value.is_finite())
            .then_some(ConstrainedNullspaceStep {
                step,
                stationary,
                critical_cone,
            });
    }
    None
}

pub(super) fn snap_constrained_roundoff(
    step: &mut DVector<f64>,
    bounds: &[NormalizedStepBound],
) -> Option<()> {
    let norm = stable_norm(step.iter().copied())?;
    let tolerance = 64.0 * f64::EPSILON * norm;
    for (value, bound) in step.iter_mut().zip(bounds) {
        if bound.lower == 0.0 && *value < 0.0 && value.abs() <= tolerance {
            *value = 0.0;
        }
        if bound.upper == 0.0 && *value > 0.0 && value.abs() <= tolerance {
            *value = 0.0;
        }
    }
    Some(())
}

pub(super) fn solve_active_reduced_least_squares(
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    relative_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    let active = working
        .iter()
        .enumerate()
        .filter_map(|(index, status)| (*status != WorkingBound::Free).then_some(index))
        .collect::<Vec<_>>();
    let dimension = jacobian.ncols();
    let (particular, tangent) = if active.is_empty() {
        (
            DVector::zeros(dimension),
            DMatrix::identity(dimension, dimension),
        )
    } else {
        let matrix = DMatrix::from_fn(active.len(), dimension, |row, column| {
            constraints[active[row]].normal[column]
        });
        let right_hand_side = DVector::from_iterator(
            active.len(),
            active.iter().map(|&index| match working[index] {
                WorkingBound::Lower | WorkingBound::Fixed => constraints[index].lower,
                WorkingBound::Upper => constraints[index].upper,
                WorkingBound::Free => 0.0,
            }),
        );
        let particular =
            solve_dense_least_squares(&matrix, &right_hand_side, control.as_deref_mut())?.0;
        let tangent = controlled_dense_factorization(
            matrix.nrows(),
            matrix.ncols(),
            control.as_deref_mut(),
            || numerical_nullspace(&matrix, relative_tolerance),
        )?;
        (particular, tangent)
    };
    if tangent.ncols() == 0 {
        return Some(particular);
    }
    let reduced = jacobian * &tangent;
    let effective = residuals + jacobian * &particular;
    let correction =
        solve_rank_aware_least_squares(&reduced, &(-effective), relative_tolerance, control)?;
    let candidate = particular + tangent * correction;
    candidate
        .iter()
        .all(|value| value.is_finite())
        .then_some(candidate)
}

pub(super) fn first_linear_bound_event(
    current: &DVector<f64>,
    candidate: &DVector<f64>,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
) -> Option<(f64, usize, WorkingBound)> {
    let mut event: Option<(f64, usize, WorkingBound)> = None;
    for (index, constraint) in constraints.iter().enumerate() {
        if working[index] != WorkingBound::Free {
            continue;
        }
        let current_value = constraint.normal.dot(current);
        let candidate_value = constraint.normal.dot(candidate);
        let direction = candidate_value - current_value;
        let fixed = constraint.lower <= constraint.upper && constraint.upper <= constraint.lower;
        let candidate_event = if candidate_value < constraint.lower && direction < 0.0 {
            Some((
                (constraint.lower - current_value) / direction,
                if fixed {
                    WorkingBound::Fixed
                } else {
                    WorkingBound::Lower
                },
            ))
        } else if candidate_value > constraint.upper && direction > 0.0 {
            Some((
                (constraint.upper - current_value) / direction,
                if fixed {
                    WorkingBound::Fixed
                } else {
                    WorkingBound::Upper
                },
            ))
        } else {
            None
        };
        let Some((alpha, side)) = candidate_event else {
            continue;
        };
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return None;
        }
        if event
            .as_ref()
            .is_none_or(|(current_alpha, current_index, _)| {
                alpha < *current_alpha
                    || alpha.total_cmp(current_alpha).is_eq() && index < *current_index
            })
        {
            event = Some((alpha, index, side));
        }
    }
    event
}

pub(super) fn independent_initial_working_set(
    constraints: &[ReducedStepBound],
    desired: &[WorkingBound],
    relative_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<Vec<WorkingBound>> {
    let mut working = vec![WorkingBound::Free; constraints.len()];
    for fixed_only in [true, false] {
        for (index, status) in desired.iter().copied().enumerate() {
            if status == WorkingBound::Free || (status == WorkingBound::Fixed) != fixed_only {
                continue;
            }
            let independent = working_constraint_is_independent(
                constraints,
                &working,
                index,
                relative_tolerance,
                control.as_deref_mut(),
            )?;
            if status == WorkingBound::Fixed || independent {
                // Fixed coordinate bounds are equalities. Retain even
                // linearly dependent occurrences: their step right-hand side
                // is zero at the accepted bound, and dropping one allows
                // roundoff in the rank-reduced solve to rediscover it forever
                // as a zero-length bound event. The independence check still
                // runs so controlled-work accounting stays unchanged.
                working[index] = status;
            }
        }
    }
    Some(working)
}

pub(super) fn working_constraint_is_independent(
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    candidate: usize,
    relative_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<bool> {
    if working.get(candidate)? != &WorkingBound::Free {
        return Some(false);
    }
    let active = working
        .iter()
        .enumerate()
        .filter_map(|(index, status)| (*status != WorkingBound::Free).then_some(index))
        .collect::<Vec<_>>();
    let dimension = constraints.get(candidate)?.normal.len();
    let current = DMatrix::from_fn(active.len(), dimension, |row, column| {
        constraints[active[row]].normal[column]
    });
    let next = DMatrix::from_fn(active.len() + 1, dimension, |row, column| {
        if row < active.len() {
            constraints[active[row]].normal[column]
        } else {
            constraints[candidate].normal[column]
        }
    });
    let current_nullity = controlled_dense_factorization(
        current.nrows(),
        current.ncols(),
        control.as_deref_mut(),
        || numerical_nullspace(&current, relative_tolerance),
    )?
    .ncols();
    let next_nullity = controlled_dense_factorization(next.nrows(), next.ncols(), control, || {
        numerical_nullspace(&next, relative_tolerance)
    })?
    .ncols();
    Some(next_nullity < current_nullity)
}

pub(super) fn constraint_satisfied(constraint: &ReducedStepBound, step: &DVector<f64>) -> bool {
    let value = constraint.normal.dot(step);
    let tolerance = 64.0
        * f64::EPSILON
        * stable_norm(step.iter().copied()).unwrap_or(f64::INFINITY)
        * stable_norm(constraint.normal.iter().copied()).unwrap_or(f64::INFINITY);
    value >= constraint.lower - tolerance && value <= constraint.upper + tolerance
}

pub(super) fn working_set_kkt(
    gradient: &DVector<f64>,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    tolerance: f64,
    control: Option<&mut OperationController>,
) -> Result<WorkingSetKkt, ()> {
    let active = working
        .iter()
        .enumerate()
        .filter_map(|(index, status)| (*status != WorkingBound::Free).then_some(index))
        .collect::<Vec<_>>();
    let mut multipliers = vec![0.0; constraints.len()];
    if active.is_empty() {
        let norm = stable_norm(gradient.iter().copied()).ok_or(())?;
        let roundoff = 64.0 * f64::EPSILON * norm;
        return (norm <= tolerance.max(roundoff))
            .then_some(WorkingSetKkt {
                release: None,
                multipliers,
            })
            .ok_or(());
    }

    let matrix = DMatrix::from_fn(active.len(), gradient.len(), |row, column| {
        constraints[active[row]].normal[column]
    });
    let multiplier_values = solve_dense_least_squares(&matrix.transpose(), &(-gradient), control)
        .ok_or(())?
        .0;
    let stationarity = gradient + matrix.transpose() * &multiplier_values;
    let stationarity_norm = stable_norm(stationarity.iter().copied()).ok_or(())?;
    let gradient_norm = stable_norm(gradient.iter().copied()).ok_or(())?;
    let stationarity_tolerance = tolerance.max(64.0 * f64::EPSILON * gradient_norm);
    if stationarity_norm > stationarity_tolerance {
        return Err(());
    }
    for (position, &index) in active.iter().enumerate() {
        multipliers[index] = multiplier_values[position];
    }

    let mut release: Option<(usize, f64)> = None;
    for &index in &active {
        let violation = match working[index] {
            WorkingBound::Lower if multipliers[index] > tolerance => multipliers[index],
            WorkingBound::Upper if multipliers[index] < -tolerance => -multipliers[index],
            WorkingBound::Free
            | WorkingBound::Lower
            | WorkingBound::Upper
            | WorkingBound::Fixed => 0.0,
        };
        if violation > 0.0
            && release
                .as_ref()
                .is_none_or(|(_, current)| violation > *current)
        {
            release = Some((index, violation));
        }
    }
    Ok(WorkingSetKkt {
        release: release.map(|(index, _)| index),
        multipliers,
    })
}

pub(super) fn reduced_critical_cone(
    dimension: usize,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    multipliers: &[f64],
    relative_tolerance: f64,
    multiplier_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<ReducedCriticalCone> {
    let mut equalities = Vec::new();
    let mut weak_inequalities = Vec::new();
    for (index, constraint) in constraints.iter().enumerate() {
        let fixed = constraint.lower == 0.0 && constraint.upper == 0.0;
        if fixed {
            equalities.push(constraint.normal.clone());
            continue;
        }
        let active_lower = constraint.lower == 0.0;
        let active_upper = constraint.upper == 0.0;
        if !active_lower && !active_upper {
            continue;
        }
        let strong = match working[index] {
            WorkingBound::Lower => -multipliers[index] > multiplier_tolerance,
            WorkingBound::Upper => multipliers[index] > multiplier_tolerance,
            WorkingBound::Free | WorkingBound::Fixed => false,
        };
        let sign = if active_lower { 1.0 } else { -1.0 };
        if strong {
            equalities.push(constraint.normal.clone());
        } else {
            weak_inequalities.push(&constraint.normal * sign);
        }
    }

    let equality_matrix = DMatrix::from_fn(equalities.len(), dimension, |row, column| {
        equalities[row][column]
    });
    let span = controlled_dense_factorization(
        equality_matrix.nrows(),
        equality_matrix.ncols(),
        control,
        || numerical_nullspace(&equality_matrix, relative_tolerance),
    )?;
    let mut projected_inequalities = Vec::new();
    for inequality in weak_inequalities {
        let projected = span.transpose() * inequality;
        let norm = stable_norm(projected.iter().copied())?;
        if norm > 64.0 * f64::EPSILON {
            projected_inequalities.push(projected);
        }
    }
    let inequalities =
        DMatrix::from_fn(projected_inequalities.len(), span.ncols(), |row, column| {
            projected_inequalities[row][column]
        });
    Some(ReducedCriticalCone { span, inequalities })
}

pub(super) fn step_is_within_bounds(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    step: &mut DVector<f64>,
) -> Option<()> {
    let bounds = normalized_step_bounds(problem, state, layout, step.len())?;
    snap_constrained_roundoff(step, &bounds)?;
    step.iter()
        .zip(&bounds)
        .all(|(value, bound)| *value >= bound.lower && *value <= bound.upper)
        .then_some(())
}

pub(super) fn operator_step_is_within_bounds(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    step: &mut DVector<f64>,
) -> Option<()> {
    let bounds = normalized_step_bounds(problem, state, layout, step.len())?;
    snap_constrained_roundoff(step, &bounds)?;
    operator_full_step_satisfies_bounds(step, &bounds).then_some(())
}

pub(super) fn operator_full_step_satisfies_bounds(
    step: &DVector<f64>,
    bounds: &[NormalizedStepBound],
) -> bool {
    step.len() == bounds.len()
        && step.iter().all(|value| value.is_finite())
        && step
            .iter()
            .zip(bounds)
            .all(|(value, bound)| *value >= bound.lower && *value <= bound.upper)
}

pub(super) fn limit_operator_step(
    reduced_step: &mut DVector<f64>,
    full_step: &mut DVector<f64>,
    layout: &ActiveLayout,
    limit: f64,
) -> Option<()> {
    let maximum = maximum_block_step(full_step, layout)?;
    if maximum > limit {
        let scale = limit / maximum;
        reduced_step.scale_mut(scale);
        full_step.scale_mut(scale);
    }
    reduced_step
        .iter()
        .chain(full_step.iter())
        .all(|value| value.is_finite())
        .then_some(())
}

pub(super) fn limit_step_to_bound_events(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    step: &mut DVector<f64>,
) -> Option<f64> {
    let bounds = normalized_step_bounds(problem, state, layout, step.len())?;
    snap_constrained_roundoff(step, &bounds)?;
    let mut alpha = 1.0_f64;
    for (_, bound) in problem.bounds.iter() {
        let Some(column) = bound_column(layout, bound.variable_id(), bound.coordinate()) else {
            continue;
        };
        let block = layout
            .blocks
            .iter()
            .find(|block| block.tangent_range.contains(&column))?;
        let local = column - block.tangent_range.start;
        let raw_direction = step[column] * block.step_scales[local];
        let value = crate::bounds::coordinate_value(
            crate::analysis::state_value(state, bound.variable_id())?,
            bound.coordinate(),
        );
        if raw_direction < 0.0
            && let Some(lower) = bound.lower()
            && value + alpha * raw_direction < lower
        {
            alpha = alpha.min((lower - value) / raw_direction);
        } else if raw_direction > 0.0
            && let Some(upper) = bound.upper()
            && value + alpha * raw_direction > upper
        {
            alpha = alpha.min((upper - value) / raw_direction);
        }
    }
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return None;
    }
    step.scale_mut(alpha);
    step.iter().all(|value| value.is_finite()).then_some(alpha)
}

pub(super) fn enforce_state_bounds(
    problem: &Problem,
    plan: &EliminationPlan,
    state: &mut VariableState,
) -> Result<(), CoreError> {
    let mut snaps = Vec::new();
    for (bound_id, bound) in problem.bounds.iter() {
        let value = crate::bounds::coordinate_value(
            crate::analysis::state_value(state, bound.variable_id())
                .ok_or(CoreError::UnknownVariable(bound.variable_id()))?,
            bound.coordinate(),
        );
        let target = if let Some(lower) = bound.lower()
            && at_bound_endpoint(value, lower)
        {
            Some(lower)
        } else if let Some(upper) = bound.upper()
            && at_bound_endpoint(value, upper)
        {
            Some(upper)
        } else {
            None
        };
        if !bound.contains(value) && target.is_none() {
            let _ = bound_id;
            return Err(CoreError::ValueOutsideBound {
                variable: bound.variable_id(),
                coordinate: bound.coordinate(),
                value,
                lower: bound.lower(),
                upper: bound.upper(),
            });
        }
        if let Some(target) = target {
            let root = plan
                .root(bound.variable_id())
                .ok_or(CoreError::UnknownVariable(bound.variable_id()))?;
            snaps.push((root, bound.coordinate(), target));
        }
    }
    for (variable_id, coordinate, target) in snaps {
        let (_, value) = state
            .values
            .iter_mut()
            .find(|(id, _)| *id == variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))?;
        crate::bounds::set_coordinate_value(value, coordinate, target)?;
    }
    plan.synchronize_state(problem, state)?;
    for (_, bound) in problem.bounds.iter() {
        let value = crate::bounds::coordinate_value(
            crate::analysis::state_value(state, bound.variable_id())
                .ok_or(CoreError::UnknownVariable(bound.variable_id()))?,
            bound.coordinate(),
        );
        if !bound.contains(value) {
            return Err(CoreError::ValueOutsideBound {
                variable: bound.variable_id(),
                coordinate: bound.coordinate(),
                value,
                lower: bound.lower(),
                upper: bound.upper(),
            });
        }
    }
    Ok(())
}

pub(super) fn project_initial_state_into_bounds(
    problem: &Problem,
    plan: &EliminationPlan,
    state: &mut VariableState,
) -> Result<(), CoreError> {
    let mut projections = Vec::new();
    for (_, bound) in problem.bounds.iter() {
        let value = crate::bounds::coordinate_value(
            crate::analysis::state_value(state, bound.variable_id())
                .ok_or(CoreError::UnknownVariable(bound.variable_id()))?,
            bound.coordinate(),
        );
        let target = if let Some(lower) = bound.lower()
            && value < lower
        {
            Some(lower)
        } else if let Some(upper) = bound.upper()
            && value > upper
        {
            Some(upper)
        } else {
            None
        };
        if let Some(target) = target {
            let root = plan
                .root(bound.variable_id())
                .ok_or(CoreError::UnknownVariable(bound.variable_id()))?;
            projections.push((root, bound.coordinate(), target));
        }
    }
    for (variable_id, coordinate, target) in projections {
        let (_, value) = state
            .values
            .iter_mut()
            .find(|(id, _)| *id == variable_id)
            .ok_or(CoreError::UnknownVariable(variable_id))?;
        crate::bounds::set_coordinate_value(value, coordinate, target)?;
    }
    plan.synchronize_state(problem, state)?;
    enforce_state_bounds(problem, plan, state)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LinearSolveMethod {
    Qr,
    Svd,
}

pub(super) fn solve_dense_least_squares(
    matrix: &DMatrix<f64>,
    right_hand_side: &DVector<f64>,
    mut control: Option<&mut OperationController>,
) -> Option<(DVector<f64>, LinearSolveMethod)> {
    let rows = matrix.nrows();
    let columns = matrix.ncols();
    if right_hand_side.len() != rows {
        return None;
    }
    if columns == 0 {
        return Some((DVector::zeros(0), LinearSolveMethod::Qr));
    }
    if rows >= columns {
        let qr = controlled_dense_factorization(rows, columns, control.as_deref_mut(), || {
            Some(matrix.clone().qr())
        })?;
        let mut transformed = right_hand_side.clone();
        qr.q_tr_mul(&mut transformed);
        let triangular = qr.r();
        let transformed_top = transformed.rows(0, columns).into_owned();
        if let Some(solution) = triangular.solve_upper_triangular(&transformed_top)
            && solution.iter().all(|value| value.is_finite())
        {
            return Some((solution, LinearSolveMethod::Qr));
        }
    }
    let svd = controlled_dense_factorization(rows, columns, control, || {
        Some(matrix.clone().svd(true, true))
    })?;
    let largest = svd.singular_values.iter().copied().fold(0.0_f64, f64::max);
    let dimension = u32::try_from(rows.max(columns)).ok()?;
    let epsilon = f64::EPSILON * f64::from(dimension) * largest;
    let mut solution = svd.solve(right_hand_side, epsilon).ok()?;
    if solution.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut residual = matrix * &solution - right_hand_side;
    let mut residual_norm = stable_norm(residual.iter().copied())?;
    let mut normal_residual = matrix.transpose() * &residual;
    let mut normal_residual_norm = stable_norm(normal_residual.iter().copied())?;
    let matrix_norm = stable_norm(matrix.iter().copied())?;
    let right_hand_side_norm = stable_norm(right_hand_side.iter().copied())?;
    for _ in 0..8 {
        let solution_norm = stable_norm(solution.iter().copied())?;
        let roundoff_floor = 64.0
            * f64::EPSILON
            * matrix_norm
            * (matrix_norm * solution_norm + right_hand_side_norm);
        if !roundoff_floor.is_finite() || normal_residual_norm <= roundoff_floor {
            break;
        }
        let correction_right_hand_side = -&residual;
        let Ok(correction) = svd.solve(&correction_right_hand_side, epsilon) else {
            break;
        };
        let candidate = &solution + correction;
        if candidate.iter().any(|value| !value.is_finite()) {
            break;
        }
        let candidate_residual = matrix * &candidate - right_hand_side;
        let Some(candidate_residual_norm) = stable_norm(candidate_residual.iter().copied()) else {
            break;
        };
        normal_residual = matrix.transpose() * &candidate_residual;
        let Some(candidate_norm) = stable_norm(normal_residual.iter().copied()) else {
            break;
        };
        if candidate_norm >= normal_residual_norm || candidate_residual_norm > residual_norm {
            break;
        }
        solution = candidate;
        residual = candidate_residual;
        residual_norm = candidate_residual_norm;
        normal_residual_norm = candidate_norm;
    }
    Some((solution, LinearSolveMethod::Svd))
}

pub(super) fn solve_rank_aware_least_squares(
    matrix: &DMatrix<f64>,
    right_hand_side: &DVector<f64>,
    relative_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    if right_hand_side.len() != matrix.nrows() {
        return None;
    }
    if matrix.ncols() == 0 {
        return Some(DVector::zeros(0));
    }
    if matrix.nrows() == 2 && matrix.ncols() == 2 {
        return solve_fixed_2x2_rank_aware_least_squares(
            matrix,
            right_hand_side,
            relative_tolerance,
            control,
        );
    }
    let diagnostics = controlled_dense_factorization(
        matrix.nrows(),
        matrix.ncols(),
        control.as_deref_mut(),
        || rank_diagnostics(matrix, relative_tolerance),
    )?;
    let decomposition =
        controlled_dense_factorization(matrix.nrows(), matrix.ncols(), control, || {
            Some(matrix.clone().svd(true, true))
        })?;
    let mut solution = decomposition
        .solve(right_hand_side, diagnostics.threshold)
        .ok()?;
    if solution.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut residual = matrix * &solution - right_hand_side;
    let mut residual_norm = stable_norm(residual.iter().copied())?;
    let mut normal_residual = matrix.transpose() * &residual;
    let mut normal_residual_norm = stable_norm(normal_residual.iter().copied())?;
    let matrix_norm = stable_norm(matrix.iter().copied())?;
    let right_hand_side_norm = stable_norm(right_hand_side.iter().copied())?;
    for _ in 0..8 {
        let solution_norm = stable_norm(solution.iter().copied())?;
        let roundoff_floor = 64.0
            * f64::EPSILON
            * matrix_norm
            * (matrix_norm * solution_norm + right_hand_side_norm);
        if !roundoff_floor.is_finite() || normal_residual_norm <= roundoff_floor {
            break;
        }
        let correction_right_hand_side = -&residual;
        let Ok(correction) =
            decomposition.solve(&correction_right_hand_side, diagnostics.threshold)
        else {
            break;
        };
        let candidate = &solution + correction;
        if candidate.iter().any(|value| !value.is_finite()) {
            break;
        }
        let candidate_residual = matrix * &candidate - right_hand_side;
        let Some(candidate_residual_norm) = stable_norm(candidate_residual.iter().copied()) else {
            break;
        };
        normal_residual = matrix.transpose() * &candidate_residual;
        let Some(candidate_norm) = stable_norm(normal_residual.iter().copied()) else {
            break;
        };
        if candidate_norm >= normal_residual_norm || candidate_residual_norm > residual_norm {
            break;
        }
        solution = candidate;
        residual = candidate_residual;
        residual_norm = candidate_residual_norm;
        normal_residual_norm = candidate_norm;
    }
    Some(solution)
}

/// Rank-aware minimum-norm solve for the common two-coordinate cursor projection.
///
/// A 2D target projected through one instantaneous mechanism freedom produces an
/// almost exactly rank-one `2 x 2` system. Nalgebra's fixed-size analytic SVD avoids
/// the cancellation that its dynamic bidiagonal path can accumulate in this corner.
/// The returned step is still accepted only after first-order stationarity and
/// retained-row-space (minimum-norm) certification under the authoritative rank
/// cutoff.
pub(super) fn solve_fixed_2x2_rank_aware_least_squares(
    matrix: &DMatrix<f64>,
    right_hand_side: &DVector<f64>,
    relative_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    let (solution, row_space_projection, threshold) =
        controlled_dense_factorization(2, 2, control, || {
            let fixed = Matrix2::new(
                matrix[(0, 0)],
                matrix[(0, 1)],
                matrix[(1, 0)],
                matrix[(1, 1)],
            );
            let decomposition = fixed.svd(true, true);
            let sigma_max = decomposition
                .singular_values
                .iter()
                .copied()
                .fold(0.0_f64, f64::max);
            let (_, _, threshold) = rank_thresholds(2, 2, sigma_max, relative_tolerance)?;
            let fixed_right_hand_side = Vector2::new(right_hand_side[0], right_hand_side[1]);
            let fixed_solution = decomposition
                .solve(&fixed_right_hand_side, threshold)
                .ok()?;
            let right_vectors = decomposition.v_t.as_ref()?;
            let mut fixed_projection = Vector2::zeros();
            for row in 0..2 {
                if decomposition.singular_values[row] <= threshold {
                    continue;
                }
                let coefficient = right_vectors[(row, 0)] * fixed_solution[0]
                    + right_vectors[(row, 1)] * fixed_solution[1];
                fixed_projection[0] += right_vectors[(row, 0)] * coefficient;
                fixed_projection[1] += right_vectors[(row, 1)] * coefficient;
            }
            Some((
                DVector::from_vec(vec![fixed_solution[0], fixed_solution[1]]),
                DVector::from_vec(vec![fixed_projection[0], fixed_projection[1]]),
                threshold,
            ))
        })?;
    rank_aware_least_squares_is_certified(
        matrix,
        right_hand_side,
        &solution,
        &row_space_projection,
        threshold,
    )
    .then_some(solution)
}

pub(super) fn rank_aware_least_squares_is_certified(
    matrix: &DMatrix<f64>,
    right_hand_side: &DVector<f64>,
    solution: &DVector<f64>,
    row_space_projection: &DVector<f64>,
    singular_value_threshold: f64,
) -> bool {
    if solution.len() != matrix.ncols()
        || row_space_projection.len() != solution.len()
        || solution.iter().any(|value| !value.is_finite())
        || row_space_projection.iter().any(|value| !value.is_finite())
    {
        return false;
    }
    let Some(solution_norm) = stable_norm(solution.iter().copied()) else {
        return false;
    };
    let Some(minimum_norm_error) = stable_norm((solution - row_space_projection).iter().copied())
    else {
        return false;
    };
    let dimension = f64::from(u32::try_from(solution.len().max(1)).unwrap_or(u32::MAX));
    let minimum_norm_roundoff = 64.0 * f64::EPSILON * dimension * solution_norm;
    least_squares_stationarity_is_certified(
        matrix,
        right_hand_side,
        solution,
        singular_value_threshold,
    ) && minimum_norm_roundoff.is_finite()
        && minimum_norm_error <= minimum_norm_roundoff
}

pub(super) fn least_squares_stationarity_is_certified(
    matrix: &DMatrix<f64>,
    right_hand_side: &DVector<f64>,
    solution: &DVector<f64>,
    singular_value_threshold: f64,
) -> bool {
    if right_hand_side.len() != matrix.nrows()
        || solution.len() != matrix.ncols()
        || matrix.iter().any(|value| !value.is_finite())
        || right_hand_side.iter().any(|value| !value.is_finite())
        || solution.iter().any(|value| !value.is_finite())
        || !singular_value_threshold.is_finite()
    {
        return false;
    }
    let residual = matrix * solution - right_hand_side;
    let gradient = matrix.transpose() * &residual;
    let Some(residual_norm) = stable_norm(residual.iter().copied()) else {
        return false;
    };
    let Some(gradient_norm) = stable_norm(gradient.iter().copied()) else {
        return false;
    };
    let Some(matrix_norm) = stable_norm(matrix.iter().copied()) else {
        return false;
    };
    let Some(solution_norm) = stable_norm(solution.iter().copied()) else {
        return false;
    };
    let Some(right_hand_side_norm) = stable_norm(right_hand_side.iter().copied()) else {
        return false;
    };
    let rank_cutoff = singular_value_threshold * residual_norm;
    let roundoff =
        64.0 * f64::EPSILON * matrix_norm * (matrix_norm * solution_norm + right_hand_side_norm);
    rank_cutoff.is_finite() && roundoff.is_finite() && gradient_norm <= rank_cutoff.max(roundoff)
}

pub(super) fn limit_block_steps(
    step: &mut DVector<f64>,
    layout: &ActiveLayout,
    limit: f64,
) -> Option<f64> {
    let mut maximum = 0.0_f64;
    for block in &layout.blocks {
        let range = block.tangent_range.clone();
        let norm = stable_norm(step.rows(range.start, range.len()).iter().copied())?;
        if norm > limit {
            step.rows_mut(range.start, range.len())
                .scale_mut(limit / norm);
        }
        maximum = maximum.max(norm.min(limit));
    }
    step.iter()
        .all(|value| value.is_finite())
        .then_some(maximum)
}

pub(super) fn maximum_block_step(step: &DVector<f64>, layout: &ActiveLayout) -> Option<f64> {
    layout.blocks.iter().try_fold(0.0_f64, |maximum, block| {
        stable_norm(
            step.rows(block.tangent_range.start, block.tangent_range.len())
                .iter()
                .copied(),
        )
        .map(|norm| maximum.max(norm))
    })
}

pub(super) fn predicted_reduction(
    system: &HardSystem,
    step: &DVector<f64>,
    cost: f64,
) -> Option<f64> {
    let model_cost = residual_cost(&(&system.residuals + &system.jacobian * step))?;
    let reduction = cost - model_cost;
    reduction.is_finite().then_some(reduction)
}

pub(super) fn residual_cost(residuals: &DVector<f64>) -> Option<f64> {
    let cost = 0.5 * residuals.dot(residuals);
    cost.is_finite().then_some(cost)
}

pub(super) fn residual_max(residuals: &DVector<f64>) -> f64 {
    residuals
        .iter()
        .map(|value| value.abs())
        .fold(0.0, f64::max)
}

pub(super) fn residual_norms(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
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

pub(super) fn stable_norm(values: impl Iterator<Item = f64>) -> Option<f64> {
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

pub(crate) struct RankDiagnostics {
    pub(crate) rank: usize,
    pub(crate) relative_tolerance: f64,
    pub(crate) relative_threshold: f64,
    pub(crate) machine_tolerance: f64,
    pub(crate) threshold: f64,
    pub(crate) sigma_max: f64,
    pub(crate) smallest_retained: Option<f64>,
    pub(crate) near_singular_factor: f64,
    pub(crate) near_singular_ratio: Option<f64>,
    pub(crate) near_singular: bool,
    pub(crate) singular_values: Vec<f64>,
}

pub(crate) fn rank_diagnostics(
    jacobian: &DMatrix<f64>,
    relative_tolerance: f64,
) -> Option<RankDiagnostics> {
    if jacobian.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let singular_values: Vec<_> = if jacobian.nrows() == 0 || jacobian.ncols() == 0 {
        Vec::new()
    } else {
        jacobian
            .clone()
            .svd(false, false)
            .singular_values
            .iter()
            .copied()
            .collect()
    };
    if singular_values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let sigma_max = singular_values.iter().copied().fold(0.0, f64::max);
    let (relative_threshold, machine_tolerance, threshold) = rank_thresholds(
        jacobian.nrows(),
        jacobian.ncols(),
        sigma_max,
        relative_tolerance,
    )?;
    let rank = singular_values
        .iter()
        .filter(|&&value| value > threshold)
        .count();
    let smallest_retained = singular_values
        .iter()
        .copied()
        .filter(|&value| value > threshold)
        .min_by(f64::total_cmp);
    let near_singular_ratio = smallest_retained.map(|value| value / threshold);
    if near_singular_ratio.is_some_and(|value| !value.is_finite()) {
        return None;
    }
    Some(RankDiagnostics {
        rank,
        relative_tolerance,
        relative_threshold,
        machine_tolerance,
        threshold,
        sigma_max,
        smallest_retained,
        near_singular_factor: NEAR_SINGULAR_FACTOR,
        near_singular_ratio,
        near_singular: near_singular_ratio.is_some_and(|ratio| ratio <= NEAR_SINGULAR_FACTOR),
        singular_values,
    })
}

pub(super) fn empty_rank_diagnostics(
    rows: usize,
    columns: usize,
    relative_tolerance: f64,
) -> RankDiagnostics {
    let (relative_threshold, machine_tolerance, threshold) =
        rank_thresholds(rows, columns, 0.0, relative_tolerance).unwrap_or((
            f64::MAX,
            f64::MAX,
            f64::MAX,
        ));
    RankDiagnostics {
        rank: 0,
        relative_tolerance,
        relative_threshold,
        machine_tolerance,
        threshold,
        sigma_max: 0.0,
        smallest_retained: None,
        near_singular_factor: NEAR_SINGULAR_FACTOR,
        near_singular_ratio: None,
        near_singular: false,
        singular_values: Vec::new(),
    }
}

pub(super) fn rank_thresholds(
    rows: usize,
    columns: usize,
    sigma_max: f64,
    relative_tolerance: f64,
) -> Option<(f64, f64, f64)> {
    if !sigma_max.is_finite() || !relative_tolerance.is_finite() {
        return None;
    }
    let dimension = u32::try_from(rows.max(columns).max(1)).ok()?;
    let machine_tolerance = f64::EPSILON * f64::from(dimension) * sigma_max.max(1.0);
    let relative_threshold = relative_tolerance * sigma_max;
    let threshold = relative_threshold.max(machine_tolerance);
    (relative_threshold.is_finite() && machine_tolerance.is_finite() && threshold.is_finite())
        .then_some((relative_threshold, machine_tolerance, threshold))
}

pub(super) fn rejected_record(
    iteration: usize,
    cost: f64,
    damping: f64,
    predicted_reduction: f64,
    normalized_step_max: f64,
    trial_valid: bool,
) -> SolveTraceRecord {
    SolveTraceRecord {
        component_index: None,
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

pub(super) fn stamp_component_trace(trace: &mut SolveTrace, component_index: usize) {
    for record in &mut trace.records {
        record.component_index = Some(component_index);
    }
}

pub(super) fn append_component_trace(combined: &mut SolveTrace, component: &SolveTrace) {
    for record in &component.records {
        let mut record = record.clone();
        record.iteration = combined.records.len() + 1;
        combined.records.push(record);
    }
}

pub(super) fn increase_damping(damping: &mut f64, config: &SolverConfig) -> bool {
    if *damping >= config.maximum_damping {
        return false;
    }
    *damping = (*damping * config.damping_increase_factor).min(config.maximum_damping);
    true
}

pub(super) fn update_accepted_damping(damping: &mut f64, ratio: f64, config: &SolverConfig) {
    if ratio > 0.75 {
        *damping = (*damping * config.damping_decrease_factor).max(config.minimum_damping);
    } else if ratio < 0.25 {
        *damping = (*damping * config.damping_increase_factor).min(config.maximum_damping);
    }
}

pub(super) fn recoverable_trial_error(error: &CoreError) -> bool {
    matches!(
        error,
        CoreError::InvalidGeometry { .. }
            | CoreError::CategorizedEvaluation { .. }
            | CoreError::NonFiniteValue { .. }
    )
}

pub(super) fn error_termination(error: &CoreError) -> SolveTermination {
    if matches!(
        error,
        CoreError::InvalidGeometry { .. } | CoreError::CategorizedEvaluation { .. }
    ) {
        SolveTermination::InvalidGeometry
    } else {
        SolveTermination::NumericalFailure
    }
}

pub(super) const fn secondary_status(
    termination: SolveTermination,
    fixed_only: bool,
) -> SecondaryStatus {
    match termination {
        SolveTermination::Converged if fixed_only => SecondaryStatus::Acceptable,
        SolveTermination::Converged => SecondaryStatus::Optimal,
        SolveTermination::Stalled => SecondaryStatus::Stalled,
        SolveTermination::IterationLimit => SecondaryStatus::IterationLimit,
        SolveTermination::InvalidGeometry | SolveTermination::NumericalFailure => {
            SecondaryStatus::EvaluationFailure
        }
    }
}

pub(super) fn aggregate_hard_validity(components: &[ComponentSolveReport]) -> HardValidity {
    if components
        .iter()
        .any(|component| component.hard_validity == HardValidity::NotEvaluated)
    {
        HardValidity::NotEvaluated
    } else if components
        .iter()
        .any(|component| component.hard_validity == HardValidity::Invalid)
    {
        HardValidity::Invalid
    } else {
        HardValidity::Valid
    }
}

pub(super) fn aggregate_secondary_status(
    problem: &Problem,
    category: ResidualCategory,
    reports: &[PrioritySolveReport],
    prerequisite: Option<SecondaryStatus>,
) -> SecondaryStatus {
    let requested = problem
        .residuals
        .iter()
        .any(|(_, residual)| residual.category() == category);
    if !requested {
        return SecondaryStatus::NotRequested;
    }
    if prerequisite.is_some_and(|status| {
        !matches!(
            status,
            SecondaryStatus::NotRequested | SecondaryStatus::Optimal | SecondaryStatus::Acceptable
        )
    }) {
        return SecondaryStatus::EvaluationFailure;
    }
    reports
        .iter()
        .filter(|report| report.category == category)
        .map(|report| report.status)
        .max_by_key(|status| secondary_status_severity(*status))
        .unwrap_or(SecondaryStatus::EvaluationFailure)
}

pub(super) const fn secondary_status_severity(status: SecondaryStatus) -> u8 {
    match status {
        SecondaryStatus::NotRequested => 0,
        SecondaryStatus::Optimal => 1,
        SecondaryStatus::Acceptable => 2,
        SecondaryStatus::Stalled => 3,
        SecondaryStatus::IterationLimit => 4,
        SecondaryStatus::EvaluationFailure => 5,
    }
}

pub(super) fn worse_termination(
    first: SolveTermination,
    second: SolveTermination,
) -> SolveTermination {
    if termination_severity(second) > termination_severity(first) {
        second
    } else {
        first
    }
}

pub(super) const fn termination_severity(termination: SolveTermination) -> u8 {
    match termination {
        SolveTermination::Converged => 0,
        SolveTermination::IterationLimit => 1,
        SolveTermination::Stalled => 2,
        SolveTermination::InvalidGeometry => 3,
        SolveTermination::NumericalFailure => 4,
    }
}

pub(super) fn positive_finite(value: f64, field: &'static str) -> Result<(), CoreError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        invalid_config(field, "must be positive and finite")
    }
}

pub(super) fn invalid_config<T>(
    field: &'static str,
    message: &'static str,
) -> Result<T, CoreError> {
    Err(CoreError::InvalidSolverConfig { field, message })
}

pub(super) fn push_unique<T: Copy + PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::super::priority::{
        CurvatureStencilPolicy, EqualityProjector, MultiScaleCurvature,
        cached_temporary_attained_cost, controlled_rank_kernel,
        curvature_stencil_coordinate_radius, multi_scale_curvature,
    };
    use super::*;
    use crate::linearization::IndexedJacobianEntry;
    use crate::{
        CONTROLLED_DENSE_KERNEL_MAX_DIMENSION, CancellationToken, OperationControl,
        OperationLimits, OperationOutcome, OperationStopReason, PrioritySolveBackend,
        PrioritySolveScope,
    };

    #[test]
    fn defaults_are_strict_but_finite() {
        SolverConfig::default().validate().unwrap();
    }

    #[test]
    fn controlled_dense_kernels_reject_cap_plus_one_before_invocation() {
        let cap = CONTROLLED_DENSE_KERNEL_MAX_DIMENSION;
        let mut controller = OperationController::new(OperationControl::default());
        let mut calls = 0;
        assert_eq!(
            controlled_dense_factorization(cap, cap, Some(&mut controller), || {
                calls += 1;
                Some(7)
            }),
            Some(7)
        );
        assert_eq!(calls, 1);
        assert_eq!(controller.report().consumed.dense_kernel_rows, cap);
        assert_eq!(controller.report().consumed.dense_kernel_columns, cap);

        let mut controller = OperationController::new(OperationControl::default());
        let mut calls = 0;
        assert_eq!(
            controlled_dense_factorization(cap + 1, cap, Some(&mut controller), || {
                calls += 1;
                Some(())
            }),
            None
        );
        assert_eq!(calls, 0);
        assert_eq!(controller.report().consumed.factorizations, 0);
        assert_eq!(
            controller.report().stopping_reason,
            Some(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DenseKernelRows,
                checkpoint: OperationCheckpoint::BeforeFactorization,
            })
        );
        assert!(matches!(
            controller.outcome(()),
            OperationOutcome::WorkExhausted { .. }
        ));

        let mut controller = OperationController::new(OperationControl::default());
        let mut calls = 0;
        assert_eq!(
            controlled_rank_kernel(cap, cap + 1, Some(&mut controller), || {
                calls += 1;
                Some(())
            }),
            None
        );
        assert_eq!(calls, 0);
        assert_eq!(controller.report().consumed.rank_kernels, 0);
        assert_eq!(
            controller.report().stopping_reason,
            Some(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DenseKernelColumns,
                checkpoint: OperationCheckpoint::BeforeRankKernel,
            })
        );

        let mut limits = OperationLimits::unlimited();
        limits.dense_kernel_columns = 12;
        let mut controller =
            OperationController::new(OperationControl::new(CancellationToken::default(), limits));
        let mut calls = 0;
        assert_eq!(
            controlled_dense_factorization(12, 13, Some(&mut controller), || {
                calls += 1;
                Some(())
            }),
            None
        );
        assert_eq!(calls, 0);
        assert_eq!(controller.report().configured.dense_kernel_columns, 12);
    }

    #[test]
    #[ignore = "manual reproducible non-interruptible kernel-boundary measurement"]
    fn measure_native_dense_kernel_boundary_latency() {
        const DIMENSION: usize = 256;
        const RUNS: usize = 20;
        let matrix = DMatrix::from_fn(DIMENSION, DIMENSION, |row, column| {
            if row == column {
                2.0
            } else {
                let pattern = (row.wrapping_mul(31).wrapping_add(column * 17)) % 19;
                (f64::from(u32::try_from(pattern).unwrap()) - 9.0) * 1.0e-4
            }
        });
        let right_hand_side = DVector::from_element(DIMENSION, 1.0);
        let mut factorization_maximum = std::time::Duration::ZERO;
        let mut rank_maximum = std::time::Duration::ZERO;

        for _ in 0..RUNS {
            let mut controller = OperationController::new(OperationControl::default());
            let started = std::time::Instant::now();
            let solution =
                controlled_dense_factorization(DIMENSION, DIMENSION, Some(&mut controller), || {
                    matrix.clone().qr().solve(&right_hand_side)
                });
            factorization_maximum = factorization_maximum.max(started.elapsed());
            assert!(solution.is_some());
            assert_eq!(controller.report().consumed.factorizations, 1);

            let mut controller = OperationController::new(OperationControl::default());
            let started = std::time::Instant::now();
            let diagnostics = controlled_rank_diagnostics(&matrix, 1.0e-10, Some(&mut controller));
            rank_maximum = rank_maximum.max(started.elapsed());
            assert!(diagnostics.is_some());
            assert_eq!(controller.report().consumed.rank_kernels, 1);
        }

        println!(
            "{RUNS} runs at {DIMENSION}x{DIMENSION}: factorization maximum {factorization_maximum:?}; rank maximum {rank_maximum:?}"
        );
    }

    #[test]
    fn singleton_curvature_preserves_resolved_coarse_descent_when_fine_scale_is_flat() {
        let sample = |delta: &DVector<f64>| {
            let value = delta[0];
            Some(0.5 + 1.0e-8 * value * value - 0.1 * value.powi(4))
        };
        let singleton = multi_scale_curvature(
            1,
            0.5,
            1.0e-3,
            SolverConfig::default(),
            CurvatureStencilPolicy::SingletonAnyResolvedScale,
            sample,
        );
        assert!(matches!(singleton, Some(MultiScaleCurvature::Negative(_))));

        let coupled = multi_scale_curvature(
            1,
            0.5,
            1.0e-3,
            SolverConfig::default(),
            CurvatureStencilPolicy::ConsistentFineScales,
            sample,
        );
        assert!(matches!(coupled, Some(MultiScaleCurvature::Inconclusive)));
    }

    #[test]
    fn curvature_stencil_radius_matches_axis_and_mixed_coordinate_probes() {
        let step = 1.0e-3;
        assert_eq!(
            curvature_stencil_coordinate_radius([1.0].into_iter(), step),
            Some(step)
        );
        let diagonal = 1.0 / 2.0_f64.sqrt();
        let mixed =
            curvature_stencil_coordinate_radius([diagonal, diagonal].into_iter(), step).unwrap();
        assert!(
            (mixed - 2.0_f64.sqrt() * step).abs() <= f64::EPSILON * step,
            "{mixed:e}"
        );
        assert_eq!(
            curvature_stencil_coordinate_radius([1.0, -3.0, 2.0].into_iter(), step),
            Some(5.0 * step)
        );
    }

    #[test]
    fn auto_sparse_crossover_enforces_dimensions_nnz_and_density() {
        let system = |rows: usize, columns: usize, nnz: usize| ComponentIndexedSystem {
            residuals: DVector::zeros(rows),
            rows: Vec::new(),
            row_count: rows,
            column_count: columns,
            entries: (0..nnz)
                .map(|position| IndexedJacobianEntry {
                    row: position / columns,
                    column: position % columns,
                    value: 1.0,
                })
                .collect(),
            sparsity_signature: 0,
        };
        let all_columns = |columns: usize| (0..columns).collect::<Vec<_>>();

        assert!(auto_prefers_sparse(
            &system(256, 256, 256),
            &all_columns(256)
        ));
        assert!(!auto_prefers_sparse(
            &system(255, 256, 256),
            &all_columns(256)
        ));
        assert!(!auto_prefers_sparse(
            &system(256, 255, 256),
            &all_columns(255)
        ));
        assert!(!auto_prefers_sparse(
            &system(256, 256, 255),
            &all_columns(256)
        ));
        assert!(auto_prefers_sparse(
            &system(256, 256, 512),
            &all_columns(256)
        ));
        assert!(!auto_prefers_sparse(
            &system(256, 256, 513),
            &all_columns(256)
        ));
    }

    #[test]
    fn malformed_sparse_input_falls_back_to_a_validated_dense_step_with_typed_evidence() {
        let malformed = ComponentIndexedSystem {
            residuals: DVector::from_vec(vec![-2.0]),
            rows: Vec::new(),
            row_count: 1,
            column_count: 1,
            entries: vec![IndexedJacobianEntry {
                row: 1,
                column: 0,
                value: 1.0,
            }],
            sparsity_signature: 0,
        };
        let jacobian = DMatrix::from_element(1, 1, 1.0);
        let residual = DVector::from_vec(vec![-2.0]);
        let problem = Problem::new();
        let mut evidence = BackendEvidence::new(LinearSolveBackendPolicy::SparsePreferred);
        let step = lm_step_with_backend(
            &problem,
            Some(&malformed),
            &jacobian,
            &residual,
            &[0],
            1.0e-3,
            1.0e-12,
            1.0e-10,
            LinearSolveBackendPolicy::SparsePreferred,
            &mut evidence,
            None,
        )
        .unwrap();

        assert!(step[0].is_finite());
        assert!(
            residual_cost(&(&residual + jacobian * &step)).unwrap()
                < residual_cost(&residual).unwrap()
        );
        assert_eq!(evidence.actual, Some(LinearSolveBackend::Dense));
        assert_eq!(
            evidence.fallback_reason,
            Some(SparseFallbackReason::ConstructionFailure)
        );
    }

    #[test]
    fn cached_temporary_protection_uses_the_attained_cost_not_the_returned_cost() {
        let report = PrioritySolveReport {
            group_index: 0,
            component_index: Some(0),
            component_indices: vec![0],
            scope: PrioritySolveScope::Movable,
            backend: Some(PrioritySolveBackend::DenseNullspace),
            largest_explicit_nullspace_block_rows: 1,
            protected_temporary: Vec::new(),
            category: ResidualCategory::Temporary,
            iterations: 0,
            initial_cost: Some(1.0),
            final_cost: Some(0.25 + 1.0e-12),
            attained_temporary_cost: Some(0.25),
            termination: SolveTermination::Converged,
            status: SecondaryStatus::Acceptable,
        };

        assert_eq!(cached_temporary_attained_cost(&report), Some(0.25));
    }

    #[test]
    fn equality_projector_uses_the_authoritative_unsquared_rank_threshold() {
        let rows = DMatrix::from_diagonal(&DVector::from_vec(vec![1.0, 1.0e-7]));
        let projector = EqualityProjector::new(rows, 1.0e-10, None).unwrap();
        let projected = projector
            .project(&DVector::from_vec(vec![1.0, 1.0]), 1.0e-12)
            .unwrap();

        assert!(projected.norm() <= 1.0e-12, "{projected:?}");
    }

    #[test]
    fn singular_dense_system_falls_back_from_qr_to_svd() {
        let matrix = DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, 0.0]);
        let right_hand_side = DVector::from_vec(vec![2.0, 0.0]);
        let (solution, method) =
            solve_dense_least_squares(&matrix, &right_hand_side, None).unwrap();
        assert_eq!(method, LinearSolveMethod::Svd);
        assert!((solution[0] - 2.0).abs() <= f64::EPSILON);
        assert!(solution[1].abs() <= f64::EPSILON);
    }

    #[test]
    fn rank_one_cursor_projection_is_bounded_minimum_norm_and_kkt_certified() {
        // A two-coordinate cursor projected through one instantaneous mechanism
        // freedom produces two dependent target rows. This is the exact reduced
        // system observed while dragging pantograph guide B off its circle.
        let matrix = DMatrix::from_row_slice(
            2,
            2,
            &[
                -0.197_634_476_267_791_96,
                -0.632_455_532_033_676_2,
                0.065_878_158_755_930_41,
                0.210_818_510_677_892_06,
            ],
        );
        let right_hand_side = DVector::from_vec(vec![0.04, 0.0]);
        let mut controller = OperationController::new(OperationControl::default());
        let solution = solve_rank_aware_least_squares(
            &matrix,
            &right_hand_side,
            SolverConfig::default().rank_relative_tolerance,
            Some(&mut controller),
        )
        .unwrap();

        assert!(solution.iter().all(|value| value.is_finite()));
        assert!(solution.norm() <= 0.1, "{solution:?}");
        let model_residual = &matrix * &solution - &right_hand_side;
        let normal_residual = matrix.transpose() * &model_residual;
        assert!(normal_residual.norm() <= 1.0e-12, "{normal_residual:?}");
        let first_row = matrix.row(0);
        let null_direction = DVector::from_vec(vec![first_row[1], -first_row[0]]);
        assert!(
            solution.dot(&null_direction).abs()
                <= 1.0e-12 * solution.norm().max(1.0) * null_direction.norm().max(1.0),
            "{solution:?}"
        );
        assert_eq!(controller.report().consumed.factorizations, 1);
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn wide_rank_deficient_svd_refines_the_normal_equation_residual() {
        let matrix = DMatrix::from_row_slice(
            2,
            6,
            &[
                0.06399878875589683,
                0.0,
                0.0,
                0.0027820941212632405,
                -0.03072473036699153,
                0.04005075120820309,
                -0.6983875185918699,
                0.0,
                0.0,
                -0.030359634105717875,
                0.3352839736116159,
                -0.43705428333532453,
            ],
        );
        let right_hand_side =
            DVector::from_vec(vec![-0.006487831513659573, -0.0005734453448315077]);
        let (solution, method) =
            solve_dense_least_squares(&matrix, &right_hand_side, None).unwrap();
        assert_eq!(method, LinearSolveMethod::Svd);
        let normal_residual = matrix.transpose() * (&matrix * &solution - &right_hand_side);
        assert!(
            normal_residual.norm() <= 8.025757393464364e-11,
            "normal-equation residual: {:e}",
            normal_residual.norm()
        );
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn rank_aware_wide_reduced_solve_refines_to_kkt_stationarity() {
        let matrix = DMatrix::from_row_slice(
            2,
            6,
            &[
                0.06399878875589683,
                0.0,
                0.0,
                0.0027820941212632405,
                -0.03072473036699153,
                0.04005075120820309,
                -0.6983875185918699,
                0.0,
                0.0,
                -0.030359634105717875,
                0.3352839736116159,
                -0.43705428333532453,
            ],
        );
        let right_hand_side =
            DVector::from_vec(vec![-0.006487831513659573, -0.0005734453448315077]);
        let relative_tolerance = 1.0e-10;
        let solution =
            solve_rank_aware_least_squares(&matrix, &right_hand_side, relative_tolerance, None)
                .unwrap();
        let residual = &matrix * &solution - &right_hand_side;
        let gradient = matrix.transpose() * &residual;
        let model_residuals = -&right_hand_side;
        let tolerance =
            kkt_gradient_tolerance(&matrix, &model_residuals, &solution, 0.0, 1.0e-10).unwrap();

        assert!(solution.iter().all(|value| value.is_finite()));
        assert!(residual_cost(&residual).unwrap() < residual_cost(&right_hand_side).unwrap());
        assert!(
            gradient.norm() <= tolerance,
            "rank-aware reduced response did not satisfy first-order stationarity: \
             gradient={:e}, tolerance={tolerance:e}",
            gradient.norm()
        );
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn wide_rank_deficient_svd_refines_without_increasing_the_objective() {
        let matrix = DMatrix::from_row_slice(
            2,
            6,
            &[
                0.16534448628478354,
                0.0,
                0.0,
                0.0004625611690347226,
                -0.003704744068830011,
                0.025314667355545502,
                -0.8668417318768023,
                0.0,
                0.0,
                -0.0024250420069924306,
                0.01942264200605787,
                -0.13271570516461637,
            ],
        );
        let right_hand_side = DVector::from_vec(vec![-0.018425042972668774, 0.01555989854196734]);
        let svd = matrix.clone().svd(true, true);
        let largest = svd.singular_values.iter().copied().fold(0.0_f64, f64::max);
        let initial = svd
            .solve(&right_hand_side, f64::EPSILON * 6.0 * largest)
            .unwrap();
        let initial_residual = (&matrix * initial - &right_hand_side).norm();
        let (solution, method) =
            solve_dense_least_squares(&matrix, &right_hand_side, None).unwrap();
        assert_eq!(method, LinearSolveMethod::Svd);
        let normal_residual = matrix.transpose() * (&matrix * &solution - &right_hand_side);
        let residual = (&matrix * solution - right_hand_side).norm();
        assert!(
            normal_residual.norm() <= 7.995668163103057e-11,
            "normal-equation residual: {:e}",
            normal_residual.norm()
        );
        assert!(residual <= initial_residual);
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn rank_deficient_reduced_step_ignores_an_independent_passive_direction() {
        let jacobian = DMatrix::from_column_slice(
            2,
            2,
            &[
                -0.19763447626779196,
                0.06587815875593041,
                -0.6324555320336762,
                0.21081851067789206,
            ],
        );
        let residuals = DVector::from_vec(vec![-0.02365457882101376, 0.008950957506131462]);
        let constraints = Vec::new();
        let working = Vec::new();
        let relative_tolerance = 1.0e-10;
        let normalized_step_tolerance = 1.0e-10;

        let step = solve_active_reduced_least_squares(
            &jacobian,
            &residuals,
            &constraints,
            &working,
            relative_tolerance,
            None,
        )
        .unwrap();
        let model_residuals = &jacobian * &step + &residuals;
        let gradient = jacobian.transpose() * &model_residuals;
        let tolerance =
            kkt_gradient_tolerance(&jacobian, &residuals, &step, 0.0, normalized_step_tolerance)
                .unwrap();

        assert!(
            step.norm() < 1.0,
            "the passive rank-null direction received a material response: {step:?}"
        );
        assert!(residual_cost(&model_residuals).unwrap() < residual_cost(&residuals).unwrap());
        assert!(
            working_set_kkt(&gradient, &constraints, &working, tolerance, None).is_ok(),
            "rank-aware reduced response did not satisfy first-order stationarity: {gradient:?}"
        );
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn constrained_roundoff_snap_preserves_material_step_coordinates() {
        let mut step = DVector::from_vec(vec![1.0e-3, 1.2068573960262244e-19]);
        let bounds = [
            NormalizedStepBound {
                lower: f64::NEG_INFINITY,
                upper: f64::INFINITY,
            },
            NormalizedStepBound {
                lower: 0.0,
                upper: 0.0,
            },
        ];
        snap_constrained_roundoff(&mut step, &bounds).unwrap();
        assert_eq!(step[0].to_bits(), 1.0e-3f64.to_bits());
        assert_eq!(step[1].to_bits(), 0.0f64.to_bits());
    }
}
