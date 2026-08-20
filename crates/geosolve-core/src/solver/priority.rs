use super::{
    ActiveLayout, ComponentExecution, ComponentIterationObjective, ComponentLinearization,
    CoreError, DMatrix, DVector, EliminationPlan, HardSystem, MAX_PRIORITY_LINE_SEARCH_STEPS,
    NormalizedStepBound, OperationCheckpoint, OperationController, OperationWorkCounter,
    PRIORITY_COST_RESOLUTION_FACTOR, PRIORITY_HESSIAN_NORMALIZED_STEP,
    PRIORITY_REPROJECTION_TOLERANCE, PRIORITY_ZERO_COST_ROUNDOFF, PROJECTED_CGLS_MIN_NULLITY,
    PrioritySolveBackend, PrioritySolveReport, PrioritySolveScope, Problem,
    ProtectedTemporaryReport, RankDiagnostics, ReducedCriticalCone, ReducedStepBound,
    ResidualCategory, ResidualId, SecondaryStatus, SolveComponent, SolveReport, SolveTermination,
    SolveTrace, SolverConfig, VariableState, WorkingBound, WorkingSetKkt, active_layout,
    append_component_trace, apply_normalized_step, component_dense_system,
    composite_tangent_layout, constrained_nullspace_step, constraint_satisfied, error_termination,
    first_linear_bound_event, independent_initial_working_set, iterate_component,
    iterate_component_objective, kkt_gradient_tolerance, limit_block_steps, limit_operator_step,
    limit_step_to_bound_events, maximum_block_step, merge_actual_backend, normalized_step_bounds,
    operator_full_step_satisfies_bounds, operator_step_is_within_bounds, push_unique,
    rank_diagnostics, rank_thresholds, residual_cost, secondary_status, snap_constrained_roundoff,
    solve_active_reduced_least_squares, solve_rank_aware_least_squares, stable_norm, state_value,
    step_is_within_bounds, validate_component, working_constraint_is_independent, working_set_kkt,
    worse_termination,
};

#[derive(Debug)]
pub(super) struct PriorityPassOutcome {
    pub(super) state: VariableState,
    pub(super) reports: Vec<PriorityReportRecord>,
    pub(super) component_participated: Vec<bool>,
    pub(super) component_state_changed: Vec<bool>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SecondaryResidualPreservation {
    pub(crate) preserved: bool,
    pub(crate) maximum_row_error: f64,
    pub(crate) tolerance: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExactSecondaryPreservation {
    pub(crate) temporary: SecondaryResidualPreservation,
    pub(crate) preference: SecondaryResidualPreservation,
}

pub(super) fn merge_exact_certification_execution_provenance(
    fresh: &mut SolveReport,
    retained: &SolveReport,
) {
    for component in &mut fresh.component_solves {
        let Some(prior) = retained.component_solves.iter().find(|prior| {
            prior.component_index == component.component_index
                && prior.pattern_signature == component.pattern_signature
                && prior.sparsity_signature == component.sparsity_signature
        }) else {
            continue;
        };
        component.actual_backend = prior.actual_backend;
        component.symbolic_analysis_reused = prior.symbolic_analysis_reused;
        component.symbolic_analysis_reuse_count = prior.symbolic_analysis_reuse_count;
        component.sparse_fallback_reason = prior.sparse_fallback_reason;
        component.reused = prior.reused;
        component.secondary_participated |= prior.secondary_participated;
        component.state_changed_by_secondary = prior.state_changed_by_secondary;
        component.iterations = prior.iterations;
        component.hard_termination = prior.hard_termination;
        component.trace.clone_from(&prior.trace);
    }
    for priority in &mut fresh.priority_solves {
        let Some(prior) = retained.priority_solves.iter().find(|prior| {
            prior.group_index == priority.group_index
                && prior.component_indices == priority.component_indices
                && prior.scope == priority.scope
                && prior.category == priority.category
        }) else {
            continue;
        };
        priority.backend = prior.backend;
        priority.largest_explicit_nullspace_block_rows =
            prior.largest_explicit_nullspace_block_rows;
        priority.iterations = prior.iterations;
    }
    fresh.actual_backend = fresh
        .component_solves
        .iter()
        .fold(None, |aggregate, component| {
            merge_actual_backend(aggregate, component.actual_backend)
        });
    fresh.symbolic_analysis_reuse_count = fresh
        .component_solves
        .iter()
        .map(|component| component.symbolic_analysis_reuse_count)
        .sum();
    fresh.symbolic_analysis_reused = fresh.symbolic_analysis_reuse_count > 0;
    fresh.sparse_fallback_reason = fresh
        .component_solves
        .iter()
        .find_map(|component| component.sparse_fallback_reason);
    fresh.hard_termination = fresh
        .component_solves
        .iter()
        .map(|component| component.hard_termination)
        .fold(SolveTermination::Converged, worse_termination);
    let mut trace = SolveTrace::default();
    for component in &fresh.component_solves {
        append_component_trace(&mut trace, &component.trace);
    }
    fresh.trace = trace;
    fresh.iterations = fresh
        .component_solves
        .iter()
        .map(|component| component.iterations)
        .sum::<usize>()
        .saturating_add(
            fresh
                .priority_solves
                .iter()
                .map(|priority| priority.iterations)
                .sum::<usize>(),
        );
}

#[derive(Debug)]
pub(super) struct PriorityReportRecord {
    pub(super) report: PrioritySolveReport,
    pub(super) residual_ids: Vec<ResidualId>,
}

#[derive(Debug)]
pub(super) struct PriorityGroup {
    pub(super) group_index: usize,
    pub(super) component_indices: Vec<usize>,
    pub(super) residual_ids: Vec<ResidualId>,
    pub(super) protected_temporary_groups: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct PriorityCategoryPlan {
    pub(super) movable: Vec<PriorityGroup>,
    pub(super) fixed: Vec<ResidualId>,
    pub(super) invalid: Vec<ResidualId>,
}

#[derive(Debug)]
pub(super) struct PriorityPlan {
    pub(super) temporary: PriorityCategoryPlan,
    pub(super) preference: PriorityCategoryPlan,
}

#[derive(Debug)]
pub(super) struct PriorityIncidence {
    pub(super) residual_id: ResidualId,
    pub(super) component_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct TemporaryLevel {
    pub(super) group_index: usize,
    pub(super) component_indices: Vec<usize>,
    pub(super) residual_ids: Vec<ResidualId>,
    pub(super) attained_cost: f64,
}

#[derive(Debug)]
pub(super) struct DisjointSet {
    pub(super) parents: Vec<usize>,
}

impl DisjointSet {
    fn new(len: usize) -> Self {
        Self {
            parents: (0..len).collect(),
        }
    }

    fn root(&mut self, index: usize) -> usize {
        let parent = self.parents[index];
        if parent == index {
            index
        } else {
            let root = self.root(parent);
            self.parents[index] = root;
            root
        }
    }

    fn union(&mut self, first: usize, second: usize) {
        let first_root = self.root(first);
        let second_root = self.root(second);
        if first_root == second_root {
            return;
        }
        let (root, child) = if first_root < second_root {
            (first_root, second_root)
        } else {
            (second_root, first_root)
        };
        self.parents[child] = root;
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn optimize_priorities(
    problem: &Problem,
    plan: &EliminationPlan,
    mut state: VariableState,
    config: SolverConfig,
    executions: &[ComponentExecution],
    prior_report: Option<&SolveReport>,
    dirty_hierarchy_residuals: &[ResidualId],
    mut control: Option<&mut OperationController>,
) -> Option<PriorityPassOutcome> {
    let priority_plan = build_priority_plan(problem, plan);
    let hard_state = state.clone();
    let mut reports = Vec::new();
    let mut component_participated = vec![false; plan.components.len()];
    let mut temporary_levels = vec![None; priority_plan.temporary.movable.len()];
    let mut temporary_reran = vec![false; priority_plan.temporary.movable.len()];

    for group in &priority_plan.temporary.movable {
        let dirty = priority_group_is_dirty(group, executions, dirty_hierarchy_residuals, &[]);
        if !dirty {
            let report = cached_priority_report(prior_report, group, ResidualCategory::Temporary);
            if report.termination == SolveTermination::Converged
                && let Some(attained_cost) = cached_temporary_attained_cost(&report)
            {
                temporary_levels[group.group_index] = Some(TemporaryLevel {
                    group_index: group.group_index,
                    component_indices: group.component_indices.clone(),
                    residual_ids: group.residual_ids.clone(),
                    attained_cost,
                });
            }
            reports.push(PriorityReportRecord {
                report,
                residual_ids: group.residual_ids.clone(),
            });
            continue;
        }
        for &component_index in &group.component_indices {
            component_participated[component_index] = true;
        }
        temporary_reran[group.group_index] = true;
        let mut outcome = optimize_priority_group(
            problem,
            plan,
            group,
            state,
            ResidualCategory::Temporary,
            &[],
            config,
            control.as_deref_mut(),
        )?;
        state = outcome.state;
        if outcome.report.termination == SolveTermination::Converged
            && let Some(attained_cost) = outcome.report.final_cost
        {
            outcome.report.attained_temporary_cost = Some(attained_cost);
            temporary_levels[group.group_index] = Some(TemporaryLevel {
                group_index: group.group_index,
                component_indices: group.component_indices.clone(),
                residual_ids: group.residual_ids.clone(),
                attained_cost,
            });
        }
        reports.push(PriorityReportRecord {
            report: outcome.report,
            residual_ids: group.residual_ids.clone(),
        });
    }

    let mut next_temporary_group = priority_plan.temporary.movable.len();
    if !priority_plan.temporary.fixed.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &state,
            next_temporary_group,
            ResidualCategory::Temporary,
            &priority_plan.temporary.fixed,
            PrioritySolveScope::Fixed,
            SolveTermination::Converged,
        ));
        next_temporary_group += 1;
    }
    if !priority_plan.temporary.invalid.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &hard_state,
            next_temporary_group,
            ResidualCategory::Temporary,
            &priority_plan.temporary.invalid,
            PrioritySolveScope::InvalidIncidence,
            SolveTermination::NumericalFailure,
        ));
    }

    for group in &priority_plan.preference.movable {
        let protected = group
            .protected_temporary_groups
            .iter()
            .map(|&index| temporary_levels[index].clone())
            .collect::<Option<Vec<_>>>();
        let dirty = priority_group_is_dirty(
            group,
            executions,
            dirty_hierarchy_residuals,
            &group
                .protected_temporary_groups
                .iter()
                .copied()
                .filter(|&index| temporary_reran[index])
                .collect::<Vec<_>>(),
        );
        if protected.is_some() && !dirty {
            reports.push(PriorityReportRecord {
                report: cached_priority_report(prior_report, group, ResidualCategory::Preference),
                residual_ids: group.residual_ids.clone(),
            });
            continue;
        }
        for &component_index in &group.component_indices {
            component_participated[component_index] = true;
        }
        let outcome = if let Some(protected) = protected {
            optimize_priority_group(
                problem,
                plan,
                group,
                state,
                ResidualCategory::Preference,
                &protected,
                config,
                control.as_deref_mut(),
            )?
        } else {
            priority_group_failure_report(
                group,
                state,
                ResidualCategory::Preference,
                &[],
                SolveTermination::NumericalFailure,
            )
        };
        state = outcome.state;
        reports.push(PriorityReportRecord {
            report: outcome.report,
            residual_ids: group.residual_ids.clone(),
        });
    }

    let mut next_preference_group = priority_plan.preference.movable.len();
    if !priority_plan.preference.fixed.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &state,
            next_preference_group,
            ResidualCategory::Preference,
            &priority_plan.preference.fixed,
            PrioritySolveScope::Fixed,
            SolveTermination::Converged,
        ));
        next_preference_group += 1;
    }
    if !priority_plan.preference.invalid.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &state,
            next_preference_group,
            ResidualCategory::Preference,
            &priority_plan.preference.invalid,
            PrioritySolveScope::InvalidIncidence,
            SolveTermination::NumericalFailure,
        ));
    }

    refresh_priority_final_costs(
        problem,
        &state,
        &priority_plan.temporary.movable,
        &mut reports,
        config,
    );
    let component_state_changed = plan
        .components
        .iter()
        .map(|component| component_state_changed(component, &hard_state, &state))
        .collect();
    Some(PriorityPassOutcome {
        state,
        reports,
        component_participated,
        component_state_changed,
    })
}

#[allow(clippy::too_many_lines)]
pub(super) fn certify_current_priorities(
    problem: &Problem,
    retained: Option<(&Problem, &SolveReport)>,
    plan: &EliminationPlan,
    state: &VariableState,
    config: SolverConfig,
    controller: &mut OperationController,
) -> Result<Option<(PriorityPassOutcome, ExactSecondaryPreservation)>, CoreError> {
    let priority_plan = build_priority_plan(problem, plan);
    let unchanged = SecondaryResidualPreservation {
        preserved: true,
        maximum_row_error: 0.0,
        tolerance: residual_target_row_tolerance(config),
    };
    let secondary_preservation = match retained {
        Some((retained, _)) => {
            let temporary_ids = priority_category_residual_ids(&priority_plan.temporary);
            let preference_ids = priority_category_residual_ids(&priority_plan.preference);
            let retained_group_linearizations = priority_plan
                .temporary
                .movable
                .iter()
                .chain(&priority_plan.preference.movable)
                .map(|group| group.residual_ids.len())
                .sum::<usize>();
            let protected_temporary_linearizations = priority_plan
                .preference
                .movable
                .iter()
                .flat_map(|group| group.protected_temporary_groups.iter().copied())
                .filter_map(|index| priority_plan.temporary.movable.get(index))
                .map(|group| group.residual_ids.len())
                .sum::<usize>();
            let comparison_work = temporary_ids
                .len()
                .saturating_add(preference_ids.len())
                .saturating_mul(2)
                .saturating_add(retained_group_linearizations)
                .saturating_add(protected_temporary_linearizations);
            if controller
                .charge(
                    OperationWorkCounter::ComponentLinearizations,
                    comparison_work,
                    OperationCheckpoint::ComponentBoundary,
                )
                .is_err()
            {
                return Ok(None);
            }
            ExactSecondaryPreservation {
                temporary: category_residual_preservation(
                    problem,
                    retained,
                    state,
                    ResidualCategory::Temporary,
                    &temporary_ids,
                    config,
                )?,
                preference: category_residual_preservation(
                    problem,
                    retained,
                    state,
                    ResidualCategory::Preference,
                    &preference_ids,
                    config,
                )?,
            }
        }
        None => ExactSecondaryPreservation {
            temporary: unchanged,
            preference: unchanged,
        },
    };
    let mut reports = Vec::new();
    let mut component_participated = vec![false; plan.components.len()];
    let mut temporary_levels = vec![None; priority_plan.temporary.movable.len()];

    for group in &priority_plan.temporary.movable {
        if !charge_priority_certification(controller, &group.residual_ids) {
            return Ok(None);
        }
        for &component_index in &group.component_indices {
            component_participated[component_index] = true;
        }
        let record = match retained {
            Some((retained_problem, retained_report)) => certify_preserved_priority_group(
                problem,
                retained_problem,
                retained_report,
                state,
                group,
                ResidualCategory::Temporary,
                &[],
                secondary_preservation.temporary.preserved,
                secondary_preservation.temporary.preserved,
                config,
            ),
            None => certify_movable_priority(
                problem,
                state,
                group,
                ResidualCategory::Temporary,
                &[],
                config,
            ),
        };
        if record.report.termination == SolveTermination::Converged
            && matches!(
                record.report.status,
                SecondaryStatus::Optimal | SecondaryStatus::Acceptable
            )
            && let Some(attained_cost) = record.report.attained_temporary_cost
        {
            temporary_levels[group.group_index] = Some(TemporaryLevel {
                group_index: group.group_index,
                component_indices: group.component_indices.clone(),
                residual_ids: group.residual_ids.clone(),
                attained_cost,
            });
        }
        reports.push(record);
    }

    let mut next_temporary_group = priority_plan.temporary.movable.len();
    if !priority_plan.temporary.fixed.is_empty() {
        if !charge_priority_certification(controller, &priority_plan.temporary.fixed) {
            return Ok(None);
        }
        reports.push(evaluate_nonmoving_priority(
            problem,
            state,
            next_temporary_group,
            ResidualCategory::Temporary,
            &priority_plan.temporary.fixed,
            PrioritySolveScope::Fixed,
            SolveTermination::Converged,
        ));
        next_temporary_group += 1;
    }
    if !priority_plan.temporary.invalid.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            state,
            next_temporary_group,
            ResidualCategory::Temporary,
            &priority_plan.temporary.invalid,
            PrioritySolveScope::InvalidIncidence,
            SolveTermination::NumericalFailure,
        ));
    }

    for group in &priority_plan.preference.movable {
        if !charge_priority_certification(controller, &group.residual_ids) {
            return Ok(None);
        }
        for &component_index in &group.component_indices {
            component_participated[component_index] = true;
        }
        let protected = group
            .protected_temporary_groups
            .iter()
            .map(|&index| temporary_levels.get(index)?.clone())
            .collect::<Option<Vec<_>>>();
        reports.push(if let Some(protected) = protected {
            match retained {
                Some((retained_problem, retained_report)) => certify_preserved_priority_group(
                    problem,
                    retained_problem,
                    retained_report,
                    state,
                    group,
                    ResidualCategory::Preference,
                    &protected,
                    secondary_preservation.preference.preserved,
                    secondary_preservation.temporary.preserved,
                    config,
                ),
                None => certify_movable_priority(
                    problem,
                    state,
                    group,
                    ResidualCategory::Preference,
                    &protected,
                    config,
                ),
            }
        } else {
            PriorityReportRecord {
                report: priority_group_failure_report(
                    group,
                    state.clone(),
                    ResidualCategory::Preference,
                    &[],
                    SolveTermination::NumericalFailure,
                )
                .report,
                residual_ids: group.residual_ids.clone(),
            }
        });
    }

    let mut next_preference_group = priority_plan.preference.movable.len();
    if !priority_plan.preference.fixed.is_empty() {
        if !charge_priority_certification(controller, &priority_plan.preference.fixed) {
            return Ok(None);
        }
        reports.push(evaluate_nonmoving_priority(
            problem,
            state,
            next_preference_group,
            ResidualCategory::Preference,
            &priority_plan.preference.fixed,
            PrioritySolveScope::Fixed,
            SolveTermination::Converged,
        ));
        next_preference_group += 1;
    }
    if !priority_plan.preference.invalid.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            state,
            next_preference_group,
            ResidualCategory::Preference,
            &priority_plan.preference.invalid,
            PrioritySolveScope::InvalidIncidence,
            SolveTermination::NumericalFailure,
        ));
    }

    Ok(Some((
        PriorityPassOutcome {
            state: state.clone(),
            reports,
            component_participated,
            component_state_changed: vec![false; plan.components.len()],
        },
        secondary_preservation,
    )))
}

