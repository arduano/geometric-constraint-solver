use super::{
    AuditBoundAnnotation, AuditEvaluationStatus, AuditSnapshot, BoundId, BoundReport, BoundStatus,
    ComponentDenseSystem, ComponentIndexedSystem, ComponentSolveReport, ComponentTangentLayout,
    CoordinateBound, CoreError, DMatrix, DVector, DiagnosticBudget, DiagnosticCompleteness,
    DiagnosticIncompleteReason, DiagnosticStatus, DiagnosticWork, EliminationPlan, HardValidity,
    OneSidedMobility, OperationCheckpoint, OperationController, OperationWorkCounter, Problem,
    RankDiagnostics, RedundancyKind, RedundantRowCandidate, ResidualCategory, ResidualId,
    ResidualRowRef, SolveComponent, SolveTermination, SolverConfig, SourceConstraintId, VariableId,
    VariableState, controlled_numerical_nullspace, controlled_numerical_nullspace_for_rank,
    controlled_rank_diagnostics, empty_rank_diagnostics, enforce_state_bounds, error_termination,
    iterate_component, push_unique, rank_diagnostics, residual_norms, stable_norm,
    worse_termination,
};

#[derive(Debug)]
pub(super) struct ComponentBoundMobility {
    pub(super) bidirectional_dof: usize,
    pub(super) one_sided: OneSidedMobility,
    pub(super) active_bounds: Vec<BoundId>,
}

