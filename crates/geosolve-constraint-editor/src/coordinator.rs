// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained-design lifecycle coordination for presentation adapters.

use std::collections::{BTreeSet, HashSet};

use geosolve_sketch::{
    CancellationToken, ContactBranchEdit, ContactDomain, ContactId, ContactNeighborhood,
    CurveDefinition, CurveId, CurveSpan, DesignPointId, DocumentAngleOrientation,
    DocumentCommandEffect, DocumentConstraintDefinition, DocumentCurveBranchEdit,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentDimensionMode, DocumentDragLocalityPlan, DocumentEdit,
    DocumentElementId, DocumentExternalBindingId, DocumentMeasurementCatalog,
    DocumentMeasurementProvenance, DocumentMeasurementValue, DocumentObjectId, DocumentRuntimeMap,
    DocumentSessionError, DocumentSolveRequest, DocumentSourceId, DocumentSourceOwner,
    ExternalFeatureKindV1, ExternalSnapshotSet, ExternalTopologyDigest, GeometryRole,
    OperationControl, OperationController, OperationLimits, OperationOutcome, OperationReport,
    OperationWork, ParameterBatch, RetainedSketchDocumentSession, RuntimeCurve, ScalarDomain,
    ScalarUnit, SketchAcceptedDocumentRedundancy, SketchAcceptedStateIdentity,
    SketchAttemptFailure, SketchAttemptFailureKind, SketchAttemptIdentity, SketchBound,
    SketchDesignIdentity, SketchDocument, SketchLifecycleRevisionHighWater, SketchSessionError,
    SketchSolveResult, SketchSolveWorkSummary, SketchSource, SolveRejection, TangentOrientation,
};
use thiserror::Error;

use crate::{
    ActionChoice, AuthoringApplication, AuthoringOperand, AuthoringOptions, AuthoringTool,
    ConstraintActionRequest, ConstraintEditor, ConstraintIntent, ConstraintKind,
    ConstraintRelationChoice, ConstructionProposal, ConstructionResult, DimensionActionRequest,
    DimensionKind, EditorEffect, ProjectedDragRequestDisposition, ProvisionalInferenceCandidate,
    ResolvedConstraintKind, SelectionItem,
};

// Ordinary pointer work must return promptly on the single-threaded WASM path.
// The complete M65 mechanism corpus peaks at 155 factorizations, 147 nonlinear
// iterations, 76 rejected trials, 12 component linearizations and 363 locality
// items. These limits retain measured headroom while preventing an invalid
// pointer sample from monopolizing the UI thread.
const PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS: usize = 256;
const PROJECTED_DRAG_MAX_FACTORIZATIONS: usize = 256;
const PROJECTED_DRAG_MAX_REJECTED_TRIALS: usize = 512;
const PROJECTED_DRAG_MAX_COMPONENT_LINEARIZATIONS: usize = 1_024;
const PROJECTED_DRAG_MAX_LOCALITY_ITEMS: usize = 16_384;
const PROJECTED_DRAG_MAX_VALIDATION_ITEMS: usize = 16_384;
const PROJECTED_DRAG_MAX_LOWERING_ITEMS: usize = 16_384;
const PROJECTED_DRAG_MAX_DENSE_KERNEL_WORK_UNITS: usize = 1 << 25;
const PROJECTED_DRAG_MAX_RANK_KERNELS: usize = 256;
const PROJECTED_DRAG_MAX_DIAGNOSTIC_CANDIDATES: usize = 512;
const PROJECTED_DRAG_MAX_DIAGNOSTIC_TRIALS: usize = 1_024;

fn projected_drag_control() -> OperationControl {
    let mut control = OperationControl::unlimited();
    control.limits.document_validation_items = PROJECTED_DRAG_MAX_VALIDATION_ITEMS;
    control.limits.nonlinear_iterations = PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS;
    control.limits.factorizations = PROJECTED_DRAG_MAX_FACTORIZATIONS;
    control.limits.rejected_trials = PROJECTED_DRAG_MAX_REJECTED_TRIALS;
    control.limits.component_linearizations = PROJECTED_DRAG_MAX_COMPONENT_LINEARIZATIONS;
    control.limits.document_dependency_items = PROJECTED_DRAG_MAX_LOCALITY_ITEMS;
    control.limits.document_lowering_items = PROJECTED_DRAG_MAX_LOWERING_ITEMS;
    control.limits.dense_kernel_work_units = PROJECTED_DRAG_MAX_DENSE_KERNEL_WORK_UNITS;
    control.limits.rank_kernels = PROJECTED_DRAG_MAX_RANK_KERNELS;
    control.limits.diagnostic_candidates = PROJECTED_DRAG_MAX_DIAGNOSTIC_CANDIDATES;
    control.limits.diagnostic_trials = PROJECTED_DRAG_MAX_DIAGNOSTIC_TRIALS;
    control
}

const ALTERNATE_BRANCH_MAX_FACTORIZATIONS: usize = 4_096;
const ALTERNATE_BRANCH_MAX_NONLINEAR_ITERATIONS: usize = 4_096;

fn alternate_branch_limits() -> OperationLimits {
    let dense = OperationLimits::unlimited();
    OperationLimits {
        document_validation_items: 250_000,
        document_dependency_items: 250_000,
        document_lowering_items: 250_000,
        nonlinear_iterations: ALTERNATE_BRANCH_MAX_NONLINEAR_ITERATIONS,
        rejected_trials: 16_384,
        component_linearizations: 16_384,
        dense_kernel_rows: dense.dense_kernel_rows,
        dense_kernel_columns: dense.dense_kernel_columns,
        dense_kernel_work_units: 1 << 26,
        factorizations: ALTERNATE_BRANCH_MAX_FACTORIZATIONS,
        rank_kernels: 4_096,
        diagnostic_candidates: 250_000,
        diagnostic_trials: 250_000,
        profile_candidate_pairs: 0,
        profile_subdivisions: 0,
        profile_roots: 0,
        profile_fragments: 0,
        profile_integrations: 0,
        profile_containment_tests: 0,
        profile_faces: 0,
        measurement_integrations: 250_000,
        measurement_derivative_evaluations: 250_000,
    }
}

fn remaining_operation_limits(
    configured: OperationLimits,
    consumed: OperationWork,
) -> OperationLimits {
    OperationLimits {
        document_validation_items: configured
            .document_validation_items
            .saturating_sub(consumed.document_validation_items),
        document_dependency_items: configured
            .document_dependency_items
            .saturating_sub(consumed.document_dependency_items),
        document_lowering_items: configured
            .document_lowering_items
            .saturating_sub(consumed.document_lowering_items),
        nonlinear_iterations: configured
            .nonlinear_iterations
            .saturating_sub(consumed.nonlinear_iterations),
        rejected_trials: configured
            .rejected_trials
            .saturating_sub(consumed.rejected_trials),
        component_linearizations: configured
            .component_linearizations
            .saturating_sub(consumed.component_linearizations),
        // Dense dimensions are maxima, not additive work.
        dense_kernel_rows: configured.dense_kernel_rows,
        dense_kernel_columns: configured.dense_kernel_columns,
        dense_kernel_work_units: configured
            .dense_kernel_work_units
            .saturating_sub(consumed.dense_kernel_work_units),
        factorizations: configured
            .factorizations
            .saturating_sub(consumed.factorizations),
        rank_kernels: configured
            .rank_kernels
            .saturating_sub(consumed.rank_kernels),
        diagnostic_candidates: configured
            .diagnostic_candidates
            .saturating_sub(consumed.diagnostic_candidates),
        diagnostic_trials: configured
            .diagnostic_trials
            .saturating_sub(consumed.diagnostic_trials),
        profile_candidate_pairs: configured
            .profile_candidate_pairs
            .saturating_sub(consumed.profile_candidate_pairs),
        profile_subdivisions: configured
            .profile_subdivisions
            .saturating_sub(consumed.profile_subdivisions),
        profile_roots: configured
            .profile_roots
            .saturating_sub(consumed.profile_roots),
        profile_fragments: configured
            .profile_fragments
            .saturating_sub(consumed.profile_fragments),
        profile_integrations: configured
            .profile_integrations
            .saturating_sub(consumed.profile_integrations),
        profile_containment_tests: configured
            .profile_containment_tests
            .saturating_sub(consumed.profile_containment_tests),
        profile_faces: configured
            .profile_faces
            .saturating_sub(consumed.profile_faces),
        measurement_integrations: configured
            .measurement_integrations
            .saturating_sub(consumed.measurement_integrations),
        measurement_derivative_evaluations: configured
            .measurement_derivative_evaluations
            .saturating_sub(consumed.measurement_derivative_evaluations),
    }
}

fn accumulate_operation_report(aggregate: &mut OperationReport, next: &OperationReport) {
    macro_rules! add {
        ($field:ident) => {
            aggregate.consumed.$field = aggregate
                .consumed
                .$field
                .saturating_add(next.consumed.$field);
        };
    }
    add!(document_validation_items);
    add!(document_dependency_items);
    add!(document_lowering_items);
    add!(nonlinear_iterations);
    add!(rejected_trials);
    add!(component_linearizations);
    aggregate.consumed.dense_kernel_rows = aggregate
        .consumed
        .dense_kernel_rows
        .max(next.consumed.dense_kernel_rows);
    aggregate.consumed.dense_kernel_columns = aggregate
        .consumed
        .dense_kernel_columns
        .max(next.consumed.dense_kernel_columns);
    add!(dense_kernel_work_units);
    add!(factorizations);
    add!(rank_kernels);
    add!(diagnostic_candidates);
    add!(diagnostic_trials);
    add!(profile_candidate_pairs);
    add!(profile_subdivisions);
    add!(profile_roots);
    add!(profile_fragments);
    add!(profile_integrations);
    add!(profile_containment_tests);
    add!(profile_faces);
    add!(measurement_integrations);
    add!(measurement_derivative_evaluations);
    if next.stopping_reason.is_some() {
        aggregate.stopping_reason = next.stopping_reason;
    }
}

fn projected_drag_locality_failure(
    error: &DocumentSessionError,
) -> ProjectedDragLocalityPlanningFailure {
    match error {
        DocumentSessionError::DragLocalityUnavailable => {
            ProjectedDragLocalityPlanningFailure::AcceptedStateUnavailable
        }
        DocumentSessionError::SketchSession(SketchSessionError::DragLocalityEnvelopeExceeded {
            active_tangent_dimensions,
            limit,
        }) => ProjectedDragLocalityPlanningFailure::InteractiveEnvelopeExceeded {
            active_tangent_dimensions: *active_tangent_dimensions,
            limit: *limit,
        },
        DocumentSessionError::SketchSession(
            SketchSessionError::DragLocalityRowEnvelopeExceeded {
                active_hard_rows,
                limit,
            },
        ) => ProjectedDragLocalityPlanningFailure::InteractiveRowEnvelopeExceeded {
            active_hard_rows: *active_hard_rows,
            limit: *limit,
        },
        DocumentSessionError::SketchSession(SketchSessionError::DragLocalityIncomplete {
            required,
            spanned,
        }) => ProjectedDragLocalityPlanningFailure::IncompleteAnchorCover {
            required: *required,
            spanned: *spanned,
        },
        DocumentSessionError::SketchSession(SketchSessionError::DragLocalityUnavailable {
            ..
        })
        | DocumentSessionError::InvalidDragLocalityPlan { .. } => {
            ProjectedDragLocalityPlanningFailure::InvalidAcceptedEvidence
        }
        _ => ProjectedDragLocalityPlanningFailure::Session,
    }
}

/// Opaque, application-persistable restore material for one history position.
#[derive(Clone, Debug)]
pub struct RestoreCheckpoint {
    design_json: String,
    design_is_draft_v5: bool,
    accepted_json: Option<String>,
    accepted_is_draft_v5: bool,
    revisions: SketchLifecycleRevisionHighWater,
}

impl RestoreCheckpoint {
    /// Retained-design JSON in the encoding reported by
    /// [`Self::design_uses_draft_v5`].
    #[must_use]
    pub fn design_json(&self) -> &str {
        &self.design_json
    }

    /// Whether [`Self::design_json`] uses the explicitly unstable draft-v5
    /// sketch encoding rather than frozen canonical v4.
    #[must_use]
    pub const fn design_uses_draft_v5(&self) -> bool {
        self.design_is_draft_v5
    }

    /// Accepted-state JSON in the encoding reported by
    /// [`Self::accepted_uses_draft_v5`], if an accepted state existed.
    #[must_use]
    pub fn accepted_json(&self) -> Option<&str> {
        self.accepted_json.as_deref()
    }

    /// Whether the accepted-state payload uses the explicitly unstable draft-v5
    /// sketch encoding. This is false when there is no accepted payload.
    #[must_use]
    pub const fn accepted_uses_draft_v5(&self) -> bool {
        self.accepted_is_draft_v5
    }

    /// Never-reuse lifecycle revision metadata.
    #[must_use]
    pub const fn revisions(&self) -> SketchLifecycleRevisionHighWater {
        self.revisions
    }
}

/// Stable lifecycle relationship for presentation; no solve report is interpreted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleStatus {
    Accepted,
    DesignUnsolved,
    RejectedAttempt,
    SolvedPreview,
    Solving,
}

/// Persistent identities participating in the current lifecycle view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleDto {
    pub status: LifecycleStatus,
    pub design: SketchDesignIdentity,
    /// The persisted last domain attempt; this is never preview provenance.
    pub attempt: SketchAttemptIdentity,
    /// The independently supplied identity for a transient solved preview.
    ///
    /// This is `None` while solving or when no preview is active, so an outstanding
    /// solve is never assigned a fabricated identity.
    pub preview_attempt: Option<SketchAttemptIdentity>,
    /// The independently accepted state published by the transient preview attempt.
    pub preview_accepted: Option<SketchAcceptedStateIdentity>,
    pub accepted: Option<SketchAcceptedStateIdentity>,
    pub parent_accepted: Option<SketchAcceptedStateIdentity>,
}

/// Verbatim domain problem evidence for exactly one attempted design.
#[derive(Clone, Copy, Debug)]
pub struct ProblemsDto<'a> {
    pub attempt: SketchAttemptIdentity,
    pub design: SketchDesignIdentity,
    pub parent_accepted: Option<SketchAcceptedStateIdentity>,
    pub failure: Option<&'a SketchAttemptFailure>,
    pub rejection: Option<&'a SolveRejection>,
}

/// Typed stage at which one projected drag sample stopped before preview publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectedDragRejectionStage {
    RequestOrdering,
    LocalityPlanning,
    ControlledOperation,
    Session,
    AttemptInput,
    Solve,
    AcceptedState,
    PreviewPublication,
}

/// Typed reason a pointer sample could not establish bounded passive-freedom ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectedDragLocalityPlanningFailure {
    AcceptedStateUnavailable,
    InvalidPointerTarget,
    InteractiveEnvelopeExceeded {
        active_tangent_dimensions: usize,
        limit: usize,
    },
    InteractiveRowEnvelopeExceeded {
        active_hard_rows: usize,
        limit: usize,
    },
    IncompleteAnchorCover {
        required: usize,
        spanned: usize,
    },
    InvalidAcceptedEvidence,
    OperationStopped,
    Session,
}

/// Deterministic work evidence for exactly one projected pointer sample.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedDragWorkEvidence {
    pub pointer_id: u64,
    pub request_id: u64,
    pub point: DesignPointId,
    /// Whether the sample used the last independently accepted preview as its numerical parent.
    pub continued: bool,
    /// Ordinary projected dragging performs exactly one retained solve attempt. A sample
    /// rejected during locality planning performs zero.
    pub attempts: u8,
    pub accepted: bool,
    pub rejection_stage: Option<ProjectedDragRejectionStage>,
    pub operation: OperationReport,
    pub solve: Option<SketchSolveWorkSummary>,
    /// Exact accepted-state-stamped ownership, or the typed reason planning failed closed.
    pub locality: Result<DocumentDragLocalityPlan, ProjectedDragLocalityPlanningFailure>,
}

impl ProjectedDragWorkEvidence {
    #[must_use]
    pub fn locality_plan(&self) -> Option<&DocumentDragLocalityPlan> {
        self.locality.as_ref().ok()
    }

    #[must_use]
    pub fn locality_planning_failure(&self) -> Option<ProjectedDragLocalityPlanningFailure> {
        self.locality.as_ref().err().copied()
    }
}

pub const ALTERNATE_BRANCH_MAX_SEEDS: u8 = 24;

/// Deterministic bounded-search evidence for one explicit assembly-mode request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlternateBranchSearchEvidence {
    pub maximum_seeds: u8,
    pub attempted_seeds: u8,
    pub independently_valid_candidates: u8,
    pub representable_modes: u8,
    /// Aggregate deterministic work across all attempted seeds.
    pub operation: OperationReport,
}

impl Default for AlternateBranchSearchEvidence {
    fn default() -> Self {
        let limits = alternate_branch_limits();
        Self {
            maximum_seeds: 0,
            attempted_seeds: 0,
            independently_valid_candidates: 0,
            representable_modes: 0,
            operation: OperationReport {
                configured: limits,
                consumed: OperationWork::default(),
                stopping_reason: None,
            },
        }
    }
}

/// Closed result vocabulary for a bounded alternate-branch search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlternateBranchSearchStatus {
    Proposed,
    NoAlternative,
    Ambiguous,
    Unrepresentable,
    Exhausted,
}

/// Exact-stamped non-authoritative assembly-mode proposal.
#[derive(Clone, Debug, PartialEq)]
pub struct AlternateBranchProposal {
    pub proposal_id: u64,
    pub design: SketchDesignIdentity,
    pub accepted: SketchAcceptedStateIdentity,
    pub point: DesignPointId,
    pub position: [f64; 2],
    pub branches: Vec<DocumentCurveBranchEdit>,
    pub evidence: AlternateBranchSearchEvidence,
}

/// Result of requesting a bounded alternate assembly mode.
#[derive(Clone, Debug, PartialEq)]
pub struct AlternateBranchSearchResult {
    pub status: AlternateBranchSearchStatus,
    pub proposal: Option<AlternateBranchProposal>,
    pub evidence: AlternateBranchSearchEvidence,
}

/// Stable high-level classification for one current editor problem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorProblemCategory {
    Input,
    Lowering,
    Solver,
    Validation,
    Geometry,
    Constraint,
    Dimension,
    Bound,
    Publication,
}

/// Whether a current problem has defensible persistent presentation targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorProblemScope {
    Global,
    Targeted,
}

/// Persistent canvas-addressable identity associated with one current problem.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EditorProblemTarget {
    Point(DesignPointId),
    Curve(CurveId),
    Constraint(geosolve_sketch::DocumentConstraintId),
    Dimension(DocumentDimensionId),
}

/// Presentation-neutral metadata for the latest failed or rejected retained-design attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorProblemMetadata {
    pub attempt: SketchAttemptIdentity,
    pub design: SketchDesignIdentity,
    pub category: EditorProblemCategory,
    pub scope: EditorProblemScope,
    pub message: String,
    pub targets: Vec<EditorProblemTarget>,
}

/// Provenance of an audit evidence reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditProvenance {
    Accepted(SketchAcceptedStateIdentity),
    Attempt(SketchAttemptIdentity),
}

/// Opaque domain audit evidence. Consumers may render it but need not reconstruct mappings.
#[derive(Clone, Copy, Debug)]
pub struct AuditDto<'a> {
    pub provenance: AuditProvenance,
    pub design: SketchDesignIdentity,
    pub solve_result: &'a SketchSolveResult,
    pub mappings: &'a DocumentRuntimeMap,
}

/// Deterministic reason why an editor action cannot currently be emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisabledReason {
    EmptySelection,
    WrongArity,
    WrongOperandKind,
    MissingObject,
    InvalidSpan,
    AlreadyInRequestedState,
    NothingToUndo,
    NothingToRedo,
}

/// An action is either constructible now or has one stable disabled reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionState {
    Enabled,
    Disabled(DisabledReason),
}

/// Actions whose availability is owned by the retained coordinator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinatorActionKind {
    Constraint(ConstraintIntent),
    Dimension(DimensionKind, DocumentDimensionMode),
    SetDimensionMode(DocumentDimensionMode),
    EditContactBranch,
    SetAngleOrientation(DocumentAngleOrientation),
    Delete,
    Suppress,
    Unsuppress,
    Undo,
    Redo,
    Reattempt,
}

/// One action and its deterministic availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionAvailability {
    pub action: CoordinatorActionKind,
    pub state: ActionState,
}

/// Complete current branch state and legal same-curve choices for one contact.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactBranchAction {
    pub current: ContactBranchEdit,
    pub spans: Vec<CurveSpan>,
    pub domains: Vec<ContactDomain>,
    pub neighborhoods: Vec<ContactNeighborhood>,
    pub tangent_orientations: Vec<Option<TangentOrientation>>,
}

/// Selection-scoped explicit branch controls returned to presentation adapters.
#[derive(Clone, Debug, PartialEq)]
pub enum BranchAction {
    Contact(ContactBranchAction),
    AngleOrientation {
        dimension: DocumentDimensionId,
        current: DocumentAngleOrientation,
    },
}

/// Identity-only outcome from a retained mutation.
#[derive(Clone, Debug)]
pub struct MutationOutcome<T> {
    pub value: T,
    pub design: SketchDesignIdentity,
    pub attempt: SketchAttemptIdentity,
    pub published_accepted: Option<SketchAcceptedStateIdentity>,
}

/// Typed result of applying a mutating [`EditorEffect`].
#[derive(Clone, Debug)]
pub enum EditorMutation {
    PointMove(DocumentCommandEffect),
    Construction(ConstructionResult),
    Inference(DocumentCommandEffect),
}

/// Typed retained mutation emitted by one complete headless authoring application.
#[derive(Clone, Debug)]
pub enum AuthoringMutation {
    Constraint(MutationOutcome<geosolve_sketch::DocumentConstraintId>),
    Dimension(MutationOutcome<DocumentDimensionId>),
}

/// Editable target metadata for one selected dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimensionTargetMetadata {
    pub dimension: DocumentDimensionId,
    pub scalar: geosolve_sketch::DesignScalarId,
    /// Exact persisted solver-domain value. Angles are radians and retain their
    /// explicit directed branch.
    pub value: f64,
    pub unit: ScalarUnit,
    /// Presentation value owned by this headless adapter. Oriented line angles
    /// are exposed as the acute supporting-line angle in degrees.
    pub display_value: f64,
    pub display_unit: DimensionTargetDisplayUnit,
    pub mode: DocumentDimensionMode,
}

/// Presentation unit for an editable dimension target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionTargetDisplayUnit {
    ModelUnits,
    AcuteDegrees,
}

/// Presentation-neutral conversion of one solver-domain target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayDimensionTarget {
    pub value: f64,
    pub unit: DimensionTargetDisplayUnit,
}

/// Converts one finite solver-domain dimension value for presentation.
///
/// Angle storage remains explicit directed radians. Presentation uses the acute
/// angle between the two supporting lines, which is independent of invisible
/// endpoint direction and of which intersection ray is chosen.
#[must_use]
pub fn display_dimension_target(value: f64, unit: ScalarUnit) -> Option<DisplayDimensionTarget> {
    if !value.is_finite() {
        return None;
    }
    let display = match unit {
        ScalarUnit::Angle => {
            let line_angle = value.rem_euclid(std::f64::consts::PI);
            DisplayDimensionTarget {
                value: line_angle
                    .min(std::f64::consts::PI - line_angle)
                    .to_degrees(),
                unit: DimensionTargetDisplayUnit::AcuteDegrees,
            }
        }
        ScalarUnit::Length | ScalarUnit::Parameter => DisplayDimensionTarget {
            value,
            unit: DimensionTargetDisplayUnit::ModelUnits,
        },
    };
    display.value.is_finite().then_some(display)
}

/// Measurement publication preserves the exact M38 value and audit provenance.
#[derive(Clone, Debug)]
pub enum MeasurementPublication {
    Published(DocumentMeasurementValue),
    Withheld {
        source: DocumentSourceId,
        reason: String,
    },
}

/// Closed replay vocabulary used by deterministic generated/model qualification.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplayAction {
    Edit {
        expected: SketchDesignIdentity,
        edit: DocumentEdit,
    },
    Construction {
        expected: SketchDesignIdentity,
        proposal: ConstructionProposal,
    },
    ConstraintAction {
        expected: SketchDesignIdentity,
        selection: Vec<SelectionItem>,
        request: ConstraintActionRequest,
    },
    DimensionAction {
        expected: SketchDesignIdentity,
        selection: Vec<SelectionItem>,
        request: DimensionActionRequest,
    },
    PointDistance {
        expected: SketchDesignIdentity,
        points: [DesignPointId; 2],
        mode: DocumentDimensionMode,
        label: String,
    },
    SegmentLength {
        expected: SketchDesignIdentity,
        curve: CurveSpan,
        mode: DocumentDimensionMode,
        label: String,
    },
    SetDimensionMode {
        expected: SketchDesignIdentity,
        dimension: DocumentDimensionId,
        mode: DocumentDimensionMode,
    },
    SetContactBranches {
        expected: SketchDesignIdentity,
        selection: Vec<SelectionItem>,
        edits: Vec<ContactBranchEdit>,
    },
    SetAngleOrientation {
        expected: SketchDesignIdentity,
        dimension: DocumentDimensionId,
        orientation: DocumentAngleOrientation,
    },
    RebindExternalBinding {
        expected: SketchDesignIdentity,
        binding: DocumentExternalBindingId,
        expected_kind: ExternalFeatureKindV1,
        expected_topology: Option<ExternalTopologyDigest>,
    },
    Delete {
        expected: SketchDesignIdentity,
        selection: Vec<SelectionItem>,
    },
    SetSuppressed {
        expected: SketchDesignIdentity,
        selection: Vec<SelectionItem>,
        suppressed: bool,
    },
    Reattempt {
        expected: SketchDesignIdentity,
    },
    AlternateBranch {
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        branches: Vec<DocumentCurveBranchEdit>,
    },
    Undo,
    Redo,
}

/// Coordinator setup, history restore, or domain mutation failure.
#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error(transparent)]
    Session(#[from] DocumentSessionError),
    #[error(transparent)]
    Document(#[from] geosolve_sketch::DocumentError),
    #[error(transparent)]
    Editor(#[from] crate::EditorError),
    #[error("selected operands cannot construct the requested dimension")]
    IncompatibleDimension,
    #[error("invalid typed action input: {0}")]
    InvalidActionInput(&'static str),
    #[error("action is unavailable: {0:?}")]
    ActionUnavailable(DisabledReason),
    #[error("preview session belongs to a different document")]
    PreviewForeignDocument,
    #[error("preview design identity does not match the current design")]
    PreviewStaleDesign,
    #[error("preview attempt identity must differ from the persisted last attempt")]
    PreviewAttemptMatchesPersisted,
    #[error("preview last attempt did not publish an accepted state")]
    PreviewNotAccepted,
    #[error("preview attempt and accepted state have inconsistent provenance")]
    PreviewAcceptedStateMismatch,
    #[error("point-move commit has no retained solved preview")]
    MissingSolvedPreview,
    #[error("point-move commit has no matching gesture-locality plan")]
    MissingDragLocalityPlan,
    #[error("point-move commit does not match the retained solved preview")]
    SolvedPreviewMismatch,
    #[error("there is no current alternate-branch proposal")]
    MissingAlternateBranchProposal,
    #[error("alternate-branch proposal is stale")]
    StaleAlternateBranchProposal,
    #[error("{context} stopped before publication: {report:?}")]
    OperationStopped {
        context: &'static str,
        report: Box<OperationReport>,
    },
    #[error("history has no earlier checkpoint")]
    NothingToUndo,
    #[error("history has no later checkpoint")]
    NothingToRedo,
}

/// Owner of retained lifecycle, interaction selection, restore history, and transcript.
#[derive(Debug)]
pub struct RetainedEditorCoordinator {
    session: RetainedSketchDocumentSession,
    editor: ConstraintEditor,
    history: Vec<RestoreCheckpoint>,
    history_cursor: usize,
    transcript: Vec<ReplayAction>,
    transient: Option<TransientLifecycle>,
    solved_preview: Option<RetainedSketchDocumentSession>,
    solved_preview_locality: Option<DocumentDragLocalityPlan>,
    drag_continuation: Option<ProjectedDragContinuation>,
    projected_drag_work: Option<ProjectedDragWorkEvidence>,
    alternate_branch: Option<AlternateBranchCandidate>,
    next_alternate_branch_proposal_id: u64,
}

#[derive(Clone, Debug)]
struct AlternateBranchCandidate {
    proposal: AlternateBranchProposal,
    preview: RetainedSketchDocumentSession,
}

#[derive(Clone, Copy, Debug)]
struct AlternateBranchSearchBase {
    design: SketchDesignIdentity,
    accepted: SketchAcceptedStateIdentity,
    point: DesignPointId,
    position: [f64; 2],
    scale: f64,
    equality_degrees_of_freedom: usize,
}

type AlternateBranchSelection = (Vec<CurveSpan>, [f64; 2], AlternateBranchCandidate);

enum AlternateBranchPreviewOutcome {
    Accepted {
        preview: Box<RetainedSketchDocumentSession>,
        position: [f64; 2],
    },
    Rejected,
    Exhausted,
}

enum AlternateBranchSeedOutcome {
    Continue,
    Selected(Box<AlternateBranchSelection>),
    Ambiguous,
    Exhausted,
}

struct AlternateBranchCanonicalSeed {
    signature: Vec<CurveSpan>,
    position: [f64; 2],
    branches: Vec<DocumentCurveBranchEdit>,
}

#[derive(Clone, Debug)]
struct ProjectedDragContinuation {
    gesture_epoch: Option<u64>,
    pointer_id: u64,
    point: DesignPointId,
    design: SketchDesignIdentity,
    last_request_id: u64,
    locality: DocumentDragLocalityPlan,
}

struct ProjectedDragSample {
    request: DocumentSolveRequest,
    continuation: Option<ProjectedDragContinuation>,
    continued: bool,
}

struct ProjectedDragAttemptResult {
    operation: OperationReport,
    solve: Option<SketchSolveWorkSummary>,
    attempts: u8,
    retain_continuation: bool,
    accepted_position: Option<[f64; 2]>,
    rejection_stage: Option<ProjectedDragRejectionStage>,
}

#[derive(Clone, Copy, Debug)]
enum TransientLifecycle {
    Solving,
    SolvedPreview {
        attempt: SketchAttemptIdentity,
        accepted: SketchAcceptedStateIdentity,
    },
}

impl RetainedEditorCoordinator {
    /// Starts editor history at the supplied retained lifecycle.
    ///
    /// # Errors
    ///
    /// Returns a document serialization error if the initial checkpoint cannot be made.
    pub fn new(session: RetainedSketchDocumentSession) -> Result<Self, CoordinatorError> {
        let checkpoint = checkpoint(&session)?;
        Ok(Self {
            session,
            editor: ConstraintEditor::default(),
            history: vec![checkpoint],
            history_cursor: 0,
            transcript: Vec::new(),
            transient: None,
            solved_preview: None,
            solved_preview_locality: None,
            drag_continuation: None,
            projected_drag_work: None,
            alternate_branch: None,
            next_alternate_branch_proposal_id: 1,
        })
    }

    #[must_use]
    pub const fn session(&self) -> &RetainedSketchDocumentSession {
        &self.session
    }

    #[must_use]
    pub const fn editor(&self) -> &ConstraintEditor {
        &self.editor
    }

    #[must_use]
    pub fn editor_mut(&mut self) -> &mut ConstraintEditor {
        &mut self.editor
    }

    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    #[must_use]
    pub const fn history_cursor(&self) -> usize {
        self.history_cursor
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history_cursor > 0
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history_cursor + 1 < self.history.len()
    }

    #[must_use]
    pub fn checkpoint(&self) -> &RestoreCheckpoint {
        &self.history[self.history_cursor]
    }

    #[must_use]
    pub fn transcript(&self) -> &[ReplayAction] {
        &self.transcript
    }

    /// The independently validated solved-preview session currently published for rendering.
    #[must_use]
    pub fn solved_preview_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.solved_preview.as_ref()
    }

    /// Work evidence for the latest projected pointer sample, if one is active.
    #[must_use]
    pub const fn projected_drag_work_evidence(&self) -> Option<&ProjectedDragWorkEvidence> {
        self.projected_drag_work.as_ref()
    }

    /// Returns the current exact-stamped alternate assembly-mode proposal.
    #[must_use]
    pub fn alternate_branch_proposal(&self) -> Option<&AlternateBranchProposal> {
        self.alternate_branch
            .as_ref()
            .map(|candidate| &candidate.proposal)
    }

    /// Returns the non-authoritative independently accepted ghost preview.
    #[must_use]
    pub fn alternate_branch_preview_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.alternate_branch
            .as_ref()
            .map(|candidate| &candidate.preview)
    }

    /// Returns the highest-priority visible preview for a presentation adapter.
    #[must_use]
    pub fn visible_preview_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.alternate_branch_preview_session()
            .or_else(|| self.solved_preview_session())
    }

    /// Performs a deterministic bounded search for a representable alternate line branch.
    ///
    /// At most eight canonical directions at three component-relative radii are tried.
    /// Every candidate is solved and independently validated on a cloned lifecycle. Nothing
    /// becomes authoritative until [`Self::accept_alternate_branch`] is called.
    #[must_use]
    pub fn propose_alternate_branch(
        &mut self,
        point: DesignPointId,
    ) -> AlternateBranchSearchResult {
        self.propose_alternate_branch_with_limits(point, alternate_branch_limits())
    }

    fn propose_alternate_branch_with_limits(
        &mut self,
        point: DesignPointId,
        limits: OperationLimits,
    ) -> AlternateBranchSearchResult {
        self.alternate_branch = None;
        let mut evidence = AlternateBranchSearchEvidence {
            maximum_seeds: ALTERNATE_BRANCH_MAX_SEEDS,
            operation: OperationReport {
                configured: limits,
                consumed: OperationWork::default(),
                stopping_reason: None,
            },
            ..AlternateBranchSearchEvidence::default()
        };
        let Some(base) = self.alternate_branch_search_base(point) else {
            return alternate_branch_search_result(
                AlternateBranchSearchStatus::Unrepresentable,
                &evidence,
                None,
            );
        };
        let directions = canonical_branch_search_directions();
        let radii = [0.5, 1.0, 2.0];
        let mut selected: Option<AlternateBranchSelection> = None;
        let mut ambiguous = false;
        let mut exhausted = false;
        'search: for radius in radii {
            for direction in directions {
                evidence.attempted_seeds = evidence.attempted_seeds.saturating_add(1);
                let seed = [
                    base.position[0] + direction[0] * radius * base.scale,
                    base.position[1] + direction[1] * radius * base.scale,
                ];
                match self.evaluate_alternate_branch_seed(
                    base,
                    seed,
                    limits,
                    &mut evidence,
                    selected.as_ref(),
                ) {
                    AlternateBranchSeedOutcome::Continue => {}
                    AlternateBranchSeedOutcome::Selected(candidate) => {
                        selected = Some(*candidate);
                    }
                    AlternateBranchSeedOutcome::Ambiguous => {
                        ambiguous = true;
                        evidence.representable_modes = 2;
                        break 'search;
                    }
                    AlternateBranchSeedOutcome::Exhausted => {
                        exhausted = true;
                        break 'search;
                    }
                }
            }
        }
        if exhausted {
            return alternate_branch_search_result(
                AlternateBranchSearchStatus::Exhausted,
                &evidence,
                None,
            );
        }
        if ambiguous {
            return alternate_branch_search_result(
                AlternateBranchSearchStatus::Ambiguous,
                &evidence,
                None,
            );
        }
        let Some((_, _, mut candidate)) = selected else {
            return alternate_branch_search_result(
                AlternateBranchSearchStatus::NoAlternative,
                &evidence,
                None,
            );
        };
        candidate.proposal.evidence = evidence;
        self.next_alternate_branch_proposal_id =
            self.next_alternate_branch_proposal_id.saturating_add(1);
        let proposal = candidate.proposal.clone();
        self.alternate_branch = Some(candidate);
        alternate_branch_search_result(
            AlternateBranchSearchStatus::Proposed,
            &evidence,
            Some(proposal),
        )
    }

    fn alternate_branch_search_base(
        &self,
        point: DesignPointId,
    ) -> Option<AlternateBranchSearchBase> {
        let accepted = self.session.accepted_state()?;
        let document = accepted.document();
        Some(AlternateBranchSearchBase {
            design: self.session.design_identity(),
            accepted: accepted.identity(),
            point,
            position: document.point(point)?.position,
            scale: incident_line_scale(document, point)?,
            equality_degrees_of_freedom: self
                .session
                .accepted_diagnostics()?
                .mobility?
                .equality_degrees_of_freedom?,
        })
    }

    fn evaluate_alternate_branch_seed(
        &self,
        base: AlternateBranchSearchBase,
        seed: [f64; 2],
        limits: OperationLimits,
        evidence: &mut AlternateBranchSearchEvidence,
        selected: Option<&AlternateBranchSelection>,
    ) -> AlternateBranchSeedOutcome {
        let accepted_document = self
            .session
            .accepted_state()
            .expect("alternate-branch base proves an accepted state")
            .document();
        let Some((branches, signature)) = incident_branch_edits(
            self.session.design_document(),
            accepted_document,
            base.point,
            seed,
        ) else {
            return AlternateBranchSeedOutcome::Continue;
        };
        let (preview, position) = match self
            .attempt_alternate_branch_preview(base, seed, &branches, None, limits, evidence)
        {
            AlternateBranchPreviewOutcome::Accepted { preview, position } => (preview, position),
            AlternateBranchPreviewOutcome::Rejected => {
                return AlternateBranchSeedOutcome::Continue;
            }
            AlternateBranchPreviewOutcome::Exhausted => {
                return AlternateBranchSeedOutcome::Exhausted;
            }
        };
        evidence.independently_valid_candidates =
            evidence.independently_valid_candidates.saturating_add(1);
        if (position[0] - base.position[0]).hypot(position[1] - base.position[1])
            <= 1.0e-7 * base.scale
        {
            return AlternateBranchSeedOutcome::Continue;
        }
        if selected.is_some_and(|(selected_signature, selected_position, _)| {
            *selected_signature != signature
                && (selected_position[0] - position[0]).hypot(selected_position[1] - position[1])
                    > 1.0e-7 * base.scale
        }) {
            return AlternateBranchSeedOutcome::Ambiguous;
        }
        if selected.is_some() {
            return AlternateBranchSeedOutcome::Continue;
        }
        let Some(branches) = branches
            .iter()
            .map(|branch| {
                preview
                    .design_document()
                    .curve_branch_direction(branch.curve)
                    .map(|direction| DocumentCurveBranchEdit {
                        curve: branch.curve,
                        direction,
                    })
            })
            .collect::<Option<Vec<_>>>()
        else {
            return AlternateBranchSeedOutcome::Continue;
        };
        self.canonical_alternate_branch_candidate(
            base,
            AlternateBranchCanonicalSeed {
                signature,
                position,
                branches,
            },
            &preview,
            limits,
            evidence,
        )
    }

    fn attempt_alternate_branch_preview(
        &self,
        base: AlternateBranchSearchBase,
        target: [f64; 2],
        branches: &[DocumentCurveBranchEdit],
        seed_preview: Option<&RetainedSketchDocumentSession>,
        limits: OperationLimits,
        evidence: &mut AlternateBranchSearchEvidence,
    ) -> AlternateBranchPreviewOutcome {
        let mut preview = self.session.clone();
        let remaining = remaining_operation_limits(limits, evidence.operation.consumed);
        let control = OperationControl::new(CancellationToken::default(), remaining);
        let controlled = if let Some(seed_preview) = seed_preview {
            preview.attempt_point_and_curve_branches_with_preview_seed_controlled(
                preview.design_identity(),
                base.point,
                target,
                branches,
                seed_preview,
                control,
            )
        } else {
            preview.attempt_point_and_curve_branches_controlled(
                preview.design_identity(),
                base.point,
                target,
                branches,
                control,
            )
        };
        let Ok(controlled) = controlled else {
            return AlternateBranchPreviewOutcome::Rejected;
        };
        accumulate_operation_report(&mut evidence.operation, controlled.report());
        let OperationOutcome::Completed { value: outcome, .. } = controlled else {
            return AlternateBranchPreviewOutcome::Exhausted;
        };
        if outcome.published_accepted_identity().is_none() {
            return AlternateBranchPreviewOutcome::Rejected;
        }
        let Some(accepted) = preview.accepted_state() else {
            return AlternateBranchPreviewOutcome::Rejected;
        };
        let solve = accepted.solve_result();
        let candidate_dof = preview
            .accepted_diagnostics()
            .and_then(|diagnostics| diagnostics.mobility)
            .and_then(|mobility| mobility.equality_degrees_of_freedom);
        if solve.rejection.is_some()
            || solve
                .acceptance_hard_residual_max
                .is_none_or(|residual| residual > 1.0e-9)
            || !same_known_equality_degrees_of_freedom(
                Some(base.equality_degrees_of_freedom),
                candidate_dof,
            )
        {
            return AlternateBranchPreviewOutcome::Rejected;
        }
        let Some(position) = accepted
            .document()
            .point(base.point)
            .map(|value| value.position)
        else {
            return AlternateBranchPreviewOutcome::Rejected;
        };
        if !position.iter().all(|value| value.is_finite()) {
            return AlternateBranchPreviewOutcome::Rejected;
        }
        AlternateBranchPreviewOutcome::Accepted {
            preview: Box::new(preview),
            position,
        }
    }

    fn canonical_alternate_branch_candidate(
        &self,
        base: AlternateBranchSearchBase,
        seed: AlternateBranchCanonicalSeed,
        initial_preview: &RetainedSketchDocumentSession,
        limits: OperationLimits,
        evidence: &mut AlternateBranchSearchEvidence,
    ) -> AlternateBranchSeedOutcome {
        let initial_accepted_document = initial_preview
            .accepted_state()
            .expect("accepted alternate-branch preview")
            .document();
        // Replay the exact accepted point once, retaining base-owned candidate
        // topology while importing only compatible numerical coordinates from
        // the independently accepted seed preview. The exact accepted geometry
        // must reproduce byte-for-byte before it can become a proposal.
        let (canonical_preview, canonical_position) = match self.attempt_alternate_branch_preview(
            base,
            seed.position,
            &seed.branches,
            Some(initial_preview),
            limits,
            evidence,
        ) {
            AlternateBranchPreviewOutcome::Accepted { preview, position } => {
                let Some(accepted) = preview.accepted_state() else {
                    return AlternateBranchSeedOutcome::Continue;
                };
                if position.map(f64::to_bits) != seed.position.map(f64::to_bits)
                    || !checkpoint_documents_have_exact_bytes(
                        accepted.document(),
                        initial_accepted_document,
                    )
                    .unwrap_or(false)
                {
                    return AlternateBranchSeedOutcome::Continue;
                }
                (*preview, position)
            }
            AlternateBranchPreviewOutcome::Rejected => {
                return AlternateBranchSeedOutcome::Continue;
            }
            AlternateBranchPreviewOutcome::Exhausted => {
                return AlternateBranchSeedOutcome::Exhausted;
            }
        };
        evidence.representable_modes = 1;
        let proposal = AlternateBranchProposal {
            proposal_id: self.next_alternate_branch_proposal_id,
            design: base.design,
            accepted: base.accepted,
            point: base.point,
            position: canonical_position,
            branches: seed.branches,
            evidence: *evidence,
        };
        AlternateBranchSeedOutcome::Selected(Box::new((
            seed.signature,
            canonical_position,
            AlternateBranchCandidate {
                proposal,
                preview: canonical_preview,
            },
        )))
    }

    /// Cancels the current ghost proposal without mutating retained state.
    pub fn cancel_alternate_branch(&mut self) {
        self.alternate_branch = None;
    }

    /// Accepts the current exact-stamped proposal as one atomic point-and-branch edit.
    ///
    /// # Errors
    ///
    /// Rejects a missing or stale proposal and preserves the authoritative session.
    pub fn accept_alternate_branch(
        &mut self,
        proposal_id: u64,
    ) -> Result<MutationOutcome<()>, CoordinatorError> {
        self.accept_alternate_branch_with_control(
            proposal_id,
            OperationControl::new(CancellationToken::default(), alternate_branch_limits()),
        )
    }

    fn accept_alternate_branch_with_control(
        &mut self,
        proposal_id: u64,
        control: OperationControl,
    ) -> Result<MutationOutcome<()>, CoordinatorError> {
        let candidate = self
            .alternate_branch
            .clone()
            .ok_or(CoordinatorError::MissingAlternateBranchProposal)?;
        let proposal = &candidate.proposal;
        if proposal.proposal_id != proposal_id
            || proposal.design != self.session.design_identity()
            || self
                .session
                .accepted_state()
                .map(geosolve_sketch::SketchAcceptedDocumentState::identity)
                != Some(proposal.accepted)
        {
            return Err(CoordinatorError::StaleAlternateBranchProposal);
        }
        let mut candidate_session = self.session.clone();
        let controlled = candidate_session.apply_point_and_curve_branches_from_preview_controlled(
            proposal.design,
            proposal.point,
            proposal.position,
            &proposal.branches,
            &candidate.preview,
            control,
        )?;
        let retained = match controlled {
            OperationOutcome::Completed { value, .. } => value,
            OperationOutcome::Cancelled { report } | OperationOutcome::WorkExhausted { report } => {
                return Err(CoordinatorError::OperationStopped {
                    context: "alternate-branch acceptance",
                    report: Box::new(report),
                });
            }
            outcome => {
                return Err(CoordinatorError::OperationStopped {
                    context: "alternate-branch acceptance",
                    report: Box::new(*outcome.report()),
                });
            }
        };
        let next_checkpoint = checkpoint(&candidate_session)?;
        let outcome = MutationOutcome {
            value: (),
            design: retained.design_identity(),
            attempt: retained.attempt_identity(),
            published_accepted: retained.published_accepted_identity(),
        };
        let replay = ReplayAction::AlternateBranch {
            expected: proposal.design,
            point: proposal.point,
            position: proposal.position,
            branches: proposal.branches.clone(),
        };
        self.session = candidate_session;
        self.record_prepared_mutation(next_checkpoint, replay);
        Ok(outcome)
    }

    /// Executes and publishes one editor-requested projected point-move preview.
    ///
    /// A failed or rejected projection is reported back to the editor without replacing the
    /// last valid solved preview. Request construction and acceptance validation remain here,
    /// outside presentation adapters.
    pub fn resolve_projected_point_move(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        model_position: [f64; 2],
    ) -> Vec<EditorEffect> {
        let gesture_epoch = match self
            .editor
            .projected_drag_request_disposition(pointer_id, request_id, point)
        {
            ProjectedDragRequestDisposition::Current { gesture_epoch } => Some(gesture_epoch),
            ProjectedDragRequestDisposition::Untracked => None,
            ProjectedDragRequestDisposition::Stale => return Vec::new(),
        };
        let sample = match self.prepare_projected_drag_sample(
            gesture_epoch,
            pointer_id,
            request_id,
            point,
            model_position,
        ) {
            Ok(sample) => sample,
            Err(effects) => return effects,
        };
        let (locality, planning_report) = match self.plan_projected_drag_locality(
            pointer_id,
            request_id,
            point,
            sample.continuation,
        ) {
            Ok(planning) => planning,
            Err(effects) => return effects,
        };
        let result = self.attempt_projected_drag(
            point,
            sample.request,
            &locality,
            sample.continued,
            &planning_report,
        );
        if result.retain_continuation {
            self.drag_continuation = Some(ProjectedDragContinuation {
                gesture_epoch,
                pointer_id,
                point,
                design: self.session.design_identity(),
                last_request_id: request_id,
                locality: locality.clone(),
            });
        } else {
            self.solved_preview = None;
            self.solved_preview_locality = None;
            self.drag_continuation = None;
        }
        self.projected_drag_work = Some(ProjectedDragWorkEvidence {
            pointer_id,
            request_id,
            point,
            continued: sample.continued,
            attempts: result.attempts,
            accepted: result.accepted_position.is_some(),
            rejection_stage: result.rejection_stage,
            operation: result.operation,
            solve: result.solve,
            locality: Ok(locality),
        });
        self.editor
            .projected_drag_result(pointer_id, request_id, point, result.accepted_position)
    }

    fn prepare_projected_drag_sample(
        &mut self,
        gesture_epoch: Option<u64>,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        model_position: [f64; 2],
    ) -> Result<ProjectedDragSample, Vec<EditorEffect>> {
        let same_gesture = self.drag_continuation.as_ref().is_some_and(|continuation| {
            continuation.gesture_epoch == gesture_epoch
                && continuation.pointer_id == pointer_id
                && continuation.point == point
                && continuation.design == self.session.design_identity()
        });
        if !same_gesture {
            self.transient = None;
            self.solved_preview = None;
            self.solved_preview_locality = None;
            self.drag_continuation = None;
            self.alternate_branch = None;
        }
        let continuation = self.drag_continuation.clone();
        if let Some(continuation) = continuation.as_ref()
            && request_id <= continuation.last_request_id
        {
            self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                pointer_id,
                request_id,
                point,
                continued: self.solved_preview.is_some(),
                attempts: 0,
                accepted: false,
                rejection_stage: Some(ProjectedDragRejectionStage::RequestOrdering),
                operation: OperationController::new(projected_drag_control()).report(),
                solve: None,
                locality: Ok(continuation.locality.clone()),
            });
            return Err(self
                .editor
                .projected_drag_result(pointer_id, request_id, point, None));
        }
        if !model_position.iter().all(|value| value.is_finite()) {
            let locality = continuation.as_ref().map_or(
                Err(ProjectedDragLocalityPlanningFailure::InvalidPointerTarget),
                |continuation| Ok(continuation.locality.clone()),
            );
            self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                pointer_id,
                request_id,
                point,
                continued: continuation.is_some() && self.solved_preview.is_some(),
                attempts: 0,
                accepted: false,
                rejection_stage: Some(ProjectedDragRejectionStage::AttemptInput),
                operation: OperationController::new(projected_drag_control()).report(),
                solve: None,
                locality,
            });
            return Err(self
                .editor
                .projected_drag_result(pointer_id, request_id, point, None));
        }
        let request = self
            .session
            .last_attempt()
            .input()
            .candidate_request()
            .with_previous_state_preferences()
            .with_drag(point, model_position);
        let matching_continuation = continuation.filter(|continuation| {
            continuation.gesture_epoch == gesture_epoch
                && continuation.pointer_id == pointer_id
                && continuation.point == point
                && continuation.design == self.session.design_identity()
                && request_id > continuation.last_request_id
        });
        let continued = matching_continuation.is_some() && self.solved_preview.is_some();
        Ok(ProjectedDragSample {
            request,
            continuation: matching_continuation,
            continued,
        })
    }

    fn plan_projected_drag_locality(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        continuation: Option<ProjectedDragContinuation>,
    ) -> Result<(DocumentDragLocalityPlan, OperationReport), Vec<EditorEffect>> {
        let mut planning_controller = OperationController::new(projected_drag_control());
        let locality = if let Some(continuation) = continuation {
            continuation.locality
        } else {
            match self
                .session
                .preflight_design_with_controller(&mut planning_controller)
            {
                Ok(true) => {}
                Ok(false) => {
                    let operation = planning_controller.report();
                    self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                        pointer_id,
                        request_id,
                        point,
                        continued: false,
                        attempts: 0,
                        accepted: false,
                        rejection_stage: Some(ProjectedDragRejectionStage::LocalityPlanning),
                        operation,
                        solve: None,
                        locality: Err(ProjectedDragLocalityPlanningFailure::OperationStopped),
                    });
                    return Err(self
                        .editor
                        .projected_drag_result(pointer_id, request_id, point, None));
                }
                Err(error) => {
                    let failure = projected_drag_locality_failure(&error);
                    let operation = planning_controller.report();
                    self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                        pointer_id,
                        request_id,
                        point,
                        continued: false,
                        attempts: 0,
                        accepted: false,
                        rejection_stage: Some(ProjectedDragRejectionStage::LocalityPlanning),
                        operation,
                        solve: None,
                        locality: Err(failure),
                    });
                    return Err(self
                        .editor
                        .projected_drag_result(pointer_id, request_id, point, None));
                }
            }
            match self
                .session
                .drag_locality_plan_with_controller(point, &mut planning_controller)
            {
                Ok(Some(locality)) => locality,
                Ok(None) => {
                    let operation = planning_controller.report();
                    self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                        pointer_id,
                        request_id,
                        point,
                        continued: false,
                        attempts: 0,
                        accepted: false,
                        rejection_stage: Some(ProjectedDragRejectionStage::LocalityPlanning),
                        operation,
                        solve: None,
                        locality: Err(ProjectedDragLocalityPlanningFailure::OperationStopped),
                    });
                    return Err(self
                        .editor
                        .projected_drag_result(pointer_id, request_id, point, None));
                }
                Err(error) => {
                    let failure = projected_drag_locality_failure(&error);
                    let operation = planning_controller.report();
                    self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                        pointer_id,
                        request_id,
                        point,
                        continued: false,
                        attempts: 0,
                        accepted: false,
                        rejection_stage: Some(ProjectedDragRejectionStage::LocalityPlanning),
                        operation,
                        solve: None,
                        locality: Err(failure),
                    });
                    return Err(self
                        .editor
                        .projected_drag_result(pointer_id, request_id, point, None));
                }
            }
        };
        Ok((locality, planning_controller.report()))
    }

    fn attempt_projected_drag(
        &mut self,
        point: DesignPointId,
        request: DocumentSolveRequest,
        locality: &DocumentDragLocalityPlan,
        continued: bool,
        planning_report: &OperationReport,
    ) -> ProjectedDragAttemptResult {
        let mut attempt_control = projected_drag_control();
        attempt_control.limits =
            remaining_operation_limits(planning_report.configured, planning_report.consumed);
        let mut candidate = self.session.clone();
        let outcome = if let (true, Some(preview)) = (continued, self.solved_preview.as_ref()) {
            candidate.reattempt_from_accepted_preview_with_drag_locality_controlled(
                candidate.design_identity(),
                request,
                preview,
                locality,
                attempt_control,
            )
        } else {
            candidate.reattempt_with_drag_locality_controlled(
                candidate.design_identity(),
                request,
                locality,
                attempt_control,
            )
        };
        let mut rejection_stage = None;
        let mut solve_work = None;
        let mut operation = *planning_report;
        let mut attempts = 1;
        let mut retain_continuation = true;
        let mut accepted_position = None;
        match outcome {
            Ok(OperationOutcome::Completed {
                value: attempt,
                report,
            }) => {
                accumulate_operation_report(&mut operation, &report);
                solve_work = attempt.solve_result().map(SketchSolveResult::work_summary);
                if attempt.failure().is_some() {
                    rejection_stage = Some(ProjectedDragRejectionStage::AttemptInput);
                } else if attempt
                    .solve_result()
                    .is_none_or(|solve| solve.rejection.is_some())
                {
                    rejection_stage = Some(ProjectedDragRejectionStage::Solve);
                } else if attempt.accepted_state_identity().is_none() {
                    rejection_stage = Some(ProjectedDragRejectionStage::AcceptedState);
                } else {
                    accepted_position = candidate
                        .accepted_state()
                        .and_then(|state| state.document().point(point))
                        .map(|value| value.position)
                        .filter(|position| position.iter().all(|value| value.is_finite()));
                    if accepted_position.is_none() {
                        rejection_stage = Some(ProjectedDragRejectionStage::AcceptedState);
                    } else if self
                        .mark_solved_preview_with_locality(&candidate, locality)
                        .is_err()
                    {
                        accepted_position = None;
                        rejection_stage = Some(ProjectedDragRejectionStage::PreviewPublication);
                    }
                }
            }
            Ok(stopped) => {
                accumulate_operation_report(&mut operation, stopped.report());
                rejection_stage = Some(ProjectedDragRejectionStage::ControlledOperation);
            }
            Err(_) => {
                // Both locality-aware retained APIs perform every fallible
                // identity/request/preview check before constructing their
                // OperationController. Once controlled solve work begins, setup failures are
                // retained as typed attempts and interruption is returned as OperationOutcome.
                // Locality-planning work, if any, remains visible in `operation`.
                attempts = 0;
                retain_continuation = false;
                rejection_stage = Some(ProjectedDragRejectionStage::Session);
            }
        }
        ProjectedDragAttemptResult {
            operation,
            solve: solve_work,
            attempts,
            retain_continuation,
            accepted_position,
            rejection_stage,
        }
    }

    /// Explicitly marks an outstanding solve. It does not mutate lifecycle history.
    pub fn mark_solving(&mut self) {
        self.transient = Some(TransientLifecycle::Solving);
        self.solved_preview = None;
        self.solved_preview_locality = None;
        self.drag_continuation = None;
        self.projected_drag_work = None;
    }

    /// Publishes a solved transient preview proved by a separate retained session.
    /// It does not mutate either retained session or claim persistent acceptance.
    ///
    /// # Errors
    ///
    /// Rejects foreign, stale, persisted, failed, rejected, or incoherent preview evidence.
    pub fn mark_solved_preview(
        &mut self,
        preview: &RetainedSketchDocumentSession,
    ) -> Result<(), CoordinatorError> {
        self.mark_solved_preview_inner(preview, None)
    }

    fn mark_solved_preview_with_locality(
        &mut self,
        preview: &RetainedSketchDocumentSession,
        locality: &DocumentDragLocalityPlan,
    ) -> Result<(), CoordinatorError> {
        self.mark_solved_preview_inner(preview, Some(locality))
    }

    fn mark_solved_preview_inner(
        &mut self,
        preview: &RetainedSketchDocumentSession,
        supplied_locality: Option<&DocumentDragLocalityPlan>,
    ) -> Result<(), CoordinatorError> {
        match self.session.validate_preview_for_current_design(preview) {
            Ok(()) => {}
            Err(DocumentSessionError::PreviewForeignDocument) => {
                return Err(CoordinatorError::PreviewForeignDocument);
            }
            Err(DocumentSessionError::PreviewStaleDesign) => {
                return Err(CoordinatorError::PreviewStaleDesign);
            }
            Err(DocumentSessionError::PreviewAcceptedProvenance) => {
                return Err(CoordinatorError::PreviewAcceptedStateMismatch);
            }
            Err(DocumentSessionError::PreviewNotAccepted) => {
                return Err(CoordinatorError::PreviewNotAccepted);
            }
            Err(error) => return Err(CoordinatorError::Session(error)),
        }
        let current_design = self.session.design_identity();
        let preview_design = preview.design_identity();
        let preview_attempt = preview.last_attempt();
        let preview_accepted = preview.accepted_state();
        if preview_design.document() != current_design.document()
            || preview_attempt.identity().document() != preview_design.document()
            || preview_accepted
                .is_some_and(|state| state.identity().document() != preview_design.document())
        {
            return Err(CoordinatorError::PreviewForeignDocument);
        }
        if preview_design != current_design || preview_attempt.design_identity() != current_design {
            return Err(CoordinatorError::PreviewStaleDesign);
        }
        if preview_attempt.identity() == self.session.last_attempt().identity() {
            return Err(CoordinatorError::PreviewAttemptMatchesPersisted);
        }
        let Some(preview_accepted) =
            preview_accepted.map(geosolve_sketch::SketchAcceptedDocumentState::identity)
        else {
            return Err(CoordinatorError::PreviewNotAccepted);
        };
        if preview_attempt.failure().is_some()
            || preview_attempt
                .solve_result()
                .is_some_and(|solve| solve.rejection.is_some())
            || preview_attempt.accepted_state_identity().is_none()
        {
            return Err(CoordinatorError::PreviewNotAccepted);
        }
        if preview_attempt.accepted_state_identity() != Some(preview_accepted) {
            return Err(CoordinatorError::PreviewAcceptedStateMismatch);
        }
        let preview_drag = preview_attempt.input().candidate_request().drag;
        let locality = match (preview_drag, supplied_locality) {
            (Some(drag), Some(locality))
                if locality.point() == drag.point
                    && locality.design_identity() == current_design
                    && Some(locality.accepted_state_identity())
                        == self
                            .session
                            .accepted_state()
                            .map(geosolve_sketch::SketchAcceptedDocumentState::identity) =>
            {
                Some(locality.clone())
            }
            (Some(drag), None) => Some(self.session.drag_locality_plan(drag.point)?),
            (None, None) => None,
            _ => return Err(CoordinatorError::SolvedPreviewMismatch),
        };
        self.transient = Some(TransientLifecycle::SolvedPreview {
            attempt: preview_attempt.identity(),
            accepted: preview_accepted,
        });
        self.solved_preview = Some(preview.clone());
        self.solved_preview_locality = locality;
        Ok(())
    }

    pub fn clear_transient(&mut self) {
        self.editor.acknowledge_point_preview_clear();
        self.transient = None;
        self.solved_preview = None;
        self.solved_preview_locality = None;
        self.drag_continuation = None;
        self.projected_drag_work = None;
        self.alternate_branch = None;
    }

    #[must_use]
    pub fn lifecycle(&self) -> LifecycleDto {
        let attempt = self.session.last_attempt();
        let accepted = self
            .session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
        let (status, preview_attempt, preview_accepted) = self.transient.map_or_else(
            || {
                if attempt.accepted_state_identity().is_some() {
                    (LifecycleStatus::Accepted, None, None)
                } else if accepted.is_some() {
                    (LifecycleStatus::RejectedAttempt, None, None)
                } else {
                    (LifecycleStatus::DesignUnsolved, None, None)
                }
            },
            |transient| match transient {
                TransientLifecycle::Solving => (LifecycleStatus::Solving, None, None),
                TransientLifecycle::SolvedPreview { attempt, accepted } => (
                    LifecycleStatus::SolvedPreview,
                    Some(attempt),
                    Some(accepted),
                ),
            },
        );
        LifecycleDto {
            status,
            design: self.session.design_identity(),
            attempt: attempt.identity(),
            preview_attempt,
            preview_accepted,
            accepted,
            parent_accepted: attempt.parent_accepted_identity(),
        }
    }

    #[must_use]
    pub fn problems(&self) -> ProblemsDto<'_> {
        let attempt = self.session.last_attempt();
        ProblemsDto {
            attempt: attempt.identity(),
            design: attempt.design_identity(),
            parent_accepted: attempt.parent_accepted_identity(),
            failure: attempt.failure(),
            rejection: attempt
                .solve_result()
                .and_then(|solve| solve.rejection.as_ref()),
        }
    }

    /// Returns structured presentation metadata for only the latest failed or rejected attempt.
    ///
    /// Attribution uses attempted runtime mappings and persistent document dependencies. When no
    /// individual persistent element is defensible, the problem remains explicitly global.
    #[must_use]
    pub fn current_problem_metadata(&self) -> Option<EditorProblemMetadata> {
        let attempt = self.session.last_attempt();
        if let Some(failure) = attempt.failure() {
            return Some(EditorProblemMetadata {
                attempt: attempt.identity(),
                design: attempt.design_identity(),
                category: failure_category(failure.kind()),
                scope: EditorProblemScope::Global,
                message: failure.message().to_owned(),
                targets: Vec::new(),
            });
        }
        let solve = attempt.solve_result()?;
        let rejection = solve.rejection.as_ref()?;
        let document = self.session.design_document();
        let mut elements = BTreeSet::new();

        for source in self
            .session
            .latest_attempt_diagnostics()
            .conflicts
            .candidates
        {
            insert_source_owner(&mut elements, document, source);
        }
        insert_rejection_elements(&mut elements, attempt, document, rejection);

        let roots = elements.iter().copied().collect::<Vec<_>>();
        for root in roots {
            elements.extend(document.dependency_closure(root));
        }
        let targets = elements
            .into_iter()
            .filter_map(problem_target)
            .collect::<Vec<_>>();
        let scope = if targets.is_empty() {
            EditorProblemScope::Global
        } else {
            EditorProblemScope::Targeted
        };
        Some(EditorProblemMetadata {
            attempt: attempt.identity(),
            design: attempt.design_identity(),
            category: rejection_category(rejection),
            scope,
            message: rejection_message(rejection),
            targets,
        })
    }

    /// Accepted audit is returned only from the coherent accepted state.
    #[must_use]
    pub fn accepted_audit(&self) -> Option<AuditDto<'_>> {
        let accepted = self.session.accepted_state()?;
        Some(AuditDto {
            provenance: AuditProvenance::Accepted(accepted.identity()),
            design: accepted.design_identity(),
            solve_result: accepted.solve_result(),
            mappings: accepted.mappings(),
        })
    }

    /// Passes through sketch-owned accepted redundancy without report interpretation.
    #[must_use]
    pub fn accepted_redundancy(&self) -> Option<&SketchAcceptedDocumentRedundancy> {
        self.session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::accepted_redundancy)
    }

    /// Attempt audit and mappings are kept together and never interpret accepted state.
    #[must_use]
    pub fn attempt_audit(&self) -> Option<AuditDto<'_>> {
        let attempt = self.session.last_attempt();
        Some(AuditDto {
            provenance: AuditProvenance::Attempt(attempt.identity()),
            design: attempt.design_identity(),
            solve_result: attempt.solve_result()?,
            mappings: attempt.mappings()?,
        })
    }

    /// Evaluates requested M38 sources. Stale, foreign, or missing provenance is withheld.
    #[must_use]
    pub fn measurements(
        &self,
        catalog: &DocumentMeasurementCatalog,
        sources: impl IntoIterator<Item = DocumentSourceId>,
    ) -> Vec<MeasurementPublication> {
        sources
            .into_iter()
            .map(
                |source| match catalog.evaluate_measurement(&self.session, source) {
                    Ok(value) => MeasurementPublication::Published(value),
                    Err(error) => MeasurementPublication::Withheld {
                        source,
                        reason: error.to_string(),
                    },
                },
            )
            .collect()
    }

    /// Publishes only measurements bound to the current accepted-state revision.
    ///
    /// M38 performs the foreign/stale revision check before returning a value; this
    /// additional filter prevents a retained-design value from entering an accepted panel.
    #[must_use]
    pub fn accepted_measurements(
        &self,
        catalog: &DocumentMeasurementCatalog,
        sources: impl IntoIterator<Item = DocumentSourceId>,
    ) -> Vec<MeasurementPublication> {
        let expected = self
            .session
            .accepted_state()
            .map(|state| state.identity().revision().get());
        self.measurements(catalog, sources)
            .into_iter()
            .map(|publication| match publication {
                MeasurementPublication::Published(value)
                    if matches!(
                        (value.audit.provenance, expected),
                        (
                            Some(DocumentMeasurementProvenance::AcceptedDocument { revision }),
                            Some(expected_revision)
                        ) if revision == expected_revision
                    ) =>
                {
                    MeasurementPublication::Published(value)
                }
                MeasurementPublication::Published(value) => MeasurementPublication::Withheld {
                    source: value.source_id,
                    reason: "measurement is not bound to the current accepted revision".into(),
                },
                withheld @ MeasurementPublication::Withheld { .. } => withheld,
            })
            .collect()
    }

    /// Replaces this lifecycle from an opaque checkpoint and starts fresh editor history.
    ///
    /// Current and checkpoint high-water metadata are merged, so reload cannot reuse
    /// any revision already observed by either lifecycle.
    ///
    /// # Errors
    ///
    /// Returns JSON, foreign-document, accepted-snapshot, solve-setup, or revision errors.
    pub fn reload(&mut self, saved_checkpoint: &RestoreCheckpoint) -> Result<(), CoordinatorError> {
        let current = self.session.revision_high_water();
        let saved = saved_checkpoint.revisions;
        let accepted = match (current.accepted(), saved.accepted()) {
            (Some(first), Some(second)) => Some(first.get().max(second.get())),
            (Some(value), None) | (None, Some(value)) => Some(value.get()),
            (None, None) => None,
        };
        let revisions = SketchLifecycleRevisionHighWater::from_raw(
            current.design().get().max(saved.design().get()),
            current.attempt().get().max(saved.attempt().get()),
            accepted,
        );
        let design = checkpoint_document_from_json(
            &saved_checkpoint.design_json,
            saved_checkpoint.design_is_draft_v5,
        )?;
        let input = self.session.last_attempt().input();
        let request = input
            .candidate_request()
            .without_temporary_targets()
            .without_previous_state_preferences();
        let restored = if let Some(json) = &saved_checkpoint.accepted_json {
            RetainedSketchDocumentSession::restore_design_with_accepted(
                design,
                checkpoint_document_from_json(json, saved_checkpoint.accepted_is_draft_v5)?,
                revisions,
                request,
                input.solver_config(),
            )?
        } else {
            RetainedSketchDocumentSession::restore_design(
                design,
                revisions,
                request,
                input.solver_config(),
            )?
        };
        self.session = restored;
        self.history.clear();
        self.history.push(checkpoint(&self.session)?);
        self.history_cursor = 0;
        self.transcript.clear();
        self.clear_transient();
        self.reconcile_selection();
        Ok(())
    }

    /// Returns the complete fixed-order action matrix for the current design selection.
    #[must_use]
    pub fn actions(&self) -> Vec<ActionAvailability> {
        let document = self.session.design_document();
        let selection = self.editor.selection();
        let mut actions = constraint_action_matrix(document, selection);
        actions.extend(dimension_action_matrix(document, selection));
        actions.extend([
            ActionAvailability {
                action: CoordinatorActionKind::EditContactBranch,
                state: contact_branch_availability(document, selection),
            },
            ActionAvailability {
                action: CoordinatorActionKind::SetAngleOrientation(
                    DocumentAngleOrientation::CounterClockwise,
                ),
                state: angle_orientation_availability(
                    document,
                    selection,
                    DocumentAngleOrientation::CounterClockwise,
                ),
            },
            ActionAvailability {
                action: CoordinatorActionKind::SetAngleOrientation(
                    DocumentAngleOrientation::Clockwise,
                ),
                state: angle_orientation_availability(
                    document,
                    selection,
                    DocumentAngleOrientation::Clockwise,
                ),
            },
            ActionAvailability {
                action: CoordinatorActionKind::Delete,
                state: availability(selected_objects(document, selection)),
            },
            ActionAvailability {
                action: CoordinatorActionKind::Suppress,
                state: source_availability(document, selection, true),
            },
            ActionAvailability {
                action: CoordinatorActionKind::Unsuppress,
                state: source_availability(document, selection, false),
            },
            ActionAvailability {
                action: CoordinatorActionKind::Undo,
                state: if self.can_undo() {
                    ActionState::Enabled
                } else {
                    ActionState::Disabled(DisabledReason::NothingToUndo)
                },
            },
            ActionAvailability {
                action: CoordinatorActionKind::Redo,
                state: if self.can_redo() {
                    ActionState::Enabled
                } else {
                    ActionState::Disabled(DisabledReason::NothingToRedo)
                },
            },
            ActionAvailability {
                action: CoordinatorActionKind::Reattempt,
                state: ActionState::Enabled,
            },
        ]);
        actions
    }

    /// Resolves one contextual intent to the exact persistent definition family.
    #[must_use]
    pub fn resolved_constraint(&self, intent: ConstraintIntent) -> Option<ResolvedConstraintKind> {
        resolve_constraint(
            self.session.design_document(),
            self.editor.selection(),
            intent,
        )
        .ok()
    }

    /// Returns explicit branch-choice metadata for one action.
    ///
    /// Defaults are fixed semantic values, never coordinate-derived root choices.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "the closed contextual branch-choice matrix is clearest in one exhaustive match"
    )]
    pub fn action_choices(&self, action: CoordinatorActionKind) -> Vec<ActionChoice> {
        let document = self.session.design_document();
        let selection = self.editor.selection();
        match action {
            CoordinatorActionKind::Constraint(intent) => {
                let Some(resolved) = self.resolved_constraint(intent) else {
                    return Vec::new();
                };
                if resolved == ResolvedConstraintKind::RadialLine {
                    return selected_radial_line(document, selection)
                        .and_then(|(line, _, operand)| {
                            contact_action_choice(
                                document,
                                operand,
                                line,
                                false,
                                false,
                                self.editor.curve_pick_parameter(line),
                            )
                        })
                        .into_iter()
                        .collect();
                }
                let (spans, tangency) = match resolved {
                    ResolvedConstraintKind::PointOnCurve
                    | ResolvedConstraintKind::CurveContact
                    | ResolvedConstraintKind::EqualCurvature
                    | ResolvedConstraintKind::EndpointContinuity => {
                        (selected_curve_spans(selection), false)
                    }
                    ResolvedConstraintKind::CurveTangency => {
                        (selected_curve_spans(selection), true)
                    }
                    _ => (Vec::new(), false),
                };
                let mut choices = spans
                    .into_iter()
                    .enumerate()
                    .filter_map(|(operand, span)| {
                        contact_action_choice(
                            document,
                            u8::try_from(operand).ok()?,
                            span,
                            tangency,
                            resolved == ResolvedConstraintKind::EndpointContinuity,
                            self.editor.curve_pick_parameter(span),
                        )
                    })
                    .collect::<Vec<_>>();
                choices.extend(match resolved {
                    ResolvedConstraintKind::EqualCurvature => {
                        vec![ActionChoice::EqualCurvature {
                            values: vec![
                                DocumentCurveCurvatureRelation::Signed,
                                DocumentCurveCurvatureRelation::MagnitudeSameSign,
                                DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
                            ],
                        }]
                    }
                    ResolvedConstraintKind::EndpointContinuity => {
                        vec![ActionChoice::Continuity {
                            values: vec![
                                DocumentCurveContinuity::G0,
                                DocumentCurveContinuity::G1,
                                DocumentCurveContinuity::G2,
                                DocumentCurveContinuity::ParametricC2 {
                                    first_rate: 1.0,
                                    second_rate: 1.0,
                                },
                            ],
                        }]
                    }
                    _ => Vec::new(),
                });
                choices
            }
            CoordinatorActionKind::Dimension(DimensionKind::OrientedAngle, _) => {
                vec![ActionChoice::AngleOrientation {
                    values: vec![
                        DocumentAngleOrientation::CounterClockwise,
                        DocumentAngleOrientation::Clockwise,
                    ],
                }]
            }
            _ => Vec::new(),
        }
    }

    /// Returns complete selection-scoped branch controls with persistent identities.
    #[must_use]
    pub fn branch_actions(&self) -> Vec<BranchAction> {
        let document = self.session.design_document();
        if let Some(contacts) = selected_contact_ids(document, self.editor.selection()) {
            return contacts
                .into_iter()
                .filter_map(|id| {
                    let contact = document
                        .contacts()
                        .iter()
                        .find(|contact| contact.id == id)?;
                    let value = document.scalar(contact.parameter)?.value;
                    Some(BranchAction::Contact(ContactBranchAction {
                        current: ContactBranchEdit {
                            contact: id,
                            curve: contact.curve,
                            domain: contact.domain,
                            value,
                            winding: contact.winding,
                            neighborhood: contact.neighborhood,
                            tangent_orientation: contact.tangent_orientation,
                        },
                        spans: document.curve_spans(contact.curve.curve).ok()?,
                        domains: document.curve_contact_domains(contact.curve).ok()?,
                        neighborhoods: contact_neighborhood_options(contact.domain, value),
                        tangent_orientations: if contact.tangent_orientation.is_some() {
                            vec![
                                Some(TangentOrientation::Aligned),
                                Some(TangentOrientation::Opposed),
                            ]
                        } else {
                            vec![None]
                        },
                    }))
                })
                .collect();
        }
        let [SelectionItem::Dimension(id)] = self.editor.selection() else {
            return Vec::new();
        };
        document
            .dimensions()
            .iter()
            .find(|dimension| dimension.id == *id)
            .and_then(|dimension| {
                let DocumentDimensionDefinition::OrientedAngle { orientation, .. } =
                    &dimension.definition
                else {
                    return None;
                };
                Some(BranchAction::AngleOrientation {
                    dimension: *id,
                    current: *orientation,
                })
            })
            .into_iter()
            .collect()
    }

    /// Applies one exact revision-checked closed edit and records valid retained mutations.
    ///
    /// # Errors
    ///
    /// Returns stale-design, document-validation, solve-setup, or checkpoint errors.
    pub fn apply_edit(
        &mut self,
        expected: SketchDesignIdentity,
        edit: DocumentEdit,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let replay = ReplayAction::Edit {
            expected,
            edit: edit.clone(),
        };
        let outcome = self.session.apply(expected, edit)?;
        let result = MutationOutcome {
            value: outcome.value().clone(),
            design: outcome.design_identity(),
            attempt: outcome.attempt_identity(),
            published_accepted: outcome.published_accepted_identity(),
        };
        self.record_mutation(replay)?;
        Ok(result)
    }

    /// Revision-checks and changes one curve's profile/construction role through the
    /// ordinary retained document-edit path.
    ///
    /// # Errors
    ///
    /// Returns stale-design, document-validation, solve-setup, or checkpoint errors.
    pub fn set_geometry_role(
        &mut self,
        expected: SketchDesignIdentity,
        curve: CurveId,
        role: GeometryRole,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.apply_edit(expected, DocumentEdit::SetGeometryRole { curve, role })
    }

    /// Explicitly changes one external binding's declared family/topology contract.
    ///
    /// This records one ordinary retained document transaction and never derives a
    /// replacement declaration from geometry.
    ///
    /// # Errors
    ///
    /// Returns stale-design, document-validation, solve-setup, or checkpoint errors.
    pub fn rebind_external_binding(
        &mut self,
        expected: SketchDesignIdentity,
        binding: DocumentExternalBindingId,
        expected_kind: ExternalFeatureKindV1,
        expected_topology: Option<ExternalTopologyDigest>,
    ) -> Result<MutationOutcome<()>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let outcome = self.session.transact(expected, |document| {
            document.rebind_external_binding(binding, expected_kind, expected_topology)
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::RebindExternalBinding {
            expected,
            binding,
            expected_kind,
            expected_topology,
        })?;
        Ok(result)
    }

    /// Replaces the complete immutable parameter input for one retained attempt.
    ///
    /// Host inputs are not canonical document history, so this clears transient preview
    /// state but deliberately creates neither a checkpoint nor replay action.
    ///
    /// # Errors
    ///
    /// Returns stale-design, stale-parameter-revision, or solve-setup errors.
    pub fn replace_parameter_batch(
        &mut self,
        expected: SketchDesignIdentity,
        batch: ParameterBatch,
        request: DocumentSolveRequest,
    ) -> Result<MutationOutcome<()>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let attempt = self
            .session
            .update_parameter_batch(expected, batch, request)?;
        let result = MutationOutcome {
            value: (),
            design: attempt.design_identity(),
            attempt: attempt.identity(),
            published_accepted: attempt.accepted_state_identity(),
        };
        self.clear_transient();
        Ok(result)
    }

    /// Replaces the complete immutable external snapshot input for one retained attempt.
    ///
    /// Host inputs are not canonical document history, so this clears transient preview
    /// state but deliberately creates neither a checkpoint nor replay action.
    ///
    /// # Errors
    ///
    /// Returns stale-design, stale-snapshot-revision, or solve-setup errors.
    pub fn replace_external_snapshot_set(
        &mut self,
        expected: SketchDesignIdentity,
        snapshots: ExternalSnapshotSet,
        request: DocumentSolveRequest,
    ) -> Result<MutationOutcome<()>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let attempt = self
            .session
            .update_external_snapshot_set(expected, snapshots, request)?;
        let result = MutationOutcome {
            value: (),
            design: attempt.design_identity(),
            attempt: attempt.identity(),
            published_accepted: attempt.accepted_state_identity(),
        };
        self.clear_transient();
        Ok(result)
    }

    /// Applies a construction proposal as one retained transaction and one checkpoint.
    ///
    /// # Errors
    ///
    /// Returns stale-design, construction, solve-setup, or checkpoint errors.
    pub fn apply_construction(
        &mut self,
        expected: SketchDesignIdentity,
        proposal: &ConstructionProposal,
    ) -> Result<MutationOutcome<ConstructionResult>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let replay = ReplayAction::Construction {
            expected,
            proposal: proposal.clone(),
        };
        let outcome = self
            .session
            .transact(expected, |document| proposal.apply(document))?;
        let result = MutationOutcome {
            value: outcome.value().clone(),
            design: outcome.design_identity(),
            attempt: outcome.attempt_identity(),
            published_accepted: outcome.published_accepted_identity(),
        };
        self.record_mutation(replay)?;
        Ok(result)
    }

    /// Applies one complete alpha relation action over the current selection.
    ///
    /// Contact-based actions require explicit domain, span, parameter,
    /// neighborhood, winding and tangent-orientation state. No root or branch is
    /// inferred from coordinates.
    ///
    /// # Errors
    ///
    /// Returns an applicability, branch-input, stale-design, document,
    /// solve-setup, or checkpoint error.
    pub fn apply_constraint_action(
        &mut self,
        expected: SketchDesignIdentity,
        request: ConstraintActionRequest,
    ) -> Result<MutationOutcome<geosolve_sketch::DocumentConstraintId>, CoordinatorError> {
        let selection = self.editor.selection().to_vec();
        self.apply_constraint_action_for(expected, &selection, request)
    }

    /// Applies one complete relation action over explicit immutable operands.
    ///
    /// Unlike [`Self::apply_constraint_action`], this entry point never reads or
    /// changes application selection.
    ///
    /// # Errors
    ///
    /// Returns an applicability, branch-input, stale-design, document, solve-setup,
    /// or checkpoint error.
    pub fn apply_constraint_action_for(
        &mut self,
        expected: SketchDesignIdentity,
        selection: &[SelectionItem],
        request: ConstraintActionRequest,
    ) -> Result<MutationOutcome<geosolve_sketch::DocumentConstraintId>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let resolved =
            resolve_constraint(self.session.design_document(), selection, request.intent)
                .map_err(CoordinatorError::ActionUnavailable)?;
        let replay_request = request.clone();
        let selection = selection.to_vec();
        let outcome = match resolved {
            ResolvedConstraintKind::PointOnCurve => self.apply_point_curve_action(
                expected,
                &selection,
                request.label,
                &request.contacts,
            )?,
            ResolvedConstraintKind::CurveContact
            | ResolvedConstraintKind::CurveTangency
            | ResolvedConstraintKind::EqualCurvature
            | ResolvedConstraintKind::EndpointContinuity => self.apply_curve_pair_action(
                expected,
                &selection,
                resolved,
                request.label,
                &request.contacts,
                request.relation,
            )?,
            ResolvedConstraintKind::RadialLine => self.apply_radial_line_action(
                expected,
                &selection,
                request.label,
                &request.contacts,
                request.relation,
            )?,
            _ => {
                if !request.contacts.is_empty() || request.relation.is_some() {
                    return Err(CoordinatorError::InvalidActionInput(
                        "this relation action accepts no explicit branch choices",
                    ));
                }
                let kind = simple_constraint_kind(resolved).ok_or(
                    CoordinatorError::InvalidActionInput(
                        "contextual relation did not resolve to a simple constraint",
                    ),
                )?;
                let edit = crate::constraint_edit(
                    self.session.design_document(),
                    &selection,
                    kind,
                    request.label,
                )?;
                let DocumentEdit::CreateConstraint { label, definition } = edit else {
                    unreachable!("simple relation policy emits one constraint creation");
                };
                self.session.transact(expected, move |document| {
                    document.add_constraint(label, definition)
                })?
            }
        };
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::ConstraintAction {
            expected,
            selection,
            request: replay_request,
        })?;
        Ok(result)
    }

    fn apply_point_curve_action(
        &mut self,
        expected: SketchDesignIdentity,
        selection: &[SelectionItem],
        label: String,
        contacts: &[crate::ContactActionChoice],
    ) -> Result<
        geosolve_sketch::RetainedDocumentTransactionOutcome<geosolve_sketch::DocumentConstraintId>,
        CoordinatorError,
    > {
        let (point, span) =
            selected_point_curve(selection).ok_or(CoordinatorError::InvalidActionInput(
                "point-on-curve requires one point and one curve span",
            ))?;
        let [choice] = contacts else {
            return Err(CoordinatorError::InvalidActionInput(
                "point-on-curve requires one explicit contact choice",
            ));
        };
        validate_contact_choice(span, choice, false)?;
        let choice = *choice;
        Ok(self.session.transact(expected, move |document| {
            let contact = add_action_contact(document, &label, 0, choice)?;
            document.add_constraint(
                label,
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            )
        })?)
    }

    fn apply_curve_pair_action(
        &mut self,
        expected: SketchDesignIdentity,
        selection: &[SelectionItem],
        resolved: ResolvedConstraintKind,
        label: String,
        contacts: &[crate::ContactActionChoice],
        relation: Option<ConstraintRelationChoice>,
    ) -> Result<
        geosolve_sketch::RetainedDocumentTransactionOutcome<geosolve_sketch::DocumentConstraintId>,
        CoordinatorError,
    > {
        let spans = selected_curve_pair(selection).ok_or(CoordinatorError::InvalidActionInput(
            "generic relations require two curve spans",
        ))?;
        let [first, second] = contacts else {
            return Err(CoordinatorError::InvalidActionInput(
                "generic relations require two explicit contact choices",
            ));
        };
        let tangency = resolved == ResolvedConstraintKind::CurveTangency;
        validate_contact_choice(spans[0], first, tangency)?;
        validate_contact_choice(spans[1], second, tangency)?;
        if tangency && first.tangent_orientation != second.tangent_orientation {
            return Err(CoordinatorError::InvalidActionInput(
                "tangency contacts must share one explicit orientation",
            ));
        }
        let first = *first;
        let second = *second;
        validate_pair_relation_choice(resolved, relation)?;
        Ok(self.session.transact(expected, move |document| {
            let first_contact = add_action_contact(document, &label, 0, first)?;
            let second_contact = add_action_contact(document, &label, 1, second)?;
            let definition = match resolved {
                ResolvedConstraintKind::CurveTangency => {
                    DocumentConstraintDefinition::CurveCurveTangency {
                        first_contact,
                        second_contact,
                    }
                }
                ResolvedConstraintKind::CurveContact => {
                    DocumentConstraintDefinition::CurveCurveContact {
                        first_contact,
                        second_contact,
                    }
                }
                ResolvedConstraintKind::EqualCurvature => {
                    let Some(ConstraintRelationChoice::EqualCurvature(relation)) = relation else {
                        unreachable!("equal-curvature relation choice validated");
                    };
                    DocumentConstraintDefinition::EqualCurvature {
                        first_contact,
                        second_contact,
                        relation,
                    }
                }
                ResolvedConstraintKind::EndpointContinuity => {
                    let Some(ConstraintRelationChoice::Continuity(continuity)) = relation else {
                        unreachable!("continuity relation choice validated");
                    };
                    DocumentConstraintDefinition::EndpointContinuity {
                        first_contact,
                        second_contact,
                        continuity,
                    }
                }
                _ => unreachable!("curve-pair action resolution validated"),
            };
            document.add_constraint(label, definition)
        })?)
    }

    fn apply_radial_line_action(
        &mut self,
        expected: SketchDesignIdentity,
        selection: &[SelectionItem],
        label: String,
        contacts: &[crate::ContactActionChoice],
        relation: Option<ConstraintRelationChoice>,
    ) -> Result<
        geosolve_sketch::RetainedDocumentTransactionOutcome<geosolve_sketch::DocumentConstraintId>,
        CoordinatorError,
    > {
        let (line, center, _) = selected_radial_line(self.session.design_document(), selection)
            .ok_or(CoordinatorError::InvalidActionInput(
                "circle normal requires one line and one circle or circular arc",
            ))?;
        let [choice] = contacts else {
            return Err(CoordinatorError::InvalidActionInput(
                "circle normal requires one explicit line contact",
            ));
        };
        validate_contact_choice(line, choice, false)?;
        if relation.is_some() {
            return Err(CoordinatorError::InvalidActionInput(
                "circle normal accepts no separate direction branch",
            ));
        }
        let choice = *choice;
        Ok(self.session.transact(expected, move |document| {
            let line_contact = add_action_contact(document, &label, 0, choice)?;
            document.add_constraint(
                label,
                DocumentConstraintDefinition::PointOnCurve {
                    point: center,
                    contact: line_contact,
                },
            )
        })?)
    }

    /// Applies one complete alpha dimension action at the current accepted value.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-design, document, solve-setup, or
    /// checkpoint error.
    pub fn apply_dimension_action(
        &mut self,
        expected: SketchDesignIdentity,
        request: DimensionActionRequest,
    ) -> Result<MutationOutcome<DocumentDimensionId>, CoordinatorError> {
        let selection = self.editor.selection().to_vec();
        self.apply_dimension_action_for(expected, &selection, request)
    }

    /// Applies one complete dimension action over explicit immutable operands.
    ///
    /// This entry point never reads or changes application selection.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-design, document, solve-setup, or checkpoint
    /// error.
    pub fn apply_dimension_action_for(
        &mut self,
        expected: SketchDesignIdentity,
        selection: &[SelectionItem],
        request: DimensionActionRequest,
    ) -> Result<MutationOutcome<DocumentDimensionId>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let accepted = self
            .session
            .accepted_state()
            .ok_or(CoordinatorError::ActionUnavailable(
                DisabledReason::MissingObject,
            ))?;
        let target = dimension_target(
            accepted.document(),
            selection,
            request.kind,
            request.angle_orientation,
        )
        .map_err(CoordinatorError::ActionUnavailable)?;
        let definition =
            dimension_operands(self.session.design_document(), selection, request.kind)?;
        let selection = selection.to_vec();
        let replay_request = request.clone();
        let label = request.label;
        let mode = request.mode;
        let angle_orientation = request.angle_orientation;
        let unit = if request.kind == DimensionKind::OrientedAngle {
            ScalarUnit::Angle
        } else {
            ScalarUnit::Length
        };
        let outcome = self.session.transact(expected, move |document| {
            let scalar = document.add_scalar(
                format!("{label} target"),
                target,
                unit,
                ScalarDomain::Positive,
            )?;
            document.add_dimension(
                label,
                definition.definition(scalar, angle_orientation),
                mode,
            )
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::DimensionAction {
            expected,
            selection,
            request: replay_request,
        })?;
        Ok(result)
    }

    /// Applies one complete request produced by [`crate::AuthoringState`].
    ///
    /// Branch defaults are explicit values from [`AuthoringOptions`]. Picked curve
    /// parameters are retained when valid for the selected semantic domain.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-resolution, branch-input, retained-session,
    /// document or checkpoint error.
    pub fn apply_authoring(
        &mut self,
        expected: SketchDesignIdentity,
        application: &AuthoringApplication,
    ) -> Result<AuthoringMutation, CoordinatorError> {
        let selection = application
            .operands
            .iter()
            .map(|operand| operand.item)
            .collect::<Vec<_>>();
        match application.tool {
            AuthoringTool::Constraint(intent) => {
                let resolved =
                    resolve_constraint(self.session.design_document(), &selection, intent)
                        .map_err(CoordinatorError::ActionUnavailable)?;
                if application.resolved_constraint != Some(resolved) {
                    return Err(CoordinatorError::InvalidActionInput(
                        "authoring resolution is stale",
                    ));
                }
                let request = self.authoring_constraint_request(
                    intent,
                    resolved,
                    &selection,
                    &application.operands,
                    application.options,
                )?;
                self.apply_constraint_action_for(expected, &selection, request)
                    .map(AuthoringMutation::Constraint)
            }
            AuthoringTool::Dimension(kind) => self
                .apply_dimension_action_for(
                    expected,
                    &selection,
                    DimensionActionRequest {
                        kind,
                        mode: application.options.dimension_mode,
                        label: dimension_action_label(kind).to_owned(),
                        angle_orientation: application.options.angle_orientation,
                    },
                )
                .map(AuthoringMutation::Dimension),
        }
    }

    fn authoring_constraint_request(
        &self,
        intent: ConstraintIntent,
        resolved: ResolvedConstraintKind,
        selection: &[SelectionItem],
        operands: &[AuthoringOperand],
        options: AuthoringOptions,
    ) -> Result<ConstraintActionRequest, CoordinatorError> {
        let document = self.session.design_document();
        let contact_operands = match resolved {
            ResolvedConstraintKind::RadialLine => selected_radial_line(document, selection)
                .and_then(|(line, _, _)| {
                    operands
                        .iter()
                        .find(|operand| operand.item == SelectionItem::Curve(line))
                        .map(|operand| vec![(line, operand.curve_parameter)])
                })
                .unwrap_or_default(),
            ResolvedConstraintKind::PointOnCurve
            | ResolvedConstraintKind::CurveContact
            | ResolvedConstraintKind::CurveTangency
            | ResolvedConstraintKind::EqualCurvature
            | ResolvedConstraintKind::EndpointContinuity => operands
                .iter()
                .filter_map(|operand| match operand.item {
                    SelectionItem::Curve(span) => Some((span, operand.curve_parameter)),
                    SelectionItem::Point(_)
                    | SelectionItem::Constraint(_)
                    | SelectionItem::Dimension(_) => None,
                })
                .collect(),
            ResolvedConstraintKind::FixedPoint
            | ResolvedConstraintKind::CoincidentPoints
            | ResolvedConstraintKind::HorizontalLine
            | ResolvedConstraintKind::VerticalLine
            | ResolvedConstraintKind::ParallelLines
            | ResolvedConstraintKind::PerpendicularLines
            | ResolvedConstraintKind::EqualLength
            | ResolvedConstraintKind::EqualRadius
            | ResolvedConstraintKind::Midpoint
            | ResolvedConstraintKind::SymmetricAboutLine => Vec::new(),
        };
        let tangency = resolved == ResolvedConstraintKind::CurveTangency;
        let endpoint_only = resolved == ResolvedConstraintKind::EndpointContinuity;
        let contacts = contact_operands
            .into_iter()
            .enumerate()
            .map(|(index, (span, picked_parameter))| {
                let ActionChoice::Contact {
                    domains,
                    default_parameter,
                    neighborhoods,
                    default_winding,
                    ..
                } = contact_action_choice(
                    document,
                    u8::try_from(index).map_err(|_| {
                        CoordinatorError::InvalidActionInput("too many authoring contacts")
                    })?,
                    span,
                    tangency,
                    endpoint_only,
                    picked_parameter,
                )
                .ok_or(CoordinatorError::InvalidActionInput(
                    "selected curve has no valid contact domain",
                ))?
                else {
                    unreachable!("contact choice constructor emits contact metadata");
                };
                let domain = *domains.first().ok_or(CoordinatorError::InvalidActionInput(
                    "selected curve has no valid contact domain",
                ))?;
                let neighborhood =
                    *neighborhoods
                        .first()
                        .ok_or(CoordinatorError::InvalidActionInput(
                            "selected curve has no valid contact neighborhood",
                        ))?;
                Ok(crate::ContactActionChoice {
                    support: geosolve_sketch::DocumentCurveSpanRef {
                        span,
                        winding: default_winding,
                    },
                    domain,
                    parameter: default_parameter,
                    neighborhood,
                    tangent_orientation: tangency.then_some(options.tangent_orientation),
                })
            })
            .collect::<Result<Vec<_>, CoordinatorError>>()?;
        let relation = match resolved {
            ResolvedConstraintKind::EqualCurvature => Some(
                ConstraintRelationChoice::EqualCurvature(options.curvature_relation),
            ),
            ResolvedConstraintKind::EndpointContinuity => {
                Some(ConstraintRelationChoice::Continuity(options.continuity))
            }
            _ => None,
        };
        Ok(ConstraintActionRequest {
            intent,
            label: resolved.label().to_owned(),
            contacts,
            relation,
        })
    }

    /// Returns editable target metadata for exactly one explicitly selected dimension.
    #[must_use]
    pub fn dimension_target_metadata_for(
        &self,
        selection: &[SelectionItem],
    ) -> Option<DimensionTargetMetadata> {
        let [SelectionItem::Dimension(id)] = selection else {
            return None;
        };
        let dimension = self
            .session
            .design_document()
            .dimensions()
            .iter()
            .find(|dimension| dimension.id == *id)?;
        let scalar = dimension_target_scalar(&dimension.definition);
        let target = self.session.design_document().scalar(scalar)?;
        let display = display_dimension_target(target.value, target.unit)?;
        Some(DimensionTargetMetadata {
            dimension: *id,
            scalar,
            value: target.value,
            unit: target.unit,
            display_value: display.value,
            display_unit: display.unit,
            mode: dimension.mode,
        })
    }

    /// Returns editable target metadata for the current application selection.
    #[must_use]
    pub fn selected_dimension_target_metadata(&self) -> Option<DimensionTargetMetadata> {
        self.dimension_target_metadata_for(self.editor.selection())
    }

    /// Retains one finite target edit through ordinary document history.
    ///
    /// # Errors
    ///
    /// Returns a missing-dimension, invalid-scalar, stale-design, retained-session or
    /// checkpoint error.
    pub fn set_dimension_target(
        &mut self,
        expected: SketchDesignIdentity,
        dimension: DocumentDimensionId,
        value: f64,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        let metadata = self
            .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
            .ok_or(CoordinatorError::ActionUnavailable(
                DisabledReason::MissingObject,
            ))?;
        self.apply_edit(
            expected,
            DocumentEdit::SetScalarValue {
                scalar: metadata.scalar,
                value,
            },
        )
    }

    /// Retains one finite presentation-domain target edit through ordinary history.
    ///
    /// Lengths use model units. Oriented angles use acute supporting-line degrees;
    /// the existing directed radian quadrant and complete-turn branch remain
    /// explicit and unchanged.
    ///
    /// # Errors
    ///
    /// Returns a missing-dimension, invalid display value, invalid-scalar,
    /// stale-design, retained-session or checkpoint error.
    pub fn set_dimension_display_target(
        &mut self,
        expected: SketchDesignIdentity,
        dimension: DocumentDimensionId,
        display_value: f64,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        let metadata = self
            .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
            .ok_or(CoordinatorError::ActionUnavailable(
                DisabledReason::MissingObject,
            ))?;
        let value = storage_dimension_target(metadata, display_value)?;
        self.set_dimension_target(expected, dimension, value)
    }

    /// Applies complete explicit branch edits for one selected contact source.
    ///
    /// # Errors
    ///
    /// Returns a stale-design, source-membership, document, solve-setup, or
    /// checkpoint error.
    pub fn set_contact_branches(
        &mut self,
        expected: SketchDesignIdentity,
        edits: Vec<ContactBranchEdit>,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let selection = self.editor.selection().to_vec();
        let selected =
            selected_contact_ids(self.session.design_document(), self.editor.selection()).ok_or(
                CoordinatorError::ActionUnavailable(DisabledReason::WrongOperandKind),
            )?;
        if selected != edits.iter().map(|edit| edit.contact).collect::<Vec<_>>() {
            return Err(CoordinatorError::InvalidActionInput(
                "branch edits must cover the selected source contacts in semantic order",
            ));
        }
        let replay_edits = edits.clone();
        let outcome = self
            .session
            .apply(expected, DocumentEdit::SetContactBranches { edits })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::SetContactBranches {
            expected,
            selection,
            edits: replay_edits,
        })?;
        Ok(result)
    }

    /// Changes one selected oriented-angle dimension's explicit direction.
    ///
    /// # Errors
    ///
    /// Returns a stale-design, applicability, document, solve-setup, or
    /// checkpoint error.
    pub fn set_selected_angle_orientation(
        &mut self,
        expected: SketchDesignIdentity,
        orientation: DocumentAngleOrientation,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let [SelectionItem::Dimension(dimension)] = self.editor.selection() else {
            return Err(CoordinatorError::ActionUnavailable(
                DisabledReason::WrongOperandKind,
            ));
        };
        if angle_orientation_availability(
            self.session.design_document(),
            self.editor.selection(),
            orientation,
        ) != ActionState::Enabled
        {
            return Err(CoordinatorError::ActionUnavailable(
                DisabledReason::AlreadyInRequestedState,
            ));
        }
        let dimension = *dimension;
        let outcome = self.session.apply(
            expected,
            DocumentEdit::SetOrientedAngleOrientation {
                dimension,
                orientation,
            },
        )?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::SetAngleOrientation {
            expected,
            dimension,
            orientation,
        })?;
        Ok(result)
    }

    /// Adds a point-distance dimension and its target scalar atomically.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-design, document, solve-setup, or checkpoint error.
    pub fn add_point_distance_dimension(
        &mut self,
        expected: SketchDesignIdentity,
        mode: DocumentDimensionMode,
        label: impl Into<String>,
    ) -> Result<MutationOutcome<DocumentDimensionId>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let [SelectionItem::Point(first), SelectionItem::Point(second)] = self.editor.selection()
        else {
            return Err(CoordinatorError::IncompatibleDimension);
        };
        let target = point_distance_target(self.session.design_document(), self.editor.selection())
            .map_err(|_| CoordinatorError::IncompatibleDimension)?;
        let first = *first;
        let second = *second;
        let label = label.into();
        let replay_label = label.clone();
        let outcome = self.session.transact(expected, move |document| {
            let scalar = document.add_scalar(
                format!("{label} target"),
                target,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )?;
            document.add_dimension(
                label,
                DocumentDimensionDefinition::PointDistance {
                    first,
                    second,
                    target: scalar,
                },
                mode,
            )
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::PointDistance {
            expected,
            points: [first, second],
            mode,
            label: replay_label,
        })?;
        Ok(result)
    }

    /// Routes the current selection to the one compatible frozen core dimension family.
    ///
    /// Presentation adapters provide only widget-owned mode and label values. Operand
    /// compatibility, point-distance versus linear-span routing, target evaluation, and
    /// mutation remain coordinator policy.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-design, document, solve-setup, or checkpoint error.
    pub fn add_selected_dimension(
        &mut self,
        expected: SketchDesignIdentity,
        mode: DocumentDimensionMode,
        label: impl Into<String>,
    ) -> Result<MutationOutcome<DocumentDimensionId>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let label = label.into();
        if point_distance_target(self.session.design_document(), self.editor.selection()).is_ok() {
            self.add_point_distance_dimension(expected, mode, label)
        } else if segment_length_target(self.session.design_document(), self.editor.selection())
            .is_ok()
        {
            self.add_segment_length_dimension(expected, mode, label)
        } else {
            Err(CoordinatorError::IncompatibleDimension)
        }
    }

    /// Adds a selected linear-span length dimension and target scalar atomically.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-design, document, solve-setup, or checkpoint error.
    pub fn add_segment_length_dimension(
        &mut self,
        expected: SketchDesignIdentity,
        mode: DocumentDimensionMode,
        label: impl Into<String>,
    ) -> Result<MutationOutcome<DocumentDimensionId>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let [SelectionItem::Curve(curve)] = self.editor.selection() else {
            return Err(CoordinatorError::IncompatibleDimension);
        };
        let target = segment_length_target(self.session.design_document(), self.editor.selection())
            .map_err(|_| CoordinatorError::IncompatibleDimension)?;
        let curve = *curve;
        let label = label.into();
        let replay_label = label.clone();
        let outcome = self.session.transact(expected, move |document| {
            let scalar = document.add_scalar(
                format!("{label} target"),
                target,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )?;
            document.add_dimension(
                label,
                DocumentDimensionDefinition::CurveLength {
                    curve,
                    target: scalar,
                },
                mode,
            )
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::SegmentLength {
            expected,
            curve,
            mode,
            label: replay_label,
        })?;
        Ok(result)
    }

    /// Revision-checks and changes one extant dimension's driving/reference mode.
    ///
    /// # Errors
    ///
    /// Returns a deterministic unavailable-action reason, stale-design, document,
    /// solve-setup, or checkpoint error.
    pub fn set_dimension_mode(
        &mut self,
        expected: SketchDesignIdentity,
        dimension: DocumentDimensionId,
        mode: DocumentDimensionMode,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let value = self
            .session
            .design_document()
            .dimensions()
            .iter()
            .find(|value| value.id == dimension)
            .ok_or(CoordinatorError::ActionUnavailable(
                DisabledReason::MissingObject,
            ))?;
        if value.mode == mode {
            return Err(CoordinatorError::ActionUnavailable(
                DisabledReason::AlreadyInRequestedState,
            ));
        }
        let outcome = self
            .session
            .apply(expected, DocumentEdit::SetDimensionMode { dimension, mode })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::SetDimensionMode {
            expected,
            dimension,
            mode,
        })?;
        Ok(result)
    }

    /// Deletes every distinct selected document object in ordered selection order.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-design, dependency, solve-setup, or checkpoint error.
    pub fn delete_selected(
        &mut self,
        expected: SketchDesignIdentity,
    ) -> Result<MutationOutcome<Vec<DocumentObjectId>>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let selection = self.editor.selection().to_vec();
        let objects = selected_objects(self.session.design_document(), self.editor.selection())
            .map_err(|_| CoordinatorError::IncompatibleDimension)?;
        let outcome = self.session.transact(expected, move |document| {
            document.remove_many_with_dependents(&objects)?;
            Ok(objects)
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::Delete {
            expected,
            selection,
        })?;
        Ok(result)
    }

    /// Changes suppression for every selected persistent source atomically.
    ///
    /// # Errors
    ///
    /// Returns an applicability, stale-design, document, solve-setup, or checkpoint error.
    pub fn set_selected_suppressed(
        &mut self,
        expected: SketchDesignIdentity,
        suppressed: bool,
    ) -> Result<MutationOutcome<Vec<DocumentSourceId>>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let selection = self.editor.selection().to_vec();
        let sources = selected_sources(self.session.design_document(), self.editor.selection())
            .ok_or(CoordinatorError::IncompatibleDimension)?;
        if sources.iter().any(|source| {
            self.session
                .design_document()
                .source(*source)
                .is_none_or(|value| value.suppressed == suppressed)
        }) {
            return Err(CoordinatorError::IncompatibleDimension);
        }
        let outcome = self.session.transact(expected, move |document| {
            for source in &sources {
                document.set_source_suppressed(*source, suppressed)?;
            }
            Ok(sources)
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(ReplayAction::SetSuppressed {
            expected,
            selection,
            suppressed,
        })?;
        Ok(result)
    }

    /// Reattempts current design without creating a history checkpoint.
    ///
    /// # Errors
    ///
    /// Returns a stale-design or solve-setup error.
    pub fn reattempt(
        &mut self,
        expected: SketchDesignIdentity,
    ) -> Result<SketchAttemptIdentity, CoordinatorError> {
        self.ensure_expected(expected)?;
        let request = self.session.last_attempt().input().candidate_request();
        let attempt = self.session.reattempt(expected, request)?.identity();
        self.transcript.push(ReplayAction::Reattempt { expected });
        self.clear_transient();
        Ok(attempt)
    }

    /// Applies a commit effect through the same revision-checked retained policy as
    /// direct coordinator actions. Preview, clear, and selection effects are ignored.
    ///
    /// # Errors
    ///
    /// Returns stale-design, document, solve-setup, or checkpoint errors.
    pub fn apply_editor_effect(
        &mut self,
        effect: &EditorEffect,
    ) -> Result<Option<MutationOutcome<EditorMutation>>, CoordinatorError> {
        self.apply_editor_effect_with_point_publication_control(effect, projected_drag_control())
    }

    fn apply_editor_effect_with_point_publication_control(
        &mut self,
        effect: &EditorEffect,
        point_publication_control: OperationControl,
    ) -> Result<Option<MutationOutcome<EditorMutation>>, CoordinatorError> {
        match effect {
            EditorEffect::CommitPointMove {
                expected,
                point,
                model_position,
            } => self
                .commit_point_move_from_preview(
                    *expected,
                    *point,
                    *model_position,
                    point_publication_control,
                )
                .map(Some),
            EditorEffect::ClearPointPreview => {
                // A successful publication records a mutation and has already cleared
                // `solved_preview`. A failed publication deliberately leaves that exact
                // independently accepted preview in place, so its immediately following
                // terminal clear is consumed without discarding retryable state.
                if self.solved_preview.is_none() {
                    self.clear_transient();
                }
                Ok(None)
            }
            EditorEffect::CancelPointPreview => {
                self.clear_transient();
                Ok(None)
            }
            EditorEffect::CommitConstruction { expected, proposal } => {
                self.ensure_expected(*expected)?;
                let outcome = self.apply_construction(*expected, proposal)?;
                Ok(Some(MutationOutcome {
                    value: EditorMutation::Construction(outcome.value),
                    design: outcome.design,
                    attempt: outcome.attempt,
                    published_accepted: outcome.published_accepted,
                }))
            }
            EditorEffect::CommitInference(ProvisionalInferenceCandidate {
                expected, edit, ..
            }) => {
                let outcome = self.apply_edit(*expected, edit.clone())?;
                Ok(Some(MutationOutcome {
                    value: EditorMutation::Inference(outcome.value),
                    design: outcome.design,
                    attempt: outcome.attempt,
                    published_accepted: outcome.published_accepted,
                }))
            }
            EditorEffect::SelectionChanged(_)
            | EditorEffect::HoverChanged(_)
            | EditorEffect::PreviewPointMove { .. }
            | EditorEffect::RequestProjectedPointMove { .. }
            | EditorEffect::PreviewConstruction(_)
            | EditorEffect::ClearConstructionPreview
            | EditorEffect::PreviewInference(_)
            | EditorEffect::ClearInferencePreview => Ok(None),
        }
    }

    fn commit_point_move_from_preview(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        model_position: [f64; 2],
        publication_control: OperationControl,
    ) -> Result<MutationOutcome<EditorMutation>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let preview = self
            .solved_preview
            .as_ref()
            .ok_or(CoordinatorError::MissingSolvedPreview)?;
        let preview_attempt = preview.last_attempt();
        let preview_position = preview
            .accepted_state()
            .and_then(|state| state.document().point(point))
            .map(|value| value.position);
        if preview_attempt
            .input()
            .candidate_request()
            .drag
            .map(|drag| drag.point)
            != Some(point)
            || preview_position.map(|value| value.map(f64::to_bits))
                != Some(model_position.map(f64::to_bits))
        {
            return Err(CoordinatorError::SolvedPreviewMismatch);
        }
        let locality = self
            .solved_preview_locality
            .as_ref()
            .filter(|locality| locality.point() == point && locality.design_identity() == expected)
            .cloned()
            .ok_or(CoordinatorError::MissingDragLocalityPlan)?;
        let replay = ReplayAction::Edit {
            expected,
            edit: DocumentEdit::SetPointPosition {
                point,
                position: model_position,
            },
        };
        let mut candidate_session = self.session.clone();
        let controlled = candidate_session
            .apply_point_position_from_preview_with_drag_locality_controlled(
                expected,
                point,
                model_position,
                preview,
                &locality,
                publication_control,
            )?;
        let retained = match controlled {
            OperationOutcome::Completed { value, .. } => value,
            OperationOutcome::Cancelled { report } | OperationOutcome::WorkExhausted { report } => {
                return Err(CoordinatorError::OperationStopped {
                    context: "point-move publication",
                    report: Box::new(report),
                });
            }
            outcome => {
                return Err(CoordinatorError::OperationStopped {
                    context: "point-move publication",
                    report: Box::new(*outcome.report()),
                });
            }
        };
        let next_checkpoint = checkpoint(&candidate_session)?;
        let outcome = MutationOutcome {
            value: retained.value().clone(),
            design: retained.design_identity(),
            attempt: retained.attempt_identity(),
            published_accepted: retained.published_accepted_identity(),
        };
        self.session = candidate_session;
        self.record_prepared_mutation(next_checkpoint, replay);
        Ok(MutationOutcome {
            value: EditorMutation::PointMove(outcome.value),
            design: outcome.design,
            attempt: outcome.attempt,
            published_accepted: outcome.published_accepted,
        })
    }

    /// Applies one recorded transition against the identities encoded in the transcript.
    ///
    /// # Errors
    ///
    /// Returns the same applicability, stale-design, domain, history, and checkpoint
    /// errors as the corresponding coordinator operation.
    pub fn replay(&mut self, action: &ReplayAction) -> Result<(), CoordinatorError> {
        if let Some(expected) = action.expected_design() {
            self.ensure_expected(expected)?;
        }
        if self.replay_m55_action(action)? {
            return Ok(());
        }
        match action {
            ReplayAction::Edit { expected, edit } => {
                self.apply_edit(*expected, edit.clone())?;
            }
            ReplayAction::Construction { expected, proposal } => {
                self.apply_construction(*expected, proposal)?;
            }
            ReplayAction::PointDistance {
                expected,
                points,
                mode,
                label,
            } => {
                self.editor
                    .set_selection(points.iter().copied().map(SelectionItem::Point));
                self.add_point_distance_dimension(*expected, *mode, label.clone())?;
            }
            ReplayAction::SegmentLength {
                expected,
                curve,
                mode,
                label,
            } => {
                self.editor.set_selection([SelectionItem::Curve(*curve)]);
                self.add_segment_length_dimension(*expected, *mode, label.clone())?;
            }
            ReplayAction::SetDimensionMode {
                expected,
                dimension,
                mode,
            } => {
                self.editor
                    .set_selection([SelectionItem::Dimension(*dimension)]);
                self.set_dimension_mode(*expected, *dimension, *mode)?;
            }
            ReplayAction::RebindExternalBinding {
                expected,
                binding,
                expected_kind,
                expected_topology,
            } => {
                self.rebind_external_binding(
                    *expected,
                    *binding,
                    *expected_kind,
                    *expected_topology,
                )?;
            }
            ReplayAction::Delete {
                expected,
                selection,
            } => {
                self.editor.set_selection(selection.iter().copied());
                self.delete_selected(*expected)?;
            }
            ReplayAction::SetSuppressed {
                expected,
                selection,
                suppressed,
            } => {
                self.editor.set_selection(selection.iter().copied());
                self.set_selected_suppressed(*expected, *suppressed)?;
            }
            ReplayAction::Reattempt { expected } => {
                self.reattempt(*expected)?;
            }
            ReplayAction::AlternateBranch {
                expected,
                point,
                position,
                branches,
            } => {
                let mut candidate_session = self.session.clone();
                candidate_session
                    .attempt_point_and_curve_branches(*expected, *point, *position, branches)?;
                let next_checkpoint = checkpoint(&candidate_session)?;
                self.session = candidate_session;
                self.record_prepared_mutation(next_checkpoint, action.clone());
            }
            ReplayAction::Undo => self.undo()?,
            ReplayAction::Redo => self.redo()?,
            ReplayAction::ConstraintAction { .. }
            | ReplayAction::DimensionAction { .. }
            | ReplayAction::SetContactBranches { .. }
            | ReplayAction::SetAngleOrientation { .. } => {
                unreachable!("M55 replay actions were handled above")
            }
        }
        Ok(())
    }

    fn replay_m55_action(&mut self, action: &ReplayAction) -> Result<bool, CoordinatorError> {
        match action {
            ReplayAction::ConstraintAction {
                expected,
                selection,
                request,
            } => {
                self.editor.set_selection(selection.iter().copied());
                self.apply_constraint_action(*expected, request.clone())?;
            }
            ReplayAction::DimensionAction {
                expected,
                selection,
                request,
            } => {
                self.editor.set_selection(selection.iter().copied());
                self.apply_dimension_action(*expected, request.clone())?;
            }
            ReplayAction::SetContactBranches {
                expected,
                selection,
                edits,
            } => {
                self.editor.set_selection(selection.iter().copied());
                self.set_contact_branches(*expected, edits.clone())?;
            }
            ReplayAction::SetAngleOrientation {
                expected,
                dimension,
                orientation,
            } => {
                self.editor
                    .set_selection([SelectionItem::Dimension(*dimension)]);
                self.set_selected_angle_orientation(*expected, *orientation)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Restores the prior retained checkpoint with fresh lifecycle revisions.
    ///
    /// # Errors
    ///
    /// Returns [`CoordinatorError::NothingToUndo`] or a restore error.
    pub fn undo(&mut self) -> Result<(), CoordinatorError> {
        let target = self
            .history_cursor
            .checked_sub(1)
            .ok_or(CoordinatorError::NothingToUndo)?;
        self.restore_history(target)?;
        self.transcript.push(ReplayAction::Undo);
        Ok(())
    }

    /// Restores the next retained checkpoint with fresh lifecycle revisions.
    ///
    /// # Errors
    ///
    /// Returns [`CoordinatorError::NothingToRedo`] or a restore error.
    pub fn redo(&mut self) -> Result<(), CoordinatorError> {
        let target = self.history_cursor + 1;
        if target >= self.history.len() {
            return Err(CoordinatorError::NothingToRedo);
        }
        self.restore_history(target)?;
        self.transcript.push(ReplayAction::Redo);
        Ok(())
    }

    fn restore_history(&mut self, target: usize) -> Result<(), CoordinatorError> {
        let checkpoint = &self.history[target];
        let design =
            checkpoint_document_from_json(&checkpoint.design_json, checkpoint.design_is_draft_v5)?;
        let input = self.session.last_attempt().input();
        let request = input
            .candidate_request()
            .without_temporary_targets()
            .without_previous_state_preferences();
        let revisions = self.session.revision_high_water();
        let restored = if let Some(json) = &checkpoint.accepted_json {
            let accepted = checkpoint_document_from_json(json, checkpoint.accepted_is_draft_v5)?;
            RetainedSketchDocumentSession::restore_design_with_accepted(
                design,
                accepted,
                revisions,
                request,
                input.solver_config(),
            )?
        } else {
            RetainedSketchDocumentSession::restore_design(
                design,
                revisions,
                request,
                input.solver_config(),
            )?
        };
        self.session = restored;
        self.history_cursor = target;
        self.clear_transient();
        self.reconcile_selection();
        Ok(())
    }

    fn record_mutation(&mut self, replay: ReplayAction) -> Result<(), CoordinatorError> {
        let next = checkpoint(&self.session)?;
        self.record_prepared_mutation(next, replay);
        Ok(())
    }

    fn record_prepared_mutation(&mut self, next: RestoreCheckpoint, replay: ReplayAction) {
        self.history.truncate(self.history_cursor + 1);
        self.history.push(next);
        self.history_cursor += 1;
        self.transcript.push(replay);
        self.clear_transient();
        self.reconcile_selection();
    }

    fn reconcile_selection(&mut self) {
        let document = self.session.design_document();
        let retained = self
            .editor
            .selection()
            .iter()
            .copied()
            .filter(|item| selection_exists(document, *item))
            .collect::<Vec<_>>();
        self.editor.set_selection(retained);
    }

    fn ensure_expected(&self, expected: SketchDesignIdentity) -> Result<(), CoordinatorError> {
        let actual = self.session.design_identity();
        if expected == actual {
            Ok(())
        } else {
            Err(DocumentSessionError::StaleDesign { expected, actual }.into())
        }
    }
}

const fn failure_category(kind: SketchAttemptFailureKind) -> EditorProblemCategory {
    match kind {
        SketchAttemptFailureKind::ParameterInput
        | SketchAttemptFailureKind::ExternalSnapshotInput => EditorProblemCategory::Input,
        SketchAttemptFailureKind::Lowering => EditorProblemCategory::Lowering,
        SketchAttemptFailureKind::Request | SketchAttemptFailureKind::Solve => {
            EditorProblemCategory::Solver
        }
        SketchAttemptFailureKind::Publication => EditorProblemCategory::Publication,
        _ => EditorProblemCategory::Validation,
    }
}

const fn rejection_category(rejection: &SolveRejection) -> EditorProblemCategory {
    match rejection {
        SolveRejection::CoreTermination(_) | SolveRejection::HardResidual { .. } => {
            EditorProblemCategory::Solver
        }
        SolveRejection::SegmentBranchFlipped(_)
        | SolveRejection::NonPositiveCircleRadius(_)
        | SolveRejection::NonPositiveArcRadius(_)
        | SolveRejection::DegenerateSegment(_)
        | SolveRejection::InvalidConicEntity(_)
        | SolveRejection::InvalidNurbsEntity { .. } => EditorProblemCategory::Geometry,
        SolveRejection::DegenerateCurve(_)
        | SolveRejection::NurbsEvaluation { .. }
        | SolveRejection::IndependentConstraintResidual { .. }
        | SolveRejection::InvalidFilletGeometry(_)
        | SolveRejection::FilletSideFlipped(_)
        | SolveRejection::ContactParameterOutOfDomain(_)
        | SolveRejection::AmbiguousContactNeighborhood(_)
        | SolveRejection::LineSideFlipped(_)
        | SolveRejection::InvalidTangencyMode(_)
        | SolveRejection::AmbiguousTangencyScale(_)
        | SolveRejection::CenterDirectionFlipped(_) => EditorProblemCategory::Constraint,
        SolveRejection::IndependentDimensionResidual { .. }
        | SolveRejection::LineOffsetBranchFlipped(_) => EditorProblemCategory::Dimension,
        SolveRejection::BoundViolation(_) => EditorProblemCategory::Bound,
        _ => EditorProblemCategory::Validation,
    }
}

#[allow(clippy::too_many_lines)]
fn rejection_message(rejection: &SolveRejection) -> String {
    match rejection {
        SolveRejection::CoreTermination(_) => {
            "Solver stopped before producing an acceptable validated result.".into()
        }
        SolveRejection::HardResidual { maximum, tolerance } => format!(
            "Hard residual validation failed: maximum {maximum:.3e}, tolerance {tolerance:.3e}."
        ),
        SolveRejection::IndependentValidationFailed(message) => {
            format!("Independent validation failed: {message}")
        }
        SolveRejection::SegmentBranchFlipped(_) => {
            "A line segment crossed its retained branch.".into()
        }
        SolveRejection::NonPositiveCircleRadius(_) => {
            "A circle radius was not positive after solving.".into()
        }
        SolveRejection::NonPositiveArcRadius(_) => {
            "An arc radius was not positive after solving.".into()
        }
        SolveRejection::DegenerateSegment(_) => "A line segment became degenerate.".into(),
        SolveRejection::DegenerateCurve(_) => "A constrained curve became degenerate.".into(),
        SolveRejection::InvalidConicEntity(_) => "A conic became invalid after solving.".into(),
        SolveRejection::InvalidNurbsEntity { source, .. } => {
            format!("A NURBS definition became invalid: {source}")
        }
        SolveRejection::NurbsEvaluation { source, .. } => {
            format!("A constrained NURBS could not be evaluated: {source}")
        }
        SolveRejection::IndependentConstraintResidual {
            maximum, tolerance, ..
        } => format!(
            "Independent constraint validation failed: maximum {maximum:.3e}, tolerance {tolerance:.3e}."
        ),
        SolveRejection::IndependentDimensionResidual {
            maximum, tolerance, ..
        } => format!(
            "Independent dimension validation failed: maximum {maximum:.3e}, tolerance {tolerance:.3e}."
        ),
        SolveRejection::LineOffsetBranchFlipped(_) => {
            "A line-offset dimension crossed its retained orientation branch.".into()
        }
        SolveRejection::InvalidFilletGeometry(_) => {
            "A fillet no longer has valid derived geometry.".into()
        }
        SolveRejection::FilletSideFlipped(_) => "A fillet crossed its retained side.".into(),
        SolveRejection::ContactParameterOutOfDomain(_) => {
            "A constrained contact left its permitted curve interval.".into()
        }
        SolveRejection::AmbiguousContactNeighborhood(_) => {
            "A constrained contact neighborhood became ambiguous.".into()
        }
        SolveRejection::LineSideFlipped(_) => "A line contact crossed its retained side.".into(),
        SolveRejection::InvalidTangencyMode(_) => {
            "A tangency no longer satisfies its retained mode.".into()
        }
        SolveRejection::AmbiguousTangencyScale(_) => "A tangency scale became ambiguous.".into(),
        SolveRejection::CenterDirectionFlipped(_) => {
            "A center/contact direction crossed its retained branch.".into()
        }
        SolveRejection::BoundViolation(_) => {
            "A solved coordinate violated its retained bound.".into()
        }
        _ => "Independent validation rejected the attempted design.".into(),
    }
}

fn insert_source_owner(
    elements: &mut BTreeSet<DocumentElementId>,
    document: &SketchDocument,
    source: DocumentSourceId,
) {
    let Some(source) = document.source(source) else {
        return;
    };
    elements.insert(match source.owner {
        DocumentSourceOwner::Constraint(id) => DocumentElementId::Constraint(id),
        DocumentSourceOwner::Dimension(id) => DocumentElementId::Dimension(id),
    });
}

fn insert_runtime_source(
    elements: &mut BTreeSet<DocumentElementId>,
    attempt: &geosolve_sketch::SketchDocumentAttempt,
    document: &SketchDocument,
    source: SketchSource,
) {
    if let Some(source) = attempt.persistent_source(source) {
        insert_source_owner(elements, document, source);
    }
}

#[allow(clippy::too_many_lines)]
fn insert_rejection_elements(
    elements: &mut BTreeSet<DocumentElementId>,
    attempt: &geosolve_sketch::SketchDocumentAttempt,
    document: &SketchDocument,
    rejection: &SolveRejection,
) {
    match rejection {
        SolveRejection::SegmentBranchFlipped(id) | SolveRejection::DegenerateSegment(id) => {
            insert_runtime_curve(elements, attempt, |curve| match curve {
                RuntimeCurve::Line(candidate) => candidate == id,
                RuntimeCurve::Polyline(segments) => segments.contains(id),
                _ => false,
            });
        }
        SolveRejection::NonPositiveCircleRadius(id) => {
            insert_runtime_curve(
                elements,
                attempt,
                |curve| matches!(curve, RuntimeCurve::Circle(candidate) if candidate == id),
            );
        }
        SolveRejection::NonPositiveArcRadius(id) => {
            insert_runtime_curve(
                elements,
                attempt,
                |curve| matches!(curve, RuntimeCurve::CircularArc(candidate) if candidate == id),
            );
        }
        SolveRejection::InvalidConicEntity(id) => {
            insert_runtime_curve(
                elements,
                attempt,
                |curve| matches!(curve, RuntimeCurve::Conic(candidate) if candidate == id),
            );
        }
        SolveRejection::InvalidNurbsEntity { nurbs, .. } => {
            insert_runtime_curve(
                elements,
                attempt,
                |curve| matches!(curve, RuntimeCurve::Nurbs { nurbs: candidate, .. } if candidate == nurbs),
            );
        }
        SolveRejection::NurbsEvaluation {
            constraint, nurbs, ..
        } => {
            insert_runtime_source(
                elements,
                attempt,
                document,
                SketchSource::Constraint(*constraint),
            );
            insert_runtime_curve(
                elements,
                attempt,
                |curve| matches!(curve, RuntimeCurve::Nurbs { nurbs: candidate, .. } if candidate == nurbs),
            );
        }
        SolveRejection::DegenerateCurve(id)
        | SolveRejection::InvalidFilletGeometry(id)
        | SolveRejection::FilletSideFlipped(id)
        | SolveRejection::ContactParameterOutOfDomain(id)
        | SolveRejection::AmbiguousContactNeighborhood(id)
        | SolveRejection::LineSideFlipped(id)
        | SolveRejection::InvalidTangencyMode(id)
        | SolveRejection::AmbiguousTangencyScale(id)
        | SolveRejection::CenterDirectionFlipped(id)
        | SolveRejection::IndependentConstraintResidual { constraint: id, .. } => {
            insert_runtime_source(elements, attempt, document, SketchSource::Constraint(*id));
        }
        SolveRejection::LineOffsetBranchFlipped(id)
        | SolveRejection::IndependentDimensionResidual { dimension: id, .. } => {
            insert_runtime_source(elements, attempt, document, SketchSource::Dimension(*id));
        }
        SolveRejection::BoundViolation(bound) => {
            let mapping = attempt.solve_result().and_then(|solve| {
                solve
                    .bound_mappings
                    .iter()
                    .find(|mapping| mapping.bound_id == *bound)
            });
            if let Some(mapping) = mapping {
                match mapping.bound {
                    SketchBound::CircleRadius(id) => {
                        insert_runtime_curve(
                            elements,
                            attempt,
                            |curve| matches!(curve, RuntimeCurve::Circle(candidate) if *candidate == id),
                        );
                    }
                    SketchBound::ArcRadius(id) => {
                        insert_runtime_curve(
                            elements,
                            attempt,
                            |curve| matches!(curve, RuntimeCurve::CircularArc(candidate) if *candidate == id),
                        );
                    }
                    SketchBound::ConicScalar { conic_id, .. } => {
                        insert_runtime_curve(
                            elements,
                            attempt,
                            |curve| matches!(curve, RuntimeCurve::Conic(candidate) if *candidate == conic_id),
                        );
                    }
                    SketchBound::NurbsWeight { nurbs_id, .. } => {
                        insert_runtime_curve(
                            elements,
                            attempt,
                            |curve| matches!(curve, RuntimeCurve::Nurbs { nurbs, .. } if *nurbs == nurbs_id),
                        );
                    }
                    SketchBound::Contact { constraint_id, .. } => insert_runtime_source(
                        elements,
                        attempt,
                        document,
                        SketchSource::Constraint(constraint_id),
                    ),
                }
            }
        }
        _ => {}
    }
}

fn insert_runtime_curve(
    elements: &mut BTreeSet<DocumentElementId>,
    attempt: &geosolve_sketch::SketchDocumentAttempt,
    matches: impl Fn(&RuntimeCurve) -> bool,
) {
    let Some(mappings) = attempt.mappings() else {
        return;
    };
    for mapping in mappings.curve_mappings() {
        if matches(&mapping.runtime) {
            elements.insert(DocumentElementId::Curve(mapping.persistent));
        }
    }
}

const fn problem_target(element: DocumentElementId) -> Option<EditorProblemTarget> {
    match element {
        DocumentElementId::Point(id) => Some(EditorProblemTarget::Point(id)),
        DocumentElementId::Curve(id) => Some(EditorProblemTarget::Curve(id)),
        DocumentElementId::Constraint(id) => Some(EditorProblemTarget::Constraint(id)),
        DocumentElementId::Dimension(id) => Some(EditorProblemTarget::Dimension(id)),
        _ => None,
    }
}

impl ReplayAction {
    const fn expected_design(&self) -> Option<SketchDesignIdentity> {
        match self {
            Self::Edit { expected, .. }
            | Self::Construction { expected, .. }
            | Self::ConstraintAction { expected, .. }
            | Self::DimensionAction { expected, .. }
            | Self::PointDistance { expected, .. }
            | Self::SegmentLength { expected, .. }
            | Self::SetDimensionMode { expected, .. }
            | Self::SetContactBranches { expected, .. }
            | Self::SetAngleOrientation { expected, .. }
            | Self::RebindExternalBinding { expected, .. }
            | Self::Delete { expected, .. }
            | Self::SetSuppressed { expected, .. }
            | Self::Reattempt { expected }
            | Self::AlternateBranch { expected, .. } => Some(*expected),
            Self::Undo | Self::Redo => None,
        }
    }
}

fn mutation_from<T: Clone>(
    outcome: &geosolve_sketch::RetainedDocumentTransactionOutcome<T>,
) -> MutationOutcome<T> {
    MutationOutcome {
        value: outcome.value().clone(),
        design: outcome.design_identity(),
        attempt: outcome.attempt_identity(),
        published_accepted: outcome.published_accepted_identity(),
    }
}

fn alternate_branch_search_result(
    status: AlternateBranchSearchStatus,
    evidence: &AlternateBranchSearchEvidence,
    proposal: Option<AlternateBranchProposal>,
) -> AlternateBranchSearchResult {
    AlternateBranchSearchResult {
        status,
        proposal,
        evidence: *evidence,
    }
}

fn same_known_equality_degrees_of_freedom(base: Option<usize>, candidate: Option<usize>) -> bool {
    matches!((base, candidate), (Some(base), Some(candidate)) if base == candidate)
}

fn canonical_branch_search_directions() -> [[f64; 2]; 8] {
    let diagonal = 0.5_f64.sqrt();
    [
        [1.0, 0.0],
        [diagonal, diagonal],
        [0.0, 1.0],
        [-diagonal, diagonal],
        [-1.0, 0.0],
        [-diagonal, -diagonal],
        [0.0, -1.0],
        [diagonal, -diagonal],
    ]
}

fn line_segments(document: &SketchDocument) -> Vec<(CurveSpan, DesignPointId, DesignPointId)> {
    let mut spans = Vec::new();
    for curve in document.curves() {
        match &curve.definition {
            CurveDefinition::Line { start, end, .. } => {
                spans.push((CurveSpan::line(curve.id), *start, *end));
            }
            CurveDefinition::Polyline { points, closed, .. } => {
                for (segment, pair) in points.windows(2).enumerate() {
                    spans.push((
                        CurveSpan {
                            curve: curve.id,
                            segment: u32::try_from(segment).unwrap_or(u32::MAX),
                        },
                        pair[0],
                        pair[1],
                    ));
                }
                if *closed
                    && points.len() > 2
                    && let (Some(&start), Some(&end)) = (points.last(), points.first())
                {
                    spans.push((
                        CurveSpan {
                            curve: curve.id,
                            segment: u32::try_from(points.len() - 1).unwrap_or(u32::MAX),
                        },
                        start,
                        end,
                    ));
                }
            }
            _ => {}
        }
    }
    spans
}

fn incident_line_scale(document: &SketchDocument, point: DesignPointId) -> Option<f64> {
    let lengths = line_segments(document)
        .into_iter()
        .filter_map(|(_, start, end)| {
            (start == point || end == point).then(|| {
                let first = document.point(start)?.position;
                let second = document.point(end)?.position;
                let length = (second[0] - first[0]).hypot(second[1] - first[1]);
                (length.is_finite() && length > 0.0).then_some(length)
            })?
        })
        .collect::<Vec<_>>();
    (!lengths.is_empty())
        .then(|| {
            let count = f64::from(u32::try_from(lengths.len()).unwrap_or(u32::MAX));
            lengths.iter().sum::<f64>() / count
        })
        .filter(|scale| scale.is_finite() && *scale > 0.0)
}

fn incident_branch_edits(
    design: &SketchDocument,
    accepted: &SketchDocument,
    point: DesignPointId,
    target: [f64; 2],
) -> Option<(Vec<DocumentCurveBranchEdit>, Vec<CurveSpan>)> {
    if !target.iter().all(|value| value.is_finite()) {
        return None;
    }
    let mut branches = Vec::new();
    let mut signature = Vec::new();
    for (span, start, end) in line_segments(design) {
        if start != point && end != point {
            continue;
        }
        let start_position = if start == point {
            target
        } else {
            accepted.point(start)?.position
        };
        let end_position = if end == point {
            target
        } else {
            accepted.point(end)?.position
        };
        let delta = [
            end_position[0] - start_position[0],
            end_position[1] - start_position[1],
        ];
        let norm = delta[0].hypot(delta[1]);
        if !norm.is_finite() || norm <= f64::EPSILON {
            return None;
        }
        let direction = [delta[0] / norm, delta[1] / norm];
        let old = design.curve_branch_direction(span)?;
        let branch_dot = old[0] * direction[0] + old[1] * direction[1];
        let branch_direction = if branch_dot < 0.0 {
            signature.push(span);
            let separator_delta = [direction[0] - old[0], direction[1] - old[1]];
            let separator_norm = separator_delta[0].hypot(separator_delta[1]);
            if !separator_norm.is_finite() || separator_norm <= f64::EPSILON {
                return None;
            }
            let separator = [
                separator_delta[0] / separator_norm,
                separator_delta[1] / separator_norm,
            ];
            if direction[0] * separator[0] + direction[1] * separator[1] <= 0.0
                || old[0] * separator[0] + old[1] * separator[1] >= 0.0
            {
                return None;
            }
            separator
        } else if branch_dot > 0.0 {
            old
        } else {
            return None;
        };
        branches.push(DocumentCurveBranchEdit {
            curve: span,
            direction: branch_direction,
        });
    }
    (!branches.is_empty() && !signature.is_empty()).then_some((branches, signature))
}

fn checkpoint(
    session: &RetainedSketchDocumentSession,
) -> Result<RestoreCheckpoint, geosolve_sketch::DocumentError> {
    let (design_json, design_is_draft_v5) = checkpoint_document_to_json(session.design_document())?;
    let (accepted_json, accepted_is_draft_v5) = session.accepted_state().map_or_else(
        || Ok((None, false)),
        |accepted| {
            checkpoint_document_to_json(accepted.document())
                .map(|(json, is_draft)| (Some(json), is_draft))
        },
    )?;
    Ok(RestoreCheckpoint {
        design_json,
        design_is_draft_v5,
        accepted_json,
        accepted_is_draft_v5,
        revisions: session.revision_high_water(),
    })
}

fn checkpoint_document_to_json(
    document: &SketchDocument,
) -> Result<(String, bool), geosolve_sketch::DocumentError> {
    match document.to_canonical_json() {
        Ok(json) => Ok((json, false)),
        Err(_) => document.to_draft_v5_json().map(|json| (json, true)),
    }
}

fn checkpoint_documents_have_exact_bytes(
    first: &SketchDocument,
    second: &SketchDocument,
) -> Result<bool, geosolve_sketch::DocumentError> {
    let (first_json, first_is_draft_v5) = checkpoint_document_to_json(first)?;
    let (second_json, second_is_draft_v5) = checkpoint_document_to_json(second)?;
    Ok(first_is_draft_v5 == second_is_draft_v5 && first_json.as_bytes() == second_json.as_bytes())
}

fn checkpoint_document_from_json(
    json: &str,
    is_draft_v5: bool,
) -> Result<SketchDocument, geosolve_sketch::DocumentError> {
    if is_draft_v5 {
        SketchDocument::from_draft_v5_json(json)
    } else {
        SketchDocument::from_json(json)
    }
}

fn availability<T>(result: Result<T, DisabledReason>) -> ActionState {
    result.map_or_else(ActionState::Disabled, |_| ActionState::Enabled)
}

fn constraint_action_matrix(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Vec<ActionAvailability> {
    [
        ConstraintIntent::Lock,
        ConstraintIntent::Coincident,
        ConstraintIntent::Horizontal,
        ConstraintIntent::Vertical,
        ConstraintIntent::Parallel,
        ConstraintIntent::Perpendicular,
        ConstraintIntent::Equal,
        ConstraintIntent::Midpoint,
        ConstraintIntent::Symmetric,
        ConstraintIntent::Tangent,
        ConstraintIntent::Continuity,
    ]
    .into_iter()
    .map(|intent| ActionAvailability {
        action: CoordinatorActionKind::Constraint(intent),
        state: resolve_constraint(document, selection, intent)
            .map_or_else(ActionState::Disabled, |_| ActionState::Enabled),
    })
    .collect()
}

fn dimension_action_matrix(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Vec<ActionAvailability> {
    let mut actions = Vec::new();
    for mode in [
        DocumentDimensionMode::Driving,
        DocumentDimensionMode::Reference,
    ] {
        for kind in [
            DimensionKind::PointDistance,
            DimensionKind::SegmentLength,
            DimensionKind::Radius,
            DimensionKind::Diameter,
            DimensionKind::OrientedAngle,
        ] {
            actions.push(ActionAvailability {
                action: CoordinatorActionKind::Dimension(kind, mode),
                state: availability(dimension_target(
                    document,
                    selection,
                    kind,
                    DocumentAngleOrientation::CounterClockwise,
                )),
            });
        }
        actions.push(ActionAvailability {
            action: CoordinatorActionKind::SetDimensionMode(mode),
            state: dimension_mode_availability(document, selection, mode),
        });
    }
    actions
}

fn selection_reason(document: &SketchDocument, selection: &[SelectionItem]) -> DisabledReason {
    if selection.is_empty() {
        DisabledReason::EmptySelection
    } else if selection
        .iter()
        .any(|item| !selection_exists(document, *item))
    {
        DisabledReason::MissingObject
    } else {
        DisabledReason::WrongOperandKind
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed intent-to-definition dispatch matrix is clearer as one exhaustive match"
)]
pub(crate) fn resolve_constraint(
    document: &SketchDocument,
    selection: &[SelectionItem],
    intent: ConstraintIntent,
) -> Result<ResolvedConstraintKind, DisabledReason> {
    if selection.is_empty() {
        return Err(DisabledReason::EmptySelection);
    }
    if selection
        .iter()
        .any(|item| !selection_exists(document, *item))
    {
        return Err(DisabledReason::MissingObject);
    }
    let resolved = match (intent, selection) {
        (ConstraintIntent::Lock, [SelectionItem::Point(_)]) => ResolvedConstraintKind::FixedPoint,
        (ConstraintIntent::Coincident, [SelectionItem::Point(_), SelectionItem::Point(_)]) => {
            ResolvedConstraintKind::CoincidentPoints
        }
        (
            ConstraintIntent::Coincident,
            [SelectionItem::Point(_), SelectionItem::Curve(span)]
            | [SelectionItem::Curve(span), SelectionItem::Point(_)],
        ) if supports_curve_contact(document, *span) => ResolvedConstraintKind::PointOnCurve,
        (
            ConstraintIntent::Coincident,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if supports_curve_contact(document, *first)
            && supports_curve_contact(document, *second) =>
        {
            ResolvedConstraintKind::CurveContact
        }
        (ConstraintIntent::Horizontal, [SelectionItem::Curve(span)])
            if line_endpoints(document, *span).is_ok() =>
        {
            ResolvedConstraintKind::HorizontalLine
        }
        (ConstraintIntent::Vertical, [SelectionItem::Curve(span)])
            if line_endpoints(document, *span).is_ok() =>
        {
            ResolvedConstraintKind::VerticalLine
        }
        (
            ConstraintIntent::Parallel,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if line_endpoints(document, *first).is_ok()
            && line_endpoints(document, *second).is_ok() =>
        {
            ResolvedConstraintKind::ParallelLines
        }
        (
            ConstraintIntent::Perpendicular,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if line_endpoints(document, *first).is_ok()
            && line_endpoints(document, *second).is_ok() =>
        {
            ResolvedConstraintKind::PerpendicularLines
        }
        (ConstraintIntent::Perpendicular, _)
            if selected_radial_line(document, selection).is_some() =>
        {
            ResolvedConstraintKind::RadialLine
        }
        (ConstraintIntent::Equal, [SelectionItem::Curve(first), SelectionItem::Curve(second)])
            if line_endpoints(document, *first).is_ok()
                && line_endpoints(document, *second).is_ok() =>
        {
            ResolvedConstraintKind::EqualLength
        }
        (ConstraintIntent::Equal, [SelectionItem::Curve(first), SelectionItem::Curve(second)])
            if is_radius_curve(document, first.curve)
                && is_radius_curve(document, second.curve) =>
        {
            ResolvedConstraintKind::EqualRadius
        }
        (ConstraintIntent::Equal, [SelectionItem::Curve(first), SelectionItem::Curve(second)])
            if supports_curve_contact(document, *first)
                && supports_curve_contact(document, *second) =>
        {
            ResolvedConstraintKind::EqualCurvature
        }
        (
            ConstraintIntent::Midpoint,
            [SelectionItem::Point(_), SelectionItem::Curve(line)]
            | [SelectionItem::Curve(line), SelectionItem::Point(_)],
        ) if line_endpoints(document, *line).is_ok() => ResolvedConstraintKind::Midpoint,
        (
            ConstraintIntent::Symmetric,
            [
                SelectionItem::Point(_),
                SelectionItem::Point(_),
                SelectionItem::Curve(line),
            ],
        ) if line_endpoints(document, *line).is_ok() => ResolvedConstraintKind::SymmetricAboutLine,
        (
            ConstraintIntent::Tangent,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if supports_curve_contact(document, *first)
            && supports_curve_contact(document, *second) =>
        {
            ResolvedConstraintKind::CurveTangency
        }
        (
            ConstraintIntent::Continuity,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if supports_endpoint_contact(document, *first)
            && supports_endpoint_contact(document, *second) =>
        {
            ResolvedConstraintKind::EndpointContinuity
        }
        _ => {
            let expected = match intent {
                ConstraintIntent::Lock
                | ConstraintIntent::Horizontal
                | ConstraintIntent::Vertical => 1,
                ConstraintIntent::Coincident
                | ConstraintIntent::Parallel
                | ConstraintIntent::Perpendicular
                | ConstraintIntent::Equal
                | ConstraintIntent::Midpoint
                | ConstraintIntent::Tangent
                | ConstraintIntent::Continuity => 2,
                ConstraintIntent::Symmetric => 3,
            };
            return Err(if selection.len() == expected {
                DisabledReason::WrongOperandKind
            } else {
                DisabledReason::WrongArity
            });
        }
    };
    Ok(resolved)
}

pub(crate) fn selection_exists(document: &SketchDocument, item: SelectionItem) -> bool {
    match item {
        SelectionItem::Point(id) => document.point(id).is_some(),
        SelectionItem::Curve(span) => document
            .curve_spans(span.curve)
            .is_ok_and(|spans| spans.contains(&span)),
        SelectionItem::Constraint(id) => document.constraints().iter().any(|value| value.id == id),
        SelectionItem::Dimension(id) => document.dimensions().iter().any(|value| value.id == id),
    }
}

fn point_distance_target(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Result<f64, DisabledReason> {
    let [SelectionItem::Point(first), SelectionItem::Point(second)] = selection else {
        return Err(if selection.len() == 2 {
            DisabledReason::WrongOperandKind
        } else {
            DisabledReason::WrongArity
        });
    };
    let first = document
        .point(*first)
        .ok_or(DisabledReason::MissingObject)?
        .position;
    let second = document
        .point(*second)
        .ok_or(DisabledReason::MissingObject)?
        .position;
    let value = (second[0] - first[0]).hypot(second[1] - first[1]);
    (value > 0.0 && value.is_finite())
        .then_some(value)
        .ok_or(DisabledReason::WrongOperandKind)
}

fn segment_length_target(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Result<f64, DisabledReason> {
    let [SelectionItem::Curve(span)] = selection else {
        return Err(if selection.len() == 1 {
            DisabledReason::WrongOperandKind
        } else {
            DisabledReason::WrongArity
        });
    };
    let (first, second) = line_endpoints(document, *span)?;
    let first = document
        .point(first)
        .ok_or(DisabledReason::MissingObject)?
        .position;
    let second = document
        .point(second)
        .ok_or(DisabledReason::MissingObject)?
        .position;
    let value = (second[0] - first[0]).hypot(second[1] - first[1]);
    (value > 0.0 && value.is_finite())
        .then_some(value)
        .ok_or(DisabledReason::WrongOperandKind)
}

fn line_endpoints(
    document: &SketchDocument,
    span: CurveSpan,
) -> Result<(DesignPointId, DesignPointId), DisabledReason> {
    let curve = document
        .curve(span.curve)
        .ok_or(DisabledReason::MissingObject)?;
    match &curve.definition {
        CurveDefinition::Line { start, end, .. } if span.segment == 0 => Ok((*start, *end)),
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = usize::try_from(span.segment).map_err(|_| DisabledReason::InvalidSpan)?;
            if index + 1 < points.len() {
                Ok((points[index], points[index + 1]))
            } else if *closed && index + 1 == points.len() {
                Ok((points[index], points[0]))
            } else {
                Err(DisabledReason::InvalidSpan)
            }
        }
        _ => Err(DisabledReason::WrongOperandKind),
    }
}

pub(crate) fn dimension_target(
    document: &SketchDocument,
    selection: &[SelectionItem],
    kind: DimensionKind,
    orientation: DocumentAngleOrientation,
) -> Result<f64, DisabledReason> {
    match kind {
        DimensionKind::PointDistance => point_distance_target(document, selection),
        DimensionKind::SegmentLength => segment_length_target(document, selection),
        DimensionKind::Radius | DimensionKind::Diameter => {
            let [SelectionItem::Curve(span)] = selection else {
                return Err(if selection.len() == 1 {
                    DisabledReason::WrongOperandKind
                } else {
                    DisabledReason::WrongArity
                });
            };
            let curve = document
                .curve(span.curve)
                .ok_or(DisabledReason::MissingObject)?;
            let radius = match curve.definition {
                CurveDefinition::Circle { radius, .. }
                | CurveDefinition::CircularArc { radius, .. } => {
                    document
                        .scalar(radius)
                        .ok_or(DisabledReason::MissingObject)?
                        .value
                }
                _ => return Err(DisabledReason::WrongOperandKind),
            };
            let value = if kind == DimensionKind::Diameter {
                radius * 2.0
            } else {
                radius
            };
            (value.is_finite() && value > 0.0)
                .then_some(value)
                .ok_or(DisabledReason::WrongOperandKind)
        }
        DimensionKind::OrientedAngle => {
            let [SelectionItem::Curve(first), SelectionItem::Curve(second)] = selection else {
                return Err(if selection.len() == 2 {
                    DisabledReason::WrongOperandKind
                } else {
                    DisabledReason::WrongArity
                });
            };
            if first == second {
                return Err(DisabledReason::WrongOperandKind);
            }
            let first = line_vector(document, *first)?;
            let second = line_vector(document, *second)?;
            let cross = first[0].mul_add(second[1], -first[1] * second[0]);
            let dot = first[0].mul_add(second[0], first[1] * second[1]);
            let signed = match orientation {
                DocumentAngleOrientation::CounterClockwise => cross.atan2(dot),
                DocumentAngleOrientation::Clockwise => (-cross).atan2(dot),
            };
            let value = signed.rem_euclid(std::f64::consts::TAU);
            (value.is_finite() && value > 0.0)
                .then_some(value)
                .ok_or(DisabledReason::WrongOperandKind)
        }
    }
}

pub(crate) fn validate_dimension_selection(
    document: &SketchDocument,
    selection: &[SelectionItem],
    kind: DimensionKind,
) -> Result<(), DisabledReason> {
    match (kind, selection) {
        (
            DimensionKind::PointDistance,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) => {
            if first == second {
                return Err(DisabledReason::WrongOperandKind);
            }
            document
                .point(*first)
                .ok_or(DisabledReason::MissingObject)?;
            document
                .point(*second)
                .ok_or(DisabledReason::MissingObject)?;
        }
        (DimensionKind::SegmentLength, [SelectionItem::Curve(span)]) => {
            let (first, second) = line_endpoints(document, *span)?;
            document.point(first).ok_or(DisabledReason::MissingObject)?;
            document
                .point(second)
                .ok_or(DisabledReason::MissingObject)?;
        }
        (DimensionKind::Radius | DimensionKind::Diameter, [SelectionItem::Curve(span)]) => {
            let curve = document
                .curve(span.curve)
                .ok_or(DisabledReason::MissingObject)?;
            let (CurveDefinition::Circle { radius, .. }
            | CurveDefinition::CircularArc { radius, .. }) = curve.definition
            else {
                return Err(DisabledReason::WrongOperandKind);
            };
            document
                .scalar(radius)
                .ok_or(DisabledReason::MissingObject)?;
        }
        (
            DimensionKind::OrientedAngle,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => {
            if first == second {
                return Err(DisabledReason::WrongOperandKind);
            }
            for span in [first, second] {
                let (start, end) = line_endpoints(document, *span)?;
                document.point(start).ok_or(DisabledReason::MissingObject)?;
                document.point(end).ok_or(DisabledReason::MissingObject)?;
            }
        }
        (_, values) => {
            let expected = match kind {
                DimensionKind::PointDistance | DimensionKind::OrientedAngle => 2,
                DimensionKind::SegmentLength | DimensionKind::Radius | DimensionKind::Diameter => 1,
            };
            return Err(if values.len() == expected {
                DisabledReason::WrongOperandKind
            } else {
                DisabledReason::WrongArity
            });
        }
    }
    Ok(())
}

const fn dimension_action_label(kind: DimensionKind) -> &'static str {
    match kind {
        DimensionKind::PointDistance => "Point distance",
        DimensionKind::SegmentLength => "Segment length",
        DimensionKind::Radius => "Radius",
        DimensionKind::Diameter => "Diameter",
        DimensionKind::OrientedAngle => "Oriented angle",
    }
}

const fn dimension_target_scalar(
    definition: &DocumentDimensionDefinition,
) -> geosolve_sketch::DesignScalarId {
    match definition {
        DocumentDimensionDefinition::PointDistance { target, .. }
        | DocumentDimensionDefinition::CurveLength { target, .. }
        | DocumentDimensionDefinition::Radius { target, .. }
        | DocumentDimensionDefinition::Diameter { target, .. }
        | DocumentDimensionDefinition::OrientedAngle { target, .. }
        | DocumentDimensionDefinition::SupportingLineOffset { target, .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. } => *target,
    }
}

fn storage_dimension_target(
    metadata: DimensionTargetMetadata,
    display_value: f64,
) -> Result<f64, CoordinatorError> {
    if !display_value.is_finite() {
        return Err(CoordinatorError::InvalidActionInput(
            "dimension target must be finite",
        ));
    }
    if metadata.display_unit == DimensionTargetDisplayUnit::ModelUnits {
        return Ok(display_value);
    }
    if display_value <= 0.0 || display_value > 90.0 {
        return Err(CoordinatorError::InvalidActionInput(
            "acute angle target must be greater than zero and at most 90 degrees",
        ));
    }

    let acute = display_value.to_radians();
    let principal = metadata.value.rem_euclid(std::f64::consts::TAU);
    let turns = metadata.value - principal;
    let branch_value = if principal <= std::f64::consts::FRAC_PI_2 {
        acute
    } else if principal <= std::f64::consts::PI {
        std::f64::consts::PI - acute
    } else if principal <= 3.0 * std::f64::consts::FRAC_PI_2 {
        std::f64::consts::PI + acute
    } else {
        std::f64::consts::TAU - acute
    };
    let value = turns + branch_value;
    (value.is_finite() && value > 0.0)
        .then_some(value)
        .ok_or(CoordinatorError::InvalidActionInput(
            "display target does not map to a positive finite solver target",
        ))
}

#[derive(Clone, Copy)]
enum DimensionOperands {
    PointDistance(DesignPointId, DesignPointId),
    CurveLength(CurveSpan),
    Radius(CurveId),
    Diameter(CurveId),
    OrientedAngle(CurveSpan, CurveSpan),
}

impl DimensionOperands {
    const fn definition(
        self,
        target: geosolve_sketch::DesignScalarId,
        orientation: DocumentAngleOrientation,
    ) -> DocumentDimensionDefinition {
        match self {
            Self::PointDistance(first, second) => DocumentDimensionDefinition::PointDistance {
                first,
                second,
                target,
            },
            Self::CurveLength(curve) => DocumentDimensionDefinition::CurveLength { curve, target },
            Self::Radius(curve) => DocumentDimensionDefinition::Radius { curve, target },
            Self::Diameter(curve) => DocumentDimensionDefinition::Diameter { curve, target },
            Self::OrientedAngle(first, second) => DocumentDimensionDefinition::OrientedAngle {
                first,
                second,
                target,
                orientation,
            },
        }
    }
}

fn dimension_operands(
    document: &SketchDocument,
    selection: &[SelectionItem],
    kind: DimensionKind,
) -> Result<DimensionOperands, CoordinatorError> {
    let operands = match (kind, selection) {
        (
            DimensionKind::PointDistance,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) => DimensionOperands::PointDistance(*first, *second),
        (DimensionKind::SegmentLength, [SelectionItem::Curve(curve)]) => {
            DimensionOperands::CurveLength(*curve)
        }
        (DimensionKind::Radius, [SelectionItem::Curve(curve)]) => {
            DimensionOperands::Radius(curve.curve)
        }
        (DimensionKind::Diameter, [SelectionItem::Curve(curve)]) => {
            DimensionOperands::Diameter(curve.curve)
        }
        (
            DimensionKind::OrientedAngle,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DimensionOperands::OrientedAngle(*first, *second),
        _ => return Err(CoordinatorError::IncompatibleDimension),
    };
    validate_dimension_selection(document, selection, kind)
        .map_err(CoordinatorError::ActionUnavailable)?;
    Ok(operands)
}

fn line_vector(document: &SketchDocument, span: CurveSpan) -> Result<[f64; 2], DisabledReason> {
    let (start, end) = line_endpoints(document, span)?;
    let start = document
        .point(start)
        .ok_or(DisabledReason::MissingObject)?
        .position;
    let end = document
        .point(end)
        .ok_or(DisabledReason::MissingObject)?
        .position;
    let vector = [end[0] - start[0], end[1] - start[1]];
    (vector.into_iter().all(f64::is_finite)
        && vector[0].mul_add(vector[0], vector[1] * vector[1]) > 0.0)
        .then_some(vector)
        .ok_or(DisabledReason::WrongOperandKind)
}

fn selected_curve_spans(selection: &[SelectionItem]) -> Vec<CurveSpan> {
    selection
        .iter()
        .filter_map(|item| match item {
            SelectionItem::Curve(span) => Some(*span),
            _ => None,
        })
        .collect()
}

fn selected_curve_pair(selection: &[SelectionItem]) -> Option<[CurveSpan; 2]> {
    let [SelectionItem::Curve(first), SelectionItem::Curve(second)] = selection else {
        return None;
    };
    Some([*first, *second])
}

fn selected_point_curve(selection: &[SelectionItem]) -> Option<(DesignPointId, CurveSpan)> {
    match selection {
        [SelectionItem::Point(point), SelectionItem::Curve(curve)]
        | [SelectionItem::Curve(curve), SelectionItem::Point(point)] => Some((*point, *curve)),
        _ => None,
    }
}

fn selected_radial_line(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Option<(CurveSpan, DesignPointId, u8)> {
    let [SelectionItem::Curve(first), SelectionItem::Curve(second)] = selection else {
        return None;
    };
    match (
        line_endpoints(document, *first),
        radial_center(document, *second),
    ) {
        (Ok(_), Some(center)) => Some((*first, center, 0)),
        _ if line_endpoints(document, *second).is_ok() => {
            radial_center(document, *first).map(|center| (*second, center, 1))
        }
        _ => None,
    }
}

fn radial_center(document: &SketchDocument, span: CurveSpan) -> Option<DesignPointId> {
    let curve = document.curve(span.curve)?;
    match &curve.definition {
        CurveDefinition::Circle { center, .. } | CurveDefinition::CircularArc { center, .. } => {
            Some(*center)
        }
        _ => None,
    }
}

fn supports_curve_contact(document: &SketchDocument, span: CurveSpan) -> bool {
    document.curve_contact_domains(span).is_ok()
}

fn supports_endpoint_contact(document: &SketchDocument, span: CurveSpan) -> bool {
    document.curve_contact_domains(span).is_ok_and(|domains| {
        domains
            .iter()
            .any(|domain| matches!(domain, ContactDomain::Bounded { .. }))
    })
}

fn is_radius_curve(document: &SketchDocument, curve: CurveId) -> bool {
    document.curve(curve).is_some_and(|curve| {
        matches!(
            curve.definition,
            CurveDefinition::Circle { .. } | CurveDefinition::CircularArc { .. }
        )
    })
}

const fn simple_constraint_kind(resolved: ResolvedConstraintKind) -> Option<ConstraintKind> {
    match resolved {
        ResolvedConstraintKind::FixedPoint => Some(ConstraintKind::Fixed),
        ResolvedConstraintKind::CoincidentPoints => Some(ConstraintKind::Coincident),
        ResolvedConstraintKind::HorizontalLine => Some(ConstraintKind::Horizontal),
        ResolvedConstraintKind::VerticalLine => Some(ConstraintKind::Vertical),
        ResolvedConstraintKind::ParallelLines => Some(ConstraintKind::Parallel),
        ResolvedConstraintKind::PerpendicularLines => Some(ConstraintKind::Perpendicular),
        ResolvedConstraintKind::EqualLength => Some(ConstraintKind::EqualLength),
        ResolvedConstraintKind::EqualRadius => Some(ConstraintKind::EqualRadius),
        ResolvedConstraintKind::Midpoint => Some(ConstraintKind::Midpoint),
        ResolvedConstraintKind::SymmetricAboutLine => Some(ConstraintKind::Symmetry),
        ResolvedConstraintKind::PointOnCurve
        | ResolvedConstraintKind::CurveContact
        | ResolvedConstraintKind::RadialLine
        | ResolvedConstraintKind::EqualCurvature
        | ResolvedConstraintKind::CurveTangency
        | ResolvedConstraintKind::EndpointContinuity => None,
    }
}

fn contact_action_choice(
    document: &SketchDocument,
    operand: u8,
    span: CurveSpan,
    tangency: bool,
    endpoint_only: bool,
    picked_parameter: Option<f64>,
) -> Option<ActionChoice> {
    let mut domains = document.curve_contact_domains(span).ok()?;
    if endpoint_only {
        domains.retain(|domain| matches!(domain, ContactDomain::Bounded { .. }));
    }
    let first = *domains.first()?;
    let semantic_default = match first {
        ContactDomain::Bounded { lower, upper: _ } if endpoint_only => lower,
        ContactDomain::Bounded { lower, upper } => (lower + upper) * 0.5,
        ContactDomain::SupportingLine | ContactDomain::Periodic { .. } => 0.0,
    };
    let default_parameter = picked_parameter
        .filter(|parameter| {
            if endpoint_only {
                matches!(
                    first,
                    ContactDomain::Bounded { lower, upper }
                        if parameter.to_bits() == lower.to_bits()
                            || parameter.to_bits() == upper.to_bits()
                )
            } else {
                parameter.is_finite() && contact_domain_contains(first, *parameter)
            }
        })
        .unwrap_or(semantic_default);
    let neighborhoods = if endpoint_only {
        match first {
            ContactDomain::Bounded { upper, .. }
                if default_parameter.to_bits() == upper.to_bits() =>
            {
                vec![ContactNeighborhood::End, ContactNeighborhood::Start]
            }
            ContactDomain::Bounded { lower, .. }
                if default_parameter.to_bits() == lower.to_bits() =>
            {
                vec![ContactNeighborhood::Start, ContactNeighborhood::End]
            }
            ContactDomain::Bounded { .. }
            | ContactDomain::SupportingLine
            | ContactDomain::Periodic { .. } => {
                unreachable!("endpoint-only contact defaults to a bounded endpoint")
            }
        }
    } else {
        contact_neighborhood_options(first, default_parameter)
    };
    Some(ActionChoice::Contact {
        operand,
        span,
        domains,
        default_parameter,
        neighborhoods,
        tangent_orientations: if tangency {
            vec![TangentOrientation::Aligned, TangentOrientation::Opposed]
        } else {
            Vec::new()
        },
        default_winding: 0,
    })
}

fn contact_domain_contains(domain: ContactDomain, parameter: f64) -> bool {
    match domain {
        ContactDomain::SupportingLine | ContactDomain::Periodic { .. } => parameter.is_finite(),
        ContactDomain::Bounded { lower, upper } => {
            parameter.is_finite() && parameter >= lower && parameter <= upper
        }
    }
}

fn validate_pair_relation_choice(
    resolved: ResolvedConstraintKind,
    relation: Option<ConstraintRelationChoice>,
) -> Result<(), CoordinatorError> {
    let valid = matches!(
        (resolved, relation),
        (
            ResolvedConstraintKind::CurveContact | ResolvedConstraintKind::CurveTangency,
            None
        ) | (
            ResolvedConstraintKind::EqualCurvature,
            Some(ConstraintRelationChoice::EqualCurvature(_))
        ) | (
            ResolvedConstraintKind::EndpointContinuity,
            Some(ConstraintRelationChoice::Continuity(_))
        )
    );
    if !valid {
        return Err(CoordinatorError::InvalidActionInput(
            "relation choice does not match the resolved curve-pair action",
        ));
    }
    if let Some(ConstraintRelationChoice::Continuity(DocumentCurveContinuity::ParametricC2 {
        first_rate,
        second_rate,
    })) = relation
        && (!first_rate.is_finite()
            || first_rate <= 0.0
            || !second_rate.is_finite()
            || second_rate <= 0.0)
    {
        return Err(CoordinatorError::InvalidActionInput(
            "parametric C2 rates must be finite and positive",
        ));
    }
    Ok(())
}

fn contact_neighborhood_options(domain: ContactDomain, value: f64) -> Vec<ContactNeighborhood> {
    match domain {
        ContactDomain::Bounded { lower, upper } => {
            let local = ContactNeighborhood::Local {
                lower: lower + (upper - lower) * 0.25,
                upper: lower + (upper - lower) * 0.75,
            };
            if value.to_bits() == lower.to_bits() {
                vec![
                    ContactNeighborhood::Start,
                    ContactNeighborhood::Interior,
                    local,
                    ContactNeighborhood::End,
                ]
            } else if value.to_bits() == upper.to_bits() {
                vec![
                    ContactNeighborhood::End,
                    ContactNeighborhood::Interior,
                    local,
                    ContactNeighborhood::Start,
                ]
            } else {
                vec![
                    ContactNeighborhood::Interior,
                    local,
                    ContactNeighborhood::Start,
                    ContactNeighborhood::End,
                ]
            }
        }
        ContactDomain::SupportingLine => vec![
            ContactNeighborhood::Interior,
            ContactNeighborhood::Local {
                lower: value - 0.5,
                upper: value + 0.5,
            },
        ],
        ContactDomain::Periodic { period } => vec![
            ContactNeighborhood::Interior,
            ContactNeighborhood::Local {
                lower: value - period * 0.25,
                upper: value + period * 0.25,
            },
        ],
    }
}

fn validate_contact_choice(
    selected_span: CurveSpan,
    choice: &crate::ContactActionChoice,
    tangency: bool,
) -> Result<(), CoordinatorError> {
    if choice.support.span != selected_span {
        return Err(CoordinatorError::InvalidActionInput(
            "contact span must match the selected semantic span",
        ));
    }
    if tangency != choice.tangent_orientation.is_some() {
        return Err(CoordinatorError::InvalidActionInput(
            "tangent orientation must be present only for tangency actions",
        ));
    }
    Ok(())
}

fn add_action_contact(
    document: &mut SketchDocument,
    label: &str,
    operand: u8,
    choice: crate::ContactActionChoice,
) -> Result<ContactId, geosolve_sketch::DocumentError> {
    document.add_curve_contact_with_domain(
        format!("{label} contact {}", usize::from(operand) + 1),
        choice.support.span,
        choice.domain,
        choice.parameter,
        choice.support.winding,
        choice.neighborhood,
        choice.tangent_orientation,
    )
}

fn selected_contact_ids(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Option<Vec<ContactId>> {
    let [SelectionItem::Constraint(id)] = selection else {
        return None;
    };
    let definition = &document
        .constraints()
        .iter()
        .find(|constraint| constraint.id == *id)?
        .definition;
    Some(match definition {
        DocumentConstraintDefinition::PointOnCurve { contact, .. }
        | DocumentConstraintDefinition::LineCurveTangency {
            curve_contact: contact,
            ..
        }
        | DocumentConstraintDefinition::CurveDirection {
            curve_contact: contact,
            ..
        } => vec![*contact],
        DocumentConstraintDefinition::LineCircleTangency {
            line_contact,
            circle_contact,
            ..
        } => vec![*line_contact, *circle_contact],
        DocumentConstraintDefinition::CircleArcTangency {
            circle_contact,
            arc_contact,
            ..
        } => vec![*circle_contact, *arc_contact],
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact,
            second_contact,
        }
        | DocumentConstraintDefinition::CurveCurveTangency {
            first_contact,
            second_contact,
        }
        | DocumentConstraintDefinition::EqualCurvature {
            first_contact,
            second_contact,
            ..
        }
        | DocumentConstraintDefinition::EndpointContinuity {
            first_contact,
            second_contact,
            ..
        }
        | DocumentConstraintDefinition::LineLineFillet {
            first_contact,
            second_contact,
            ..
        }
        | DocumentConstraintDefinition::CurveCurveFillet {
            first_contact,
            second_contact,
            ..
        } => vec![*first_contact, *second_contact],
        _ => return None,
    })
}

fn contact_branch_availability(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> ActionState {
    selected_contact_ids(document, selection).map_or_else(
        || ActionState::Disabled(selection_reason(document, selection)),
        |_| ActionState::Enabled,
    )
}

fn angle_orientation_availability(
    document: &SketchDocument,
    selection: &[SelectionItem],
    orientation: DocumentAngleOrientation,
) -> ActionState {
    let [SelectionItem::Dimension(id)] = selection else {
        return ActionState::Disabled(selection_reason(document, selection));
    };
    let Some(dimension) = document
        .dimensions()
        .iter()
        .find(|dimension| dimension.id == *id)
    else {
        return ActionState::Disabled(DisabledReason::MissingObject);
    };
    let DocumentDimensionDefinition::OrientedAngle {
        orientation: current,
        ..
    } = &dimension.definition
    else {
        return ActionState::Disabled(DisabledReason::WrongOperandKind);
    };
    if *current == orientation {
        ActionState::Disabled(DisabledReason::AlreadyInRequestedState)
    } else {
        ActionState::Enabled
    }
}

fn selected_objects(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Result<Vec<DocumentObjectId>, DisabledReason> {
    if selection.is_empty() {
        return Err(DisabledReason::EmptySelection);
    }
    let mut seen = HashSet::new();
    let mut objects = Vec::new();
    for item in selection {
        if !selection_exists(document, *item) {
            return Err(DisabledReason::MissingObject);
        }
        let object = item.object();
        if seen.insert(object) {
            objects.push(object);
        }
    }
    Ok(objects)
}

fn selected_sources(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Option<Vec<DocumentSourceId>> {
    if selection.is_empty() {
        return None;
    }
    selection
        .iter()
        .map(|item| match item {
            SelectionItem::Constraint(id) => document
                .constraints()
                .iter()
                .find(|value| value.id == *id)
                .map(|value| value.source_id),
            SelectionItem::Dimension(id) => document
                .dimensions()
                .iter()
                .find(|value| value.id == *id)
                .map(|value| value.source_id),
            SelectionItem::Point(_) | SelectionItem::Curve(_) => None,
        })
        .collect()
}

fn dimension_mode_availability(
    document: &SketchDocument,
    selection: &[SelectionItem],
    mode: DocumentDimensionMode,
) -> ActionState {
    let [SelectionItem::Dimension(dimension)] = selection else {
        return ActionState::Disabled(if selection.is_empty() {
            DisabledReason::EmptySelection
        } else if selection.len() == 1 {
            DisabledReason::WrongOperandKind
        } else {
            DisabledReason::WrongArity
        });
    };
    let Some(value) = document
        .dimensions()
        .iter()
        .find(|value| value.id == *dimension)
    else {
        return ActionState::Disabled(DisabledReason::MissingObject);
    };
    if value.mode == mode {
        ActionState::Disabled(DisabledReason::AlreadyInRequestedState)
    } else {
        ActionState::Enabled
    }
}

fn source_availability(
    document: &SketchDocument,
    selection: &[SelectionItem],
    suppressed: bool,
) -> ActionState {
    let Some(sources) = selected_sources(document, selection) else {
        return ActionState::Disabled(if selection.is_empty() {
            DisabledReason::EmptySelection
        } else {
            DisabledReason::WrongOperandKind
        });
    };
    if sources.iter().all(|source| {
        document
            .source(*source)
            .is_some_and(|value| value.suppressed != suppressed)
    }) {
        ActionState::Enabled
    } else {
        ActionState::Disabled(DisabledReason::AlreadyInRequestedState)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuthoringOutcome, AuthoringState, EditorScene, EditorTool, Modifiers, PickTolerance,
        PointerInput, Viewport,
    };
    use geosolve_sketch::{
        AlphaScenarioIds, AlphaScenarioKind, ContactNeighborhood, DocumentConstraintDefinition,
        DocumentCurveDirectionRelation, DocumentExternalPointRef, DocumentM38DimensionDefinition,
        DocumentMeasurementDefinition, DocumentParameterKind, DocumentPointRef,
        ExternalLineOrientationV1, ExternalSnapshotDigest, ExternalSnapshotEntry,
        ExternalSnapshotFeatureV1, ExternalSnapshotInputError, ExternalSnapshotResourcesV1,
        ExternalSnapshotSet, OperationCheckpoint, OperationStopReason, OperationWorkCounter,
        ParameterBatch, ParameterBatchEntry, ParameterValue, SolverConfig, alpha_scenario,
    };

    fn fixed_line_session() -> (
        RetainedSketchDocumentSession,
        [DesignPointId; 2],
        CurveSpan,
        geosolve_sketch::DesignScalarId,
    ) {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [2.0, 0.0]).expect("point");
        let curve = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        document
            .add_constraint(
                "fix first",
                DocumentConstraintDefinition::FixedPoint {
                    point: first,
                    target: [0.0, 0.0],
                },
            )
            .expect("constraint");
        document
            .add_constraint(
                "fix second",
                DocumentConstraintDefinition::FixedPoint {
                    point: second,
                    target: [2.0, 0.0],
                },
            )
            .expect("constraint");
        let incompatible_target = document
            .add_scalar(
                "incompatible target",
                3.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("scalar");
        #[allow(clippy::default_trait_access)]
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("retained session");
        assert!(session.accepted_state().is_some());
        (
            session,
            [first, second],
            CurveSpan { curve, segment: 0 },
            incompatible_target,
        )
    }

    fn redundant_distance_session() -> (RetainedSketchDocumentSession, DocumentSourceId) {
        let mut document = SketchDocument::new(4.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [2.0, 0.0]).expect("point");
        document
            .add_constraint(
                "fix first",
                DocumentConstraintDefinition::FixedPoint {
                    point: first,
                    target: [0.0, 0.0],
                },
            )
            .expect("fixed point");
        let targets = ["first target", "duplicate target"].map(|label| {
            document
                .add_scalar(label, 2.0, ScalarUnit::Length, ScalarDomain::Positive)
                .expect("target")
        });
        let dimensions = targets.map(|target| {
            document
                .add_dimension(
                    "distance",
                    DocumentDimensionDefinition::PointDistance {
                        first,
                        second,
                        target,
                    },
                    DocumentDimensionMode::Driving,
                )
                .expect("dimension")
        });
        let duplicate = document
            .dimension(dimensions[1])
            .expect("duplicate dimension")
            .source_id;
        #[allow(clippy::default_trait_access)]
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("accepted redundant session");
        (session, duplicate)
    }

    fn external_point_entry(
        binding: DocumentExternalBindingId,
        position: [f64; 2],
    ) -> ExternalSnapshotEntry {
        ExternalSnapshotEntry {
            binding,
            source_revision: 1,
            source_digest: ExternalSnapshotDigest::from_bytes([17; 32]),
            feature: ExternalSnapshotFeatureV1::Point {
                position,
                scale: 1.0,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        }
    }

    fn inference_candidate_coordinator() -> (
        RetainedEditorCoordinator,
        ProvisionalInferenceCandidate,
        SketchDesignIdentity,
        usize,
    ) {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [2.0, 0.0]).expect("point");
        let curve = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        #[allow(clippy::default_trait_access)]
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        let history = coordinator.history_len();
        let span = CurveSpan { curve, segment: 0 };
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(span)]);
        let candidate = ProvisionalInferenceCandidate {
            expected,
            label: "horizontal inference".into(),
            edit: coordinator
                .editor()
                .constraint_edit(
                    coordinator.session().design_document(),
                    ConstraintKind::Horizontal,
                    "inferred horizontal",
                )
                .expect("horizontal edit"),
        };
        assert!(matches!(
            candidate.edit,
            DocumentEdit::CreateConstraint {
                definition: DocumentConstraintDefinition::Horizontal { line },
                ..
            } if line == span
        ));
        (coordinator, candidate, expected, history)
    }

    struct RetainedStateSnapshot {
        design: SketchDesignIdentity,
        accepted: SketchAcceptedStateIdentity,
        design_json: String,
        accepted_json: Option<String>,
        history: usize,
        transcript: usize,
    }

    fn retained_state_snapshot(coordinator: &RetainedEditorCoordinator) -> RetainedStateSnapshot {
        RetainedStateSnapshot {
            design: coordinator.session().design_identity(),
            accepted: coordinator
                .session()
                .accepted_state()
                .expect("accepted")
                .identity(),
            design_json: coordinator
                .session()
                .export_design_json()
                .expect("design JSON"),
            accepted_json: coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON"),
            history: coordinator.history_len(),
            transcript: coordinator.transcript().len(),
        }
    }

    fn assert_retained_state_snapshot(
        coordinator: &RetainedEditorCoordinator,
        snapshot: &RetainedStateSnapshot,
    ) {
        assert_eq!(coordinator.session().design_identity(), snapshot.design);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted")
                .identity(),
            snapshot.accepted
        );
        assert_eq!(
            coordinator
                .session()
                .export_design_json()
                .expect("design JSON"),
            snapshot.design_json
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON"),
            snapshot.accepted_json
        );
        assert_eq!(coordinator.history_len(), snapshot.history);
        assert_eq!(coordinator.transcript().len(), snapshot.transcript);
    }

    #[test]
    fn geometry_role_uses_ordinary_history_replay_and_stale_identity_rejects() {
        let (session, _, span, _) = fixed_line_session();
        let replay_session = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        coordinator
            .set_geometry_role(expected, span.curve, GeometryRole::Construction)
            .expect("role edit");

        assert_eq!(coordinator.history_len(), 2);
        assert!(matches!(
            coordinator.transcript(),
            [ReplayAction::Edit {
                edit: DocumentEdit::SetGeometryRole { curve, role: GeometryRole::Construction },
                ..
            }] if *curve == span.curve
        ));
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .geometry_role(span.curve),
            Some(GeometryRole::Construction)
        );
        assert!(matches!(
            coordinator.set_geometry_role(expected, span.curve, GeometryRole::Profile),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleDesign { .. }
            ))
        ));

        let mut replay = RetainedEditorCoordinator::new(replay_session).expect("replay");
        replay
            .replay(&coordinator.transcript()[0])
            .expect("replay role");
        assert_eq!(
            replay.checkpoint().design_json(),
            coordinator.checkpoint().design_json()
        );
    }

    #[test]
    #[allow(clippy::default_trait_access)]
    fn external_rebind_records_and_replays_only_the_explicit_declaration() {
        let (mut session, _, _, _) = fixed_line_session();
        // Rebuild once so the binding is part of the retained design under test.
        let mut document = session.design_document().clone();
        document
            .add_external_binding("external", ExternalFeatureKindV1::Point, None)
            .expect("binding");
        session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let binding = session.design_document().external_bindings()[0].id;
        let replay_session = session.clone();
        let before = session.design_document().clone();
        let topology = ExternalTopologyDigest::from_bytes([18; 32]);
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .rebind_external_binding(
                coordinator.session().design_identity(),
                binding,
                ExternalFeatureKindV1::LineSegment,
                Some(topology),
            )
            .expect("rebind");

        let after = coordinator.session().design_document();
        assert_eq!(after.points(), before.points());
        assert_eq!(after.curves(), before.curves());
        assert_eq!(after.constraints(), before.constraints());
        assert_eq!(after.dimensions(), before.dimensions());
        assert_eq!(
            after
                .external_binding(binding)
                .expect("binding")
                .expected_kind,
            ExternalFeatureKindV1::LineSegment
        );
        assert_eq!(
            after
                .external_binding(binding)
                .expect("binding")
                .expected_topology,
            Some(topology)
        );
        assert_eq!(coordinator.history_len(), 2);
        assert!(
            matches!(coordinator.transcript(), [ReplayAction::RebindExternalBinding { binding: recorded, .. }] if *recorded == binding)
        );

        let mut replay = RetainedEditorCoordinator::new(replay_session).expect("replay");
        replay
            .replay(&coordinator.transcript()[0])
            .expect("replay rebind");
        assert_eq!(
            replay.checkpoint().design_json(),
            coordinator.checkpoint().design_json()
        );
    }

    #[test]
    #[allow(clippy::default_trait_access)]
    fn parameter_batch_wrapper_stamps_exact_attempt_and_stale_revision_does_not_attempt() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let parameter = document
            .add_parameter("input", DocumentParameterKind::Length)
            .expect("parameter");
        let initial = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(1.0),
            }],
        )
        .expect("batch");
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            initial,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let history = coordinator.history_len();
        let transcript = coordinator.transcript().len();
        let replacement = ParameterBatch::new(
            2,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(2.0),
            }],
        )
        .expect("replacement");
        let outcome = coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                replacement,
                DocumentSolveRequest::default(),
            )
            .expect("replacement attempt");
        assert_eq!(
            outcome.attempt,
            coordinator.session().last_attempt().identity()
        );
        assert_eq!(
            coordinator
                .session()
                .last_attempt()
                .input()
                .parameter_revision(),
            2
        );
        assert_eq!(coordinator.history_len(), history);
        assert_eq!(coordinator.transcript().len(), transcript);

        let attempt = coordinator.session().last_attempt().identity();
        let stale = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(3.0),
            }],
        )
        .expect("stale batch");
        assert!(matches!(
            coordinator.replace_parameter_batch(
                coordinator.session().design_identity(),
                stale,
                DocumentSolveRequest::default(),
            ),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleParameterRevision { .. }
            ))
        ));
        assert_eq!(coordinator.session().last_attempt().identity(), attempt);
    }

    #[test]
    #[allow(clippy::default_trait_access, clippy::too_many_lines)]
    fn snapshot_wrapper_stamps_attempt_retains_accepted_on_bad_input_and_rejects_pre_attempt_stale()
    {
        let mut document = SketchDocument::new(1.0).expect("document");
        let point = document.add_point("point", [1.0, 2.0]).expect("point");
        let binding = document
            .add_external_binding("external", ExternalFeatureKindV1::Point, None)
            .expect("binding");
        let inactive_binding = document
            .add_external_binding("inactive external", ExternalFeatureKindV1::Point, None)
            .expect("inactive binding");
        document
            .add_constraint(
                "external point",
                DocumentConstraintDefinition::ExternalPointCoincident {
                    point,
                    external: DocumentExternalPointRef { binding },
                },
            )
            .expect("constraint");
        let initial = ExternalSnapshotSet::new(1, vec![external_point_entry(binding, [1.0, 2.0])])
            .expect("initial snapshots");
        let session = RetainedSketchDocumentSession::new_with_inputs(
            document,
            ParameterBatch::default(),
            initial,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted").identity();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let wrong_kind = ExternalSnapshotSet::new(
            2,
            vec![ExternalSnapshotEntry {
                binding,
                source_revision: 1,
                source_digest: ExternalSnapshotDigest::from_bytes([19; 32]),
                feature: ExternalSnapshotFeatureV1::LineSegment {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                    domain: [0.0, 1.0],
                    orientation: ExternalLineOrientationV1::StartToEnd,
                    scale: 1.0,
                    topology_digest: ExternalTopologyDigest::from_bytes([20; 32]),
                    resources: ExternalSnapshotResourcesV1 {
                        point_count: 2,
                        control_count: 0,
                        span_count: 1,
                    },
                },
            }],
        )
        .expect("wrong-kind snapshots");
        let outcome = coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                wrong_kind,
                DocumentSolveRequest::default(),
            )
            .expect("typed failed attempt");
        assert_eq!(
            outcome.attempt,
            coordinator.session().last_attempt().identity()
        );
        assert!(matches!(
            coordinator
                .session()
                .last_attempt()
                .failure()
                .and_then(|failure| failure.external_snapshot_error()),
            Some(ExternalSnapshotInputError::WrongKind { .. })
        ));
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("retained accepted")
                .identity(),
            accepted
        );

        coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                ExternalSnapshotSet::new(
                    3,
                    vec![external_point_entry(inactive_binding, [3.0, 4.0])],
                )
                .expect("unavailable snapshots"),
                DocumentSolveRequest::default(),
            )
            .expect("unavailable attempt");
        assert!(matches!(
            coordinator.session().last_attempt().failure().and_then(|failure| failure.external_snapshot_error()),
            Some(ExternalSnapshotInputError::MissingBinding { binding: actual }) if *actual == binding
        ));
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("retained accepted")
                .identity(),
            accepted
        );

        let attempt = coordinator.session().last_attempt().identity();
        assert!(matches!(
            coordinator.replace_external_snapshot_set(
                coordinator.session().design_identity(),
                ExternalSnapshotSet::new(
                    1,
                    vec![
                        external_point_entry(binding, [1.0, 2.0]),
                        external_point_entry(inactive_binding, [3.0, 4.0]),
                    ],
                )
                .expect("stale snapshots"),
                DocumentSolveRequest::default(),
            ),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleExternalSnapshotRevision { .. }
            ))
        ));
        assert_eq!(coordinator.session().last_attempt().identity(), attempt);
    }

    #[test]
    fn accepted_redundancy_is_a_verbatim_sketch_dto() {
        let (session, duplicate) = redundant_distance_session();
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted state");
        let domain = accepted.accepted_redundancy();
        let editor = coordinator
            .accepted_redundancy()
            .expect("accepted redundancy");

        assert_eq!(editor, domain);
        assert_eq!(editor.accepted_state_identity(), accepted.identity());
        assert_eq!(editor.design_identity(), accepted.design_identity());
        assert_eq!(editor.fully_redundant_sources(), [duplicate]);
        assert_eq!(editor.sources_containing_redundant_rows(), [duplicate]);
    }

    #[test]
    #[allow(clippy::default_trait_access, clippy::too_many_lines)]
    fn current_problem_metadata_uses_attempted_owner_dependencies_and_clears_on_recovery() {
        let (session, points, span, target) = fixed_line_session();
        let accepted_identity = session.accepted_state().expect("accepted").identity();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        assert!(coordinator.current_problem_metadata().is_none());

        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateDimension {
                    label: "incompatible line length".into(),
                    definition: DocumentDimensionDefinition::CurveLength {
                        curve: span,
                        target,
                    },
                    mode: DocumentDimensionMode::Driving,
                },
            )
            .expect("retain rejected dimension");

        let dimension = coordinator.session().design_document().dimensions()[0].id;
        assert!(
            coordinator
                .session()
                .accepted_state()
                .expect("retained accepted")
                .document()
                .dimension(dimension)
                .is_none(),
            "the rejected owner must exist only in the attempted design"
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("retained accepted")
                .identity(),
            accepted_identity
        );

        let metadata = coordinator
            .current_problem_metadata()
            .expect("rejected attempt metadata");
        assert_eq!(
            metadata.attempt,
            coordinator.session().last_attempt().identity()
        );
        assert_eq!(metadata.design, coordinator.session().design_identity());
        assert_eq!(metadata.scope, EditorProblemScope::Targeted);
        assert_eq!(metadata.category, EditorProblemCategory::Solver);
        assert!(
            metadata
                .targets
                .contains(&EditorProblemTarget::Dimension(dimension))
        );
        assert!(
            metadata
                .targets
                .contains(&EditorProblemTarget::Curve(span.curve))
        );
        for point in points {
            assert!(
                metadata
                    .targets
                    .contains(&EditorProblemTarget::Point(point))
            );
        }
        assert!(!metadata.message.is_empty());

        let closure = coordinator
            .session()
            .design_document()
            .dependency_closure(dimension);
        assert_eq!(
            closure,
            coordinator
                .session()
                .design_document()
                .dependency_closure(dimension),
            "dependency ordering must be deterministic"
        );
        assert!(closure.contains(&DocumentElementId::Curve(span.curve)));
        assert!(closure.contains(&DocumentElementId::Point(points[0])));
        assert!(closure.contains(&DocumentElementId::Point(points[1])));
        assert!(closure.contains(&DocumentElementId::Scalar(target)));

        coordinator
            .set_dimension_mode(
                coordinator.session().design_identity(),
                dimension,
                DocumentDimensionMode::Reference,
            )
            .expect("reference recovery");
        assert!(coordinator.current_problem_metadata().is_none());
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("recovered accepted")
                .document()
                .dimension(dimension)
                .expect("recovered dimension")
                .mode,
            DocumentDimensionMode::Reference
        );
    }

    #[test]
    #[allow(clippy::default_trait_access)]
    fn current_problem_metadata_keeps_wrong_kind_parameter_failure_global() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let rectangle = document
            .add_rectangle("parameter input", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("length input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                geosolve_sketch::DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("parameter binding");
        let initial = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(4.0),
            }],
        )
        .expect("initial input");
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            initial,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted").identity();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                ParameterBatch::new(
                    2,
                    vec![ParameterBatchEntry {
                        parameter,
                        value: ParameterValue::Angle(1.0),
                    }],
                )
                .expect("wrong-kind input"),
                DocumentSolveRequest::default(),
            )
            .expect("record failed attempt");
        let metadata = coordinator
            .current_problem_metadata()
            .expect("failed-attempt metadata");
        assert_eq!(metadata.category, EditorProblemCategory::Input);
        assert_eq!(metadata.scope, EditorProblemScope::Global);
        assert!(metadata.targets.is_empty());
        assert!(metadata.message.contains("wrong kind"));
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("retained accepted")
                .identity(),
            accepted
        );

        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                ParameterBatch::new(
                    3,
                    vec![ParameterBatchEntry {
                        parameter,
                        value: ParameterValue::Length(2.0),
                    }],
                )
                .expect("recovery input"),
                DocumentSolveRequest::default(),
            )
            .expect("recover");
        assert!(coordinator.current_problem_metadata().is_none());
    }

    #[test]
    fn rejected_dimension_is_retained_and_undo_restores_with_fresh_revisions() {
        let (session, points, _, target) = fixed_line_session();
        let initial_accepted = session.accepted_state().expect("accepted").identity();
        let initial_accepted_json = session
            .export_accepted_json()
            .expect("accepted JSON")
            .expect("accepted bytes");
        let initial_revision = session.design_identity().revision().get();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection(points.map(SelectionItem::Point));
        let expected = coordinator.session().design_identity();
        let outcome = coordinator
            .apply_edit(
                expected,
                DocumentEdit::CreateDimension {
                    label: "conflict".into(),
                    definition: DocumentDimensionDefinition::PointDistance {
                        first: points[0],
                        second: points[1],
                        target,
                    },
                    mode: DocumentDimensionMode::Driving,
                },
            )
            .expect("valid retained edit");

        assert!(outcome.published_accepted.is_none());
        assert_eq!(
            coordinator.lifecycle().status,
            LifecycleStatus::RejectedAttempt
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("retained accepted")
                .identity(),
            initial_accepted
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON")
                .expect("accepted bytes"),
            initial_accepted_json
        );
        assert_eq!(
            coordinator.session().design_document().dimensions().len(),
            1
        );
        assert_eq!(coordinator.history_len(), 2);

        let rejected_revision = coordinator.session().design_identity().revision().get();
        coordinator.undo().expect("undo");
        assert!(
            coordinator
                .session()
                .design_document()
                .dimensions()
                .is_empty()
        );
        assert!(coordinator.session().design_identity().revision().get() > rejected_revision);
        assert!(rejected_revision > initial_revision);
        coordinator.redo().expect("redo");
        assert_eq!(
            coordinator.session().design_document().dimensions().len(),
            1
        );
    }

    #[test]
    fn stale_edit_is_history_and_selection_neutral_and_new_edit_truncates_redo() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection(points.map(SelectionItem::Point));
        let stale = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                stale,
                DocumentEdit::CreatePoint {
                    label: "one".into(),
                    position: [4.0, 0.0],
                },
            )
            .expect("first edit");
        let history = coordinator.history_len();
        let selection = coordinator.editor().selection().to_vec();
        assert!(matches!(
            coordinator.apply_edit(
                stale,
                DocumentEdit::CreatePoint {
                    label: "stale".into(),
                    position: [5.0, 0.0],
                }
            ),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleDesign { .. }
            ))
        ));
        assert_eq!(coordinator.history_len(), history);
        assert_eq!(coordinator.editor().selection(), selection);

        coordinator.undo().expect("undo");
        let current = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                current,
                DocumentEdit::CreatePoint {
                    label: "replacement".into(),
                    position: [6.0, 0.0],
                },
            )
            .expect("replacement");
        assert!(!coordinator.can_redo());
    }

    #[test]
    fn stale_identity_precedes_incompatible_selection_without_mutation() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let stale = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                stale,
                DocumentEdit::CreatePoint {
                    label: "advance".into(),
                    position: [4.0, 0.0],
                },
            )
            .expect("advance design");
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Point(points[0])]);
        let design_json = coordinator.checkpoint().design_json().to_owned();
        let history = coordinator.history_len();
        let transcript = coordinator.transcript().len();

        assert!(matches!(
            coordinator.add_point_distance_dimension(
                stale,
                DocumentDimensionMode::Reference,
                "incompatible stale"
            ),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleDesign { .. }
            ))
        ));
        assert_eq!(coordinator.checkpoint().design_json(), design_json);
        assert_eq!(coordinator.history_len(), history);
        assert_eq!(coordinator.transcript().len(), transcript);
    }

    #[test]
    fn stale_editor_commit_effects_are_rejected_before_dispatch_mutation() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let stale = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                stale,
                DocumentEdit::CreatePoint {
                    label: "advance".into(),
                    position: [4.0, 0.0],
                },
            )
            .expect("advance design");
        let design_json = coordinator.checkpoint().design_json().to_owned();
        let history = coordinator.history_len();
        let transcript = coordinator.transcript().len();
        let effects = [
            EditorEffect::CommitPointMove {
                expected: stale,
                point: points[0],
                model_position: [1.0, 1.0],
            },
            EditorEffect::CommitConstruction {
                expected: stale,
                proposal: ConstructionProposal::Point {
                    position: [7.0, 2.0],
                },
            },
        ];

        for effect in &effects {
            assert!(matches!(
                coordinator.apply_editor_effect(effect),
                Err(CoordinatorError::Session(
                    DocumentSessionError::StaleDesign { .. }
                ))
            ));
            assert_eq!(coordinator.checkpoint().design_json(), design_json);
            assert_eq!(coordinator.history_len(), history);
            assert_eq!(coordinator.transcript().len(), transcript);
        }
    }

    #[test]
    fn undo_restores_checkpoint_geometry_without_current_state_preferences() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [2.0, 0.0]).expect("point");
        let curve = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        document
            .add_constraint(
                "horizontal",
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan { curve, segment: 0 },
                },
            )
            .expect("constraint");
        #[allow(clippy::default_trait_access)]
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let initial_document = session
            .accepted_state()
            .expect("initial accepted")
            .document();
        let initial = [
            initial_document.point(first).expect("first").position,
            initial_document.point(second).expect("second").position,
        ];
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                expected,
                DocumentEdit::SetPointPosition {
                    point: first,
                    position: [3.0, 2.0],
                },
            )
            .expect("move");

        coordinator.undo().expect("undo");

        let restored = coordinator
            .session()
            .accepted_state()
            .expect("restored accepted")
            .document();
        assert_eq!(
            [
                restored.point(first).expect("first").position,
                restored.point(second).expect("second").position,
            ]
            .map(|position| position.map(f64::to_bits)),
            initial.map(|position| position.map(f64::to_bits)),
        );
    }

    #[test]
    fn reattempt_records_once_and_replays_the_same_attempt_transition() {
        let (session, _, _, _) = fixed_line_session();
        let replay_session = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        let attempt = coordinator.reattempt(expected).expect("reattempt");

        assert_eq!(coordinator.transcript().len(), 1);
        assert!(matches!(
            coordinator.transcript(),
            [ReplayAction::Reattempt { expected: recorded }] if *recorded == expected
        ));
        assert_eq!(coordinator.history_len(), 1);

        let mut replay = RetainedEditorCoordinator::new(replay_session).expect("replay");
        replay
            .replay(&coordinator.transcript()[0])
            .expect("replay reattempt");
        assert_eq!(replay.session().last_attempt().identity(), attempt);
        assert_eq!(replay.transcript(), coordinator.transcript());
        assert_eq!(replay.history_len(), 1);
    }

    #[test]
    fn action_matrix_dimensions_and_replay_are_deterministic() {
        let (session, points, span, _) = fixed_line_session();
        let replay_session = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection(points.map(SelectionItem::Point));
        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::Dimension(
                DimensionKind::PointDistance,
                DocumentDimensionMode::Reference,
            ),
            state: ActionState::Enabled,
        }));
        let expected = coordinator.session().design_identity();
        coordinator
            .add_point_distance_dimension(expected, DocumentDimensionMode::Reference, "distance")
            .expect("reference dimension");
        let transcript = coordinator.transcript().to_vec();

        let mut replay = RetainedEditorCoordinator::new(replay_session).expect("replay");
        for action in &transcript {
            replay.replay(action).expect("replay action");
        }
        assert_eq!(
            replay.checkpoint().design_json(),
            coordinator.checkpoint().design_json()
        );

        replay
            .editor_mut()
            .set_selection([SelectionItem::Curve(span)]);
        assert!(replay.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::Dimension(
                DimensionKind::SegmentLength,
                DocumentDimensionMode::Driving,
            ),
            state: ActionState::Enabled,
        }));
    }

    #[test]
    fn selected_dimension_routes_points_and_linear_spans_without_adapter_policy() {
        for mode in [
            DocumentDimensionMode::Driving,
            DocumentDimensionMode::Reference,
        ] {
            let (session, points, _, _) = fixed_line_session();
            let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
            coordinator
                .editor_mut()
                .set_selection(points.map(SelectionItem::Point));
            let point_dimension = coordinator
                .add_selected_dimension(
                    coordinator.session().design_identity(),
                    mode,
                    "selected points",
                )
                .expect("point-distance route")
                .value;
            assert!(matches!(
                coordinator
                    .session()
                    .design_document()
                    .dimension(point_dimension)
                    .expect("point dimension")
                    .definition,
                DocumentDimensionDefinition::PointDistance { first, second, .. }
                    if first == points[0] && second == points[1]
            ));

            let (session, _, span_again, _) = fixed_line_session();
            let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
            coordinator
                .editor_mut()
                .set_selection([SelectionItem::Curve(span_again)]);
            let segment_dimension = coordinator
                .add_selected_dimension(
                    coordinator.session().design_identity(),
                    mode,
                    "selected span",
                )
                .expect("segment-length route")
                .value;
            assert!(matches!(
                coordinator
                    .session()
                    .design_document()
                    .dimension(segment_dimension)
                    .expect("segment dimension")
                    .definition,
                DocumentDimensionDefinition::CurveLength { curve, .. } if curve == span_again
            ));
        }
    }

    #[test]
    fn selected_dimension_rejects_incompatible_selection_without_mutation() {
        let (session, points, span, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        let before = coordinator.checkpoint().design_json().to_owned();
        let history = coordinator.history_len();
        let transcript = coordinator.transcript().len();

        for selection in [
            vec![],
            vec![SelectionItem::Point(points[0])],
            vec![SelectionItem::Point(points[0]), SelectionItem::Curve(span)],
        ] {
            coordinator.editor_mut().set_selection(selection);
            assert!(matches!(
                coordinator.add_selected_dimension(
                    expected,
                    DocumentDimensionMode::Reference,
                    "incompatible"
                ),
                Err(CoordinatorError::IncompatibleDimension)
            ));
            assert_eq!(coordinator.checkpoint().design_json(), before);
            assert_eq!(coordinator.history_len(), history);
            assert_eq!(coordinator.transcript().len(), transcript);
        }
    }

    #[test]
    fn explicit_authoring_operands_are_selection_independent_and_clear_no_host_selection() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        assert!(coordinator.editor().selection().is_empty());
        let application = AuthoringApplication {
            tool: AuthoringTool::Constraint(ConstraintIntent::Coincident),
            operands: points
                .map(SelectionItem::Point)
                .map(AuthoringOperand::selected)
                .to_vec(),
            options: AuthoringOptions::default(),
            resolved_constraint: Some(ResolvedConstraintKind::CoincidentPoints),
        };
        let history = coordinator.history_len();
        let result = coordinator
            .apply_authoring(coordinator.session().design_identity(), &application)
            .expect("retained rejected constraint");
        assert!(matches!(result, AuthoringMutation::Constraint(_)));
        assert_eq!(coordinator.history_len(), history + 1);
        assert!(coordinator.editor().selection().is_empty());
        assert!(coordinator.session().accepted_state().is_some());
        assert_eq!(
            coordinator.session().design_document().constraints().len(),
            3
        );
    }

    #[test]
    fn explicit_dimension_target_edit_is_retained_undoable_and_redoable() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let selection = points.map(SelectionItem::Point);
        let created = coordinator
            .apply_dimension_action_for(
                coordinator.session().design_identity(),
                &selection,
                DimensionActionRequest {
                    kind: DimensionKind::PointDistance,
                    mode: DocumentDimensionMode::Reference,
                    label: "distance".into(),
                    angle_orientation: DocumentAngleOrientation::CounterClockwise,
                },
            )
            .expect("reference dimension");
        let dimension = created.value;
        let metadata = coordinator
            .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
            .expect("target metadata");
        assert!((metadata.value - 2.0).abs() < 1.0e-12);
        coordinator
            .set_dimension_target(coordinator.session().design_identity(), dimension, 3.5)
            .expect("target edit");
        assert!(
            (coordinator
                .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
                .expect("edited metadata")
                .value
                - 3.5)
                .abs()
                < 1.0e-12
        );
        coordinator.undo().expect("undo target");
        assert!(
            (coordinator
                .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
                .expect("restored metadata")
                .value
                - 2.0)
                .abs()
                < 1.0e-12
        );
        coordinator.redo().expect("redo target");
        assert!(
            (coordinator
                .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
                .expect("redone metadata")
                .value
                - 3.5)
                .abs()
                < 1.0e-12
        );
    }

    #[test]
    #[allow(clippy::default_trait_access, clippy::too_many_lines)]
    fn angle_authoring_measures_accepted_geometry_and_does_not_move_it() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let origin = document.add_point("origin", [0.0, 0.0]).expect("point");
        let x = document.add_point("x", [2.0, 0.0]).expect("point");
        let moving = document
            .add_point("moving", [2.0 * 0.5_f64.cos(), 2.0 * 0.5_f64.sin()])
            .expect("point");
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: origin,
                        end: x,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: origin,
                        end: moving,
                        branch_direction: [0.5_f64.cos(), 0.5_f64.sin()],
                    },
                )
                .expect("line"),
        );
        for (label, point, target) in [("fix origin", origin, [0.0, 0.0]), ("fix x", x, [2.0, 0.0])]
        {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("constraint");
        }
        document
            .add_constraint(
                "accepted vertical",
                DocumentConstraintDefinition::Vertical { line: second },
            )
            .expect("vertical");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let accepted_before = session
            .accepted_state()
            .expect("accepted")
            .document()
            .point(moving)
            .expect("accepted point")
            .position;
        assert!((accepted_before[0]).abs() < 1.0e-9);
        assert!(
            (session
                .design_document()
                .point(moving)
                .expect("design point")
                .position[0])
                .abs()
                > 1.0
        );

        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let created = coordinator
            .apply_dimension_action_for(
                coordinator.session().design_identity(),
                &[SelectionItem::Curve(first), SelectionItem::Curve(second)],
                DimensionActionRequest {
                    kind: DimensionKind::OrientedAngle,
                    mode: DocumentDimensionMode::Driving,
                    label: "accepted angle".into(),
                    angle_orientation: DocumentAngleOrientation::CounterClockwise,
                },
            )
            .expect("angle dimension");
        assert!(created.published_accepted.is_some());
        let metadata = coordinator
            .dimension_target_metadata_for(&[SelectionItem::Dimension(created.value)])
            .expect("metadata");
        assert!((metadata.value - std::f64::consts::FRAC_PI_2).abs() < 1.0e-9);
        assert!((metadata.display_value - 90.0).abs() < 1.0e-8);
        assert_eq!(
            metadata.display_unit,
            DimensionTargetDisplayUnit::AcuteDegrees
        );
        let accepted_after = coordinator
            .session()
            .accepted_state()
            .expect("accepted")
            .document()
            .point(moving)
            .expect("accepted point")
            .position;
        assert!((accepted_after[0] - accepted_before[0]).abs() < 1.0e-9);
        assert!((accepted_after[1] - accepted_before[1]).abs() < 1.0e-9);
    }

    #[test]
    #[allow(clippy::default_trait_access, clippy::too_many_lines)]
    fn authored_horizontal_and_perpendicular_publish_from_skew_free_lines() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first_start = document.add_point("first start", [-2.0, -1.0]).unwrap();
        let first_end = document.add_point("first end", [1.0, 0.5]).unwrap();
        let second_start = document.add_point("second start", [-1.0, 2.0]).unwrap();
        let second_end = document.add_point("second end", [1.0, -1.0]).unwrap();
        let first_direction = [2.0 / 5.0_f64.sqrt(), 1.0 / 5.0_f64.sqrt()];
        let second_direction = [2.0 / 13.0_f64.sqrt(), -3.0 / 13.0_f64.sqrt()];
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: first_start,
                        end: first_end,
                        branch_direction: first_direction,
                    },
                )
                .unwrap(),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: second_start,
                        end: second_end,
                        branch_direction: second_direction,
                    },
                )
                .unwrap(),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();

        let horizontal = AuthoringState::default().activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            &[AuthoringOperand::selected(SelectionItem::Curve(first))],
        );
        let AuthoringOutcome::Apply(horizontal) = horizontal else {
            panic!("horizontal application");
        };
        let AuthoringMutation::Constraint(horizontal) = coordinator
            .apply_authoring(coordinator.session().design_identity(), &horizontal)
            .unwrap()
        else {
            panic!("horizontal mutation");
        };
        assert!(horizontal.published_accepted.is_some());
        assert!(matches!(
            coordinator
                .session()
                .design_document()
                .constraint(horizontal.value)
                .unwrap()
                .definition,
            DocumentConstraintDefinition::Horizontal { line } if line == first
        ));

        let perpendicular = AuthoringState::default().activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Perpendicular),
            &[
                AuthoringOperand::selected(SelectionItem::Curve(first)),
                AuthoringOperand::selected(SelectionItem::Curve(second)),
            ],
        );
        let AuthoringOutcome::Apply(perpendicular) = perpendicular else {
            panic!("perpendicular application");
        };
        let AuthoringMutation::Constraint(perpendicular) = coordinator
            .apply_authoring(coordinator.session().design_identity(), &perpendicular)
            .unwrap()
        else {
            panic!("perpendicular mutation");
        };
        assert!(perpendicular.published_accepted.is_some());
        assert!(matches!(
            coordinator
                .session()
                .design_document()
                .constraint(perpendicular.value)
                .unwrap()
                .definition,
            DocumentConstraintDefinition::Perpendicular { first: actual_first, second: actual_second }
                if actual_first == first && actual_second == second
        ));
    }

    #[test]
    #[allow(clippy::default_trait_access, clippy::too_many_lines)]
    fn every_resolved_authoring_family_emits_only_its_owned_metadata() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let points = [
            document.add_point("a", [0.0, 0.0]).unwrap(),
            document.add_point("b", [2.0, 0.0]).unwrap(),
            document.add_point("c", [0.0, 1.0]).unwrap(),
            document.add_point("d", [2.0, 1.0]).unwrap(),
        ];
        let first_line = CurveSpan::line(
            document
                .add_curve(
                    "first line",
                    CurveDefinition::Line {
                        start: points[0],
                        end: points[1],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let second_line = CurveSpan::line(
            document
                .add_curve(
                    "second line",
                    CurveDefinition::Line {
                        start: points[2],
                        end: points[3],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let radius = document
            .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        let circle = CurveSpan::line(
            document
                .add_curve(
                    "circle",
                    CurveDefinition::Circle {
                        center: points[2],
                        radius,
                    },
                )
                .unwrap(),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .unwrap();
        let coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let options = AuthoringOptions {
            tangent_orientation: TangentOrientation::Opposed,
            curvature_relation: DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
            continuity: DocumentCurveContinuity::G2,
            ..AuthoringOptions::default()
        };
        let point = |index| SelectionItem::Point(points[index]);
        let curve = SelectionItem::Curve;
        let cases = [
            (
                ConstraintIntent::Lock,
                ResolvedConstraintKind::FixedPoint,
                vec![point(0)],
                0,
            ),
            (
                ConstraintIntent::Coincident,
                ResolvedConstraintKind::CoincidentPoints,
                vec![point(0), point(1)],
                0,
            ),
            (
                ConstraintIntent::Coincident,
                ResolvedConstraintKind::PointOnCurve,
                vec![point(0), curve(first_line)],
                1,
            ),
            (
                ConstraintIntent::Coincident,
                ResolvedConstraintKind::CurveContact,
                vec![curve(first_line), curve(second_line)],
                2,
            ),
            (
                ConstraintIntent::Horizontal,
                ResolvedConstraintKind::HorizontalLine,
                vec![curve(first_line)],
                0,
            ),
            (
                ConstraintIntent::Vertical,
                ResolvedConstraintKind::VerticalLine,
                vec![curve(first_line)],
                0,
            ),
            (
                ConstraintIntent::Parallel,
                ResolvedConstraintKind::ParallelLines,
                vec![curve(first_line), curve(second_line)],
                0,
            ),
            (
                ConstraintIntent::Perpendicular,
                ResolvedConstraintKind::PerpendicularLines,
                vec![curve(first_line), curve(second_line)],
                0,
            ),
            (
                ConstraintIntent::Perpendicular,
                ResolvedConstraintKind::RadialLine,
                vec![curve(circle), curve(first_line)],
                1,
            ),
            (
                ConstraintIntent::Equal,
                ResolvedConstraintKind::EqualLength,
                vec![curve(first_line), curve(second_line)],
                0,
            ),
            (
                ConstraintIntent::Equal,
                ResolvedConstraintKind::EqualRadius,
                vec![curve(circle), curve(circle)],
                0,
            ),
            (
                ConstraintIntent::Equal,
                ResolvedConstraintKind::EqualCurvature,
                vec![curve(first_line), curve(second_line)],
                2,
            ),
            (
                ConstraintIntent::Midpoint,
                ResolvedConstraintKind::Midpoint,
                vec![point(0), curve(first_line)],
                0,
            ),
            (
                ConstraintIntent::Symmetric,
                ResolvedConstraintKind::SymmetricAboutLine,
                vec![point(0), point(1), curve(second_line)],
                0,
            ),
            (
                ConstraintIntent::Tangent,
                ResolvedConstraintKind::CurveTangency,
                vec![curve(first_line), curve(second_line)],
                2,
            ),
            (
                ConstraintIntent::Continuity,
                ResolvedConstraintKind::EndpointContinuity,
                vec![curve(first_line), curve(second_line)],
                2,
            ),
        ];
        assert_eq!(cases.len(), 16);
        for (intent, resolved, selection, expected_contacts) in cases {
            let mut curve_occurrence = 0_u8;
            let operands = selection
                .iter()
                .copied()
                .map(|item| {
                    let parameter = matches!(item, SelectionItem::Curve(_)).then(|| {
                        curve_occurrence += 1;
                        f64::from(curve_occurrence) * 0.25
                    });
                    AuthoringOperand::picked(item, parameter)
                })
                .collect::<Vec<_>>();
            let request = coordinator
                .authoring_constraint_request(intent, resolved, &selection, &operands, options)
                .unwrap_or_else(|error| panic!("{resolved:?}: {error}"));
            assert_eq!(
                request.contacts.len(),
                expected_contacts,
                "{resolved:?} contact count"
            );
            for contact in &request.contacts {
                assert_eq!(
                    contact.tangent_orientation,
                    (resolved == ResolvedConstraintKind::CurveTangency)
                        .then_some(TangentOrientation::Opposed),
                    "{resolved:?} tangent metadata"
                );
            }
            let expected_relation = match resolved {
                ResolvedConstraintKind::EqualCurvature => {
                    Some(ConstraintRelationChoice::EqualCurvature(
                        DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
                    ))
                }
                ResolvedConstraintKind::EndpointContinuity => Some(
                    ConstraintRelationChoice::Continuity(DocumentCurveContinuity::G2),
                ),
                _ => None,
            };
            assert_eq!(
                request.relation, expected_relation,
                "{resolved:?} relation metadata"
            );
            if resolved == ResolvedConstraintKind::RadialLine {
                assert_eq!(request.contacts[0].support.span, first_line);
                assert_eq!(request.contacts[0].parameter.to_bits(), 0.5_f64.to_bits());
            }
        }

        let repeated = [
            AuthoringOperand::picked(curve(first_line), Some(0.2)),
            AuthoringOperand::picked(curve(first_line), Some(0.8)),
        ];
        let request = coordinator
            .authoring_constraint_request(
                ConstraintIntent::Coincident,
                ResolvedConstraintKind::CurveContact,
                &[curve(first_line), curve(first_line)],
                &repeated,
                options,
            )
            .unwrap();
        assert_eq!(request.contacts[0].parameter.to_bits(), 0.2_f64.to_bits());
        assert_eq!(request.contacts[1].parameter.to_bits(), 0.8_f64.to_bits());
    }

    #[test]
    #[allow(clippy::default_trait_access, clippy::too_many_lines)]
    fn reversed_line_angle_displays_and_edits_the_acute_intersection_branch() {
        for stored_degrees in [30.0_f64, 150.0, 210.0, 330.0] {
            let display = display_dimension_target(stored_degrees.to_radians(), ScalarUnit::Angle)
                .expect("finite display");
            assert!((display.value - 30.0).abs() < 1.0e-12);
            assert_eq!(display.unit, DimensionTargetDisplayUnit::AcuteDegrees);
        }

        let mut document = SketchDocument::new(1.0).expect("document");
        let intersection = document
            .add_point("intersection", [0.0, 0.0])
            .expect("point");
        let x = document.add_point("x", [2.0, 0.0]).expect("point");
        let tip = document
            .add_point(
                "tip",
                [
                    2.0 * std::f64::consts::FRAC_1_SQRT_2,
                    2.0 * std::f64::consts::FRAC_1_SQRT_2,
                ],
            )
            .expect("point");
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: intersection,
                        end: x,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "reversed second",
                    CurveDefinition::Line {
                        start: tip,
                        end: intersection,
                        branch_direction: [
                            -std::f64::consts::FRAC_1_SQRT_2,
                            -std::f64::consts::FRAC_1_SQRT_2,
                        ],
                    },
                )
                .expect("line"),
        );
        for (label, point, target) in [
            ("fix intersection", intersection, [0.0, 0.0]),
            ("fix x", x, [2.0, 0.0]),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("constraint");
        }
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let created = coordinator
            .apply_dimension_action_for(
                coordinator.session().design_identity(),
                &[SelectionItem::Curve(first), SelectionItem::Curve(second)],
                DimensionActionRequest {
                    kind: DimensionKind::OrientedAngle,
                    mode: DocumentDimensionMode::Driving,
                    label: "acute angle".into(),
                    angle_orientation: DocumentAngleOrientation::CounterClockwise,
                },
            )
            .expect("angle dimension");
        assert!(created.published_accepted.is_some());
        let metadata = coordinator
            .dimension_target_metadata_for(&[SelectionItem::Dimension(created.value)])
            .expect("metadata");
        assert!((metadata.value - 5.0 * std::f64::consts::FRAC_PI_4).abs() < 1.0e-9);
        assert!((metadata.display_value - 45.0).abs() < 1.0e-8);
        let design = coordinator.session().design_identity();
        assert!(matches!(
            coordinator.set_dimension_display_target(design, created.value, 91.0),
            Err(CoordinatorError::InvalidActionInput(_))
        ));
        assert_eq!(coordinator.session().design_identity(), design);

        let edited = coordinator
            .set_dimension_display_target(
                coordinator.session().design_identity(),
                created.value,
                60.0,
            )
            .expect("display edit");
        assert!(edited.published_accepted.is_some());
        let metadata = coordinator
            .dimension_target_metadata_for(&[SelectionItem::Dimension(created.value)])
            .expect("metadata");
        assert!((metadata.value - 4.0 * std::f64::consts::PI / 3.0).abs() < 1.0e-9);
        assert!((metadata.display_value - 60.0).abs() < 1.0e-8);
        let tip = coordinator
            .session()
            .accepted_state()
            .expect("accepted")
            .document()
            .point(tip)
            .expect("tip")
            .position;
        let visible = tip[1].atan2(tip[0]).abs().to_degrees();
        assert!((visible - 60.0).abs() < 1.0e-7);
    }

    #[test]
    fn reload_uses_checkpoint_bytes_without_reusing_revisions() {
        let (session, _, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let saved = coordinator.checkpoint().clone();
        let expected = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                expected,
                DocumentEdit::CreatePoint {
                    label: "later".into(),
                    position: [8.0, 1.0],
                },
            )
            .expect("edit");
        let high_water = coordinator.session().revision_high_water();

        coordinator.reload(&saved).expect("reload");
        assert_eq!(coordinator.history_len(), 1);
        assert_eq!(coordinator.history_cursor(), 0);
        assert_eq!(
            coordinator.session().design_document().points().len(),
            SketchDocument::from_json(saved.design_json())
                .expect("saved document")
                .points()
                .len()
        );
        assert!(
            coordinator.session().design_identity().revision().get() > high_water.design().get()
        );
        assert!(
            coordinator
                .session()
                .last_attempt()
                .identity()
                .revision()
                .get()
                > high_water.attempt().get()
        );
    }

    #[test]
    fn suppression_delete_and_selection_reconciliation_use_persistent_ids() {
        let (session, _, _, _) = fixed_line_session();
        let constraint = session.design_document().constraints()[0].id;
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Constraint(constraint)]);
        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::Suppress,
            state: ActionState::Enabled,
        }));
        let expected = coordinator.session().design_identity();
        coordinator
            .set_selected_suppressed(expected, true)
            .expect("suppress");
        let source = coordinator.session().design_document().constraints()[0].source_id;
        assert!(
            coordinator
                .session()
                .design_document()
                .source(source)
                .expect("source")
                .suppressed
        );

        let expected = coordinator.session().design_identity();
        coordinator
            .delete_selected(expected)
            .expect("delete constraint");
        assert!(coordinator.editor().selection().is_empty());
        assert!(
            coordinator
                .session()
                .design_document()
                .constraints()
                .iter()
                .all(|value| value.id != constraint)
        );
    }

    #[test]
    fn delete_selected_uses_domain_dependency_cleanup_and_undo_restores_ids() {
        let (session, points, span, _) = fixed_line_session();
        let curve = span.curve;
        let dependent_constraint = session.design_document().constraints()[0].id;
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Point(points[0])]);
        let expected = coordinator.session().design_identity();
        let outcome = coordinator.delete_selected(expected).expect("delete point");

        assert_eq!(outcome.value, vec![DocumentObjectId::Point(points[0])]);
        assert_eq!(coordinator.history_len(), 2);
        assert!(matches!(
            coordinator.transcript(),
            [ReplayAction::Delete { selection, .. }]
                if selection == &vec![SelectionItem::Point(points[0])]
        ));
        let document = coordinator.session().design_document();
        assert!(document.point(points[0]).is_none());
        assert!(document.curve(curve).is_none());
        assert!(
            document
                .constraints()
                .iter()
                .all(|value| value.id != dependent_constraint)
        );
        assert!(coordinator.editor().selection().is_empty());

        coordinator.undo().expect("undo");
        let document = coordinator.session().design_document();
        assert!(document.point(points[0]).is_some());
        assert!(document.curve(curve).is_some());
        assert!(
            document
                .constraints()
                .iter()
                .any(|value| value.id == dependent_constraint)
        );
    }

    #[test]
    fn accepted_preview_session_has_coherent_distinct_provenance() {
        let (session, _, _, _) = fixed_line_session();
        let mut preview = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let preview_design = preview.design_identity();
        preview
            .reattempt(
                preview_design,
                preview.last_attempt().input().candidate_request(),
            )
            .expect("preview reattempt");
        let persisted_attempt = coordinator.session().last_attempt().identity();
        let preview_attempt = preview.last_attempt().identity();
        let preview_accepted = preview
            .accepted_state()
            .expect("preview accepted")
            .identity();
        assert_ne!(persisted_attempt, preview_attempt);

        coordinator
            .mark_solved_preview(&preview)
            .expect("accepted preview evidence");
        assert_eq!(
            coordinator.lifecycle(),
            LifecycleDto {
                status: LifecycleStatus::SolvedPreview,
                design: coordinator.session().design_identity(),
                attempt: persisted_attempt,
                preview_attempt: Some(preview_attempt),
                preview_accepted: Some(preview_accepted),
                accepted: coordinator
                    .session()
                    .accepted_state()
                    .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
                parent_accepted: coordinator
                    .session()
                    .last_attempt()
                    .parent_accepted_identity(),
            }
        );
        coordinator.mark_solving();
        assert_eq!(coordinator.lifecycle().status, LifecycleStatus::Solving);
        assert_eq!(coordinator.lifecycle().preview_attempt, None);
        assert_eq!(coordinator.lifecycle().preview_accepted, None);
        coordinator.clear_transient();
        assert_eq!(coordinator.lifecycle().preview_attempt, None);
    }

    #[test]
    fn coordinator_owns_projected_preview_solving_and_publication() {
        let (session, points, _, _) = fixed_line_session();
        let accepted = session.accepted_state().expect("accepted");
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.5,
        )
        .expect("scene");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Select);
        let start = viewport.model_to_screen([0.0, 0.0]);
        let target = viewport.model_to_screen([1.0, 1.0]);
        let pointer = |position| PointerInput {
            pointer_id: 9,
            position,
            modifiers: Modifiers::default(),
        };
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(start));
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(target));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("projected request");
        };
        assert_eq!(*point, points[0]);

        let effects = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *point,
            *model_position,
        );
        assert!(matches!(
            effects.as_slice(),
            [EditorEffect::PreviewPointMove {
                point,
                model_position,
            }] if *point == points[0] && *model_position == [0.0, 0.0]
        ));
        assert!(coordinator.solved_preview_session().is_some());
        assert_eq!(
            coordinator.lifecycle().status,
            LifecycleStatus::SolvedPreview
        );
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("fixed-point drag work");
        assert_eq!(work.attempts, 1);
        assert!(work.accepted);
        let locality = work.locality_plan().expect("fixed-point locality plan");
        assert_eq!(locality.point(), points[0]);
        assert_eq!(locality.hard_degrees_of_freedom(), 0);
        assert_eq!(locality.active_rank(), 0);
        assert_eq!(locality.passive_degrees_of_freedom(), 0);
        assert!(locality.anchors().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the complete pointer gesture, exact preview publication, release, and undo/redo lifecycle are one linear contract"
    )]
    fn pointer_drag_dispatch_commits_exact_preview_and_round_trips_undo_redo() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let point = document.add_point("free point", [0.0, 0.0]).expect("point");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("free-point session");
        let initial_accepted_json = session.export_accepted_json().unwrap();
        let accepted = session.accepted_state().expect("accepted");
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.5,
        )
        .expect("scene");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Select);
        let start = viewport.model_to_screen([0.0, 0.0]);
        let target_model = [1.25, -0.75];
        let target = viewport.model_to_screen(target_model);
        let pointer = |position| PointerInput {
            pointer_id: 101,
            position,
            modifiers: Modifiers::default(),
        };
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(start));
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(target));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point: requested_point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("projected request")
        };
        assert_eq!(*requested_point, point);
        let preview_effects = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *requested_point,
            *model_position,
        );
        assert!(matches!(
            preview_effects.as_slice(),
            [EditorEffect::PreviewPointMove {
                point: preview_point,
                model_position: preview_position,
            }] if *preview_point == point
                && (preview_position[0] - target_model[0]).abs() <= 1.0e-10
                && (preview_position[1] - target_model[1]).abs() <= 1.0e-10
        ));
        let release_effects =
            coordinator
                .editor_mut()
                .pointer_up(&scene, scene.design_identity, pointer(target));
        assert!(matches!(
            release_effects.as_slice(),
            [
                EditorEffect::CommitPointMove {
                    point: committed_point,
                    model_position: committed_position,
                    ..
                },
                EditorEffect::ClearPointPreview,
            ] if *committed_point == point
                && (committed_position[0] - target_model[0]).abs() <= 1.0e-10
                && (committed_position[1] - target_model[1]).abs() <= 1.0e-10
        ));
        for effect in &release_effects {
            coordinator
                .apply_editor_effect(effect)
                .expect("dispatch release effect");
        }
        let committed_json = coordinator.session().export_accepted_json().unwrap();
        assert_ne!(committed_json, initial_accepted_json);
        assert_eq!(coordinator.history_len(), 2);
        assert!(coordinator.solved_preview_session().is_none());
        assert!(coordinator.drag_continuation.is_none());
        assert!(coordinator.projected_drag_work_evidence().is_none());

        coordinator.undo().expect("undo drag");
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            initial_accepted_json
        );
        coordinator.redo().expect("redo drag");
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            committed_json
        );
        let redone = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .document()
            .point(point)
            .unwrap()
            .position;
        assert!((redone[0] - target_model[0]).hypot(redone[1] - target_model[1]) <= 1.0e-10);
    }

    #[test]
    fn rejected_pointer_drag_release_clears_coordinator_continuation_state() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let point = document.add_point("free point", [0.0, 0.0]).expect("point");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("free-point session");
        let initial_design_json = session.export_design_json().unwrap();
        let initial_accepted_json = session.export_accepted_json().unwrap();
        let accepted = session.accepted_state().expect("accepted");
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.5,
        )
        .expect("scene");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Select);
        let start = viewport.model_to_screen([0.0, 0.0]);
        let target = viewport.model_to_screen([1.0, 0.5]);
        let pointer = |position| PointerInput {
            pointer_id: 102,
            position,
            modifiers: Modifiers::default(),
        };
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(start));
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(target));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point: requested_point,
                ..
            },
        ] = request.as_slice()
        else {
            panic!("projected request")
        };
        assert_eq!(*requested_point, point);
        assert!(
            coordinator
                .resolve_projected_point_move(
                    *pointer_id,
                    *request_id,
                    *requested_point,
                    [f64::NAN, 0.0],
                )
                .is_empty()
        );
        assert!(coordinator.projected_drag_work_evidence().is_some());
        assert_eq!(
            coordinator
                .editor_mut()
                .pointer_up(&scene, scene.design_identity, pointer(target),),
            vec![EditorEffect::ClearPointPreview]
        );
        coordinator
            .apply_editor_effect(&EditorEffect::ClearPointPreview)
            .expect("dispatch terminal preview clear");
        assert!(coordinator.solved_preview_session().is_none());
        assert!(coordinator.drag_continuation.is_none());
        assert!(coordinator.projected_drag_work_evidence().is_none());
        assert_eq!(coordinator.history_len(), 1);
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            initial_design_json
        );
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            initial_accepted_json
        );
    }

    #[test]
    fn stale_projected_request_ids_are_zero_work_no_ops_on_the_frozen_gesture() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let _ = coordinator.resolve_projected_point_move(17, 5, points[0], [0.5, 0.5]);
        let first = coordinator
            .projected_drag_work_evidence()
            .expect("first sample")
            .clone();
        assert_eq!(first.attempts, 1);
        assert!(first.accepted);
        let preview = coordinator
            .solved_preview_session()
            .expect("accepted preview")
            .accepted_state()
            .expect("accepted preview state")
            .identity();
        let last_request_id = coordinator
            .drag_continuation
            .as_ref()
            .expect("frozen continuation")
            .last_request_id;

        for stale_request_id in [5, 4] {
            let effects = coordinator.resolve_projected_point_move(
                17,
                stale_request_id,
                points[0],
                [100.0, -100.0],
            );
            assert!(effects.is_empty());
            let work = coordinator
                .projected_drag_work_evidence()
                .expect("stale request evidence");
            assert_eq!(work.attempts, 0);
            assert!(!work.accepted);
            assert_eq!(
                work.rejection_stage,
                Some(ProjectedDragRejectionStage::RequestOrdering)
            );
            assert_eq!(work.operation.consumed, OperationWork::default());
            assert_eq!(work.locality, first.locality);
            assert_eq!(
                coordinator
                    .solved_preview_session()
                    .expect("retained preview")
                    .accepted_state()
                    .expect("retained preview state")
                    .identity(),
                preview
            );
            assert_eq!(
                coordinator
                    .drag_continuation
                    .as_ref()
                    .expect("retained continuation")
                    .last_request_id,
                last_request_id
            );
        }
    }

    #[test]
    fn non_finite_projected_targets_reject_before_planning_or_controlled_work() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let effects = coordinator.resolve_projected_point_move(18, 1, points[0], [f64::NAN, 0.0]);
        assert!(effects.is_empty());
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("input-error work evidence");
        assert_eq!(work.attempts, 0);
        assert!(!work.accepted);
        assert_eq!(
            work.rejection_stage,
            Some(ProjectedDragRejectionStage::AttemptInput)
        );
        assert_eq!(work.operation.consumed, OperationWork::default());
        assert!(work.solve.is_none());
        assert_eq!(
            work.locality_planning_failure(),
            Some(ProjectedDragLocalityPlanningFailure::InvalidPointerTarget)
        );
        assert!(coordinator.solved_preview_session().is_none());
    }

    #[test]
    fn projected_drag_default_policy_bounds_and_exhausts_rank_and_diagnostics() {
        let limits = projected_drag_control().limits;
        assert_eq!(limits.rank_kernels, PROJECTED_DRAG_MAX_RANK_KERNELS);
        assert_eq!(
            limits.diagnostic_candidates,
            PROJECTED_DRAG_MAX_DIAGNOSTIC_CANDIDATES
        );
        assert_eq!(
            limits.diagnostic_trials,
            PROJECTED_DRAG_MAX_DIAGNOSTIC_TRIALS
        );
        for (counter, checkpoint, limit) in [
            (
                OperationWorkCounter::RankKernels,
                OperationCheckpoint::BeforeRankKernel,
                limits.rank_kernels,
            ),
            (
                OperationWorkCounter::DiagnosticCandidates,
                OperationCheckpoint::DiagnosticCandidate,
                limits.diagnostic_candidates,
            ),
            (
                OperationWorkCounter::DiagnosticTrials,
                OperationCheckpoint::DiagnosticTrial,
                limits.diagnostic_trials,
            ),
        ] {
            assert_ne!(limit, usize::MAX);
            let mut controller = OperationController::new(projected_drag_control());
            controller
                .charge(counter, limit, checkpoint)
                .expect("work through the configured ceiling");
            assert_eq!(
                controller.charge(counter, 1, checkpoint),
                Err(OperationStopReason::WorkExhausted {
                    counter,
                    checkpoint,
                })
            );
            let report = controller.report();
            let consumed = match counter {
                OperationWorkCounter::RankKernels => report.consumed.rank_kernels,
                OperationWorkCounter::DiagnosticCandidates => report.consumed.diagnostic_candidates,
                OperationWorkCounter::DiagnosticTrials => report.consumed.diagnostic_trials,
                _ => unreachable!("the regression enumerates only rank and diagnostic counters"),
            };
            assert_eq!(consumed, limit);
            assert_eq!(
                report.stopping_reason,
                Some(OperationStopReason::WorkExhausted {
                    counter,
                    checkpoint,
                })
            );
        }
    }

    #[test]
    fn stale_locality_session_errors_retain_typed_zero_work_evidence() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let _ = coordinator.resolve_projected_point_move(18, 1, points[0], [0.0, 0.0]);
        assert!(
            coordinator
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted)
        );

        // Advance only the authoritative accepted stamp, deliberately leaving
        // the coordinator's private frozen plan stale. Locality validation
        // rejects before either retained API constructs its controller.
        let expected = coordinator.session.design_identity();
        let request = coordinator
            .session
            .last_attempt()
            .input()
            .candidate_request();
        coordinator
            .session
            .reattempt(expected, request)
            .expect("advance accepted stamp");
        let effects = coordinator.resolve_projected_point_move(18, 2, points[0], [0.0, 0.0]);
        assert!(effects.is_empty());
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("session-error work evidence");
        assert_eq!(work.attempts, 0);
        assert!(!work.accepted);
        assert_eq!(
            work.rejection_stage,
            Some(ProjectedDragRejectionStage::Session)
        );
        assert_eq!(work.operation.consumed, OperationWork::default());
        assert!(work.solve.is_none());
        assert!(work.locality_plan().is_some());
        assert!(coordinator.solved_preview_session().is_none());
        assert!(coordinator.drag_continuation.is_none());
    }

    #[test]
    fn oversized_drag_locality_fails_before_any_retained_solve_attempt() {
        let limit = OperationLimits::unlimited().dense_kernel_rows;
        let mut document = SketchDocument::new(1.0).expect("document");
        let point = document.add_point("start", [0.0, 0.0]).expect("point");
        let end = document.add_point("end", [1.0, 0.0]).expect("point");
        let curve = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: point,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        let hard_rows = limit + 1;
        for index in 0..hard_rows {
            document
                .add_constraint(
                    format!("horizontal {index}"),
                    DocumentConstraintDefinition::Horizontal {
                        line: CurveSpan { curve, segment: 0 },
                    },
                )
                .expect("horizontal");
        }
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted oversized document");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let effects = coordinator.resolve_projected_point_move(19, 1, point, [0.1, 0.1]);
        assert!(effects.is_empty());
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("planning-failure evidence");
        assert_eq!(work.attempts, 0);
        assert!(!work.accepted);
        assert_eq!(
            work.rejection_stage,
            Some(ProjectedDragRejectionStage::LocalityPlanning)
        );
        assert_eq!(
            work.operation.consumed,
            OperationWork {
                // Controlled validation establishes activity before inspecting the
                // hard-row envelope. This deliberately adversarial duplicate-source
                // document exhausts that bounded traversal first.
                document_validation_items: 1,
                document_dependency_items: PROJECTED_DRAG_MAX_LOCALITY_ITEMS,
                ..OperationWork::default()
            }
        );
        assert_eq!(
            work.operation.stopping_reason,
            Some(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DocumentDependencyItems,
                checkpoint: OperationCheckpoint::DocumentDependency,
            })
        );
        assert!(
            work.operation.consumed.document_validation_items
                <= work.operation.configured.document_validation_items
        );
        assert!(
            work.operation.consumed.document_dependency_items
                <= work.operation.configured.document_dependency_items
        );
        assert_eq!(work.operation.consumed.document_lowering_items, 0);
        assert_eq!(work.operation.consumed.nonlinear_iterations, 0);
        assert_eq!(work.operation.consumed.component_linearizations, 0);
        assert_eq!(work.operation.consumed.factorizations, 0);
        assert!(work.solve.is_none());
        assert_eq!(
            work.locality_planning_failure(),
            Some(ProjectedDragLocalityPlanningFailure::OperationStopped)
        );
        assert!(coordinator.solved_preview_session().is_none());
        assert!(coordinator.drag_continuation.is_none());
    }

    #[test]
    fn tangent_envelope_failure_keeps_its_typed_planning_evidence() {
        let limit = OperationLimits::unlimited().dense_kernel_columns;
        let error =
            DocumentSessionError::SketchSession(SketchSessionError::DragLocalityEnvelopeExceeded {
                active_tangent_dimensions: limit + 1,
                limit,
            });
        assert_eq!(
            projected_drag_locality_failure(&error),
            ProjectedDragLocalityPlanningFailure::InteractiveEnvelopeExceeded {
                active_tangent_dimensions: limit + 1,
                limit,
            }
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the fixture construction and zero-attempt locality rejection form one linear fail-closed regression"
    )]
    fn scalar_only_contact_freedom_fails_closed_before_a_drag_attempt() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let curve_start = document
            .add_point("curve start", [-2.0, 0.0])
            .expect("point");
        let curve_end = document.add_point("curve end", [2.0, 0.0]).expect("point");
        let pivot = document.add_point("pivot", [0.0, 2.0]).expect("point");
        let active = document.add_point("active", [2.0, 2.0]).expect("point");
        let reference = CurveSpan::line(
            document
                .add_curve(
                    "reference",
                    CurveDefinition::Line {
                        start: curve_start,
                        end: curve_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        );
        let moving = CurveSpan::line(
            document
                .add_curve(
                    "moving",
                    CurveDefinition::Line {
                        start: pivot,
                        end: active,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        );
        for (label, point, target) in [
            ("fix reference start", curve_start, [-2.0, 0.0]),
            ("fix reference end", curve_end, [2.0, 0.0]),
            ("fix pivot", pivot, [0.0, 2.0]),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("fixed point");
        }
        let length = document
            .add_scalar(
                "moving length",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("length scalar");
        document
            .add_dimension(
                "moving length",
                DocumentDimensionDefinition::CurveLength {
                    curve: moving,
                    target: length,
                },
                DocumentDimensionMode::Driving,
            )
            .expect("length dimension");
        let contact = document
            .add_curve_contact(
                "straight contact",
                reference,
                0.5,
                0,
                ContactNeighborhood::Interior,
                Some(TangentOrientation::Aligned),
            )
            .expect("contact");
        document
            .add_constraint(
                "constant straight tangent",
                DocumentConstraintDefinition::CurveDirection {
                    line: moving,
                    curve_contact: contact,
                    relation: DocumentCurveDirectionRelation::Tangent {
                        orientation: TangentOrientation::Aligned,
                    },
                },
            )
            .expect("curve direction");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted scalar-freedom document");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let effects = coordinator.resolve_projected_point_move(20, 1, active, [1.9, 2.1]);
        assert!(effects.is_empty());
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("incomplete-cover evidence");
        assert_eq!(work.attempts, 0);
        assert_eq!(
            work.rejection_stage,
            Some(ProjectedDragRejectionStage::LocalityPlanning)
        );
        assert!(work.operation.consumed.document_dependency_items > 0);
        assert_eq!(work.operation.consumed.component_linearizations, 1);
        assert_eq!(work.operation.consumed.rank_kernels, 2);
        assert_eq!(work.operation.consumed.factorizations, 0);
        assert_eq!(work.operation.consumed.nonlinear_iterations, 0);
        assert_eq!(work.operation.stopping_reason, None);
        assert_eq!(
            work.locality_planning_failure(),
            Some(
                ProjectedDragLocalityPlanningFailure::IncompleteAnchorCover {
                    required: 1,
                    spanned: 0,
                }
            )
        );
        assert!(coordinator.solved_preview_session().is_none());
        assert!(coordinator.drag_continuation.is_none());
    }

    fn motion_cam_roller_center(parameter: f64) -> [f64; 2] {
        let tangent = [8.0, 8.0 - 16.0 * parameter];
        let tangent_norm = tangent[0].hypot(tangent[1]);
        [
            -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
            8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
        ]
    }

    fn nearest_motion_cam_roller_center(target: [f64; 2], left_branch: bool) -> [f64; 2] {
        fn cost(parameter: f64, target: [f64; 2]) -> f64 {
            let center = motion_cam_roller_center(parameter);
            (center[0] - target[0]).mul_add(
                center[0] - target[0],
                (center[1] - target[1]) * (center[1] - target[1]),
            )
        }

        let (mut lower, mut upper) = if left_branch { (0.0, 0.5) } else { (0.5, 1.0) };
        let ratio = (5.0_f64.sqrt() - 1.0) * 0.5;
        let mut left = upper - ratio * (upper - lower);
        let mut right = lower + ratio * (upper - lower);
        let mut left_cost = cost(left, target);
        let mut right_cost = cost(right, target);
        for _ in 0..96 {
            if left_cost <= right_cost {
                upper = right;
                right = left;
                right_cost = left_cost;
                left = upper - ratio * (upper - lower);
                left_cost = cost(left, target);
            } else {
                lower = left;
                left = right;
                left_cost = right_cost;
                right = lower + ratio * (upper - lower);
                right_cost = cost(right, target);
            }
        }
        motion_cam_roller_center((lower + upper) * 0.5)
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn projected_drag_preserves_independent_freedoms_without_passive_retries() {
        let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("cam sample");
        let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
            panic!("cam persistent roles");
        };
        let session = RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .expect("cam session");
        let baseline_design = session.design_identity();
        let baseline_accepted = session.accepted_state().expect("accepted cam").identity();
        let accepted = session.accepted_state().expect("accepted cam").document();
        let right_before = accepted
            .point(ids.right_center)
            .expect("right roller")
            .position;
        let parameter = 0.26;
        let tangent: [f64; 2] = [8.0, 8.0 - 16.0 * parameter];
        let tangent_norm = tangent[0].hypot(tangent[1]);
        let left_target = [
            -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
            8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
        ];
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let _ = coordinator.resolve_projected_point_move(41, 1, ids.left_center, left_target);
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("first drag work");
        assert_eq!(
            work.operation.configured.document_validation_items,
            PROJECTED_DRAG_MAX_VALIDATION_ITEMS
        );
        assert_eq!(
            work.operation.configured.document_lowering_items,
            PROJECTED_DRAG_MAX_LOWERING_ITEMS
        );
        assert!(
            work.operation.consumed.document_validation_items > 0,
            "gesture-start preflight must validate before the preview clone"
        );
        assert!(
            work.operation.consumed.document_lowering_items > 0,
            "the retained preview lowering must remain in the same finite vector"
        );
        assert_eq!(work.attempts, 1);
        assert!(!work.continued);
        assert!(work.accepted);
        let locality = work.locality_plan().expect("first-gesture locality plan");
        assert_eq!(locality.design_identity(), baseline_design);
        assert_eq!(locality.accepted_state_identity(), baseline_accepted);
        assert_eq!(locality.point(), ids.left_center);
        assert_eq!(locality.passive_degrees_of_freedom(), 1);
        assert_eq!(locality.anchors().len(), 1);
        assert_eq!(locality.anchors()[0].point(), ids.right_center);
        assert_eq!(
            locality.anchors()[0].target().map(f64::to_bits),
            right_before.map(f64::to_bits)
        );
        let preview = coordinator
            .solved_preview_session()
            .expect("accepted projected preview");
        let preview_document = preview
            .accepted_state()
            .expect("preview accepted")
            .document();
        let right_preview = preview_document
            .point(ids.right_center)
            .expect("right roller")
            .position;
        assert!(
            (right_preview[0] - right_before[0]).hypot(right_preview[1] - right_before[1])
                <= 1.0e-8,
            "passive roller moved from {right_before:?} to {right_preview:?}"
        );
        assert_eq!(
            preview
                .accepted_state()
                .expect("accepted preview")
                .solve_result()
                .unstable_core_report()
                .right_nullity,
            2
        );

        let parameter = 0.28;
        let tangent: [f64; 2] = [8.0, 8.0 - 16.0 * parameter];
        let tangent_norm = tangent[0].hypot(tangent[1]);
        let continued_target = [
            -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
            8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
        ];
        let _ = coordinator.resolve_projected_point_move(41, 2, ids.left_center, continued_target);
        let continued_work = coordinator
            .projected_drag_work_evidence()
            .expect("continued drag work")
            .clone();
        assert_eq!(continued_work.attempts, 1);
        assert!(continued_work.continued);
        assert!(continued_work.accepted);
        assert!(continued_work.operation.consumed.factorizations > 0);
        let retained_preview_identity = coordinator
            .solved_preview_session()
            .unwrap()
            .accepted_state()
            .unwrap()
            .identity();

        let _ = coordinator.resolve_projected_point_move(41, 3, ids.left_center, [f64::NAN, 0.0]);
        let failed_work = coordinator
            .projected_drag_work_evidence()
            .expect("failed drag work");
        assert_eq!(failed_work.attempts, 0);
        assert!(failed_work.continued);
        assert!(!failed_work.accepted);
        assert_eq!(
            failed_work.rejection_stage,
            Some(ProjectedDragRejectionStage::AttemptInput)
        );
        assert_eq!(failed_work.operation.consumed, OperationWork::default());
        assert_eq!(
            coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .identity(),
            retained_preview_identity,
            "a rejected sample must retain the last valid preview"
        );

        let parameter = 0.30;
        let tangent: [f64; 2] = [8.0, 8.0 - 16.0 * parameter];
        let tangent_norm = tangent[0].hypot(tangent[1]);
        let recovered_target = [
            -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
            8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
        ];
        let _ = coordinator.resolve_projected_point_move(41, 4, ids.left_center, recovered_target);
        let recovered_work = coordinator
            .projected_drag_work_evidence()
            .expect("recovered drag work");
        assert_eq!(recovered_work.attempts, 1);
        assert!(recovered_work.continued);
        assert!(recovered_work.accepted);

        let left_preview = coordinator
            .solved_preview_session()
            .unwrap()
            .accepted_state()
            .unwrap()
            .document()
            .point(ids.left_center)
            .unwrap()
            .position;
        let commit_effect = EditorEffect::CommitPointMove {
            expected: coordinator.session().design_identity(),
            point: ids.left_center,
            model_position: left_preview,
        };
        let before_stopped_release = (
            coordinator.session().design_identity(),
            coordinator.session().last_attempt().identity(),
            coordinator.session().accepted_state().unwrap().identity(),
            coordinator.session().export_design_json().unwrap(),
            coordinator.session().export_accepted_json().unwrap(),
            coordinator.history_len(),
            coordinator.history_cursor(),
            coordinator.transcript().len(),
            coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .identity(),
            coordinator.solved_preview_locality.clone(),
        );
        let mut exhausted_release = projected_drag_control();
        exhausted_release.limits.document_validation_items = 0;
        assert!(matches!(
            coordinator.apply_editor_effect_with_point_publication_control(
                &commit_effect,
                exhausted_release,
            ),
            Err(CoordinatorError::OperationStopped {
                context,
                report,
            }) if context == "point-move publication"
                && matches!(
                    report.stopping_reason,
                    Some(OperationStopReason::WorkExhausted {
                        counter: OperationWorkCounter::DocumentValidationItems,
                        ..
                    })
                )
        ));
        assert_eq!(
            coordinator.session().design_identity(),
            before_stopped_release.0
        );
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            before_stopped_release.1
        );
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            before_stopped_release.2
        );
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            before_stopped_release.3
        );
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            before_stopped_release.4
        );
        assert_eq!(coordinator.history_len(), before_stopped_release.5);
        assert_eq!(coordinator.history_cursor(), before_stopped_release.6);
        assert_eq!(coordinator.transcript().len(), before_stopped_release.7);
        assert_eq!(
            coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .identity(),
            before_stopped_release.8
        );
        assert_eq!(
            coordinator.solved_preview_locality,
            before_stopped_release.9
        );
        let mut exhausted_rank = projected_drag_control();
        exhausted_rank.limits.rank_kernels = 0;
        assert!(matches!(
            coordinator.apply_editor_effect_with_point_publication_control(
                &commit_effect,
                exhausted_rank,
            ),
            Err(CoordinatorError::OperationStopped {
                context,
                report,
            }) if context == "point-move publication"
                && matches!(
                    report.stopping_reason,
                    Some(OperationStopReason::WorkExhausted {
                        counter: OperationWorkCounter::RankKernels,
                        ..
                    })
                )
        ));
        assert_eq!(
            coordinator.session().design_identity(),
            before_stopped_release.0
        );
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            before_stopped_release.1
        );
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            before_stopped_release.2
        );
        assert_eq!(coordinator.history_len(), before_stopped_release.5);
        assert_eq!(coordinator.history_cursor(), before_stopped_release.6);
        assert_eq!(coordinator.transcript().len(), before_stopped_release.7);
        assert_eq!(
            coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .identity(),
            before_stopped_release.8
        );
        coordinator
            .apply_editor_effect(&commit_effect)
            .expect("commit projected drag")
            .expect("retained mutation");
        let right_after_release = coordinator
            .session()
            .accepted_state()
            .expect("accepted retained state")
            .document()
            .point(ids.right_center)
            .expect("right roller")
            .position;
        assert!(
            (right_after_release[0] - right_before[0])
                .hypot(right_after_release[1] - right_before[1])
                <= 1.0e-8,
            "releasing the active roller moved the passive roller from {right_before:?} \
             to {right_after_release:?}"
        );
        let retained_request = coordinator
            .session()
            .last_attempt()
            .input()
            .publication_request();
        assert_eq!(retained_request.drag, None);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted retained state")
                .solve_result()
                .unstable_core_report()
                .right_nullity,
            2
        );
        assert!(coordinator.projected_drag_work_evidence().is_none());
        assert!(coordinator.drag_continuation.is_none());

        let left_before_second_drag = coordinator
            .session()
            .accepted_state()
            .expect("accepted commit")
            .document()
            .point(ids.left_center)
            .expect("left roller")
            .position;
        let parameter = 0.74;
        let tangent: [f64; 2] = [8.0, 8.0 - 16.0 * parameter];
        let tangent_norm = tangent[0].hypot(tangent[1]);
        let right_target = [
            -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
            8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
        ];
        let _ = coordinator.resolve_projected_point_move(42, 2, ids.right_center, right_target);
        let second_work = coordinator
            .projected_drag_work_evidence()
            .expect("second gesture work");
        let locality = second_work
            .locality_plan()
            .expect("second-gesture locality plan");
        assert_eq!(locality.point(), ids.right_center);
        assert_eq!(locality.anchors().len(), 1);
        assert_eq!(locality.anchors()[0].point(), ids.left_center);
        assert_eq!(
            locality.anchors()[0].target().map(f64::to_bits),
            left_before_second_drag.map(f64::to_bits),
            "a new gesture must capture the current accepted visible baseline"
        );
        let second_preview = coordinator
            .solved_preview_session()
            .expect("second accepted preview")
            .accepted_state()
            .expect("second preview accepted")
            .document();
        let left_after = second_preview
            .point(ids.left_center)
            .expect("left roller")
            .position;
        let right_after = second_preview
            .point(ids.right_center)
            .expect("right roller")
            .position;
        assert!(
            (left_after[0] - left_before_second_drag[0])
                .hypot(left_after[1] - left_before_second_drag[1])
                <= 1.0e-8,
            "first control moved while independently dragging the second"
        );
        coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: coordinator.session().design_identity(),
                point: ids.right_center,
                model_position: right_after,
            })
            .expect("commit second projected drag")
            .expect("retained second mutation");
        let left_after_second_release = coordinator
            .session()
            .accepted_state()
            .expect("accepted second release")
            .document()
            .point(ids.left_center)
            .expect("left roller")
            .position;
        assert!(
            (left_after_second_release[0] - left_before_second_drag[0])
                .hypot(left_after_second_release[1] - left_before_second_drag[1])
                <= 1.0e-8,
            "releasing the second roller moved the first from {left_before_second_drag:?} \
             to {left_after_second_release:?}"
        );
        assert!(coordinator.projected_drag_work_evidence().is_none());
        assert!(coordinator.drag_continuation.is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn natural_twin_roller_paths_keep_the_passive_roller_at_the_gesture_baseline() {
        const LOCAL_PATH: [[f64; 2]; 8] = [
            [0.04, 0.0],
            [0.0, 0.04],
            [-0.04, 0.0],
            [0.0, -0.04],
            [0.03, 0.025],
            [-0.03, -0.025],
            [0.02, 0.0],
            [-0.02, 0.0],
        ];

        for (pointer_id, drag_left) in [(43, true), (44, false)] {
            let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("cam sample");
            let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
                panic!("cam persistent roles");
            };
            let session = RetainedSketchDocumentSession::new(
                fixture.document,
                fixture.request,
                SolverConfig::default(),
            )
            .expect("cam session");
            let baseline_design = session.design_identity();
            let baseline_accepted = session.accepted_state().expect("accepted cam").identity();
            let accepted = session.accepted_state().expect("accepted cam").document();
            let (active, passive) = if drag_left {
                (ids.left_center, ids.right_center)
            } else {
                (ids.right_center, ids.left_center)
            };
            let active_baseline = accepted.point(active).expect("active roller").position;
            let passive_baseline = accepted.point(passive).expect("passive roller").position;
            let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
            let mut largest_active_motion = 0.0_f64;
            let mut previous_active = active_baseline;
            let mut previous_oracle = active_baseline;

            for (index, delta) in LOCAL_PATH.into_iter().enumerate() {
                let target = [active_baseline[0] + delta[0], active_baseline[1] + delta[1]];
                let nearest = nearest_motion_cam_roller_center(target, drag_left);
                let _ = coordinator.resolve_projected_point_move(
                    pointer_id,
                    u64::try_from(index + 1).expect("request id"),
                    active,
                    target,
                );
                let work = coordinator
                    .projected_drag_work_evidence()
                    .expect("projected drag work")
                    .clone();
                assert_eq!(work.attempts, 1, "{work:#?}");
                assert_eq!(work.continued, index > 0, "{work:#?}");
                assert!(work.accepted, "{work:#?}");
                let locality = work.locality_plan().expect("gesture locality plan");
                assert_eq!(locality.design_identity(), baseline_design);
                assert_eq!(locality.accepted_state_identity(), baseline_accepted);
                assert_eq!(locality.point(), active);
                assert_eq!(locality.anchors().len(), 1);
                assert_eq!(locality.anchors()[0].point(), passive);
                assert_eq!(
                    locality.anchors()[0].target().map(f64::to_bits),
                    passive_baseline.map(f64::to_bits),
                    "gesture locality changed on sample {index}: {work:#?}"
                );
                assert!(
                    work.operation.consumed.factorizations <= PROJECTED_DRAG_MAX_FACTORIZATIONS
                        && work.operation.consumed.nonlinear_iterations
                            <= PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS,
                    "{work:#?}"
                );

                let preview = coordinator
                    .solved_preview_session()
                    .expect("accepted preview")
                    .accepted_state()
                    .expect("accepted preview state");
                let active_position = preview
                    .document()
                    .point(active)
                    .expect("active roller")
                    .position;
                let passive_position = preview
                    .document()
                    .point(passive)
                    .expect("passive roller")
                    .position;
                let accepted_target_distance =
                    (active_position[0] - target[0]).hypot(active_position[1] - target[1]);
                let nearest_target_distance =
                    (nearest[0] - target[0]).hypot(nearest[1] - target[1]);
                assert!(
                    (active_position[0] - nearest[0]).hypot(active_position[1] - nearest[1])
                        <= 2.0e-5,
                    "dragging {active:?} did not select the nearest local cam-offset point: \
                     target={target:?}, expected={nearest:?}, accepted={active_position:?}"
                );
                assert!(
                    accepted_target_distance <= nearest_target_distance + 2.0e-5,
                    "dragging {active:?} did not minimize cursor distance on its local branch: \
                     expected distance {nearest_target_distance:e}, accepted distance \
                     {accepted_target_distance:e}"
                );
                let active_step = (active_position[0] - previous_active[0])
                    .hypot(active_position[1] - previous_active[1]);
                let oracle_step =
                    (nearest[0] - previous_oracle[0]).hypot(nearest[1] - previous_oracle[1]);
                assert!(
                    active_step <= oracle_step + 4.0e-5,
                    "dragging {active:?} jumped farther than the nearest local continuation: \
                     actual step {active_step:e}, oracle step {oracle_step:e}"
                );
                largest_active_motion = largest_active_motion.max(
                    (active_position[0] - active_baseline[0])
                        .hypot(active_position[1] - active_baseline[1]),
                );
                assert!(
                    (passive_position[0] - passive_baseline[0])
                        .hypot(passive_position[1] - passive_baseline[1])
                        <= 1.0e-8,
                    "dragging {active:?} moved passive {passive:?} away from the authoritative \
                     gesture baseline {passive_baseline:?} to {passive_position:?}"
                );
                assert_eq!(
                    preview.solve_result().unstable_core_report().right_nullity,
                    2
                );

                let runtime_passive = preview
                    .mappings()
                    .runtime_point(passive)
                    .expect("passive runtime point");
                let solve = preview.solve_result();
                let source = solve
                    .source_mappings
                    .iter()
                    .find(|mapping| mapping.source == SketchSource::PreviousState(runtime_passive))
                    .expect("passive previous-state source");
                let core_source = source
                    .core_source_id
                    .expect("previous-state source has core rows");
                let audit = solve
                    .display_audit
                    .sources
                    .iter()
                    .find(|candidate| candidate.source_id == core_source)
                    .expect("passive previous-state audit");
                assert_eq!(audit.rows.len(), 2);
                let mut audited_cost = 0.0;
                let mut geometry_cost = 0.0;
                for row in &audit.rows {
                    assert!(row.row_in_block < 2, "{row:#?}");
                    let coordinate = row.row_in_block;
                    let audited_target = passive_position[coordinate] - row.raw_residual;
                    assert!(
                        (audited_target - passive_baseline[coordinate]).abs() <= 1.0e-12,
                        "passive previous-state target changed from {passive_baseline:?}: \
                         {audit:#?}"
                    );
                    let geometry_residual =
                        (passive_position[coordinate] - passive_baseline[coordinate]) / row.scale;
                    assert!(
                        (row.normalized_residual - geometry_residual).abs() <= 1.0e-15,
                        "passive audit does not match returned geometry: {audit:#?}"
                    );
                    audited_cost += 0.5 * row.normalized_residual * row.normalized_residual;
                    geometry_cost += 0.5 * geometry_residual * geometry_residual;
                }
                assert!(
                    (audited_cost - geometry_cost).abs() <= 1.0e-24,
                    "passive Preference cost does not match returned geometry: \
                    audited={audited_cost} geometry={geometry_cost}"
                );
                previous_active = active_position;
                previous_oracle = nearest;
            }
            assert!(
                largest_active_motion > 1.0e-3,
                "the active roller never responded to the natural cursor path"
            );
            coordinator
                .apply_editor_effect(&EditorEffect::CommitPointMove {
                    expected: coordinator.session().design_identity(),
                    point: active,
                    model_position: previous_active,
                })
                .expect("commit natural off-manifold roller path")
                .expect("retained roller mutation");
            let published = coordinator
                .session()
                .accepted_state()
                .expect("accepted natural-path release");
            let published_active = published
                .document()
                .point(active)
                .expect("published active roller")
                .position;
            let published_passive = published
                .document()
                .point(passive)
                .expect("published passive roller")
                .position;
            assert!(
                (published_active[0] - previous_active[0])
                    .hypot(published_active[1] - previous_active[1])
                    <= 1.0e-10,
                "release changed the accepted active preview from {previous_active:?} to \
                 {published_active:?}"
            );
            assert!(
                (published_passive[0] - passive_baseline[0])
                    .hypot(published_passive[1] - passive_baseline[1])
                    <= 1.0e-8,
                "release moved passive {passive:?} away from the gesture baseline \
                 {passive_baseline:?} to {published_passive:?}"
            );
            assert_eq!(
                published
                    .solve_result()
                    .unstable_core_report()
                    .right_nullity,
                2
            );
            assert!(coordinator.projected_drag_work_evidence().is_none());
            assert!(coordinator.drag_continuation.is_none());
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn motion_cam_circumference_gesture_publishes_the_exact_visible_preview() {
        const POINTER_ID: u64 = 45;
        const LOCAL_PATH: [[f64; 2]; 2] = [[0.04, 0.0], [0.03, 0.025]];
        const PASSIVE_TOLERANCE: f64 = 1.0e-8;
        const PARAMETER_STEP_LIMIT: f64 = 0.5;

        let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("cam sample");
        let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
            panic!("cam persistent roles");
        };
        let session = RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .expect("cam session");
        let initial_accepted_json = session.export_accepted_json().expect("accepted JSON");
        let initial_accepted = session.accepted_state().expect("accepted cam");
        let initial_document = initial_accepted.document().clone();
        let initial_design = session.design_identity();
        let initial_accepted_identity = initial_accepted.identity();
        let active_baseline = initial_document
            .point(ids.left_center)
            .expect("left roller")
            .position;
        let passive_baseline = initial_document
            .point(ids.right_center)
            .expect("right roller")
            .position;
        let contact_metadata = initial_document.contacts().to_vec();
        let contact_values = |document: &SketchDocument| {
            contact_metadata
                .iter()
                .map(|contact| {
                    (
                        contact.id,
                        document
                            .scalar(contact.parameter)
                            .expect("contact parameter")
                            .value,
                    )
                })
                .collect::<Vec<_>>()
        };
        let initial_contact_values = contact_values(&initial_document);
        let passive_contacts = initial_document
            .constraints()
            .iter()
            .find_map(|constraint| {
                let DocumentConstraintDefinition::CurveCurveTangency {
                    first_contact,
                    second_contact,
                } = constraint.definition
                else {
                    return None;
                };
                let contacts = [first_contact, second_contact];
                contacts
                    .iter()
                    .any(|contact| {
                        initial_document
                            .contact(*contact)
                            .is_some_and(|slot| slot.curve.curve == ids.right_circle)
                    })
                    .then_some(contacts)
            })
            .expect("right-roller tangency contacts");
        let curve_definitions = [ids.cam, ids.left_circle, ids.right_circle].map(|curve| {
            initial_document
                .curve(curve)
                .expect("motion-cam curve")
                .definition
                .clone()
        });

        let viewport =
            Viewport::new([1000.0, 700.0], [0.0, 2.0], 100.0).expect("motion-cam viewport");
        let mut scene = EditorScene::from_accepted_for_design(
            initial_accepted.identity().revision().get(),
            session.design_identity(),
            &initial_document,
            session.design_document(),
            viewport,
            0.25,
        )
        .expect("initial motion-cam scene");
        let initial_scene_points = scene.points.clone();
        let initial_scene_curves = scene.curves.clone();

        // Use the top of the left circumference: it is far from the center,
        // the positive-X radius annotation, and the cam contact near the lower-right arc.
        let circumference_model = [active_baseline[0], active_baseline[1] + 1.0];
        let circumference = viewport.model_to_screen(circumference_model);
        let expected_curve = SelectionItem::Curve(CurveSpan {
            curve: ids.left_circle,
            segment: 0,
        });
        let hit = scene
            .hit_test(circumference, PickTolerance::default())
            .expect("left circumference hit");
        assert_eq!(hit.item, expected_curve);
        assert!(
            scene
                .annotation_hit_test(circumference, PickTolerance::default(), &[], None, &[],)
                .is_none(),
            "the gesture origin must exercise geometry picking rather than an annotation"
        );

        let pointer = |position| PointerInput {
            pointer_id: POINTER_ID,
            position,
            modifiers: Modifiers::default(),
        };
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Select);
        assert_eq!(
            coordinator
                .editor_mut()
                .pointer_down(&scene, pointer(circumference)),
            vec![EditorEffect::SelectionChanged(vec![expected_curve])]
        );
        assert_eq!(coordinator.editor().selection(), &[expected_curve]);

        let mut frozen_locality = None;
        let mut previous_contact_values = initial_contact_values.clone();
        let mut final_pointer = circumference;
        let mut final_preview_document = None;
        let mut final_scene_points = None;
        let mut final_scene_curves = None;

        for (index, delta) in LOCAL_PATH.into_iter().enumerate() {
            final_pointer = viewport.model_to_screen([
                circumference_model[0] + delta[0],
                circumference_model[1] + delta[1],
            ]);
            let request = coordinator
                .editor_mut()
                .pointer_move(&scene, pointer(final_pointer));
            let [
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point,
                    model_position,
                },
            ] = request.as_slice()
            else {
                panic!("one projected move request for sample {index}: {request:#?}");
            };
            assert_eq!(*pointer_id, POINTER_ID);
            assert_eq!(*point, ids.left_center);
            assert!(
                (model_position[0] - (active_baseline[0] + delta[0]))
                    .hypot(model_position[1] - (active_baseline[1] + delta[1]))
                    <= 1.0e-12,
                "circumference offset was not preserved on sample {index}: {request:#?}"
            );

            let preview_effects = coordinator.resolve_projected_point_move(
                *pointer_id,
                *request_id,
                *point,
                *model_position,
            );
            assert!(matches!(
                preview_effects.as_slice(),
                [EditorEffect::PreviewPointMove {
                    point: preview_point,
                    ..
                }] if *preview_point == ids.left_center
            ));
            let work = coordinator
                .projected_drag_work_evidence()
                .expect("projected drag work");
            assert_eq!(work.attempts, 1, "{work:#?}");
            assert_eq!(work.continued, index > 0, "{work:#?}");
            assert!(work.accepted, "{work:#?}");
            let locality = work.locality_plan().expect("motion-cam gesture locality");
            assert_eq!(locality.design_identity(), initial_design);
            assert_eq!(
                locality.accepted_state_identity(),
                initial_accepted_identity
            );
            assert_eq!(locality.point(), ids.left_center);
            assert_eq!(locality.anchors().len(), 1);
            assert_eq!(locality.anchors()[0].point(), ids.right_center);
            assert_eq!(
                locality.anchors()[0].target().map(f64::to_bits),
                passive_baseline.map(f64::to_bits)
            );
            if let Some(expected) = &frozen_locality {
                assert_eq!(
                    locality, expected,
                    "gesture locality changed on sample {index}"
                );
            } else {
                frozen_locality = Some(locality.clone());
            }

            let preview_session = coordinator
                .visible_preview_session()
                .expect("visible accepted preview");
            let preview = preview_session
                .accepted_state()
                .expect("accepted visible preview");
            let preview_document = preview.document();
            let passive = preview_document
                .point(ids.right_center)
                .expect("passive roller")
                .position;
            assert!(
                (passive[0] - passive_baseline[0]).hypot(passive[1] - passive_baseline[1])
                    <= PASSIVE_TOLERANCE,
                "passive roller moved on sample {index}: baseline={passive_baseline:?}, \
                 preview={passive:?}"
            );
            assert_eq!(
                preview_document.contacts(),
                contact_metadata.as_slice(),
                "persistent contact branch metadata changed on sample {index}"
            );
            assert_eq!(
                [ids.cam, ids.left_circle, ids.right_circle].map(|curve| {
                    preview_document
                        .curve(curve)
                        .expect("preview motion-cam curve")
                        .definition
                        .clone()
                }),
                curve_definitions,
                "curve branch definitions changed on sample {index}"
            );

            let current_contact_values = contact_values(preview_document);
            for ((contact, previous), (current_contact, current)) in
                previous_contact_values.iter().zip(&current_contact_values)
            {
                assert_eq!(contact, current_contact);
                assert!(
                    (current - previous).abs() <= PARAMETER_STEP_LIMIT,
                    "contact {contact:?} jumped from {previous} to {current} on sample {index}"
                );
            }
            for passive_contact in passive_contacts {
                let baseline = initial_contact_values
                    .iter()
                    .find_map(|(contact, value)| (*contact == passive_contact).then_some(*value))
                    .expect("baseline passive contact parameter");
                let current = current_contact_values
                    .iter()
                    .find_map(|(contact, value)| (*contact == passive_contact).then_some(*value))
                    .expect("preview passive contact parameter");
                assert!(
                    (current - baseline).abs() <= PASSIVE_TOLERANCE,
                    "passive contact {passive_contact:?} moved from {baseline} to {current} \
                     on sample {index}"
                );
            }
            previous_contact_values = current_contact_values;

            scene = EditorScene::from_accepted_for_design(
                preview.identity().revision().get(),
                preview_session.design_identity(),
                preview_document,
                preview_session.design_document(),
                viewport,
                0.25,
            )
            .expect("scene rebuilt from visible accepted preview");
            final_scene_points = Some(scene.points.clone());
            final_scene_curves = Some(scene.curves.clone());
            final_preview_document = Some(preview_document.clone());
        }

        let final_preview_document = final_preview_document.expect("final accepted preview");
        let final_scene_points = final_scene_points.expect("final preview points");
        let final_scene_curves = final_scene_curves.expect("final preview curves");
        let final_active = final_preview_document
            .point(ids.left_center)
            .expect("final active roller")
            .position;
        let release_effects = coordinator.editor_mut().pointer_up(
            &scene,
            scene.design_identity,
            pointer(final_pointer),
        );
        assert!(matches!(
            release_effects.as_slice(),
            [
                EditorEffect::CommitPointMove {
                    point,
                    model_position,
                    ..
                },
                EditorEffect::ClearPointPreview,
            ] if *point == ids.left_center
                && model_position.map(f64::to_bits) == final_active.map(f64::to_bits)
        ));
        coordinator
            .apply_editor_effect(&release_effects[0])
            .expect("dispatch exact preview release")
            .expect("retained point mutation");
        assert!(matches!(
            release_effects[1],
            EditorEffect::ClearPointPreview
        ));
        coordinator
            .apply_editor_effect(&release_effects[1])
            .expect("dispatch release disposition");

        let published = coordinator
            .session()
            .accepted_state()
            .expect("accepted release");
        assert_eq!(
            published.document(),
            &final_preview_document,
            "release did not publish the exact visible accepted preview"
        );
        assert_eq!(
            published
                .document()
                .point(ids.left_center)
                .expect("published active roller")
                .position
                .map(f64::to_bits),
            final_active.map(f64::to_bits)
        );
        let published_passive = published
            .document()
            .point(ids.right_center)
            .expect("published passive roller")
            .position;
        assert!(
            (published_passive[0] - passive_baseline[0])
                .hypot(published_passive[1] - passive_baseline[1])
                <= PASSIVE_TOLERANCE
        );
        assert_eq!(published.document().contacts(), contact_metadata.as_slice());
        assert_eq!(
            coordinator
                .session()
                .last_attempt()
                .input()
                .publication_request()
                .drag,
            None
        );
        assert_eq!(coordinator.history_len(), 2);
        assert_eq!(coordinator.history_cursor(), 1);
        assert!(coordinator.solved_preview_session().is_none());
        assert!(coordinator.projected_drag_work_evidence().is_none());
        let committed_json = coordinator
            .session()
            .export_accepted_json()
            .expect("committed accepted JSON");

        coordinator.undo().expect("undo circumference drag");
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("undone accepted JSON"),
            initial_accepted_json
        );
        let undone = coordinator
            .session()
            .accepted_state()
            .expect("undone accepted state");
        let undone_scene = EditorScene::from_accepted_for_design(
            undone.identity().revision().get(),
            coordinator.session().design_identity(),
            undone.document(),
            coordinator.session().design_document(),
            viewport,
            0.25,
        )
        .expect("undone scene");
        assert_eq!(undone_scene.points, initial_scene_points);
        assert_eq!(undone_scene.curves, initial_scene_curves);

        coordinator.redo().expect("redo circumference drag");
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("redone accepted JSON"),
            committed_json
        );
        let redone = coordinator
            .session()
            .accepted_state()
            .expect("redone accepted state");
        let redone_scene = EditorScene::from_accepted_for_design(
            redone.identity().revision().get(),
            coordinator.session().design_identity(),
            redone.document(),
            coordinator.session().design_document(),
            viewport,
            0.25,
        )
        .expect("redone scene");
        assert_eq!(redone_scene.points, final_scene_points);
        assert_eq!(redone_scene.curves, final_scene_curves);
        assert_eq!(redone.document(), &final_preview_document);
    }

    fn alternate_branch_fixture() -> (RetainedEditorCoordinator, DesignPointId, [CurveId; 2]) {
        let mut document = SketchDocument::new(1.0).expect("document");
        let base = document.add_point("base", [-2.0, 0.0]).expect("point");
        let elbow = document.add_point("elbow", [0.0, 1.5]).expect("point");
        let end = document.add_point("end", [2.0, 0.0]).expect("point");
        let first = document
            .add_curve(
                "first link",
                CurveDefinition::Line {
                    start: base,
                    end: elbow,
                    branch_direction: [0.8, 0.6],
                },
            )
            .expect("line");
        let second = document
            .add_curve(
                "second link",
                CurveDefinition::Line {
                    start: elbow,
                    end,
                    branch_direction: [0.8, -0.6],
                },
            )
            .expect("line");
        for (label, point, target) in [
            ("fixed base", base, [-2.0, 0.0]),
            ("fixed end", end, [2.0, 0.0]),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("fixed point");
        }
        for (label, first_point, second_point) in
            [("first length", base, elbow), ("second length", elbow, end)]
        {
            let target = document
                .add_scalar(label, 2.5, ScalarUnit::Length, ScalarDomain::Positive)
                .expect("length target");
            document
                .add_dimension(
                    label,
                    DocumentDimensionDefinition::PointDistance {
                        first: first_point,
                        second: second_point,
                        target,
                    },
                    DocumentDimensionMode::Driving,
                )
                .expect("length");
        }
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("locked elbow");
        (
            RetainedEditorCoordinator::new(session).expect("coordinator"),
            elbow,
            [first, second],
        )
    }

    fn assert_operation_within_limits(report: &OperationReport) {
        macro_rules! within {
            ($field:ident) => {
                assert!(
                    report.consumed.$field <= report.configured.$field,
                    "{} consumed {} above configured {}",
                    stringify!($field),
                    report.consumed.$field,
                    report.configured.$field
                );
            };
        }
        within!(document_validation_items);
        within!(document_dependency_items);
        within!(document_lowering_items);
        within!(nonlinear_iterations);
        within!(rejected_trials);
        within!(component_linearizations);
        within!(dense_kernel_rows);
        within!(dense_kernel_columns);
        within!(dense_kernel_work_units);
        within!(factorizations);
        within!(rank_kernels);
        within!(diagnostic_candidates);
        within!(diagnostic_trials);
        within!(profile_candidate_pairs);
        within!(profile_subdivisions);
        within!(profile_roots);
        within!(profile_fragments);
        within!(profile_integrations);
        within!(profile_containment_tests);
        within!(profile_faces);
        within!(measurement_integrations);
        within!(measurement_derivative_evaluations);
    }

    fn assert_branch_direction_eq(actual: [f64; 2], expected: [f64; 2], context: &str) {
        assert_eq!(
            actual.map(f64::to_bits),
            expected.map(f64::to_bits),
            "{context}: explicit branch direction changed; expected={expected:?}, actual={actual:?}"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end branch transaction regression keeps proposal, stopped acceptance, cancel, replacement, and successful publication evidence together"
    )]
    fn public_locked_four_bar_branch_is_bounded_exact_and_atomic() {
        let fixture =
            alpha_scenario(AlphaScenarioKind::BranchFourBar, 1.0).expect("four-bar fixture");
        let AlphaScenarioIds::BranchFourBar(ids) = &fixture.ids else {
            panic!("four-bar persistent roles");
        };
        let output_joint = ids.joints[1];
        let session = RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .expect("accepted public four-bar");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let base_design = coordinator.session().design_identity();
        let base_accepted = coordinator
            .session()
            .accepted_state()
            .expect("base accepted")
            .identity();
        let base_design_json = coordinator.session().export_design_json().unwrap();
        let base_accepted_json = coordinator.session().export_accepted_json().unwrap();

        let result = coordinator.propose_alternate_branch(output_joint);
        assert_eq!(result.status, AlternateBranchSearchStatus::Proposed);
        assert!(result.evidence.attempted_seeds <= ALTERNATE_BRANCH_MAX_SEEDS);
        assert!(result.evidence.independently_valid_candidates > 0);
        assert_eq!(result.evidence.representable_modes, 1);
        assert_eq!(result.evidence.operation.stopping_reason, None);
        assert_operation_within_limits(&result.evidence.operation);
        let proposal = result.proposal.expect("four-bar proposal");
        assert_eq!(proposal.design, base_design);
        assert_eq!(proposal.accepted, base_accepted);
        assert_eq!(proposal.point, output_joint);
        assert_eq!(proposal.branches.len(), 2);
        assert_eq!(coordinator.session().design_identity(), base_design);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("authoritative accepted")
                .identity(),
            base_accepted
        );
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            base_design_json
        );
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            base_accepted_json
        );
        let ghost_accepted_json = coordinator
            .alternate_branch_preview_session()
            .expect("four-bar ghost")
            .export_accepted_json()
            .expect("ghost accepted JSON");
        let before_stopped_accept = (
            coordinator.session().last_attempt().identity(),
            coordinator.history_len(),
            coordinator.history_cursor(),
            coordinator.transcript().len(),
            coordinator
                .alternate_branch_proposal()
                .expect("retained proposal")
                .clone(),
        );
        let mut exhausted_accept =
            OperationControl::new(CancellationToken::default(), alternate_branch_limits());
        exhausted_accept.limits.document_validation_items = 0;
        assert!(matches!(
            coordinator
                .accept_alternate_branch_with_control(proposal.proposal_id, exhausted_accept),
            Err(CoordinatorError::OperationStopped {
                context,
                report,
            }) if context == "alternate-branch acceptance"
                && matches!(
                    report.stopping_reason,
                    Some(OperationStopReason::WorkExhausted {
                        counter: OperationWorkCounter::DocumentValidationItems,
                        ..
                    })
                )
        ));
        assert_eq!(coordinator.session().design_identity(), base_design);
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            before_stopped_accept.0
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("authoritative accepted")
                .identity(),
            base_accepted
        );
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            base_design_json
        );
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            base_accepted_json
        );
        assert_eq!(coordinator.history_len(), before_stopped_accept.1);
        assert_eq!(coordinator.history_cursor(), before_stopped_accept.2);
        assert_eq!(coordinator.transcript().len(), before_stopped_accept.3);
        assert_eq!(
            coordinator
                .alternate_branch_proposal()
                .expect("proposal retained after stop"),
            &before_stopped_accept.4
        );
        assert_eq!(
            coordinator
                .alternate_branch_preview_session()
                .expect("ghost retained after stop")
                .export_accepted_json()
                .expect("retained ghost JSON"),
            ghost_accepted_json
        );

        coordinator.cancel_alternate_branch();
        assert!(coordinator.alternate_branch_proposal().is_none());
        assert!(coordinator.alternate_branch_preview_session().is_none());
        assert_eq!(coordinator.session().design_identity(), base_design);
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            base_design_json
        );

        let proposal = coordinator
            .propose_alternate_branch(output_joint)
            .proposal
            .expect("replacement four-bar proposal");
        let replacement_ghost_json = coordinator
            .alternate_branch_preview_session()
            .expect("replacement four-bar ghost")
            .export_accepted_json()
            .expect("replacement ghost JSON");
        assert_eq!(replacement_ghost_json, ghost_accepted_json);
        let accepted = coordinator
            .accept_alternate_branch(proposal.proposal_id)
            .expect("accept four-bar branch");
        assert!(accepted.published_accepted.is_some());
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            replacement_ghost_json
        );
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted alternate four-bar");
        assert!(
            accepted
                .solve_result()
                .acceptance_hard_residual_max
                .is_some_and(|residual| residual <= 1.0e-9)
        );
        assert!(
            accepted
                .document()
                .point(output_joint)
                .is_some_and(|point| point.position.iter().all(|value| value.is_finite()))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn alternate_branch_is_bounded_ghosted_exact_stamped_and_atomic() {
        fn accepted_with_authoritative_line_branches(
            coordinator: &RetainedEditorCoordinator,
            curves: [CurveId; 2],
        ) -> String {
            let mut accepted = coordinator
                .session()
                .accepted_state()
                .expect("accepted branch state")
                .document()
                .clone();
            for curve in curves {
                let span = CurveSpan::line(curve);
                let direction = coordinator
                    .session()
                    .design_document()
                    .curve_branch_direction(span)
                    .expect("authoritative line branch");
                accepted
                    .set_curve_branch(span, direction)
                    .expect("normalize non-enforced accepted branch seed");
            }
            accepted.to_canonical_json().expect("accepted state JSON")
        }

        let (mut coordinator, elbow, [first, second]) = alternate_branch_fixture();
        let replay_session = coordinator.session().clone();
        let base_identity = coordinator.session().design_identity();
        let base_accepted = coordinator.session().accepted_state().unwrap().identity();
        let base_json = coordinator.session().export_design_json().unwrap();
        let base_accepted_json = coordinator.session().export_accepted_json().unwrap();

        let result = coordinator.propose_alternate_branch(elbow);
        assert_eq!(result.status, AlternateBranchSearchStatus::Proposed);
        assert_eq!(result.evidence.maximum_seeds, ALTERNATE_BRANCH_MAX_SEEDS);
        assert!(result.evidence.attempted_seeds <= ALTERNATE_BRANCH_MAX_SEEDS);
        assert!(result.evidence.independently_valid_candidates > 0);
        assert_eq!(result.evidence.operation.stopping_reason, None);
        assert_operation_within_limits(&result.evidence.operation);
        let proposal = result.proposal.expect("proposal");
        assert_eq!(proposal.evidence, result.evidence);
        assert_eq!(proposal.design, base_identity);
        assert_eq!(proposal.accepted, base_accepted);
        assert!(proposal.position[1] < 0.0, "{proposal:#?}");
        assert_eq!(
            proposal.branches.len(),
            2,
            "proposal must carry every incident line branch"
        );
        assert!(result.evidence.representable_modes > 0);
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            base_json,
            "ghost search must not mutate authoritative design"
        );
        assert!(coordinator.alternate_branch_preview_session().is_some());
        let mut exact_proposal_design = coordinator.session().design_document().clone();
        exact_proposal_design
            .set_point_position(proposal.point, proposal.position)
            .expect("proposal point");
        for branch in &proposal.branches {
            exact_proposal_design
                .set_curve_branch(branch.curve, branch.direction)
                .expect("proposal branch");
        }
        assert_eq!(
            coordinator
                .alternate_branch_preview_session()
                .expect("canonical ghost")
                .design_document(),
            &exact_proposal_design,
            "stored ghost design must be exactly base + accepted point + complete branches"
        );

        coordinator.cancel_alternate_branch();
        assert!(coordinator.alternate_branch_proposal().is_none());
        assert!(coordinator.alternate_branch_preview_session().is_none());
        assert_eq!(coordinator.session().design_identity(), base_identity);

        let proposal = coordinator
            .propose_alternate_branch(elbow)
            .proposal
            .expect("replacement proposal");
        let ghost_accepted_json = coordinator
            .alternate_branch_preview_session()
            .expect("replacement ghost")
            .export_accepted_json()
            .expect("ghost accepted JSON");
        let accepted = coordinator
            .accept_alternate_branch(proposal.proposal_id)
            .expect("accept branch");
        assert!(accepted.published_accepted.is_some());
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            ghost_accepted_json,
            "accepted branch geometry must equal the inspected ghost"
        );
        let crossed = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .document()
            .point(elbow)
            .unwrap()
            .position;
        assert!(crossed[1] < 0.0);
        for branch in &proposal.branches {
            let old = if branch.curve == CurveSpan::line(first) {
                [0.8, 0.6]
            } else {
                assert_eq!(branch.curve, CurveSpan::line(second));
                [0.8, -0.6]
            };
            let direction = coordinator
                .session()
                .design_document()
                .curve_branch_direction(branch.curve)
                .unwrap();
            assert!(old[0] * direction[0] + old[1] * direction[1] < 0.0);
        }
        let crossed_design_json = coordinator.session().export_design_json().unwrap();
        // Restore independently re-solves accepted snapshots. Non-enforced line branch
        // references are numerical seeds rather than accepted geometry, so compare the
        // complete state after normalizing those references to authoritative design intent.
        let crossed_accepted_state_json =
            accepted_with_authoritative_line_branches(&coordinator, [first, second]);
        coordinator.undo().expect("undo branch");
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            base_json
        );
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            base_accepted_json
        );
        assert!(
            coordinator
                .session()
                .accepted_state()
                .unwrap()
                .document()
                .point(elbow)
                .unwrap()
                .position[1]
                > 0.0
        );
        coordinator.redo().expect("redo branch");
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            crossed_design_json
        );
        assert_eq!(
            accepted_with_authoritative_line_branches(&coordinator, [first, second]),
            crossed_accepted_state_json
        );
        assert!(
            coordinator
                .session()
                .accepted_state()
                .unwrap()
                .document()
                .point(elbow)
                .unwrap()
                .position[1]
                < 0.0
        );
        let transcript = coordinator.transcript().to_vec();
        let mut replay =
            RetainedEditorCoordinator::new(replay_session).expect("replay coordinator");
        for action in &transcript {
            replay.replay(action).expect("replay branch lifecycle");
        }
        assert_eq!(replay.transcript(), transcript);
        assert_eq!(
            replay.session().export_design_json().unwrap(),
            crossed_design_json
        );
        assert_eq!(
            accepted_with_authoritative_line_branches(&replay, [first, second]),
            crossed_accepted_state_json
        );

        let stale = coordinator
            .propose_alternate_branch(elbow)
            .proposal
            .expect("stale proposal");
        let replacement = coordinator
            .propose_alternate_branch(elbow)
            .proposal
            .expect("newer proposal");
        assert_ne!(stale.proposal_id, replacement.proposal_id);
        assert!(matches!(
            coordinator.accept_alternate_branch(stale.proposal_id),
            Err(CoordinatorError::StaleAlternateBranchProposal)
        ));
        coordinator
            .reattempt(coordinator.session().design_identity())
            .expect("advance accepted stamp");
        assert!(coordinator.alternate_branch_proposal().is_none());
        assert!(coordinator.alternate_branch_preview_session().is_none());
        assert!(matches!(
            coordinator.accept_alternate_branch(replacement.proposal_id),
            Err(CoordinatorError::MissingAlternateBranchProposal)
        ));

        let _ = coordinator.resolve_projected_point_move(301, 1, elbow, crossed);
        assert!(
            coordinator
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted)
        );
        assert!(coordinator.solved_preview_session().is_some());
        let ordinary_stale = coordinator
            .propose_alternate_branch(elbow)
            .proposal
            .expect("proposal beside an ordinary preview");
        assert!(coordinator.alternate_branch_preview_session().is_some());

        let _ = coordinator.resolve_projected_point_move(302, 1, elbow, [f64::NAN, crossed[1]]);
        assert!(coordinator.solved_preview_session().is_none());
        assert!(coordinator.alternate_branch_preview_session().is_none());
        assert!(coordinator.drag_continuation.is_none());
        assert!(matches!(
            coordinator.accept_alternate_branch(ordinary_stale.proposal_id),
            Err(CoordinatorError::MissingAlternateBranchProposal)
        ));
    }

    #[test]
    fn alternate_branch_rejects_tampered_missing_and_extra_proposal_edits_atomically() {
        for corruption in 0..3 {
            let (mut coordinator, elbow, _) = alternate_branch_fixture();
            let proposal = coordinator
                .propose_alternate_branch(elbow)
                .proposal
                .expect("proposal");
            let candidate = coordinator
                .alternate_branch
                .as_mut()
                .expect("retained candidate");
            match corruption {
                0 => {
                    candidate.proposal.branches.pop();
                }
                1 => {
                    candidate.proposal.branches[0].direction = [1.0, 0.0];
                }
                2 => {
                    let extra = candidate.proposal.branches[0];
                    candidate.proposal.branches.push(extra);
                }
                _ => unreachable!(),
            }
            let before = (
                coordinator.session().design_identity(),
                coordinator.session().last_attempt().identity(),
                coordinator.session().accepted_state().unwrap().identity(),
                coordinator.session().export_design_json().unwrap(),
                coordinator.session().export_accepted_json().unwrap(),
                coordinator.history_len(),
                coordinator.transcript().len(),
            );
            assert!(matches!(
                coordinator.accept_alternate_branch(proposal.proposal_id),
                Err(CoordinatorError::Session(
                    DocumentSessionError::PreviewBranchMismatch
                ))
            ));
            assert_eq!(coordinator.session().design_identity(), before.0);
            assert_eq!(coordinator.session().last_attempt().identity(), before.1);
            assert_eq!(
                coordinator.session().accepted_state().unwrap().identity(),
                before.2
            );
            assert_eq!(
                coordinator.session().export_design_json().unwrap(),
                before.3
            );
            assert_eq!(
                coordinator.session().export_accepted_json().unwrap(),
                before.4
            );
            assert_eq!(coordinator.history_len(), before.5);
            assert_eq!(coordinator.transcript().len(), before.6);
        }
    }

    #[test]
    fn alternate_branch_aggregate_budget_exhaustion_is_typed_and_non_mutating() {
        let (mut coordinator, elbow, _) = alternate_branch_fixture();
        let base_design = coordinator.session().design_identity();
        let base_accepted = coordinator.session().accepted_state().unwrap().identity();
        let base_json = coordinator.session().export_design_json().unwrap();
        let mut limits = alternate_branch_limits();
        limits.document_validation_items = 0;

        let result = coordinator.propose_alternate_branch_with_limits(elbow, limits);

        assert_eq!(result.status, AlternateBranchSearchStatus::Exhausted);
        assert!(result.proposal.is_none());
        assert_eq!(result.evidence.maximum_seeds, ALTERNATE_BRANCH_MAX_SEEDS);
        assert!(
            result.evidence.attempted_seeds > 0
                && result.evidence.attempted_seeds <= ALTERNATE_BRANCH_MAX_SEEDS
        );
        assert_eq!(result.evidence.independently_valid_candidates, 0);
        assert_eq!(result.evidence.representable_modes, 0);
        assert_eq!(result.evidence.operation.configured, limits);
        assert!(matches!(
            result.evidence.operation.stopping_reason,
            Some(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DocumentValidationItems,
                ..
            })
        ));
        assert_operation_within_limits(&result.evidence.operation);
        assert!(coordinator.alternate_branch_proposal().is_none());
        assert!(coordinator.alternate_branch_preview_session().is_none());
        assert_eq!(coordinator.session().design_identity(), base_design);
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            base_accepted
        );
        assert_eq!(
            coordinator.session().export_design_json().unwrap(),
            base_json
        );
    }

    #[test]
    fn alternate_branch_requires_known_equal_base_and_candidate_dof() {
        assert!(same_known_equality_degrees_of_freedom(Some(2), Some(2)));
        assert!(!same_known_equality_degrees_of_freedom(None, Some(2)));
        assert!(!same_known_equality_degrees_of_freedom(Some(2), None));
        assert!(!same_known_equality_degrees_of_freedom(None, None));
        assert!(!same_known_equality_degrees_of_freedom(Some(2), Some(1)));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the deterministic work gate keeps the four comparable mechanism paths and frozen ratios in one corpus"
    )]
    fn deterministic_mechanism_drag_corpus_has_one_attempt_per_sample() {
        fn run_path(
            mut coordinator: RetainedEditorCoordinator,
            point: DesignPointId,
            targets: &[[f64; 2]],
        ) -> (usize, usize) {
            let mut factorizations = 0usize;
            let mut nonlinear_iterations = 0usize;
            for (index, target) in targets.iter().copied().enumerate() {
                let _ = coordinator.resolve_projected_point_move(
                    91,
                    u64::try_from(index + 1).unwrap(),
                    point,
                    target,
                );
                let work = coordinator
                    .projected_drag_work_evidence()
                    .expect("drag work");
                assert_eq!(work.attempts, 1);
                assert_eq!(work.continued, index > 0);
                assert!(work.accepted, "{work:#?}");
                assert_eq!(work.rejection_stage, None, "{work:#?}");
                assert!(work.operation.stopping_reason.is_none(), "{work:#?}");
                assert_operation_within_limits(&work.operation);
                assert!(work.solve.is_some(), "{work:#?}");
                factorizations =
                    factorizations.saturating_add(work.operation.consumed.factorizations);
                nonlinear_iterations = nonlinear_iterations
                    .saturating_add(work.operation.consumed.nonlinear_iterations);
            }
            (factorizations, nonlinear_iterations)
        }

        let mut scotch = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                alpha_scenario(AlphaScenarioKind::MotionScotchYoke, 1.0)
                    .unwrap()
                    .document,
                DocumentSolveRequest::default().without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let AlphaScenarioIds::MotionScotchYoke(scotch_ids) =
            alpha_scenario(AlphaScenarioKind::MotionScotchYoke, 1.0)
                .unwrap()
                .ids
        else {
            unreachable!()
        };
        let guide = scotch
            .session()
            .design_document()
            .constraints()
            .iter()
            .find(|constraint| constraint.label == "Yoke slider on horizontal guide")
            .unwrap()
            .id;
        scotch
            .editor_mut()
            .set_selection([SelectionItem::Constraint(guide)]);
        scotch
            .delete_selected(scotch.session().design_identity())
            .expect("delete yoke guide");
        let scotch_work = run_path(
            scotch,
            scotch_ids.slider,
            &[[3.1, -5.9], [3.2, -5.8], [3.3, -5.7]],
        );

        let scissor_fixture = alpha_scenario(AlphaScenarioKind::MotionScissor, 1.0).unwrap();
        let AlphaScenarioIds::MotionScissor(scissor_ids) = scissor_fixture.ids else {
            unreachable!()
        };
        let scissor_work = run_path(
            RetainedEditorCoordinator::new(
                RetainedSketchDocumentSession::new(
                    scissor_fixture.document,
                    scissor_fixture.request,
                    SolverConfig::default(),
                )
                .unwrap(),
            )
            .unwrap(),
            scissor_ids.slider,
            &[[3.9, 0.0], [3.7, 0.0], [3.5, 0.0]],
        );

        let tower_fixture = alpha_scenario(AlphaScenarioKind::MotionScissorTower, 1.0).unwrap();
        let AlphaScenarioIds::MotionScissorTower(tower_ids) = tower_fixture.ids else {
            unreachable!()
        };
        let tower_work = run_path(
            RetainedEditorCoordinator::new(
                RetainedSketchDocumentSession::new(
                    tower_fixture.document,
                    tower_fixture.request,
                    SolverConfig::default(),
                )
                .unwrap(),
            )
            .unwrap(),
            tower_ids.right_levels[0],
            &[[3.9, 0.0], [3.7, 0.0], [3.5, 0.0]],
        );

        let pantograph_fixture = alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).unwrap();
        let AlphaScenarioIds::MotionPantograph(pantograph_ids) = pantograph_fixture.ids else {
            unreachable!()
        };
        let radius = 17.0_f64.sqrt();
        let pantograph_targets =
            [0.27_f64, 0.30, 0.33].map(|angle| [radius * angle.cos(), radius * angle.sin()]);
        let pantograph_work = run_path(
            RetainedEditorCoordinator::new(
                RetainedSketchDocumentSession::new(
                    pantograph_fixture.document,
                    pantograph_fixture.request,
                    SolverConfig::default(),
                )
                .unwrap(),
            )
            .unwrap(),
            pantograph_ids.input,
            &pantograph_targets,
        );
        for work in [scotch_work, scissor_work, tower_work, pantograph_work] {
            assert!(work.0 > 0);
            assert!(work.1 > 0);
        }
        assert!(
            pantograph_work.0 <= 2 * tower_work.0,
            "pantograph factorizations {pantograph_work:?} exceeded twice tower {tower_work:?}"
        );
        assert!(
            pantograph_work.1 <= 2 * tower_work.1,
            "pantograph iterations {pantograph_work:?} exceeded twice tower {tower_work:?}"
        );
        assert!(
            pantograph_work.0 <= 24_482 && pantograph_work.1 <= 24_095,
            "pantograph work {pantograph_work:?} did not improve by at least 90% from the \
             starting-commit baseline (244824 factorizations, 240953 iterations)"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the reversible two-DOF gesture and its branch, locality, and continuation assertions are one end-to-end regression"
    )]
    fn scotch_yoke_two_dof_drag_tracks_targets_and_reverses_without_a_branch_jump() {
        let fixture = alpha_scenario(AlphaScenarioKind::MotionScotchYoke, 1.0).unwrap();
        let AlphaScenarioIds::MotionScotchYoke(ids) = fixture.ids else {
            unreachable!()
        };
        let mut coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                fixture.document,
                fixture.request.without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let guide = coordinator
            .session()
            .design_document()
            .constraints()
            .iter()
            .find(|constraint| constraint.label == "Yoke slider on horizontal guide")
            .unwrap()
            .id;
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Constraint(guide)]);
        coordinator
            .delete_selected(coordinator.session().design_identity())
            .expect("delete yoke guide");
        assert_eq!(
            coordinator
                .session()
                .accepted_diagnostics()
                .and_then(|diagnostics| diagnostics.mobility)
                .and_then(|mobility| mobility.equality_degrees_of_freedom),
            Some(2)
        );
        let branch_directions = [ids.crank, ids.slot].map(|curve| {
            let CurveDefinition::Line {
                branch_direction, ..
            } = coordinator
                .session()
                .design_document()
                .curve(curve)
                .expect("yoke line")
                .definition
            else {
                panic!("yoke curves remain lines")
            };
            branch_direction
        });
        let mut first_positions = None;

        for (index, target) in [[3.1_f64, -5.9], [3.3, -5.7], [3.1, -5.9], [2.9, -6.1]]
            .into_iter()
            .enumerate()
        {
            let _ = coordinator.resolve_projected_point_move(
                98,
                u64::try_from(index + 1).unwrap(),
                ids.slider,
                target,
            );
            let work = coordinator
                .projected_drag_work_evidence()
                .expect("yoke drag work");
            assert_eq!(work.attempts, 1, "{work:#?}");
            assert_eq!(work.continued, index > 0, "{work:#?}");
            assert!(work.accepted, "{work:#?}");
            let locality = work.locality_plan().expect("yoke locality");
            assert_eq!(locality.hard_degrees_of_freedom(), 2);
            assert_eq!(locality.active_rank(), 2);
            assert_eq!(locality.passive_degrees_of_freedom(), 0);
            assert!(locality.anchors().is_empty());
            let preview = coordinator
                .solved_preview_session()
                .expect("accepted yoke preview")
                .accepted_state()
                .expect("accepted yoke state")
                .document();
            let slider = preview.point(ids.slider).unwrap().position;
            let pin = preview.point(ids.crank_pin).unwrap().position;
            let center = preview.point(ids.crank_center).unwrap().position;
            assert!(
                (slider[0] - target[0]).hypot(slider[1] - target[1]) <= 1.0e-8,
                "two-DOF yoke slider did not follow its attainable target: \
                 target={target:?}, accepted={slider:?}"
            );
            assert!((pin[0] - slider[0]).abs() <= 1.0e-8);
            assert!(((pin[0] - center[0]).hypot(pin[1] - center[1]) - 5.0).abs() <= 1.0e-8);
            assert!(pin[1] > 0.0, "crank silently crossed branch: {pin:?}");
            for (curve, expected) in [ids.crank, ids.slot].into_iter().zip(branch_directions) {
                let CurveDefinition::Line {
                    branch_direction, ..
                } = preview.curve(curve).expect("yoke preview line").definition
                else {
                    panic!("yoke curves remain lines")
                };
                assert_branch_direction_eq(
                    branch_direction,
                    expected,
                    "yoke drag changed explicit line branch state",
                );
            }
            let positions = [slider, pin];
            if index == 0 {
                first_positions = Some(positions);
            } else if index == 2 {
                for (returned, first) in positions.into_iter().zip(first_positions.unwrap()) {
                    assert!(
                        (returned[0] - first[0]).hypot(returned[1] - first[1]) <= 1.0e-8,
                        "reversing to the same yoke target did not return to the same local \
                         continuation: first={first:?}, returned={returned:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn scissor_jack_drag_follows_the_nearest_open_branch_through_reversal() {
        let fixture = alpha_scenario(AlphaScenarioKind::MotionScissor, 1.0).unwrap();
        let AlphaScenarioIds::MotionScissor(ids) = fixture.ids else {
            unreachable!()
        };
        let mut coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                fixture.document,
                fixture.request,
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut first_positions = None;

        for (index, target_x) in [3.9_f64, 3.6, 3.9, 4.1].into_iter().enumerate() {
            let target = [target_x, 0.0];
            let _ = coordinator.resolve_projected_point_move(
                99,
                u64::try_from(index + 1).unwrap(),
                ids.slider,
                target,
            );
            let work = coordinator
                .projected_drag_work_evidence()
                .expect("scissor drag work");
            assert_eq!(work.attempts, 1, "{work:#?}");
            assert_eq!(work.continued, index > 0, "{work:#?}");
            assert!(work.accepted, "{work:#?}");
            let preview = coordinator
                .solved_preview_session()
                .expect("accepted scissor preview")
                .accepted_state()
                .expect("accepted scissor state")
                .document();
            let anchor = preview.point(ids.anchor).unwrap().position;
            let slider = preview.point(ids.slider).unwrap().position;
            let upper = preview.point(ids.upper_joint).unwrap().position;
            let lower = preview.point(ids.lower_joint).unwrap().position;
            let half_span = 0.5 * (target_x - anchor[0]);
            let expected_height = (25.0 - half_span * half_span).sqrt();
            let expected_x = 0.5 * (anchor[0] + target_x);
            let expected_upper = [expected_x, expected_height];
            let expected_lower = [expected_x, -expected_height];
            assert!(
                (slider[0] - target[0]).hypot(slider[1] - target[1]) <= 1.0e-8,
                "scissor slider did not follow its attainable target"
            );
            assert!(
                (upper[0] - expected_upper[0]).hypot(upper[1] - expected_upper[1]) <= 1.0e-8,
                "scissor upper joint left the nearest open branch: \
                 expected={expected_upper:?}, accepted={upper:?}"
            );
            assert!(
                (lower[0] - expected_lower[0]).hypot(lower[1] - expected_lower[1]) <= 1.0e-8,
                "scissor lower joint left the mirrored branch: \
                 expected={expected_lower:?}, accepted={lower:?}"
            );
            let positions = [slider, upper, lower];
            if index == 0 {
                first_positions = Some(positions);
            } else if index == 2 {
                for (returned, first) in positions.into_iter().zip(first_positions.unwrap()) {
                    assert!(
                        (returned[0] - first[0]).hypot(returned[1] - first[1]) <= 1.0e-8,
                        "reversing to the same scissor target changed the local assembly: \
                         first={first:?}, returned={returned:?}"
                    );
                }
            }
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "all tower levels must be checked together across one reversible retained gesture"
    )]
    fn scissor_tower_drag_reverses_on_the_same_upright_assembly() {
        let fixture = alpha_scenario(AlphaScenarioKind::MotionScissorTower, 1.0).unwrap();
        let AlphaScenarioIds::MotionScissorTower(ids) = fixture.ids else {
            unreachable!()
        };
        let mut coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                fixture.document,
                fixture.request,
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let line_ids = ids
            .platforms
            .into_iter()
            .chain(ids.diagonal_bars)
            .collect::<Vec<_>>();
        let branch_directions = line_ids
            .iter()
            .map(|curve| {
                let CurveDefinition::Line {
                    branch_direction, ..
                } = coordinator
                    .session()
                    .design_document()
                    .curve(*curve)
                    .expect("tower line")
                    .definition
                else {
                    panic!("tower curves remain lines")
                };
                branch_direction
            })
            .collect::<Vec<_>>();
        let mut first_positions = None;

        for (index, target_x) in [3.9_f64, 3.6, 3.9].into_iter().enumerate() {
            let target = [target_x, 0.0];
            let _ = coordinator.resolve_projected_point_move(
                100,
                u64::try_from(index + 1).unwrap(),
                ids.right_levels[0],
                target,
            );
            let work = coordinator
                .projected_drag_work_evidence()
                .expect("tower drag work");
            assert_eq!(work.attempts, 1, "{work:#?}");
            assert_eq!(work.continued, index > 0, "{work:#?}");
            assert!(work.accepted, "{work:#?}");
            let preview = coordinator
                .solved_preview_session()
                .expect("accepted tower preview")
                .accepted_state()
                .expect("accepted tower state")
                .document();
            let base_left = preview.point(ids.left_levels[0]).unwrap().position;
            let base_right = preview.point(ids.right_levels[0]).unwrap().position;
            assert!(
                (base_right[0] - target[0]).hypot(base_right[1] - target[1]) <= 1.0e-8,
                "tower base slider did not follow its attainable target"
            );
            let width = target_x - base_left[0];
            let stage_height = (100.0 - width * width).sqrt();
            let mut positions = Vec::with_capacity(12);
            for level in 0..=5 {
                let left = preview.point(ids.left_levels[level]).unwrap().position;
                let right = preview.point(ids.right_levels[level]).unwrap().position;
                let expected_left = [
                    base_left[0],
                    base_left[1] + f64::from(u32::try_from(level).unwrap()) * stage_height,
                ];
                let expected_right = [target_x, expected_left[1]];
                assert!(
                    (left[0] - expected_left[0]).hypot(left[1] - expected_left[1]) <= 2.0e-7,
                    "tower level {level} left point left the upright branch: \
                     expected={expected_left:?}, accepted={left:?}"
                );
                assert!(
                    (right[0] - expected_right[0]).hypot(right[1] - expected_right[1]) <= 2.0e-7,
                    "tower level {level} right point left the upright branch: \
                     expected={expected_right:?}, accepted={right:?}"
                );
                positions.extend([left, right]);
            }
            for (curve, expected) in line_ids.iter().zip(&branch_directions) {
                let CurveDefinition::Line {
                    branch_direction, ..
                } = preview
                    .curve(*curve)
                    .expect("tower preview line")
                    .definition
                else {
                    panic!("tower curves remain lines")
                };
                assert_branch_direction_eq(
                    branch_direction,
                    *expected,
                    "tower drag changed explicit line branch state",
                );
            }
            if index == 0 {
                first_positions = Some(positions);
            } else if index == 2 {
                for (returned, first) in positions
                    .into_iter()
                    .zip(first_positions.take().expect("first tower state"))
                {
                    assert!(
                        (returned[0] - first[0]).hypot(returned[1] - first[1]) <= 2.0e-7,
                        "reversing to the same tower target changed the local assembly: \
                         first={first:?}, returned={returned:?}"
                    );
                }
            }
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the off-manifold pointer path keeps work bounds, locality, geometry, and branch continuity in one audited gesture"
    )]
    fn off_manifold_pantograph_cursor_path_is_accepted_with_bounded_work() {
        let fixture = alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).unwrap();
        let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
            unreachable!()
        };
        let mut coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                fixture.document,
                fixture.request,
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let baseline_design = coordinator.session().design_identity();
        let baseline_accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted pantograph")
            .identity();
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted pantograph")
            .document();
        let guide_baseline = accepted.point(ids.guide).expect("guide").position;
        let branch_directions = ids.bars.map(|bar| {
            let CurveDefinition::Line {
                branch_direction, ..
            } = accepted.curve(bar).expect("pantograph bar").definition
            else {
                panic!("pantograph bars are lines")
            };
            branch_direction
        });
        let mut total_factorizations = 0;
        let mut total_iterations = 0;
        let mut maximum_factorizations = 0;
        let mut maximum_iterations = 0;
        let input_radius = 17.0_f64.sqrt();
        for (index, target) in [[4.0_f64, 2.0], [3.8, 2.2], [3.6, 2.4]]
            .into_iter()
            .enumerate()
        {
            let target_norm = target[0].hypot(target[1]);
            let expected_input = [
                target[0] * input_radius / target_norm,
                target[1] * input_radius / target_norm,
            ];
            let _ = coordinator.resolve_projected_point_move(
                92,
                u64::try_from(index + 1).unwrap(),
                ids.input,
                target,
            );
            let work = coordinator.projected_drag_work_evidence().unwrap();
            assert_eq!(work.attempts, 1);
            assert_eq!(work.continued, index > 0);
            assert!(work.accepted, "{work:#?}");
            assert_eq!(work.rejection_stage, None, "{work:#?}");
            assert!(work.operation.stopping_reason.is_none(), "{work:#?}");
            assert_operation_within_limits(&work.operation);
            assert!(work.solve.is_some(), "{work:#?}");
            let locality = work.locality_plan().expect("pantograph locality plan");
            assert_eq!(locality.design_identity(), baseline_design);
            assert_eq!(locality.accepted_state_identity(), baseline_accepted);
            assert_eq!(locality.point(), ids.input);
            assert_eq!(locality.anchors().len(), 1);
            assert_eq!(locality.anchors()[0].point(), ids.guide);
            assert_eq!(
                locality.anchors()[0].target().map(f64::to_bits),
                guide_baseline.map(f64::to_bits)
            );
            assert!(
                work.operation.consumed.factorizations <= PANTOGRAPH_POINTER_MAX_FACTORIZATIONS
                    && work.operation.consumed.nonlinear_iterations
                        <= PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
                "{work:#?}"
            );
            total_factorizations += work.operation.consumed.factorizations;
            total_iterations += work.operation.consumed.nonlinear_iterations;
            maximum_factorizations =
                maximum_factorizations.max(work.operation.consumed.factorizations);
            maximum_iterations =
                maximum_iterations.max(work.operation.consumed.nonlinear_iterations);
            let preview = coordinator
                .solved_preview_session()
                .expect("accepted preview")
                .accepted_state()
                .expect("accepted preview state")
                .document();
            let input = preview.point(ids.input).expect("input").position;
            let guide = preview.point(ids.guide).expect("guide").position;
            let output = preview.point(ids.output).expect("output").position;
            let center = preview.point(ids.center).expect("center").position;
            assert!(
                (input[0] - expected_input[0]).hypot(input[1] - expected_input[1]) <= 1.0e-8,
                "input arm did not take the nearest fixed-radius projection: \
                 target={target:?}, expected={expected_input:?}, accepted={input:?}"
            );
            assert!(
                (guide[0] - guide_baseline[0]).hypot(guide[1] - guide_baseline[1]) <= 1.0e-8,
                "dragging the independent input arm moved the guide from {guide_baseline:?} \
                 to {guide:?}"
            );
            assert!(
                (output[0] - input[0] - guide[0]).hypot(output[1] - input[1] - guide[1]) <= 1.0e-8,
                "pantograph parallelogram lost C = A + B: A={input:?} B={guide:?} C={output:?}"
            );
            assert!(
                (2.0 * center[0] - output[0]).hypot(2.0 * center[1] - output[1]) <= 1.0e-8,
                "pantograph midpoint lost M = C / 2: M={center:?} C={output:?}"
            );
            for (bar, expected) in ids.bars.into_iter().zip(branch_directions) {
                let CurveDefinition::Line {
                    branch_direction, ..
                } = preview.curve(bar).expect("pantograph bar").definition
                else {
                    panic!("pantograph bars remain lines")
                };
                assert_branch_direction_eq(
                    branch_direction,
                    expected,
                    "ordinary drag changed explicit pantograph branch state",
                );
            }
        }
        assert!(
            maximum_factorizations <= PANTOGRAPH_POINTER_MAX_FACTORIZATIONS,
            "{maximum_factorizations}"
        );
        assert!(
            maximum_iterations <= PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
            "{maximum_iterations}"
        );
        assert!(
            total_factorizations <= 3 * PANTOGRAPH_POINTER_MAX_FACTORIZATIONS,
            "{total_factorizations}"
        );
        assert!(
            total_iterations <= 3 * PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
            "{total_iterations}"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the passive-arm stability path keeps bounded work, frozen locality, geometry, and branch continuity together"
    )]
    fn off_manifold_pantograph_guide_path_keeps_the_input_arm_fixed() {
        let fixture = alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).unwrap();
        let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
            unreachable!()
        };
        let mut coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                fixture.document,
                fixture.request,
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let baseline_design = coordinator.session().design_identity();
        let baseline_accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted pantograph")
            .identity();
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted pantograph")
            .document();
        let input_baseline = coordinator
            .session()
            .accepted_state()
            .expect("accepted pantograph")
            .document()
            .point(ids.input)
            .expect("input")
            .position;
        let branch_directions = ids.bars.map(|bar| {
            let CurveDefinition::Line {
                branch_direction, ..
            } = accepted.curve(bar).expect("pantograph bar").definition
            else {
                panic!("pantograph bars are lines")
            };
            branch_direction
        });
        let guide_radius = 10.0_f64.sqrt();
        let mut maximum_factorizations = 0;
        let mut maximum_iterations = 0;
        let mut total_factorizations = 0;
        let mut total_iterations = 0;

        for (index, target) in [[1.4_f64, 3.0], [1.5, 2.9], [1.6, 2.8]]
            .into_iter()
            .enumerate()
        {
            let target_norm = target[0].hypot(target[1]);
            let expected_guide = [
                target[0] * guide_radius / target_norm,
                target[1] * guide_radius / target_norm,
            ];
            let _ = coordinator.resolve_projected_point_move(
                94,
                u64::try_from(index + 1).unwrap(),
                ids.guide,
                target,
            );
            let work = coordinator.projected_drag_work_evidence().unwrap();
            assert_eq!(work.attempts, 1, "{work:#?}");
            assert_eq!(work.continued, index > 0, "{work:#?}");
            assert!(work.accepted, "{work:#?}");
            assert_eq!(work.rejection_stage, None, "{work:#?}");
            assert!(work.operation.stopping_reason.is_none(), "{work:#?}");
            assert_operation_within_limits(&work.operation);
            assert!(work.solve.is_some(), "{work:#?}");
            let locality = work.locality_plan().expect("pantograph locality plan");
            assert_eq!(locality.design_identity(), baseline_design);
            assert_eq!(locality.accepted_state_identity(), baseline_accepted);
            assert_eq!(locality.point(), ids.guide);
            assert_eq!(locality.anchors().len(), 1);
            assert_eq!(locality.anchors()[0].point(), ids.input);
            assert_eq!(
                locality.anchors()[0].target().map(f64::to_bits),
                input_baseline.map(f64::to_bits)
            );
            assert!(
                work.operation.consumed.factorizations <= PANTOGRAPH_POINTER_MAX_FACTORIZATIONS
                    && work.operation.consumed.nonlinear_iterations
                        <= PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
                "{work:#?}"
            );
            maximum_factorizations =
                maximum_factorizations.max(work.operation.consumed.factorizations);
            maximum_iterations =
                maximum_iterations.max(work.operation.consumed.nonlinear_iterations);
            total_factorizations += work.operation.consumed.factorizations;
            total_iterations += work.operation.consumed.nonlinear_iterations;
            let preview = coordinator
                .solved_preview_session()
                .expect("accepted preview")
                .accepted_state()
                .expect("accepted preview state")
                .document();
            let input = preview.point(ids.input).expect("input").position;
            let guide = preview.point(ids.guide).expect("guide").position;
            assert!(
                (input[0] - input_baseline[0]).hypot(input[1] - input_baseline[1]) <= 1.0e-8,
                "dragging the independent guide arm moved the input from {input_baseline:?} \
                 to {input:?}"
            );
            assert!(
                (guide[0] - expected_guide[0]).hypot(guide[1] - expected_guide[1]) <= 1.0e-8,
                "guide arm did not take the nearest fixed-radius projection: \
                 target={target:?}, expected={expected_guide:?}, accepted={guide:?}"
            );
            for (bar, expected) in ids.bars.into_iter().zip(branch_directions) {
                let CurveDefinition::Line {
                    branch_direction, ..
                } = preview.curve(bar).expect("pantograph bar").definition
                else {
                    panic!("pantograph bars remain lines")
                };
                assert_branch_direction_eq(
                    branch_direction,
                    expected,
                    "ordinary guide drag changed explicit pantograph branch state",
                );
            }
        }
        assert!(
            maximum_factorizations <= PANTOGRAPH_POINTER_MAX_FACTORIZATIONS,
            "{maximum_factorizations}"
        );
        assert!(
            maximum_iterations <= PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
            "{maximum_iterations}"
        );
        assert!(
            total_factorizations <= 3 * PANTOGRAPH_POINTER_MAX_FACTORIZATIONS,
            "{total_factorizations}"
        );
        assert!(
            total_iterations <= 3 * PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
            "{total_iterations}"
        );
    }

    // Each pantograph sample is governed by the production pointer ceiling;
    // aggregate path characterization remains a separate non-publication bound.
    const PANTOGRAPH_POINTER_MAX_FACTORIZATIONS: usize = PROJECTED_DRAG_MAX_FACTORIZATIONS;
    const PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS: usize =
        PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS;
    const PANTOGRAPH_WIDE_PATH_MAX_FACTORIZATIONS: usize = 768;
    const PANTOGRAPH_WIDE_PATH_MAX_NONLINEAR_ITERATIONS: usize = 640;

    fn pantograph_positions(
        document: &SketchDocument,
        ids: geosolve_sketch::MotionPantographIds,
    ) -> [[f64; 2]; 5] {
        [
            document.point(ids.anchor).expect("anchor").position,
            document.point(ids.input).expect("input").position,
            document.point(ids.guide).expect("guide").position,
            document.point(ids.output).expect("output").position,
            document.point(ids.center).expect("center").position,
        ]
    }

    fn positive_pantograph_arms(output: [f64; 2]) -> ([f64; 2], [f64; 2]) {
        let distance = output[0].hypot(output[1]);
        let input_radius = 17.0_f64.sqrt();
        let guide_radius = 10.0_f64.sqrt();
        assert!(
            distance > input_radius - guide_radius && distance < input_radius + guide_radius,
            "output {output:?} is outside the regular pantograph annulus"
        );
        let direction = [output[0] / distance, output[1] / distance];
        let axial = (distance * distance + 7.0) / (2.0 * distance);
        let transverse = (17.0 - axial * axial).sqrt();
        let input = [
            axial * direction[0] + transverse * direction[1],
            axial * direction[1] - transverse * direction[0],
        ];
        let guide = [output[0] - input[0], output[1] - input[1]];
        (input, guide)
    }

    fn polar_point(radius: f64, degrees: f64) -> [f64; 2] {
        let angle = degrees.to_radians();
        [radius * angle.cos(), radius * angle.sin()]
    }

    fn assert_pantograph_geometry(
        role: &str,
        document: &SketchDocument,
        ids: geosolve_sketch::MotionPantographIds,
        expected_output: [f64; 2],
        expected_branches: [[f64; 2]; 4],
        previous: [[f64; 2]; 5],
    ) -> [[f64; 2]; 5] {
        let positions = pantograph_positions(document, ids);
        let [anchor, input, guide, output, center] = positions;
        let (expected_input, expected_guide) = positive_pantograph_arms(expected_output);
        for (name, actual, expected) in [
            ("anchor", anchor, [0.0, 0.0]),
            ("input", input, expected_input),
            ("guide", guide, expected_guide),
            ("output", output, expected_output),
            (
                "center",
                [2.0 * center[0], 2.0 * center[1]],
                expected_output,
            ),
        ] {
            assert!(
                (actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= 2.0e-7,
                "{role} drag produced the wrong {name}: expected={expected:?}, actual={actual:?}"
            );
        }
        assert!(
            (output[0] - input[0] - guide[0]).hypot(output[1] - input[1] - guide[1]) <= 1.0e-8,
            "{role} drag lost affine closure C = A + B: A={input:?}, B={guide:?}, C={output:?}"
        );
        let signed_orientation = input[0] * guide[1] - input[1] * guide[0];
        assert!(
            signed_orientation >= 5.0,
            "{role} drag left the positive regular assembly: A={input:?}, B={guide:?}, \
             signed orientation={signed_orientation}"
        );
        for (name, index) in [("input A", 1), ("guide B", 2)] {
            let step = (positions[index][0] - previous[index][0])
                .hypot(positions[index][1] - previous[index][1]);
            assert!(
                step <= 2.0,
                "{role} drag made a discontinuous {name} step: previous={:?}, current={:?}",
                previous[index],
                positions[index]
            );
        }
        for (bar, expected) in ids.bars.into_iter().zip(expected_branches) {
            let CurveDefinition::Line {
                branch_direction, ..
            } = document.curve(bar).expect("pantograph bar").definition
            else {
                panic!("pantograph bars remain lines")
            };
            assert_branch_direction_eq(branch_direction, expected, role);
        }
        positions
    }

    fn assert_accepted_pantograph_work(
        role: &str,
        index: usize,
        expected_point: DesignPointId,
        expected_design: SketchDesignIdentity,
        expected_accepted: SketchAcceptedStateIdentity,
        work: &ProjectedDragWorkEvidence,
    ) {
        assert_eq!(work.point, expected_point, "{role}: {work:#?}");
        assert_eq!(work.attempts, 1, "{role}: {work:#?}");
        assert_eq!(work.continued, index > 0, "{role}: {work:#?}");
        assert!(work.accepted, "{role}: {work:#?}");
        assert_eq!(work.rejection_stage, None, "{role}: {work:#?}");
        assert!(
            work.operation.stopping_reason.is_none(),
            "{role}: {work:#?}"
        );
        assert_operation_within_limits(&work.operation);
        assert!(
            work.operation.consumed.factorizations > 0
                && work.operation.consumed.nonlinear_iterations > 0
                && work.operation.consumed.component_linearizations > 0,
            "{role}: accepted work must expose real bounded solver work: {work:#?}"
        );
        assert!(
            work.operation.consumed.factorizations <= PANTOGRAPH_POINTER_MAX_FACTORIZATIONS
                && work.operation.consumed.nonlinear_iterations
                    <= PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
            "{role}: {work:#?}"
        );
        let solve = work.solve.expect("accepted drag solve summary");
        assert!(solve.components > 0, "{role}: {work:#?}");
        let locality = work.locality_plan().expect("pantograph locality plan");
        assert_eq!(locality.design_identity(), expected_design);
        assert_eq!(locality.accepted_state_identity(), expected_accepted);
        assert_eq!(locality.point(), expected_point);
        assert_eq!(locality.hard_degrees_of_freedom(), 2);
        assert_eq!(locality.active_rank(), 2);
        assert_eq!(locality.passive_degrees_of_freedom(), 0);
        assert!(locality.anchors().is_empty());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn pantograph_output_and_center_follow_wide_reversible_positive_assembly_paths() {
        let output_path = [
            polar_point(6.0, 50.0),
            polar_point(5.7, 62.0),
            polar_point(5.3, 74.0),
            polar_point(4.9, 86.0),
            polar_point(4.7, 95.0),
            polar_point(4.9, 86.0),
            polar_point(5.3, 74.0),
            polar_point(5.7, 62.0),
            polar_point(6.0, 50.0),
            [5.0, 4.0],
        ];
        for (pointer_id, role, use_center) in [(95, "output", false), (96, "center", true)] {
            let fixture = alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).unwrap();
            let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
                unreachable!()
            };
            let point = if use_center { ids.center } else { ids.output };
            let mut coordinator = RetainedEditorCoordinator::new(
                RetainedSketchDocumentSession::new(
                    fixture.document,
                    fixture.request,
                    SolverConfig::default(),
                )
                .unwrap(),
            )
            .unwrap();
            let baseline_design = coordinator.session().design_identity();
            let baseline_accepted = coordinator
                .session()
                .accepted_state()
                .expect("accepted pantograph")
                .identity();
            let accepted = coordinator
                .session()
                .accepted_state()
                .expect("accepted pantograph")
                .document();
            let branch_directions = ids.bars.map(|bar| {
                let CurveDefinition::Line {
                    branch_direction, ..
                } = accepted.curve(bar).expect("pantograph bar").definition
                else {
                    panic!("pantograph bars are lines")
                };
                branch_direction
            });
            let initial_positions = pantograph_positions(accepted, ids);
            let mut previous_positions = initial_positions;
            let parallel_relations = accepted
                .constraints()
                .iter()
                .filter_map(|constraint| match constraint.definition {
                    DocumentConstraintDefinition::Parallel { first, second } => {
                        Some((first.curve, second.curve))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                parallel_relations,
                vec![(ids.bars[0], ids.bars[3]), (ids.bars[1], ids.bars[2])],
                "the representative fixture must exercise ordinary Parallel relations"
            );
            let mut maximum_factorizations = 0;
            let mut maximum_iterations = 0;
            let mut total_factorizations = 0;
            let mut total_iterations = 0;

            for (index, expected_output) in output_path.into_iter().enumerate() {
                let target = if use_center {
                    [0.5 * expected_output[0], 0.5 * expected_output[1]]
                } else {
                    expected_output
                };
                let _ = coordinator.resolve_projected_point_move(
                    pointer_id,
                    u64::try_from(index + 1).unwrap(),
                    point,
                    target,
                );
                let work = coordinator
                    .projected_drag_work_evidence()
                    .expect("projected drag work");
                assert_accepted_pantograph_work(
                    role,
                    index,
                    point,
                    baseline_design,
                    baseline_accepted,
                    work,
                );
                maximum_factorizations =
                    maximum_factorizations.max(work.operation.consumed.factorizations);
                maximum_iterations =
                    maximum_iterations.max(work.operation.consumed.nonlinear_iterations);
                total_factorizations += work.operation.consumed.factorizations;
                total_iterations += work.operation.consumed.nonlinear_iterations;
                let preview = coordinator
                    .solved_preview_session()
                    .expect("accepted preview")
                    .accepted_state()
                    .expect("accepted preview state")
                    .document();
                let position = preview.point(point).expect("drag point").position;
                assert!(
                    (position[0] - target[0]).hypot(position[1] - target[1]) <= 1.0e-8,
                    "{role} did not follow its locally attainable target: target={target:?}, \
                     accepted={position:?}"
                );
                previous_positions = assert_pantograph_geometry(
                    role,
                    preview,
                    ids,
                    expected_output,
                    branch_directions,
                    previous_positions,
                );
            }
            for (name, returned, initial) in ["anchor", "input", "guide", "output", "center"]
                .into_iter()
                .zip(previous_positions)
                .zip(initial_positions)
                .map(|((name, returned), initial)| (name, returned, initial))
            {
                assert!(
                    (returned[0] - initial[0]).hypot(returned[1] - initial[1]) <= 2.0e-7,
                    "{role} full-path reversal changed {name}: initial={initial:?}, \
                     returned={returned:?}"
                );
            }
            assert!(
                maximum_factorizations <= PANTOGRAPH_POINTER_MAX_FACTORIZATIONS,
                "{role}: {maximum_factorizations}"
            );
            assert!(
                maximum_iterations <= PANTOGRAPH_POINTER_MAX_NONLINEAR_ITERATIONS,
                "{role}: {maximum_iterations}"
            );
            assert!(
                total_factorizations <= PANTOGRAPH_WIDE_PATH_MAX_FACTORIZATIONS,
                "{role}: {total_factorizations}"
            );
            assert!(
                total_iterations <= PANTOGRAPH_WIDE_PATH_MAX_NONLINEAR_ITERATIONS,
                "{role}: {total_iterations}"
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn difficult_twin_roller_projection_is_bounded_and_recovery_retains_continuation() {
        for (pointer_id, drag_left, first_parameter, recovery_parameter, difficult_target) in [
            (93, true, 0.26, 0.28, [-8.0, 0.0]),
            (97, false, 0.74, 0.72, [8.0, 0.0]),
        ] {
            let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).unwrap();
            let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
                unreachable!()
            };
            let mut coordinator = RetainedEditorCoordinator::new(
                RetainedSketchDocumentSession::new(
                    fixture.document,
                    fixture.request,
                    SolverConfig::default(),
                )
                .unwrap(),
            )
            .unwrap();
            let (active, passive) = if drag_left {
                (ids.left_center, ids.right_center)
            } else {
                (ids.right_center, ids.left_center)
            };
            let passive_baseline = coordinator
                .session()
                .accepted_state()
                .unwrap()
                .document()
                .point(passive)
                .unwrap()
                .position;

            let first_target = motion_cam_roller_center(first_parameter);
            let _ = coordinator.resolve_projected_point_move(pointer_id, 1, active, first_target);
            let first = coordinator.projected_drag_work_evidence().unwrap().clone();
            assert_eq!(first.attempts, 1, "{first:#?}");
            assert!(first.accepted, "{first:#?}");
            let locality = first.locality_plan().expect("roller locality plan");
            assert_eq!(locality.point(), active);
            assert_eq!(locality.anchors().len(), 1);
            assert_eq!(locality.anchors()[0].point(), passive);
            assert_eq!(
                locality.anchors()[0].target().map(f64::to_bits),
                passive_baseline.map(f64::to_bits)
            );
            let frozen_locality = first.locality.clone();
            assert!(
                first.operation.consumed.factorizations <= PROJECTED_DRAG_MAX_FACTORIZATIONS
                    && first.operation.consumed.nonlinear_iterations
                        <= PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS,
                "{first:#?}"
            );
            let retained_preview = coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap();
            let retained_preview_identity = retained_preview.identity();
            let retained_active = retained_preview.document().point(active).unwrap().position;
            let retained_passive = retained_preview.document().point(passive).unwrap().position;
            assert!(
                (retained_active[0] - first_target[0]).hypot(retained_active[1] - first_target[1])
                    <= 5.0e-8,
                "active roller did not reach its exact cam-offset target: \
                 target={first_target:?}, accepted={retained_active:?}"
            );
            assert!(
                (retained_passive[0] - passive_baseline[0])
                    .hypot(retained_passive[1] - passive_baseline[1])
                    <= 1.0e-8,
                "passive roller moved from {passive_baseline:?} to {retained_passive:?}"
            );

            let _ =
                coordinator.resolve_projected_point_move(pointer_id, 2, active, difficult_target);
            let difficult = coordinator.projected_drag_work_evidence().unwrap().clone();
            assert_eq!(difficult.attempts, 1, "{difficult:#?}");
            assert!(!difficult.accepted, "{difficult:#?}");
            assert_eq!(difficult.locality, frozen_locality);
            assert!(
                difficult.operation.consumed.factorizations <= PROJECTED_DRAG_MAX_FACTORIZATIONS
                    && difficult.operation.consumed.nonlinear_iterations
                        <= PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS,
                "{difficult:#?}"
            );
            let rejected_preview = coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap();
            assert_eq!(rejected_preview.identity(), retained_preview_identity);
            assert_eq!(
                rejected_preview
                    .document()
                    .point(active)
                    .unwrap()
                    .position
                    .map(f64::to_bits),
                retained_active.map(f64::to_bits),
                "a rejected sample changed the retained active geometry"
            );
            assert_eq!(
                rejected_preview
                    .document()
                    .point(passive)
                    .unwrap()
                    .position
                    .map(f64::to_bits),
                retained_passive.map(f64::to_bits),
                "a rejected sample changed the retained passive geometry"
            );

            let recovery_target = motion_cam_roller_center(recovery_parameter);
            let _ =
                coordinator.resolve_projected_point_move(pointer_id, 3, active, recovery_target);
            let recovered = coordinator.projected_drag_work_evidence().unwrap().clone();
            assert_eq!(recovered.attempts, 1, "{recovered:#?}");
            assert!(recovered.accepted, "{recovered:#?}");
            assert!(recovered.continued);
            assert_eq!(recovered.locality, frozen_locality);
            assert!(
                recovered.operation.consumed.factorizations <= PROJECTED_DRAG_MAX_FACTORIZATIONS
                    && recovered.operation.consumed.nonlinear_iterations
                        <= PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS,
                "{recovered:#?}"
            );
            let recovered_document = coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .document();
            let recovered_active = recovered_document.point(active).unwrap().position;
            let recovered_passive = recovered_document.point(passive).unwrap().position;
            assert!(
                (recovered_active[0] - recovery_target[0])
                    .hypot(recovered_active[1] - recovery_target[1])
                    <= 5.0e-8,
                "recovery did not return the active roller to its nearby cam-offset target: \
                 target={recovery_target:?}, accepted={recovered_active:?}"
            );
            assert!(
                (recovered_active[0] - retained_active[0])
                    .hypot(recovered_active[1] - retained_active[1])
                    <= 0.25,
                "recovery jumped away from the retained continuation"
            );
            assert!(
                (recovered_passive[0] - passive_baseline[0])
                    .hypot(recovered_passive[1] - passive_baseline[1])
                    <= 1.0e-8,
                "recovery moved passive roller from {passive_baseline:?} \
                 to {recovered_passive:?}"
            );
        }
    }

    #[test]
    #[allow(
        clippy::default_trait_access,
        clippy::too_many_lines,
        reason = "the branch-continuity regression keeps setup, preview, release, and undo evidence together"
    )]
    fn constrained_release_preserves_exact_preview_seed_branch_and_one_step_undo() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let base = document.add_point("base", [0.0, 0.0]).expect("point");
        let elbow = document.add_point("elbow", [1.0, 1.0]).expect("point");
        let end = document.add_point("end", [2.0, 0.0]).expect("point");
        let diagonal = 0.5_f64.sqrt();
        let first_link = document
            .add_curve(
                "first link",
                CurveDefinition::Line {
                    start: base,
                    end: elbow,
                    branch_direction: [diagonal, diagonal],
                },
            )
            .expect("line");
        let second_link = document
            .add_curve(
                "second link",
                CurveDefinition::Line {
                    start: elbow,
                    end,
                    branch_direction: [0.0, -1.0],
                },
            )
            .expect("line");
        document
            .add_constraint(
                "fixed base",
                DocumentConstraintDefinition::FixedPoint {
                    point: base,
                    target: [0.0, 0.0],
                },
            )
            .expect("constraint");
        for (label, first, second) in [("first length", base, elbow), ("second length", elbow, end)]
        {
            let target = document
                .add_scalar(
                    label,
                    2.0_f64.sqrt(),
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .expect("length");
            document
                .add_dimension(
                    label,
                    DocumentDimensionDefinition::PointDistance {
                        first,
                        second,
                        target,
                    },
                    DocumentDimensionMode::Driving,
                )
                .expect("dimension");
        }
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let initial = session
            .accepted_state()
            .expect("initial accepted")
            .document()
            .clone();
        let mut cold_release = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let _ = coordinator.resolve_projected_point_move(91, 1, end, [0.0, 0.0]);
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("projected drag work");
        assert_eq!(work.attempts, 1);
        assert!(work.accepted, "{work:#?}");
        let preview = coordinator
            .solved_preview_session()
            .expect("accepted preview")
            .clone();
        let preview_document = preview
            .accepted_state()
            .expect("preview accepted state")
            .document()
            .clone();
        let release_position = preview_document.point(end).expect("preview point").position;

        cold_release
            .apply(
                cold_release.design_identity(),
                DocumentEdit::SetPointPosition {
                    point: end,
                    position: release_position,
                },
            )
            .expect("former cold release");
        let cold_elbow = cold_release
            .accepted_state()
            .expect("cold release accepted")
            .document()
            .point(elbow)
            .expect("cold elbow")
            .position;
        let preview_elbow = preview_document
            .point(elbow)
            .expect("preview elbow")
            .position;
        assert!(
            (cold_elbow[0] - preview_elbow[0]).hypot(cold_elbow[1] - preview_elbow[1]) > 0.5,
            "former pre-drag seeded release must expose the branch/configuration jump"
        );

        coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: coordinator.session().design_identity(),
                point: end,
                model_position: release_position,
            })
            .expect("release commit")
            .expect("point mutation");

        let committed = coordinator
            .session()
            .accepted_state()
            .expect("committed accepted")
            .document();
        for point in [base, elbow, end] {
            let preview_position = preview_document
                .point(point)
                .expect("preview point")
                .position;
            let committed_position = committed.point(point).expect("committed point").position;
            for axis in 0..2 {
                assert!((preview_position[axis] - committed_position[axis]).abs() <= 1.0e-10);
            }
        }
        let branch = |document: &SketchDocument, curve| match &document
            .curve(curve)
            .expect("line")
            .definition
        {
            CurveDefinition::Line {
                branch_direction, ..
            } => *branch_direction,
            _ => panic!("line expected"),
        };
        for curve in [first_link, second_link] {
            assert_eq!(
                branch(committed, curve).map(f64::to_bits),
                branch(&preview_document, curve).map(f64::to_bits)
            );
        }
        assert_eq!(coordinator.history_len(), 2);

        coordinator.undo().expect("one-step undo");
        assert_eq!(coordinator.history_cursor(), 0);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("restored accepted")
                .document(),
            &initial
        );
    }

    #[test]
    fn mismatched_preview_commit_retains_preview_for_a_correct_retry() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let mut preview = coordinator.session().clone();
        let position = [2.0, 0.0];
        preview
            .reattempt(
                preview.design_identity(),
                DocumentSolveRequest::default().with_drag(points[1], position),
            )
            .expect("accepted preview");
        coordinator
            .mark_solved_preview(&preview)
            .expect("retain solved preview");

        let lifecycle = coordinator.lifecycle();
        let design = coordinator.session().design_identity();
        let attempt = coordinator.session().last_attempt().identity();
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted state")
            .identity();
        let design_json = coordinator
            .session()
            .export_design_json()
            .expect("design JSON");
        let accepted_json = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON");
        let history = coordinator.history_len();
        let transcript = coordinator.transcript().len();

        assert!(matches!(
            coordinator.apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: design,
                point: points[1],
                model_position: [f64::from_bits(2.0_f64.to_bits() + 1), 0.0],
            }),
            Err(CoordinatorError::SolvedPreviewMismatch)
        ));
        assert_eq!(coordinator.lifecycle(), lifecycle);
        assert_eq!(coordinator.session().design_identity(), design);
        assert_eq!(coordinator.session().last_attempt().identity(), attempt);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted state")
                .identity(),
            accepted
        );
        assert_eq!(
            coordinator
                .session()
                .export_design_json()
                .expect("design JSON"),
            design_json
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON"),
            accepted_json
        );
        assert_eq!(coordinator.history_len(), history);
        assert_eq!(coordinator.transcript().len(), transcript);

        let committed = coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: design,
                point: points[1],
                model_position: position,
            })
            .expect("correct retry")
            .expect("point mutation");
        assert!(committed.published_accepted.is_some());
        assert_eq!(coordinator.history_len(), history + 1);
        assert_eq!(coordinator.transcript().len(), transcript + 1);
        assert_eq!(coordinator.lifecycle().status, LifecycleStatus::Accepted);
    }

    #[test]
    fn stale_preview_design_is_transient_and_lifecycle_neutral() {
        let (session, _, _, _) = fixed_line_session();
        let preview = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                expected,
                DocumentEdit::CreatePoint {
                    label: "new design".into(),
                    position: [4.0, 0.0],
                },
            )
            .expect("edit");
        coordinator.mark_solving();
        let lifecycle = coordinator.lifecycle();

        assert!(matches!(
            coordinator.mark_solved_preview(&preview),
            Err(CoordinatorError::PreviewStaleDesign)
        ));
        assert_eq!(coordinator.lifecycle(), lifecycle);
        assert!(matches!(
            coordinator.transient,
            Some(TransientLifecycle::Solving)
        ));
    }

    #[test]
    fn same_identity_divergent_parent_cannot_publish_a_solved_preview() {
        let (base, points, _, _) = fixed_line_session();
        let mut authoritative = base.clone();
        let mut divergent_parent = base;
        authoritative
            .reattempt(
                authoritative.design_identity(),
                DocumentSolveRequest::default(),
            )
            .expect("authoritative accepted parent");
        divergent_parent
            .reattempt(
                divergent_parent.design_identity(),
                DocumentSolveRequest::default(),
            )
            .expect("divergent accepted parent");
        assert_eq!(
            authoritative.accepted_state().unwrap().identity(),
            divergent_parent.accepted_state().unwrap().identity(),
            "independent publications deliberately collide in revision identity"
        );
        let mut preview = divergent_parent;
        preview
            .reattempt(
                preview.design_identity(),
                DocumentSolveRequest::default().with_drag(points[1], [2.0, 0.0]),
            )
            .expect("accepted divergent preview");

        let mut coordinator = RetainedEditorCoordinator::new(authoritative).expect("coordinator");
        let lifecycle = coordinator.lifecycle();
        assert!(matches!(
            coordinator.mark_solved_preview(&preview),
            Err(CoordinatorError::PreviewAcceptedStateMismatch)
        ));
        assert_eq!(coordinator.lifecycle(), lifecycle);
        assert!(coordinator.solved_preview_session().is_none());
    }

    #[test]
    fn same_revision_divergent_design_content_cannot_publish_a_solved_preview() {
        let (base, _, _, _) = fixed_line_session();
        let mut authoritative = base.clone();
        let mut divergent = base;
        authoritative
            .apply(
                authoritative.design_identity(),
                DocumentEdit::CreatePoint {
                    label: "authoritative-only".into(),
                    position: [3.0, 1.0],
                },
            )
            .expect("authoritative design");
        divergent
            .apply(
                divergent.design_identity(),
                DocumentEdit::CreatePoint {
                    label: "divergent-only".into(),
                    position: [-3.0, -1.0],
                },
            )
            .expect("divergent design");
        assert_eq!(authoritative.design_identity(), divergent.design_identity());
        assert_ne!(authoritative.design_document(), divergent.design_document());
        divergent
            .reattempt(divergent.design_identity(), DocumentSolveRequest::default())
            .expect("accepted divergent preview");

        let mut coordinator = RetainedEditorCoordinator::new(authoritative).expect("coordinator");
        let lifecycle = coordinator.lifecycle();
        assert!(matches!(
            coordinator.mark_solved_preview(&divergent),
            Err(CoordinatorError::PreviewStaleDesign)
        ));
        assert_eq!(coordinator.lifecycle(), lifecycle);
        assert!(coordinator.solved_preview_session().is_none());
    }

    #[test]
    fn foreign_preview_session_is_transient_and_lifecycle_neutral() {
        let (session, _, _, _) = fixed_line_session();
        let (foreign, _, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let lifecycle = coordinator.lifecycle();

        assert!(matches!(
            coordinator.mark_solved_preview(&foreign),
            Err(CoordinatorError::PreviewForeignDocument)
        ));
        assert_eq!(coordinator.lifecycle(), lifecycle);
        assert!(coordinator.transient.is_none());
    }

    #[test]
    fn rejected_preview_is_not_solved_preview_and_is_transient_neutral() {
        let (session, points, _, target) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let edit = DocumentEdit::CreateDimension {
            label: "conflict".into(),
            definition: DocumentDimensionDefinition::PointDistance {
                first: points[0],
                second: points[1],
                target,
            },
            mode: DocumentDimensionMode::Driving,
        };
        coordinator
            .apply_edit(coordinator.session().design_identity(), edit)
            .expect("retain rejected coordinator design");
        // Clone only after the rejected design publication so the preview shares its exact
        // process-local design provenance while owning a distinct rejected attempt.
        let mut preview = coordinator.session().clone();
        preview
            .reattempt(preview.design_identity(), DocumentSolveRequest::default())
            .expect("distinct rejected preview attempt");
        assert_eq!(
            preview.design_identity(),
            coordinator.session().design_identity()
        );
        assert_ne!(
            preview.last_attempt().identity(),
            coordinator.session().last_attempt().identity()
        );
        assert!(preview.last_attempt().accepted_state_identity().is_none());
        coordinator.mark_solving();
        let lifecycle = coordinator.lifecycle();
        assert_eq!(lifecycle.status, LifecycleStatus::Solving);
        assert!(matches!(
            coordinator.transient,
            Some(TransientLifecycle::Solving)
        ));

        assert!(matches!(
            coordinator.mark_solved_preview(&preview),
            Err(CoordinatorError::PreviewNotAccepted)
        ));
        assert_eq!(coordinator.lifecycle(), lifecycle);
        assert!(matches!(
            coordinator.transient,
            Some(TransientLifecycle::Solving)
        ));
        assert!(coordinator.solved_preview_session().is_none());
    }

    #[test]
    fn dimension_mode_transition_replays_and_undoes_without_stale_mutation() {
        let (session, points, _, _) = fixed_line_session();
        let replay_session = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection(points.map(SelectionItem::Point));
        let expected = coordinator.session().design_identity();
        let dimension = coordinator
            .add_point_distance_dimension(expected, DocumentDimensionMode::Driving, "length")
            .expect("driving dimension")
            .value;
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Dimension(dimension)]);
        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::SetDimensionMode(DocumentDimensionMode::Reference),
            state: ActionState::Enabled,
        }));
        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::SetDimensionMode(DocumentDimensionMode::Driving),
            state: ActionState::Disabled(DisabledReason::AlreadyInRequestedState),
        }));
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Point(points[0])]);
        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::SetDimensionMode(DocumentDimensionMode::Reference),
            state: ActionState::Disabled(DisabledReason::WrongOperandKind),
        }));
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Dimension(dimension)]);

        let stale = coordinator.session().design_identity();
        let reference = coordinator
            .set_dimension_mode(stale, dimension, DocumentDimensionMode::Reference)
            .expect("reference mode");
        assert!(
            matches!(reference.value, DocumentCommandEffect::UpdatedDimension(id) if id == dimension)
        );
        assert_eq!(coordinator.history_len(), 3);
        assert_eq!(
            coordinator.session().design_document().dimensions()[0].mode,
            DocumentDimensionMode::Reference
        );
        assert!(matches!(
            coordinator.set_dimension_mode(stale, dimension, DocumentDimensionMode::Driving),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleDesign { .. }
            ))
        ));
        assert_eq!(
            coordinator.session().design_document().dimensions()[0].mode,
            DocumentDimensionMode::Reference
        );
        assert!(matches!(
            coordinator.set_dimension_mode(stale, dimension, DocumentDimensionMode::Reference),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleDesign { .. }
            ))
        ));

        let expected = coordinator.session().design_identity();
        coordinator
            .set_dimension_mode(expected, dimension, DocumentDimensionMode::Driving)
            .expect("driving mode");
        coordinator.undo().expect("undo driving");
        assert_eq!(
            coordinator.session().design_document().dimensions()[0].mode,
            DocumentDimensionMode::Reference
        );
        coordinator.redo().expect("redo driving");
        assert_eq!(
            coordinator.session().design_document().dimensions()[0].mode,
            DocumentDimensionMode::Driving
        );

        let transcript = coordinator.transcript().to_vec();
        let mut replay = RetainedEditorCoordinator::new(replay_session).expect("replay");
        for action in &transcript[..3] {
            replay.replay(action).expect("replay action");
        }
        assert_eq!(
            replay.session().design_document().dimensions()[0].mode,
            DocumentDimensionMode::Driving
        );

        let expected = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                expected,
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(dimension),
                },
            )
            .expect("delete dimension");
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Dimension(dimension)]);
        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::SetDimensionMode(DocumentDimensionMode::Reference),
            state: ActionState::Disabled(DisabledReason::MissingObject),
        }));
    }

    #[test]
    fn accepted_measurements_withhold_stale_provenance() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [2.0, 1.0]).expect("point");
        let mut catalog = DocumentMeasurementCatalog::new(&mut document).expect("catalog");
        let source = catalog
            .add_measurement(
                &mut document,
                "horizontal distance",
                DocumentMeasurementDefinition::DimensionValue {
                    definition: DocumentM38DimensionDefinition::RelativeHorizontal {
                        first: DocumentPointRef::Point { point: first },
                        second: DocumentPointRef::Point { point: second },
                    },
                },
                DocumentMeasurementProvenance::AcceptedDocument { revision: 0 },
            )
            .expect("measurement");
        #[allow(clippy::default_trait_access)]
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        assert!(matches!(
            coordinator
                .accepted_measurements(&catalog, [source])
                .as_slice(),
            [MeasurementPublication::Published(_)]
        ));

        let expected = coordinator.session().design_identity();
        coordinator
            .apply_edit(
                expected,
                DocumentEdit::CreatePoint {
                    label: "new revision".into(),
                    position: [3.0, 3.0],
                },
            )
            .expect("accepted edit");
        assert!(matches!(
            coordinator
                .accepted_measurements(&catalog, [source])
                .as_slice(),
            [MeasurementPublication::Withheld { source: withheld, .. }] if *withheld == source
        ));
    }

    #[test]
    fn relation_availability_and_edit_building_are_prospective_until_one_coordinator_apply() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Point(points[0])]);
        let design = coordinator.session().design_identity();
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted")
            .identity();
        let design_json = coordinator
            .session()
            .export_design_json()
            .expect("design JSON");
        let accepted_json = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON");
        let history = coordinator.history_len();
        let transcript = coordinator.transcript().to_vec();

        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::Constraint(ConstraintIntent::Lock),
            state: ActionState::Enabled,
        }));
        let edit = coordinator
            .editor()
            .constraint_edit(
                coordinator.session().design_document(),
                ConstraintKind::Fixed,
                "prospective fixed",
            )
            .expect("prospective edit");
        assert!(matches!(
            edit,
            DocumentEdit::CreateConstraint {
                definition: DocumentConstraintDefinition::FixedPoint { point, target },
                ..
            } if point == points[0] && target == [0.0, 0.0]
        ));
        assert_eq!(coordinator.session().design_identity(), design);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted")
                .identity(),
            accepted
        );
        assert_eq!(
            coordinator
                .session()
                .export_design_json()
                .expect("design JSON"),
            design_json
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON"),
            accepted_json
        );
        assert_eq!(coordinator.history_len(), history);
        assert_eq!(coordinator.transcript(), transcript);

        let outcome = coordinator
            .apply_edit(design, edit)
            .expect("explicit apply");
        assert!(outcome.published_accepted.is_none());
        assert_ne!(coordinator.session().design_identity(), design);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("previous accepted state")
                .identity(),
            accepted
        );
        assert_eq!(coordinator.history_len(), history + 1);
        assert_eq!(coordinator.transcript().len(), transcript.len() + 1);
        assert!(matches!(
            coordinator.session().design_document().constraints().last(),
            Some(value) if matches!(
                value.definition,
                DocumentConstraintDefinition::FixedPoint { point, target }
                    if point == points[0] && target == [0.0, 0.0]
            )
        ));
    }

    #[test]
    fn staged_inference_is_non_authoritative_until_its_commit_effect_is_applied() {
        let (mut coordinator, candidate, expected, history) = inference_candidate_coordinator();

        assert_eq!(
            coordinator.editor_mut().stage_inference(candidate.clone()),
            vec![EditorEffect::PreviewInference(candidate.clone())]
        );
        assert_eq!(coordinator.editor().staged_inference(), Some(&candidate));
        assert_eq!(coordinator.session().design_identity(), expected);
        assert!(
            coordinator
                .session()
                .design_document()
                .constraints()
                .is_empty()
        );
        assert_eq!(coordinator.history_len(), history);

        assert_eq!(
            coordinator.editor_mut().cancel_inference(),
            vec![EditorEffect::ClearInferencePreview]
        );
        assert!(coordinator.editor().staged_inference().is_none());
        assert_eq!(coordinator.session().design_identity(), expected);
        assert!(
            coordinator
                .session()
                .design_document()
                .constraints()
                .is_empty()
        );
        assert_eq!(coordinator.history_len(), history);

        coordinator.editor_mut().stage_inference(candidate.clone());
        let confirmation = coordinator.editor_mut().confirm_inference();
        assert_eq!(
            confirmation,
            vec![
                EditorEffect::CommitInference(candidate.clone()),
                EditorEffect::ClearInferencePreview,
            ]
        );
        assert!(coordinator.editor().staged_inference().is_none());
        assert_eq!(coordinator.session().design_identity(), expected);
        assert!(
            coordinator
                .session()
                .design_document()
                .constraints()
                .is_empty()
        );
        assert_eq!(coordinator.history_len(), history);

        let outcome = coordinator
            .apply_editor_effect(&confirmation[0])
            .expect("inference commit")
            .expect("mutation");
        assert!(matches!(outcome.value, EditorMutation::Inference(_)));
        assert_eq!(
            coordinator.session().design_document().constraints().len(),
            1
        );
        assert_eq!(coordinator.history_len(), history + 1);

        let before_stale = coordinator.session().design_identity();
        let history = coordinator.history_len();
        coordinator.editor_mut().stage_inference(candidate);
        let stale_confirmation = coordinator.editor_mut().confirm_inference();
        assert!(matches!(
            coordinator.apply_editor_effect(&stale_confirmation[0]),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleDesign { .. }
            ))
        ));
        assert_eq!(coordinator.session().design_identity(), before_stale);
        assert_eq!(
            coordinator.session().design_document().constraints().len(),
            1
        );
        assert_eq!(coordinator.history_len(), history);
    }

    #[test]
    fn invalid_drafts_and_cancellation_dispatch_no_retained_mutation() {
        let (session, _, _, _) = fixed_line_session();
        let accepted = session.accepted_state().expect("accepted");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let snapshot = retained_state_snapshot(&coordinator);
        let design = snapshot.design;
        let center = scene.viewport.model_to_screen([0.0, 0.0]);

        coordinator.editor_mut().activate_tool(EditorTool::Circle);
        let anchor = coordinator.editor_mut().pointer_down(
            &scene,
            PointerInput {
                pointer_id: 1,
                position: center,
                modifiers: Modifiers::default(),
            },
        );
        let invalid = coordinator.editor_mut().pointer_down(
            &scene,
            PointerInput {
                pointer_id: 1,
                position: center,
                modifiers: Modifiers::default(),
            },
        );
        let cancelled = coordinator.editor_mut().cancel();
        assert!(
            anchor
                .iter()
                .all(|effect| !matches!(effect, EditorEffect::CommitConstruction { .. }))
        );
        assert!(
            invalid
                .iter()
                .all(|effect| !matches!(effect, EditorEffect::CommitConstruction { .. }))
        );
        assert_eq!(cancelled, vec![EditorEffect::ClearConstructionPreview]);

        coordinator.editor_mut().activate_tool(EditorTool::Polyline);
        let incomplete = coordinator.editor_mut().pointer_down(
            &scene,
            PointerInput {
                pointer_id: 2,
                position: center,
                modifiers: Modifiers::default(),
            },
        );
        let unfinished = coordinator.editor_mut().complete_draft(design);
        assert!(
            incomplete
                .iter()
                .all(|effect| !matches!(effect, EditorEffect::CommitConstruction { .. }))
        );
        assert!(
            unfinished
                .iter()
                .all(|effect| !matches!(effect, EditorEffect::CommitConstruction { .. }))
        );

        for effect in anchor
            .iter()
            .chain(&invalid)
            .chain(&cancelled)
            .chain(&incomplete)
            .chain(&unfinished)
        {
            assert!(
                coordinator
                    .apply_editor_effect(effect)
                    .expect("non-commit effect")
                    .is_none()
            );
        }
        assert_retained_state_snapshot(&coordinator, &snapshot);
    }
}
