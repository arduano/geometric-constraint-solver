use nalgebra::{DMatrix, DVector, Matrix2, Vector2};

use crate::analysis::{
    CachedComponent, DecompositionCache, EliminationPlan, SolveComponent, set_state_value,
    state_value,
};
use crate::linearization::{
    ComponentDenseSystem, ComponentIndexedSystem, ComponentLinearization, ComponentTangentLayout,
    composite_tangent_layout,
};
use crate::problem::VariableState;
use crate::sparse::{restricted_structural_nnz, solve_damped_least_squares};
use crate::{
    AuditBoundAnnotation, AuditEvaluationStatus, AuditSnapshot, BoundId, BoundReport, BoundStatus,
    CoordinateBound, CoreError, OneSidedMobility, OperationCheckpoint, OperationControl,
    OperationController, OperationOutcome, OperationWorkCounter, Problem, ResidualCategory,
    ResidualId, SourceConstraintId, StructuralSummary, VariableId,
};

mod hard;
mod priority;
mod validation;

use hard::{
    ComponentExecution, ComponentIterationObjective, DirtyRequest, NormalizedStepBound,
    ReducedCriticalCone, ReducedStepBound, WorkingBound, WorkingSetKkt, aggregate_hard_validity,
    aggregate_secondary_status, append_component_trace, cache_matches, constrained_nullspace_step,
    constraint_satisfied, empty_rank_diagnostics, enforce_state_bounds, error_termination,
    first_linear_bound_event, independent_initial_working_set, invalid_config, iterate_component,
    iterate_component_objective, kkt_gradient_tolerance, limit_block_steps, limit_operator_step,
    limit_step_to_bound_events, maximum_block_step, merge_actual_backend, normalized_step_bounds,
    operator_full_step_satisfies_bounds, operator_step_is_within_bounds, positive_finite,
    project_initial_state_into_bounds, push_unique, rank_thresholds, residual_cost, residual_norms,
    secondary_status, snap_constrained_roundoff, solve_active_reduced_least_squares,
    solve_rank_aware_least_squares, stable_norm, stamp_component_trace, step_is_within_bounds,
    trusted_cached_state, validated_cached_state, working_constraint_is_independent,
    working_set_kkt, worse_termination,
};
use priority::{
    PriorityPassOutcome, certify_current_priorities, controlled_dense_factorization,
    controlled_factorization, controlled_numerical_nullspace,
    controlled_numerical_nullspace_for_rank, controlled_rank_diagnostics,
    linearized_component_objective, merge_exact_certification_execution_provenance,
    numerical_nullspace, objective_decreases, optimize_priorities,
    variable_states_have_exact_values,
};
use validation::{
    ActiveLayout, HardSystem, active_layout, aggregate_one_sided_mobility, annotate_audit,
    annotate_evaluation_failures, apply_normalized_step, at_bound_endpoint, bound_column,
    bound_reports, candidate_sources, component_bound_mobility, component_dense_system,
    component_numerics, deduplicate_rows, diagnostic_completeness,
    diagnostic_component_budget_reason, find_conflicting_sources, find_redundancy,
    globally_fully_redundant_sources, ordered_sources, sort_redundancy, source_affects_component,
    validate_component, validate_returned_rows,
};

pub(crate) use hard::{BackendEvidence, RankDiagnostics, rank_diagnostics};
pub(crate) use priority::ExactSecondaryPreservation;

const MAX_PRIORITY_LINE_SEARCH_STEPS: usize = 20;
// Reproducing an already attained positive Temporary residual vector after lower-priority work
// cannot demand resolution below a few representable floating-point steps. This floor applies
// only to row-by-row reproduction; it does not relax Hard validation or Temporary attainment.
const PRIORITY_REPROJECTION_TOLERANCE: f64 = 8.0 * f64::EPSILON;
const PRIORITY_COST_RESOLUTION_FACTOR: f64 = 8.0;
const PRIORITY_HESSIAN_NORMALIZED_STEP: f64 = 1.0e-3;
const PROJECTED_CGLS_MIN_NULLITY: usize = 128;
const NEAR_SINGULAR_FACTOR: f64 = 100.0;
/// Auto keeps components below this hard-row count on the dense correctness path.
pub const AUTO_SPARSE_MIN_ROWS: usize = 256;
/// Auto keeps components below this active/free-column count on the dense correctness path.
pub const AUTO_SPARSE_MIN_COLUMNS: usize = 256;
/// Auto requires at least this many canonical Jacobian structural entries.
pub const AUTO_SPARSE_MIN_NNZ: usize = 256;
const AUTO_SPARSE_MAX_DENSITY_DENOMINATOR: usize = 128;
/// Auto uses sparse QR only at or below the measured 256-column chain envelope.
pub const AUTO_SPARSE_MAX_DENSITY: f64 = 1.0 / 128.0;
// This is only the squared residual roundoff band for preserving an attained zero cost.
const PRIORITY_ZERO_COST_ROUNDOFF: f64 = 0.5 * (8.0 * f64::EPSILON) * (8.0 * f64::EPSILON);

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

