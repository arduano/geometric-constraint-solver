use nalgebra::{DMatrix, DVector};

use crate::analysis::{
    CachedComponent, DecompositionCache, EliminationPlan, SolveComponent, set_state_value,
};
use crate::linearization::{
    ComponentDenseSystem, ComponentTangentLayout, component_tangent_layout,
};
use crate::problem::VariableState;
use crate::{
    AuditEvaluationStatus, AuditSnapshot, CoreError, Problem, ResidualCategory, ResidualId,
    SourceConstraintId, StructuralSummary, VariableId,
};

const MAX_CONFLICT_COMPONENT_SOURCES: usize = 12;
const MAX_CONFLICT_COMPONENT_DIMENSION: usize = 24;
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

        let PriorityPassOutcome {
            state: priority_state,
            reports: priority_records,
        } = optimize_priorities(self, &plan, state, config);
        state = priority_state;
        let priority_reports: Vec<_> = priority_records
            .into_iter()
            .map(|record| record.report)
            .collect();

        self.replace_variable_state(&state)?;
        let report = self.build_report(config, &plan, &executions, &priority_reports)?;
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
                    numerical.diagnostics.threshold,
                    config.normalized_residual_tolerance,
                );
                all_redundancy.extend(redundancy.rows);
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
                rank_is_valid: numerical.rank_is_valid,
                rank: numerical.diagnostics.rank,
                left_nullity,
                right_nullity,
                local_degrees_of_freedom: right_nullity,
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
        let left_nullity = component_solves
            .iter()
            .map(|component| component.left_nullity)
            .sum();
        let right_nullity = component_solves
            .iter()
            .map(|component| component.right_nullity)
            .sum();
        let local_degrees_of_freedom = right_nullity;
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
        let returned_evaluation = validate_returned_rows(self, &state);
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
            singular_rows,
            structural: plan.structural.clone(),
            component_solves,
            priority_solves: priority_solves.to_vec(),
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
        let reduced_gradient = reduced_jacobian.transpose() * &current.residuals;
        let Some(gradient_is_stationary) = projected_gradient_is_stationary(
            &reduced_jacobian,
            &current.residuals,
            &reduced_gradient,
            config.normalized_step_tolerance,
        ) else {
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
        };
        let right_hand_side = -&current.residuals;
        let Some((reduced_step, _)) =
            solve_dense_least_squares(&reduced_jacobian, &right_hand_side)
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
        let mut step = &nullspace * reduced_step;
        let layout = active_layout(plan, component.index);
        let Some(normalized_step_max) =
            limit_block_steps(&mut step, &layout, config.max_block_normalized_step)
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
        if !step_is_stationary && !model_is_stationary && !gradient_is_stationary {
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

        match search_negative_curvature(
            problem,
            plan,
            component,
            &state,
            category,
            residual_ids,
            protected_priority_ids,
            attained_temporary_cost,
            &nullspace,
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
            CurvatureSearch::Failed => {
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
    }
}

fn refresh_priority_final_costs(
    problem: &Problem,
    state: &VariableState,
    reports: &mut [PriorityReportRecord],
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

fn projected_gradient_is_stationary(
    reduced_jacobian: &DMatrix<f64>,
    residuals: &DVector<f64>,
    gradient: &DVector<f64>,
    normalized_step_tolerance: f64,
) -> Option<bool> {
    let jacobian_norm = stable_norm(reduced_jacobian.iter().copied())?;
    let residual_norm = stable_norm(residuals.iter().copied())?;
    let gradient_norm = stable_norm(gradient.iter().copied())?;
    let roundoff_tolerance =
        (PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON).sqrt() * jacobian_norm * residual_norm;
    let step_tolerance = normalized_step_tolerance * jacobian_norm * jacobian_norm;
    let tolerance = roundoff_tolerance.max(step_tolerance);
    tolerance.is_finite().then_some(gradient_norm <= tolerance)
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
    Failed,
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
            let step = &direction * (sign * alpha);
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

type ActiveLayout = ComponentTangentLayout;

fn active_layout(plan: &EliminationPlan, component_index: usize) -> ActiveLayout {
    component_tangent_layout(plan, component_index)
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
    plan.synchronize_state(problem, state)
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
    category: Option<crate::EvaluationErrorCategory>,
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
}