pub(super) fn bound_reports(
    problem: &Problem,
    state: &VariableState,
) -> Result<Vec<BoundReport>, CoreError> {
    problem
        .bounds
        .iter()
        .map(|(bound_id, bound)| {
            let value = crate::analysis::state_value(state, bound.variable_id())
                .ok_or(CoreError::UnknownVariable(bound.variable_id()))?;
            let value = crate::bounds::coordinate_value(value, bound.coordinate());
            if !bound.contains(value) {
                return Err(CoreError::ValueOutsideBound {
                    variable: bound.variable_id(),
                    coordinate: bound.coordinate(),
                    value,
                    lower: bound.lower(),
                    upper: bound.upper(),
                });
            }
            Ok(BoundReport {
                bound_id,
                variable_id: bound.variable_id(),
                coordinate: bound.coordinate(),
                label: bound.label().to_owned(),
                lower: bound.lower(),
                upper: bound.upper(),
                value,
                status: bound_status(bound, value),
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn component_bound_mobility(
    plan: &EliminationPlan,
    component: &SolveComponent,
    equality: &DMatrix<f64>,
    rank_is_valid: bool,
    equality_rank: usize,
    relative_tolerance: f64,
    reports: &[BoundReport],
    mut control: Option<&mut OperationController>,
) -> ComponentBoundMobility {
    let layout = active_layout(plan, component.index);
    let active_reports: Vec<_> = reports
        .iter()
        .filter(|report| {
            report.status != BoundStatus::Inactive
                && (component.variable_ids.contains(&report.variable_id)
                    || layout
                        .blocks
                        .iter()
                        .any(|block| block.members.contains(&report.variable_id)))
        })
        .collect();
    let active_bounds = active_reports
        .iter()
        .map(|report| report.bound_id)
        .collect();
    if !rank_is_valid || equality.ncols() != layout.tangent_dimension {
        return ComponentBoundMobility {
            bidirectional_dof: 0,
            one_sided: OneSidedMobility::NotEvaluated,
            active_bounds,
        };
    }

    let active_normals = deduplicated_bound_columns(
        active_reports
            .iter()
            .filter_map(|report| bound_column(&layout, report.variable_id, report.coordinate)),
    );
    let Some(equality_nullspace) =
        controlled_numerical_nullspace_for_rank(equality, equality_rank, control.as_deref_mut())
    else {
        return ComponentBoundMobility {
            bidirectional_dof: 0,
            one_sided: OneSidedMobility::NotEvaluated,
            active_bounds,
        };
    };
    let projected_active = projected_coordinate_normals(&equality_nullspace, &active_normals);
    let Some(active_rank) = controlled_rank_diagnostics(
        &projected_active,
        relative_tolerance,
        control.as_deref_mut(),
    ) else {
        return ComponentBoundMobility {
            bidirectional_dof: 0,
            one_sided: OneSidedMobility::NotEvaluated,
            active_bounds,
        };
    };
    let bidirectional_dof = equality_nullspace.ncols().saturating_sub(active_rank.rank);

    let fixed_normals = deduplicated_bound_columns(
        active_reports
            .iter()
            .filter(|report| report.status == BoundStatus::Fixed)
            .filter_map(|report| bound_column(&layout, report.variable_id, report.coordinate)),
    );
    let unilateral: Vec<_> = active_reports
        .iter()
        .filter_map(|report| {
            let column = bound_column(&layout, report.variable_id, report.coordinate)?;
            match report.status {
                BoundStatus::ActiveLower => Some((column, 1.0)),
                BoundStatus::ActiveUpper => Some((column, -1.0)),
                BoundStatus::Inactive | BoundStatus::Fixed => None,
            }
        })
        .collect();
    let projected_fixed = projected_coordinate_normals(&equality_nullspace, &fixed_normals);
    let Some(fixed_nullspace) = controlled_numerical_nullspace(
        &projected_fixed,
        relative_tolerance,
        control.as_deref_mut(),
    ) else {
        return ComponentBoundMobility {
            bidirectional_dof,
            one_sided: OneSidedMobility::NotEvaluated,
            active_bounds,
        };
    };
    let reduced_inequalities =
        DMatrix::from_fn(unilateral.len(), fixed_nullspace.ncols(), |row, column| {
            let (coordinate, sign) = unilateral[row];
            let projected = equality_nullspace.row(coordinate) * &fixed_nullspace;
            sign * projected[column]
        });
    let one_sided = feasible_inequality_cone_has_nonzero_direction(
        &reduced_inequalities,
        relative_tolerance,
        control,
    );
    ComponentBoundMobility {
        bidirectional_dof,
        one_sided,
        active_bounds,
    }
}

pub(super) fn deduplicated_bound_columns(columns: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut deduplicated = Vec::new();
    for column in columns {
        if !deduplicated.contains(&column) {
            deduplicated.push(column);
        }
    }
    deduplicated
}

pub(super) fn projected_coordinate_normals(
    nullspace: &DMatrix<f64>,
    columns: &[usize],
) -> DMatrix<f64> {
    DMatrix::from_fn(columns.len(), nullspace.ncols(), |row, column| {
        nullspace[(columns[row], column)]
    })
}

pub(super) fn feasible_inequality_cone_has_nonzero_direction(
    inequalities: &DMatrix<f64>,
    relative_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> OneSidedMobility {
    let reduced_dimension = inequalities.ncols();
    if reduced_dimension == 0 {
        return OneSidedMobility::None;
    }
    if inequalities.nrows() == 0 {
        return OneSidedMobility::Exists;
    }
    let Some(inequality_rank) =
        controlled_rank_diagnostics(inequalities, relative_tolerance, control.as_deref_mut())
    else {
        return OneSidedMobility::NotEvaluated;
    };
    if inequality_rank.rank < reduced_dimension {
        return OneSidedMobility::Exists;
    }

    let subset_size = reduced_dimension.saturating_sub(1);
    if subset_size == 0 {
        return if direction_satisfies_inequalities(inequalities, &DVector::from_element(1, 1.0))
            || direction_satisfies_inequalities(inequalities, &DVector::from_element(1, -1.0))
        {
            OneSidedMobility::Exists
        } else {
            OneSidedMobility::None
        };
    }
    let mut combination: Vec<_> = (0..subset_size).collect();
    loop {
        let face = DMatrix::from_fn(subset_size, reduced_dimension, |row, column| {
            inequalities[(combination[row], column)]
        });
        let Some(face_nullspace) =
            controlled_numerical_nullspace(&face, relative_tolerance, control.as_deref_mut())
        else {
            return OneSidedMobility::NotEvaluated;
        };
        if face_nullspace.ncols() == 1 {
            let direction = face_nullspace.column(0).into_owned();
            if direction_satisfies_inequalities(inequalities, &direction)
                || direction_satisfies_inequalities(inequalities, &(-direction))
            {
                return OneSidedMobility::Exists;
            }
        }
        if !next_combination(&mut combination, inequalities.nrows()) {
            break;
        }
    }
    OneSidedMobility::None
}

pub(super) fn direction_satisfies_inequalities(
    inequalities: &DMatrix<f64>,
    direction: &DVector<f64>,
) -> bool {
    let Some(norm) = stable_norm(direction.iter().copied()) else {
        return false;
    };
    norm > 64.0 * f64::EPSILON
        && (inequalities * direction)
            .iter()
            .all(|value| *value >= -64.0 * f64::EPSILON * norm)
}

pub(super) fn next_combination(indices: &mut [usize], population: usize) -> bool {
    for position in (0..indices.len()).rev() {
        let maximum = population - (indices.len() - position);
        if indices[position] < maximum {
            indices[position] += 1;
            for next in (position + 1)..indices.len() {
                indices[next] = indices[next - 1] + 1;
            }
            return true;
        }
    }
    false
}

pub(super) fn bound_status(bound: &CoordinateBound, value: f64) -> BoundStatus {
    if bound.lower().is_some() && bound.lower() == bound.upper() {
        BoundStatus::Fixed
    } else if bound
        .lower()
        .is_some_and(|lower| at_bound_endpoint(value, lower))
    {
        BoundStatus::ActiveLower
    } else if bound
        .upper()
        .is_some_and(|upper| at_bound_endpoint(value, upper))
    {
        BoundStatus::ActiveUpper
    } else {
        BoundStatus::Inactive
    }
}

pub(super) fn at_bound_endpoint(value: f64, endpoint: f64) -> bool {
    value.partial_cmp(&endpoint) == Some(std::cmp::Ordering::Equal)
}

pub(super) fn bound_column(
    layout: &ActiveLayout,
    variable_id: VariableId,
    coordinate: usize,
) -> Option<usize> {
    layout.blocks.iter().find_map(|block| {
        block.members.contains(&variable_id).then_some(
            block
                .tangent_range
                .start
                .checked_add(coordinate)
                .filter(|column| *column < block.tangent_range.end),
        )?
    })
}

pub(super) fn aggregate_one_sided_mobility(
    components: &[ComponentSolveReport],
) -> OneSidedMobility {
    if components
        .iter()
        .any(|component| component.one_sided_mobility == OneSidedMobility::NotEvaluated)
    {
        OneSidedMobility::NotEvaluated
    } else if components
        .iter()
        .any(|component| component.one_sided_mobility == OneSidedMobility::Exists)
    {
        OneSidedMobility::Exists
    } else {
        OneSidedMobility::None
    }
}

pub(super) type ActiveLayout = ComponentTangentLayout;

pub(super) fn active_layout(plan: &EliminationPlan, component_index: usize) -> ActiveLayout {
    plan.component_layouts[component_index].clone()
}

#[derive(Debug)]
pub(super) struct HardSystem {
    pub(super) residuals: DVector<f64>,
    pub(super) jacobian: DMatrix<f64>,
    pub(super) rows: Vec<ResidualRowRef>,
    pub(super) indexed: Option<ComponentIndexedSystem>,
}

pub(super) fn component_dense_system(system: ComponentDenseSystem) -> HardSystem {
    HardSystem {
        residuals: system.residuals,
        jacobian: system.jacobian,
        rows: system
            .rows
            .into_iter()
            .map(|row| ResidualRowRef {
                residual_id: row.residual_id,
                row_in_block: row.row_in_block,
                source_id: row.source_id,
            })
            .collect(),
        indexed: None,
    }
}

pub(super) fn apply_normalized_step(
    problem: &Problem,
    plan: &EliminationPlan,
    state: &mut VariableState,
    layout: &ActiveLayout,
    step: &DVector<f64>,
) -> Result<(), CoreError> {
    for block in &layout.blocks {
        let (_, value) = state
            .values
            .iter_mut()
            .find(|(id, _)| *id == block.root)
            .ok_or(CoreError::UnknownVariable(block.root))?;
        let raw_delta: Vec<_> = block
            .step_scales
            .iter()
            .enumerate()
            .map(|(column, scale)| step[block.tangent_range.start + column] * scale)
            .collect();
        value.plus(&raw_delta)?;
    }
    plan.synchronize_state(problem, state)?;
    enforce_state_bounds(problem, plan, state)
}

pub(super) struct ComponentValidation {
    pub(super) evaluated: bool,
    pub(super) valid: bool,
    pub(super) hard_validity: HardValidity,
    pub(super) maximum: f64,
    pub(super) l2: f64,
    pub(super) termination: SolveTermination,
    pub(super) rows: Vec<(ResidualId, usize, SourceConstraintId, f64)>,
}

pub(super) fn validate_component(
    problem: &Problem,
    component: &SolveComponent,
    state: &VariableState,
    config: SolverConfig,
) -> ComponentValidation {
    match problem.normalized_category_values_for_residuals(
        state,
        ResidualCategory::Hard,
        &component.residual_ids,
    ) {
        Ok(rows) => {
            let maximum = rows.iter().map(|row| row.3.abs()).fold(0.0, f64::max);
            if let Some((_, l2)) = residual_norms(rows.iter().map(|row| row.3)) {
                ComponentValidation {
                    evaluated: true,
                    valid: maximum <= config.normalized_residual_tolerance,
                    hard_validity: if maximum <= config.normalized_residual_tolerance {
                        HardValidity::Valid
                    } else {
                        HardValidity::Invalid
                    },
                    maximum,
                    l2,
                    termination: SolveTermination::Converged,
                    rows,
                }
            } else {
                ComponentValidation {
                    evaluated: false,
                    valid: false,
                    hard_validity: HardValidity::NotEvaluated,
                    maximum,
                    l2: 0.0,
                    termination: SolveTermination::NumericalFailure,
                    rows,
                }
            }
        }
        Err(error) => {
            let hard_validity = if matches!(
                error,
                CoreError::InvalidGeometry { .. } | CoreError::CategorizedEvaluation { .. }
            ) {
                HardValidity::Invalid
            } else {
                HardValidity::NotEvaluated
            };
            ComponentValidation {
                evaluated: false,
                valid: false,
                hard_validity,
                maximum: 0.0,
                l2: 0.0,
                termination: error_termination(&error),
                rows: Vec::new(),
            }
        }
    }
}

pub(super) struct ComponentNumerics {
    pub(super) termination: SolveTermination,
    pub(super) rank_is_valid: bool,
    pub(super) is_singular: bool,
    pub(super) diagnostics: RankDiagnostics,
    pub(super) singular_rows: Vec<ResidualRowRef>,
    pub(super) hard: HardSystem,
}

pub(super) fn component_numerics(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    config: SolverConfig,
) -> ComponentNumerics {
    let summary = &plan.structural.component_summaries[component.index];
    let empty = || HardSystem {
        residuals: DVector::zeros(0),
        jacobian: DMatrix::zeros(0, summary.active_tangent_dimensions),
        rows: Vec::new(),
        indexed: None,
    };
    let projections = (|| {
        let linearization =
            problem.linearize_component(plan, component, state, &component.active_residual_ids)?;
        let dense = linearization.project_dense(plan, ResidualCategory::Hard)?;
        let indexed = linearization.project_indexed(plan, ResidualCategory::Hard)?;
        Ok::<_, CoreError>((dense, indexed))
    })();
    let (dense, indexed) = match projections {
        Ok(projections) => projections,
        Err(error) => {
            return ComponentNumerics {
                termination: error_termination(&error),
                rank_is_valid: false,
                is_singular: false,
                diagnostics: empty_rank_diagnostics(
                    summary.active_hard_rows,
                    summary.active_tangent_dimensions,
                    config.rank_relative_tolerance,
                ),
                singular_rows: Vec::new(),
                hard: empty(),
            };
        }
    };
    if indexed.sparsity_signature != summary.sparsity_signature
        || !indexed.numerically_matches(&dense)
    {
        return ComponentNumerics {
            termination: SolveTermination::NumericalFailure,
            rank_is_valid: false,
            is_singular: false,
            diagnostics: empty_rank_diagnostics(
                summary.active_hard_rows,
                summary.active_tangent_dimensions,
                config.rank_relative_tolerance,
            ),
            singular_rows: Vec::new(),
            hard: empty(),
        };
    }
    let hard = component_dense_system(dense);
    let Some(rank) = rank_diagnostics(&hard.jacobian, config.rank_relative_tolerance) else {
        return ComponentNumerics {
            termination: SolveTermination::NumericalFailure,
            rank_is_valid: false,
            is_singular: false,
            diagnostics: empty_rank_diagnostics(
                hard.jacobian.nrows(),
                hard.jacobian.ncols(),
                config.rank_relative_tolerance,
            ),
            singular_rows: Vec::new(),
            hard,
        };
    };
    let is_singular = rank.rank < hard.jacobian.nrows().min(hard.jacobian.ncols());
    let singular_rows = find_singular_rows(&hard, rank.threshold, is_singular);
    ComponentNumerics {
        termination: SolveTermination::Converged,
        rank_is_valid: true,
        is_singular,
        diagnostics: rank,
        singular_rows,
        hard,
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn find_conflicting_sources(
    problem: &Problem,
    plan: &EliminationPlan,
    returned_state: &VariableState,
    config: SolverConfig,
    component_reports: &[ComponentSolveReport],
    mut control: Option<&mut OperationController>,
) -> Option<(Vec<SourceConstraintId>, DiagnosticCompleteness)> {
    let budget = config.conflict_diagnostic_budget;
    if !budget.enabled {
        return Some((
            Vec::new(),
            diagnostic_completeness(
                budget,
                DiagnosticWork::default(),
                Some(DiagnosticIncompleteReason::Disabled),
                false,
            ),
        ));
    }
    let failed_components: Vec<_> = plan
        .components
        .iter()
        .filter(|component| {
            let report = &component_reports[component.index];
            !report.hard_residuals_validated
                || report.hard_residual_max > config.normalized_residual_tolerance
        })
        .collect();
    if failed_components.is_empty() {
        return Some((
            Vec::new(),
            diagnostic_completeness(
                budget,
                DiagnosticWork::default(),
                Some(DiagnosticIncompleteReason::HardConstraintsValid),
                false,
            ),
        ));
    }
    let mut work = DiagnosticWork::default();
    let mut reason = None;
    let mut eligible_components = Vec::new();
    for component in failed_components {
        let report = &component_reports[component.index];
        if !report.hard_residuals_validated {
            reason.get_or_insert(DiagnosticIncompleteReason::InvalidEvaluation);
            continue;
        }
        if !report.rank_is_valid {
            reason.get_or_insert(DiagnosticIncompleteReason::InvalidRank);
            continue;
        }
        let candidate_count = candidate_sources(problem, component).len();
        if let Some(component_reason) = diagnostic_component_budget_reason(
            &plan.structural.component_summaries[component.index],
            candidate_count,
            work.trials,
            budget,
        ) {
            reason.get_or_insert(component_reason);
            continue;
        }
        work.components += 1;
        let summary = &plan.structural.component_summaries[component.index];
        work.tangent_dimensions = work
            .tangent_dimensions
            .saturating_add(summary.active_tangent_dimensions);
        work.scalar_rows = work.scalar_rows.saturating_add(summary.active_hard_rows);
        work.candidate_sources += candidate_count;
        eligible_components.push(component);
    }
    let mut candidates = Vec::new();
    let mut stopped = false;
    for source in problem.source_order() {
        if let Some(controller) = control.as_deref_mut()
            && controller
                .charge(
                    OperationWorkCounter::DiagnosticCandidates,
                    1,
                    OperationCheckpoint::DiagnosticCandidate,
                )
                .is_err()
        {
            return None;
        }
        for component in &eligible_components {
            if !source_affects_component(problem, source, component) {
                continue;
            }
            if work.trials >= budget.max_trials {
                reason.get_or_insert(DiagnosticIncompleteReason::TrialBudget);
                stopped = true;
                break;
            }
            work.trials += 1;
            if let Some(controller) = control.as_deref_mut()
                && controller
                    .charge(
                        OperationWorkCounter::DiagnosticTrials,
                        1,
                        OperationCheckpoint::DiagnosticTrial,
                    )
                    .is_err()
            {
                return None;
            }
            let restores = deletion_restores_component(
                problem,
                plan,
                component,
                source,
                returned_state,
                config,
                control.as_deref_mut(),
            )?;
            if restores {
                candidates.push(source);
                break;
            }
        }
        if stopped {
            break;
        }
    }
    let analyzed = !eligible_components.is_empty();
    Some((
        candidates,
        diagnostic_completeness(budget, work, reason, analyzed),
    ))
}

pub(super) fn diagnostic_component_budget_reason(
    summary: &crate::ComponentStructuralSummary,
    candidate_sources: usize,
    consumed_trials: usize,
    budget: DiagnosticBudget,
) -> Option<DiagnosticIncompleteReason> {
    if !budget.enabled {
        Some(DiagnosticIncompleteReason::Disabled)
    } else if summary.active_tangent_dimensions > budget.max_component_tangent_dimension {
        Some(DiagnosticIncompleteReason::ComponentTangentBudget)
    } else if summary.active_hard_rows > budget.max_component_scalar_rows {
        Some(DiagnosticIncompleteReason::ComponentRowBudget)
    } else if candidate_sources > budget.max_candidate_sources {
        Some(DiagnosticIncompleteReason::CandidateSourceBudget)
    } else if consumed_trials >= budget.max_trials {
        Some(DiagnosticIncompleteReason::TrialBudget)
    } else {
        None
    }
}

pub(super) fn diagnostic_completeness(
    budget: DiagnosticBudget,
    consumed: DiagnosticWork,
    reason: Option<DiagnosticIncompleteReason>,
    analyzed: bool,
) -> DiagnosticCompleteness {
    DiagnosticCompleteness {
        status: match (reason, analyzed) {
            (None, _) => DiagnosticStatus::Complete,
            (Some(_), true) => DiagnosticStatus::Truncated,
            (Some(_), false) => DiagnosticStatus::Skipped,
        },
        budget,
        consumed,
        reason,
    }
}

pub(super) fn candidate_sources(
    problem: &Problem,
    component: &SolveComponent,
) -> Vec<SourceConstraintId> {
    problem
        .source_order()
        .into_iter()
        .filter(|&source| source_affects_component(problem, source, component))
        .collect()
}

pub(super) fn source_affects_component(
    problem: &Problem,
    source: SourceConstraintId,
    component: &SolveComponent,
) -> bool {
    let owns_residual = problem.residuals.iter().any(|(residual_id, residual)| {
        residual.category() == ResidualCategory::Hard
            && residual.source() == source
            && component.residual_ids.contains(&residual_id)
    });
    if owns_residual {
        return true;
    }
    let controls_variable = |variable: VariableId| {
        component.referenced_variables.contains(&variable)
            || component.variable_ids.contains(&variable)
    };
    problem.fixed_eliminations.iter().any(|fixed| {
        problem
            .residual(fixed.residual_id)
            .is_some_and(|residual| residual.source() == source)
            && controls_variable(fixed.variable_id)
    }) || problem.alias_eliminations.iter().any(|alias| {
        problem
            .residual(alias.residual_id)
            .is_some_and(|residual| residual.source() == source)
            && (controls_variable(alias.alias) || controls_variable(alias.representative))
    })
}

pub(super) fn deletion_restores_component(
    problem: &Problem,
    _normal_plan: &EliminationPlan,
    failed_component: &SolveComponent,
    source: SourceConstraintId,
    returned_state: &VariableState,
    config: SolverConfig,
    mut control: Option<&mut OperationController>,
) -> Option<bool> {
    let Ok(trial_plan) = EliminationPlan::new_suppressed(problem, &[source]) else {
        return Some(false);
    };
    let mut state = returned_state.clone();
    if trial_plan.synchronize_state(problem, &mut state).is_err() {
        return Some(false);
    }
    let affected_variables: Vec<_> = failed_component
        .variable_ids
        .iter()
        .chain(&failed_component.referenced_variables)
        .copied()
        .fold(Vec::new(), |mut values, value| {
            push_unique(&mut values, value);
            values
        });
    let trial_components: Vec<_> = trial_plan
        .components
        .iter()
        .filter(|component| {
            component
                .variable_ids
                .iter()
                .chain(&component.referenced_variables)
                .any(|variable| affected_variables.contains(variable))
                || component
                    .residual_ids
                    .iter()
                    .any(|residual| failed_component.residual_ids.contains(residual))
        })
        .collect();
    for component in trial_components {
        let outcome = iterate_component(
            problem,
            &trial_plan,
            component,
            state,
            config,
            control.as_deref_mut(),
        )?;
        state = outcome.state;
        if matches!(
            outcome.termination,
            SolveTermination::InvalidGeometry | SolveTermination::NumericalFailure
        ) {
            return Some(false);
        }
    }
    let remaining_residuals: Vec<_> = failed_component
        .residual_ids
        .iter()
        .copied()
        .filter(|&residual_id| {
            problem
                .residual(residual_id)
                .is_some_and(|residual| residual.source() != source)
        })
        .collect();
    Some(
        problem
            .normalized_category_values_for_residuals(
                &state,
                ResidualCategory::Hard,
                &remaining_residuals,
            )
            .ok()
            .is_some_and(|rows| {
                rows.iter()
                    .all(|row| row.3.abs() <= config.normalized_residual_tolerance)
            }),
    )
}

#[derive(Default)]
pub(super) struct RedundancyDiagnostics {
    pub(super) rows: Vec<RedundantRowCandidate>,
}

#[allow(clippy::too_many_lines)]
pub(super) fn find_redundancy(
    hard: &HardSystem,
    validated_rows: &[(ResidualId, usize, SourceConstraintId, f64)],
    source_order: &[SourceConstraintId],
    threshold: f64,
    residual_tolerance: f64,
    mut control: Option<&mut OperationController>,
) -> Option<RedundancyDiagnostics> {
    let mut diagnostics = RedundancyDiagnostics::default();
    let mut prior_source_rows = Vec::new();
    let mut prior_rank = 0;
    for &source in source_order {
        let source_rows: Vec<_> = hard
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| (row.source_id == source).then_some(index))
            .collect();
        if source_rows.is_empty() {
            continue;
        }
        if let Some(controller) = control.as_deref_mut()
            && controller
                .charge(
                    OperationWorkCounter::DiagnosticCandidates,
                    1,
                    OperationCheckpoint::DiagnosticCandidate,
                )
                .is_err()
        {
            return None;
        }
        let all_nonzero = source_rows
            .iter()
            .all(|&row| row_is_nonzero(hard, row, threshold));
        let all_satisfied = source_rows.iter().all(|&row| {
            validated_value(validated_rows, hard.rows[row])
                .is_some_and(|value| value.abs() <= residual_tolerance)
        });
        let mut combined = prior_source_rows.clone();
        combined.extend_from_slice(&source_rows);
        let combined_rank = controlled_selected_row_rank(
            &hard.jacobian,
            &combined,
            threshold,
            control.as_deref_mut(),
        )?;
        let fully_redundant =
            prior_rank > 0 && all_nonzero && all_satisfied && combined_rank == prior_rank;
        if fully_redundant {
            diagnostics
                .rows
                .extend(source_rows.iter().map(|&row| RedundantRowCandidate {
                    row: hard.rows[row],
                    kind: RedundancyKind::SeparateSource,
                }));
        } else {
            let mut earlier_source_rows = Vec::new();
            let mut basis = prior_source_rows.clone();
            let mut basis_rank = prior_rank;
            for (position, &row) in source_rows.iter().enumerate() {
                let before = basis_rank;
                let mut with_row = basis.clone();
                with_row.push(row);
                let after = if position + 1 == source_rows.len() {
                    combined_rank
                } else {
                    controlled_selected_row_rank(
                        &hard.jacobian,
                        &with_row,
                        threshold,
                        control.as_deref_mut(),
                    )?
                };
                if before > 0
                    && row_is_nonzero(hard, row, threshold)
                    && validated_value(validated_rows, hard.rows[row])
                        .is_some_and(|value| value.abs() <= residual_tolerance)
                    && after == before
                {
                    let local_before = controlled_selected_row_rank(
                        &hard.jacobian,
                        &earlier_source_rows,
                        threshold,
                        control.as_deref_mut(),
                    )?;
                    let mut local_with = earlier_source_rows.clone();
                    local_with.push(row);
                    let local_after = controlled_selected_row_rank(
                        &hard.jacobian,
                        &local_with,
                        threshold,
                        control.as_deref_mut(),
                    )?;
                    diagnostics.rows.push(RedundantRowCandidate {
                        row: hard.rows[row],
                        kind: if local_before > 0 && local_after == local_before {
                            RedundancyKind::WithinSource
                        } else {
                            RedundancyKind::SeparateSource
                        },
                    });
                }
                basis.push(row);
                basis_rank = after;
                earlier_source_rows.push(row);
            }
        }
        prior_source_rows = combined;
        prior_rank = combined_rank;
    }
    Some(diagnostics)
}

pub(super) fn controlled_selected_row_rank(
    jacobian: &DMatrix<f64>,
    rows: &[usize],
    threshold: f64,
    mut controller: Option<&mut OperationController>,
) -> Option<usize> {
    if let Some(controller) = controller.as_deref_mut()
        && controller
            .charge(
                OperationWorkCounter::DiagnosticTrials,
                1,
                OperationCheckpoint::DiagnosticTrial,
            )
            .is_err()
    {
        return None;
    }
    if let Some(controller) = controller.as_deref_mut()
        && controller
            .authorize_dense_kernel(
                rows.len(),
                jacobian.ncols(),
                OperationCheckpoint::BeforeRankKernel,
            )
            .is_err()
    {
        return None;
    }
    if let Some(controller) = controller.as_deref_mut()
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
    let rank = selected_row_rank(jacobian, rows, threshold);
    if let Some(controller) = controller
        && controller
            .checkpoint(OperationCheckpoint::AfterRankKernel)
            .is_err()
    {
        return None;
    }
    Some(rank)
}

pub(super) fn find_singular_rows(
    hard: &HardSystem,
    threshold: f64,
    component_is_singular: bool,
) -> Vec<ResidualRowRef> {
    if component_is_singular {
        return hard.rows.clone();
    }
    hard.rows
        .iter()
        .enumerate()
        .filter_map(|(row, &row_ref)| {
            stable_norm(hard.jacobian.row(row).iter().copied())
                .is_some_and(|norm| norm <= threshold)
                .then_some(row_ref)
        })
        .collect()
}

pub(super) fn globally_fully_redundant_sources(
    problem: &Problem,
    plan: &EliminationPlan,
    redundant_rows: &[RedundantRowCandidate],
) -> Vec<SourceConstraintId> {
    ordered_sources(problem, |source| {
        let active_rows: Vec<_> = plan
            .components
            .iter()
            .flat_map(|component| component.active_residual_ids.iter().copied())
            .filter_map(|residual_id| {
                problem
                    .residual(residual_id)
                    .filter(|residual| residual.source() == source)
                    .map(|residual| (residual_id, residual.output_dimension()))
            })
            .flat_map(|(residual_id, dimension)| {
                (0..dimension).map(move |row_in_block| ResidualRowRef {
                    residual_id,
                    row_in_block,
                    source_id: source,
                })
            })
            .collect();
        !active_rows.is_empty()
            && active_rows
                .iter()
                .all(|row| redundant_rows.iter().any(|candidate| candidate.row == *row))
    })
}

pub(super) struct RowEvaluationFailure {
    pub(super) residual_id: ResidualId,
    pub(super) category: Option<crate::EvaluationErrorCategory>,
    pub(super) error: String,
}

pub(super) struct ReturnedEvaluation {
    pub(super) termination: SolveTermination,
    pub(super) failures: Vec<RowEvaluationFailure>,
}

pub(super) fn validate_returned_rows(
    problem: &Problem,
    state: &VariableState,
    residual_filter: Option<&[ResidualId]>,
) -> ReturnedEvaluation {
    let mut termination = SolveTermination::Converged;
    let mut failures = Vec::new();
    for (residual_id, _) in problem.residuals.iter() {
        if residual_filter.is_some_and(|filter| !filter.contains(&residual_id)) {
            continue;
        }
        if let Err(error) = problem.validate_residual_linearization(state, residual_id) {
            termination = worse_termination(termination, error_termination(&error));
            failures.push(RowEvaluationFailure {
                residual_id,
                category: match &error {
                    CoreError::CategorizedEvaluation { category, .. } => Some(*category),
                    _ => None,
                },
                error: error.to_string(),
            });
        }
    }
    ReturnedEvaluation {
        termination,
        failures,
    }
}

pub(super) fn annotate_evaluation_failures(
    audit: &mut AuditSnapshot,
    failures: &[RowEvaluationFailure],
) {
    for source in &mut audit.sources {
        for row in &mut source.rows {
            if let Some(failure) = failures
                .iter()
                .find(|failure| failure.residual_id == row.residual_id)
            {
                row.evaluation_status = AuditEvaluationStatus::Failed;
                row.evaluation_error_category = failure.category;
                row.evaluation_error = Some(failure.error.clone());
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn annotate_audit(
    audit: &mut AuditSnapshot,
    plan: &EliminationPlan,
    redundant_rows: &[RedundantRowCandidate],
    conflicting_sources: &[SourceConstraintId],
    singular_rows: &[ResidualRowRef],
    bounds: &[BoundReport],
    redundancy_diagnostics: DiagnosticCompleteness,
    conflict_diagnostics: DiagnosticCompleteness,
) {
    for source in &mut audit.sources {
        for row in &mut source.rows {
            let row_ref = ResidualRowRef {
                residual_id: row.residual_id,
                row_in_block: row.row_in_block,
                source_id: source.source_id,
            };
            row.annotations.eliminated = plan.is_eliminated(row.residual_id);
            row.annotations.suppressed = row.annotations.eliminated;
            row.annotations.redundant = redundant_rows.iter().any(|item| item.row == row_ref);
            row.annotations.conflicting = conflicting_sources.contains(&source.source_id);
            row.annotations.singular = singular_rows.contains(&row_ref);
            row.active_bounds = bounds
                .iter()
                .filter(|bound| {
                    bound.status != BoundStatus::Inactive
                        && row
                            .incident_variables
                            .iter()
                            .any(|variable| variable.variable_id == bound.variable_id)
                })
                .map(|bound| AuditBoundAnnotation {
                    bound_id: bound.bound_id,
                    variable_id: bound.variable_id,
                    coordinate: bound.coordinate,
                    status: bound.status,
                })
                .collect();
            row.annotations.active_bound = !row.active_bounds.is_empty();
            row.annotations.redundancy_diagnostics = Some(redundancy_diagnostics);
            row.annotations.conflict_diagnostics = Some(conflict_diagnostics);
        }
        source.annotations.eliminated = source.rows.iter().any(|row| row.annotations.eliminated);
        source.annotations.suppressed = source.rows.iter().any(|row| row.annotations.suppressed);
        source.annotations.redundant = source.rows.iter().any(|row| row.annotations.redundant);
        source.annotations.conflicting = conflicting_sources.contains(&source.source_id)
            || source.rows.iter().any(|row| row.annotations.conflicting);
        source.annotations.singular = source.rows.iter().any(|row| row.annotations.singular);
        source.annotations.active_bound =
            source.rows.iter().any(|row| row.annotations.active_bound);
        source.annotations.redundancy_diagnostics = Some(redundancy_diagnostics);
        source.annotations.conflict_diagnostics = Some(conflict_diagnostics);
        source.active_bounds.clear();
        for bound in source
            .rows
            .iter()
            .flat_map(|row| row.active_bounds.iter().copied())
        {
            if !source
                .active_bounds
                .iter()
                .any(|current| current.bound_id == bound.bound_id)
            {
                source.active_bounds.push(bound);
            }
        }
    }
}

pub(super) fn row_is_nonzero(hard: &HardSystem, row: usize, threshold: f64) -> bool {
    stable_norm(hard.jacobian.row(row).iter().copied()).is_some_and(|norm| norm > threshold)
}

pub(super) fn validated_value(
    validated_rows: &[(ResidualId, usize, SourceConstraintId, f64)],
    row: ResidualRowRef,
) -> Option<f64> {
    validated_rows
        .iter()
        .find(|item| item.0 == row.residual_id && item.1 == row.row_in_block)
        .map(|item| item.3)
}

pub(super) fn selected_row_rank(matrix: &DMatrix<f64>, rows: &[usize], threshold: f64) -> usize {
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

pub(super) fn sort_redundancy(problem: &Problem, rows: &mut [RedundantRowCandidate]) {
    let sources = problem.source_order();
    let residuals: Vec<_> = problem.residuals.iter().map(|(id, _)| id).collect();
    rows.sort_by_key(|candidate| {
        (
            sources
                .iter()
                .position(|source| *source == candidate.row.source_id)
                .unwrap_or(usize::MAX),
            residuals
                .iter()
                .position(|residual| *residual == candidate.row.residual_id)
                .unwrap_or(usize::MAX),
            candidate.row.row_in_block,
        )
    });
}

pub(super) fn ordered_sources(
    problem: &Problem,
    predicate: impl Fn(SourceConstraintId) -> bool,
) -> Vec<SourceConstraintId> {
    problem
        .source_order()
        .into_iter()
        .filter(|&source| predicate(source))
        .collect()
}

pub(super) fn deduplicate_rows(rows: &mut Vec<ResidualRowRef>) {
    let mut unique = Vec::new();
    for row in rows.drain(..) {
        if !unique.contains(&row) {
            unique.push(row);
        }
    }
    *rows = unique;
}
