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

#[derive(Clone, Copy, Debug)]
pub(crate) struct BackendEvidence {
    requested: LinearSolveBackendPolicy,
    actual: Option<LinearSolveBackend>,
    symbolic_reuse_count: usize,
    fallback_reason: Option<SparseFallbackReason>,
}

impl BackendEvidence {
    const fn new(requested: LinearSolveBackendPolicy) -> Self {
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

fn record_backend_evidence(problem: &Problem, component_index: usize, evidence: BackendEvidence) {
    let mut aggregate = problem.solve_backend_evidence.borrow_mut();
    // Conflict diagnostics may solve a temporary suppression plan after the
    // accepted-plan component reports have already captured their evidence.
    if let Some(component) = aggregate.get_mut(component_index) {
        component.merge(evidence);
    }
}

const fn merge_actual_backend(
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
enum ComponentIterationObjective<'a> {
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
fn iterate_component_objective(
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

fn charge_rejected_trial<C>(control: &mut Option<C>) -> bool
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

#[derive(Debug)]
struct PriorityPassOutcome {
    state: VariableState,
    reports: Vec<PriorityReportRecord>,
    component_participated: Vec<bool>,
    component_state_changed: Vec<bool>,
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

fn merge_exact_certification_execution_provenance(fresh: &mut SolveReport, retained: &SolveReport) {
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
struct PriorityReportRecord {
    report: PrioritySolveReport,
    residual_ids: Vec<ResidualId>,
}

#[derive(Debug)]
struct PriorityGroup {
    group_index: usize,
    component_indices: Vec<usize>,
    residual_ids: Vec<ResidualId>,
    protected_temporary_groups: Vec<usize>,
}

#[derive(Debug)]
struct PriorityCategoryPlan {
    movable: Vec<PriorityGroup>,
    fixed: Vec<ResidualId>,
    invalid: Vec<ResidualId>,
}

#[derive(Debug)]
struct PriorityPlan {
    temporary: PriorityCategoryPlan,
    preference: PriorityCategoryPlan,
}

#[derive(Debug)]
struct PriorityIncidence {
    residual_id: ResidualId,
    component_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
struct TemporaryLevel {
    group_index: usize,
    component_indices: Vec<usize>,
    residual_ids: Vec<ResidualId>,
    attained_cost: f64,
}

#[derive(Debug)]
struct DisjointSet {
    parents: Vec<usize>,
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
fn optimize_priorities(
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
fn certify_current_priorities(
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

fn priority_category_residual_ids(category: &PriorityCategoryPlan) -> Vec<ResidualId> {
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

fn category_residual_preservation(
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
fn certify_preserved_priority_group(
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

fn priority_cost_within_vector_limit(
    candidate: f64,
    attained: f64,
    residual_rows: usize,
    config: SolverConfig,
) -> bool {
    candidate <= attained
        || candidate - attained <= residual_vector_cost_tolerance(attained, residual_rows, config)
}

fn charge_priority_certification(
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

fn certify_movable_priority(
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

fn cached_priority_report(
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

fn cached_temporary_attained_cost(report: &PrioritySolveReport) -> Option<f64> {
    report.attained_temporary_cost.or(report.final_cost)
}

fn build_priority_plan(problem: &Problem, plan: &EliminationPlan) -> PriorityPlan {
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

fn classify_priority_incidence(
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

fn union_priority_hyperedges(dsu: &mut DisjointSet, incidences: &[PriorityIncidence]) {
    for incidence in incidences {
        union_components(dsu, &incidence.component_indices);
    }
}

fn union_components(dsu: &mut DisjointSet, component_indices: &[usize]) {
    if let Some((&first, rest)) = component_indices.split_first() {
        for &component_index in rest {
            dsu.union(first, component_index);
        }
    }
}

fn priority_groups(incidences: &[PriorityIncidence], dsu: &mut DisjointSet) -> Vec<PriorityGroup> {
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

fn priority_group_is_dirty(
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

fn component_state_changed(
    component: &SolveComponent,
    before: &VariableState,
    after: &VariableState,
) -> bool {
    component
        .variable_ids
        .iter()
        .any(|&variable_id| state_value(before, variable_id) != state_value(after, variable_id))
}

fn variable_states_have_exact_values(first: &VariableState, second: &VariableState) -> bool {
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
fn optimize_priority_group(
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

fn decorate_priority_report(
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

fn protected_reports(protected: &[TemporaryLevel]) -> Vec<ProtectedTemporaryReport> {
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

fn priority_group_failure_report(
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
struct LocalNullspaceBlock {
    full_range: std::ops::Range<usize>,
    reduced_range: std::ops::Range<usize>,
    map: LocalNullspaceMap,
}

#[derive(Debug)]
enum LocalNullspaceMap {
    Explicit(DMatrix<f64>),
    Identity,
}

#[derive(Debug)]
struct BlockProtectedSpace {
    blocks: Vec<LocalNullspaceBlock>,
    full_dimension: usize,
    reduced_dimension: usize,
    largest_block_rows: usize,
    protected_rows: DMatrix<f64>,
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

struct EqualityProjector {
    rows: DMatrix<f64>,
    row_space_basis: DMatrix<f64>,
}

impl EqualityProjector {
    fn new(
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

    fn project(&self, value: &DVector<f64>, tolerance: f64) -> Option<DVector<f64>> {
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
fn optimize_coupled_priority(
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
fn optimize_coupled_priority_inner(
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
            operator_step_is_within_bounds(problem, &state, &layout, &step)
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
fn coupled_priority_report(
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

fn block_protected_space(
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
fn dense_block_constrained_step(
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

fn projected_cgls_step(
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
fn projected_cgls_correction(
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
fn bounded_projected_cgls_step(
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
            if operator_bound_is_independent(
                space,
                &constraints,
                &working,
                index,
                rank_tolerance,
                step_tolerance,
                control.as_deref_mut(),
            )? {
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

        let full_step = space.apply_local_bases(&step)?;
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

fn operator_bound_constraints(
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

fn operator_equality_rows(
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

fn operator_bound_is_independent(
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

enum OperatorBoundEvent {
    None,
    Event(f64, usize, WorkingBound),
}

fn first_operator_bound_event(
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

fn operator_working_set_kkt(
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

fn operator_active_equalities_satisfied(
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

fn priority_group_has_bounds(
    problem: &Problem,
    plan: &EliminationPlan,
    group: &PriorityGroup,
) -> bool {
    problem.bounds().any(|(_, bound)| {
        plan.component_for_variable(bound.variable_id())
            .is_some_and(|component| group.component_indices.contains(&component))
    })
}

fn validate_priority_components(
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

fn protected_levels_are_preserved(
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
fn evaluate_coupled_priority_trial(
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
fn search_coupled_negative_curvature(
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

fn curvature_stencil_coordinate_radius(
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
fn sample_coupled_priority_cost(
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
struct PriorityComponentOutcome {
    state: VariableState,
    report: PrioritySolveReport,
}

fn acceptable_secondary_outcome(mut outcome: PriorityComponentOutcome) -> PriorityComponentOutcome {
    debug_assert_eq!(outcome.report.termination, SolveTermination::Converged);
    outcome.report.status = SecondaryStatus::Acceptable;
    outcome
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn optimize_component_priority(
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

fn attained_temporary_residual_target(
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

fn residual_target_rows_are_preserved(
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

fn residual_target_row_tolerance(config: SolverConfig) -> f64 {
    // Preserve at the tighter configured solve tolerance unless that value is below the
    // documented machine reproducibility floor.
    config
        .normalized_residual_tolerance
        .min(config.normalized_step_tolerance)
        .max(PRIORITY_REPROJECTION_TOLERANCE)
}

fn priority_component_cost(
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
fn positive_temporary_candidate_is_valid(
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

fn component_has_movable_priority_incidence(
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
fn optimize_component_priority_inner(
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

fn evaluate_nonmoving_priority(
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

fn refresh_priority_final_costs(
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

fn priority_cost_for_residuals(
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

fn priority_residual_rows(problem: &Problem, residual_ids: &[ResidualId]) -> usize {
    residual_ids
        .iter()
        .filter_map(|&residual| problem.residual(residual))
        .map(crate::ResidualBlock::output_dimension)
        .sum()
}

/// Roundoff allowance relative only to the compared objective magnitudes.
/// There is intentionally no additive absolute floor.
fn objective_roundoff_tolerance(first: f64, second: f64) -> f64 {
    PRIORITY_COST_RESOLUTION_FACTOR * f64::EPSILON * first.abs().max(second.abs())
}

fn objective_decreases(current: f64, candidate: f64) -> bool {
    current - candidate > objective_roundoff_tolerance(current, candidate)
}

fn charge_rejected_priority_trial<C>(control: &mut Option<C>) -> bool
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

fn objective_within_limit(candidate: f64, limit: f64) -> bool {
    candidate <= limit
        || candidate - limit
            <= objective_roundoff_tolerance(candidate, limit).max(PRIORITY_ZERO_COST_ROUNDOFF)
}

fn priority_zero_cost_limit(residual_rows: usize, config: SolverConfig) -> f64 {
    let rows = f64::from(u32::try_from(residual_rows.max(1)).unwrap_or(u32::MAX));
    let residual_resolution = (PRIORITY_COST_RESOLUTION_FACTOR
        * config.normalized_residual_tolerance)
        .max(config.normalized_step_tolerance);
    0.5 * residual_resolution * residual_resolution * rows
}

fn priority_preservation_tolerance(
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

fn residual_vector_cost_tolerance(
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

fn priority_cost_within_limit(
    candidate: f64,
    attained: f64,
    residual_rows: usize,
    config: SolverConfig,
) -> bool {
    objective_within_limit(candidate, attained)
        || candidate - attained <= priority_preservation_tolerance(attained, residual_rows, config)
}

fn priority_cost_is_numerically_zero(
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
fn evaluate_priority_trial(
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

enum CurvatureSearch {
    Improved(VariableState, f64),
    NoNegativeCurvature,
    Incomplete,
    Failed,
}

enum MultiScaleCurvature {
    Negative(DVector<f64>),
    NoNegative,
    Inconclusive,
}

#[derive(Clone, Copy)]
enum CurvatureStencilPolicy {
    ConsistentFineScales,
    SingletonAnyResolvedScale,
}

struct CurvatureStencil {
    minimum: f64,
    tolerance: f64,
    minimum_direction: DVector<f64>,
}

#[allow(clippy::too_many_lines)]
fn multi_scale_curvature(
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
fn search_critical_cone_curvature(
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
fn search_one_sided_curvature(
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
fn search_negative_curvature(
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
fn sample_reduced_priority_cost(
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

fn linearized_hard_system(
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

fn linearized_component_objective(
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

fn stack_systems(first: HardSystem, second: HardSystem) -> Option<HardSystem> {
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

fn linearized_composite_category_system(
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

fn controlled_rank_kernel<T>(
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

fn controlled_factorization<T>(
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

fn controlled_dense_factorization<T>(
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

fn controlled_rank_diagnostics(
    matrix: &DMatrix<f64>,
    relative_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<RankDiagnostics> {
    controlled_rank_kernel(matrix.nrows(), matrix.ncols(), control, || {
        rank_diagnostics(matrix, relative_tolerance)
    })
}

fn controlled_numerical_nullspace(
    matrix: &DMatrix<f64>,
    relative_tolerance: f64,
    control: Option<&mut OperationController>,
) -> Option<DMatrix<f64>> {
    controlled_rank_kernel(matrix.nrows(), matrix.ncols(), control, || {
        numerical_nullspace(matrix, relative_tolerance)
    })
}

fn controlled_numerical_nullspace_for_rank(
    matrix: &DMatrix<f64>,
    rank: usize,
    control: Option<&mut OperationController>,
) -> Option<DMatrix<f64>> {
    controlled_rank_kernel(matrix.nrows(), matrix.ncols(), control, || {
        numerical_nullspace_for_rank(matrix, rank)
    })
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
    indexed: Option<ComponentIndexedSystem>,
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
        indexed: None,
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
fn find_conflicting_sources(
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
struct RedundancyDiagnostics {
    rows: Vec<RedundantRowCandidate>,
}

#[allow(clippy::too_many_lines)]
fn find_redundancy(
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

fn controlled_selected_row_rank(
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
fn bounded_lm_step(
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
fn lm_step_with_backend(
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

fn auto_prefers_sparse(system: &ComponentIndexedSystem, free_columns: &[usize]) -> bool {
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
    mut control: Option<&mut OperationController>,
) -> Option<Vec<WorkingBound>> {
    let mut working = vec![WorkingBound::Free; constraints.len()];
    for fixed_only in [true, false] {
        for (index, status) in desired.iter().copied().enumerate() {
            if status == WorkingBound::Free || (status == WorkingBound::Fixed) != fixed_only {
                continue;
            }
            if working_constraint_is_independent(
                constraints,
                &working,
                index,
                relative_tolerance,
                control.as_deref_mut(),
            )? {
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

fn reduced_critical_cone(
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

fn operator_step_is_within_bounds(
    problem: &Problem,
    state: &VariableState,
    layout: &ActiveLayout,
    step: &DVector<f64>,
) -> Option<()> {
    let bounds = normalized_step_bounds(problem, state, layout, step.len())?;
    operator_full_step_satisfies_bounds(step, &bounds).then_some(())
}

fn operator_full_step_satisfies_bounds(
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

fn limit_operator_step(
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

fn solve_rank_aware_least_squares(
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
fn solve_fixed_2x2_rank_aware_least_squares(
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

fn rank_aware_least_squares_is_certified(
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

fn least_squares_stationarity_is_certified(
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

fn empty_rank_diagnostics(rows: usize, columns: usize, relative_tolerance: f64) -> RankDiagnostics {
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

fn rank_thresholds(
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
    use crate::linearization::IndexedJacobianEntry;
    use crate::{
        CONTROLLED_DENSE_KERNEL_MAX_DIMENSION, CancellationToken, OperationLimits,
        OperationStopReason,
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
