use std::ops::Range;

use nalgebra::{DMatrix, DVector};

use crate::analysis::{
    CachedComponent, DecompositionCache, EliminationPlan, SolveComponent, set_state_value,
};
use crate::problem::VariableState;
use crate::{
    AuditEvaluationStatus, AuditSnapshot, CoreError, DenseAssembly, Problem, ResidualCategory,
    ResidualId, SourceConstraintId, StructuralSummary, VariableId,
};

const MAX_CONFLICT_COMPONENT_SOURCES: usize = 12;
const MAX_CONFLICT_COMPONENT_DIMENSION: usize = 24;

/// Why nonlinear iteration stopped. Constraint-system diagnostics are separate.
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
    /// Reduced component owning this cost sequence. `None` is used only by an
    /// internal trace before it is attached to a component.
    pub component_index: Option<usize>,
    pub iteration: usize,
    pub accepted: bool,
    pub trial_valid: bool,
    pub cost_before: f64,
    pub trial_cost: f64,
    /// Cost of this component's accepted state after this record.
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

/// Stable identity of one generated scalar residual row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidualRowRef {
    pub residual_id: ResidualId,
    pub row_in_block: usize,
    pub source_id: SourceConstraintId,
}

/// Whether a dependent row is explained within its source or by prior sources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedundancyKind {
    WithinSource,
    SeparateSource,
}

/// A deterministic, nonzero, satisfied row redundancy candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedundantRowCandidate {
    pub row: ResidualRowRef,
    pub kind: RedundancyKind,
}

/// Numerical outcome for one reduced solve component.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ComponentSolveReport {
    pub component_index: usize,
    pub pattern_signature: u64,
    pub reused: bool,
    pub iterations: usize,
    pub termination: SolveTermination,
    pub hard_residuals_validated: bool,
    pub hard_residual_max: f64,
    pub rank_is_valid: bool,
    pub rank: usize,
    pub local_degrees_of_freedom: usize,
    pub is_singular: bool,
    pub rank_threshold: f64,
    pub singular_values: Vec<f64>,
    /// Component-local costs. Reused components always have an empty trace.
    pub trace: SolveTrace,
}