/// Deterministic policy selecting the hard LM linear solve backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum LinearSolveBackendPolicy {
    /// Use the documented row/column/nnz/density crossover constants.
    #[default]
    Auto,
    /// Always use the existing dense QR/SVD least-squares path.
    DenseOnly,
    /// Attempt sparse QR for every nonempty free-column LM solve and fall back truthfully.
    SparsePreferred,
}

/// Backend that actually produced one or more hard LM steps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LinearSolveBackend {
    Dense,
    SparseQr,
    /// Both backends produced steps during this component/report.
    Mixed,
}

/// Why an attempted sparse LM solve used the dense fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SparseFallbackReason {
    /// Auto retained dense solving because current numerical rank was ambiguous.
    RankAmbiguous,
    ConstructionFailure,
    SymbolicAnalysisFailure,
    NumericFactorizationFailure,
    SolutionValidationFailure,
}

/// Geometric scope of one deterministic secondary optimization group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrioritySolveScope {
    /// At least one active hard-component tangent participates.
    Movable,
    /// Every incident coordinate was removed by trusted fixed elimination.
    Fixed,
    /// At least one incident variable could not be mapped through the accepted plan.
    InvalidIncidence,
}

/// Linear hierarchy implementation used by one priority pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrioritySolveBackend {
    /// Existing singleton dense nullspace, active-set, and curvature path.
    DenseNullspace,
    /// Coupled solve using independently materialized component nullspaces.
    DenseBlockNullspace,
    /// Coupled unbounded least squares using block-local projector operations and CGLS.
    ProjectedCgls,
}

/// Evidence that one independently attained Temporary group survived a Preference pass.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ProtectedTemporaryReport {
    pub group_index: usize,
    pub attained_cost: f64,
    pub final_cost: Option<f64>,
    pub preservation_tolerance: f64,
    pub preserved: bool,
}

/// Aggregate outcome for one requested secondary priority level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SecondaryStatus {
    /// No residuals requested this priority level.
    NotRequested,
    /// The requested secondary objective met its optimum criteria.
    Optimal,
    /// The finite result is acceptable for commit, but second-order optimality
    /// is unproved (or immutable hard state prevents further optimization).
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
    /// Scale- and value-independent reduced block-envelope sparsity signature.
    pub sparsity_signature: u64,
    /// Canonical reduced hard block-envelope entries, including explicit zero slots.
    pub structural_nnz: usize,
    /// Public policy requested for hard LM solves.
    pub requested_backend: LinearSolveBackendPolicy,
    /// Backend(s) that produced a hard LM step, including hard reprojection
    /// performed inside a secondary trial; `None` means no hard LM solve ran.
    pub actual_backend: Option<LinearSolveBackend>,
    /// Whether any successful sparse numeric solve reused an immutable symbolic QR entry.
    pub symbolic_analysis_reused: bool,
    /// Number of successful sparse numeric solves that reused an exact symbolic cache entry.
    pub symbolic_analysis_reuse_count: usize,
    /// First deterministic sparse failure that required a dense fallback.
    pub sparse_fallback_reason: Option<SparseFallbackReason>,
    /// Whether the existing decomposed cache supplied this component result.
    pub reused: bool,
    /// Whether a rerun secondary group included this hard component.
    pub secondary_participated: bool,
    /// Whether secondary optimization changed this component after its hard solve/reuse.
    pub state_changed_by_secondary: bool,
    /// Hard nonlinear iterations only. Secondary outer iterations are reported
    /// by `PrioritySolveReport::iterations`.
    pub iterations: usize,
    /// Hard component solve/numerical-report termination. Secondary termination
    /// is reported separately by `PrioritySolveReport` and aggregated globally.
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
    /// Hard nonlinear trace only. Reused components always have an empty trace;
    /// secondary hard reprojection may still contribute backend evidence above.
    pub trace: SolveTrace,
}

