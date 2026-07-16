use nalgebra::{DMatrix, DVector};

use crate::analysis::{
    CachedComponent, DecompositionCache, EliminationPlan, SolveComponent, set_state_value,
};
use crate::linearization::{ComponentDenseSystem, ComponentTangentLayout};
use crate::problem::VariableState;
use crate::{
    AuditBoundAnnotation, AuditEvaluationStatus, AuditSnapshot, BoundId, BoundReport, BoundStatus,
    CoordinateBound, CoreError, OneSidedMobility, Problem, ResidualCategory, ResidualId,
    SourceConstraintId, StructuralSummary, VariableId,
};

const MAX_PRIORITY_LINE_SEARCH_STEPS: usize = 20;
const PRIORITY_REPROJECTION_TOLERANCE: f64 = 8.0 * f64::EPSILON;
const PRIORITY_COST_RESOLUTION_FACTOR: f64 = 8.0;
const PRIORITY_HESSIAN_NORMALIZED_STEP: f64 = 1.0e-3;
const NEAR_SINGULAR_FACTOR: f64 = 100.0;
// This is only the squared residual roundoff band for preserving an attained zero cost.
const PRIORITY_ZERO_COST_ROUNDOFF: f64 =
    0.5 * PRIORITY_REPROJECTION_TOLERANCE * PRIORITY_REPROJECTION_TOLERANCE;

/// Why nonlinear iteration stopped. Constraint-system diagnostics are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SolveTermination {
    /// The requested iteration policy met its convergence criteria.
    Converged,
    /// Finite iteration could not find an acceptable improving step.
    Stalled,
    /// The configured iteration budget was exhausted.
    IterationLimit,
    /// A residual rejected the returned or trial geometry.
    InvalidGeometry,
    /// Non-finite arithmetic or another numerical operation failed.
    NumericalFailure,
}

/// Result of fresh independent hard-constraint validation at the returned state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HardValidity {
    /// Core hard rows and all caller-owned domain validators accepted the returned state.
    Valid,
    /// Complete validation ran and rejected at least one hard equation or domain invariant.
    Invalid,
    /// Complete independent validation could not be evaluated.
    NotEvaluated,
}

/// Aggregate outcome for one requested secondary priority level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SecondaryStatus {
    /// No residuals requested this priority level.
    NotRequested,
    /// The requested secondary objective met its optimum criteria.
    Optimal,
    /// The immutable hard state made the finite secondary result acceptable but not optimal.
    Acceptable,
    /// Finite secondary iteration could not find an acceptable improving step.
    Stalled,
    /// The secondary iteration budget was exhausted.
    IterationLimit,
    /// Evaluation failed without changing authoritative hard validity.
    EvaluationFailure,
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

/// Attempted hard LM steps in deterministic execution order.
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

/// Bounded deterministic work policy for one explanatory diagnostic section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticBudget {
    pub enabled: bool,
    pub max_component_tangent_dimension: usize,
    pub max_component_scalar_rows: usize,
    pub max_candidate_sources: usize,
    pub max_trials: usize,
}

impl DiagnosticBudget {
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            enabled: true,
            max_component_tangent_dimension: usize::MAX,
            max_component_scalar_rows: usize::MAX,
            max_candidate_sources: usize::MAX,
            max_trials: usize::MAX,
        }
    }
}

/// Actual deterministic work consumed by one diagnostic section.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticWork {
    pub components: usize,
    pub tangent_dimensions: usize,
    pub scalar_rows: usize,
    pub candidate_sources: usize,
    pub trials: usize,
}

/// Completeness of the documented bounded candidate algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DiagnosticStatus {
    Complete,
    Truncated,
    Skipped,
}

/// Machine-readable reason why candidate analysis was incomplete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DiagnosticIncompleteReason {
    Disabled,
    HardConstraintsValid,
    HardInvalid,
    InvalidEvaluation,
    InvalidRank,
    ComponentTangentBudget,
    ComponentRowBudget,
    CandidateSourceBudget,
    TrialBudget,
}

/// Reported budget, consumed work, and completeness for one diagnostic section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticCompleteness {
    pub status: DiagnosticStatus,
    pub budget: DiagnosticBudget,
    pub consumed: DiagnosticWork,
    pub reason: Option<DiagnosticIncompleteReason>,
}

/// Numerical outcome for one reduced solve component.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
#[non_exhaustive]
pub struct ComponentSolveReport {
    /// Stable index into the report's reduced component ordering.
    pub component_index: usize,
    /// Deterministic reduced-incidence pattern signature.
    pub pattern_signature: u64,
    /// Whether the existing decomposed cache supplied this component result.
    pub reused: bool,
    /// Hard and secondary outer iterations attributed to this component.
    pub iterations: usize,
    /// Compatibility aggregate termination including secondary optimization.
    pub termination: SolveTermination,
    /// Termination of hard nonlinear iteration alone.
    pub hard_termination: SolveTermination,
    /// Fresh independent core-row validity for this component.
    pub hard_validity: HardValidity,
    /// Whether every active hard core row was freshly evaluated and finite.
    pub hard_residuals_validated: bool,
    /// Maximum absolute normalized hard residual at the returned state.
    pub hard_residual_max: f64,
    /// Stable Euclidean norm of normalized hard rows in this component.
    pub hard_residual_l2: f64,
    /// Whether all numerical-rank inputs and decomposition outputs were finite.
    pub rank_is_valid: bool,
    /// Count of singular values strictly greater than `rank_threshold`.
    pub rank: usize,
    /// Component hard-row count minus numerical rank.
    pub left_nullity: usize,
    /// Component active tangent dimension minus numerical rank.
    pub right_nullity: usize,
    /// Compatibility alias for `right_nullity` before active bounds exist.
    pub local_degrees_of_freedom: usize,
    /// Lineality dimension after independent active-bound normals are appended.
    pub bidirectional_degrees_of_freedom: usize,
    /// Existence of a nonzero direction in the active feasible tangent cone.
    pub one_sided_mobility: OneSidedMobility,
    /// Active/fixed bounds affecting this component, in global bound order.
    pub active_bounds: Vec<BoundId>,
    /// Whether rank is below the smaller component matrix dimension.
    pub is_singular: bool,
    /// Configured relative factor used by the numerical-rank policy.
    pub rank_relative_tolerance: f64,
    /// Machine-floor threshold before taking the maximum with the relative threshold.
    pub rank_machine_tolerance: f64,
    /// Authoritative component threshold used with a strict greater-than comparison.
    pub rank_threshold: f64,
    /// Largest component singular value, or zero for an empty/all-zero spectrum.
    pub sigma_max: f64,
    /// Smallest singular value retained in numerical rank, when one exists.
    pub smallest_retained_singular_value: Option<f64>,
    /// Inclusive warning-band multiplier applied to `rank_threshold`.
    pub near_singular_factor: f64,
    /// Smallest retained singular value divided by `rank_threshold`.
    pub near_singular_ratio: Option<f64>,
    /// Whether the retained spectrum lies inside the warning band without changing rank.
    pub near_singular: bool,
    /// Finite component singular values in decomposition order.
    pub singular_values: Vec<f64>,
    /// Component-local costs. Reused components always have an empty trace.
    pub trace: SolveTrace,
}

/// Outcome of one category-level optimization on one reduced hard component.
///
/// `component_index` is `None` for fixed-only rows and unsupported
/// cross-component incidence. Costs are absent when initial evaluation failed.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PrioritySolveReport {
    pub component_index: Option<usize>,
    /// Priority category represented by this pass.
    pub category: ResidualCategory,
    /// Iterations consumed by this category-level pass.
    pub iterations: usize,
    /// Initial dimensionless `0.5 * ||r||^2`, absent when evaluation failed.
    pub initial_cost: Option<f64>,
    /// Returned-state dimensionless `0.5 * ||r||^2`, absent when evaluation failed.
    pub final_cost: Option<f64>,
    /// Temporary cost attained before this Preference pass, when applicable.
    pub attained_temporary_cost: Option<f64>,
    /// Compatibility termination for this priority pass.
    pub termination: SolveTermination,
    /// Orthogonal secondary optimization outcome.
    pub status: SecondaryStatus,
}