/// Numerical and structural facts evaluated at the returned state.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveReport {
    pub termination: SolveTermination,
    pub iterations: usize,
    pub accepted_state: crate::PackedState,
    pub hard_residuals_validated: bool,
    pub hard_residual_max: f64,
    pub hard_residual_l2: f64,
    pub rank_is_valid: bool,
    pub rank: usize,
    pub local_degrees_of_freedom: usize,
    pub is_singular: bool,
    pub rank_relative_tolerance: f64,
    /// Maximum component-local rank threshold; component reports retain each threshold.
    pub rank_threshold: f64,
    /// Component singular values concatenated in reduced component order.
    pub singular_values: Vec<f64>,
    pub conflicting_sources: Vec<SourceConstraintId>,
    /// Sources whose complete active row group is redundant to prior sources.
    pub redundant_sources: Vec<SourceConstraintId>,
    /// Sources containing at least one redundant row, including partial groups.
    pub sources_containing_redundant_rows: Vec<SourceConstraintId>,
    pub redundant_rows: Vec<RedundantRowCandidate>,
    pub singular_rows: Vec<ResidualRowRef>,
    pub structural: StructuralSummary,
    pub component_solves: Vec<ComponentSolveReport>,
    /// Combined records carry component identity; costs are not cross-component sequences.
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
    /// Returns [`CoreError::InvalidSolverConfig`] for invalid policy data.
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
    /// Solves every reduced hard component independently without cache reuse.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration or stale/invalid static
    /// declarations. Evaluator and numerical failures remain report outcomes.
    pub fn solve(&mut self, config: SolverConfig) -> Result<SolveReport, CoreError> {
        self.solve_reduced(config, None)
    }

    /// Solves edited/cache-invalid components and reuses independently validated cache entries.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration, unknown edited IDs, or
    /// stale/invalid static declarations.
    pub fn solve_decomposed(
        &mut self,
        config: SolverConfig,
        edited_variables: &[VariableId],
    ) -> Result<SolveReport, CoreError> {
        self.solve_reduced(config, Some(edited_variables))
    }

    fn solve_reduced(
        &mut self,
        config: SolverConfig,
        edited_variables: Option<&[VariableId]>,
    ) -> Result<SolveReport, CoreError> {
        config.validate()?;
        let plan = EliminationPlan::new(self)?;
        let mut edited_components = Vec::new();
        if let Some(edited_variables) = edited_variables {
            for &variable_id in edited_variables {
                let component = plan
                    .component_for_variable(variable_id)
                    .ok_or(CoreError::UnknownVariable(variable_id))?;
                push_unique(&mut edited_components, component);
            }
        }

        let prior_cache = self.decomposition_cache.clone().unwrap_or_default();
        let mut state = self.variable_state();
        plan.synchronize_state(self, &mut state)?;
        let mut executions = Vec::with_capacity(plan.components.len());
        for component in &plan.components {
            let may_reuse =
                edited_variables.is_some() && !edited_components.contains(&component.index);
            let cached = prior_cache
                .components
                .iter()
                .find(|cached| cache_matches(cached, &plan, component));
            if may_reuse
                && let Some(cached_state) = cached.and_then(|cached| {
                    validated_cached_state(self, &plan, component, &state, cached, config)
                })
            {
                state = cached_state;
                executions.push(ComponentExecution {
                    component_index: component.index,
                    reused: true,
                    termination: SolveTermination::Converged,
                    trace: SolveTrace::default(),
                });
                continue;
            }

            let mut outcome = iterate_component(self, &plan, component, state, config);
            stamp_component_trace(&mut outcome.trace, component.index);
            state = outcome.state;
            executions.push(ComponentExecution {
                component_index: component.index,
                reused: false,
                termination: outcome.termination,
                trace: outcome.trace,
            });
        }

        self.replace_variable_state(&state)?;
        let report = self.build_report(config, &plan, &executions)?;
        self.update_decomposition_cache(&plan, &report)?;
        Ok(report)
    }

    #[allow(clippy::too_many_lines)]
    fn build_report(
        &self,
        config: SolverConfig,
        plan: &EliminationPlan,
        executions: &[ComponentExecution],
    ) -> Result<SolveReport, CoreError> {
        let state = self.variable_state();
        let accepted_state = self.packed_state()?;
        let mut component_solves = Vec::with_capacity(plan.components.len());
        let mut all_redundancy = Vec::new();
        let mut singular_rows = Vec::new();
        let mut hard_residual_l2 = 0.0_f64;
        let mut hard_l2_is_valid = true;

        for component in &plan.components {
            let execution = executions
                .iter()
                .find(|execution| execution.component_index == component.index)
                .ok_or(CoreError::DimensionMismatch {
                    context: "component execution report",
                    expected: plan.components.len(),
                    actual: executions.len(),
                })?;
            let validation = validate_component(self, component, &state, config);
            if validation.evaluated {
                let combined = hard_residual_l2.hypot(validation.l2);
                if combined.is_finite() {
                    hard_residual_l2 = combined;
                } else {
                    hard_l2_is_valid = false;
                    hard_residual_l2 = f64::MAX;
                }
            }
            let numerical = component_numerics(self, plan, component, &state, config);
            let mut termination = worse_termination(execution.termination, numerical.termination);
            if !validation.evaluated {
                termination = worse_termination(termination, validation.termination);
            } else if validation.maximum > config.normalized_residual_tolerance
                && termination == SolveTermination::Converged
            {
                termination = SolveTermination::NumericalFailure;
            } else if validation.valid
                && numerical.rank_is_valid
                && !matches!(
                    termination,
                    SolveTermination::InvalidGeometry | SolveTermination::NumericalFailure
                )
            {
                termination = SolveTermination::Converged;
            }

            if validation.valid && numerical.rank_is_valid {
                let redundancy = find_redundancy(
                    &numerical.hard,
                    &validation.rows,
                    &self.source_order(),
                    numerical.rank_threshold,
                    config.normalized_residual_tolerance,
                );
                all_redundancy.extend(redundancy.rows);
            }
            singular_rows.extend(numerical.singular_rows.iter().copied());
            let summary = &plan.structural.component_summaries[component.index];
            component_solves.push(ComponentSolveReport {
                component_index: component.index,
                pattern_signature: summary.pattern_signature,
                reused: execution.reused,
                iterations: execution.trace.records.len(),
                termination,
                hard_residuals_validated: validation.evaluated,
                hard_residual_max: validation.maximum,
                rank_is_valid: numerical.rank_is_valid,
                rank: numerical.rank,
                local_degrees_of_freedom: summary
                    .active_tangent_dimensions
                    .saturating_sub(numerical.rank),
                is_singular: numerical.is_singular,
                rank_threshold: numerical.rank_threshold,
                singular_values: numerical.singular_values,
                trace: execution.trace.clone(),
            });
        }

        sort_redundancy(self, &mut all_redundancy);
        deduplicate_rows(&mut singular_rows);
        let redundant_sources = globally_fully_redundant_sources(self, plan, &all_redundancy);
        let sources_containing_redundant_rows = ordered_sources(self, |source| {
            all_redundancy
                .iter()
                .any(|candidate| candidate.row.source_id == source)
        });
        let conflicting_sources =
            find_conflicting_sources(self, plan, &state, config, &component_solves);

        let hard_residuals_validated = hard_l2_is_valid
            && component_solves
                .iter()
                .all(|component| component.hard_residuals_validated);
        let hard_residual_max = component_solves
            .iter()
            .map(|component| component.hard_residual_max)
            .fold(0.0, f64::max);
        let rank_is_valid = component_solves
            .iter()
            .all(|component| component.rank_is_valid);
        let rank = component_solves
            .iter()
            .map(|component| component.rank)
            .sum();
        let local_degrees_of_freedom = component_solves
            .iter()
            .map(|component| component.local_degrees_of_freedom)
            .sum();
        let is_singular = component_solves
            .iter()
            .any(|component| component.is_singular);
        let rank_threshold = component_solves
            .iter()
            .map(|component| component.rank_threshold)
            .fold(0.0, f64::max);
        let singular_values = component_solves
            .iter()
            .flat_map(|component| component.singular_values.iter().copied())
            .collect();
        let returned_evaluation = validate_returned_rows(self, &state);
        let mut termination = component_solves
            .iter()
            .map(|component| component.termination)
            .fold(SolveTermination::Converged, worse_termination);
        if !hard_l2_is_valid {
            termination = worse_termination(termination, SolveTermination::NumericalFailure);
        }
        termination = worse_termination(termination, returned_evaluation.termination);
        if hard_residuals_validated
            && hard_residual_max <= config.normalized_residual_tolerance
            && rank_is_valid
            && returned_evaluation.failures.is_empty()
            && component_solves
                .iter()
                .all(|component| component.termination == SolveTermination::Converged)
        {
            termination = SolveTermination::Converged;
        } else if termination == SolveTermination::Converged {
            termination = SolveTermination::NumericalFailure;
        }

        let mut audit = self
            .audit_snapshot()
            .unwrap_or_else(|_| self.audit_snapshot_partial());
        annotate_audit(
            &mut audit,
            plan,
            &all_redundancy,
            &conflicting_sources,
            &singular_rows,
        );
        annotate_evaluation_failures(&mut audit, &returned_evaluation.failures);
        let mut trace = SolveTrace::default();
        for component in &component_solves {
            append_component_trace(&mut trace, &component.trace);
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
            conflicting_sources,
            redundant_sources,
            sources_containing_redundant_rows,
            redundant_rows: all_redundancy,
            singular_rows,
            structural: plan.structural.clone(),
            component_solves,
            trace,
            audit,
        })
    }

    fn update_decomposition_cache(
        &mut self,
        plan: &EliminationPlan,
        report: &SolveReport,
    ) -> Result<(), CoreError> {
        let prior = self.decomposition_cache.clone().unwrap_or_default();
        let mut components = Vec::with_capacity(plan.components.len());
        for component in &plan.components {
            let summary = &plan.structural.component_summaries[component.index];
            let numerical = &report.component_solves[component.index];
            if numerical.termination == SolveTermination::Converged {
                let values = component
                    .variable_ids
                    .iter()
                    .map(|&variable_id| {
                        self.variable(variable_id)
                            .map(crate::VariableBlock::value)
                            .ok_or(CoreError::UnknownVariable(variable_id))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                components.push(CachedComponent {
                    pattern_signature: summary.pattern_signature,
                    variable_ids: component.variable_ids.clone(),
                    residual_ids: component.residual_ids.clone(),
                    values,
                });
            } else if let Some(cached) = prior
                .components
                .iter()
                .find(|cached| cache_matches(cached, plan, component))
            {
                components.push(cached.clone());
            }
        }
        self.decomposition_cache = Some(DecompositionCache { components });
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ComponentExecution {
    component_index: usize,
    reused: bool,
    termination: SolveTermination,
    trace: SolveTrace,
}

#[derive(Debug)]
struct IterationOutcome {
    termination: SolveTermination,
    state: VariableState,
    trace: SolveTrace,
}

fn cache_matches(
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

fn validated_cached_state(
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
    validate_component(problem, component, &candidate, config)
        .valid
        .then_some(candidate)
}

#[allow(clippy::too_many_lines)]
fn iterate_component(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    mut state: VariableState,
    config: SolverConfig,
) -> IterationOutcome {
    let mut trace = SolveTrace::default();
    let mut damping = config.initial_damping;
    let layout = active_layout(plan, component.index);
    if plan.synchronize_state(problem, &mut state).is_err() {
        return IterationOutcome {
            termination: SolveTermination::NumericalFailure,
            state,
            trace,
        };
    }
    let assembly =
        match problem.assemble_dense_for_residuals(&state, &component.active_residual_ids) {
            Ok(assembly) => assembly,
            Err(error) => {
                return IterationOutcome {
                    termination: error_termination(&error),
                    state,
                    trace,
                };
            }
        };
    let Ok(mut current_hard) =
        extract_active_hard_system(problem, plan, &assembly, component.index)
    else {
        return IterationOutcome {
            termination: SolveTermination::NumericalFailure,
            state,
            trace,
        };
    };
    let Some(mut cost) = residual_cost(&current_hard.residuals) else {
        return IterationOutcome {
            termination: SolveTermination::NumericalFailure,
            state,
            trace,
        };
    };
    let mut termination =
        if residual_max(&current_hard.residuals) <= config.normalized_residual_tolerance {
            SolveTermination::Converged
        } else {
            SolveTermination::IterationLimit
        };

    if termination != SolveTermination::Converged {
        for iteration in 1..=config.max_iterations {
            let Some(mut step) = lm_step(&current_hard.jacobian, &current_hard.residuals, damping)
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
            let Some(predicted_reduction) = predicted_reduction(&current_hard, &step, cost) else {
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
            if apply_normalized_step(problem, plan, &mut trial_state, &layout, &step).is_err() {
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
            let trial_assembly = match problem
                .assemble_dense_for_residuals(&trial_state, &component.active_residual_ids)
            {
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
            let Ok(trial_hard) =
                extract_active_hard_system(problem, plan, &trial_assembly, component.index)
            else {
                termination = SolveTermination::NumericalFailure;
                break;
            };
            let Some(trial_cost) = residual_cost(&trial_hard.residuals) else {
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
    IterationOutcome {
        termination,
        state,
        trace,
    }
}

#[derive(Clone, Debug)]
struct ActiveLayoutBlock {
    root: VariableId,
    members: Vec<VariableId>,
    tangent_range: Range<usize>,
    step_scales: Vec<f64>,
}

#[derive(Clone, Debug, Default)]
struct ActiveLayout {
    blocks: Vec<ActiveLayoutBlock>,
    tangent_dimension: usize,
}

fn active_layout(plan: &EliminationPlan, component_index: usize) -> ActiveLayout {
    let mut layout = ActiveLayout::default();
    for group in &plan.active_groups {
        if group.component_index != component_index {
            continue;
        }
        let start = layout.tangent_dimension;
        let end = start + group.kind.tangent_dimension();
        layout.blocks.push(ActiveLayoutBlock {
            root: group.root,
            members: group.members.clone(),
            tangent_range: start..end,
            step_scales: group.step_scales.clone(),
        });
        layout.tangent_dimension = end;
    }
    layout
}

#[derive(Debug)]
struct HardSystem {
    residuals: DVector<f64>,
    jacobian: DMatrix<f64>,
    rows: Vec<ResidualRowRef>,
}

fn extract_active_hard_system(
    problem: &Problem,
    plan: &EliminationPlan,
    assembly: &DenseAssembly,
    component_index: usize,
) -> Result<HardSystem, CoreError> {
    let layout = active_layout(plan, component_index);
    let mut selected_rows = Vec::new();
    for residual_layout in assembly.residual_layout() {
        let residual = problem
            .residual(residual_layout.residual_id)
            .ok_or(CoreError::UnknownResidual(residual_layout.residual_id))?;
        if residual.category() != ResidualCategory::Hard
            || plan.is_eliminated(residual_layout.residual_id)
            || plan.source_is_suppressed(residual.source())
        {
            continue;
        }
        for (row_in_block, dense_row) in residual_layout.row_range.clone().enumerate() {
            selected_rows.push((
                dense_row,
                ResidualRowRef {
                    residual_id: residual_layout.residual_id,
                    row_in_block,
                    source_id: residual.source(),
                },
            ));
        }
    }
    let mut residuals = DVector::zeros(selected_rows.len());
    let mut jacobian = DMatrix::zeros(selected_rows.len(), layout.tangent_dimension);
    for (target_row, (dense_row, _)) in selected_rows.iter().enumerate() {
        residuals[target_row] = assembly.residuals()[*dense_row];
        for block in &layout.blocks {
            for &member in &block.members {
                let full_block = assembly
                    .variable_layout()
                    .block(member)
                    .ok_or(CoreError::UnknownVariable(member))?;
                for local_column in 0..block.tangent_range.len() {
                    jacobian[(target_row, block.tangent_range.start + local_column)] += assembly
                        .jacobian()[(*dense_row, full_block.tangent_range.start + local_column)];
                }
            }
        }
    }
    Ok(HardSystem {
        residuals,
        jacobian,
        rows: selected_rows.into_iter().map(|(_, row)| row).collect(),
    })
}

fn apply_normalized_step(
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
    plan.synchronize_state(problem, state)
}

struct ComponentValidation {
    evaluated: bool,
    valid: bool,
    maximum: f64,
    l2: f64,
    termination: SolveTermination,
    rows: Vec<(ResidualId, usize, SourceConstraintId, f64)>,
}

fn validate_component(
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
                    maximum,
                    l2,
                    termination: SolveTermination::Converged,
                    rows,
                }
            } else {
                ComponentValidation {
                    evaluated: false,
                    valid: false,
                    maximum,
                    l2: 0.0,
                    termination: SolveTermination::NumericalFailure,
                    rows,
                }
            }
        }
        Err(error) => ComponentValidation {
            evaluated: false,
            valid: false,
            maximum: 0.0,
            l2: 0.0,
            termination: error_termination(&error),
            rows: Vec::new(),
        },
    }
}

struct ComponentNumerics {
    termination: SolveTermination,
    rank_is_valid: bool,
    rank: usize,
    is_singular: bool,
    rank_threshold: f64,
    singular_values: Vec<f64>,
    singular_rows: Vec<ResidualRowRef>,
    hard: HardSystem,
}

fn component_numerics(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    config: SolverConfig,
) -> ComponentNumerics {
    let empty = || HardSystem {
        residuals: DVector::zeros(0),
        jacobian: DMatrix::zeros(
            0,
            plan.structural.component_summaries[component.index].active_tangent_dimensions,
        ),
        rows: Vec::new(),
    };
    let assembly = match problem.assemble_dense_for_residuals(state, &component.active_residual_ids)
    {
        Ok(assembly) => assembly,
        Err(error) => {
            return ComponentNumerics {
                termination: error_termination(&error),
                rank_is_valid: false,
                rank: 0,
                is_singular: false,
                rank_threshold: 0.0,
                singular_values: Vec::new(),
                singular_rows: Vec::new(),
                hard: empty(),
            };
        }
    };
    let Ok(hard) = extract_active_hard_system(problem, plan, &assembly, component.index) else {
        return ComponentNumerics {
            termination: SolveTermination::NumericalFailure,
            rank_is_valid: false,
            rank: 0,
            is_singular: false,
            rank_threshold: 0.0,
            singular_values: Vec::new(),
            singular_rows: Vec::new(),
            hard: empty(),
        };
    };
    let Some(rank) = rank_diagnostics(&hard.jacobian, config.rank_relative_tolerance) else {
        return ComponentNumerics {
            termination: SolveTermination::NumericalFailure,
            rank_is_valid: false,
            rank: 0,
            is_singular: false,
            rank_threshold: 0.0,
            singular_values: Vec::new(),
            singular_rows: Vec::new(),
            hard,
        };
    };
    let is_singular = rank.rank < hard.jacobian.nrows().min(hard.jacobian.ncols());
    let singular_rows = find_singular_rows(&hard, rank.threshold, is_singular);
    ComponentNumerics {
        termination: SolveTermination::Converged,
        rank_is_valid: true,
        rank: rank.rank,
        is_singular,
        rank_threshold: rank.threshold,
        singular_values: rank.singular_values,
        singular_rows,
        hard,
    }
}

fn find_conflicting_sources(
    problem: &Problem,
    plan: &EliminationPlan,
    returned_state: &VariableState,
    config: SolverConfig,
    component_reports: &[ComponentSolveReport],
) -> Vec<SourceConstraintId> {
    let eligible_components: Vec<_> = plan
        .components
        .iter()
        .filter(|component| {
            let report = &component_reports[component.index];
            (!report.hard_residuals_validated
                || report.hard_residual_max > config.normalized_residual_tolerance)
                && plan.structural.component_summaries[component.index].active_tangent_dimensions
                    <= MAX_CONFLICT_COMPONENT_DIMENSION
                && candidate_sources(problem, component).len() <= MAX_CONFLICT_COMPONENT_SOURCES
        })
        .collect();
    let mut candidates = Vec::new();
    for source in problem.source_order() {
        for component in &eligible_components {
            if !source_affects_component(problem, source, component) {
                continue;
            }
            if deletion_restores_component(problem, plan, component, source, returned_state, config)
            {
                candidates.push(source);
                break;
            }
        }
    }
    candidates
}

fn candidate_sources(problem: &Problem, component: &SolveComponent) -> Vec<SourceConstraintId> {
    problem
        .source_order()
        .into_iter()
        .filter(|&source| source_affects_component(problem, source, component))
        .collect()
}

fn source_affects_component(
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

fn deletion_restores_component(
    problem: &Problem,
    _normal_plan: &EliminationPlan,
    failed_component: &SolveComponent,
    source: SourceConstraintId,
    returned_state: &VariableState,
    config: SolverConfig,
) -> bool {
    let Ok(trial_plan) = EliminationPlan::new_suppressed(problem, &[source]) else {
        return false;
    };
    let mut state = returned_state.clone();
    if trial_plan.synchronize_state(problem, &mut state).is_err() {
        return false;
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
        let outcome = iterate_component(problem, &trial_plan, component, state, config);
        state = outcome.state;
        if matches!(
            outcome.termination,
            SolveTermination::InvalidGeometry | SolveTermination::NumericalFailure
        ) {
            return false;
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
        })
}

#[derive(Default)]
struct RedundancyDiagnostics {
    rows: Vec<RedundantRowCandidate>,
}

fn find_redundancy(
    hard: &HardSystem,
    validated_rows: &[(ResidualId, usize, SourceConstraintId, f64)],
    source_order: &[SourceConstraintId],
    threshold: f64,
    residual_tolerance: f64,
) -> RedundancyDiagnostics {
    let mut diagnostics = RedundancyDiagnostics::default();
    let mut prior_source_rows = Vec::new();
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
        let all_nonzero = source_rows
            .iter()
            .all(|&row| row_is_nonzero(hard, row, threshold));
        let all_satisfied = source_rows.iter().all(|&row| {
            validated_value(validated_rows, hard.rows[row])
                .is_some_and(|value| value.abs() <= residual_tolerance)
        });
        let prior_rank = selected_row_rank(&hard.jacobian, &prior_source_rows, threshold);
        let mut combined = prior_source_rows.clone();
        combined.extend_from_slice(&source_rows);
        let combined_rank = selected_row_rank(&hard.jacobian, &combined, threshold);
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
            for &row in &source_rows {
                let before = selected_row_rank(&hard.jacobian, &basis, threshold);
                let mut with_row = basis.clone();
                with_row.push(row);
                let after = selected_row_rank(&hard.jacobian, &with_row, threshold);
                if before > 0
                    && row_is_nonzero(hard, row, threshold)
                    && validated_value(validated_rows, hard.rows[row])
                        .is_some_and(|value| value.abs() <= residual_tolerance)
                    && after == before
                {
                    let local_before =
                        selected_row_rank(&hard.jacobian, &earlier_source_rows, threshold);
                    let mut local_with = earlier_source_rows.clone();
                    local_with.push(row);
                    let local_after = selected_row_rank(&hard.jacobian, &local_with, threshold);
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
                earlier_source_rows.push(row);
            }
        }
        prior_source_rows = combined;
    }
    diagnostics
}

fn find_singular_rows(
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

fn globally_fully_redundant_sources(
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

struct RowEvaluationFailure {
    residual_id: ResidualId,
    error: String,
}

struct ReturnedEvaluation {
    termination: SolveTermination,
    failures: Vec<RowEvaluationFailure>,
}

fn validate_returned_rows(problem: &Problem, state: &VariableState) -> ReturnedEvaluation {
    let mut termination = SolveTermination::Converged;
    let mut failures = Vec::new();
    for (residual_id, _) in problem.residuals.iter() {
        if let Err(error) = problem.assemble_dense_for_residuals(state, &[residual_id]) {
            termination = worse_termination(termination, error_termination(&error));
            failures.push(RowEvaluationFailure {
                residual_id,
                error: error.to_string(),
            });
        }
    }
    ReturnedEvaluation {
        termination,
        failures,
    }
}

fn annotate_evaluation_failures(audit: &mut AuditSnapshot, failures: &[RowEvaluationFailure]) {
    for source in &mut audit.sources {
        for row in &mut source.rows {
            if let Some(failure) = failures
                .iter()
                .find(|failure| failure.residual_id == row.residual_id)
            {
                row.evaluation_status = AuditEvaluationStatus::Failed;
                row.evaluation_error = Some(failure.error.clone());
            }
        }
    }
}

fn annotate_audit(
    audit: &mut AuditSnapshot,
    plan: &EliminationPlan,
    redundant_rows: &[RedundantRowCandidate],
    conflicting_sources: &[SourceConstraintId],
    singular_rows: &[ResidualRowRef],
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
        }
        source.annotations.eliminated = source.rows.iter().any(|row| row.annotations.eliminated);
        source.annotations.suppressed = source.rows.iter().any(|row| row.annotations.suppressed);
        source.annotations.redundant = source.rows.iter().any(|row| row.annotations.redundant);
        source.annotations.conflicting = conflicting_sources.contains(&source.source_id)
            || source.rows.iter().any(|row| row.annotations.conflicting);
        source.annotations.singular = source.rows.iter().any(|row| row.annotations.singular);
    }
}

fn row_is_nonzero(hard: &HardSystem, row: usize, threshold: f64) -> bool {
    stable_norm(hard.jacobian.row(row).iter().copied()).is_some_and(|norm| norm > threshold)
}

fn validated_value(
    validated_rows: &[(ResidualId, usize, SourceConstraintId, f64)],
    row: ResidualRowRef,
) -> Option<f64> {
    validated_rows
        .iter()
        .find(|item| item.0 == row.residual_id && item.1 == row.row_in_block)
        .map(|item| item.3)
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

fn sort_redundancy(problem: &Problem, rows: &mut [RedundantRowCandidate]) {
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

fn ordered_sources(
    problem: &Problem,
    predicate: impl Fn(SourceConstraintId) -> bool,
) -> Vec<SourceConstraintId> {
    problem
        .source_order()
        .into_iter()
        .filter(|&source| predicate(source))
        .collect()
}

fn deduplicate_rows(rows: &mut Vec<ResidualRowRef>) {
    let mut unique = Vec::new();
    for row in rows.drain(..) {
        if !unique.contains(&row) {
            unique.push(row);
        }
    }
    *rows = unique;
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
    solve_dense_least_squares(&augmented, &right_hand_side).map(|(solution, _)| solution)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinearSolveMethod {
    Qr,
    Svd,
}

fn solve_dense_least_squares(
    matrix: &DMatrix<f64>,
    right_hand_side: &DVector<f64>,
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
        let qr = matrix.clone().qr();
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
    let svd = matrix.clone().svd(true, true);
    let largest = svd.singular_values.iter().copied().fold(0.0_f64, f64::max);
    let dimension = u32::try_from(rows.max(columns)).ok()?;
    let epsilon = f64::EPSILON * f64::from(dimension) * largest;
    let solution = svd.solve(right_hand_side, epsilon).ok()?;
    solution
        .iter()
        .all(|value| value.is_finite())
        .then_some((solution, LinearSolveMethod::Svd))
}

fn limit_block_steps(step: &mut DVector<f64>, layout: &ActiveLayout, limit: f64) -> Option<f64> {
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

fn predicted_reduction(system: &HardSystem, step: &DVector<f64>, cost: f64) -> Option<f64> {
    let model_cost = residual_cost(&(&system.residuals + &system.jacobian * step))?;
    let reduction = cost - model_cost;
    reduction.is_finite().then_some(reduction)
}

fn residual_cost(residuals: &DVector<f64>) -> Option<f64> {
    let cost = 0.5 * residuals.dot(residuals);
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
    Some(RankDiagnostics {
        rank: singular_values
            .iter()
            .filter(|&&value| value > threshold)
            .count(),
        threshold,
        singular_values,
    })
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

fn stamp_component_trace(trace: &mut SolveTrace, component_index: usize) {
    for record in &mut trace.records {
        record.component_index = Some(component_index);
    }
}

fn append_component_trace(combined: &mut SolveTrace, component: &SolveTrace) {
    for record in &component.records {
        let mut record = record.clone();
        record.iteration = combined.records.len() + 1;
        combined.records.push(record);
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

fn worse_termination(first: SolveTermination, second: SolveTermination) -> SolveTermination {
    if termination_severity(second) > termination_severity(first) {
        second
    } else {
        first
    }
}

const fn termination_severity(termination: SolveTermination) -> u8 {
    match termination {
        SolveTermination::Converged => 0,
        SolveTermination::IterationLimit => 1,
        SolveTermination::Stalled => 2,
        SolveTermination::InvalidGeometry => 3,
        SolveTermination::NumericalFailure => 4,
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

fn push_unique<T: Copy + PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_strict_but_finite() {
        SolverConfig::default().validate().unwrap();
    }

    #[test]
    fn singular_dense_system_falls_back_from_qr_to_svd() {
        let matrix = DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, 0.0]);
        let right_hand_side = DVector::from_vec(vec![2.0, 0.0]);
        let (solution, method) = solve_dense_least_squares(&matrix, &right_hand_side).unwrap();
        assert_eq!(method, LinearSolveMethod::Svd);
        assert!((solution[0] - 2.0).abs() <= f64::EPSILON);
        assert!(solution[1].abs() <= f64::EPSILON);
    }
}