/// Outcome of one deterministic secondary coupling group.
///
/// `component_index` is retained only for singleton movable groups. Use
/// `component_indices` and `scope` for cross-component, fixed-only, and invalid
/// incidence. Costs are absent when initial evaluation failed.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PrioritySolveReport {
    /// Stable zero-based group index within this category.
    pub group_index: usize,
    pub component_index: Option<usize>,
    /// All participating hard components in deterministic hard-component order.
    pub component_indices: Vec<usize>,
    pub scope: PrioritySolveScope,
    pub backend: Option<PrioritySolveBackend>,
    /// Largest full-coordinate row count of any explicitly stored local nullspace block.
    pub largest_explicit_nullspace_block_rows: usize,
    /// Independently protected Temporary levels for a Preference group.
    pub protected_temporary: Vec<ProtectedTemporaryReport>,
    /// Priority category represented by this pass.
    pub category: ResidualCategory,
    /// Iterations consumed by this category-level pass.
    pub iterations: usize,
    /// Initial dimensionless `0.5 * ||r||^2`, absent when evaluation failed.
    pub initial_cost: Option<f64>,
    /// Returned-state dimensionless `0.5 * ||r||^2`, absent when evaluation failed.
    pub final_cost: Option<f64>,
    /// Cost attained by this Temporary pass, or the legacy singleton Temporary
    /// cost protected by this Preference pass. Cross-component Preference
    /// callers should inspect `protected_temporary` instead.
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
    /// Sum of canonical reduced hard-Jacobian structural entries.
    pub structural_nnz: usize,
    /// Public policy requested for hard LM solves.
    pub requested_backend: LinearSolveBackendPolicy,
    /// Backend(s) that actually produced hard LM steps; `None` means no such solve ran.
    pub actual_backend: Option<LinearSolveBackend>,
    /// Whether any successful sparse numeric solve reused symbolic QR analysis.
    pub symbolic_analysis_reused: bool,
    /// Aggregate successful exact symbolic-cache reuse count.
    pub symbolic_analysis_reuse_count: usize,
    /// First component-ordered reason Auto or a sparse failure selected dense fallback.
    pub sparse_fallback_reason: Option<SparseFallbackReason>,
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
    pub linear_solve_backend: LinearSolveBackendPolicy,
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
            linear_solve_backend: LinearSolveBackendPolicy::Auto,
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
    ///
    /// # Panics
    ///
    /// Panics only if the internal unlimited path reports an interruption
    /// without an operation controller.
    pub fn solve(&mut self, config: SolverConfig) -> Result<SolveReport, CoreError> {
        self.solve_reduced(config, DirtyRequest::All, None, &[], None)
            .map(|report| report.expect("uncontrolled solving cannot be interrupted"))
    }

    /// Solves on a scratch clone and publishes it only after the final checkpoint.
    ///
    /// Cancellation and work exhaustion are operation outcomes, never solver
    /// terminations. An interrupted call leaves this problem bitwise unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration or stale/invalid static
    /// declarations. Evaluator and numerical failures remain report outcomes.
    pub fn solve_controlled(
        &mut self,
        config: SolverConfig,
        control: OperationControl,
    ) -> Result<OperationOutcome<SolveReport>, CoreError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::ComponentBoundary)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let report = self.solve_with_controller(config, &mut controller)?;
        let Some(report) = report else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(report))
    }

    /// Runs a solve under an operation controller shared with a domain pipeline.
    ///
    /// The candidate is cloned and swapped only after final validation and the
    /// precommit checkpoint. `Ok(None)` means the controller records the exact
    /// cancellation or work-exhaustion reason.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration or stale/invalid static
    /// declarations. Evaluator and numerical failures remain report outcomes.
    #[doc(hidden)]
    pub fn solve_with_controller(
        &mut self,
        config: SolverConfig,
        controller: &mut OperationController,
    ) -> Result<Option<SolveReport>, CoreError> {
        let mut candidate = self.clone();
        let report =
            candidate.solve_reduced(config, DirtyRequest::All, None, &[], Some(controller))?;
        let Some(report) = report else {
            return Ok(None);
        };
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(None);
        }
        *self = candidate;
        Ok(Some(report))
    }

    /// Freshly certifies the exact materialized state without optimization.
    ///
    /// This rebuilds hard, rank, bound, secondary, diagnostic, and audit
    /// evidence but never projects, iterates, or changes a coordinate. A
    /// movable secondary objective is success-like only when its exact
    /// evaluated cost is zero.
    ///
    /// `Ok(None)` means operation control stopped before a complete report was
    /// available. This problem is unchanged in every outcome.
    #[doc(hidden)]
    pub fn certify_current_state_with_controller(
        &self,
        config: SolverConfig,
        controller: &mut OperationController,
    ) -> Result<Option<SolveReport>, CoreError> {
        self.certify_current_state_inner(config, None, controller)
            .map(|certification| certification.map(|(report, _)| report))
    }

    pub(crate) fn certify_current_state_preserving_secondary_with_controller(
        &self,
        retained: &Self,
        retained_report: &SolveReport,
        config: SolverConfig,
        controller: &mut OperationController,
    ) -> Result<Option<(SolveReport, ExactSecondaryPreservation)>, CoreError> {
        self.certify_current_state_inner(config, Some((retained, retained_report)), controller)
    }

    fn certify_current_state_inner(
        &self,
        config: SolverConfig,
        retained: Option<(&Self, &SolveReport)>,
        controller: &mut OperationController,
    ) -> Result<Option<(SolveReport, ExactSecondaryPreservation)>, CoreError> {
        config.validate()?;
        if controller
            .checkpoint(OperationCheckpoint::ComponentBoundary)
            .is_err()
        {
            return Ok(None);
        }

        let structural_items = self
            .variables
            .iter()
            .count()
            .saturating_add(self.residuals.iter().count())
            .saturating_add(self.sources.iter().count())
            .saturating_add(self.bounds.iter().count())
            .saturating_add(self.fixed_eliminations.len())
            .saturating_add(self.alias_eliminations.len());
        if controller
            .charge(
                OperationWorkCounter::DocumentDependencyItems,
                structural_items,
                OperationCheckpoint::DocumentDependency,
            )
            .is_err()
        {
            return Ok(None);
        }

        let plan = EliminationPlan::new(self)?;
        let state = self.variable_state();
        let mut canonical = state.clone();
        plan.synchronize_state(self, &mut canonical)?;
        enforce_state_bounds(self, &plan, &mut canonical)?;
        if !variable_states_have_exact_values(&state, &canonical) {
            return Err(CoreError::InvalidAcceptedLinearization {
                context: "materialized state requires fixed, alias, or bound canonicalization",
            });
        }

        let executions = plan
            .components
            .iter()
            .map(|component| ComponentExecution {
                component_index: component.index,
                reused: false,
                termination: SolveTermination::Converged,
                trace: SolveTrace::default(),
            })
            .collect::<Vec<_>>();
        let Some((
            PriorityPassOutcome {
                reports,
                component_participated,
                component_state_changed,
                ..
            },
            secondary_preservation,
        )) = certify_current_priorities(self, retained, &plan, &state, config, controller)?
        else {
            return Ok(None);
        };
        let priority_reports = reports
            .into_iter()
            .map(|record| record.report)
            .collect::<Vec<_>>();
        let backend_evidence =
            vec![BackendEvidence::new(config.linear_solve_backend); plan.components.len()];

        if controller
            .checkpoint(OperationCheckpoint::BeforeFinalValidation)
            .is_err()
        {
            return Ok(None);
        }
        let Some(mut report) = self.build_report(
            config,
            &plan,
            &executions,
            &priority_reports,
            &component_participated,
            &component_state_changed,
            &backend_evidence,
            Some(controller),
        )?
        else {
            return Ok(None);
        };
        if let Some((_, retained_report)) = retained {
            merge_exact_certification_execution_provenance(&mut report, retained_report);
        }
        if controller
            .checkpoint(OperationCheckpoint::AfterFinalValidation)
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some((report, secondary_preservation)))
    }

    /// Solves edited/cache-invalid components and reuses independently validated cache entries.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration, unknown edited IDs, or
    /// stale/invalid static declarations.
    ///
    /// # Panics
    ///
    /// Panics only if the internal unlimited path reports an interruption
    /// without an operation controller.
    pub fn solve_decomposed(
        &mut self,
        config: SolverConfig,
        edited_variables: &[VariableId],
    ) -> Result<SolveReport, CoreError> {
        self.solve_reduced(
            config,
            DirtyRequest::Variables(edited_variables),
            None,
            &[],
            None,
        )
        .map(|report| report.expect("uncontrolled solving cannot be interrupted"))
    }

    pub(crate) fn solve_session_components(
        &mut self,
        config: SolverConfig,
        dirty_components: &[usize],
        dirty_hierarchy_residuals: &[ResidualId],
        cached_plan: &EliminationPlan,
    ) -> Result<SolveReport, CoreError> {
        self.solve_reduced(
            config,
            DirtyRequest::Components(dirty_components),
            Some(cached_plan),
            dirty_hierarchy_residuals,
            None,
        )
        .map(|report| report.expect("uncontrolled solving cannot be interrupted"))
    }

    pub(crate) fn solve_session_components_with_controller(
        &mut self,
        config: SolverConfig,
        dirty_components: &[usize],
        dirty_hierarchy_residuals: &[ResidualId],
        cached_plan: &EliminationPlan,
        controller: &mut OperationController,
    ) -> Result<Option<SolveReport>, CoreError> {
        self.solve_reduced(
            config,
            DirtyRequest::Components(dirty_components),
            Some(cached_plan),
            dirty_hierarchy_residuals,
            Some(controller),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn solve_reduced(
        &mut self,
        config: SolverConfig,
        dirty_request: DirtyRequest<'_>,
        cached_plan: Option<&EliminationPlan>,
        dirty_hierarchy_residuals: &[ResidualId],
        mut control: Option<&mut OperationController>,
    ) -> Result<Option<SolveReport>, CoreError> {
        config.validate()?;
        let plan = if let Some(plan) = cached_plan {
            plan.clone()
        } else {
            EliminationPlan::new(self)?
        };
        *self.solve_backend_evidence.borrow_mut() =
            vec![BackendEvidence::new(config.linear_solve_backend); plan.components.len()];
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
            if let Some(controller) = control.as_deref_mut()
                && controller
                    .checkpoint(OperationCheckpoint::ComponentBoundary)
                    .is_err()
            {
                return Ok(None);
            }
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

            if let Some(controller) = control.as_deref_mut()
                && controller
                    .charge(
                        OperationWorkCounter::ComponentLinearizations,
                        1,
                        OperationCheckpoint::ComponentBoundary,
                    )
                    .is_err()
            {
                return Ok(None);
            }
            let Some(mut outcome) = iterate_component(
                self,
                &plan,
                component,
                state,
                config,
                control.as_deref_mut(),
            ) else {
                return Ok(None);
            };
            stamp_component_trace(&mut outcome.trace, component.index);
            state = outcome.state;
            executions.push(ComponentExecution {
                component_index: component.index,
                reused: false,
                termination: outcome.termination,
                trace: outcome.trace,
            });
        }

        let Some(PriorityPassOutcome {
            state: priority_state,
            reports: priority_records,
            component_participated,
            component_state_changed,
        }) = optimize_priorities(
            self,
            &plan,
            state,
            config,
            &executions,
            prior_report,
            dirty_hierarchy_residuals,
            control.as_deref_mut(),
        )
        else {
            return Ok(None);
        };
        state = priority_state;
        let priority_reports: Vec<_> = priority_records
            .into_iter()
            .map(|record| record.report)
            .collect();

        self.replace_variable_state(&state)?;
        if let Some(controller) = control.as_deref_mut()
            && controller
                .checkpoint(OperationCheckpoint::BeforeFinalValidation)
                .is_err()
        {
            return Ok(None);
        }
        let backend_evidence = self.solve_backend_evidence.borrow().clone();
        let Some(report) = self.build_report(
            config,
            &plan,
            &executions,
            &priority_reports,
            &component_participated,
            &component_state_changed,
            &backend_evidence,
            control.as_deref_mut(),
        )?
        else {
            return Ok(None);
        };
        if let Some(controller) = control
            && controller
                .checkpoint(OperationCheckpoint::AfterFinalValidation)
                .is_err()
        {
            return Ok(None);
        }
        self.update_decomposition_cache(&plan, &report)?;
        Ok(Some(report))
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn build_report(
        &self,
        config: SolverConfig,
        plan: &EliminationPlan,
        executions: &[ComponentExecution],
        priority_solves: &[PrioritySolveReport],
        component_secondary_participated: &[bool],
        component_state_changed_by_secondary: &[bool],
        backend_evidence: &[BackendEvidence],
        mut control: Option<&mut OperationController>,
    ) -> Result<Option<SolveReport>, CoreError> {
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
            if let Some(controller) = control.as_deref_mut()
                && controller
                    .checkpoint(OperationCheckpoint::ComponentBoundary)
                    .is_err()
            {
                return Ok(None);
            }
            let execution = executions
                .iter()
                .find(|execution| execution.component_index == component.index)
                .ok_or(CoreError::DimensionMismatch {
                    context: "component execution report",
                    expected: plan.components.len(),
                    actual: executions.len(),
                })?;
            let backend = backend_evidence.get(component.index).copied().ok_or(
                CoreError::DimensionMismatch {
                    context: "component backend evidence",
                    expected: plan.components.len(),
                    actual: component.index,
                },
            )?;
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
            if let Some(controller) = control.as_deref_mut() {
                let summary = &plan.structural.component_summaries[component.index];
                if controller
                    .authorize_dense_kernel(
                        summary.active_hard_rows,
                        summary.active_tangent_dimensions,
                        OperationCheckpoint::BeforeRankKernel,
                    )
                    .is_err()
                    || controller
                        .charge(
                            OperationWorkCounter::RankKernels,
                            1,
                            OperationCheckpoint::BeforeRankKernel,
                        )
                        .is_err()
                {
                    return Ok(None);
                }
            }
            let numerical = component_numerics(self, plan, component, &state, config);
            if let Some(controller) = control.as_deref_mut()
                && controller
                    .checkpoint(OperationCheckpoint::AfterRankKernel)
                    .is_err()
            {
                return Ok(None);
            }
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
                        let Some(redundancy) = find_redundancy(
                            &numerical.hard,
                            &validation.rows,
                            &source_order,
                            numerical.diagnostics.threshold,
                            config.normalized_residual_tolerance,
                            control.as_deref_mut(),
                        ) else {
                            return Ok(None);
                        };
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
                control.as_deref_mut(),
            );
            if control
                .as_ref()
                .is_some_and(|controller| controller.is_stopped())
            {
                return Ok(None);
            }
            component_solves.push(ComponentSolveReport {
                component_index: component.index,
                pattern_signature: summary.pattern_signature,
                sparsity_signature: summary.sparsity_signature,
                structural_nnz: summary.structural_nnz,
                requested_backend: backend.requested,
                actual_backend: backend.actual,
                symbolic_analysis_reused: backend.symbolic_reuse_count > 0,
                symbolic_analysis_reuse_count: backend.symbolic_reuse_count,
                sparse_fallback_reason: backend.fallback_reason,
                reused: execution.reused,
                secondary_participated: component_secondary_participated
                    .get(component.index)
                    .copied()
                    .unwrap_or(false),
                state_changed_by_secondary: component_state_changed_by_secondary
                    .get(component.index)
                    .copied()
                    .unwrap_or(false),
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
        let Some((conflicting_sources, conflict_diagnostics)) =
            find_conflicting_sources(self, plan, &state, config, &component_solves, control)
        else {
            return Ok(None);
        };
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
        let actual_backend = component_solves.iter().fold(None, |aggregate, component| {
            merge_actual_backend(aggregate, component.actual_backend)
        });
        let symbolic_analysis_reuse_count = component_solves
            .iter()
            .map(|component| component.symbolic_analysis_reuse_count)
            .sum();
        let sparse_fallback_reason = component_solves
            .iter()
            .find_map(|component| component.sparse_fallback_reason);
        // Cache reuse may skip optimization, never returned-state value/Jacobian
        // evaluation. Audit and secondary costs must describe this exact state.
        let returned_evaluation = validate_returned_rows(self, &state, None);
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
        Ok(Some(SolveReport {
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
            structural_nnz: plan.structural.structural_nnz,
            requested_backend: config.linear_solve_backend,
            actual_backend,
            symbolic_analysis_reused: symbolic_analysis_reuse_count > 0,
            symbolic_analysis_reuse_count,
            sparse_fallback_reason,
            structural: plan.structural.clone(),
            component_solves,
            priority_solves: priority_solves.to_vec(),
            trace,
            audit,
            bounds,
        }))
    }

    pub(crate) fn update_decomposition_cache(
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