pub(super) fn priority_category_residual_ids(category: &PriorityCategoryPlan) -> Vec<ResidualId> {
    let mut residual_ids = Vec::new();
    for residual_id in category
        .movable
        .iter()
        .flat_map(|group| group.residual_ids.iter().copied())
        .chain(category.fixed.iter().copied())
        .chain(category.invalid.iter().copied())
    {
        if !residual_ids.contains(&residual_id) {
            residual_ids.push(residual_id);
        }
    }
    residual_ids
}

pub(super) fn category_residual_preservation(
    problem: &Problem,
    retained: &Problem,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    config: SolverConfig,
) -> Result<SecondaryResidualPreservation, CoreError> {
    let retained_state = retained.variable_state();
    let before = retained.normalized_category_values_for_residuals(
        &retained_state,
        category,
        residual_ids,
    )?;
    let after = problem.normalized_category_values_for_residuals(state, category, residual_ids)?;
    if before.len() != after.len()
        || before.iter().zip(&after).any(|(before, after)| {
            before.0 != after.0 || before.1 != after.1 || before.2 != after.2
        })
    {
        return Err(CoreError::InvalidAcceptedLinearization {
            context: "exact-state synchronization changed a secondary residual layout",
        });
    }
    let maximum_row_error = before
        .iter()
        .zip(&after)
        .try_fold(0.0_f64, |maximum, (before, after)| {
            let error = (after.3 - before.3).abs();
            error.is_finite().then_some(maximum.max(error))
        })
        .ok_or(CoreError::InvalidAcceptedLinearization {
            context: "exact-state synchronization produced a non-finite secondary residual",
        })?;
    let tolerance = residual_target_row_tolerance(config);
    Ok(SecondaryResidualPreservation {
        preserved: maximum_row_error <= tolerance,
        maximum_row_error,
        tolerance,
    })
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one preserved priority group rebuilds its cost, status, and protected-Temporary evidence atomically"
)]
pub(super) fn certify_preserved_priority_group(
    problem: &Problem,
    retained_problem: &Problem,
    retained_report: &SolveReport,
    state: &VariableState,
    group: &PriorityGroup,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    category_rows_preserved: bool,
    temporary_rows_preserved: bool,
    config: SolverConfig,
) -> PriorityReportRecord {
    let retained_group = retained_report.priority_solves.iter().find(|report| {
        report.group_index == group.group_index
            && report.component_indices == group.component_indices
            && report.category == category
            && report.scope == PrioritySolveScope::Movable
    });
    let retained_state = retained_problem.variable_state();
    let before =
        priority_cost_for_residuals(retained_problem, &retained_state, &group.residual_ids);
    let after = priority_cost_for_residuals(problem, state, &group.residual_ids);
    let mut valid = category_rows_preserved;
    let (initial_cost, final_cost) = if let (Ok(initial), Ok(final_cost)) = (before, after) {
        (Some(initial), Some(final_cost))
    } else {
        valid = false;
        (None, None)
    };
    let mut attained_temporary_cost = None;
    if category == ResidualCategory::Temporary {
        attained_temporary_cost = retained_group.and_then(|report| report.attained_temporary_cost);
        let rows = priority_residual_rows(problem, &group.residual_ids);
        valid &= attained_temporary_cost.is_some_and(|attained| {
            initial_cost
                .is_some_and(|cost| priority_cost_within_vector_limit(cost, attained, rows, config))
                && final_cost.is_some_and(|cost| {
                    priority_cost_within_vector_limit(cost, attained, rows, config)
                })
        });
    }
    valid &= retained_group.is_some_and(|report| {
        !matches!(
            report.status,
            SecondaryStatus::NotRequested | SecondaryStatus::EvaluationFailure
        ) && !matches!(
            report.termination,
            SolveTermination::InvalidGeometry | SolveTermination::NumericalFailure
        )
    });
    let (termination, status) = if !valid {
        (
            SolveTermination::NumericalFailure,
            SecondaryStatus::EvaluationFailure,
        )
    } else if final_cost == Some(0.0) {
        (SolveTermination::Converged, SecondaryStatus::Optimal)
    } else {
        match retained_group
            .expect("validated retained priority group")
            .status
        {
            SecondaryStatus::Optimal | SecondaryStatus::Acceptable => {
                (SolveTermination::Converged, SecondaryStatus::Acceptable)
            }
            SecondaryStatus::Stalled => (SolveTermination::Stalled, SecondaryStatus::Stalled),
            SecondaryStatus::IterationLimit => (
                SolveTermination::IterationLimit,
                SecondaryStatus::IterationLimit,
            ),
            SecondaryStatus::NotRequested | SecondaryStatus::EvaluationFailure => (
                SolveTermination::NumericalFailure,
                SecondaryStatus::EvaluationFailure,
            ),
        }
    };

    let mut protected_temporary = protected_reports(protected);
    for protection in &mut protected_temporary {
        let Some(level) = protected
            .iter()
            .find(|level| level.group_index == protection.group_index)
        else {
            protection.preserved = false;
            continue;
        };
        let rows = priority_residual_rows(problem, &level.residual_ids);
        protection.preservation_tolerance =
            residual_vector_cost_tolerance(level.attained_cost, rows, config);
        protection.final_cost =
            priority_cost_for_residuals(problem, state, &level.residual_ids).ok();
        protection.preserved = temporary_rows_preserved
            && protection.final_cost.is_some_and(|cost| {
                priority_cost_within_vector_limit(cost, level.attained_cost, rows, config)
            });
    }
    let (termination, status) = if protected_temporary
        .iter()
        .all(|protection| protection.preserved)
    {
        (termination, status)
    } else {
        (
            SolveTermination::NumericalFailure,
            SecondaryStatus::EvaluationFailure,
        )
    };
    PriorityReportRecord {
        report: PrioritySolveReport {
            group_index: group.group_index,
            component_index: (group.component_indices.len() == 1)
                .then_some(group.component_indices[0]),
            component_indices: group.component_indices.clone(),
            scope: PrioritySolveScope::Movable,
            backend: None,
            largest_explicit_nullspace_block_rows: 0,
            protected_temporary,
            category,
            iterations: 0,
            initial_cost,
            final_cost,
            attained_temporary_cost: match category {
                ResidualCategory::Temporary => attained_temporary_cost,
                ResidualCategory::Preference => protected.first().map(|level| level.attained_cost),
                ResidualCategory::Hard => None,
            },
            termination,
            status,
        },
        residual_ids: group.residual_ids.clone(),
    }
}

pub(super) fn priority_cost_within_vector_limit(
    candidate: f64,
    attained: f64,
    residual_rows: usize,
    config: SolverConfig,
) -> bool {
    candidate <= attained
        || candidate - attained <= residual_vector_cost_tolerance(attained, residual_rows, config)
}

pub(super) fn charge_priority_certification(
    controller: &mut OperationController,
    residual_ids: &[ResidualId],
) -> bool {
    controller
        .charge(
            OperationWorkCounter::ComponentLinearizations,
            residual_ids.len(),
            OperationCheckpoint::ComponentBoundary,
        )
        .is_ok()
}

pub(super) fn certify_movable_priority(
    problem: &Problem,
    state: &VariableState,
    group: &PriorityGroup,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    config: SolverConfig,
) -> PriorityReportRecord {
    let (cost, termination, status) =
        match priority_cost_for_residuals(problem, state, &group.residual_ids) {
            Ok(cost) if cost == 0.0 => (
                Some(cost),
                SolveTermination::Converged,
                SecondaryStatus::Optimal,
            ),
            Ok(cost) => (
                Some(cost),
                SolveTermination::Stalled,
                SecondaryStatus::Stalled,
            ),
            Err(error) => (
                None,
                error_termination(&error),
                SecondaryStatus::EvaluationFailure,
            ),
        };
    let protected_temporary = protected
        .iter()
        .map(|level| ProtectedTemporaryReport {
            group_index: level.group_index,
            attained_cost: level.attained_cost,
            final_cost: Some(level.attained_cost),
            preservation_tolerance: priority_preservation_tolerance(
                level.attained_cost,
                priority_residual_rows(problem, &level.residual_ids),
                config,
            ),
            preserved: true,
        })
        .collect();
    PriorityReportRecord {
        report: PrioritySolveReport {
            group_index: group.group_index,
            component_index: (group.component_indices.len() == 1)
                .then_some(group.component_indices[0]),
            component_indices: group.component_indices.clone(),
            scope: PrioritySolveScope::Movable,
            backend: None,
            largest_explicit_nullspace_block_rows: 0,
            protected_temporary,
            category,
            iterations: 0,
            initial_cost: cost,
            final_cost: cost,
            attained_temporary_cost: match category {
                ResidualCategory::Temporary => cost,
                ResidualCategory::Preference => protected.first().map(|level| level.attained_cost),
                ResidualCategory::Hard => None,
            },
            termination,
            status,
        },
        residual_ids: group.residual_ids.clone(),
    }
}

pub(super) fn cached_priority_report(
    prior_report: Option<&SolveReport>,
    group: &PriorityGroup,
    category: ResidualCategory,
) -> PrioritySolveReport {
    prior_report
        .and_then(|report| {
            report.priority_solves.iter().find(|priority| {
                priority.group_index == group.group_index
                    && priority.component_indices == group.component_indices
                    && priority.category == category
            })
        })
        .cloned()
        .map(|mut report| {
            report.iterations = 0;
            report
        })
        .unwrap_or(PrioritySolveReport {
            group_index: group.group_index,
            component_index: (group.component_indices.len() == 1)
                .then_some(group.component_indices[0]),
            component_indices: group.component_indices.clone(),
            scope: PrioritySolveScope::Movable,
            backend: None,
            largest_explicit_nullspace_block_rows: 0,
            protected_temporary: Vec::new(),
            category,
            iterations: 0,
            initial_cost: None,
            final_cost: None,
            attained_temporary_cost: None,
            termination: SolveTermination::NumericalFailure,
            status: SecondaryStatus::EvaluationFailure,
        })
}

pub(super) fn cached_temporary_attained_cost(report: &PrioritySolveReport) -> Option<f64> {
    report.attained_temporary_cost.or(report.final_cost)
}

pub(super) fn build_priority_plan(problem: &Problem, plan: &EliminationPlan) -> PriorityPlan {
    let (temporary_incidence, fixed_temporary, invalid_temporary) =
        classify_priority_incidence(problem, plan, ResidualCategory::Temporary);
    let (preference_incidence, fixed_preference, invalid_preference) =
        classify_priority_incidence(problem, plan, ResidualCategory::Preference);
    let mut temporary_dsu = DisjointSet::new(plan.components.len());
    union_priority_hyperedges(&mut temporary_dsu, &temporary_incidence);
    let temporary = priority_groups(&temporary_incidence, &mut temporary_dsu);

    let mut preference_dsu = DisjointSet::new(plan.components.len());
    for group in &temporary {
        union_components(&mut preference_dsu, &group.component_indices);
    }
    union_priority_hyperedges(&mut preference_dsu, &preference_incidence);
    let mut preference = priority_groups(&preference_incidence, &mut preference_dsu);
    for group in &mut preference {
        group.protected_temporary_groups = temporary
            .iter()
            .filter(|temporary_group| {
                temporary_group
                    .component_indices
                    .iter()
                    .any(|component| group.component_indices.contains(component))
            })
            .map(|temporary_group| temporary_group.group_index)
            .collect();
    }
    PriorityPlan {
        temporary: PriorityCategoryPlan {
            movable: temporary,
            fixed: fixed_temporary,
            invalid: invalid_temporary,
        },
        preference: PriorityCategoryPlan {
            movable: preference,
            fixed: fixed_preference,
            invalid: invalid_preference,
        },
    }
}

pub(super) fn classify_priority_incidence(
    problem: &Problem,
    plan: &EliminationPlan,
    selected_category: ResidualCategory,
) -> (Vec<PriorityIncidence>, Vec<ResidualId>, Vec<ResidualId>) {
    let mut movable = Vec::new();
    let mut fixed = Vec::new();
    let mut invalid = Vec::new();
    for (residual_id, residual) in problem.residuals.iter() {
        let category = residual.category();
        if category != selected_category {
            continue;
        }
        let mut components = Vec::new();
        let mut incidence_is_valid = true;
        for &variable_id in residual.incident_variables() {
            let Some(root) = plan.root(variable_id) else {
                incidence_is_valid = false;
                break;
            };
            if let Some(group) = plan.active_groups.iter().find(|group| group.root == root) {
                push_unique(&mut components, group.component_index);
            }
        }
        components.sort_unstable();
        if !incidence_is_valid {
            invalid.push(residual_id);
        } else if components.is_empty() {
            fixed.push(residual_id);
        } else {
            movable.push(PriorityIncidence {
                residual_id,
                component_indices: components,
            });
        }
    }
    (movable, fixed, invalid)
}

pub(super) fn union_priority_hyperedges(dsu: &mut DisjointSet, incidences: &[PriorityIncidence]) {
    for incidence in incidences {
        union_components(dsu, &incidence.component_indices);
    }
}

pub(super) fn union_components(dsu: &mut DisjointSet, component_indices: &[usize]) {
    if let Some((&first, rest)) = component_indices.split_first() {
        for &component_index in rest {
            dsu.union(first, component_index);
        }
    }
}

pub(super) fn priority_groups(
    incidences: &[PriorityIncidence],
    dsu: &mut DisjointSet,
) -> Vec<PriorityGroup> {
    let mut roots = Vec::new();
    let mut groups = Vec::new();
    for incidence in incidences {
        let root = dsu.root(incidence.component_indices[0]);
        let group_index = if let Some(index) = roots.iter().position(|&existing| existing == root) {
            index
        } else {
            roots.push(root);
            groups.push(PriorityGroup {
                group_index: groups.len(),
                component_indices: Vec::new(),
                residual_ids: Vec::new(),
                protected_temporary_groups: Vec::new(),
            });
            groups.len() - 1
        };
        groups[group_index].residual_ids.push(incidence.residual_id);
    }
    for (group_index, &root) in roots.iter().enumerate() {
        for component_index in 0..dsu.parents.len() {
            if dsu.root(component_index) == root {
                groups[group_index].component_indices.push(component_index);
            }
        }
    }
    groups
}

pub(super) fn priority_group_is_dirty(
    group: &PriorityGroup,
    executions: &[ComponentExecution],
    dirty_hierarchy_residuals: &[ResidualId],
    rerun_protected_groups: &[usize],
) -> bool {
    !rerun_protected_groups.is_empty()
        || group
            .component_indices
            .iter()
            .any(|&index| !executions[index].reused)
        || group
            .residual_ids
            .iter()
            .any(|id| dirty_hierarchy_residuals.contains(id))
}

pub(super) fn component_state_changed(
    component: &SolveComponent,
    before: &VariableState,
    after: &VariableState,
) -> bool {
    component
        .variable_ids
        .iter()
        .any(|&variable_id| state_value(before, variable_id) != state_value(after, variable_id))
}