/// Numerical and structural facts evaluated at the returned state.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
#[non_exhaustive]
pub struct SolveReport {
    /// Compatibility aggregate termination including secondary optimization.
    pub termination: SolveTermination,
    /// Aggregate hard-only nonlinear termination.
    pub hard_termination: SolveTermination,
    /// Authoritative fresh hard validity; domain wrappers extend this before returning results.
    pub hard_validity: HardValidity,
    /// Aggregate temporary-objective outcome.
    pub temporary_status: SecondaryStatus,
    /// Aggregate preference-objective outcome.
    pub preference_status: SecondaryStatus,
    /// Hard trace records plus category-level priority outer iterations.
    pub iterations: usize,
    /// Finite problem state returned by core, whether accepted or rolled back.
    pub accepted_state: crate::PackedState,
    /// Whether every active hard core row was freshly evaluated and finite.
    pub hard_residuals_validated: bool,
    /// Maximum absolute normalized hard core residual.
    pub hard_residual_max: f64,
    /// Stable Euclidean norm of normalized hard core residuals.
    pub hard_residual_l2: f64,
    /// Whether every component numerical-rank result is valid.
    pub rank_is_valid: bool,
    /// Sum of component-local numerical ranks.
    pub rank: usize,
    /// Sum of component-local numerical left nullities.
    pub left_nullity: usize,
    /// Sum of component-local numerical right nullities.
    pub right_nullity: usize,
    /// Compatibility alias for aggregate `right_nullity` before active bounds exist.
    pub local_degrees_of_freedom: usize,
    /// Aggregate lineality dimension after independent active-bound normals.
    pub bidirectional_degrees_of_freedom: usize,
    /// Whole-problem one-sided feasible-motion result.
    pub one_sided_mobility: OneSidedMobility,
    /// Whether any component is numerically singular.
    pub is_singular: bool,
    /// Whether any component lies in the distinct near-singular warning band.
    pub near_singular: bool,
    /// Configured component-local relative rank tolerance.
    pub rank_relative_tolerance: f64,
    /// Maximum component-local machine floor; component reports are authoritative.
    pub rank_machine_tolerance: f64,
    /// Maximum component-local rank threshold; component reports retain each threshold.
    pub rank_threshold: f64,
    /// Component singular values concatenated in reduced component order.
    pub singular_values: Vec<f64>,
    /// Sources identified by the bounded core conflict analysis.
    pub conflicting_sources: Vec<SourceConstraintId>,
    /// Sources whose complete active row group is redundant to prior sources.
    pub redundant_sources: Vec<SourceConstraintId>,
    /// Sources containing at least one redundant row, including partial groups.
    pub sources_containing_redundant_rows: Vec<SourceConstraintId>,
    /// Individual nonzero satisfied rows identified as numerically dependent.
    pub redundant_rows: Vec<RedundantRowCandidate>,
    /// Completeness and consumed work for redundancy candidates.
    pub redundancy_diagnostics: DiagnosticCompleteness,
    /// Completeness and consumed work for conflict candidates.
    pub conflict_diagnostics: DiagnosticCompleteness,
    /// Hard rows associated with a numerical singularity diagnostic.
    pub singular_rows: Vec<ResidualRowRef>,
    /// Separate graph/count structural summary; this is not numerical rank.
    pub structural: StructuralSummary,
    /// Authoritative component-local solve and rank reports.
    pub component_solves: Vec<ComponentSolveReport>,
    /// Lexicographic secondary outcomes, kept separate from hard-cost traces.
    pub priority_solves: Vec<PrioritySolveReport>,
    /// Hard-only records carry component identity; priority costs are reported separately.
    pub trace: SolveTrace,
    /// Returned-state equation audit, including failed evaluation rows.
    pub audit: AuditSnapshot,
    /// Separate bound audit; bounds do not add or reinterpret equation rows.
    pub bounds: Vec<BoundReport>,
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
    pub redundancy_diagnostic_budget: DiagnosticBudget,
    pub conflict_diagnostic_budget: DiagnosticBudget,
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
            redundancy_diagnostic_budget: DiagnosticBudget::unlimited(),
            conflict_diagnostic_budget: DiagnosticBudget {
                enabled: true,
                max_component_tangent_dimension: 24,
                max_component_scalar_rows: usize::MAX,
                max_candidate_sources: 12,
                max_trials: usize::MAX,
            },
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
        self.solve_reduced(config, DirtyRequest::All, None)
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
        self.solve_reduced(config, DirtyRequest::Variables(edited_variables), None)
    }

    pub(crate) fn solve_session_components(
        &mut self,
        config: SolverConfig,
        dirty_components: &[usize],
        cached_plan: &EliminationPlan,
    ) -> Result<SolveReport, CoreError> {
        self.solve_reduced(
            config,
            DirtyRequest::Components(dirty_components),
            Some(cached_plan),
        )
    }

    fn solve_reduced(
        &mut self,
        config: SolverConfig,
        dirty_request: DirtyRequest<'_>,
        cached_plan: Option<&EliminationPlan>,
    ) -> Result<SolveReport, CoreError> {
        config.validate()?;
        let plan = if let Some(plan) = cached_plan {
            plan.clone()
        } else {
            EliminationPlan::new(self)?
        };
        let mut edited_components = Vec::new();
        if let DirtyRequest::Variables(edited_variables) = dirty_request {
            for &variable_id in edited_variables {
                let component = plan
                    .component_for_variable(variable_id)
                    .ok_or(CoreError::UnknownVariable(variable_id))?;
                push_unique(&mut edited_components, component);
            }
        } else if let DirtyRequest::Components(components) = dirty_request {
            for &component in components {
                if component >= plan.components.len() {
                    return Err(CoreError::DimensionMismatch {
                        context: "dirty solve component",
                        expected: plan.components.len(),
                        actual: component,
                    });
                }
                push_unique(&mut edited_components, component);
            }
        }

        let prior_cache = self.decomposition_cache.clone().unwrap_or_default();
        let prior_report = (!matches!(dirty_request, DirtyRequest::All))
            .then_some(prior_cache.report.as_deref())
            .flatten();
        let mut state = self.variable_state();
        plan.synchronize_state(self, &mut state)?;
        project_initial_state_into_bounds(self, &plan, &mut state)?;
        let mut executions = Vec::with_capacity(plan.components.len());
        for component in &plan.components {
            let may_reuse = !matches!(dirty_request, DirtyRequest::All)
                && !edited_components.contains(&component.index);
            let cached = prior_cache
                .components
                .iter()
                .find(|cached| cache_matches(cached, &plan, component));
            if may_reuse
                && let Some(cached_state) = cached.and_then(|cached| {
                    if matches!(dirty_request, DirtyRequest::Components(_)) {
                        trusted_cached_state(self, &plan, component, &state, cached)
                    } else {
                        validated_cached_state(self, &plan, component, &state, cached, config)
                    }
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

        let PriorityPassOutcome {
            state: priority_state,
            reports: priority_records,
        } = optimize_priorities(self, &plan, state, config, &executions, prior_report);
        state = priority_state;
        let priority_reports: Vec<_> = priority_records
            .into_iter()
            .map(|record| record.report)
            .collect();

        self.replace_variable_state(&state)?;
        let report =
            self.build_report(config, &plan, &executions, &priority_reports, prior_report)?;
        self.update_decomposition_cache(&plan, &report)?;
        Ok(report)
    }

    #[allow(clippy::too_many_lines)]
    fn build_report(
        &self,
        config: SolverConfig,
        plan: &EliminationPlan,
        executions: &[ComponentExecution],
        priority_solves: &[PrioritySolveReport],
        prior_report: Option<&SolveReport>,
    ) -> Result<SolveReport, CoreError> {
        let state = self.variable_state();
        let accepted_state = self.packed_state()?;
        let mut component_solves = Vec::with_capacity(plan.components.len());
        let mut all_redundancy = Vec::new();
        let mut singular_rows = Vec::new();
        let mut hard_residual_l2 = 0.0_f64;
        let mut hard_l2_is_valid = true;
        let bounds = bound_reports(self, &state)?;
        let mut redundancy_work = DiagnosticWork::default();
        let mut redundancy_reason = (!config.redundancy_diagnostic_budget.enabled)
            .then_some(DiagnosticIncompleteReason::Disabled);
        let mut redundancy_analyzed = false;

        for component in &plan.components {
            let execution = executions
                .iter()
                .find(|execution| execution.component_index == component.index)
                .ok_or(CoreError::DimensionMismatch {
                    context: "component execution report",
                    expected: plan.components.len(),
                    actual: executions.len(),
                })?;
            // Reuse skips nonlinear iteration only. Acceptance values,
            // Jacobians/rank, mobility, and bounded diagnostics are rebuilt at
            // every returned state so a cache can never stand in for fresh
            // independent validation.
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
                let candidate_count = candidate_sources(self, component).len();
                if let Some(reason) = diagnostic_component_budget_reason(
                    &plan.structural.component_summaries[component.index],
                    candidate_count,
                    redundancy_work.trials,
                    config.redundancy_diagnostic_budget,
                ) {
                    redundancy_reason.get_or_insert(reason);
                } else {
                    let source_order = self.source_order();
                    let trials = source_order
                        .iter()
                        .filter(|&&source| source_affects_component(self, source, component))
                        .count();
                    if redundancy_work.trials.saturating_add(trials)
                        > config.redundancy_diagnostic_budget.max_trials
                    {
                        redundancy_reason.get_or_insert(DiagnosticIncompleteReason::TrialBudget);
                    } else {
                        let summary = &plan.structural.component_summaries[component.index];
                        redundancy_work.components += 1;
                        redundancy_work.tangent_dimensions = redundancy_work
                            .tangent_dimensions
                            .saturating_add(summary.active_tangent_dimensions);
                        redundancy_work.scalar_rows = redundancy_work
                            .scalar_rows
                            .saturating_add(summary.active_hard_rows);
                        redundancy_work.candidate_sources += candidate_count;
                        redundancy_work.trials += trials;
                        redundancy_analyzed = true;
                        let redundancy = find_redundancy(
                            &numerical.hard,
                            &validation.rows,
                            &source_order,
                            numerical.diagnostics.threshold,
                            config.normalized_residual_tolerance,
                        );
                        all_redundancy.extend(redundancy.rows);
                    }
                }
            } else if validation.evaluated && !validation.valid {
                redundancy_reason.get_or_insert(DiagnosticIncompleteReason::HardInvalid);
            } else if !validation.valid {
                redundancy_reason.get_or_insert(DiagnosticIncompleteReason::InvalidEvaluation);
            } else {
                redundancy_reason.get_or_insert(DiagnosticIncompleteReason::InvalidRank);
            }
            singular_rows.extend(numerical.singular_rows.iter().copied());
            let summary = &plan.structural.component_summaries[component.index];
            let left_nullity = numerical
                .hard
                .jacobian
                .nrows()
                .saturating_sub(numerical.diagnostics.rank);
            let right_nullity = numerical
                .hard
                .jacobian
                .ncols()
                .saturating_sub(numerical.diagnostics.rank);
            let bound_mobility = component_bound_mobility(
                plan,
                component,
                &numerical.hard.jacobian,
                numerical.rank_is_valid,
                numerical.diagnostics.rank,
                config.rank_relative_tolerance,
                &bounds,
            );
            component_solves.push(ComponentSolveReport {
                component_index: component.index,
                pattern_signature: summary.pattern_signature,
                reused: execution.reused,
                iterations: execution.trace.records.len(),
                termination,
                hard_termination: execution.termination,
                hard_validity: validation.hard_validity,
                hard_residuals_validated: validation.evaluated,
                hard_residual_max: validation.maximum,
                hard_residual_l2: validation.l2,
                rank_is_valid: numerical.rank_is_valid,
                rank: numerical.diagnostics.rank,
                left_nullity,
                right_nullity,
                local_degrees_of_freedom: right_nullity,
                bidirectional_degrees_of_freedom: bound_mobility.bidirectional_dof,
                one_sided_mobility: bound_mobility.one_sided,
                active_bounds: bound_mobility.active_bounds,
                is_singular: numerical.is_singular,
                rank_relative_tolerance: numerical.diagnostics.relative_tolerance,
                rank_machine_tolerance: numerical.diagnostics.machine_tolerance,
                rank_threshold: numerical.diagnostics.threshold,
                sigma_max: numerical.diagnostics.sigma_max,
                smallest_retained_singular_value: numerical.diagnostics.smallest_retained,
                near_singular_factor: numerical.diagnostics.near_singular_factor,
                near_singular_ratio: numerical.diagnostics.near_singular_ratio,
                near_singular: numerical.diagnostics.near_singular,
                singular_values: numerical.diagnostics.singular_values,
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
        let (conflicting_sources, conflict_diagnostics) =
            find_conflicting_sources(self, plan, &state, config, &component_solves);
        let redundancy_diagnostics = diagnostic_completeness(
            config.redundancy_diagnostic_budget,
            redundancy_work,
            redundancy_reason,
            redundancy_analyzed,
        );

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
        let left_nullity = component_solves
            .iter()
            .map(|component| component.left_nullity)
            .sum();
        let right_nullity = component_solves
            .iter()
            .map(|component| component.right_nullity)
            .sum();
        let local_degrees_of_freedom = right_nullity;
        let bidirectional_degrees_of_freedom = component_solves
            .iter()
            .map(|component| component.bidirectional_degrees_of_freedom)
            .sum();
        let one_sided_mobility = aggregate_one_sided_mobility(&component_solves);
        let is_singular = component_solves
            .iter()
            .any(|component| component.is_singular);
        let near_singular = component_solves
            .iter()
            .any(|component| component.near_singular);
        let rank_machine_tolerance = component_solves
            .iter()
            .map(|component| component.rank_machine_tolerance)
            .fold(0.0, f64::max);
        let rank_threshold = component_solves
            .iter()
            .map(|component| component.rank_threshold)
            .fold(0.0, f64::max);
        let singular_values = component_solves
            .iter()
            .flat_map(|component| component.singular_values.iter().copied())
            .collect();
        let mut evaluated_residuals = self
            .residuals
            .iter()
            .filter_map(|(residual_id, residual)| {
                (residual.category() == ResidualCategory::Hard).then_some(residual_id)
            })
            .collect::<Vec<_>>();
        let priority_assignments = classify_priority_residuals(self, plan);
        for execution in executions.iter().filter(|execution| !execution.reused) {
            for category in [ResidualCategory::Temporary, ResidualCategory::Preference] {
                for &residual_id in
                    priority_assignments.component(category, execution.component_index)
                {
                    push_unique(&mut evaluated_residuals, residual_id);
                }
            }
        }
        for category in [ResidualCategory::Temporary, ResidualCategory::Preference] {
            for &residual_id in priority_assignments.fixed(category) {
                push_unique(&mut evaluated_residuals, residual_id);
            }
            for &residual_id in priority_assignments.unsupported(category) {
                push_unique(&mut evaluated_residuals, residual_id);
            }
        }
        let returned_evaluation = validate_returned_rows(
            self,
            &state,
            prior_report
                .is_some()
                .then_some(evaluated_residuals.as_slice()),
        );
        let hard_termination = component_solves
            .iter()
            .map(|component| component.hard_termination)
            .fold(SolveTermination::Converged, worse_termination);
        let hard_validity = if hard_l2_is_valid {
            aggregate_hard_validity(&component_solves)
        } else {
            HardValidity::NotEvaluated
        };
        let temporary_status =
            aggregate_secondary_status(self, ResidualCategory::Temporary, priority_solves, None);
        let preference_status = aggregate_secondary_status(
            self,
            ResidualCategory::Preference,
            priority_solves,
            Some(temporary_status),
        );
        let mut termination = component_solves
            .iter()
            .map(|component| component.termination)
            .fold(SolveTermination::Converged, worse_termination);
        if !hard_l2_is_valid {
            termination = worse_termination(termination, SolveTermination::NumericalFailure);
        }
        termination = worse_termination(termination, returned_evaluation.termination);
        for priority in priority_solves {
            termination = worse_termination(termination, priority.termination);
        }
        if hard_residuals_validated
            && hard_residual_max <= config.normalized_residual_tolerance
            && rank_is_valid
            && returned_evaluation.failures.is_empty()
            && component_solves
                .iter()
                .all(|component| component.termination == SolveTermination::Converged)
            && priority_solves
                .iter()
                .all(|priority| priority.termination == SolveTermination::Converged)
        {
            termination = SolveTermination::Converged;
        } else if termination == SolveTermination::Converged {
            termination = SolveTermination::NumericalFailure;
        }

        let mut audit = if let Some(prior) = prior_report {
            merge_reused_audit(
                prior.audit.clone(),
                self.audit_snapshot_partial_for_residuals(&evaluated_residuals),
            )
        } else {
            self.audit_snapshot()
                .unwrap_or_else(|_| self.audit_snapshot_partial())
        };
        annotate_audit(
            &mut audit,
            plan,
            &all_redundancy,
            &conflicting_sources,
            &singular_rows,
            &bounds,
            redundancy_diagnostics,
            conflict_diagnostics,
        );
        annotate_evaluation_failures(&mut audit, &returned_evaluation.failures);
        let mut trace = SolveTrace::default();
        for component in &component_solves {
            append_component_trace(&mut trace, &component.trace);
        }

        let priority_iterations = priority_solves
            .iter()
            .map(|priority| priority.iterations)
            .sum::<usize>();
        Ok(SolveReport {
            termination,
            hard_termination,
            hard_validity,
            temporary_status,
            preference_status,
            iterations: trace.records.len().saturating_add(priority_iterations),
            accepted_state,
            hard_residuals_validated,
            hard_residual_max,
            hard_residual_l2,
            rank_is_valid,
            rank,
            left_nullity,
            right_nullity,
            local_degrees_of_freedom,
            bidirectional_degrees_of_freedom,
            one_sided_mobility,
            is_singular,
            near_singular,
            rank_relative_tolerance: config.rank_relative_tolerance,
            rank_machine_tolerance,
            rank_threshold,
            singular_values,
            conflicting_sources,
            redundant_sources,
            sources_containing_redundant_rows,
            redundant_rows: all_redundancy,
            redundancy_diagnostics,
            conflict_diagnostics,
            singular_rows,
            structural: plan.structural.clone(),
            component_solves,
            priority_solves: priority_solves.to_vec(),
            trace,
            audit,
            bounds,
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
        self.decomposition_cache = Some(DecompositionCache {
            components,
            report: Some(Box::new(report.clone())),
        });
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum DirtyRequest<'a> {
    All,
    Variables(&'a [VariableId]),
    Components(&'a [usize]),
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
    enforce_state_bounds(problem, plan, &mut candidate).ok()?;
    validate_component(problem, component, &candidate, config)
        .valid
        .then_some(candidate)
}

fn trusted_cached_state(
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
    if plan.synchronize_state(problem, &mut state).is_err()
        || enforce_state_bounds(problem, plan, &mut state).is_err()
    {
        return IterationOutcome {
            termination: SolveTermination::NumericalFailure,
            state,
            trace,
        };
    }
    let mut current_hard = match linearized_hard_system(problem, plan, component, &state) {
        Ok(system) => system,
        Err(error) => {
            return IterationOutcome {
                termination: error_termination(&error),
                state,
                trace,
            };
        }
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
            let Some(mut step) = bounded_lm_step(
                problem,
                &state,
                &layout,
                &current_hard.jacobian,
                &current_hard.residuals,
                damping,
                config.normalized_step_tolerance,
            ) else {
                termination = SolveTermination::NumericalFailure;
                break;
            };
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
            let trial_hard = match linearized_hard_system(problem, plan, component, &trial_state) {
                Ok(system) => system,
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

#[derive(Debug)]
struct PriorityPassOutcome {
    state: VariableState,
    reports: Vec<PriorityReportRecord>,
}

#[derive(Debug)]
struct PriorityReportRecord {
    report: PrioritySolveReport,
    residual_ids: Vec<ResidualId>,
    reused: bool,
}

#[derive(Debug)]
struct PriorityAssignments {
    temporary: Vec<Vec<ResidualId>>,
    preference: Vec<Vec<ResidualId>>,
    fixed_temporary: Vec<ResidualId>,
    fixed_preference: Vec<ResidualId>,
    unsupported_temporary: Vec<ResidualId>,
    unsupported_preference: Vec<ResidualId>,
}

impl PriorityAssignments {
    fn new(component_count: usize) -> Self {
        Self {
            temporary: vec![Vec::new(); component_count],
            preference: vec![Vec::new(); component_count],
            fixed_temporary: Vec::new(),
            fixed_preference: Vec::new(),
            unsupported_temporary: Vec::new(),
            unsupported_preference: Vec::new(),
        }
    }

    fn component(&self, category: ResidualCategory, component: usize) -> &[ResidualId] {
        match category {
            ResidualCategory::Temporary => &self.temporary[component],
            ResidualCategory::Preference => &self.preference[component],
            ResidualCategory::Hard => &[],
        }
    }

    fn fixed(&self, category: ResidualCategory) -> &[ResidualId] {
        match category {
            ResidualCategory::Temporary => &self.fixed_temporary,
            ResidualCategory::Preference => &self.fixed_preference,
            ResidualCategory::Hard => &[],
        }
    }

    fn unsupported(&self, category: ResidualCategory) -> &[ResidualId] {
        match category {
            ResidualCategory::Temporary => &self.unsupported_temporary,
            ResidualCategory::Preference => &self.unsupported_preference,
            ResidualCategory::Hard => &[],
        }
    }
}

#[allow(clippy::too_many_lines)]
fn optimize_priorities(
    problem: &Problem,
    plan: &EliminationPlan,
    mut state: VariableState,
    config: SolverConfig,
    executions: &[ComponentExecution],
    prior_report: Option<&SolveReport>,
) -> PriorityPassOutcome {
    let assignments = classify_priority_residuals(problem, plan);
    let hard_state = state.clone();
    let mut reports = Vec::new();
    let mut temporary_succeeded = vec![true; plan.components.len()];
    let mut attained_temporary_costs = vec![None; plan.components.len()];

    for component in &plan.components {
        let residual_ids = assignments.component(ResidualCategory::Temporary, component.index);
        if residual_ids.is_empty() {
            continue;
        }
        if executions[component.index].reused {
            let report =
                cached_priority_report(prior_report, component.index, ResidualCategory::Temporary);
            let succeeded =
                report.termination == SolveTermination::Converged && report.final_cost.is_some();
            temporary_succeeded[component.index] = succeeded;
            if succeeded {
                attained_temporary_costs[component.index] = report.final_cost;
            }
            reports.push(PriorityReportRecord {
                report,
                residual_ids: residual_ids.to_vec(),
                reused: true,
            });
            continue;
        }
        let outcome = optimize_component_priority(
            problem,
            plan,
            component,
            state,
            ResidualCategory::Temporary,
            residual_ids,
            &[],
            None,
            config,
        );
        state = outcome.state;
        let succeeded = outcome.report.termination == SolveTermination::Converged
            && outcome.report.final_cost.is_some();
        temporary_succeeded[component.index] = succeeded;
        if succeeded {
            attained_temporary_costs[component.index] = outcome.report.final_cost;
        }
        reports.push(PriorityReportRecord {
            report: outcome.report,
            residual_ids: residual_ids.to_vec(),
            reused: false,
        });
    }

    let fixed_temporary = assignments.fixed(ResidualCategory::Temporary);
    if !fixed_temporary.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &state,
            ResidualCategory::Temporary,
            fixed_temporary,
            SolveTermination::Converged,
        ));
    }
    let unsupported_temporary = assignments.unsupported(ResidualCategory::Temporary);
    if !unsupported_temporary.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &hard_state,
            ResidualCategory::Temporary,
            unsupported_temporary,
            SolveTermination::NumericalFailure,
        ));
    }

    let preference_start = state.clone();
    for component in &plan.components {
        let residual_ids = assignments.component(ResidualCategory::Preference, component.index);
        if residual_ids.is_empty() || !temporary_succeeded[component.index] {
            continue;
        }
        if executions[component.index].reused {
            reports.push(PriorityReportRecord {
                report: cached_priority_report(
                    prior_report,
                    component.index,
                    ResidualCategory::Preference,
                ),
                residual_ids: residual_ids.to_vec(),
                reused: true,
            });
            continue;
        }
        let protected_temporary_ids =
            assignments.component(ResidualCategory::Temporary, component.index);
        let outcome = optimize_component_priority(
            problem,
            plan,
            component,
            state,
            ResidualCategory::Preference,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_costs[component.index],
            config,
        );
        state = outcome.state;
        reports.push(PriorityReportRecord {
            report: outcome.report,
            residual_ids: residual_ids.to_vec(),
            reused: false,
        });
    }

    let fixed_preference = assignments.fixed(ResidualCategory::Preference);
    if !fixed_preference.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &state,
            ResidualCategory::Preference,
            fixed_preference,
            SolveTermination::Converged,
        ));
    }
    let unsupported_preference = assignments.unsupported(ResidualCategory::Preference);
    if !unsupported_preference.is_empty() {
        reports.push(evaluate_nonmoving_priority(
            problem,
            &preference_start,
            ResidualCategory::Preference,
            unsupported_preference,
            SolveTermination::NumericalFailure,
        ));
    }

    refresh_priority_final_costs(problem, &state, &mut reports);
    PriorityPassOutcome { state, reports }
}

fn cached_priority_report(
    prior_report: Option<&SolveReport>,
    component_index: usize,
    category: ResidualCategory,
) -> PrioritySolveReport {
    prior_report
        .and_then(|report| {
            report.priority_solves.iter().find(|priority| {
                priority.component_index == Some(component_index) && priority.category == category
            })
        })
        .cloned()
        .map(|mut report| {
            report.iterations = 0;
            report
        })
        .unwrap_or(PrioritySolveReport {
            component_index: Some(component_index),
            category,
            iterations: 0,
            initial_cost: None,
            final_cost: None,
            attained_temporary_cost: None,
            termination: SolveTermination::NumericalFailure,
            status: SecondaryStatus::EvaluationFailure,
        })
}

fn classify_priority_residuals(problem: &Problem, plan: &EliminationPlan) -> PriorityAssignments {
    let mut assignments = PriorityAssignments::new(plan.components.len());
    for (residual_id, residual) in problem.residuals.iter() {
        let category = residual.category();
        if category == ResidualCategory::Hard {
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
        if !incidence_is_valid || components.len() > 1 {
            match category {
                ResidualCategory::Temporary => {
                    assignments.unsupported_temporary.push(residual_id);
                }
                ResidualCategory::Preference => {
                    assignments.unsupported_preference.push(residual_id);
                }
                ResidualCategory::Hard => {}
            }
            continue;
        }
        if let Some(&component) = components.first() {
            match category {
                ResidualCategory::Temporary => assignments.temporary[component].push(residual_id),
                ResidualCategory::Preference => {
                    assignments.preference[component].push(residual_id);
                }
                ResidualCategory::Hard => {}
            }
        } else {
            match category {
                ResidualCategory::Temporary => assignments.fixed_temporary.push(residual_id),
                ResidualCategory::Preference => assignments.fixed_preference.push(residual_id),
                ResidualCategory::Hard => {}
            }
        }
    }
    assignments
}

#[derive(Debug)]
struct PriorityComponentOutcome {
    state: VariableState,
    report: PrioritySolveReport,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn optimize_component_priority(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    mut state: VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_priority_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    config: SolverConfig,
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
        if !objective_within_limit(current_temporary_cost, limit) {
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
    let mut reprojection_config = config;
    reprojection_config.normalized_residual_tolerance = config
        .normalized_residual_tolerance
        .min(config.normalized_step_tolerance)
        .min(PRIORITY_REPROJECTION_TOLERANCE);
    reprojection_config.normalized_step_tolerance = config
        .normalized_step_tolerance
        .min(PRIORITY_REPROJECTION_TOLERANCE);
    for iteration in 1..=config.max_iterations {
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
                Ok(system) => match scalar_objective_gradient_row(&system) {
                    Some(gradient) => gradient,
                    None => {
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
                },
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
        let Some(nullspace) = numerical_nullspace(&protected, config.rank_relative_tolerance)
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
        if !step_is_stationary && !model_is_stationary && !constrained_step.stationary {
            let mut alpha = 1.0;
            for _ in 0..MAX_PRIORITY_LINE_SEARCH_STEPS {
                let trial_step = &step * alpha;
                let mut trial_state = state.clone();
                if apply_normalized_step(problem, plan, &mut trial_state, &layout, &trial_step)
                    .is_err()
                {
                    alpha *= 0.5;
                    continue;
                }
                let Some((accepted_state, trial_cost)) = evaluate_priority_trial(
                    problem,
                    plan,
                    component,
                    trial_state,
                    category,
                    residual_ids,
                    protected_priority_ids,
                    attained_temporary_cost,
                    config,
                    reprojection_config,
                ) else {
                    alpha *= 0.5;
                    continue;
                };
                if objective_decreases(cost, trial_cost)
                    && accepted
                        .as_ref()
                        .is_none_or(|(_, accepted_cost)| trial_cost < *accepted_cost)
                {
                    accepted = Some((accepted_state, trial_cost));
                }
                alpha *= 0.5;
            }
        }
        if let Some((accepted_state, accepted_cost)) = accepted {
            state = accepted_state;
            cost = accepted_cost;
            continue;
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
            &nullspace,
            &critical_cone,
            &layout,
            cost,
            config,
            reprojection_config,
        ) {
            CurvatureSearch::Improved(curvature_state, curvature_cost) => {
                state = curvature_state;
                cost = curvature_cost;
            }
            CurvatureSearch::NoNegativeCurvature => {
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
fn priority_component_failure(
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
fn priority_component_report(
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
            component_index: Some(component_index),
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

fn evaluate_nonmoving_priority(
    problem: &Problem,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    required_termination: SolveTermination,
) -> PriorityReportRecord {
    let (cost, evaluation_termination) =
        match priority_cost_for_residuals(problem, state, residual_ids) {
            Ok(cost) => (Some(cost), SolveTermination::Converged),
            Err(error) => (None, error_termination(&error)),
        };
    PriorityReportRecord {
        report: PrioritySolveReport {
            component_index: None,
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
        reused: false,
    }
}

fn refresh_priority_final_costs(
    problem: &Problem,
    state: &VariableState,
    reports: &mut [PriorityReportRecord],
) {
    for record in reports {
        if record.reused {
            continue;
        }
        match priority_cost_for_residuals(problem, state, &record.residual_ids) {
            Ok(cost) => record.report.final_cost = Some(cost),
            Err(error) => {
                record.report.final_cost = None;
                record.report.termination =
                    worse_termination(record.report.termination, error_termination(&error));
                record.report.status = SecondaryStatus::EvaluationFailure;
            }
        }
    }
}

fn priority_cost_for_residuals(
    problem: &Problem,
    state: &VariableState,
    residual_ids: &[ResidualId],
) -> Result<f64, CoreError> {
    let residuals =
        DVector::from_vec(problem.normalized_values_for_residuals(state, residual_ids)?);
    residual_cost(&residuals).ok_or(CoreError::NonFiniteValue {
        context: "priority residual cost",
        index: 0,
        value: f64::INFINITY,
    })
}

fn scalar_objective_gradient_row(system: &HardSystem) -> Option<DMatrix<f64>> {
    let gradient = system.jacobian.transpose() * &system.residuals;
    if gradient.iter().any(|value| !value.is_finite()) {
        return None;
    }
    Some(DMatrix::from_fn(1, gradient.len(), |_, column| {
        gradient[column]
    }))
}

/// Roundoff allowance relative only to the compared objective magnitudes.
/// There is intentionally no additive absolute floor.
fn objective_roundoff_tolerance(first: f64, second: f64) -> f64 {
    PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON * first.abs().max(second.abs())
}

fn objective_decreases(current: f64, candidate: f64) -> bool {
    current - candidate > objective_roundoff_tolerance(current, candidate)
}

fn objective_within_limit(candidate: f64, limit: f64) -> bool {
    candidate <= limit
        || candidate - limit
            <= objective_roundoff_tolerance(candidate, limit).max(PRIORITY_ZERO_COST_ROUNDOFF)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_priority_trial(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    trial_state: VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    config: SolverConfig,
    reprojection_config: SolverConfig,
) -> Option<(VariableState, f64)> {
    let reprojected = iterate_component(problem, plan, component, trial_state, reprojection_config);
    if !validate_component(problem, component, &reprojected.state, config).valid
        || linearized_hard_system(problem, plan, component, &reprojected.state).is_err()
    {
        return None;
    }
    let mut candidate_state = reprojected.state;
    if let Some(limit) = attained_temporary_cost {
        if category != ResidualCategory::Preference || protected_temporary_ids.is_empty() {
            return None;
        }
        // Correct back onto the nonlinear Temporary optimum before measuring Preference.
        let temporary_outcome = optimize_component_priority(
            problem,
            plan,
            component,
            candidate_state,
            ResidualCategory::Temporary,
            protected_temporary_ids,
            &[],
            None,
            config,
        );
        if temporary_outcome.report.termination != SolveTermination::Converged {
            return None;
        }
        candidate_state = temporary_outcome.state;
        if !validate_component(problem, component, &candidate_state, config).valid
            || linearized_hard_system(problem, plan, component, &candidate_state).is_err()
        {
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
        if !objective_within_limit(temporary_cost, limit) {
            return None;
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
    Some((candidate_state, trial_cost))
}

enum CurvatureSearch {
    Improved(VariableState, f64),
    NoNegativeCurvature,
    Incomplete,
    Failed,
}

#[allow(clippy::too_many_arguments)]
fn search_critical_cone_curvature(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    protected_nullspace: &DMatrix<f64>,
    critical_cone: &ReducedCriticalCone,
    layout: &ActiveLayout,
    current_cost: f64,
    config: SolverConfig,
    reprojection_config: SolverConfig,
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
            &full_span,
            layout,
            current_cost,
            config,
            reprojection_config,
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
        &full_span,
        &critical_cone.inequalities,
        layout,
        current_cost,
        config,
        reprojection_config,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn search_one_sided_curvature(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    span: &DMatrix<f64>,
    inequalities: &DMatrix<f64>,
    layout: &ActiveLayout,
    current_cost: f64,
    config: SolverConfig,
    reprojection_config: SolverConfig,
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
        let first_delta = DVector::from_element(1, sign * hessian_step);
        let Some(first_cost) = sample_reduced_priority_cost(
            problem,
            plan,
            component,
            state,
            category,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_cost,
            span,
            layout,
            &first_delta,
            config,
            reprojection_config,
        ) else {
            return CurvatureSearch::Failed;
        };
        let second_delta = DVector::from_element(1, sign * 2.0 * hessian_step);
        let Some(second_cost) = sample_reduced_priority_cost(
            problem,
            plan,
            component,
            state,
            category,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_cost,
            span,
            layout,
            &second_delta,
            config,
            reprojection_config,
        ) else {
            return CurvatureSearch::Failed;
        };
        let curvature =
            (second_cost - 2.0 * first_cost + current_cost) / (hessian_step * hessian_step);
        let sample_magnitude = current_cost
            .abs()
            .max(first_cost.abs())
            .max(second_cost.abs());
        let relative_tolerance = config
            .rank_relative_tolerance
            .max((PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON).sqrt());
        let stencil_roundoff =
            4.0 * PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON * sample_magnitude
                / (hessian_step * hessian_step);
        let curvature_tolerance = (curvature.abs() * relative_tolerance).max(stencil_roundoff);
        if !curvature.is_finite() || !curvature_tolerance.is_finite() {
            return CurvatureSearch::Failed;
        }
        if curvature < -curvature_tolerance
            && most_negative
                .as_ref()
                .is_none_or(|(current, _)| curvature < *current)
        {
            most_negative = Some((curvature, sign));
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
        let mut step = &direction * alpha;
        if limit_step_to_bound_events(problem, state, layout, &mut step).is_none() {
            alpha *= 0.5;
            continue;
        }
        let mut trial_state = state.clone();
        if apply_normalized_step(problem, plan, &mut trial_state, layout, &step).is_err() {
            alpha *= 0.5;
            continue;
        }
        let Some((accepted_state, trial_cost)) = evaluate_priority_trial(
            problem,
            plan,
            component,
            trial_state,
            category,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_cost,
            config,
            reprojection_config,
        ) else {
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
        alpha *= 0.5;
    }
    best.map_or(
        CurvatureSearch::Failed,
        |(improved_state, improved_cost)| CurvatureSearch::Improved(improved_state, improved_cost),
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn search_negative_curvature(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    nullspace: &DMatrix<f64>,
    layout: &ActiveLayout,
    current_cost: f64,
    config: SolverConfig,
    reprojection_config: SolverConfig,
) -> CurvatureSearch {
    let dimension = nullspace.ncols();
    if dimension == 0 {
        return CurvatureSearch::NoNegativeCurvature;
    }
    let hessian_step =
        PRIORITY_HESSIAN_NORMALIZED_STEP.min(config.max_block_normalized_step / 2.0_f64.sqrt());
    if !hessian_step.is_finite() || hessian_step <= 0.0 {
        return CurvatureSearch::Failed;
    }
    let mut hessian = DMatrix::zeros(dimension, dimension);
    let mut sample_cost_magnitude = current_cost.abs();
    for axis in 0..dimension {
        let mut delta = DVector::zeros(dimension);
        delta[axis] = hessian_step;
        let Some(positive_cost) = sample_reduced_priority_cost(
            problem,
            plan,
            component,
            state,
            category,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_cost,
            nullspace,
            layout,
            &delta,
            config,
            reprojection_config,
        ) else {
            return CurvatureSearch::Failed;
        };
        sample_cost_magnitude = sample_cost_magnitude.max(positive_cost.abs());
        delta[axis] = -hessian_step;
        let Some(negative_cost) = sample_reduced_priority_cost(
            problem,
            plan,
            component,
            state,
            category,
            residual_ids,
            protected_temporary_ids,
            attained_temporary_cost,
            nullspace,
            layout,
            &delta,
            config,
            reprojection_config,
        ) else {
            return CurvatureSearch::Failed;
        };
        sample_cost_magnitude = sample_cost_magnitude.max(negative_cost.abs());
        let diagonal =
            (positive_cost - 2.0 * current_cost + negative_cost) / (hessian_step * hessian_step);
        if !diagonal.is_finite() {
            return CurvatureSearch::Failed;
        }
        hessian[(axis, axis)] = diagonal;
    }

    for first in 0..dimension {
        for second in (first + 1)..dimension {
            let mut costs = [0.0; 4];
            for (sample, (first_sign, second_sign)) in
                [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
                    .into_iter()
                    .enumerate()
            {
                let mut delta = DVector::zeros(dimension);
                delta[first] = first_sign * hessian_step;
                delta[second] = second_sign * hessian_step;
                let Some(cost) = sample_reduced_priority_cost(
                    problem,
                    plan,
                    component,
                    state,
                    category,
                    residual_ids,
                    protected_temporary_ids,
                    attained_temporary_cost,
                    nullspace,
                    layout,
                    &delta,
                    config,
                    reprojection_config,
                ) else {
                    return CurvatureSearch::Failed;
                };
                costs[sample] = cost;
                sample_cost_magnitude = sample_cost_magnitude.max(cost.abs());
            }
            let mixed =
                (costs[0] - costs[1] - costs[2] + costs[3]) / (4.0 * hessian_step * hessian_step);
            if !mixed.is_finite() {
                return CurvatureSearch::Failed;
            }
            hessian[(first, second)] = mixed;
            hessian[(second, first)] = mixed;
        }
    }

    let eigen = hessian.symmetric_eigen();
    if eigen.eigenvalues.iter().any(|value| !value.is_finite())
        || eigen.eigenvectors.iter().any(|value| !value.is_finite())
    {
        return CurvatureSearch::Failed;
    }
    let Some((minimum_index, &minimum)) = eigen
        .eigenvalues
        .iter()
        .enumerate()
        .min_by(|(_, first), (_, second)| first.total_cmp(second))
    else {
        return CurvatureSearch::NoNegativeCurvature;
    };
    let largest = eigen
        .eigenvalues
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let relative_tolerance = config
        .rank_relative_tolerance
        .max((PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON).sqrt());
    let stencil_roundoff =
        4.0 * PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON * sample_cost_magnitude
            / (hessian_step * hessian_step);
    let curvature_tolerance = (largest * relative_tolerance).max(stencil_roundoff);
    if !curvature_tolerance.is_finite() || minimum >= -curvature_tolerance {
        return CurvatureSearch::NoNegativeCurvature;
    }

    let reduced_direction = eigen.eigenvectors.column(minimum_index).into_owned();
    let mut direction = nullspace * reduced_direction;
    if limit_block_steps(&mut direction, layout, config.max_block_normalized_step).is_none() {
        return CurvatureSearch::Failed;
    }
    let mut best: Option<(VariableState, f64)> = None;
    for sign in [1.0, -1.0] {
        let mut alpha = 1.0;
        for _ in 0..MAX_PRIORITY_LINE_SEARCH_STEPS {
            let mut step = &direction * (sign * alpha);
            if limit_step_to_bound_events(problem, state, layout, &mut step).is_none() {
                alpha *= 0.5;
                continue;
            }
            let mut trial_state = state.clone();
            if apply_normalized_step(problem, plan, &mut trial_state, layout, &step).is_err() {
                alpha *= 0.5;
                continue;
            }
            let Some((accepted_state, trial_cost)) = evaluate_priority_trial(
                problem,
                plan,
                component,
                trial_state,
                category,
                residual_ids,
                protected_temporary_ids,
                attained_temporary_cost,
                config,
                reprojection_config,
            ) else {
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
            alpha *= 0.5;
        }
    }
    best.map_or(
        CurvatureSearch::Failed,
        |(improved_state, improved_cost)| CurvatureSearch::Improved(improved_state, improved_cost),
    )
}

#[allow(clippy::too_many_arguments)]
fn sample_reduced_priority_cost(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
    category: ResidualCategory,
    residual_ids: &[ResidualId],
    protected_temporary_ids: &[ResidualId],
    attained_temporary_cost: Option<f64>,
    nullspace: &DMatrix<f64>,
    layout: &ActiveLayout,
    reduced_delta: &DVector<f64>,
    config: SolverConfig,
    reprojection_config: SolverConfig,
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
        category,
        residual_ids,
        protected_temporary_ids,
        attained_temporary_cost,
        config,
        reprojection_config,
    )
    .map(|(_, cost)| cost)
}

fn linearized_hard_system(
    problem: &Problem,
    plan: &EliminationPlan,
    component: &SolveComponent,
    state: &VariableState,
) -> Result<HardSystem, CoreError> {
    let linearization =
        problem.linearize_component(plan, component, state, &component.active_residual_ids)?;
    Ok(component_dense_system(
        linearization.project_dense(plan, ResidualCategory::Hard)?,
    ))
}

fn linearized_category_system(
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

fn stack_matrices(first: &DMatrix<f64>, second: &DMatrix<f64>) -> Option<DMatrix<f64>> {
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

fn numerical_nullspace(matrix: &DMatrix<f64>, relative_tolerance: f64) -> Option<DMatrix<f64>> {
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
    let (_, threshold) =
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

fn numerical_nullspace_for_rank(matrix: &DMatrix<f64>, rank: usize) -> Option<DMatrix<f64>> {
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

#[derive(Debug)]
struct ComponentBoundMobility {
    bidirectional_dof: usize,
    one_sided: OneSidedMobility,
    active_bounds: Vec<BoundId>,
}

fn bound_reports(problem: &Problem, state: &VariableState) -> Result<Vec<BoundReport>, CoreError> {
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
fn component_bound_mobility(
    plan: &EliminationPlan,
    component: &SolveComponent,
    equality: &DMatrix<f64>,
    rank_is_valid: bool,
    equality_rank: usize,
    relative_tolerance: f64,
    reports: &[BoundReport],
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
    let Some(equality_nullspace) = numerical_nullspace_for_rank(equality, equality_rank) else {
        return ComponentBoundMobility {
            bidirectional_dof: 0,
            one_sided: OneSidedMobility::NotEvaluated,
            active_bounds,
        };
    };
    let projected_active = projected_coordinate_normals(&equality_nullspace, &active_normals);
    let Some(active_rank) = rank_diagnostics(&projected_active, relative_tolerance) else {
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
    let Some(fixed_nullspace) = numerical_nullspace(&projected_fixed, relative_tolerance) else {
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
    let one_sided =
        feasible_inequality_cone_has_nonzero_direction(&reduced_inequalities, relative_tolerance);
    ComponentBoundMobility {
        bidirectional_dof,
        one_sided,
        active_bounds,
    }
}

fn deduplicated_bound_columns(columns: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut deduplicated = Vec::new();
    for column in columns {
        if !deduplicated.contains(&column) {
            deduplicated.push(column);
        }
    }
    deduplicated
}

fn projected_coordinate_normals(nullspace: &DMatrix<f64>, columns: &[usize]) -> DMatrix<f64> {
    DMatrix::from_fn(columns.len(), nullspace.ncols(), |row, column| {
        nullspace[(columns[row], column)]
    })
}

fn feasible_inequality_cone_has_nonzero_direction(
    inequalities: &DMatrix<f64>,
    relative_tolerance: f64,
) -> OneSidedMobility {
    let reduced_dimension = inequalities.ncols();
    if reduced_dimension == 0 {
        return OneSidedMobility::None;
    }
    if inequalities.nrows() == 0 {
        return OneSidedMobility::Exists;
    }
    let Some(inequality_rank) = rank_diagnostics(inequalities, relative_tolerance) else {
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
        let Some(face_nullspace) = numerical_nullspace(&face, relative_tolerance) else {
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

fn direction_satisfies_inequalities(inequalities: &DMatrix<f64>, direction: &DVector<f64>) -> bool {
    let Some(norm) = stable_norm(direction.iter().copied()) else {
        return false;
    };
    norm > 64.0 * f64::EPSILON
        && (inequalities * direction)
            .iter()
            .all(|value| *value >= -64.0 * f64::EPSILON * norm)
}

fn next_combination(indices: &mut [usize], population: usize) -> bool {
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

fn bound_status(bound: &CoordinateBound, value: f64) -> BoundStatus {
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

fn at_bound_endpoint(value: f64, endpoint: f64) -> bool {
    value.partial_cmp(&endpoint) == Some(std::cmp::Ordering::Equal)
}

fn bound_column(
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

fn aggregate_one_sided_mobility(components: &[ComponentSolveReport]) -> OneSidedMobility {
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

type ActiveLayout = ComponentTangentLayout;

fn active_layout(plan: &EliminationPlan, component_index: usize) -> ActiveLayout {
    plan.component_layouts[component_index].clone()
}

#[derive(Debug)]
struct HardSystem {
    residuals: DVector<f64>,
    jacobian: DMatrix<f64>,
    rows: Vec<ResidualRowRef>,
}

fn component_dense_system(system: ComponentDenseSystem) -> HardSystem {
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
    }
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
    plan.synchronize_state(problem, state)?;
    enforce_state_bounds(problem, plan, state)
}

struct ComponentValidation {
    evaluated: bool,
    valid: bool,
    hard_validity: HardValidity,
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

struct ComponentNumerics {
    termination: SolveTermination,
    rank_is_valid: bool,
    is_singular: bool,
    diagnostics: RankDiagnostics,
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
    let summary = &plan.structural.component_summaries[component.index];
    let empty = || HardSystem {
        residuals: DVector::zeros(0),
        jacobian: DMatrix::zeros(0, summary.active_tangent_dimensions),
        rows: Vec::new(),
    };
    let hard = match linearized_hard_system(problem, plan, component, state) {
        Ok(hard) => hard,
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

fn find_conflicting_sources(
    problem: &Problem,
    plan: &EliminationPlan,
    returned_state: &VariableState,
    config: SolverConfig,
    component_reports: &[ComponentSolveReport],
) -> (Vec<SourceConstraintId>, DiagnosticCompleteness) {
    let budget = config.conflict_diagnostic_budget;
    if !budget.enabled {
        return (
            Vec::new(),
            diagnostic_completeness(
                budget,
                DiagnosticWork::default(),
                Some(DiagnosticIncompleteReason::Disabled),
                false,
            ),
        );
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
        return (
            Vec::new(),
            diagnostic_completeness(
                budget,
                DiagnosticWork::default(),
                Some(DiagnosticIncompleteReason::HardConstraintsValid),
                false,
            ),
        );
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
            if deletion_restores_component(problem, plan, component, source, returned_state, config)
            {
                candidates.push(source);
                break;
            }
        }
        if stopped {
            break;
        }
    }
    let analyzed = !eligible_components.is_empty();
    (
        candidates,
        diagnostic_completeness(budget, work, reason, analyzed),
    )
}

fn diagnostic_component_budget_reason(
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

fn diagnostic_completeness(
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
    category: Option<crate::EvaluationErrorCategory>,
    error: String,
}

struct ReturnedEvaluation {
    termination: SolveTermination,
    failures: Vec<RowEvaluationFailure>,
}

fn validate_returned_rows(
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

fn merge_reused_audit(mut retained: AuditSnapshot, fresh: AuditSnapshot) -> AuditSnapshot {
    for fresh_source in fresh.sources {
        if let Some(retained_source) = retained
            .sources
            .iter_mut()
            .find(|source| source.source_id == fresh_source.source_id)
        {
            retained_source.source_label = fresh_source.source_label;
            for fresh_row in fresh_source.rows {
                if let Some(retained_row) = retained_source.rows.iter_mut().find(|row| {
                    row.residual_id == fresh_row.residual_id
                        && row.row_in_block == fresh_row.row_in_block
                }) {
                    *retained_row = fresh_row;
                }
            }
        }
    }
    retained
}

fn annotate_evaluation_failures(audit: &mut AuditSnapshot, failures: &[RowEvaluationFailure]) {
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
fn annotate_audit(
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

fn bounded_lm_step(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    damping: f64,
    normalized_step_tolerance: f64,
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
            let reduced = DMatrix::from_fn(jacobian.nrows(), free.len(), |row, column| {
                jacobian[(row, free[column])]
            });
            let active_model = jacobian * &step + residuals;
            let free_contribution = &reduced
                * DVector::from_iterator(free.len(), free.iter().map(|&column| step[column]));
            let effective_residual = active_model - free_contribution;
            let reduced_step = lm_step(&reduced, &effective_residual, damping)?;
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

#[derive(Clone, Copy, Debug)]
struct NormalizedStepBound {
    lower: f64,
    upper: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkingBound {
    Free,
    Lower,
    Upper,
    Fixed,
}

fn normalized_step_bounds(
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

fn first_step_bound_event(
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

fn kkt_gradient_tolerance(
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
struct ReducedStepBound {
    normal: DVector<f64>,
    lower: f64,
    upper: f64,
}

struct ConstrainedNullspaceStep {
    step: DVector<f64>,
    stationary: bool,
    critical_cone: Option<ReducedCriticalCone>,
}

struct ReducedCriticalCone {
    /// Basis in the protected-nullspace coordinates.
    span: DMatrix<f64>,
    /// Signed inward weak-active normals in `span` coordinates.
    inequalities: DMatrix<f64>,
}

struct WorkingSetKkt {
    release: Option<usize>,
    multipliers: Vec<f64>,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn constrained_nullspace_step(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    nullspace: &DMatrix<f64>,
    reduced_jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    relative_tolerance: f64,
    normalized_step_tolerance: f64,
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
    let mut working =
        independent_initial_working_set(&constraints, &desired_working, relative_tolerance)?;
    let maximum_iterations = 8usize.saturating_mul(constraints.len().saturating_add(1));
    for _ in 0..maximum_iterations {
        let candidate = solve_active_reduced_least_squares(
            reduced_jacobian,
            residuals,
            &constraints,
            &working,
            relative_tolerance,
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
        let kkt = working_set_kkt(&gradient, &constraints, &working, tolerance).ok()?;
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

fn snap_constrained_roundoff(
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

fn solve_active_reduced_least_squares(
    jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    relative_tolerance: f64,
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
        let particular = solve_dense_least_squares(&matrix, &right_hand_side)?.0;
        let tangent = numerical_nullspace(&matrix, relative_tolerance)?;
        (particular, tangent)
    };
    if tangent.ncols() == 0 {
        return Some(particular);
    }
    let reduced = jacobian * &tangent;
    let effective = residuals + jacobian * &particular;
    let correction = solve_dense_least_squares(&reduced, &(-effective))?.0;
    let candidate = particular + tangent * correction;
    candidate
        .iter()
        .all(|value| value.is_finite())
        .then_some(candidate)
}

fn first_linear_bound_event(
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

fn independent_initial_working_set(
    constraints: &[ReducedStepBound],
    desired: &[WorkingBound],
    relative_tolerance: f64,
) -> Option<Vec<WorkingBound>> {
    let mut working = vec![WorkingBound::Free; constraints.len()];
    for fixed_only in [true, false] {
        for (index, status) in desired.iter().copied().enumerate() {
            if status == WorkingBound::Free || (status == WorkingBound::Fixed) != fixed_only {
                continue;
            }
            if working_constraint_is_independent(constraints, &working, index, relative_tolerance)?
            {
                working[index] = status;
            }
        }
    }
    Some(working)
}

fn working_constraint_is_independent(
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    candidate: usize,
    relative_tolerance: f64,
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
    let current_nullity = numerical_nullspace(&current, relative_tolerance)?.ncols();
    let next_nullity = numerical_nullspace(&next, relative_tolerance)?.ncols();
    Some(next_nullity < current_nullity)
}

fn constraint_satisfied(constraint: &ReducedStepBound, step: &DVector<f64>) -> bool {
    let value = constraint.normal.dot(step);
    let tolerance = 64.0
        * f64::EPSILON
        * stable_norm(step.iter().copied()).unwrap_or(f64::INFINITY)
        * stable_norm(constraint.normal.iter().copied()).unwrap_or(f64::INFINITY);
    value >= constraint.lower - tolerance && value <= constraint.upper + tolerance
}

fn working_set_kkt(
    gradient: &DVector<f64>,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    tolerance: f64,
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
    let multiplier_values = solve_dense_least_squares(&matrix.transpose(), &(-gradient))
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

fn reduced_critical_cone(
    dimension: usize,
    constraints: &[ReducedStepBound],
    working: &[WorkingBound],
    multipliers: &[f64],
    relative_tolerance: f64,
    multiplier_tolerance: f64,
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
    let span = numerical_nullspace(&equality_matrix, relative_tolerance)?;
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

fn step_is_within_bounds(
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

fn limit_step_to_bound_events(
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

fn enforce_state_bounds(
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

fn project_initial_state_into_bounds(
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

fn maximum_block_step(step: &DVector<f64>, layout: &ActiveLayout) -> Option<f64> {
    layout.blocks.iter().try_fold(0.0_f64, |maximum, block| {
        stable_norm(
            step.rows(block.tangent_range.start, block.tangent_range.len())
                .iter()
                .copied(),
        )
        .map(|norm| maximum.max(norm))
    })
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
    relative_tolerance: f64,
    machine_tolerance: f64,
    threshold: f64,
    sigma_max: f64,
    smallest_retained: Option<f64>,
    near_singular_factor: f64,
    near_singular_ratio: Option<f64>,
    near_singular: bool,
    singular_values: Vec<f64>,
}

fn rank_diagnostics(jacobian: &DMatrix<f64>, relative_tolerance: f64) -> Option<RankDiagnostics> {
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
    let (machine_tolerance, threshold) = rank_thresholds(
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

fn empty_rank_diagnostics(rows: usize, columns: usize, relative_tolerance: f64) -> RankDiagnostics {
    let (machine_tolerance, threshold) =
        rank_thresholds(rows, columns, 0.0, relative_tolerance).unwrap_or((f64::MAX, f64::MAX));
    RankDiagnostics {
        rank: 0,
        relative_tolerance,
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

fn rank_thresholds(
    rows: usize,
    columns: usize,
    sigma_max: f64,
    relative_tolerance: f64,
) -> Option<(f64, f64)> {
    if !sigma_max.is_finite() || !relative_tolerance.is_finite() {
        return None;
    }
    let dimension = u32::try_from(rows.max(columns).max(1)).ok()?;
    let machine_tolerance = f64::EPSILON * f64::from(dimension) * sigma_max.max(1.0);
    let relative_threshold = relative_tolerance * sigma_max;
    let threshold = relative_threshold.max(machine_tolerance);
    (machine_tolerance.is_finite() && threshold.is_finite())
        .then_some((machine_tolerance, threshold))
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
        CoreError::InvalidGeometry { .. }
            | CoreError::CategorizedEvaluation { .. }
            | CoreError::NonFiniteValue { .. }
    )
}

fn error_termination(error: &CoreError) -> SolveTermination {
    if matches!(
        error,
        CoreError::InvalidGeometry { .. } | CoreError::CategorizedEvaluation { .. }
    ) {
        SolveTermination::InvalidGeometry
    } else {
        SolveTermination::NumericalFailure
    }
}

const fn secondary_status(termination: SolveTermination, fixed_only: bool) -> SecondaryStatus {
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

fn aggregate_hard_validity(components: &[ComponentSolveReport]) -> HardValidity {
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

fn aggregate_secondary_status(
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

const fn secondary_status_severity(status: SecondaryStatus) -> u8 {
    match status {
        SecondaryStatus::NotRequested => 0,
        SecondaryStatus::Optimal => 1,
        SecondaryStatus::Acceptable => 2,
        SecondaryStatus::Stalled => 3,
        SecondaryStatus::IterationLimit => 4,
        SecondaryStatus::EvaluationFailure => 5,
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
        let (solution, method) = solve_dense_least_squares(&matrix, &right_hand_side).unwrap();
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
        let (solution, method) = solve_dense_least_squares(&matrix, &right_hand_side).unwrap();
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