pub(super) fn variable_states_have_exact_values(
    first: &VariableState,
    second: &VariableState,
) -> bool {
    first.values.len() == second.values.len()
        && first.values.iter().zip(&second.values).all(
            |((first_id, first_value), (second_id, second_value))| {
                first_id == second_id
                    && first_value.kind() == second_value.kind()
                    && first_value.ambient_values().len() == second_value.ambient_values().len()
                    && first_value
                        .ambient_values()
                        .iter()
                        .zip(second_value.ambient_values())
                        .all(|(first, second)| first.to_bits() == second.to_bits())
            },
        )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn optimize_priority_group(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    state: VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    config: SolverConfig,
    control: Option<&mut OperationController>,
) -> Option<PriorityComponentOutcome> {
    if group.component_indices.len() == 1
        && protected.len() <= 1
        && plan.component_layouts[group.component_indices[0]].tangent_dimension
            < PROJECTED_CGLS_MIN_NULLITY
    {
        let component_index = group.component_indices[0];
        let (protected_ids, attained_cost) = protected.first().map_or((&[][..], None), |level| {
            (level.residual_ids.as_slice(), Some(level.attained_cost))
        });
        let mut outcome = optimize_component_priority(
            problem,
            plan,
            &plan.components[component_index],
            state,
            category,
            &group.residual_ids,
            protected_ids,
            attained_cost,
            config,
            control,
        )?;
        decorate_priority_report(
            &mut outcome.report,
            group,
            PrioritySolveBackend::DenseNullspace,
            plan.component_layouts[component_index].tangent_dimension,
            protected,
        );
        return Some(outcome);
    }
    optimize_coupled_priority(
        problem, plan, group, state, category, protected, config, control,
    )
}

pub(super) fn decorate_priority_report(
    report: &mut PrioritySolveReport,
    group: &PriorityGroup,
    backend: PrioritySolveBackend,
    largest_explicit_nullspace_block_rows: usize,
    protected: &[TemporaryLevel],
) {
    report.group_index = group.group_index;
    report.component_index =
        (group.component_indices.len() == 1).then_some(group.component_indices[0]);
    report
        .component_indices
        .clone_from(&group.component_indices);
    report.scope = PrioritySolveScope::Movable;
    report.backend = Some(backend);
    report.largest_explicit_nullspace_block_rows = largest_explicit_nullspace_block_rows;
    report.protected_temporary = protected_reports(protected);
}

pub(super) fn protected_reports(protected: &[TemporaryLevel]) -> Vec<ProtectedTemporaryReport> {
    protected
        .iter()
        .map(|level| ProtectedTemporaryReport {
            group_index: level.group_index,
            attained_cost: level.attained_cost,
            final_cost: None,
            preservation_tolerance: objective_roundoff_tolerance(
                level.attained_cost,
                level.attained_cost,
            )
            .max(PRIORITY_ZERO_COST_ROUNDOFF),
            preserved: false,
        })
        .collect()
}

pub(super) fn priority_group_failure_report(
    group: &PriorityGroup,
    state: VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    termination: SolveTermination,
) -> PriorityComponentOutcome {
    let mut report = PrioritySolveReport {
        group_index: group.group_index,
        component_index: (group.component_indices.len() == 1).then_some(group.component_indices[0]),
        component_indices: group.component_indices.clone(),
        scope: PrioritySolveScope::Movable,
        backend: None,
        largest_explicit_nullspace_block_rows: 0,
        protected_temporary: protected_reports(protected),
        category,
        iterations: 0,
        initial_cost: None,
        final_cost: None,
        attained_temporary_cost: protected.first().map(|level| level.attained_cost),
        termination,
        status: secondary_status(termination, false),
    };
    for protection in &mut report.protected_temporary {
        protection.preserved = false;
    }
    PriorityComponentOutcome { state, report }
}

#[derive(Debug)]
pub(super) struct LocalNullspaceBlock {
    pub(super) full_range: std::ops::Range<usize>,
    pub(super) reduced_range: std::ops::Range<usize>,
    pub(super) map: LocalNullspaceMap,
}

#[derive(Debug)]
pub(super) enum LocalNullspaceMap {
    Explicit(DMatrix<f64>),
    Identity,
}

#[derive(Debug)]
pub(super) struct BlockProtectedSpace {
    pub(super) blocks: Vec<LocalNullspaceBlock>,
    pub(super) full_dimension: usize,
    pub(super) reduced_dimension: usize,
    pub(super) largest_block_rows: usize,
    pub(super) protected_rows: DMatrix<f64>,
}

impl BlockProtectedSpace {
    fn apply_local_bases(&self, reduced: &DVector<f64>) -> Option<DVector<f64>> {
        if reduced.len() != self.reduced_dimension {
            return None;
        }
        let mut full = DVector::zeros(self.full_dimension);
        for block in &self.blocks {
            let reduced_local = reduced.rows(block.reduced_range.start, block.reduced_range.len());
            let local = match &block.map {
                LocalNullspaceMap::Explicit(basis) => basis * reduced_local,
                LocalNullspaceMap::Identity => reduced_local.into_owned(),
            };
            full.rows_mut(block.full_range.start, block.full_range.len())
                .copy_from(&local);
        }
        full.iter().all(|value| value.is_finite()).then_some(full)
    }

    fn apply_local_transposes(&self, full: &DVector<f64>) -> Option<DVector<f64>> {
        if full.len() != self.full_dimension {
            return None;
        }
        let mut reduced = DVector::zeros(self.reduced_dimension);
        for block in &self.blocks {
            let full_local = full.rows(block.full_range.start, block.full_range.len());
            let local = match &block.map {
                LocalNullspaceMap::Explicit(basis) => basis.transpose() * full_local,
                LocalNullspaceMap::Identity => full_local.into_owned(),
            };
            reduced
                .rows_mut(block.reduced_range.start, block.reduced_range.len())
                .copy_from(&local);
        }
        reduced
            .iter()
            .all(|value| value.is_finite())
            .then_some(reduced)
    }

    fn project_protected(
        &self,
        value: &DVector<f64>,
        tolerance: f64,
        control: Option<&mut OperationController>,
    ) -> Option<DVector<f64>> {
        if self.protected_rows.nrows() == 0 {
            return Some(value.clone());
        }
        let gram = &self.protected_rows * self.protected_rows.transpose();
        let rhs = &self.protected_rows * value;
        let correction = solve_rank_aware_least_squares(&gram, &rhs, tolerance, control)?;
        let projected = value - self.protected_rows.transpose() * correction;
        let violation = &self.protected_rows * &projected;
        let scale = stable_norm(self.protected_rows.iter().copied())?
            * stable_norm(projected.iter().copied())?;
        let allowed = tolerance.max(64.0 * f64::EPSILON * scale);
        (stable_norm(violation.iter().copied())? <= allowed).then_some(projected)
    }

    fn apply(
        &self,
        reduced: &DVector<f64>,
        tolerance: f64,
        control: Option<&mut OperationController>,
    ) -> Option<DVector<f64>> {
        let projected = self.project_protected(reduced, tolerance, control)?;
        self.apply_local_bases(&projected)
    }

    fn apply_transpose(
        &self,
        full: &DVector<f64>,
        tolerance: f64,
        control: Option<&mut OperationController>,
    ) -> Option<DVector<f64>> {
        let reduced = self.apply_local_transposes(full)?;
        self.project_protected(&reduced, tolerance, control)
    }
}

pub(super) struct EqualityProjector {
    pub(super) rows: DMatrix<f64>,
    pub(super) row_space_basis: DMatrix<f64>,
}

impl EqualityProjector {
    pub(super) fn new(
        rows: DMatrix<f64>,
        relative_tolerance: f64,
        mut control: Option<&mut OperationController>,
    ) -> Option<Self> {
        if rows.iter().any(|value| !value.is_finite()) {
            return None;
        }
        if rows.nrows() == 0 {
            let columns = rows.ncols();
            return Some(Self {
                rows,
                row_space_basis: DMatrix::zeros(0, columns),
            });
        }
        let diagnostics = controlled_dense_factorization(
            rows.nrows(),
            rows.ncols(),
            control.as_deref_mut(),
            || rank_diagnostics(&rows, relative_tolerance),
        )?;
        let decomposition =
            controlled_dense_factorization(rows.nrows(), rows.ncols(), control, || {
                Some(rows.clone().svd(false, true))
            })?;
        let right_vectors = decomposition.v_t?;
        let retained = decomposition
            .singular_values
            .iter()
            .enumerate()
            .filter_map(|(index, &value)| (value > diagnostics.threshold).then_some(index))
            .collect::<Vec<_>>();
        let row_space_basis = DMatrix::from_fn(retained.len(), rows.ncols(), |row, column| {
            right_vectors[(retained[row], column)]
        });
        row_space_basis
            .iter()
            .all(|value| value.is_finite())
            .then_some(Self {
                rows,
                row_space_basis,
            })
    }

    pub(super) fn project(&self, value: &DVector<f64>, tolerance: f64) -> Option<DVector<f64>> {
        if value.len() != self.rows.ncols() || value.iter().any(|entry| !entry.is_finite()) {
            return None;
        }
        if self.rows.nrows() == 0 {
            return Some(value.clone());
        }
        let projected = value - self.row_space_basis.transpose() * (&self.row_space_basis * value);
        self.contains(&projected, tolerance).then_some(projected)
    }

    fn contains(&self, value: &DVector<f64>, tolerance: f64) -> bool {
        if value.len() != self.rows.ncols() || value.iter().any(|entry| !entry.is_finite()) {
            return false;
        }
        let Some(violation) = stable_norm((&self.rows * value).iter().copied()) else {
            return false;
        };
        let Some(row_norm) = stable_norm(self.rows.iter().copied()) else {
            return false;
        };
        let Some(value_norm) = stable_norm(value.iter().copied()) else {
            return false;
        };
        violation <= tolerance.max(64.0 * f64::EPSILON * row_norm * value_norm)
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn optimize_coupled_priority(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    state: VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    config: SolverConfig,
    mut control: Option<&mut OperationController>,
) -> Option<PriorityComponentOutcome> {
    let outcome = optimize_coupled_priority_inner(
        problem,
        plan,
        group,
        state,
        category,
        protected,
        config,
        control.as_deref_mut(),
    );
    if control
        .as_ref()
        .is_some_and(|controller| controller.is_stopped())
    {
        None
    } else {
        Some(outcome)
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn optimize_coupled_priority_inner(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    mut state: VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    config: SolverConfig,
    control: Option<&mut OperationController>,
) -> PriorityComponentOutcome {
    let initial_system = match linearized_composite_category_system(
        problem,
        plan,
        &group.component_indices,
        &state,
        category,
        &group.residual_ids,
    ) {
        Ok(system) => system,
        Err(error) => {
            return priority_group_failure_report(
                group,
                state,
                category,
                protected,
                error_termination(&error),
            );
        }
    };
    let Some(initial_cost) = residual_cost(&initial_system.residuals) else {
        return priority_group_failure_report(
            group,
            state,
            category,
            protected,
            SolveTermination::NumericalFailure,
        );
    };
    if !validate_priority_components(problem, plan, group, &state, config)
        || !protected_levels_are_preserved(problem, &state, protected, config)
    {
        return coupled_priority_report(
            group,
            state,
            category,
            protected,
            None,
            0,
            Some(initial_cost),
            Some(initial_cost),
            SolveTermination::NumericalFailure,
            0,
        );
    }

    let mut cost = initial_cost;
    let mut backend = None;
    let mut largest_block_rows = 0;
    let mut reprojection_config = config;
    reprojection_config.normalized_residual_tolerance = config
        .normalized_residual_tolerance
        .min(config.normalized_step_tolerance)
        .min(PRIORITY_REPROJECTION_TOLERANCE);
    reprojection_config.normalized_step_tolerance = config
        .normalized_step_tolerance
        .min(PRIORITY_REPROJECTION_TOLERANCE);
    let mut operation = control;
    for iteration in 1..=config.max_iterations {
        let mut control = match operation.as_deref_mut() {
            Some(controller) => match controller.charged_boundary(
                OperationWorkCounter::NonlinearIterations,
                1,
                OperationCheckpoint::BeforeNonlinearIteration,
                OperationCheckpoint::AfterNonlinearIteration,
            ) {
                Ok(boundary) => Some(boundary),
                Err(_) => {
                    return coupled_priority_report(
                        group,
                        state,
                        category,
                        protected,
                        backend,
                        iteration - 1,
                        Some(initial_cost),
                        Some(cost),
                        SolveTermination::NumericalFailure,
                        largest_block_rows,
                    );
                }
            },
            None => None,
        };
        let current = match linearized_composite_category_system(
            problem,
            plan,
            &group.component_indices,
            &state,
            category,
            &group.residual_ids,
        ) {
            Ok(system) => system,
            Err(error) => {
                return coupled_priority_report(
                    group,
                    state,
                    category,
                    protected,
                    backend,
                    iteration - 1,
                    Some(initial_cost),
                    Some(cost),
                    error_termination(&error),
                    largest_block_rows,
                );
            }
        };
        if priority_cost_is_numerically_zero(cost, current.residuals.len(), config) {
            return coupled_priority_report(
                group,
                state,
                category,
                protected,
                backend,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                SolveTermination::Converged,
                largest_block_rows,
            );
        }
        let space = match block_protected_space(
            problem,
            plan,
            group,
            &state,
            protected,
            config,
            control.as_deref_mut(),
        ) {
            Ok(space) => space,
            Err(error) => {
                return coupled_priority_report(
                    group,
                    state,
                    category,
                    protected,
                    backend,
                    iteration - 1,
                    Some(initial_cost),
                    Some(cost),
                    error_termination(&error),
                    largest_block_rows,
                );
            }
        };
        largest_block_rows = largest_block_rows.max(space.largest_block_rows);
        if space.reduced_dimension == 0 {
            return coupled_priority_report(
                group,
                state,
                category,
                protected,
                Some(PrioritySolveBackend::DenseBlockNullspace),
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                SolveTermination::Converged,
                largest_block_rows,
            );
        }
        let group_has_bounds = priority_group_has_bounds(problem, plan, group);
        let use_projected = space.reduced_dimension >= PROJECTED_CGLS_MIN_NULLITY;
        let selected_backend = if use_projected {
            PrioritySolveBackend::ProjectedCgls
        } else {
            PrioritySolveBackend::DenseBlockNullspace
        };
        backend = Some(selected_backend);
        let layout = match composite_tangent_layout(plan, &group.component_indices) {
            Ok(layout) => layout,
            Err(error) => {
                return coupled_priority_report(
                    group,
                    state,
                    category,
                    protected,
                    backend,
                    iteration - 1,
                    Some(initial_cost),
                    Some(cost),
                    error_termination(&error),
                    largest_block_rows,
                );
            }
        };
        let reduced_step = if use_projected && group_has_bounds {
            bounded_projected_cgls_step(
                problem,
                &state,
                &layout,
                &current.jacobian,
                &current.residuals,
                &space,
                config.rank_relative_tolerance,
                config.normalized_step_tolerance,
                control.as_deref_mut(),
            )
        } else if use_projected {
            projected_cgls_step(
                &current.jacobian,
                &current.residuals,
                &space,
                config.rank_relative_tolerance,
                config.normalized_step_tolerance,
                control.as_deref_mut(),
            )
        } else {
            dense_block_constrained_step(
                problem,
                &state,
                &layout,
                &current.jacobian,
                &current.residuals,
                &space,
                config.rank_relative_tolerance,
                config.normalized_step_tolerance,
                control.as_deref_mut(),
            )
        };
        let Some(reduced_step) = reduced_step else {
            return coupled_priority_report(
                group,
                state,
                category,
                protected,
                backend,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                if use_projected {
                    SolveTermination::Stalled
                } else {
                    SolveTermination::NumericalFailure
                },
                largest_block_rows,
            );
        };
        let mut reduced_step = reduced_step;
        let Some(mut step) = (if use_projected {
            space.apply_local_bases(&reduced_step)
        } else {
            space.apply(
                &reduced_step,
                config.normalized_step_tolerance,
                control.as_deref_mut(),
            )
        }) else {
            return coupled_priority_report(
                group,
                state,
                category,
                protected,
                backend,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                if use_projected {
                    SolveTermination::Stalled
                } else {
                    SolveTermination::NumericalFailure
                },
                largest_block_rows,
            );
        };
        let step_limit_valid = if use_projected {
            limit_operator_step(
                &mut reduced_step,
                &mut step,
                &layout,
                config.max_block_normalized_step,
            )
        } else {
            limit_block_steps(&mut step, &layout, config.max_block_normalized_step).map(|_| ())
        };
        let bounds_valid = if use_projected && group_has_bounds {
            operator_step_is_within_bounds(problem, &state, &layout, &mut step)
        } else {
            limit_step_to_bound_events(problem, &state, &layout, &mut step).map(|_| ())
        };
        let projected_valid = !use_projected
            || EqualityProjector::new(
                space.protected_rows.clone(),
                config.rank_relative_tolerance,
                control.as_deref_mut(),
            )
            .is_some_and(|projector| {
                projector.contains(&reduced_step, config.normalized_step_tolerance)
            });
        if step_limit_valid.is_none() || bounds_valid.is_none() || !projected_valid {
            return coupled_priority_report(
                group,
                state,
                category,
                protected,
                backend,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                if use_projected {
                    SolveTermination::Stalled
                } else {
                    SolveTermination::NumericalFailure
                },
                largest_block_rows,
            );
        }
        let step_max = maximum_block_step(&step, &layout).unwrap_or(f64::INFINITY);
        let predicted_decrease = residual_cost(&(&current.residuals + &current.jacobian * &step))
            .is_some_and(|model_cost| objective_decreases(cost, model_cost));
        if use_projected && step_max > config.normalized_step_tolerance && !predicted_decrease {
            return coupled_priority_report(
                group,
                state,
                category,
                protected,
                backend,
                iteration,
                Some(initial_cost),
                Some(cost),
                SolveTermination::Stalled,
                largest_block_rows,
            );
        }
        if step_max <= config.normalized_step_tolerance {
            match search_coupled_negative_curvature(
                problem,
                plan,
                group,
                &state,
                category,
                protected,
                &space,
                &layout,
                cost,
                config,
                reprojection_config,
                control.as_deref_mut(),
            ) {
                CurvatureSearch::Improved(improved_state, improved_cost) => {
                    state = improved_state;
                    cost = improved_cost;
                    continue;
                }
                CurvatureSearch::NoNegativeCurvature => {
                    return acceptable_secondary_outcome(coupled_priority_report(
                        group,
                        state,
                        category,
                        protected,
                        backend,
                        iteration,
                        Some(initial_cost),
                        Some(cost),
                        SolveTermination::Converged,
                        largest_block_rows,
                    ));
                }
                CurvatureSearch::Incomplete | CurvatureSearch::Failed => {
                    return coupled_priority_report(
                        group,
                        state,
                        category,
                        protected,
                        backend,
                        iteration,
                        Some(initial_cost),
                        Some(cost),
                        SolveTermination::Stalled,
                        largest_block_rows,
                    );
                }
            }
        }

        let mut accepted = None;
        let mut alpha = 1.0;
        for _ in 0..MAX_PRIORITY_LINE_SEARCH_STEPS {
            let previous_best = accepted.as_ref().map(|(_, accepted_cost)| *accepted_cost);
            let trial_step = &step * alpha;
            let mut trial_state = state.clone();
            if apply_normalized_step(problem, plan, &mut trial_state, &layout, &trial_step).is_ok()
                && let Some((candidate_state, candidate_cost)) = evaluate_coupled_priority_trial(
                    problem,
                    plan,
                    group,
                    trial_state,
                    category,
                    protected,
                    config,
                    reprojection_config,
                    control.as_deref_mut(),
                )
                && objective_decreases(cost, candidate_cost)
                && accepted
                    .as_ref()
                    .is_none_or(|(_, accepted_cost)| candidate_cost < *accepted_cost)
            {
                accepted = Some((candidate_state, candidate_cost));
            }
            if accepted.as_ref().map(|(_, accepted_cost)| *accepted_cost) == previous_best
                && !charge_rejected_priority_trial(&mut control)
            {
                return coupled_priority_report(
                    group,
                    state,
                    category,
                    protected,
                    backend,
                    iteration,
                    Some(initial_cost),
                    Some(cost),
                    SolveTermination::NumericalFailure,
                    largest_block_rows,
                );
            }
            alpha *= 0.5;
        }
        if let Some((accepted_state, accepted_cost)) = accepted {
            state = accepted_state;
            cost = accepted_cost;
        } else {
            match search_coupled_negative_curvature(
                problem,
                plan,
                group,
                &state,
                category,
                protected,
                &space,
                &layout,
                cost,
                config,
                reprojection_config,
                control.as_deref_mut(),
            ) {
                CurvatureSearch::Improved(improved_state, improved_cost) => {
                    state = improved_state;
                    cost = improved_cost;
                }
                CurvatureSearch::NoNegativeCurvature => {
                    return acceptable_secondary_outcome(coupled_priority_report(
                        group,
                        state,
                        category,
                        protected,
                        backend,
                        iteration,
                        Some(initial_cost),
                        Some(cost),
                        SolveTermination::Converged,
                        largest_block_rows,
                    ));
                }
                CurvatureSearch::Incomplete | CurvatureSearch::Failed => {
                    return coupled_priority_report(
                        group,
                        state,
                        category,
                        protected,
                        backend,
                        iteration,
                        Some(initial_cost),
                        Some(cost),
                        SolveTermination::Stalled,
                        largest_block_rows,
                    );
                }
            }
        }
    }
    coupled_priority_report(
        group,
        state,
        category,
        protected,
        backend,
        config.max_iterations,
        Some(initial_cost),
        Some(cost),
        SolveTermination::IterationLimit,
        largest_block_rows,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn coupled_priority_report(
    group: &PriorityGroup,
    state: VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    backend: Option<PrioritySolveBackend>,
    iterations: usize,
    initial_cost: Option<f64>,
    final_cost: Option<f64>,
    termination: SolveTermination,
    largest_block_rows: usize,
) -> PriorityComponentOutcome {
    PriorityComponentOutcome {
        state,
        report: PrioritySolveReport {
            group_index: group.group_index,
            component_index: None,
            component_indices: group.component_indices.clone(),
            scope: PrioritySolveScope::Movable,
            backend,
            largest_explicit_nullspace_block_rows: largest_block_rows,
            protected_temporary: protected_reports(protected),
            category,
            iterations,
            initial_cost,
            final_cost,
            attained_temporary_cost: protected.first().map(|level| level.attained_cost),
            termination,
            status: secondary_status(termination, false),
        },
    }
}

pub(super) fn block_protected_space(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    state: &VariableState,
    protected: &[TemporaryLevel],
    config: SolverConfig,
    mut control: Option<&mut OperationController>,
) -> Result<BlockProtectedSpace, CoreError> {
    let mut blocks = Vec::new();
    let mut implicit_hard_rows = Vec::new();
    let mut full_offset = 0;
    let mut reduced_offset = 0;
    let mut largest_block_rows = 0;
    for &component_index in &group.component_indices {
        let component = &plan.components[component_index];
        let hard = linearized_hard_system(problem, plan, component, state)?;
        let full_rows = hard.jacobian.ncols();
        let use_implicit_hard_projector =
            full_rows >= PROJECTED_CGLS_MIN_NULLITY && !component.active_residual_ids.is_empty();
        let (reduced_columns, map) = if use_implicit_hard_projector {
            let reduced_range = reduced_offset..reduced_offset + full_rows;
            implicit_hard_rows.push((reduced_range, hard.jacobian));
            (full_rows, LocalNullspaceMap::Identity)
        } else {
            let basis = controlled_dense_factorization(
                hard.jacobian.nrows(),
                hard.jacobian.ncols(),
                control.as_deref_mut(),
                || numerical_nullspace(&hard.jacobian, config.rank_relative_tolerance),
            )
            .ok_or(CoreError::NonFiniteValue {
                context: "priority local hard nullspace",
                index: component_index,
                value: f64::NAN,
            })?;
            largest_block_rows = largest_block_rows.max(basis.nrows());
            (basis.ncols(), LocalNullspaceMap::Explicit(basis))
        };
        let full_end = full_offset + full_rows;
        let reduced_end = reduced_offset + reduced_columns;
        blocks.push(LocalNullspaceBlock {
            full_range: full_offset..full_end,
            reduced_range: reduced_offset..reduced_end,
            map,
        });
        full_offset = full_end;
        reduced_offset = reduced_end;
    }
    let mut space = BlockProtectedSpace {
        blocks,
        full_dimension: full_offset,
        reduced_dimension: reduced_offset,
        largest_block_rows,
        protected_rows: DMatrix::zeros(0, reduced_offset),
    };
    let mut protected_rows = Vec::new();
    for (range, hard) in implicit_hard_rows {
        for row in 0..hard.nrows() {
            let mut reduced_row = DVector::zeros(reduced_offset);
            reduced_row
                .rows_mut(range.start, range.len())
                .copy_from(&hard.row(row).transpose());
            protected_rows.push(reduced_row);
        }
    }
    for level in protected {
        let temporary = linearized_composite_category_system(
            problem,
            plan,
            &group.component_indices,
            state,
            ResidualCategory::Temporary,
            &level.residual_ids,
        )?;
        if priority_cost_is_numerically_zero(level.attained_cost, temporary.residuals.len(), config)
        {
            // At zero least-squares cost the scalar objective gradient vanishes.
            // Preserving the attained level instead requires tangent motion to
            // remain in the nullspace of every zero residual row.
            for row in 0..temporary.jacobian.nrows() {
                let full_row = temporary.jacobian.row(row).transpose().into_owned();
                let reduced_row =
                    space
                        .apply_local_transposes(&full_row)
                        .ok_or(CoreError::NonFiniteValue {
                            context: "protected zero-cost Temporary row",
                            index: protected_rows.len(),
                            value: f64::NAN,
                        })?;
                protected_rows.push(reduced_row);
            }
        } else {
            let full_gradient = temporary.jacobian.transpose() * temporary.residuals;
            let reduced_gradient =
                space
                    .apply_local_transposes(&full_gradient)
                    .ok_or(CoreError::NonFiniteValue {
                        context: "protected Temporary gradient",
                        index: protected_rows.len(),
                        value: f64::NAN,
                    })?;
            protected_rows.push(reduced_gradient);
        }
    }
    space.protected_rows = DMatrix::from_fn(protected_rows.len(), reduced_offset, |row, column| {
        protected_rows[row][column]
    });
    Ok(space)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn dense_block_constrained_step(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    space: &BlockProtectedSpace,
    relative_tolerance: f64,
    normalized_step_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    let protected_nullspace = controlled_dense_factorization(
        space.protected_rows.nrows(),
        space.protected_rows.ncols(),
        control.as_deref_mut(),
        || numerical_nullspace(&space.protected_rows, relative_tolerance),
    )?;
    if protected_nullspace.ncols() == 0 {
        return Some(DVector::zeros(space.reduced_dimension));
    }
    let mut reduced_jacobian = DMatrix::zeros(jacobian.nrows(), protected_nullspace.ncols());
    for column in 0..protected_nullspace.ncols() {
        let full = space.apply_local_bases(&protected_nullspace.column(column).into_owned())?;
        reduced_jacobian
            .column_mut(column)
            .copy_from(&(jacobian * full));
    }
    let full_bounds = normalized_step_bounds(problem, state, layout, space.full_dimension)?;
    let mut constraints = Vec::new();
    for (column, bound) in full_bounds.iter().enumerate() {
        if !bound.lower.is_finite() && !bound.upper.is_finite() {
            continue;
        }
        let mut coordinate = DVector::zeros(space.full_dimension);
        coordinate[column] = 1.0;
        let local_normal = space.apply_local_transposes(&coordinate)?;
        let normal = protected_nullspace.transpose() * local_normal;
        if stable_norm(normal.iter().copied())? == 0.0 {
            continue;
        }
        constraints.push(ReducedStepBound {
            normal,
            lower: bound.lower,
            upper: bound.upper,
        });
    }
    let unconstrained = solve_rank_aware_least_squares(
        &reduced_jacobian,
        &(-residuals),
        relative_tolerance,
        control.as_deref_mut(),
    )?;
    if constraints
        .iter()
        .all(|constraint| constraint_satisfied(constraint, &unconstrained))
    {
        return Some(protected_nullspace * unconstrained);
    }
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
    let mut coefficients = DVector::zeros(protected_nullspace.ncols());
    let maximum_iterations = 8usize.saturating_mul(constraints.len().saturating_add(1));
    let mut kkt_certified = false;
    for _ in 0..maximum_iterations {
        let candidate = solve_active_reduced_least_squares(
            &reduced_jacobian,
            residuals,
            &constraints,
            &working,
            relative_tolerance,
            control.as_deref_mut(),
        )?;
        if let Some((alpha, constraint, side)) =
            first_linear_bound_event(&coefficients, &candidate, &constraints, &working)
        {
            coefficients += (candidate - &coefficients) * alpha;
            if working_constraint_is_independent(
                &constraints,
                &working,
                constraint,
                relative_tolerance,
                control.as_deref_mut(),
            )? {
                working[constraint] = side;
            } else if !constraint_satisfied(&constraints[constraint], &coefficients) {
                return None;
            }
            continue;
        }
        coefficients = candidate;
        let model_residuals = &reduced_jacobian * &coefficients + residuals;
        let gradient = reduced_jacobian.transpose() * model_residuals;
        let tolerance = kkt_gradient_tolerance(
            &reduced_jacobian,
            residuals,
            &coefficients,
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
        kkt_certified = true;
        break;
    }
    if !kkt_certified {
        return None;
    }
    let step = protected_nullspace * coefficients;
    step.iter().all(|value| value.is_finite()).then_some(step)
}

pub(super) fn projected_cgls_step(
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    space: &BlockProtectedSpace,
    rank_tolerance: f64,
    step_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    let projector = EqualityProjector::new(space.protected_rows.clone(), rank_tolerance, control)?;
    projected_cgls_correction(
        jacobian,
        residuals,
        &DVector::zeros(space.reduced_dimension),
        space,
        &projector,
        rank_tolerance,
        step_tolerance,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn projected_cgls_correction(
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    current: &DVector<f64>,
    space: &BlockProtectedSpace,
    projector: &EqualityProjector,
    rank_tolerance: f64,
    step_tolerance: f64,
) -> Option<DVector<f64>> {
    let current_full = space.apply_local_bases(current)?;
    let effective_residuals = residuals + jacobian * current_full;
    let apply = |value: &DVector<f64>| -> Option<DVector<f64>> {
        Some(jacobian * space.apply_local_bases(&projector.project(value, step_tolerance)?)?)
    };
    let apply_transpose = |value: &DVector<f64>| -> Option<DVector<f64>> {
        projector.project(
            &space.apply_local_transposes(&(jacobian.transpose() * value))?,
            step_tolerance,
        )
    };
    let right_hand_side = -effective_residuals;
    let mut solution = DVector::zeros(space.reduced_dimension);
    let mut equation_residual = right_hand_side.clone();
    let mut gradient = apply_transpose(&equation_residual)?;
    let initial_gradient_norm = stable_norm(gradient.iter().copied())?;
    if initial_gradient_norm == 0.0 {
        return Some(solution);
    }
    let tolerance = step_tolerance.max(rank_tolerance) * initial_gradient_norm;
    let mut direction = gradient.clone();
    let mut gradient_norm_squared = gradient.dot(&gradient);
    let max_iterations = space.reduced_dimension.saturating_mul(2).clamp(1, 4096);
    for _ in 0..max_iterations {
        let image = apply(&direction)?;
        let denominator = image.dot(&image);
        if !denominator.is_finite() || denominator <= f64::EPSILON * gradient_norm_squared {
            break;
        }
        let alpha = gradient_norm_squared / denominator;
        solution += alpha * &direction;
        equation_residual -= alpha * image;
        let next_gradient = apply_transpose(&equation_residual)?;
        let next_norm_squared = next_gradient.dot(&next_gradient);
        if !next_norm_squared.is_finite() {
            return None;
        }
        gradient = next_gradient;
        if next_norm_squared.sqrt() <= tolerance {
            break;
        }
        direction = &gradient + (next_norm_squared / gradient_norm_squared) * direction;
        gradient_norm_squared = next_norm_squared;
    }
    let validated_gradient = apply_transpose(&(apply(&solution)? - right_hand_side))?;
    let validated_norm = stable_norm(validated_gradient.iter().copied())?;
    (validated_norm <= tolerance.max(64.0 * f64::EPSILON * initial_gradient_norm))
        .then_some(projector.project(&solution, step_tolerance)?)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn bounded_projected_cgls_step(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    space: &BlockProtectedSpace,
    rank_tolerance: f64,
    step_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<DVector<f64>> {
    let full_bounds = normalized_step_bounds(problem, state, layout, space.full_dimension)?;
    let constraints = operator_bound_constraints(&full_bounds, space)?;
    let desired = constraints
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
    let mut working = vec![WorkingBound::Free; constraints.len()];
    for fixed_only in [true, false] {
        for (index, status) in desired.iter().copied().enumerate() {
            if status == WorkingBound::Free || (status == WorkingBound::Fixed) != fixed_only {
                continue;
            }
            let independent = operator_bound_is_independent(
                space,
                &constraints,
                &working,
                index,
                rank_tolerance,
                step_tolerance,
                control.as_deref_mut(),
            )?;
            if status == WorkingBound::Fixed || independent {
                // Fixed coordinate bounds remain equality rows even when their
                // projected normals are dependent. Otherwise roundoff can
                // repeatedly rediscover an omitted equality as a zero-length
                // event. Keep the independence check for controlled-work
                // accounting and for the releasable one-sided bounds.
                working[index] = status;
            }
        }
    }

    let mut step = DVector::zeros(space.reduced_dimension);
    let maximum_iterations = 8usize.saturating_mul(constraints.len().saturating_add(1));
    for _ in 0..maximum_iterations {
        let equality_rows = operator_equality_rows(space, &constraints, &working);
        let projector =
            EqualityProjector::new(equality_rows, rank_tolerance, control.as_deref_mut())?;
        let correction = projected_cgls_correction(
            jacobian,
            residuals,
            &step,
            space,
            &projector,
            rank_tolerance,
            step_tolerance,
        )?;
        let candidate = &step + correction;
        if let OperatorBoundEvent::Event(alpha, constraint, side) =
            first_operator_bound_event(&step, &candidate, &constraints, &working)?
        {
            step += (candidate - &step) * alpha;
            if operator_bound_is_independent(
                space,
                &constraints,
                &working,
                constraint,
                rank_tolerance,
                step_tolerance,
                control.as_deref_mut(),
            )? {
                working[constraint] = side;
            } else if !constraint_satisfied(&constraints[constraint], &step) {
                return None;
            }
            continue;
        }
        step = candidate;
        if !constraints
            .iter()
            .all(|constraint| constraint_satisfied(constraint, &step))
            || !operator_active_equalities_satisfied(&step, &constraints, &working, step_tolerance)
        {
            return None;
        }

        let mut full_step = space.apply_local_bases(&step)?;
        snap_constrained_roundoff(&mut full_step, &full_bounds)?;
        if !operator_full_step_satisfies_bounds(&full_step, &full_bounds) {
            return None;
        }
        let model_residuals = residuals + jacobian * &full_step;
        let full_gradient = jacobian.transpose() * &model_residuals;
        let reduced_gradient = space.apply_local_transposes(&full_gradient)?;
        let tolerance =
            kkt_gradient_tolerance(jacobian, residuals, &full_step, 0.0, step_tolerance)?;
        let kkt = operator_working_set_kkt(
            &reduced_gradient,
            space,
            &constraints,
            &working,
            rank_tolerance,
            tolerance,
            control.as_deref_mut(),
        )?;
        if let Some(release) = kkt.release {
            working[release] = WorkingBound::Free;
            continue;
        }
        let projector = EqualityProjector::new(
            operator_equality_rows(space, &constraints, &working),
            rank_tolerance,
            control.as_deref_mut(),
        )?;
        let projected_gradient = projector.project(&reduced_gradient, step_tolerance)?;
        let projected_norm = stable_norm(projected_gradient.iter().copied())?;
        let gradient_norm = stable_norm(reduced_gradient.iter().copied())?;
        if projected_norm > tolerance.max(64.0 * f64::EPSILON * gradient_norm)
            || !EqualityProjector::new(
                space.protected_rows.clone(),
                rank_tolerance,
                control.as_deref_mut(),
            )?
            .contains(&step, step_tolerance)
        {
            return None;
        }
        let current_cost = residual_cost(residuals)?;
        let model_cost = residual_cost(&model_residuals)?;
        let step_norm = stable_norm(full_step.iter().copied())?;
        if step_norm > step_tolerance && !objective_decreases(current_cost, model_cost) {
            return None;
        }
        return step.iter().all(|value| value.is_finite()).then_some(step);
    }
    None
}

pub(super) fn operator_bound_constraints(
    full_bounds: &[NormalizedStepBound],
    space: &BlockProtectedSpace,
) -> Option<Vec<ReducedStepBound>> {
    let mut constraints = Vec::new();
    for (column, bound) in full_bounds.iter().copied().enumerate() {
        if !bound.lower.is_finite() && !bound.upper.is_finite() {
            continue;
        }
        let mut coordinate = DVector::zeros(space.full_dimension);
        coordinate[column] = 1.0;
        let normal = space.apply_local_transposes(&coordinate)?;
        if stable_norm(normal.iter().copied())? == 0.0 {
            continue;
        }
        constraints.push(ReducedStepBound {
            normal,
            lower: bound.lower,
            upper: bound.upper,
        });
    }
    Some(constraints)
}

pub(super) fn operator_equality_rows(
    space: &BlockProtectedSpace,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
) -> DMatrix<f64> {
    let active = working
        .iter()
        .enumerate()
        .filter_map(|(index, status)| (*status != WorkingBound::Free).then_some(index))
        .collect::<Vec<_>>();
    DMatrix::from_fn(
        space.protected_rows.nrows() + active.len(),
        space.reduced_dimension,
        |row, column| {
            if row < space.protected_rows.nrows() {
                space.protected_rows[(row, column)]
            } else {
                constraints[active[row - space.protected_rows.nrows()]].normal[column]
            }
        },
    )
}

pub(super) fn operator_bound_is_independent(
    space: &BlockProtectedSpace,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    candidate: usize,
    rank_tolerance: f64,
    step_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<bool> {
    if working.get(candidate)? != &WorkingBound::Free {
        return Some(false);
    }
    let projector = EqualityProjector::new(
        operator_equality_rows(space, constraints, working),
        rank_tolerance,
        control,
    )?;
    let normal = constraints.get(candidate)?.normal.clone();
    let projected = projector.project(&normal, step_tolerance)?;
    let normal_norm = stable_norm(normal.iter().copied())?;
    let projected_norm = stable_norm(projected.iter().copied())?;
    let dimension = f64::from(u32::try_from(normal.len().max(1)).unwrap_or(u32::MAX));
    let threshold =
        (rank_tolerance * normal_norm).max(f64::EPSILON * dimension * normal_norm.max(1.0));
    Some(projected_norm > threshold)
}

pub(super) enum OperatorBoundEvent {
    None,
    Event(f64, usize, WorkingBound),
}

pub(super) fn first_operator_bound_event(
    current: &DVector<f64>,
    candidate: &DVector<f64>,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
) -> Option<OperatorBoundEvent> {
    let mut event: Option<(f64, usize, WorkingBound)> = None;
    for (index, constraint) in constraints.iter().enumerate() {
        if working[index] != WorkingBound::Free {
            continue;
        }
        let current_value = constraint.normal.dot(current);
        let candidate_value = constraint.normal.dot(candidate);
        let direction = candidate_value - current_value;
        if !current_value.is_finite() || !candidate_value.is_finite() || !direction.is_finite() {
            return None;
        }
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
    Some(
        event.map_or(OperatorBoundEvent::None, |(alpha, index, side)| {
            OperatorBoundEvent::Event(alpha, index, side)
        }),
    )
}

pub(super) fn operator_working_set_kkt(
    gradient: &DVector<f64>,
    space: &BlockProtectedSpace,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    rank_tolerance: f64,
    tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<WorkingSetKkt> {
    let active = working
        .iter()
        .enumerate()
        .filter_map(|(index, status)| (*status != WorkingBound::Free).then_some(index))
        .collect::<Vec<_>>();
    let matrix = operator_equality_rows(space, constraints, working);
    if matrix.nrows() == 0 {
        let norm = stable_norm(gradient.iter().copied())?;
        return (norm <= tolerance.max(64.0 * f64::EPSILON * norm)).then_some(WorkingSetKkt {
            release: None,
            multipliers: vec![0.0; constraints.len()],
        });
    }
    let multiplier_values =
        solve_rank_aware_least_squares(&matrix.transpose(), &(-gradient), rank_tolerance, control)?;
    let stationarity = gradient + matrix.transpose() * &multiplier_values;
    let stationarity_norm = stable_norm(stationarity.iter().copied())?;
    let gradient_norm = stable_norm(gradient.iter().copied())?;
    if stationarity_norm > tolerance.max(64.0 * f64::EPSILON * gradient_norm) {
        return None;
    }
    let mut multipliers = vec![0.0; constraints.len()];
    let offset = space.protected_rows.nrows();
    for (position, &index) in active.iter().enumerate() {
        multipliers[index] = multiplier_values[offset + position];
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
    Some(WorkingSetKkt {
        release: release.map(|(index, _)| index),
        multipliers,
    })
}

pub(super) fn operator_active_equalities_satisfied(
    step: &DVector<f64>,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    tolerance: f64,
) -> bool {
    constraints.iter().zip(working).all(|(constraint, status)| {
        let target = match status {
            WorkingBound::Lower | WorkingBound::Fixed => Some(constraint.lower),
            WorkingBound::Upper => Some(constraint.upper),
            WorkingBound::Free => None,
        };
        target.is_none_or(|target| {
            let value = constraint.normal.dot(step);
            let scale = stable_norm(constraint.normal.iter().copied())
                .and_then(|normal| stable_norm(step.iter().copied()).map(|step| normal * step))
                .unwrap_or(f64::INFINITY);
            value.is_finite()
                && (value - target).abs() <= tolerance.max(64.0 * f64::EPSILON * scale)
        })
    })
}

pub(super) fn priority_group_has_bounds(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
) -> bool {
    problem.bounds().any(|(_, bound)| {
        plan.component_for_variable(bound.variable_id())
            .is_some_and(|component| group.component_indices.contains(&component))
    })
}

pub(super) fn validate_priority_components(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    state: &VariableState,
    config: SolverConfig,
) -> bool {
    group.component_indices.iter().all(|&component_index| {
        let component = &plan.components[component_index];
        validate_component(problem, component, state, config).valid
            && linearized_hard_system(problem, plan, component, state).is_ok()
    })
}

pub(super) fn protected_levels_are_preserved(
    problem: &Problem,
    state: &VariableState,
    protected: &[TemporaryLevel],
    config: SolverConfig,
) -> bool {
    protected.iter().all(|level| {
        priority_cost_for_residuals(problem, state, &level.residual_ids).is_ok_and(|cost| {
            priority_cost_within_limit(cost, level.attained_cost, level.residual_ids.len(), config)
        })
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_coupled_priority_trial(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    mut trial_state: VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    config: SolverConfig,
    reprojection_config: SolverConfig,
    control: Option<&mut OperationController>,
) -> Option<(VariableState, f64)> {
    let mut operation = control;
    let mut control = match operation.as_deref_mut() {
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
    let result = (|| {
        for &component_index in &group.component_indices {
            trial_state = iterate_component(
                problem,
                plan,
                &plan.components[component_index],
                trial_state,
                reprojection_config,
                control.as_deref_mut(),
            )?
            .state;
        }
        if !validate_priority_components(problem, plan, group, &trial_state, config) {
            return None;
        }
        if category == ResidualCategory::Preference {
            for level in protected {
                let temporary_group = PriorityGroup {
                    group_index: level.group_index,
                    component_indices: level.component_indices.clone(),
                    residual_ids: level.residual_ids.clone(),
                    protected_temporary_groups: Vec::new(),
                };
                let outcome = optimize_priority_group(
                    problem,
                    plan,
                    &temporary_group,
                    trial_state,
                    ResidualCategory::Temporary,
                    &[],
                    config,
                    control.as_deref_mut(),
                )?;
                if outcome.report.termination != SolveTermination::Converged {
                    return None;
                }
                trial_state = outcome.state;
            }
            if !protected_levels_are_preserved(problem, &trial_state, protected, config) {
                return None;
            }
        }
        let system = linearized_composite_category_system(
            problem,
            plan,
            &group.component_indices,
            &trial_state,
            category,
            &group.residual_ids,
        )
        .ok()?;
        Some((trial_state, residual_cost(&system.residuals)?))
    })();
    drop(control);
    if operation
        .as_ref()
        .is_some_and(|controller| controller.is_stopped())
    {
        return None;
    }
    result
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn search_coupled_negative_curvature(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    state: &VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    space: &BlockProtectedSpace,
    layout: &ActiveLayout,
    current_cost: f64,
    config: SolverConfig,
    reprojection_config: SolverConfig,
    mut control: Option<&mut OperationController>,
) -> CurvatureSearch {
    let hessian_step =
        PRIORITY_HESSIAN_NORMALIZED_STEP.min(config.max_block_normalized_step / 2.0_f64.sqrt());
    let Ok(current) = linearized_composite_category_system(
        problem,
        plan,
        &group.component_indices,
        state,
        category,
        &group.residual_ids,
    ) else {
        return CurvatureSearch::Failed;
    };
    let full_gradient = current.jacobian.transpose() * &current.residuals;
    let Some(reduced_gradient) = space.apply_transpose(
        &full_gradient,
        config.normalized_step_tolerance,
        control.as_deref_mut(),
    ) else {
        return CurvatureSearch::Failed;
    };
    let Some(gradient_tolerance) = kkt_gradient_tolerance(
        &current.jacobian,
        &current.residuals,
        &DVector::zeros(current.jacobian.ncols()),
        0.0,
        config.normalized_step_tolerance,
    ) else {
        return CurvatureSearch::Failed;
    };
    if stable_norm(reduced_gradient.iter().copied()).is_none_or(|norm| norm > gradient_tolerance) {
        return CurvatureSearch::Incomplete;
    }
    if space.reduced_dimension >= PROJECTED_CGLS_MIN_NULLITY {
        // Do not materialize any group-wide nullspace basis after selecting the
        // large operator path.
        return CurvatureSearch::Incomplete;
    }
    let Some(protected_nullspace) = controlled_dense_factorization(
        space.protected_rows.nrows(),
        space.protected_rows.ncols(),
        control.as_deref_mut(),
        || numerical_nullspace(&space.protected_rows, config.rank_relative_tolerance),
    ) else {
        return CurvatureSearch::Failed;
    };
    let dimension = protected_nullspace.ncols();
    if dimension == 0 {
        return CurvatureSearch::NoNegativeCurvature;
    }
    if dimension > 16 {
        // Do not hide a quadratic dense fallback inside a medium operator group.
        return CurvatureSearch::Incomplete;
    }
    let Some(step_bounds) = normalized_step_bounds(problem, state, layout, space.full_dimension)
    else {
        return CurvatureSearch::Failed;
    };
    let mut full_curvature_basis = DMatrix::zeros(space.full_dimension, dimension);
    for column in 0..dimension {
        let Some(full) = space.apply_local_bases(&protected_nullspace.column(column).into_owned())
        else {
            return CurvatureSearch::Failed;
        };
        full_curvature_basis.column_mut(column).copy_from(&full);
    }
    for (coordinate, bound) in step_bounds.iter().enumerate() {
        let Some(radius) = curvature_stencil_coordinate_radius(
            full_curvature_basis.row(coordinate).iter().copied(),
            hessian_step,
        ) else {
            return CurvatureSearch::Failed;
        };
        if bound.lower > -radius || bound.upper < radius {
            // Without a complete interior ball in the actual protected search
            // space, the symmetric stencil cannot certify a one-sided cone.
            return CurvatureSearch::Incomplete;
        }
    }
    let curvature = multi_scale_curvature(
        dimension,
        current_cost,
        hessian_step,
        config,
        CurvatureStencilPolicy::ConsistentFineScales,
        |delta| {
            sample_coupled_priority_cost(
                problem,
                plan,
                group,
                state,
                category,
                protected,
                space,
                &protected_nullspace,
                layout,
                delta,
                config,
                reprojection_config,
                control.as_deref_mut(),
            )
        },
    );
    let reduced_direction = match curvature {
        Some(MultiScaleCurvature::Negative(direction)) => direction,
        Some(MultiScaleCurvature::NoNegative) => {
            return CurvatureSearch::NoNegativeCurvature;
        }
        Some(MultiScaleCurvature::Inconclusive) => return CurvatureSearch::Incomplete,
        None => return CurvatureSearch::Failed,
    };
    let protected_direction = protected_nullspace * reduced_direction;
    let Some(mut direction) = space.apply_local_bases(&protected_direction) else {
        return CurvatureSearch::Failed;
    };
    if limit_block_steps(&mut direction, layout, config.max_block_normalized_step).is_none() {
        return CurvatureSearch::Failed;
    }
    let mut best: Option<(VariableState, f64)> = None;
    for sign in [1.0, -1.0] {
        let mut alpha = 1.0;
        for _ in 0..MAX_PRIORITY_LINE_SEARCH_STEPS {
            let previous_best = best.as_ref().map(|(_, best_cost)| *best_cost);
            let step = &direction * (sign * alpha);
            let mut trial_state = state.clone();
            if apply_normalized_step(problem, plan, &mut trial_state, layout, &step).is_ok()
                && let Some((candidate_state, candidate_cost)) = evaluate_coupled_priority_trial(
                    problem,
                    plan,
                    group,
                    trial_state,
                    category,
                    protected,
                    config,
                    reprojection_config,
                    control.as_deref_mut(),
                )
                && objective_decreases(current_cost, candidate_cost)
                && best
                    .as_ref()
                    .is_none_or(|(_, best_cost)| candidate_cost < *best_cost)
            {
                best = Some((candidate_state, candidate_cost));
            }
            if best.as_ref().map(|(_, best_cost)| *best_cost) == previous_best
                && !charge_rejected_priority_trial(&mut control)
            {
                return CurvatureSearch::Failed;
            }
            alpha *= 0.5;
        }
    }
    best.map_or(
        CurvatureSearch::Failed,
        |(improved_state, improved_cost)| CurvatureSearch::Improved(improved_state, improved_cost),
    )
}

pub(super) fn curvature_stencil_coordinate_radius(
    coefficients: impl Iterator<Item = f64>,
    step: f64,
) -> Option<f64> {
    if !step.is_finite() || step < 0.0 {
        return None;
    }
    let mut largest = 0.0_f64;
    let mut second_largest = 0.0_f64;
    for coefficient in coefficients {
        if !coefficient.is_finite() {
            return None;
        }
        let magnitude = coefficient.abs();
        if magnitude > largest {
            second_largest = largest;
            largest = magnitude;
        } else if magnitude > second_largest {
            second_largest = magnitude;
        }
    }
    let radius = step * (largest + second_largest);
    radius.is_finite().then_some(radius)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sample_coupled_priority_cost(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
    state: &VariableState,
    category: ResidualCategory,
    protected: &[TemporaryLevel],
    space: &BlockProtectedSpace,
    protected_nullspace: &DMatrix<f64>,
    layout: &ActiveLayout,
    delta: &DVector<f64>,
    config: SolverConfig,
    reprojection_config: SolverConfig,
    control: Option<&mut OperationController>,
) -> Option<f64> {
    let reduced = protected_nullspace * delta;
    let mut step = space.apply_local_bases(&reduced)?;
    limit_block_steps(&mut step, layout, config.max_block_normalized_step)?;
    step_is_within_bounds(problem, state, layout, &mut step)?;
    let mut trial_state = state.clone();
    apply_normalized_step(problem, plan, &mut trial_state, layout, &step).ok()?;
    evaluate_coupled_priority_trial(
        problem,
        plan,
        group,
        trial_state,
        category,
        protected,
        config,
        reprojection_config,
        control,
    )
    .map(|(_, cost)| cost)
}

#[derive(Debug)]
pub(super) struct PriorityComponentOutcome {
    pub(super) state: VariableState,
    pub(super) report: PrioritySolveReport,
}

pub(super) fn acceptable_secondary_outcome(
    mut outcome: PriorityComponentOutcome,
) -> PriorityComponentOutcome {
    debug_assert_eq!(outcome.report.termination, SolveTermination::Converged);
    outcome.report.status = SecondaryStatus::Acceptable;
    outcome
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn optimize_component_priority(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_priority_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    config: SolverConfig,
    mut control: Option<&mut OperationController>,
) -> Option<PriorityComponentOutcome> {
    let positive_protected_temporary = category == ResidualCategory::Preference
        && attained_temporary_cost.is_some_and(|attained| {
            !priority_cost_is_numerically_zero(
                attained,
                priority_residual_rows(problem, protected_priority_ids),
                config,
            )
        });
    let original_state = state.clone();
    let protected_target = positive_protected_temporary.then(|| {
        attained_temporary_residual_target(
            problem,
            plan,
            component,
            &state,
            protected_priority_ids,
            attained_temporary_cost?,
            config,
        )
    });
    let protected_target = match protected_target {
        Some(Some(target)) => Some(target),
        Some(None) => {
            return Some(priority_component_report(
                state,
                component.index,
                category,
                0,
                None,
                None,
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            ));
        }
        None => None,
    };

    let mut outcome = optimize_component_priority_inner(
        problem,
        plan,
        component,
        state,
        category,
        residual_ids,
        protected_priority_ids,
        attained_temporary_cost,
        protected_target.as_ref(),
        config,
        control.as_deref_mut(),
    );
    if control
        .as_ref()
        .is_some_and(|controller| controller.is_stopped())
    {
        return None;
    }
    if let Some(target) = protected_target {
        let certified_cost = priority_component_cost(
            problem,
            plan,
            component,
            &outcome.state,
            category,
            residual_ids,
        );
        let certified =
            outcome.report.termination == SolveTermination::Converged
                && attained_temporary_cost.is_some_and(|_| {
                    positive_temporary_candidate_is_valid(
                        problem,
                        plan,
                        component,
                        &outcome.state,
                        protected_priority_ids,
                        &target,
                        config,
                    )
                })
                && outcome.report.final_cost.zip(certified_cost).is_some_and(
                    |(reported, fresh)| {
                        priority_cost_within_limit(
                            fresh,
                            reported,
                            priority_residual_rows(problem, residual_ids),
                            config,
                        )
                    },
                );
        if let Some(cost) = certified.then_some(certified_cost).flatten() {
            outcome.report.final_cost = Some(cost);
        } else {
            let initial_cost = priority_component_cost(
                problem,
                plan,
                component,
                &original_state,
                category,
                residual_ids,
            );
            return Some(priority_component_report(
                original_state,
                component.index,
                category,
                outcome.report.iterations,
                initial_cost,
                initial_cost,
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            ));
        }
    }
    Some(outcome)
}

pub(super) fn attained_temporary_residual_target(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    residual_ids: &[ResidualId],
    attained_cost: f64,
    config: SolverConfig,
) -> Option<DVector<f64>> {
    let system = linearized_category_system(
        problem,
        plan,
        component.index,
        state,
        ResidualCategory::Temporary,
        residual_ids,
    )
    .ok()?;
    if system.residuals.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let cost = residual_cost(&system.residuals)?;
    priority_cost_within_limit(cost, attained_cost, system.residuals.len(), config)
        .then_some(system.residuals)
}

pub(super) fn residual_target_rows_are_preserved(
    residuals: &DVector<f64>,
    target: &DVector<f64>,
    config: SolverConfig,
) -> bool {
    if residuals.len() != target.len() {
        return false;
    }
    let tolerance = residual_target_row_tolerance(config);
    residuals
        .iter()
        .zip(target)
        .try_fold(0.0_f64, |largest, (value, target)| {
            let error = (value - target).abs();
            error.is_finite().then_some(largest.max(error))
        })
        .is_some_and(|error| error <= tolerance)
}

pub(super) fn residual_target_row_tolerance(config: SolverConfig) -> f64 {
    // Preserve at the tighter configured solve tolerance unless that value is below the
    // documented machine reproducibility floor.
    config
        .normalized_residual_tolerance
        .min(config.normalized_step_tolerance)
        .max(PRIORITY_REPROJECTION_TOLERANCE)
}

pub(super) fn priority_component_cost(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
) -> Option<f64> {
    linearized_category_system(
        problem,
        plan,
        component.index,
        state,
        category,
        residual_ids,
    )
    .ok()
    .and_then(|system| residual_cost(&system.residuals))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn positive_temporary_candidate_is_valid(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    protected_priority_ids: &[ResidualId],
    target: &DVector<f64>,
    config: SolverConfig,
) -> bool {
    if state.values.iter().any(|(_, value)| {
        value
            .ambient_values()
            .iter()
            .any(|coordinate| !coordinate.is_finite())
    }) {
        return false;
    }
    let Ok(temporary) = linearized_category_system(
        problem,
        plan,
        component.index,
        state,
        ResidualCategory::Temporary,
        protected_priority_ids,
    ) else {
        return false;
    };
    validate_component(problem, component, state, config).valid
        && linearized_hard_system(problem, plan, component, state).is_ok()
        && residual_target_rows_are_preserved(&temporary.residuals, target, config)
        && residual_cost(&temporary.residuals).is_some()
}

pub(super) fn component_has_movable_priority_incidence(
    problem: &Problem,
    plan: &EliminationPlan,
    component_index: usize,
    category: ResidualCategory,
) -> bool {
    classify_priority_incidence(problem, plan, category)
        .0
        .iter()
        .any(|incidence| incidence.component_indices.contains(&component_index))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn optimize_component_priority_inner(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    mut state: VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_priority_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    protected_target: Option<&DVector<f64>>,
    config: SolverConfig,
    control: Option<&mut OperationController>,
) -> PriorityComponentOutcome {
    let initial_system = match linearized_category_system(
        problem,
        plan,
        component.index,
        &state,
        category,
        residual_ids,
    ) {
        Ok(system) => system,
        Err(error) => {
            return priority_component_failure(
                state,
                component.index,
                category,
                0,
                None,
                None,
                attained_temporary_cost,
                &error,
            );
        }
    };
    let Some(initial_cost) = residual_cost(&initial_system.residuals) else {
        return priority_component_report(
            state,
            component.index,
            category,
            0,
            None,
            None,
            attained_temporary_cost,
            SolveTermination::NumericalFailure,
        );
    };
    let hard_validation = validate_component(problem, component, &state, config);
    if !hard_validation.valid {
        let termination = if hard_validation.evaluated {
            SolveTermination::Stalled
        } else {
            hard_validation.termination
        };
        return priority_component_report(
            state,
            component.index,
            category,
            0,
            Some(initial_cost),
            Some(initial_cost),
            attained_temporary_cost,
            termination,
        );
    }
    if let Err(error) = linearized_hard_system(problem, plan, component, &state) {
        return priority_component_failure(
            state,
            component.index,
            category,
            0,
            Some(initial_cost),
            Some(initial_cost),
            attained_temporary_cost,
            &error,
        );
    }
    if let Some(limit) = attained_temporary_cost {
        let current_temporary = match linearized_category_system(
            problem,
            plan,
            component.index,
            &state,
            ResidualCategory::Temporary,
            protected_priority_ids,
        ) {
            Ok(system) => system,
            Err(error) => {
                return priority_component_failure(
                    state,
                    component.index,
                    category,
                    0,
                    Some(initial_cost),
                    Some(initial_cost),
                    attained_temporary_cost,
                    &error,
                );
            }
        };
        let Some(current_temporary_cost) = residual_cost(&current_temporary.residuals) else {
            return priority_component_report(
                state,
                component.index,
                category,
                0,
                Some(initial_cost),
                Some(initial_cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        };
        if !priority_cost_within_limit(
            current_temporary_cost,
            limit,
            current_temporary.residuals.len(),
            config,
        ) {
            return priority_component_report(
                state,
                component.index,
                category,
                0,
                Some(initial_cost),
                Some(initial_cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        }
        if let Some(target) = protected_target
            && !residual_target_rows_are_preserved(&current_temporary.residuals, target, config)
        {
            return priority_component_report(
                state,
                component.index,
                category,
                0,
                Some(initial_cost),
                Some(initial_cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        }
    }

    let mut cost = initial_cost;
    let temporary_has_movable_preference = category == ResidualCategory::Temporary
        && component_has_movable_priority_incidence(
            problem,
            plan,
            component.index,
            ResidualCategory::Preference,
        );
    let mut reprojection_config = config;
    reprojection_config.normalized_residual_tolerance = config
        .normalized_residual_tolerance
        .min(config.normalized_step_tolerance)
        .min(PRIORITY_REPROJECTION_TOLERANCE);
    reprojection_config.normalized_step_tolerance = config
        .normalized_step_tolerance
        .min(PRIORITY_REPROJECTION_TOLERANCE);
    let mut operation = control;
    for iteration in 1..=config.max_iterations {
        let mut control = match operation.as_deref_mut() {
            Some(controller) => match controller.charged_boundary(
                OperationWorkCounter::NonlinearIterations,
                1,
                OperationCheckpoint::BeforeNonlinearIteration,
                OperationCheckpoint::AfterNonlinearIteration,
            ) {
                Ok(boundary) => Some(boundary),
                Err(_) => {
                    return priority_component_report(
                        state,
                        component.index,
                        category,
                        iteration - 1,
                        Some(initial_cost),
                        Some(cost),
                        attained_temporary_cost,
                        SolveTermination::NumericalFailure,
                    );
                }
            },
            None => None,
        };
        let hard = match linearized_hard_system(problem, plan, component, &state) {
            Ok(system) => system,
            Err(error) => {
                return priority_component_failure(
                    state,
                    component.index,
                    category,
                    iteration - 1,
                    Some(initial_cost),
                    Some(cost),
                    attained_temporary_cost,
                    &error,
                );
            }
        };
        let current = match linearized_category_system(
            problem,
            plan,
            component.index,
            &state,
            category,
            residual_ids,
        ) {
            Ok(system) => system,
            Err(error) => {
                return priority_component_failure(
                    state,
                    component.index,
                    category,
                    iteration - 1,
                    Some(initial_cost),
                    Some(cost),
                    attained_temporary_cost,
                    &error,
                );
            }
        };
        let cost_is_resolved = (category != ResidualCategory::Temporary
            || temporary_has_movable_preference)
            && priority_cost_is_numerically_zero(cost, current.residuals.len(), config);
        if cost_is_resolved {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::Converged,
            );
        }
        let protected_priority = if protected_priority_ids.is_empty() {
            DMatrix::zeros(0, hard.jacobian.ncols())
        } else {
            match linearized_category_system(
                problem,
                plan,
                component.index,
                &state,
                ResidualCategory::Temporary,
                protected_priority_ids,
            ) {
                // A protected Temporary owns its complete attained residual
                // vector. A scalar-cost gradient would permit an unrelated
                // equal-cost assembly mode to replace it.
                Ok(system) => system.jacobian,
                Err(error) => {
                    return priority_component_failure(
                        state,
                        component.index,
                        category,
                        iteration - 1,
                        Some(initial_cost),
                        Some(cost),
                        attained_temporary_cost,
                        &error,
                    );
                }
            }
        };
        let Some(protected) = stack_matrices(&hard.jacobian, &protected_priority) else {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        };
        let Some(nullspace) = controlled_dense_factorization(
            protected.nrows(),
            protected.ncols(),
            control.as_deref_mut(),
            || numerical_nullspace(&protected, config.rank_relative_tolerance),
        ) else {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        };
        if nullspace.ncols() == 0 {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::Converged,
            );
        }

        let reduced_jacobian = &current.jacobian * &nullspace;
        let layout = active_layout(plan, component.index);
        let Some(constrained_step) = constrained_nullspace_step(
            problem,
            &state,
            &layout,
            &nullspace,
            &reduced_jacobian,
            &current.residuals,
            config.rank_relative_tolerance,
            config.normalized_step_tolerance,
            control.as_deref_mut(),
        ) else {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        };
        let mut step = constrained_step.step;
        if limit_block_steps(&mut step, &layout, config.max_block_normalized_step).is_none()
            || limit_step_to_bound_events(problem, &state, &layout, &mut step).is_none()
        {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        }
        let Some(normalized_step_max) = maximum_block_step(&step, &layout) else {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        };
        let Some(model_cost) = residual_cost(&(&current.residuals + &current.jacobian * &step))
        else {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration - 1,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::NumericalFailure,
            );
        };
        let step_is_stationary = normalized_step_max <= config.normalized_step_tolerance;
        let model_is_stationary = !objective_decreases(cost, model_cost);
        let mut accepted = None;
        let mut protected_level_blocked = false;
        if !step_is_stationary && !model_is_stationary && !constrained_step.stationary {
            let mut alpha = 1.0;
            for _ in 0..MAX_PRIORITY_LINE_SEARCH_STEPS {
                let previous_best = accepted.as_ref().map(|(_, accepted_cost)| *accepted_cost);
                let trial_step = &step * alpha;
                let mut trial_state = state.clone();
                if apply_normalized_step(problem, plan, &mut trial_state, &layout, &trial_step)
                    .is_err()
                {
                    if !charge_rejected_priority_trial(&mut control) {
                        return priority_component_report(
                            state,
                            component.index,
                            category,
                            iteration,
                            Some(initial_cost),
                            Some(cost),
                            attained_temporary_cost,
                            SolveTermination::NumericalFailure,
                        );
                    }
                    alpha *= 0.5;
                    continue;
                }
                let Some((accepted_state, trial_cost, used_protected_fallback)) =
                    evaluate_priority_trial(
                        problem,
                        plan,
                        component,
                        trial_state,
                        &state,
                        category,
                        residual_ids,
                        protected_priority_ids,
                        attained_temporary_cost,
                        protected_target,
                        config,
                        reprojection_config,
                        control.as_deref_mut(),
                    )
                else {
                    if !charge_rejected_priority_trial(&mut control) {
                        return priority_component_report(
                            state,
                            component.index,
                            category,
                            iteration,
                            Some(initial_cost),
                            Some(cost),
                            attained_temporary_cost,
                            SolveTermination::NumericalFailure,
                        );
                    }
                    alpha *= 0.5;
                    continue;
                };
                if used_protected_fallback {
                    protected_level_blocked = true;
                    break;
                }
                if objective_decreases(cost, trial_cost)
                    && accepted
                        .as_ref()
                        .is_none_or(|(_, accepted_cost)| trial_cost < *accepted_cost)
                {
                    accepted = Some((accepted_state, trial_cost));
                }
                if accepted.is_some()
                    && ((category == ResidualCategory::Temporary
                        && temporary_has_movable_preference)
                        || protected_target.is_some())
                {
                    // Ordinary first-improvement backtracking avoids repeating
                    // expensive hard/vector reprojection for every smaller
                    // step. The outer iteration still performs every required
                    // lexicographic descent and independent certification.
                    break;
                }
                if accepted.is_some()
                    && attained_temporary_cost.is_some_and(|attained| {
                        priority_cost_is_numerically_zero(
                            attained,
                            priority_residual_rows(problem, protected_priority_ids),
                            config,
                        )
                    })
                {
                    break;
                }
                if (category != ResidualCategory::Temporary || temporary_has_movable_preference)
                    && priority_cost_is_numerically_zero(
                        trial_cost,
                        current.residuals.len(),
                        config,
                    )
                {
                    break;
                }
                if accepted.as_ref().map(|(_, accepted_cost)| *accepted_cost) == previous_best
                    && !charge_rejected_priority_trial(&mut control)
                {
                    return priority_component_report(
                        state,
                        component.index,
                        category,
                        iteration,
                        Some(initial_cost),
                        Some(cost),
                        attained_temporary_cost,
                        SolveTermination::NumericalFailure,
                    );
                }
                alpha *= 0.5;
            }
        }
        if let Some((accepted_state, accepted_cost)) = accepted {
            let relative_resolution = config
                .rank_relative_tolerance
                .max((PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON).sqrt());
            let improvement_is_resolved = ((category == ResidualCategory::Preference
                && attained_temporary_cost.is_some_and(|attained| {
                    priority_cost_is_numerically_zero(
                        attained,
                        priority_residual_rows(problem, protected_priority_ids),
                        config,
                    )
                }))
                || (category == ResidualCategory::Temporary && temporary_has_movable_preference))
                && cost - accepted_cost
                    <= relative_resolution * cost.abs().max(accepted_cost.abs());
            if improvement_is_resolved {
                return acceptable_secondary_outcome(priority_component_report(
                    accepted_state,
                    component.index,
                    category,
                    iteration,
                    Some(initial_cost),
                    Some(accepted_cost),
                    attained_temporary_cost,
                    SolveTermination::Converged,
                ));
            }
            state = accepted_state;
            cost = accepted_cost;
            continue;
        }
        if protected_level_blocked {
            return acceptable_secondary_outcome(priority_component_report(
                state,
                component.index,
                category,
                iteration,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::Converged,
            ));
        }
        let stationary_cost_tolerance = 0.5
            * config.normalized_step_tolerance
            * config.normalized_step_tolerance
            * f64::from(u32::try_from(current.residuals.len().max(1)).unwrap_or(u32::MAX));
        if constrained_step.stationary && cost <= stationary_cost_tolerance {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::Converged,
            );
        }
        let resolved_descent = initial_cost - cost
            > objective_roundoff_tolerance(initial_cost, cost).max(PRIORITY_ZERO_COST_ROUNDOFF);
        if temporary_has_movable_preference && resolved_descent {
            // Retain resolved local descent as Acceptable. A feasible target
            // reaches zero through the ordinary rank-aware Temporary solve;
            // an infeasible one must not launch a second nonlinear solve or
            // claim secondary optimality.
            return acceptable_secondary_outcome(priority_component_report(
                state,
                component.index,
                category,
                iteration,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::Converged,
            ));
        }
        let Some(critical_cone) = constrained_step.critical_cone else {
            return priority_component_report(
                state,
                component.index,
                category,
                iteration,
                Some(initial_cost),
                Some(cost),
                attained_temporary_cost,
                SolveTermination::Stalled,
            );
        };
        match search_critical_cone_curvature(
            problem,
            plan,
            component,
            &state,
            category,
            residual_ids,
            protected_priority_ids,
            attained_temporary_cost,
            protected_target,
            &nullspace,
            &critical_cone,
            &layout,
            cost,
            config,
            reprojection_config,
            control.as_deref_mut(),
        ) {
            CurvatureSearch::Improved(curvature_state, curvature_cost) => {
                state = curvature_state;
                cost = curvature_cost;
            }
            CurvatureSearch::NoNegativeCurvature => {
                return acceptable_secondary_outcome(priority_component_report(
                    state,
                    component.index,
                    category,
                    iteration,
                    Some(initial_cost),
                    Some(cost),
                    attained_temporary_cost,
                    SolveTermination::Converged,
                ));
            }
            CurvatureSearch::Incomplete | CurvatureSearch::Failed => {
                return priority_component_report(
                    state,
                    component.index,
                    category,
                    iteration,
                    Some(initial_cost),
                    Some(cost),
                    attained_temporary_cost,
                    SolveTermination::Stalled,
                );
            }
        }
    }
    priority_component_report(
        state,
        component.index,
        category,
        config.max_iterations,
        Some(initial_cost),
        Some(cost),
        attained_temporary_cost,
        SolveTermination::IterationLimit,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn priority_component_failure(
    state: VariableState,
    component_index: usize,
    category: ResidualCategory,
    iterations: usize,
    initial_cost: Option<f64>,
    final_cost: Option<f64>,
    attained_temporary_cost: Option<f64>,
    error: &CoreError,
) -> PriorityComponentOutcome {
    priority_component_report(
        state,
        component_index,
        category,
        iterations,
        initial_cost,
        final_cost,
        attained_temporary_cost,
        error_termination(error),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn priority_component_report(
    state: VariableState,
    component_index: usize,
    category: ResidualCategory,
    iterations: usize,
    initial_cost: Option<f64>,
    final_cost: Option<f64>,
    attained_temporary_cost: Option<f64>,
    termination: SolveTermination,
) -> PriorityComponentOutcome {
    PriorityComponentOutcome {
        state,
        report: PrioritySolveReport {
            group_index: component_index,
            component_index: Some(component_index),
            component_indices: vec![component_index],
            scope: PrioritySolveScope::Movable,
            backend: Some(PrioritySolveBackend::DenseNullspace),
            largest_explicit_nullspace_block_rows: 0,
            protected_temporary: Vec::new(),
            category,
            iterations,
            initial_cost,
            final_cost,
            attained_temporary_cost,
            termination,
            status: secondary_status(termination, false),
        },
    }
}

pub(super) fn evaluate_nonmoving_priority(
    problem: &Problem,
    state: &VariableState,
    group_index: usize,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    scope: PrioritySolveScope,
    required_termination: SolveTermination,
) -> PriorityReportRecord {
    let (cost, evaluation_termination) =
        match priority_cost_for_residuals(problem, state, residual_ids) {
            Ok(cost) => (Some(cost), SolveTermination::Converged),
            Err(error) => (None, error_termination(&error)),
        };
    PriorityReportRecord {
        report: PrioritySolveReport {
            group_index,
            component_index: None,
            component_indices: Vec::new(),
            scope,
            backend: None,
            largest_explicit_nullspace_block_rows: 0,
            protected_temporary: Vec::new(),
            category,
            iterations: 0,
            initial_cost: cost,
            final_cost: cost,
            attained_temporary_cost: None,
            termination: worse_termination(required_termination, evaluation_termination),
            status: if required_termination == SolveTermination::Converged
                && evaluation_termination == SolveTermination::Converged
            {
                SecondaryStatus::Acceptable
            } else {
                secondary_status(
                    worse_termination(required_termination, evaluation_termination),
                    false,
                )
            },
        },
        residual_ids: residual_ids.to_vec(),
    }
}

pub(super) fn refresh_priority_final_costs(
    problem: &Problem,
    state: &VariableState,
    temporary_groups: &[PriorityGroup],
    reports: &mut [PriorityReportRecord],
    config: SolverConfig,
) {
    for record in reports {
        match priority_cost_for_residuals(problem, state, &record.residual_ids) {
            Ok(cost) => record.report.final_cost = Some(cost),
            Err(error) => {
                record.report.final_cost = None;
                record.report.termination =
                    worse_termination(record.report.termination, error_termination(&error));
                record.report.status = SecondaryStatus::EvaluationFailure;
            }
        }
        let dense_vector_backend =
            record.report.backend == Some(PrioritySolveBackend::DenseNullspace);
        for protection in &mut record.report.protected_temporary {
            let result = temporary_groups
                .get(protection.group_index)
                .ok_or(CoreError::DimensionMismatch {
                    context: "protected Temporary group",
                    expected: temporary_groups.len(),
                    actual: protection.group_index,
                })
                .and_then(|group| priority_cost_for_residuals(problem, state, &group.residual_ids));
            if let Ok(cost) = result {
                protection.final_cost = Some(cost);
                let residual_rows = temporary_groups
                    .get(protection.group_index)
                    .map_or(1, |group| {
                        priority_residual_rows(problem, &group.residual_ids)
                    });
                let vector_protected = dense_vector_backend
                    && !priority_cost_is_numerically_zero(
                        protection.attained_cost,
                        residual_rows,
                        config,
                    );
                protection.preservation_tolerance = if vector_protected {
                    residual_vector_cost_tolerance(protection.attained_cost, residual_rows, config)
                } else {
                    priority_preservation_tolerance(protection.attained_cost, residual_rows, config)
                };
                protection.preserved = cost <= protection.attained_cost
                    || cost - protection.attained_cost <= protection.preservation_tolerance;
                if !protection.preserved {
                    record.report.termination = worse_termination(
                        record.report.termination,
                        SolveTermination::NumericalFailure,
                    );
                    record.report.status = SecondaryStatus::EvaluationFailure;
                }
            } else {
                protection.final_cost = None;
                protection.preserved = false;
                record.report.termination = worse_termination(
                    record.report.termination,
                    SolveTermination::NumericalFailure,
                );
                record.report.status = SecondaryStatus::EvaluationFailure;
            }
        }
    }
}

pub(super) fn priority_cost_for_residuals(
    problem: &Problem,
    state: &VariableState,
    residual_ids: &[ResidualId],
) -> Result<f64, CoreError> {
    // A finite value alone is not an acceptable secondary result: validate the
    // derivative at the same returned state before assigning a success-like status.
    let linearization = problem.linearize_blocks_for_state(state, Some(residual_ids))?;
    let residuals = DVector::from_iterator(
        linearization.scalar_rows,
        linearization
            .blocks
            .iter()
            .flat_map(|block| block.normalized_residuals.iter().copied()),
    );
    residual_cost(&residuals).ok_or(CoreError::NonFiniteValue {
        context: "priority residual cost",
        index: 0,
        value: f64::INFINITY,
    })
}

pub(super) fn priority_residual_rows(problem: &Problem, residual_ids: &[ResidualId]) -> usize {
    residual_ids
        .iter()
        .filter_map(|&residual| problem.residual(residual))
        .map(crate::ResidualBlock::output_dimension)
        .sum()
}

/// Roundoff allowance relative only to the compared objective magnitudes.
/// There is intentionally no additive absolute floor.
pub(super) fn objective_roundoff_tolerance(first: f64, second: f64) -> f64 {
    PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON * first.abs().max(second.abs())
}

pub(super) fn objective_decreases(current: f64, candidate: f64) -> bool {
    current - candidate > objective_roundoff_tolerance(current, candidate)
}

pub(super) fn charge_rejected_priority_trial<C>(control: &mut Option<C>) -> bool
where
    C: std::ops::DerefMut<Target = OperationController>,
{
    if let Some(controller) = control.as_deref_mut()
        && !controller.is_stopped()
    {
        return controller
            .charge(
                OperationWorkCounter::RejectedTrials,
                1,
                OperationCheckpoint::BeforeTrialBoundary,
            )
            .is_ok();
    }
    control
        .as_deref()
        .is_none_or(|controller| !controller.is_stopped())
}

pub(super) fn objective_within_limit(candidate: f64, limit: f64) -> bool {
    candidate <= limit
        || candidate - limit
            <= objective_roundoff_tolerance(candidate, limit).max(PRIORITY_ZERO_COST_ROUNDOFF)
}

pub(super) fn priority_zero_cost_limit(residual_rows: usize, config: SolverConfig) -> f64 {
    let rows = f64::from(u32::try_from(residual_rows.max(1)).unwrap_or(u32::MAX));
    let residual_resolution = (PRIORITY_COST_RESOLUTION_FACTOR
        * config.normalized_residual_tolerance)
        .max(config.normalized_step_tolerance);
    0.5 * residual_resolution * residual_resolution * rows
}

pub(super) fn priority_preservation_tolerance(
    attained: f64,
    residual_rows: usize,
    config: SolverConfig,
) -> f64 {
    let roundoff =
        objective_roundoff_tolerance(attained, attained).max(PRIORITY_ZERO_COST_ROUNDOFF);
    if priority_cost_is_numerically_zero(attained, residual_rows, config) {
        roundoff.max(priority_zero_cost_limit(residual_rows, config) - attained)
    } else {
        roundoff
    }
}

pub(super) fn residual_vector_cost_tolerance(
    attained: f64,
    residual_rows: usize,
    config: SolverConfig,
) -> f64 {
    let rows = f64::from(u32::try_from(residual_rows.max(1)).unwrap_or(u32::MAX));
    let error_norm = rows.sqrt() * residual_target_row_tolerance(config);
    let attained_norm = (2.0 * attained.max(0.0)).sqrt();
    attained_norm
        .mul_add(error_norm, 0.5 * error_norm * error_norm)
        .max(objective_roundoff_tolerance(attained, attained))
        .max(PRIORITY_ZERO_COST_ROUNDOFF)
}

pub(super) fn priority_cost_within_limit(
    candidate: f64,
    attained: f64,
    residual_rows: usize,
    config: SolverConfig,
) -> bool {
    objective_within_limit(candidate, attained)
        || candidate - attained <= priority_preservation_tolerance(attained, residual_rows, config)
}

pub(super) fn priority_cost_is_numerically_zero(
    cost: f64,
    residual_rows: usize,
    config: SolverConfig,
) -> bool {
    // Secondary objectives are already below observable solve resolution when
    // every normalized row is within a small multiple of the independently
    // enforced hard-residual tolerance. Do not launch the quadratic curvature
    // escape corpus for such roundoff-scale cursor error: it cannot materially
    // improve the interaction, and the same complete row space remains protected
    // from lower-priority drift.
    cost <= priority_zero_cost_limit(residual_rows, config)
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "priority trial keeps hard/zero-Temporary retraction, validation, and cost evidence atomic"
)]
pub(super) fn evaluate_priority_trial(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    trial_state: VariableState,
    protected_fallback_state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    protected_target: Option<&DVector<f64>>,
    config: SolverConfig,
    reprojection_config: SolverConfig,
    control: Option<&mut OperationController>,
) -> Option<(VariableState, f64, bool)> {
    let mut operation = control;
    let mut control = match operation.as_deref_mut() {
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
    let result = (|| {
        let exact_positive_temporary = category == ResidualCategory::Preference
            && protected_target.is_some()
            && !protected_temporary_ids.is_empty();
        let zero_like_temporary = category == ResidualCategory::Preference
            && protected_target.is_none()
            && !protected_temporary_ids.is_empty()
            && attained_temporary_cost.is_some_and(|attained| {
                priority_cost_is_numerically_zero(
                    attained,
                    priority_residual_rows(problem, protected_temporary_ids),
                    config,
                )
            });
        let reprojected = if let Some(target) = protected_target {
            iterate_component_objective(
                problem,
                plan,
                component,
                trial_state,
                reprojection_config,
                ComponentIterationObjective::HardAndPriorityResidualTarget {
                    category: ResidualCategory::Temporary,
                    residual_ids: protected_temporary_ids,
                    target,
                },
                control.as_deref_mut(),
            )?
        } else if zero_like_temporary {
            iterate_component_objective(
                problem,
                plan,
                component,
                trial_state,
                config,
                ComponentIterationObjective::HardAndPriority {
                    category: ResidualCategory::Temporary,
                    residual_ids: protected_temporary_ids,
                },
                control.as_deref_mut(),
            )?
        } else {
            iterate_component(
                problem,
                plan,
                component,
                trial_state,
                reprojection_config,
                control.as_deref_mut(),
            )?
        };
        let mut candidate_state = reprojected.state;
        let mut used_protected_fallback = false;
        if !validate_component(problem, component, &candidate_state, config).valid
            || linearized_hard_system(problem, plan, component, &candidate_state).is_err()
        {
            if !exact_positive_temporary {
                return None;
            }
            candidate_state = protected_fallback_state.clone();
            used_protected_fallback = true;
        }
        if let Some(limit) = attained_temporary_cost {
            if category != ResidualCategory::Preference || protected_temporary_ids.is_empty() {
                return None;
            }
            let temporary = linearized_category_system(
                problem,
                plan,
                component.index,
                &candidate_state,
                ResidualCategory::Temporary,
                protected_temporary_ids,
            )
            .ok()?;
            let temporary_cost = residual_cost(&temporary.residuals)?;
            let preserved = if let Some(target) = protected_target {
                residual_target_rows_are_preserved(&temporary.residuals, target, config)
            } else {
                priority_cost_is_numerically_zero(limit, temporary.residuals.len(), config)
                    && priority_cost_is_numerically_zero(
                        temporary_cost,
                        temporary.residuals.len(),
                        config,
                    )
            };
            if !preserved {
                candidate_state = protected_fallback_state.clone();
                used_protected_fallback = true;
            }
        }
        let trial_system = linearized_category_system(
            problem,
            plan,
            component.index,
            &candidate_state,
            category,
            residual_ids,
        )
        .ok()?;
        let trial_cost = residual_cost(&trial_system.residuals)?;
        Some((candidate_state, trial_cost, used_protected_fallback))
    })();
    drop(control);
    if operation
        .as_ref()
        .is_some_and(|controller| controller.is_stopped())
    {
        return None;
    }
    result
}

pub(super) enum CurvatureSearch {
    Improved(VariableState, f64),
    NoNegativeCurvature,
    Incomplete,
    Failed,
}

pub(super) enum MultiScaleCurvature {
    Negative(DVector<f64>),
    NoNegative,
    Inconclusive,
}

#[derive(Clone, Copy)]
pub(super) enum CurvatureStencilPolicy {
    ConsistentFineScales,
    SingletonAnyResolvedScale,
}

pub(super) struct CurvatureStencil {
    pub(super) minimum: f64,
    pub(super) tolerance: f64,
    pub(super) minimum_direction: DVector<f64>,
}

#[allow(clippy::too_many_lines)]
pub(super) fn multi_scale_curvature(
    dimension: usize,
    current_cost: f64,
    base_step: f64,
    config: SolverConfig,
    policy: CurvatureStencilPolicy,
    mut sample_cost: impl FnMut(&DVector<f64>) -> Option<f64>,
) -> Option<MultiScaleCurvature> {
    if dimension == 0 {
        return Some(MultiScaleCurvature::NoNegative);
    }
    if !base_step.is_finite() || base_step <= 0.0 {
        return None;
    }
    let mut stencils = Vec::with_capacity(3);
    for level in 0..3 {
        let step = base_step * 0.5_f64.powi(level);
        let mut hessian = DMatrix::zeros(dimension, dimension);
        let mut sample_magnitude = current_cost.abs();
        for axis in 0..dimension {
            let mut delta = DVector::zeros(dimension);
            delta[axis] = step;
            let positive = sample_cost(&delta)?;
            delta[axis] = -step;
            let negative = sample_cost(&delta)?;
            sample_magnitude = sample_magnitude.max(positive.abs()).max(negative.abs());
            let diagonal = (positive - 2.0 * current_cost + negative) / (step * step);
            if !diagonal.is_finite() {
                return None;
            }
            hessian[(axis, axis)] = diagonal;
        }
        for first in 0..dimension {
            for second in (first + 1)..dimension {
                let mut costs = [0.0; 4];
                for (index, (first_sign, second_sign)) in
                    [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
                        .into_iter()
                        .enumerate()
                {
                    let mut delta = DVector::zeros(dimension);
                    delta[first] = first_sign * step;
                    delta[second] = second_sign * step;
                    let cost = sample_cost(&delta)?;
                    costs[index] = cost;
                    sample_magnitude = sample_magnitude.max(cost.abs());
                }
                let mixed = (costs[0] - costs[1] - costs[2] + costs[3]) / (4.0 * step * step);
                if !mixed.is_finite() {
                    return None;
                }
                hessian[(first, second)] = mixed;
                hessian[(second, first)] = mixed;
            }
        }
        let eigen = hessian.symmetric_eigen();
        if eigen.eigenvalues.iter().any(|value| !value.is_finite())
            || eigen.eigenvectors.iter().any(|value| !value.is_finite())
        {
            return None;
        }
        let (minimum_index, &minimum) = eigen
            .eigenvalues
            .iter()
            .enumerate()
            .min_by(|(_, first), (_, second)| first.total_cmp(second))?;
        let largest = eigen
            .eigenvalues
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f64, f64::max);
        let relative_tolerance = config
            .rank_relative_tolerance
            .max((PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON).sqrt());
        let roundoff =
            4.0 * PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON * sample_magnitude / (step * step);
        let tolerance = (largest * relative_tolerance).max(roundoff);
        if !tolerance.is_finite() {
            return None;
        }
        stencils.push(CurvatureStencil {
            minimum,
            tolerance,
            minimum_direction: eigen.eigenvectors.column(minimum_index).into_owned(),
        });
    }

    let previous = &stencils[1];
    let finest = &stencils[2];
    if matches!(policy, CurvatureStencilPolicy::SingletonAnyResolvedScale)
        && let Some(stencil) = stencils
            .iter()
            .find(|stencil| stencil.minimum < -stencil.tolerance)
    {
        // Preserve the established singleton behavior: search every resolved
        // negative-curvature direction, starting at the original coarse scale.
        // Finer scales still prevent a cancellation at the coarse scale from
        // being certified as nonnegative.
        return Some(MultiScaleCurvature::Negative(
            stencil.minimum_direction.clone(),
        ));
    }
    if previous.minimum < -previous.tolerance && finest.minimum < -finest.tolerance {
        let alignment = previous
            .minimum_direction
            .dot(&finest.minimum_direction)
            .abs();
        if alignment >= 0.75 {
            return Some(MultiScaleCurvature::Negative(
                finest.minimum_direction.clone(),
            ));
        }
        return Some(MultiScaleCurvature::Inconclusive);
    }
    if previous.minimum < -previous.tolerance || finest.minimum < -finest.tolerance {
        return Some(MultiScaleCurvature::Inconclusive);
    }

    // Requiring the finite-difference Hessian magnitudes themselves to agree
    // rejects ordinary nonlinear minima whose curvature changes over the
    // stencil interval. The safety condition is instead that every scale is
    // free of significant negative curvature; cancellation at one scale is
    // exposed by either finer scale, as covered by the masked-maximum tests.
    Some(MultiScaleCurvature::NoNegative)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn search_critical_cone_curvature(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    protected_target: Option<&DVector<f64>>,
    protected_nullspace: &DMatrix<f64>,
    critical_cone: &ReducedCriticalCone,
    layout: &ActiveLayout,
    current_cost: f64,
    config: SolverConfig,
    reprojection_config: SolverConfig,
    control: Option<&mut OperationController>,
) -> CurvatureSearch {
    let full_span = protected_nullspace * &critical_cone.span;
    if full_span.iter().any(|value| !value.is_finite()) {
        return CurvatureSearch::Failed;
    }
    if critical_cone.inequalities.nrows() == 0 {
        return search_negative_curvature(
            problem,
            plan,
            component,
            state,
            category,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_cost,
            protected_target,
            &full_span,
            layout,
            current_cost,
            config,
            reprojection_config,
            control,
        );
    }
    if full_span.ncols() == 0 {
        return CurvatureSearch::NoNegativeCurvature;
    }
    if full_span.ncols() != 1 || critical_cone.inequalities.ncols() != 1 {
        // A complete multidimensional cone search needs mixed one-sided
        // curvature reconstruction. Never claim optimality from lineality alone.
        return CurvatureSearch::Incomplete;
    }
    search_one_sided_curvature(
        problem,
        plan,
        component,
        state,
        category,
        residual_ids,
        protected_temporary_ids,
        attained_temporary_cost,
        protected_target,
        &full_span,
        &critical_cone.inequalities,
        layout,
        current_cost,
        config,
        reprojection_config,
        control,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn search_one_sided_curvature(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    protected_target: Option<&DVector<f64>>,
    span: &DMatrix<f64>,
    inequalities: &DMatrix<f64>,
    layout: &ActiveLayout,
    current_cost: f64,
    config: SolverConfig,
    reprojection_config: SolverConfig,
    mut control: Option<&mut OperationController>,
) -> CurvatureSearch {
    let hessian_step = PRIORITY_HESSIAN_NORMALIZED_STEP.min(config.max_block_normalized_step / 2.0);
    if !hessian_step.is_finite() || hessian_step <= 0.0 {
        return CurvatureSearch::Failed;
    }
    let inequality_scale = inequalities
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let feasibility_tolerance = 64.0 * f64::EPSILON * inequality_scale.max(1.0);
    let feasible_signs = [1.0, -1.0]
        .into_iter()
        .filter(|sign| {
            inequalities
                .column(0)
                .iter()
                .all(|value| sign * value >= -feasibility_tolerance)
        })
        .collect::<Vec<_>>();
    if feasible_signs.is_empty() {
        return CurvatureSearch::NoNegativeCurvature;
    }

    let mut most_negative: Option<(f64, f64)> = None;
    for sign in feasible_signs {
        let mut evidence = Vec::with_capacity(3);
        for level in 0..3 {
            let step = hessian_step * 0.5_f64.powi(level);
            let first_delta = DVector::from_element(1, sign * step);
            let Some(first_cost) = sample_reduced_priority_cost(
                problem,
                plan,
                component,
                state,
                category,
                residual_ids,
                protected_temporary_ids,
                attained_temporary_cost,
                protected_target,
                span,
                layout,
                &first_delta,
                config,
                reprojection_config,
                control.as_deref_mut(),
            ) else {
                return CurvatureSearch::Failed;
            };
            let second_delta = DVector::from_element(1, sign * 2.0 * step);
            let Some(second_cost) = sample_reduced_priority_cost(
                problem,
                plan,
                component,
                state,
                category,
                residual_ids,
                protected_temporary_ids,
                attained_temporary_cost,
                protected_target,
                span,
                layout,
                &second_delta,
                config,
                reprojection_config,
                control.as_deref_mut(),
            ) else {
                return CurvatureSearch::Failed;
            };
            let curvature = (second_cost - 2.0 * first_cost + current_cost) / (step * step);
            let sample_magnitude = current_cost
                .abs()
                .max(first_cost.abs())
                .max(second_cost.abs());
            let relative_tolerance = config
                .rank_relative_tolerance
                .max((PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON).sqrt());
            let roundoff = 4.0 * PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON * sample_magnitude
                / (step * step);
            let tolerance = (curvature.abs() * relative_tolerance).max(roundoff);
            if !curvature.is_finite() || !tolerance.is_finite() {
                return CurvatureSearch::Failed;
            }
            evidence.push((curvature, tolerance));
        }
        if evidence[1].0 < -evidence[1].1
            && evidence[2].0 < -evidence[2].1
            && most_negative
                .as_ref()
                .is_none_or(|(current, _)| evidence[2].0 < *current)
        {
            most_negative = Some((evidence[2].0, sign));
        }
    }
    let Some((_, sign)) = most_negative else {
        // A finite one-sided stencil can find descent but cannot certify local
        // nonnegative curvature for an arbitrary evaluator.
        return CurvatureSearch::Incomplete;
    };

    let mut direction = span.column(0).into_owned() * sign;
    if limit_block_steps(&mut direction, layout, config.max_block_normalized_step).is_none() {
        return CurvatureSearch::Failed;
    }
    let mut best: Option<(VariableState, f64)> = None;
    let mut alpha = 1.0;
    for _ in 0..MAX_PRIORITY_LINE_SEARCH_STEPS {
        let previous_best = best.as_ref().map(|(_, best_cost)| *best_cost);
        let mut step = &direction * alpha;
        if limit_step_to_bound_events(problem, state, layout, &mut step).is_none() {
            if !charge_rejected_priority_trial(&mut control) {
                return CurvatureSearch::Failed;
            }
            alpha *= 0.5;
            continue;
        }
        let mut trial_state = state.clone();
        if apply_normalized_step(problem, plan, &mut trial_state, layout, &step).is_err() {
            if !charge_rejected_priority_trial(&mut control) {
                return CurvatureSearch::Failed;
            }
            alpha *= 0.5;
            continue;
        }
        let Some((accepted_state, trial_cost, _)) = evaluate_priority_trial(
            problem,
            plan,
            component,
            trial_state,
            state,
            category,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_cost,
            protected_target,
            config,
            reprojection_config,
            control.as_deref_mut(),
        ) else {
            if !charge_rejected_priority_trial(&mut control) {
                return CurvatureSearch::Failed;
            }
            alpha *= 0.5;
            continue;
        };
        if objective_decreases(current_cost, trial_cost)
            && best
                .as_ref()
                .is_none_or(|(_, best_cost)| trial_cost < *best_cost)
        {
            best = Some((accepted_state, trial_cost));
        }
        if best.as_ref().map(|(_, best_cost)| *best_cost) == previous_best
            && !charge_rejected_priority_trial(&mut control)
        {
            return CurvatureSearch::Failed;
        }
        alpha *= 0.5;
    }
    best.map_or(
        CurvatureSearch::Failed,
        |(improved_state, improved_cost)| CurvatureSearch::Improved(improved_state, improved_cost),
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn search_negative_curvature(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    protected_target: Option<&DVector<f64>>,
    nullspace: &DMatrix<f64>,
    layout: &ActiveLayout,
    current_cost: f64,
    config: SolverConfig,
    reprojection_config: SolverConfig,
    mut control: Option<&mut OperationController>,
) -> CurvatureSearch {
    let dimension = nullspace.ncols();
    if dimension == 0 {
        return CurvatureSearch::NoNegativeCurvature;
    }
    let hessian_step =
        PRIORITY_HESSIAN_NORMALIZED_STEP.min(config.max_block_normalized_step / 2.0_f64.sqrt());
    let curvature = multi_scale_curvature(
        dimension,
        current_cost,
        hessian_step,
        config,
        CurvatureStencilPolicy::SingletonAnyResolvedScale,
        |delta| {
            sample_reduced_priority_cost(
                problem,
                plan,
                component,
                state,
                category,
                residual_ids,
                protected_temporary_ids,
                attained_temporary_cost,
                protected_target,
                nullspace,
                layout,
                delta,
                config,
                reprojection_config,
                control.as_deref_mut(),
            )
        },
    );
    let reduced_direction = match curvature {
        Some(MultiScaleCurvature::Negative(direction)) => direction,
        Some(MultiScaleCurvature::NoNegative) => {
            return CurvatureSearch::NoNegativeCurvature;
        }
        Some(MultiScaleCurvature::Inconclusive) => return CurvatureSearch::Incomplete,
        None => return CurvatureSearch::Failed,
    };
    let mut direction = nullspace * reduced_direction;
    if limit_block_steps(&mut direction, layout, config.max_block_normalized_step).is_none() {
        return CurvatureSearch::Failed;
    }
    let mut best: Option<(VariableState, f64)> = None;
    for sign in [1.0, -1.0] {
        let mut alpha = 1.0;
        for _ in 0..MAX_PRIORITY_LINE_SEARCH_STEPS {
            let previous_best = best.as_ref().map(|(_, best_cost)| *best_cost);
            let mut step = &direction * (sign * alpha);
            if limit_step_to_bound_events(problem, state, layout, &mut step).is_none() {
                if !charge_rejected_priority_trial(&mut control) {
                    return CurvatureSearch::Failed;
                }
                alpha *= 0.5;
                continue;
            }
            let mut trial_state = state.clone();
            if apply_normalized_step(problem, plan, &mut trial_state, layout, &step).is_err() {
                if !charge_rejected_priority_trial(&mut control) {
                    return CurvatureSearch::Failed;
                }
                alpha *= 0.5;
                continue;
            }
            let Some((accepted_state, trial_cost, _)) = evaluate_priority_trial(
                problem,
                plan,
                component,
                trial_state,
                state,
                category,
                residual_ids,
                protected_temporary_ids,
                attained_temporary_cost,
                protected_target,
                config,
                reprojection_config,
                control.as_deref_mut(),
            ) else {
                if !charge_rejected_priority_trial(&mut control) {
                    return CurvatureSearch::Failed;
                }
                alpha *= 0.5;
                continue;
            };
            if objective_decreases(current_cost, trial_cost)
                && best
                    .as_ref()
                    .is_none_or(|(_, best_cost)| trial_cost < *best_cost)
            {
                best = Some((accepted_state, trial_cost));
            }
            if best.as_ref().map(|(_, best_cost)| *best_cost) == previous_best
                && !charge_rejected_priority_trial(&mut control)
            {
                return CurvatureSearch::Failed;
            }
            alpha *= 0.5;
        }
    }
    best.map_or(
        CurvatureSearch::Failed,
        |(improved_state, improved_cost)| CurvatureSearch::Improved(improved_state, improved_cost),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sample_reduced_priority_cost(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    protected_target: Option<&DVector<f64>>,
    nullspace: &DMatrix<f64>,
    layout: &ActiveLayout,
    reduced_delta: &DVector<f64>,
    config: SolverConfig,
    reprojection_config: SolverConfig,
    control: Option<&mut OperationController>,
) -> Option<f64> {
    if reduced_delta.len() != nullspace.ncols() {
        return None;
    }
    let mut step = nullspace * reduced_delta;
    limit_block_steps(&mut step, layout, config.max_block_normalized_step)?;
    step_is_within_bounds(problem, state, layout, &mut step)?;
    let mut trial_state = state.clone();
    apply_normalized_step(problem, plan, &mut trial_state, layout, &step).ok()?;
    evaluate_priority_trial(
        problem,
        plan,
        component,
        trial_state,
        state,
        category,
        residual_ids,
        protected_temporary_ids,
        attained_temporary_cost,
        protected_target,
        config,
        reprojection_config,
        control,
    )
    .map(|(_, cost, _)| cost)
}

pub(super) fn linearized_hard_system(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
) -> Result<HardSystem, CoreError> {
    let linearization =
        problem.linearize_component(plan, component, state, &component.active_residual_ids)?;
    let dense = linearization.project_dense(plan, ResidualCategory::Hard)?;
    let indexed = linearization.project_indexed(plan, ResidualCategory::Hard)?;
    if !indexed.numerically_matches(&dense)
        || indexed.sparsity_signature
            != plan.structural.component_summaries[component.index].sparsity_signature
    {
        return Err(CoreError::DimensionMismatch {
            context: "component indexed/dense hard projection",
            expected: 1,
            actual: 0,
        });
    }
    let mut hard = component_dense_system(dense);
    hard.indexed = Some(indexed);
    Ok(hard)
}

pub(super) fn linearized_component_objective(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    objective: ComponentIterationObjective<'_>,
) -> Result<HardSystem, CoreError> {
    let hard = linearized_hard_system(problem, plan, component, state)?;
    let priority = match objective {
        ComponentIterationObjective::Hard => return Ok(hard),
        ComponentIterationObjective::HardAndPriority {
            category,
            residual_ids,
        } => linearized_category_system(
            problem,
            plan,
            component.index,
            state,
            category,
            residual_ids,
        )?,
        ComponentIterationObjective::HardAndPriorityResidualTarget {
            category,
            residual_ids,
            target,
        } => {
            let mut priority = linearized_category_system(
                problem,
                plan,
                component.index,
                state,
                category,
                residual_ids,
            )?;
            if priority.residuals.len() != target.len() {
                return Err(CoreError::DimensionMismatch {
                    context: "priority residual-target retraction rows",
                    expected: target.len(),
                    actual: priority.residuals.len(),
                });
            }
            priority.residuals -= target;
            priority
        }
    };
    stack_systems(hard, priority).ok_or(CoreError::DimensionMismatch {
        context: "hard and priority retraction columns",
        expected: plan.component_layouts[component.index].tangent_dimension,
        actual: 0,
    })
}

pub(super) fn stack_systems(first: HardSystem, second: HardSystem) -> Option<HardSystem> {
    if first.jacobian.ncols() != second.jacobian.ncols() {
        return None;
    }
    let first_rows = first.residuals.len();
    let second_rows = second.residuals.len();
    let columns = first.jacobian.ncols();
    let mut residuals = DVector::zeros(first_rows + second_rows);
    residuals
        .rows_mut(0, first_rows)
        .copy_from(&first.residuals);
    residuals
        .rows_mut(first_rows, second_rows)
        .copy_from(&second.residuals);
    let mut jacobian = DMatrix::zeros(first_rows + second_rows, columns);
    jacobian
        .view_mut((0, 0), (first_rows, columns))
        .copy_from(&first.jacobian);
    jacobian
        .view_mut((first_rows, 0), (second_rows, columns))
        .copy_from(&second.jacobian);
    let mut rows = first.rows;
    rows.extend(second.rows);
    Some(HardSystem {
        residuals,
        jacobian,
        rows,
        indexed: None,
    })
}

pub(super) fn linearized_category_system(
    problem: &Problem,
    plan: &EliminationPlan,
    component_index: usize,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
) -> Result<HardSystem, CoreError> {
    let component = plan
        .components
        .get(component_index)
        .ok_or(CoreError::DimensionMismatch {
            context: "priority component index",
            expected: plan.components.len(),
            actual: component_index,
        })?;
    let linearization = problem.linearize_component(plan, component, state, residual_ids)?;
    Ok(component_dense_system(
        linearization.project_dense(plan, category)?,
    ))
}

pub(super) fn linearized_composite_category_system(
    problem: &Problem,
    plan: &EliminationPlan,
    component_indices: &[usize],
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
) -> Result<HardSystem, CoreError> {
    let linearization = ComponentLinearization {
        layout: composite_tangent_layout(plan, component_indices)?,
        numeric: problem.linearize_blocks_for_state(state, Some(residual_ids))?,
    };
    Ok(component_dense_system(
        linearization.project_dense(plan, category)?,
    ))
}

pub(super) fn stack_matrices(first: &DMatrix<f64>, second: &DMatrix<f64>) -> Option<DMatrix<f64>> {
    if first.ncols() != second.ncols() {
        return None;
    }
    let mut stacked = DMatrix::zeros(first.nrows() + second.nrows(), first.ncols());
    stacked.view_mut((0, 0), first.shape()).copy_from(first);
    stacked
        .view_mut((first.nrows(), 0), second.shape())
        .copy_from(second);
    stacked
        .iter()
        .all(|value| value.is_finite())
        .then_some(stacked)
}

pub(super) fn numerical_nullspace(
    matrix: &DMatrix<f64>,
    relative_tolerance: f64,
) -> Option<DMatrix<f64>> {
    let columns = matrix.ncols();
    if columns == 0 {
        return Some(DMatrix::zeros(0, 0));
    }
    if matrix.nrows() == 0 {
        return Some(DMatrix::identity(columns, columns));
    }
    if matrix.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let rows = matrix.nrows().max(columns);
    let mut padded = DMatrix::zeros(rows, columns);
    padded.view_mut((0, 0), matrix.shape()).copy_from(matrix);
    let decomposition = padded.svd(false, true);
    let right_vectors = decomposition.v_t?;
    let largest = decomposition
        .singular_values
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);
    let (_, _, threshold) =
        rank_thresholds(matrix.nrows(), matrix.ncols(), largest, relative_tolerance)?;
    if decomposition
        .singular_values
        .iter()
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let null_indices: Vec<_> = decomposition
        .singular_values
        .iter()
        .enumerate()
        .filter_map(|(index, &value)| (value <= threshold).then_some(index))
        .collect();
    let nullspace = DMatrix::from_fn(columns, null_indices.len(), |row, column| {
        right_vectors[(null_indices[column], row)]
    });
    nullspace
        .iter()
        .all(|value| value.is_finite())
        .then_some(nullspace)
}

pub(super) fn numerical_nullspace_for_rank(
    matrix: &DMatrix<f64>,
    rank: usize,
) -> Option<DMatrix<f64>> {
    let columns = matrix.ncols();
    if rank > columns || matrix.iter().any(|value| !value.is_finite()) {
        return None;
    }
    if columns == 0 {
        return Some(DMatrix::zeros(0, 0));
    }
    if matrix.nrows() == 0 {
        return (rank == 0).then(|| DMatrix::identity(columns, columns));
    }
    let rows = matrix.nrows().max(columns);
    let mut padded = DMatrix::zeros(rows, columns);
    padded.view_mut((0, 0), matrix.shape()).copy_from(matrix);
    let decomposition = padded.svd(false, true);
    if decomposition
        .singular_values
        .iter()
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let right_vectors = decomposition.v_t?;
    let nullspace = DMatrix::from_fn(columns, columns - rank, |row, column| {
        right_vectors[(rank + column, row)]
    });
    nullspace
        .iter()
        .all(|value| value.is_finite())
        .then_some(nullspace)
}

pub(super) fn controlled_rank_kernel<T>(
    rows: usize,
    columns: usize,
    mut control: Option<&mut OperationController>,
    kernel: impl FnOnce() -> Option<T>,
) -> Option<T> {
    let runs_decomposition = rows != 0 && columns != 0;
    if runs_decomposition
        && let Some(controller) = control.as_deref_mut()
        && controller
            .authorize_dense_kernel(rows, columns, OperationCheckpoint::BeforeRankKernel)
            .is_err()
    {
        return None;
    }
    if runs_decomposition
        && let Some(controller) = control.as_deref_mut()
        && controller
            .charge(
                OperationWorkCounter::RankKernels,
                1,
                OperationCheckpoint::BeforeRankKernel,
            )
            .is_err()
    {
        return None;
    }
    let result = kernel();
    if runs_decomposition
        && let Some(controller) = control
        && controller
            .checkpoint(OperationCheckpoint::AfterRankKernel)
            .is_err()
    {
        return None;
    }
    result
}

pub(super) fn controlled_factorization<T>(
    mut control: Option<&mut OperationController>,
    kernel: impl FnOnce() -> Option<T>,
) -> Option<T> {
    if let Some(controller) = control.as_deref_mut()
        && controller
            .charge(
                OperationWorkCounter::Factorizations,
                1,
                OperationCheckpoint::BeforeFactorization,
            )
            .is_err()
    {
        return None;
    }
    let result = kernel();
    if let Some(controller) = control
        && controller
            .checkpoint(OperationCheckpoint::AfterFactorization)
            .is_err()
    {
        return None;
    }
    result
}

pub(super) fn controlled_dense_factorization<T>(
    rows: usize,
    columns: usize,
    mut control: Option<&mut OperationController>,
    kernel: impl FnOnce() -> Option<T>,
) -> Option<T> {
    if let Some(controller) = control.as_deref_mut()
        && controller
            .authorize_dense_kernel(rows, columns, OperationCheckpoint::BeforeFactorization)
            .is_err()
    {
        return None;
    }
    controlled_factorization(control, kernel)
}

pub(super) fn controlled_rank_diagnostics(
    matrix: &DMatrix<f64>,
    relative_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<RankDiagnostics> {
    controlled_rank_kernel(matrix.nrows(), matrix.ncols(), control, || {
        rank_diagnostics(matrix, relative_tolerance)
    })
}

pub(super) fn controlled_numerical_nullspace(
    matrix: &DMatrix<f64>,
    relative_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<DMatrix<f64>> {
    controlled_rank_kernel(matrix.nrows(), matrix.ncols(), control, || {
        numerical_nullspace(matrix, relative_tolerance)
    })
}

pub(super) fn controlled_numerical_nullspace_for_rank(
    matrix: &DMatrix<f64>,
    rank: usize,
    control: Option<&mut OperationController>,
) -> Option<DMatrix<f64>> {
    controlled_rank_kernel(matrix.nrows(), matrix.ncols(), control, || {
        numerical_nullspace_for_rank(matrix, rank)
    })
}
