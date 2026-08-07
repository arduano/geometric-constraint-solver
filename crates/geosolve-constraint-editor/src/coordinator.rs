// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained-design lifecycle coordination for presentation adapters.

use std::collections::{BTreeSet, HashSet};

use geosolve_sketch::{
    ContactBranchEdit, ContactDomain, ContactId, ContactNeighborhood, CurveDefinition, CurveId,
    CurveSpan, DesignPointId, DocumentAngleOrientation, DocumentCommandEffect,
    DocumentConstraintDefinition, DocumentCurveContinuity, DocumentCurveCurvatureRelation,
    DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode,
    DocumentDragLocalityPlan, DocumentEdit, DocumentElementId, DocumentExternalBindingId,
    DocumentMeasurementCatalog, DocumentMeasurementProvenance, DocumentMeasurementValue,
    DocumentObjectId, DocumentRuntimeMap, DocumentSessionError, DocumentSolveRequest,
    DocumentSourceId, DocumentSourceOwner, ExternalFeatureKindV1, ExternalSnapshotSet,
    ExternalTopologyDigest, GeometryRole, OperationControl, OperationController, OperationLimits,
    OperationOutcome, OperationReport, OperationWork, ParameterBatch,
    RetainedSketchDocumentSession, RuntimeCurve, ScalarDomain, ScalarUnit,
    SketchAcceptedDocumentRedundancy, SketchAcceptedStateIdentity, SketchAttemptFailure,
    SketchAttemptFailureKind, SketchAttemptIdentity, SketchBound, SketchDesignIdentity,
    SketchDocument, SketchLifecycleRevisionHighWater, SketchSolveResult, SketchSource,
    SolveRejection, TangentOrientation,
};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedEdgeId, ComputedEdgeProvenance, ComputedEvaluationAllocator,
    ComputedEvaluationAllocatorHighWater, ComputedFeatureAuthoringSnapshot,
    ComputedFeatureCornerId, ComputedFeatureDocument, ComputedFeatureDocumentError,
    ComputedFeatureDocumentIdentity, ComputedFeatureEvaluationError,
    ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureId,
    ComputedFeatureLifecycleHighWater, ComputedFeatureSnapshot, ComputedFeatureSnapshotError,
    NativeCurveSpanSource,
};
use geosolve_sketch_ops::{
    SketchOperationApplication, SketchOperationApplyError, SketchOperationError,
    SketchOperationIdentityChange, SketchOperationProposal, SketchOperationRequest,
    SketchOperationResult, SketchOperationSnapshot,
};
use thiserror::Error;

use crate::feature_authoring::resolve_feature_item_picks;
use crate::operation_authoring::resolve_operation_item_picks;
use crate::{
    ActionChoice, AuthoringApplication, AuthoringOperand, AuthoringOptions, AuthoringTool,
    ConstraintActionRequest, ConstraintEditor, ConstraintIntent, ConstraintKind,
    ConstraintRelationChoice, ConstructionProposal, ConstructionResult, DimensionActionRequest,
    DimensionKind, EditorEffect, EditorScene, FeatureAuthoringCandidate, FeatureAuthoringOptions,
    FeatureAuthoringOutcome, FeatureAuthoringPick, FeatureAuthoringState, FeatureAuthoringTool,
    OperationAuthoringCandidate, OperationAuthoringOutcome, OperationAuthoringPick,
    OperationAuthoringStage, OperationAuthoringTool, OperationAuthoringWarning,
    OperationAuthoringWarningKind, PickTolerance, PointGestureSnapshot, PointerInput,
    ProjectedDragRequestDisposition, ProvisionalInferenceCandidate, ResolvedConstraintKind,
    ScreenPoint, SelectionItem, Viewport,
};

const PROJECTED_DRAG_MAX_DOCUMENT_ITEMS: usize = 16_384;
const PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS: usize = 256;
const PROJECTED_DRAG_MAX_FACTORIZATIONS: usize = 256;
const PROJECTED_DRAG_MAX_RANK_KERNELS: usize = 256;
const PROJECTED_DRAG_MAX_REJECTED_TRIALS: usize = 512;
const PROJECTED_DRAG_MAX_COMPONENT_LINEARIZATIONS: usize = 1_024;
const PROJECTED_DRAG_MAX_DENSE_DIMENSION: usize = 256;
const PROJECTED_DRAG_MAX_DIAGNOSTIC_CANDIDATES: usize = 512;
const PROJECTED_DRAG_MAX_DIAGNOSTIC_TRIALS: usize = 1_024;

const BOUNDED_GEOMETRY_MAX_DOCUMENT_ITEMS: usize = 16_384;
const BOUNDED_GEOMETRY_MAX_NONLINEAR_ITERATIONS: usize = 256;
const BOUNDED_GEOMETRY_MAX_FACTORIZATIONS: usize = 256;
const BOUNDED_GEOMETRY_MAX_RANK_KERNELS: usize = 256;
const BOUNDED_GEOMETRY_MAX_REJECTED_TRIALS: usize = 512;
const BOUNDED_GEOMETRY_MAX_COMPONENT_LINEARIZATIONS: usize = 1_024;
const BOUNDED_GEOMETRY_MAX_DENSE_DIMENSION: usize = 256;
const BOUNDED_GEOMETRY_MAX_DIAGNOSTIC_CANDIDATES: usize = 512;
const BOUNDED_GEOMETRY_MAX_DIAGNOSTIC_TRIALS: usize = 1_024;
const BOUNDED_GEOMETRY_MAX_PROFILE_WORK: usize = 16_384;
const BOUNDED_GEOMETRY_MAX_MEASUREMENT_WORK: usize = 16_384;

/// Shared finite work envelope for accepted helper-operation and computed-feature
/// geometry. The callers own distinct domain semantics; only their bounded work
/// policy is shared here.
pub(crate) fn bounded_geometry_control() -> OperationControl {
    let mut control = OperationControl::unlimited();
    control.limits.document_validation_items = BOUNDED_GEOMETRY_MAX_DOCUMENT_ITEMS;
    control.limits.document_dependency_items = BOUNDED_GEOMETRY_MAX_DOCUMENT_ITEMS;
    control.limits.document_lowering_items = BOUNDED_GEOMETRY_MAX_DOCUMENT_ITEMS;
    control.limits.nonlinear_iterations = BOUNDED_GEOMETRY_MAX_NONLINEAR_ITERATIONS;
    control.limits.factorizations = BOUNDED_GEOMETRY_MAX_FACTORIZATIONS;
    control.limits.rank_kernels = BOUNDED_GEOMETRY_MAX_RANK_KERNELS;
    control.limits.rejected_trials = BOUNDED_GEOMETRY_MAX_REJECTED_TRIALS;
    control.limits.component_linearizations = BOUNDED_GEOMETRY_MAX_COMPONENT_LINEARIZATIONS;
    control.limits.dense_kernel_rows = BOUNDED_GEOMETRY_MAX_DENSE_DIMENSION;
    control.limits.dense_kernel_columns = BOUNDED_GEOMETRY_MAX_DENSE_DIMENSION;
    control.limits.diagnostic_candidates = BOUNDED_GEOMETRY_MAX_DIAGNOSTIC_CANDIDATES;
    control.limits.diagnostic_trials = BOUNDED_GEOMETRY_MAX_DIAGNOSTIC_TRIALS;
    control.limits.profile_candidate_pairs = BOUNDED_GEOMETRY_MAX_PROFILE_WORK;
    control.limits.profile_subdivisions = BOUNDED_GEOMETRY_MAX_PROFILE_WORK;
    control.limits.profile_roots = BOUNDED_GEOMETRY_MAX_PROFILE_WORK;
    control.limits.profile_fragments = BOUNDED_GEOMETRY_MAX_PROFILE_WORK;
    control.limits.profile_integrations = BOUNDED_GEOMETRY_MAX_PROFILE_WORK;
    control.limits.profile_containment_tests = BOUNDED_GEOMETRY_MAX_PROFILE_WORK;
    control.limits.profile_faces = BOUNDED_GEOMETRY_MAX_PROFILE_WORK;
    control.limits.measurement_integrations = BOUNDED_GEOMETRY_MAX_MEASUREMENT_WORK;
    control.limits.measurement_derivative_evaluations = BOUNDED_GEOMETRY_MAX_MEASUREMENT_WORK;
    control
}

/// Aggregate envelope for a grouped computed-feature authoring transition. The
/// ordinary geometry envelope was sized for one helper request; a batch shares
/// one controller and therefore needs an explicitly larger, still finite total
/// iterative allowance.
pub(crate) fn computed_feature_authoring_control() -> OperationControl {
    const MAX_AGGREGATE_ITERATIVE_WORK: usize = 16_384;
    let mut control = bounded_geometry_control();
    control.limits.nonlinear_iterations = MAX_AGGREGATE_ITERATIVE_WORK;
    control.limits.factorizations = MAX_AGGREGATE_ITERATIVE_WORK;
    control.limits.rank_kernels = MAX_AGGREGATE_ITERATIVE_WORK;
    control.limits.rejected_trials = MAX_AGGREGATE_ITERATIVE_WORK;
    control.limits.component_linearizations = MAX_AGGREGATE_ITERATIVE_WORK;
    control
}

fn projected_drag_control() -> OperationControl {
    let mut control = OperationControl::unlimited();
    control.limits.document_validation_items = PROJECTED_DRAG_MAX_DOCUMENT_ITEMS;
    control.limits.document_dependency_items = PROJECTED_DRAG_MAX_DOCUMENT_ITEMS;
    control.limits.document_lowering_items = PROJECTED_DRAG_MAX_DOCUMENT_ITEMS;
    control.limits.nonlinear_iterations = PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS;
    control.limits.factorizations = PROJECTED_DRAG_MAX_FACTORIZATIONS;
    control.limits.rank_kernels = PROJECTED_DRAG_MAX_RANK_KERNELS;
    control.limits.rejected_trials = PROJECTED_DRAG_MAX_REJECTED_TRIALS;
    control.limits.component_linearizations = PROJECTED_DRAG_MAX_COMPONENT_LINEARIZATIONS;
    control.limits.dense_kernel_rows = PROJECTED_DRAG_MAX_DENSE_DIMENSION;
    control.limits.dense_kernel_columns = PROJECTED_DRAG_MAX_DENSE_DIMENSION;
    control.limits.diagnostic_candidates = PROJECTED_DRAG_MAX_DIAGNOSTIC_CANDIDATES;
    control.limits.diagnostic_trials = PROJECTED_DRAG_MAX_DIAGNOSTIC_TRIALS;
    control
}

fn evaluate_computed_features(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    allocator: &mut ComputedEvaluationAllocator,
    control: OperationControl,
) -> Result<OperationOutcome<ComputedFeatureSnapshot>, CoordinatorError> {
    Ok(ComputedFeatureEvaluationSnapshot::capture(
        session,
        features,
        ComputedFeatureEvaluationPolicy::default(),
    )?
    .prepare(allocator)?
    .execute(control)?)
}

fn require_current_feature_authoring_evaluation(
    snapshot: &ComputedFeatureSnapshot,
    feature: ComputedFeatureId,
) -> Result<(), CoordinatorError> {
    match snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .map(|evaluation| &evaluation.state)
    {
        Some(ComputedFeatureEvaluationState::Current { .. }) => Ok(()),
        Some(ComputedFeatureEvaluationState::Failed { failure }) => Err(
            CoordinatorError::FeatureAuthoringPreviewRejected(failure.clone()),
        ),
        Some(ComputedFeatureEvaluationState::Suppressed) | None => {
            Err(CoordinatorError::ComputedFeatureWorkStopped)
        }
    }
}

const fn merge_feature_lifecycle_high_water(
    first: ComputedFeatureLifecycleHighWater,
    second: ComputedFeatureLifecycleHighWater,
) -> ComputedFeatureLifecycleHighWater {
    ComputedFeatureLifecycleHighWater {
        revision: if first.revision.raw() >= second.revision.raw() {
            first.revision
        } else {
            second.revision
        },
        allocator: geosolve_sketch_features::ComputedFeatureAllocatorHighWater {
            next_feature_id: if first.allocator.next_feature_id.raw()
                >= second.allocator.next_feature_id.raw()
            {
                first.allocator.next_feature_id
            } else {
                second.allocator.next_feature_id
            },
            next_corner_id: if first.allocator.next_corner_id.raw()
                >= second.allocator.next_corner_id.raw()
            {
                first.allocator.next_corner_id
            } else {
                second.allocator.next_corner_id
            },
        },
    }
}

fn complete_projected_drag_release<T>(
    outcome: OperationOutcome<T>,
) -> Result<T, DocumentSessionError> {
    match outcome {
        OperationOutcome::Completed { value, .. } => Ok(value),
        stopped => {
            let Some(stopping_reason) = stopped.report().stopping_reason else {
                return Err(DocumentSessionError::PreviewReleaseMismatch);
            };
            Err(DocumentSessionError::PreviewReleaseInterrupted { stopping_reason })
        }
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

/// Opaque, application-persistable restore material for one history position.
#[derive(Clone, Debug)]
pub struct RestoreCheckpoint {
    design_json: String,
    design_is_draft_v5: bool,
    accepted_json: Option<String>,
    accepted_is_draft_v5: bool,
    accepted_belongs_to_current_design: bool,
    revisions: SketchLifecycleRevisionHighWater,
    feature_json: String,
    feature_lifecycle: ComputedFeatureLifecycleHighWater,
    evaluation_allocator: ComputedEvaluationAllocatorHighWater,
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

    /// Whether the accepted materialization was published for this checkpoint's
    /// current retained design rather than inherited from an older design.
    ///
    /// Persistence adapters may retain this provenance to select exact
    /// certification on reload. Older payloads that did not store the
    /// relationship must conservatively treat it as false.
    #[must_use]
    pub const fn accepted_belongs_to_current_design(&self) -> bool {
        self.accepted_belongs_to_current_design
    }

    /// Never-reuse lifecycle revision metadata.
    #[must_use]
    pub const fn revisions(&self) -> SketchLifecycleRevisionHighWater {
        self.revisions
    }

    /// Canonical computed-feature sidecar JSON stored beside the sketch payload.
    #[must_use]
    pub fn feature_json(&self) -> &str {
        &self.feature_json
    }

    /// Never-reuse feature revision and identity allocator metadata.
    #[must_use]
    pub const fn feature_lifecycle_high_water(&self) -> ComputedFeatureLifecycleHighWater {
        self.feature_lifecycle
    }

    /// Never-reuse generated-edge evaluation identity metadata.
    #[must_use]
    pub const fn computed_evaluation_high_water(&self) -> ComputedEvaluationAllocatorHighWater {
        self.evaluation_allocator
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
    LocalityPlanning,
    ControlledOperation,
    Session,
    AttemptInput,
    Solve,
    AcceptedState,
    PreviewPublication,
}

/// Deterministic work evidence for exactly one projected pointer sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectedDragWorkEvidence {
    pub pointer_id: u64,
    pub request_id: u64,
    pub point: DesignPointId,
    /// Whether the sample used the last independently accepted preview as its numerical parent.
    pub continued: bool,
    /// Ordinary projected dragging always performs exactly one retained solve attempt.
    pub attempts: u8,
    pub accepted: bool,
    pub passive_degrees_of_freedom: usize,
    pub anchor_count: usize,
    pub rejection_stage: Option<ProjectedDragRejectionStage>,
    /// Whether [`Self::operation`] contains every unit consumed by this sample.
    ///
    /// Controlled outcomes always carry their complete report. A typed lower-layer
    /// error currently does not, so the coordinator retains the report prefix it
    /// owns and marks that evidence incomplete instead of silently under-reporting.
    pub operation_report_complete: bool,
    pub operation: OperationReport,
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

/// Stable attribution for one current computed-feature failure.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedFeatureProblemMetadata {
    pub feature: Option<ComputedFeatureId>,
    pub corners: Vec<ComputedFeatureCornerId>,
    pub sources: Vec<NativeCurveSpanSource>,
    pub scope: EditorProblemScope,
    pub message: String,
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

/// Independently accepted scratch-preview metadata for one helper operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct OperationAuthoringPreviewToken(u64);

/// Independently accepted scratch-preview metadata for one helper operation.
#[derive(Clone, Debug, PartialEq)]
pub struct OperationAuthoringPreviewMetadata {
    /// Opaque coordinator generation required together with the exact candidate
    /// when committing this preview.
    pub token: OperationAuthoringPreviewToken,
    pub tool: OperationAuthoringTool,
    pub base_design: SketchDesignIdentity,
    pub accepted: SketchAcceptedStateIdentity,
    pub primary_created_curve: CurveId,
    pub created_curves: Vec<CurveId>,
    pub created_points: Vec<DesignPointId>,
    /// True only after all headless semantic stages are confirmed. The preview is
    /// independently accepted in either case; an unconfirmed Fillet-radius hover
    /// is renderable but cannot enter the commit path.
    pub apply_ready: bool,
}

/// Result of coordinator-owned scratch preparation. A ready result is also held
/// by the coordinator; presentation never receives a proposal it can apply itself.
#[derive(Clone, Debug, PartialEq)]
pub enum OperationAuthoringPreviewOutcome {
    Ready(OperationAuthoringPreviewMetadata),
    Warning(OperationAuthoringWarning),
}

/// Accepted helper-operation commit result.
#[derive(Clone, Debug)]
pub struct OperationAuthoringMutation {
    pub operation: MutationOutcome<SketchOperationApplication>,
    pub primary_created_curve: CurveId,
}

/// Exact persistent-intent result of one computed-feature mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedFeatureMutation<T> {
    pub value: T,
    pub before: ComputedFeatureDocumentIdentity,
    pub after: ComputedFeatureDocumentIdentity,
}

/// Exact paired output for composite scene construction. `Withheld` means a
/// visible sketch preview exists but computed evaluation for that same input did
/// not complete, so presentation must render native preview geometry without a
/// stale computed ghost.
#[derive(Clone, Copy, Debug)]
pub enum ComputedSceneState<'a> {
    Current {
        expected: &'a geosolve_sketch_features::ComputedFeatureEvaluationInput,
        snapshot: &'a ComputedFeatureSnapshot,
    },
    Withheld,
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FeatureAuthoringPreviewToken(u64);

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureAuthoringPreviewMetadata {
    pub token: FeatureAuthoringPreviewToken,
    pub feature: ComputedFeatureId,
    pub feature_identity: ComputedFeatureDocumentIdentity,
    pub input: geosolve_sketch_features::ComputedFeatureEvaluationInput,
}

/// One coordinator-accepted authoring-state transition and its optional exact
/// provisional feature. On [`Err`], neither the supplied authoring state nor a
/// previously held preview is replaced.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct FeatureAuthoringTransaction {
    pub outcome: FeatureAuthoringOutcome,
    pub preview: Option<FeatureAuthoringPreviewMetadata>,
}

/// Stable temporary owner mapped to its grouped candidate occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureAuthoringCornerBinding {
    pub owner: ComputedCornerRef,
    pub candidate_index: usize,
}

#[derive(Clone, Debug)]
pub struct FeatureAuthoringPreview {
    candidate: FeatureAuthoringCandidate,
    expected: ComputedFeatureDocumentIdentity,
    features: ComputedFeatureDocument,
    snapshot: ComputedFeatureSnapshot,
    metadata: FeatureAuthoringPreviewMetadata,
    label: String,
    /// Exact pointer-down state retained for transactional gesture cancellation.
    /// This is one boxed checkpoint, not an unbounded preview history.
    radius_origin: Box<FeatureAuthoringRadiusOrigin>,
}

#[derive(Clone, Debug)]
struct FeatureAuthoringRadiusOrigin {
    candidate: FeatureAuthoringCandidate,
    features: ComputedFeatureDocument,
    snapshot: ComputedFeatureSnapshot,
    metadata: FeatureAuthoringPreviewMetadata,
}

impl FeatureAuthoringPreview {
    #[must_use]
    pub const fn metadata(&self) -> &FeatureAuthoringPreviewMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn snapshot(&self) -> &ComputedFeatureSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub const fn candidate(&self) -> &FeatureAuthoringCandidate {
        &self.candidate
    }

    /// Maps evaluation-stable preview owners to grouped candidate order. Radius
    /// refreshes rebuild the same temporary feature/corner identity allocation,
    /// so a selected owner keeps its occurrence meaning for the gesture.
    #[must_use]
    pub fn corner_bindings(&self) -> Vec<FeatureAuthoringCornerBinding> {
        let Some(feature) = self.features.feature(self.metadata.feature) else {
            return Vec::new();
        };
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &feature.definition;
        fillet
            .corners
            .iter()
            .enumerate()
            .map(|(candidate_index, corner)| FeatureAuthoringCornerBinding {
                owner: ComputedCornerRef {
                    feature: feature.id,
                    corner: corner.id,
                },
                candidate_index,
            })
            .collect()
    }

    #[must_use]
    pub fn corner_index(&self, owner: ComputedCornerRef) -> Option<usize> {
        self.corner_bindings()
            .into_iter()
            .find_map(|binding| (binding.owner == owner).then_some(binding.candidate_index))
    }

    fn accepts_radius_input(
        &self,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
    ) -> bool {
        self.radius_origin.metadata.input == *expected || self.snapshot.input() == *expected
    }
}

/// Opaque coordinator-held accepted operation preview.
#[derive(Clone, Debug)]
pub struct OperationAuthoringPreview {
    candidate: OperationAuthoringCandidate,
    proposal: SketchOperationProposal,
    scratch: RetainedSketchDocumentSession,
    metadata: OperationAuthoringPreviewMetadata,
}

impl OperationAuthoringPreview {
    #[must_use]
    pub const fn metadata(&self) -> &OperationAuthoringPreviewMetadata {
        &self.metadata
    }

    /// Whether this held accepted preview is bound to the exact current headless
    /// candidate. Hosts may use this only for presentation gating; commit repeats
    /// the same comparison authoritatively.
    #[must_use]
    pub fn matches_candidate(&self, candidate: &OperationAuthoringCandidate) -> bool {
        &self.candidate == candidate
    }

    /// The independently accepted scratch document used for preview rendering.
    #[must_use]
    pub fn accepted_document(&self) -> &SketchDocument {
        self.accepted_state().document()
    }

    /// Complete accepted scratch state used by truthful provenance-aware scene
    /// renderers. It is read-only and belongs to the same session as [`Self::scene`].
    ///
    /// # Panics
    ///
    /// Panics only if the private preview invariant is violated and a preview is
    /// constructed without accepted scratch publication.
    #[must_use]
    pub fn accepted_state(&self) -> &geosolve_sketch::SketchAcceptedDocumentState {
        self.scratch
            .accepted_state()
            .expect("operation previews are stored only after accepted publication")
    }

    /// Builds a complete preview scene from the independently accepted scratch
    /// state through the same public scene adapter used for ordinary geometry.
    ///
    /// # Errors
    ///
    /// Returns an editor presentation error when the accepted preview cannot be
    /// projected with the supplied viewport or chord tolerance.
    pub fn scene(
        &self,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<EditorScene, crate::EditorError> {
        let accepted = self.accepted_state();
        EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            self.scratch.design_document(),
            viewport,
            chord_tolerance_pixels,
        )
    }
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

/// Whether base-sketch-only profile/fill consumers may publish honestly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputedProfileBoundary {
    BaseOnly,
    Withheld { active_features: usize },
}

/// Closed replay vocabulary used by deterministic generated/model qualification.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplayAction {
    CreateComputedFillet {
        expected: ComputedFeatureDocumentIdentity,
        label: String,
        radius: f64,
        corners: Vec<geosolve_sketch_features::NewComputedFilletCorner>,
    },
    SetComputedFilletRadius {
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
        radius: f64,
    },
    RemoveComputedFeature {
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
    },
    RemoveComputedCorner {
        expected: ComputedFeatureDocumentIdentity,
        owner: ComputedCornerRef,
    },
    SetComputedFeatureSuppressed {
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
        suppressed: bool,
    },
    Edit {
        expected: SketchDesignIdentity,
        edit: DocumentEdit,
    },
    Construction {
        expected: SketchDesignIdentity,
        proposal: ConstructionProposal,
    },
    Operation {
        expected: SketchDesignIdentity,
        request: SketchOperationRequest,
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
    #[error(transparent)]
    SketchOperation(#[from] SketchOperationError),
    #[error(transparent)]
    SketchOperationApply(#[from] SketchOperationApplyError),
    #[error(transparent)]
    ComputedFeatureDocument(#[from] ComputedFeatureDocumentError),
    #[error(transparent)]
    ComputedFeatureEvaluation(#[from] ComputedFeatureEvaluationError),
    #[error(transparent)]
    ComputedFeatureSnapshot(#[from] ComputedFeatureSnapshotError),
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
    #[error("point-move commit does not match the retained solved preview")]
    SolvedPreviewMismatch,
    #[error("history has no earlier checkpoint")]
    NothingToUndo,
    #[error("history has no later checkpoint")]
    NothingToRedo,
    #[error("the helper-operation preview has not been independently accepted")]
    MissingOperationPreview,
    #[error("the helper-operation preview still requires a semantic confirmation")]
    OperationPreviewNotConfirmed,
    #[error("the helper-operation preview token or candidate does not match")]
    OperationPreviewMismatch,
    #[error("helper-operation work was cancelled or exhausted")]
    OperationWorkStopped,
    #[error("operation authoring pick is unavailable: {0:?}")]
    OperationAuthoringPick(OperationAuthoringWarningKind),
    #[error("computed-feature authoring pick is unavailable: {0:?}")]
    FeatureAuthoringPick(crate::FeatureAuthoringWarningKind),
    #[error("computed-feature candidate does not match the current exact input")]
    StaleComputedFeatureCandidate,
    #[error("computed-feature evaluation stopped before publication")]
    ComputedFeatureWorkStopped,
    #[error("computed-feature authoring preview was rejected: {0}")]
    FeatureAuthoringPreviewRejected(ComputedFeatureFailure),
    #[error("computed-feature authoring preview is missing or does not match")]
    FeatureAuthoringPreviewMismatch,
    #[error("computed-feature authoring preview identity space is exhausted")]
    FeatureAuthoringPreviewTokenExhausted,
}

/// Owner of retained lifecycle, interaction selection, restore history, and transcript.
#[derive(Debug)]
pub struct RetainedEditorCoordinator {
    session: RetainedSketchDocumentSession,
    features: ComputedFeatureDocument,
    computed_snapshot: Option<ComputedFeatureSnapshot>,
    computed_input: Option<geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    computed_preview_snapshot: Option<ComputedFeatureSnapshot>,
    computed_preview_input: Option<geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    computed_evaluation_allocator: ComputedEvaluationAllocator,
    computed_evaluation_problem: Option<String>,
    computed_preview_evaluation_problem: Option<String>,
    editor: ConstraintEditor,
    history: Vec<RestoreCheckpoint>,
    history_cursor: usize,
    transcript: Vec<ReplayAction>,
    transient: Option<TransientLifecycle>,
    solved_preview: Option<RetainedSketchDocumentSession>,
    drag_continuation: Option<ProjectedDragContinuation>,
    projected_drag_work: Option<ProjectedDragWorkEvidence>,
    operation_preview: Option<OperationAuthoringPreview>,
    next_operation_preview_token: u64,
    feature_authoring_preview: Option<FeatureAuthoringPreview>,
    next_feature_authoring_preview_token: u64,
}

#[derive(Clone, Debug)]
struct ProjectedDragContinuation {
    gesture_epoch: Option<u64>,
    pointer_id: u64,
    point: DesignPointId,
    design: SketchDesignIdentity,
    accepted: Option<SketchAcceptedStateIdentity>,
    last_request_id: Option<u64>,
    locality: Option<DocumentDragLocalityPlan>,
    planning_operation: Option<OperationReport>,
    planning_failure: Option<ProjectedDragPlanningFailure>,
    last_accepted_preview: Option<RetainedSketchDocumentSession>,
}

#[derive(Clone, Copy, Debug)]
struct ProjectedDragPlanningFailure {
    rejection_stage: ProjectedDragRejectionStage,
    operation_report_complete: bool,
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
        let features = ComputedFeatureDocument::new(session.design_document().id());
        Self::with_features(session, features)
    }

    /// Starts one composite retained lifecycle from an existing feature sidecar.
    ///
    /// # Errors
    ///
    /// Rejects a sidecar for another sketch namespace or invalid initial
    /// evaluation/checkpoint material.
    pub fn with_features(
        session: RetainedSketchDocumentSession,
        features: ComputedFeatureDocument,
    ) -> Result<Self, CoordinatorError> {
        Self::with_feature_state(session, features, ComputedEvaluationAllocator::default())
    }

    /// Restores a persisted composite workspace above all previously observed
    /// feature, corner and generated-edge identities.
    ///
    /// # Errors
    ///
    /// Rejects incompatible feature identity state or an invalid initial checkpoint.
    pub fn with_features_and_high_water(
        session: RetainedSketchDocumentSession,
        mut features: ComputedFeatureDocument,
        feature_lifecycle: ComputedFeatureLifecycleHighWater,
        evaluation_high_water: ComputedEvaluationAllocatorHighWater,
    ) -> Result<Self, CoordinatorError> {
        features.rebase_after_restore(feature_lifecycle)?;
        Self::with_feature_state(
            session,
            features,
            ComputedEvaluationAllocator::from_high_water(evaluation_high_water),
        )
    }

    fn with_feature_state(
        session: RetainedSketchDocumentSession,
        features: ComputedFeatureDocument,
        computed_evaluation_allocator: ComputedEvaluationAllocator,
    ) -> Result<Self, CoordinatorError> {
        if features.sketch_document() != session.design_document().id() {
            return Err(ComputedFeatureSnapshotError::FeatureDocumentForDifferentSketch.into());
        }
        let mut coordinator = Self {
            session,
            features,
            computed_snapshot: None,
            computed_input: None,
            computed_preview_snapshot: None,
            computed_preview_input: None,
            computed_evaluation_allocator,
            computed_evaluation_problem: None,
            computed_preview_evaluation_problem: None,
            editor: ConstraintEditor::default(),
            history: Vec::new(),
            history_cursor: 0,
            transcript: Vec::new(),
            transient: None,
            solved_preview: None,
            drag_continuation: None,
            projected_drag_work: None,
            operation_preview: None,
            next_operation_preview_token: 1,
            feature_authoring_preview: None,
            next_feature_authoring_preview_token: 1,
        };
        coordinator.refresh_computed_features();
        coordinator.history.push(checkpoint(
            &coordinator.session,
            &coordinator.features,
            &coordinator.computed_evaluation_allocator,
        )?);
        Ok(coordinator)
    }

    #[must_use]
    pub const fn session(&self) -> &RetainedSketchDocumentSession {
        &self.session
    }

    /// Persistent computed-feature intent owned beside the sketch session.
    #[must_use]
    pub const fn feature_document(&self) -> &ComputedFeatureDocument {
        &self.features
    }

    /// Current exact computed output. During a solved source-drag preview this
    /// returns the preview-local output rather than stale base geometry. If a
    /// visible source preview has no paired computed result, this returns `None`
    /// rather than falling back to base output.
    #[must_use]
    pub fn computed_snapshot(&self) -> Option<&ComputedFeatureSnapshot> {
        if let Some(preview) = self.feature_authoring_preview.as_ref() {
            return Some(&preview.snapshot);
        }
        if self.visible_preview_session().is_some() {
            return self.computed_preview_snapshot.as_ref();
        }
        self.computed_preview_snapshot
            .as_ref()
            .or(self.computed_snapshot.as_ref())
    }

    /// Returns the exact expected/snapshot pair for scene construction, or an
    /// explicit withholding state when computed geometry cannot honestly be
    /// paired with the currently visible native sketch.
    #[must_use]
    pub fn computed_scene_state(&self) -> ComputedSceneState<'_> {
        if let Some(preview) = self.feature_authoring_preview.as_ref() {
            return ComputedSceneState::Current {
                expected: &preview.metadata.input,
                snapshot: &preview.snapshot,
            };
        }
        if self.visible_preview_session().is_some() || self.computed_preview_snapshot.is_some() {
            return match (
                self.computed_preview_input.as_ref(),
                self.computed_preview_snapshot.as_ref(),
            ) {
                (Some(expected), Some(snapshot)) => {
                    ComputedSceneState::Current { expected, snapshot }
                }
                _ => ComputedSceneState::Withheld,
            };
        }
        if self.computed_evaluation_problem.is_some() {
            return ComputedSceneState::Withheld;
        }
        match (
            self.computed_input.as_ref(),
            self.computed_snapshot.as_ref(),
        ) {
            (Some(expected), Some(snapshot)) => ComputedSceneState::Current { expected, snapshot },
            (None, Some(_)) => ComputedSceneState::Withheld,
            (None | Some(_), None) => ComputedSceneState::Absent,
        }
    }

    /// Fail-closed production/profile presentation boundary for M66.
    #[must_use]
    pub fn computed_profile_boundary(&self) -> ComputedProfileBoundary {
        let active_features = self
            .features
            .features()
            .iter()
            .filter(|feature| !feature.suppressed)
            .count();
        if active_features == 0 {
            ComputedProfileBoundary::BaseOnly
        } else {
            ComputedProfileBoundary::Withheld { active_features }
        }
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

    /// Returns the checkpoint frozen when the current history entry was
    /// recorded. Its durable geometry matches this history position, but
    /// never-reuse lifecycle cursors may since have advanced through previews
    /// or Undo/Redo traversal. Use [`Self::persistence_checkpoint`] when saving
    /// the current workspace.
    #[must_use]
    pub fn checkpoint(&self) -> &RestoreCheckpoint {
        &self.history[self.history_cursor]
    }

    /// Captures current durable sketch and feature intent together with every
    /// live never-reuse high-water cursor.
    ///
    /// Transient previews are deliberately not persisted, but any evaluation
    /// identities they consumed remain represented so a later restore cannot
    /// reuse them. This capture does not add or rewrite a history entry.
    ///
    /// # Errors
    ///
    /// Returns a document or feature serialization error if current durable
    /// state cannot be encoded.
    pub fn persistence_checkpoint(&self) -> Result<RestoreCheckpoint, CoordinatorError> {
        checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        )
    }

    #[must_use]
    pub fn transcript(&self) -> &[ReplayAction] {
        &self.transcript
    }

    /// The independently validated solved-preview session currently published for rendering.
    #[must_use]
    pub fn solved_preview_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.drag_continuation
            .as_ref()
            .and_then(|gesture| gesture.last_accepted_preview.as_ref())
            .or(self.solved_preview.as_ref())
    }

    /// Work evidence for the latest projected pointer sample, if one is active.
    #[must_use]
    pub const fn projected_drag_work_evidence(&self) -> Option<&ProjectedDragWorkEvidence> {
        self.projected_drag_work.as_ref()
    }

    /// Current accepted document eligible for helper-operation picks. Retained
    /// design divergence has no current operation-authoring geometry and returns
    /// `None` rather than exposing the older accepted state under a newer design.
    #[must_use]
    pub fn operation_authoring_document(&self) -> Option<&SketchDocument> {
        self.session
            .accepted_state_for_current_input()
            .map(geosolve_sketch::SketchAcceptedDocumentState::document)
    }

    /// Exact retained-session input paired with [`Self::operation_authoring_document`].
    /// This lets the headless state invalidate otherwise geometrically identical
    /// operands after reattempt, Undo/Redo or external-input revision changes.
    #[must_use]
    pub fn operation_authoring_input(&self) -> Option<geosolve_sketch::PreparedSketchInput> {
        self.operation_authoring_document()
            .map(|_| self.session.prepared_input())
    }

    /// Captures the exact current accepted sketch boundary used for grouped
    /// computed-feature authoring.
    ///
    /// # Errors
    ///
    /// Rejects when the retained sketch has no current independently accepted state.
    pub fn feature_authoring_snapshot(
        &self,
    ) -> Result<ComputedFeatureAuthoringSnapshot, CoordinatorError> {
        ComputedFeatureAuthoringSnapshot::capture(&self.session).map_err(Into::into)
    }

    /// Converts current native point/span selection into an ordered stream of
    /// exact computed-Fillet picks. Several corner points remain several pairs.
    ///
    /// # Errors
    ///
    /// Rejects unavailable accepted geometry or a selection that cannot form a valid pick.
    pub fn feature_authoring_preselection(
        &self,
    ) -> Result<Vec<FeatureAuthoringPick>, CoordinatorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let document = self
            .operation_authoring_document()
            .ok_or(CoordinatorError::MissingOperationPreview)?;
        self.editor
            .selection()
            .iter()
            .copied()
            .try_fold(Vec::new(), |mut picks, item| {
                let parameter = match item {
                    SelectionItem::Curve(span) => self.editor.curve_pick_parameter(span),
                    SelectionItem::Point(_)
                    | SelectionItem::Constraint(_)
                    | SelectionItem::Dimension(_)
                    | SelectionItem::Feature(_)
                    | SelectionItem::FeatureCorner(_) => None,
                };
                picks.extend(
                    resolve_feature_item_picks(&snapshot, document, item, parameter)
                        .map_err(CoordinatorError::FeatureAuthoringPick)?,
                );
                Ok(picks)
            })
    }

    /// Resolves one native item using the same grouped-Fillet topology policy as
    /// preselection.
    ///
    /// # Errors
    ///
    /// Rejects unavailable accepted geometry or an item that cannot form a valid pick.
    pub fn feature_authoring_picks_for_item(
        &self,
        item: SelectionItem,
        parameter: Option<f64>,
    ) -> Result<Vec<FeatureAuthoringPick>, CoordinatorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let document = self
            .operation_authoring_document()
            .ok_or(CoordinatorError::MissingOperationPreview)?;
        resolve_feature_item_picks(&snapshot, document, item, parameter)
            .map_err(CoordinatorError::FeatureAuthoringPick)
    }

    #[must_use]
    pub const fn feature_authoring_preview(&self) -> Option<&FeatureAuthoringPreview> {
        self.feature_authoring_preview.as_ref()
    }

    /// Applies native semantic items to a trial authoring state and commits the
    /// transition only after any resulting whole-batch preview is accepted.
    ///
    /// # Errors
    ///
    /// Returns a snapshot, document, evaluation, or provisional-feature error
    /// without changing `state` or replacing a previously held exact preview.
    pub fn transact_feature_authoring_pick_items(
        &mut self,
        state: &mut FeatureAuthoringState,
        items: &[(SelectionItem, Option<f64>)],
        label: impl Into<String>,
    ) -> Result<FeatureAuthoringTransaction, CoordinatorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let document = self
            .operation_authoring_document()
            .cloned()
            .ok_or(CoordinatorError::MissingOperationPreview)?;
        let mut trial = state.clone();
        let outcome = trial.pick_items(&snapshot, &document, items);
        self.finish_feature_authoring_transaction(state, trial, outcome, label.into())
    }

    /// Resolves one native screen click against a trial authoring state and
    /// commits it only after any resulting whole-batch preview is accepted.
    ///
    /// # Errors
    ///
    /// Returns a snapshot, document, evaluation, or provisional-feature error
    /// without changing `state` or replacing a previously held exact preview.
    pub fn transact_feature_authoring_pick_at(
        &mut self,
        state: &mut FeatureAuthoringState,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        label: impl Into<String>,
    ) -> Result<FeatureAuthoringTransaction, CoordinatorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let document = self
            .operation_authoring_document()
            .cloned()
            .ok_or(CoordinatorError::MissingOperationPreview)?;
        let mut trial = state.clone();
        let outcome = trial.pick_at(&snapshot, &document, scene, position, tolerance);
        self.finish_feature_authoring_transaction(state, trial, outcome, label.into())
    }

    /// Applies shared-radius and branch-option changes to a trial authoring
    /// state and commits them only after the complete provisional feature is
    /// independently accepted.
    ///
    /// # Errors
    ///
    /// Returns a snapshot, evaluation, or provisional-feature error without
    /// changing `state` or replacing a previously held exact preview.
    pub fn transact_feature_authoring_options(
        &mut self,
        state: &mut FeatureAuthoringState,
        options: FeatureAuthoringOptions,
        selected_corner: Option<usize>,
        label: impl Into<String>,
    ) -> Result<FeatureAuthoringTransaction, CoordinatorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let mut trial = state.clone();
        let outcome = trial.set_options_with_corner(&snapshot, options, selected_corner);
        self.finish_feature_authoring_transaction(state, trial, outcome, label.into())
    }

    fn finish_feature_authoring_transaction(
        &mut self,
        state: &mut FeatureAuthoringState,
        trial: FeatureAuthoringState,
        outcome: FeatureAuthoringOutcome,
        label: String,
    ) -> Result<FeatureAuthoringTransaction, CoordinatorError> {
        if matches!(
            outcome,
            FeatureAuthoringOutcome::NoNativeHit(_)
                | FeatureAuthoringOutcome::Warning(_)
                | FeatureAuthoringOutcome::Inactive
        ) {
            return Ok(FeatureAuthoringTransaction {
                outcome,
                preview: None,
            });
        }
        let preview = match &outcome {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => {
                Some(self.prepare_feature_authoring_preview(
                    self.feature_document().identity(),
                    candidate,
                    label,
                )?)
            }
            FeatureAuthoringOutcome::ModeEntered(_)
            | FeatureAuthoringOutcome::Collecting { .. }
            | FeatureAuthoringOutcome::CandidateCleared(_)
            | FeatureAuthoringOutcome::ModeExited => {
                self.clear_feature_authoring_preview();
                None
            }
            FeatureAuthoringOutcome::Apply(_)
            | FeatureAuthoringOutcome::NoNativeHit(_)
            | FeatureAuthoringOutcome::Warning(_)
            | FeatureAuthoringOutcome::Inactive => None,
        };
        *state = trial;
        Ok(FeatureAuthoringTransaction { outcome, preview })
    }

    /// Evaluates the complete multi-corner candidate, including endpoint-claim
    /// composition with every existing set, without publishing intent.
    ///
    /// # Errors
    ///
    /// Rejects stale input, invalid feature intent, or bounded evaluation failure.
    pub fn prepare_feature_authoring_preview(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        candidate: &FeatureAuthoringCandidate,
        label: impl Into<String>,
    ) -> Result<FeatureAuthoringPreviewMetadata, CoordinatorError> {
        let sketch_input = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        if self.features.identity() != expected
            || candidate.tool() != FeatureAuthoringTool::Fillet
            || candidate.sketch_input() != sketch_input
            || self
                .session
                .accepted_state_for_current_input()
                .is_none_or(|accepted| accepted.identity() != candidate.accepted_state_identity())
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let label = label.into();
        let mut features = self.features.clone();
        let feature = features.create_fillet_set(
            label.clone(),
            candidate.radius(),
            candidate.persistent_corners(),
        )?;
        let outcome = evaluate_computed_features(
            &self.session,
            &features,
            &mut self.computed_evaluation_allocator,
            bounded_geometry_control(),
        )?;
        let OperationOutcome::Completed {
            value: snapshot, ..
        } = outcome
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if self.features.identity() != expected
            || self.session.accepted_prepared_input() != Some(candidate.sketch_input())
            || snapshot.input().features != features.identity()
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        require_current_feature_authoring_evaluation(&snapshot, feature)?;
        let token_value = self.next_feature_authoring_preview_token;
        self.next_feature_authoring_preview_token = token_value
            .checked_add(1)
            .ok_or(CoordinatorError::FeatureAuthoringPreviewTokenExhausted)?;
        let metadata = FeatureAuthoringPreviewMetadata {
            token: FeatureAuthoringPreviewToken(token_value),
            feature,
            feature_identity: features.identity(),
            input: snapshot.input(),
        };
        let radius_origin = Box::new(FeatureAuthoringRadiusOrigin {
            candidate: candidate.clone(),
            features: features.clone(),
            snapshot: snapshot.clone(),
            metadata: metadata.clone(),
        });
        self.clear_feature_authoring_preview();
        self.feature_authoring_preview = Some(FeatureAuthoringPreview {
            candidate: candidate.clone(),
            expected,
            features,
            snapshot,
            metadata: metadata.clone(),
            label,
            radius_origin,
        });
        Ok(metadata)
    }

    /// Rebuilds the complete held authoring preview from a freshly re-resolved
    /// candidate while preserving the pointer gesture's original exact input.
    ///
    /// This is the radius-drag path for grouped Fillets: changing the shared
    /// radius may also change retained contact seeds or branch-local intent, so a
    /// radius-only sidecar mutation is insufficient. Nothing is published, and
    /// the accepted sketch plus persistent feature document remain untouched.
    ///
    /// # Errors
    ///
    /// Rejects stale gesture/candidate input or bounded evaluation failure.
    pub fn refresh_feature_authoring_preview(
        &mut self,
        gesture_origin: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        candidate: &FeatureAuthoringCandidate,
    ) -> Result<FeatureAuthoringPreviewMetadata, CoordinatorError> {
        let sketch_input = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        let current = self
            .feature_authoring_preview
            .as_ref()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        if !current.accepts_radius_input(&gesture_origin) {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        if candidate.tool() != FeatureAuthoringTool::Fillet
            || candidate.sketch_input() != sketch_input
            || self
                .session
                .accepted_state_for_current_input()
                .is_none_or(|accepted| accepted.identity() != candidate.accepted_state_identity())
            || self.features.identity() != current.expected
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let expected = current.expected;
        let radius_origin = current.radius_origin.clone();
        let previous_feature = current.metadata.feature;
        let label = current.label.clone();
        let mut features = self.features.clone();
        let feature = features.create_fillet_set(
            label.clone(),
            candidate.radius(),
            candidate.persistent_corners(),
        )?;
        if feature != previous_feature {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        let outcome = evaluate_computed_features(
            &self.session,
            &features,
            &mut self.computed_evaluation_allocator,
            bounded_geometry_control(),
        )?;
        let OperationOutcome::Completed {
            value: snapshot, ..
        } = outcome
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if self.features.identity() != expected
            || self.session.accepted_prepared_input() != Some(candidate.sketch_input())
            || snapshot.input().sketch != candidate.sketch_input()
            || snapshot.input().features != features.identity()
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        require_current_feature_authoring_evaluation(&snapshot, feature)?;
        let token_value = self.next_feature_authoring_preview_token;
        self.next_feature_authoring_preview_token = token_value
            .checked_add(1)
            .ok_or(CoordinatorError::FeatureAuthoringPreviewTokenExhausted)?;
        let metadata = FeatureAuthoringPreviewMetadata {
            token: FeatureAuthoringPreviewToken(token_value),
            feature,
            feature_identity: features.identity(),
            input: snapshot.input(),
        };
        self.feature_authoring_preview = Some(FeatureAuthoringPreview {
            candidate: candidate.clone(),
            expected,
            features,
            snapshot,
            metadata: metadata.clone(),
            label,
            radius_origin,
        });
        Ok(metadata)
    }

    /// Marks the current freshly re-resolved whole-batch preview as the origin
    /// for a later radius gesture.
    ///
    /// # Errors
    ///
    /// Rejects an input or feature that does not identify the held preview.
    pub fn accept_feature_authoring_radius_preview(
        &mut self,
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
    ) -> Result<(), CoordinatorError> {
        let preview = self
            .feature_authoring_preview
            .as_mut()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        if preview.metadata.feature != feature || !preview.accepts_radius_input(&expected) {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        *preview.radius_origin = FeatureAuthoringRadiusOrigin {
            candidate: preview.candidate.clone(),
            features: preview.features.clone(),
            snapshot: preview.snapshot.clone(),
            metadata: preview.metadata.clone(),
        };
        Ok(())
    }

    /// Restores the exact whole-batch preview captured at pointer down.
    ///
    /// # Errors
    ///
    /// Rejects an input or feature that does not identify the held preview.
    pub fn restore_feature_authoring_radius_preview(
        &mut self,
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
    ) -> Result<(), CoordinatorError> {
        let preview = self
            .feature_authoring_preview
            .as_mut()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        if preview.metadata.feature != feature || !preview.accepts_radius_input(&expected) {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        let origin = preview.radius_origin.as_ref();
        preview.candidate = origin.candidate.clone();
        preview.features = origin.features.clone();
        preview.snapshot = origin.snapshot.clone();
        preview.metadata = origin.metadata.clone();
        Ok(())
    }

    /// Publishes only the exact coordinator-held whole-batch preview.
    ///
    /// # Errors
    ///
    /// Rejects a stale token, candidate, sketch input, or feature identity.
    pub fn apply_feature_authoring_preview(
        &mut self,
        token: FeatureAuthoringPreviewToken,
        candidate: &FeatureAuthoringCandidate,
    ) -> Result<ComputedFeatureMutation<ComputedFeatureId>, CoordinatorError> {
        let preview = self
            .feature_authoring_preview
            .as_ref()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        if preview.metadata.token != token
            || preview.candidate.radius().to_bits() != candidate.radius().to_bits()
            || preview.candidate.sketch_input() != candidate.sketch_input()
            || preview.candidate.accepted_state_identity() != candidate.accepted_state_identity()
            || preview.candidate.persistent_corners() != candidate.persistent_corners()
            || self.features.identity() != preview.expected
            || self.session.accepted_prepared_input() != Some(candidate.sketch_input())
            || preview.snapshot.input().features != preview.features.identity()
            || preview.snapshot.input().sketch != candidate.sketch_input()
        {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        require_current_feature_authoring_evaluation(&preview.snapshot, preview.metadata.feature)?;
        let preview = self
            .feature_authoring_preview
            .take()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        let before = self.features.identity();
        let after = preview.features.identity();
        let feature = preview.metadata.feature;
        self.features = preview.features;
        self.computed_input = Some(preview.snapshot.input());
        self.computed_snapshot = Some(preview.snapshot);
        self.computed_evaluation_problem = None;
        self.record_feature_mutation(ReplayAction::CreateComputedFillet {
            expected: preview.expected,
            label: preview.label,
            radius: preview.candidate.radius(),
            corners: preview.candidate.persistent_corners(),
        })?;
        Ok(ComputedFeatureMutation {
            value: feature,
            before,
            after,
        })
    }

    pub fn clear_feature_authoring_preview(&mut self) {
        let Some(preview) = self.feature_authoring_preview.take() else {
            return;
        };
        let temporary_feature = preview.metadata.feature;
        self.editor
            .revoke_computed_feature_interaction(temporary_feature);
    }

    /// Commits one exact grouped authoring candidate as one persistent Fillet set.
    /// Ordinary sketch identity, equations, rank and DOF are untouched.
    ///
    /// # Errors
    ///
    /// Rejects stale or invalid intent and any computed evaluation failure.
    pub fn apply_feature_authoring(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        candidate: &FeatureAuthoringCandidate,
        label: impl Into<String>,
    ) -> Result<ComputedFeatureMutation<ComputedFeatureId>, CoordinatorError> {
        let sketch_input = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        if candidate.tool() != FeatureAuthoringTool::Fillet
            || candidate.sketch_input() != sketch_input
            || self
                .session
                .accepted_state_for_current_input()
                .is_none_or(|accepted| accepted.identity() != candidate.accepted_state_identity())
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let label = label.into();
        let corners = candidate.persistent_corners();
        let radius = candidate.radius();
        let mutation_label = label.clone();
        let mutation_corners = corners.clone();
        self.mutate_features(
            expected,
            move |features| features.create_fillet_set(mutation_label, radius, mutation_corners),
            ReplayAction::CreateComputedFillet {
                expected,
                label,
                radius,
                corners,
            },
        )
    }

    /// Changes only one Fillet set's shared computed radius.
    ///
    /// # Errors
    ///
    /// Rejects stale identity, invalid radius/feature, or computed evaluation failure.
    pub fn set_computed_fillet_radius(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        self.mutate_features(
            expected,
            |features| features.set_fillet_radius(feature, radius),
            ReplayAction::SetComputedFilletRadius {
                expected,
                feature,
                radius,
            },
        )
    }

    /// Full composite-CAS radius edit used by canvas gestures.
    ///
    /// # Errors
    ///
    /// Rejects stale composite input or any ordinary radius-edit failure.
    pub fn set_computed_fillet_radius_exact(
        &mut self,
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        if self.computed_evaluation_input()? != expected {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        self.set_computed_fillet_radius(expected.features, feature, radius)
    }

    /// Evaluates a non-persistent shared-radius preview against the exact current
    /// accepted sketch and feature identity.
    ///
    /// # Errors
    ///
    /// Rejects stale input, invalid radius/feature, or bounded evaluation failure.
    pub fn preview_computed_fillet_radius(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<&ComputedFeatureSnapshot, CoordinatorError> {
        if self.features.identity() != expected {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let sketch_input = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        let mut candidate = self.features.clone();
        candidate.set_fillet_radius(feature, radius)?;
        let outcome = evaluate_computed_features(
            &self.session,
            &candidate,
            &mut self.computed_evaluation_allocator,
            bounded_geometry_control(),
        )?;
        let OperationOutcome::Completed {
            value: snapshot, ..
        } = outcome
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if self.features.identity() != expected
            || self.session.accepted_prepared_input() != Some(sketch_input)
            || snapshot.input().sketch != sketch_input
            || snapshot.input().features != candidate.identity()
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        self.computed_preview_input = Some(snapshot.input());
        self.computed_preview_snapshot = Some(snapshot);
        self.computed_preview_snapshot
            .as_ref()
            .ok_or(CoordinatorError::ComputedFeatureWorkStopped)
    }

    /// Full composite-CAS counterpart used by a retained radius gesture.
    ///
    /// # Errors
    ///
    /// Rejects stale composite input or any ordinary preview failure.
    pub fn preview_computed_fillet_radius_exact(
        &mut self,
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<&ComputedFeatureSnapshot, CoordinatorError> {
        if self.computed_evaluation_input()? != expected {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        self.preview_computed_fillet_radius(expected.features, feature, radius)
    }

    pub fn clear_computed_feature_preview(&mut self) {
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_preview_evaluation_problem = None;
    }

    /// Captures the complete exact input expected for computed output from the
    /// coordinator's current retained sketch/feature state.
    ///
    /// # Errors
    ///
    /// Rejects when no complete current evaluation snapshot can be captured.
    pub fn computed_evaluation_input(
        &self,
    ) -> Result<geosolve_sketch_features::ComputedFeatureEvaluationInput, CoordinatorError> {
        Ok(ComputedFeatureEvaluationSnapshot::capture(
            &self.session,
            &self.features,
            ComputedFeatureEvaluationPolicy::default(),
        )?
        .input())
    }

    /// Removes one complete persistent computed feature.
    ///
    /// # Errors
    ///
    /// Rejects stale identity, an unknown feature, or computed evaluation failure.
    pub fn remove_computed_feature(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        self.mutate_features(
            expected,
            |features| features.remove_feature(feature),
            ReplayAction::RemoveComputedFeature { expected, feature },
        )
    }

    /// Removes one Fillet corner; the feature document removes the set when this
    /// is its final corner.
    ///
    /// # Errors
    ///
    /// Rejects stale identity, an unknown corner, or computed evaluation failure.
    pub fn remove_computed_corner(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        owner: ComputedCornerRef,
    ) -> Result<ComputedFeatureMutation<bool>, CoordinatorError> {
        self.mutate_features(
            expected,
            |features| features.remove_corner(owner.feature, owner.corner),
            ReplayAction::RemoveComputedCorner { expected, owner },
        )
    }

    /// Applies set-wide suppression without changing native sketch sources.
    ///
    /// # Errors
    ///
    /// Rejects stale identity, an unknown feature, or computed evaluation failure.
    pub fn set_computed_feature_suppressed(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
        suppressed: bool,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        self.mutate_features(
            expected,
            |features| features.set_suppressed(feature, suppressed),
            ReplayAction::SetComputedFeatureSuppressed {
                expected,
                feature,
                suppressed,
            },
        )
    }

    /// Resolves an evaluation-local generated edge to stable selection provenance.
    #[must_use]
    pub fn selection_for_computed_edge(&self, edge: ComputedEdgeId) -> Option<SelectionItem> {
        match &self.computed_snapshot()?.edge(edge)?.provenance {
            ComputedEdgeProvenance::FilletArc { owner, .. } => {
                Some(SelectionItem::FeatureCorner(*owner))
            }
            ComputedEdgeProvenance::SourceFragment { source, .. } => {
                Some(SelectionItem::Curve(source.span))
            }
            _ => None,
        }
    }

    /// Feature/corner/source-attributed current computed failures. Setup or
    /// bounded-work failures remain global and never masquerade as local geometry.
    #[must_use]
    pub fn computed_feature_problems(&self) -> Vec<ComputedFeatureProblemMetadata> {
        if let Some(message) = self
            .computed_preview_evaluation_problem
            .as_ref()
            .or(self.computed_evaluation_problem.as_ref())
        {
            return vec![ComputedFeatureProblemMetadata {
                feature: None,
                corners: Vec::new(),
                sources: Vec::new(),
                scope: EditorProblemScope::Global,
                message: message.clone(),
            }];
        }
        let features = self
            .feature_authoring_preview
            .as_ref()
            .map_or(&self.features, |preview| &preview.features);
        self.computed_snapshot()
            .into_iter()
            .flat_map(ComputedFeatureSnapshot::feature_evaluations)
            .filter_map(|evaluation| match &evaluation.state {
                ComputedFeatureEvaluationState::Failed { failure } => Some(
                    computed_feature_problem(features, evaluation.feature, failure),
                ),
                ComputedFeatureEvaluationState::Current { .. }
                | ComputedFeatureEvaluationState::Suppressed => None,
            })
            .collect()
    }

    /// Resolves one ordinary tree/canvas item into an exact accepted model-space
    /// operation pick. A missing canvas parameter uses the deterministic midpoint
    /// of the first visible interval; no presentation formula is involved.
    ///
    /// # Errors
    ///
    /// Returns a missing-preview or typed authoring-pick error when no exact
    /// current accepted input exists or the requested item cannot be sampled.
    pub fn operation_pick_for_item(
        &self,
        item: SelectionItem,
        curve_parameter: Option<f64>,
    ) -> Result<OperationAuthoringPick, CoordinatorError> {
        let document = self
            .operation_authoring_document()
            .ok_or(CoordinatorError::MissingOperationPreview)?;
        OperationAuthoringPick::for_item(document, item, curve_parameter)
            .map(|pick| pick.bind_input(&self.session.prepared_input()))
            .map_err(CoordinatorError::OperationAuthoringPick)
    }

    /// Resolves one item with the active helper tool's topology-aware operand
    /// policy. Most items produce one stamped curve pick; an unambiguous Fillet
    /// corner point produces its ordered incoming/outgoing span pair atomically.
    ///
    /// # Errors
    ///
    /// Returns a missing-preview or typed authoring-pick error when no exact
    /// current accepted input exists or the item is incompatible/ambiguous.
    pub fn operation_picks_for_item(
        &self,
        tool: OperationAuthoringTool,
        item: SelectionItem,
        curve_parameter: Option<f64>,
    ) -> Result<Vec<OperationAuthoringPick>, CoordinatorError> {
        let document = self
            .operation_authoring_document()
            .ok_or(CoordinatorError::MissingOperationPreview)?;
        resolve_operation_item_picks(document, tool, item, curve_parameter)
            .map(|picks| {
                let input = self.session.prepared_input();
                picks
                    .into_iter()
                    .map(|pick| pick.bind_input(&input))
                    .collect()
            })
            .map_err(CoordinatorError::OperationAuthoringPick)
    }

    /// Converts current ordinary selection into exact accepted picks using the
    /// active helper tool's topology-aware operand policy. Curve-hit parameters
    /// retained by the editor win; tree selections use the deterministic visible-
    /// midpoint fallback. Fillet may expand one unambiguous corner point into its
    /// ordered adjacent span pair.
    ///
    /// # Errors
    ///
    /// Returns the first missing-preview or typed authoring-pick error while
    /// expanding the immutable selection snapshot.
    pub fn operation_authoring_preselection(
        &self,
        tool: OperationAuthoringTool,
    ) -> Result<Vec<OperationAuthoringPick>, CoordinatorError> {
        self.editor
            .selection()
            .iter()
            .copied()
            .try_fold(Vec::new(), |mut picks, item| {
                let parameter = match item {
                    SelectionItem::Curve(span) => self.editor.curve_pick_parameter(span),
                    SelectionItem::Point(_)
                    | SelectionItem::Constraint(_)
                    | SelectionItem::Dimension(_)
                    | SelectionItem::Feature(_)
                    | SelectionItem::FeatureCorner(_) => None,
                };
                picks.extend(self.operation_picks_for_item(tool, item, parameter)?);
                Ok(picks)
            })
    }

    /// The latest independently accepted coordinator-owned helper-operation preview.
    #[must_use]
    pub const fn operation_preview(&self) -> Option<&OperationAuthoringPreview> {
        self.operation_preview.as_ref()
    }

    /// Drops the current helper-operation preview without changing document state.
    pub fn clear_operation_preview(&mut self) {
        self.operation_preview = None;
    }

    /// Synchronizes coordinator-held preview lifetime with one headless state
    /// transition. Only a preview request or exact Apply transition may retain a
    /// held preview; warnings, collection, cancellation and mode exit revoke it.
    pub fn observe_operation_authoring_outcome(&mut self, outcome: &OperationAuthoringOutcome) {
        if !matches!(
            outcome,
            OperationAuthoringOutcome::PreviewRequested { .. }
                | OperationAuthoringOutcome::Apply(_)
        ) {
            self.clear_operation_preview();
        }
    }

    /// Executes one synthesized request against an immutable operation snapshot,
    /// applies its proposal only to a scratch retained session, and stores a
    /// preview only after ordinary independent accepted-state publication.
    ///
    /// # Errors
    ///
    /// Returns operation, retained-session, metadata, or finite-work errors. Typed
    /// applicability and solve rejections are returned as warning outcomes.
    pub fn prepare_operation_preview(
        &mut self,
        candidate: &OperationAuthoringCandidate,
    ) -> Result<OperationAuthoringPreviewOutcome, CoordinatorError> {
        self.prepare_operation_preview_controlled(candidate, bounded_geometry_control())
    }

    /// Controlled counterpart to [`Self::prepare_operation_preview`]. Preparation
    /// and scratch publication share one deterministic finite work envelope.
    ///
    /// # Errors
    ///
    /// Returns operation, retained-session, metadata, or control errors. Typed
    /// applicability, cancellation, exhaustion, and solve rejection are warning
    /// outcomes and never mutate the live coordinator.
    #[allow(
        clippy::too_many_lines,
        reason = "the exact snapshot, scratch publication, provenance and preview-token lifecycle is one atomic audit path"
    )]
    pub fn prepare_operation_preview_controlled(
        &mut self,
        candidate: &OperationAuthoringCandidate,
        control: OperationControl,
    ) -> Result<OperationAuthoringPreviewOutcome, CoordinatorError> {
        self.clear_transient();
        let current_input = self.session.prepared_input();
        let Some(current_document) = self.operation_authoring_document() else {
            return Ok(OperationAuthoringPreviewOutcome::Warning(
                operation_authoring_warning(
                    candidate,
                    OperationAuthoringWarningKind::StalePick,
                    "operation candidate does not match the current exact accepted input",
                ),
            ));
        };
        if candidate.source_input() != Some(&current_input)
            || candidate
                .picks()
                .iter()
                .any(|pick| pick.validate(current_document).is_err())
        {
            return Ok(OperationAuthoringPreviewOutcome::Warning(
                operation_authoring_warning(
                    candidate,
                    OperationAuthoringWarningKind::StalePick,
                    "operation candidate does not match the current exact accepted input",
                ),
            ));
        }
        let job =
            SketchOperationSnapshot::capture(&self.session).prepare(candidate.request().clone());
        let configured_limits = control.limits;
        let cancellation = control.token.clone();
        let (result, preparation_report) = match job.execute(control)? {
            OperationOutcome::Completed { value, report } => (value, report),
            stopped => {
                return Ok(OperationAuthoringPreviewOutcome::Warning(
                    operation_authoring_warning(
                        candidate,
                        OperationAuthoringWarningKind::WorkStopped,
                        format!(
                            "operation preparation stopped: {:?}",
                            stopped.report().stopping_reason
                        ),
                    ),
                ));
            }
        };
        let proposal = match result {
            SketchOperationResult::Proposed(proposal) => *proposal,
            SketchOperationResult::Unsupported(unsupported) => {
                return Ok(OperationAuthoringPreviewOutcome::Warning(
                    operation_authoring_warning(
                        candidate,
                        OperationAuthoringWarningKind::OperationUnsupported(unsupported.reason),
                        "the selected curve family is not supported exactly by this operation",
                    ),
                ));
            }
            SketchOperationResult::Incomplete(incomplete) => {
                return Ok(OperationAuthoringPreviewOutcome::Warning(
                    operation_authoring_warning(
                        candidate,
                        OperationAuthoringWarningKind::OperationIncomplete(incomplete.reason),
                        "the current accepted input is incomplete for this operation",
                    ),
                ));
            }
            _ => {
                return Ok(OperationAuthoringPreviewOutcome::Warning(
                    operation_authoring_warning(
                        candidate,
                        OperationAuthoringWarningKind::PreviewRejected,
                        "the operation companion returned an unsupported future outcome",
                    ),
                ));
            }
        };
        let mut scratch = self.session.clone();
        let remaining = remaining_operation_limits(configured_limits, preparation_report.consumed);
        let outcome = match proposal
            .apply_controlled(&mut scratch, OperationControl::new(cancellation, remaining))?
        {
            OperationOutcome::Completed { value, .. } => value,
            stopped => {
                return Ok(OperationAuthoringPreviewOutcome::Warning(
                    operation_authoring_warning(
                        candidate,
                        OperationAuthoringWarningKind::WorkStopped,
                        format!(
                            "operation scratch publication stopped: {:?}",
                            stopped.report().stopping_reason
                        ),
                    ),
                ));
            }
        };
        let Some(published) = outcome.published_accepted_identity() else {
            return Ok(OperationAuthoringPreviewOutcome::Warning(
                operation_authoring_warning(
                    candidate,
                    OperationAuthoringWarningKind::PreviewRejected,
                    "ordinary solve validation rejected the operation preview",
                ),
            ));
        };
        let accepted = scratch.accepted_state().filter(|accepted| {
            accepted.identity() == published
                && accepted.design_identity() == outcome.design_identity()
                && accepted.design_identity() == scratch.design_identity()
        });
        if accepted.is_none() {
            return Ok(OperationAuthoringPreviewOutcome::Warning(
                operation_authoring_warning(
                    candidate,
                    OperationAuthoringWarningKind::PreviewRejected,
                    "operation preview acceptance provenance is inconsistent",
                ),
            ));
        }
        let token = self.allocate_operation_preview_token()?;
        let metadata = operation_preview_metadata(
            candidate,
            outcome.value(),
            &scratch,
            proposal.input().design_identity(),
            published,
            token,
        )?;
        self.operation_preview = Some(OperationAuthoringPreview {
            candidate: candidate.clone(),
            proposal,
            scratch,
            metadata: metadata.clone(),
        });
        Ok(OperationAuthoringPreviewOutcome::Ready(metadata))
    }

    /// Applies only the exact coordinator-held independently accepted preview.
    /// The live session is changed only after the proposal reproduces accepted
    /// publication on a clone, preserving atomicity even under an unexpected
    /// deterministic-replay failure.
    ///
    /// # Errors
    ///
    /// Returns token/candidate mismatch, stale-input, finite-work, publication,
    /// history, or retained-session errors without partially committing the live state.
    pub fn apply_operation_preview(
        &mut self,
        token: OperationAuthoringPreviewToken,
        candidate: &OperationAuthoringCandidate,
    ) -> Result<OperationAuthoringMutation, CoordinatorError> {
        self.apply_operation_preview_controlled(token, candidate, bounded_geometry_control())
    }

    /// Controlled exact commit for one token/candidate-bound accepted preview.
    ///
    /// # Errors
    ///
    /// Returns token/candidate mismatch, stale-input, finite-work, publication,
    /// history, or retained-session errors without partially committing the live state.
    pub fn apply_operation_preview_controlled(
        &mut self,
        token: OperationAuthoringPreviewToken,
        candidate: &OperationAuthoringCandidate,
        control: OperationControl,
    ) -> Result<OperationAuthoringMutation, CoordinatorError> {
        let Some(held) = self.operation_preview.as_ref() else {
            return Err(CoordinatorError::MissingOperationPreview);
        };
        if held.metadata.token != token {
            return Err(CoordinatorError::OperationPreviewMismatch);
        }
        if &held.candidate != candidate {
            self.operation_preview = None;
            return Err(CoordinatorError::OperationPreviewMismatch);
        }
        let Some(preview) = self.operation_preview.take() else {
            return Err(CoordinatorError::MissingOperationPreview);
        };
        if !preview.metadata.apply_ready {
            self.operation_preview = Some(preview);
            return Err(CoordinatorError::OperationPreviewNotConfirmed);
        }
        let expected = preview.proposal.input().design_identity();
        self.ensure_expected(expected)?;
        let mut next = self.session.clone();
        let OperationOutcome::Completed { value: outcome, .. } =
            preview.proposal.apply_controlled(&mut next, control)?
        else {
            return Err(CoordinatorError::OperationWorkStopped);
        };
        let Some(published) = outcome.published_accepted_identity() else {
            return Err(CoordinatorError::InvalidActionInput(
                "an accepted operation preview did not reproduce accepted publication",
            ));
        };
        if next.accepted_state().is_none_or(|accepted| {
            accepted.identity() != published
                || accepted.design_identity() != outcome.design_identity()
                || accepted.design_identity() != next.design_identity()
        }) {
            return Err(CoordinatorError::InvalidActionInput(
                "operation publication provenance does not match its accepted preview",
            ));
        }
        let Some(scratch_accepted) = preview.scratch.accepted_state() else {
            return Err(CoordinatorError::OperationPreviewMismatch);
        };
        let Some(next_accepted) = next.accepted_state() else {
            return Err(CoordinatorError::InvalidActionInput(
                "operation publication has no matching accepted state",
            ));
        };
        if next.prepared_input() != preview.scratch.prepared_input()
            || next.design_document() != preview.scratch.design_document()
            || next_accepted.identity() != scratch_accepted.identity()
            || next_accepted.design_identity() != scratch_accepted.design_identity()
            || next_accepted.document() != scratch_accepted.document()
        {
            return Err(CoordinatorError::OperationPreviewMismatch);
        }
        let value = MutationOutcome {
            value: outcome.value().clone(),
            design: outcome.design_identity(),
            attempt: outcome.attempt_identity(),
            published_accepted: Some(published),
        };
        let primary_created_curve = preview.metadata.primary_created_curve;
        let primary_span = next
            .design_document()
            .curve_spans(primary_created_curve)?
            .into_iter()
            .next()
            .ok_or(CoordinatorError::InvalidActionInput(
                "operation primary curve has no semantic span",
            ))?;
        self.session = next;
        self.editor
            .set_selection([SelectionItem::Curve(primary_span)]);
        self.record_mutation(ReplayAction::Operation {
            expected,
            request: preview.candidate.request().clone(),
        })?;
        Ok(OperationAuthoringMutation {
            operation: value,
            primary_created_curve,
        })
    }

    fn allocate_operation_preview_token(
        &mut self,
    ) -> Result<OperationAuthoringPreviewToken, CoordinatorError> {
        let token = OperationAuthoringPreviewToken(self.next_operation_preview_token);
        self.next_operation_preview_token =
            self.next_operation_preview_token.checked_add(1).ok_or(
                CoordinatorError::InvalidActionInput("operation preview generation exhausted"),
            )?;
        Ok(token)
    }

    /// Returns the independently accepted drag preview visible to a presentation adapter.
    #[must_use]
    pub fn visible_preview_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.solved_preview_session()
    }

    /// Resolves a pointer press and captures any point gesture's locality plan from
    /// the exact accepted state visible at press time.
    pub fn pointer_down(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items(scene, input, &[])
    }

    /// Resolves a pointer press with diagnostically forced annotations and captures
    /// any point gesture's locality plan from the exact accepted state visible at
    /// press time.
    pub fn pointer_down_with_problem_items(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items_controlled(
            scene,
            input,
            problem_items,
            projected_drag_control(),
        )
    }

    fn pointer_down_with_problem_items_controlled(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
        control: OperationControl,
    ) -> Vec<EditorEffect> {
        let before = self.editor.point_gesture_snapshot();
        let effects = self
            .editor
            .pointer_down_with_problem_items(scene, input, problem_items);
        let after = self.editor.point_gesture_snapshot();
        if after != before {
            self.clear_transient();
            if let Some(gesture) = after {
                self.drag_continuation = Some(self.plan_projected_drag_start(gesture, control));
            }
        }
        effects
    }

    fn plan_projected_drag_start(
        &self,
        gesture: PointGestureSnapshot,
        control: OperationControl,
    ) -> ProjectedDragContinuation {
        let design = self.session.design_identity();
        let accepted = self
            .session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
        let empty_operation = OperationController::new(control.clone()).report();
        let (locality, planning_operation, planning_failure) = match self
            .session
            .drag_locality_plan_controlled(gesture.point, control)
        {
            Ok(OperationOutcome::Completed { value, report }) if accepted.is_some() => {
                (Some(value), report, None)
            }
            Ok(OperationOutcome::Completed { report, .. }) => (
                None,
                report,
                Some(ProjectedDragPlanningFailure {
                    rejection_stage: ProjectedDragRejectionStage::AcceptedState,
                    operation_report_complete: true,
                }),
            ),
            Ok(stopped) => (
                None,
                *stopped.report(),
                Some(ProjectedDragPlanningFailure {
                    rejection_stage: ProjectedDragRejectionStage::LocalityPlanning,
                    operation_report_complete: true,
                }),
            ),
            Err(_) => (
                None,
                empty_operation,
                Some(ProjectedDragPlanningFailure {
                    rejection_stage: ProjectedDragRejectionStage::LocalityPlanning,
                    operation_report_complete: false,
                }),
            ),
        };
        ProjectedDragContinuation {
            gesture_epoch: Some(gesture.epoch),
            pointer_id: gesture.pointer_id,
            point: gesture.point,
            design,
            accepted,
            last_request_id: None,
            locality,
            planning_operation: Some(planning_operation),
            planning_failure,
            last_accepted_preview: None,
        }
    }

    /// Executes and publishes one editor-requested projected point-move preview.
    ///
    /// A failed or rejected projection is reported back to the editor without replacing the
    /// last valid solved preview. Request construction and acceptance validation remain here,
    /// outside presentation adapters.
    #[allow(
        clippy::too_many_lines,
        reason = "gesture validation, controlled planning, solving, and typed evidence form one atomic preview transition"
    )]
    pub fn resolve_projected_point_move(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        model_position: [f64; 2],
    ) -> Vec<EditorEffect> {
        let disposition = self
            .editor
            .projected_drag_request_disposition(pointer_id, request_id, point);
        let gesture_epoch = match disposition {
            ProjectedDragRequestDisposition::Current { gesture_epoch } => Some(gesture_epoch),
            ProjectedDragRequestDisposition::Stale => return Vec::new(),
            ProjectedDragRequestDisposition::Untracked => None,
        };
        let design = self.session.design_identity();
        let accepted = self
            .session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
        let same_gesture = self.drag_continuation.as_ref().is_some_and(|gesture| {
            gesture.gesture_epoch == gesture_epoch
                && gesture.pointer_id == pointer_id
                && gesture.point == point
                && gesture.design == design
                && gesture.accepted == accepted
        });
        // A state-machine-issued request belongs only to the press-time stamp.
        // Losing or changing that stamp makes the request stale; it must never
        // fall back to first-sample planning against a different accepted state.
        if gesture_epoch.is_some() && !same_gesture {
            return Vec::new();
        }
        if same_gesture
            && self
                .drag_continuation
                .as_ref()
                .and_then(|gesture| gesture.last_request_id)
                .is_some_and(|last_request_id| request_id <= last_request_id)
        {
            return Vec::new();
        }
        if !same_gesture {
            self.transient = None;
            self.solved_preview = None;
            self.drag_continuation = None;
            let mut gesture = self.plan_projected_drag_start(
                PointGestureSnapshot {
                    epoch: 0,
                    pointer_id,
                    point,
                },
                projected_drag_control(),
            );
            gesture.gesture_epoch = None;
            self.drag_continuation = Some(gesture);
        }

        let Some(mut gesture) = self.drag_continuation.take() else {
            return Vec::new();
        };
        gesture.last_request_id = Some(request_id);
        let planning_operation = gesture
            .planning_operation
            .take()
            .unwrap_or_else(|| OperationController::new(projected_drag_control()).report());
        let continued = gesture.last_accepted_preview.is_some();
        let passive_degrees_of_freedom = gesture
            .locality
            .as_ref()
            .map_or(0, DocumentDragLocalityPlan::passive_degrees_of_freedom);
        let anchor_count = gesture
            .locality
            .as_ref()
            .map_or(0, DocumentDragLocalityPlan::anchor_count);

        if let Some(failure) = gesture.planning_failure {
            self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                pointer_id,
                request_id,
                point,
                continued,
                attempts: 0,
                accepted: false,
                passive_degrees_of_freedom,
                anchor_count,
                rejection_stage: Some(failure.rejection_stage),
                operation_report_complete: failure.operation_report_complete,
                operation: planning_operation,
            });
            self.drag_continuation = Some(gesture);
            return self
                .editor
                .projected_drag_result(pointer_id, request_id, point, None);
        }

        if !model_position.iter().all(|value| value.is_finite()) {
            self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                pointer_id,
                request_id,
                point,
                continued,
                attempts: 0,
                accepted: false,
                passive_degrees_of_freedom,
                anchor_count,
                rejection_stage: Some(ProjectedDragRejectionStage::AttemptInput),
                operation_report_complete: true,
                operation: planning_operation,
            });
            self.drag_continuation = Some(gesture);
            return self
                .editor
                .projected_drag_result(pointer_id, request_id, point, None);
        }

        let Some(locality) = gesture.locality.clone() else {
            self.projected_drag_work = Some(ProjectedDragWorkEvidence {
                pointer_id,
                request_id,
                point,
                continued,
                attempts: 0,
                accepted: false,
                passive_degrees_of_freedom,
                anchor_count,
                rejection_stage: Some(ProjectedDragRejectionStage::LocalityPlanning),
                operation_report_complete: false,
                operation: planning_operation,
            });
            self.drag_continuation = Some(gesture);
            return self
                .editor
                .projected_drag_result(pointer_id, request_id, point, None);
        };

        let preview = gesture.last_accepted_preview.clone();
        let request = self
            .session
            .last_attempt()
            .input()
            .candidate_request()
            .with_previous_state_preferences()
            .with_drag(point, model_position);
        let mut attempt_control = projected_drag_control();
        attempt_control.limits =
            remaining_operation_limits(planning_operation.configured, planning_operation.consumed);
        let mut candidate = self.session.clone();
        let outcome = if let Some(preview) = preview.as_ref() {
            candidate.reattempt_from_accepted_preview_with_drag_locality_controlled(
                candidate.design_identity(),
                request,
                preview,
                &locality,
                attempt_control,
            )
        } else {
            candidate.reattempt_with_drag_locality_controlled(
                candidate.design_identity(),
                request,
                &locality,
                attempt_control,
            )
        };
        let mut rejection_stage = None;
        let mut operation = planning_operation;
        let mut operation_report_complete = true;
        let mut attempts = 1;
        let mut accepted_position = None;
        match outcome {
            Ok(OperationOutcome::Completed {
                value: attempt,
                report,
            }) => {
                accumulate_operation_report(&mut operation, &report);
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
                    } else if self.mark_solved_preview(&candidate).is_err() {
                        accepted_position = None;
                        rejection_stage = Some(ProjectedDragRejectionStage::PreviewPublication);
                    } else {
                        gesture.last_accepted_preview = self.solved_preview.take();
                    }
                }
            }
            Ok(stopped) => {
                accumulate_operation_report(&mut operation, stopped.report());
                rejection_stage = Some(ProjectedDragRejectionStage::ControlledOperation);
            }
            Err(_) => {
                attempts = 0;
                rejection_stage = Some(ProjectedDragRejectionStage::Session);
                operation_report_complete = false;
            }
        }
        self.drag_continuation = Some(gesture);
        self.projected_drag_work = Some(ProjectedDragWorkEvidence {
            pointer_id,
            request_id,
            point,
            continued,
            attempts,
            accepted: accepted_position.is_some(),
            passive_degrees_of_freedom,
            anchor_count,
            rejection_stage,
            operation_report_complete,
            operation,
        });
        self.editor
            .projected_drag_result(pointer_id, request_id, point, accepted_position)
    }

    /// Explicitly marks an outstanding solve. It does not mutate lifecycle history.
    pub fn mark_solving(&mut self) {
        self.transient = Some(TransientLifecycle::Solving);
        self.solved_preview = None;
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
        self.mark_solved_preview_controlled(preview, bounded_geometry_control())
    }

    fn mark_solved_preview_controlled(
        &mut self,
        preview: &RetainedSketchDocumentSession,
        control: OperationControl,
    ) -> Result<(), CoordinatorError> {
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
        match evaluate_computed_features(
            preview,
            &self.features,
            &mut self.computed_evaluation_allocator,
            control,
        ) {
            Ok(OperationOutcome::Completed { value, .. }) => {
                self.computed_preview_input = Some(value.input());
                self.computed_preview_snapshot = Some(value);
                self.computed_preview_evaluation_problem = None;
            }
            Ok(stopped) => {
                self.computed_preview_snapshot = None;
                self.computed_preview_input = None;
                self.computed_preview_evaluation_problem = Some(format!(
                    "computed-feature preview evaluation stopped: {:?}",
                    stopped.report().stopping_reason
                ));
            }
            Err(error) => {
                self.computed_preview_snapshot = None;
                self.computed_preview_input = None;
                self.computed_preview_evaluation_problem = Some(error.to_string());
            }
        }
        self.transient = Some(TransientLifecycle::SolvedPreview {
            attempt: preview_attempt.identity(),
            accepted: preview_accepted,
        });
        self.solved_preview = Some(preview.clone());
        Ok(())
    }

    pub fn clear_transient(&mut self) {
        self.transient = None;
        self.solved_preview = None;
        self.drag_continuation = None;
        self.projected_drag_work = None;
        self.operation_preview = None;
        self.clear_feature_authoring_preview();
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_preview_evaluation_problem = None;
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
            let accepted =
                checkpoint_document_from_json(json, saved_checkpoint.accepted_is_draft_v5)?;
            if saved_checkpoint.accepted_belongs_to_current_design {
                RetainedSketchDocumentSession::restore_current_design_with_accepted(
                    design,
                    accepted,
                    revisions,
                    request,
                    input.solver_config(),
                )?
            } else {
                RetainedSketchDocumentSession::restore_design_with_accepted(
                    design,
                    accepted,
                    revisions,
                    request,
                    input.solver_config(),
                )?
            }
        } else {
            RetainedSketchDocumentSession::restore_design(
                design,
                revisions,
                request,
                input.solver_config(),
            )?
        };
        let retained_features = merge_feature_lifecycle_high_water(
            self.features.lifecycle_high_water(),
            saved_checkpoint.feature_lifecycle,
        );
        let mut restored_features =
            ComputedFeatureDocument::from_json(&saved_checkpoint.feature_json)?;
        if restored_features.sketch_document() != restored.design_document().id() {
            return Err(ComputedFeatureSnapshotError::FeatureDocumentForDifferentSketch.into());
        }
        restored_features.rebase_after_restore(retained_features)?;
        self.computed_evaluation_allocator
            .retain_high_water(saved_checkpoint.evaluation_allocator);
        self.session = restored;
        self.features = restored_features;
        self.clear_transient();
        self.refresh_computed_features();
        self.history.clear();
        self.history.push(checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        )?);
        self.history_cursor = 0;
        self.transcript.clear();
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
                state: composite_delete_availability(document, &self.features, selection),
            },
            ActionAvailability {
                action: CoordinatorActionKind::Suppress,
                state: composite_suppression_availability(
                    document,
                    &self.features,
                    selection,
                    true,
                ),
            },
            ActionAvailability {
                action: CoordinatorActionKind::Unsuppress,
                state: composite_suppression_availability(
                    document,
                    &self.features,
                    selection,
                    false,
                ),
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
        self.refresh_computed_features();
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
        self.refresh_computed_features();
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
                    | SelectionItem::Dimension(_)
                    | SelectionItem::Feature(_)
                    | SelectionItem::FeatureCorner(_) => None,
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
        if let Some(targets) =
            selected_computed_targets(self.session.design_document(), &self.features, &selection)
                .map_err(CoordinatorError::ActionUnavailable)?
        {
            let feature_expected = self.features.identity();
            self.mutate_features(
                feature_expected,
                move |features| {
                    let removed_features = targets
                        .iter()
                        .filter_map(|target| match target {
                            ComputedSelectionTarget::Feature(feature) => Some(*feature),
                            ComputedSelectionTarget::Corner(_) => None,
                        })
                        .collect::<BTreeSet<_>>();
                    for feature in &removed_features {
                        features.remove_feature(*feature)?;
                    }
                    for owner in targets.iter().filter_map(|target| match target {
                        ComputedSelectionTarget::Corner(owner)
                            if !removed_features.contains(&owner.feature) =>
                        {
                            Some(*owner)
                        }
                        _ => None,
                    }) {
                        features.remove_corner(owner.feature, owner.corner)?;
                    }
                    Ok(())
                },
                ReplayAction::Delete {
                    expected,
                    selection,
                },
            )?;
            let attempt = self.session.last_attempt();
            return Ok(MutationOutcome {
                value: Vec::new(),
                design: self.session.design_identity(),
                attempt: attempt.identity(),
                published_accepted: attempt.accepted_state_identity(),
            });
        }
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
        if let Some(targets) =
            selected_computed_targets(self.session.design_document(), &self.features, &selection)
                .map_err(CoordinatorError::ActionUnavailable)?
        {
            let features_to_change = targets
                .into_iter()
                .map(|target| match target {
                    ComputedSelectionTarget::Feature(feature) => feature,
                    ComputedSelectionTarget::Corner(owner) => owner.feature,
                })
                .collect::<BTreeSet<_>>();
            if features_to_change.iter().any(|feature| {
                self.features
                    .feature(*feature)
                    .is_none_or(|value| value.suppressed == suppressed)
            }) {
                return Err(CoordinatorError::ActionUnavailable(
                    DisabledReason::AlreadyInRequestedState,
                ));
            }
            let feature_expected = self.features.identity();
            self.mutate_features(
                feature_expected,
                move |features| {
                    for feature in &features_to_change {
                        features.set_suppressed(*feature, suppressed)?;
                    }
                    Ok(())
                },
                ReplayAction::SetSuppressed {
                    expected,
                    selection,
                    suppressed,
                },
            )?;
            let attempt = self.session.last_attempt();
            return Ok(MutationOutcome {
                value: Vec::new(),
                design: self.session.design_identity(),
                attempt: attempt.identity(),
                published_accepted: attempt.accepted_state_identity(),
            });
        }
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
        self.clear_transient();
        self.refresh_computed_features();
        self.transcript.push(ReplayAction::Reattempt { expected });
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
        self.apply_editor_effect_with_projected_release_control(effect, projected_drag_control())
    }

    fn commit_solved_point_move(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        model_position: [f64; 2],
        release_control: OperationControl,
    ) -> Result<MutationOutcome<EditorMutation>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let preview = self
            .solved_preview_session()
            .cloned()
            .ok_or(CoordinatorError::MissingSolvedPreview)?;
        let locality = self
            .drag_continuation
            .as_ref()
            .filter(|gesture| gesture.last_accepted_preview.is_some())
            .and_then(|gesture| gesture.locality.clone());
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
        let replay = ReplayAction::Edit {
            expected,
            edit: DocumentEdit::SetPointPosition {
                point,
                position: model_position,
            },
        };
        let retained = if let Some(locality) = locality.as_ref() {
            complete_projected_drag_release(
                self.session
                    .apply_point_position_from_preview_with_drag_locality_controlled(
                        expected,
                        point,
                        model_position,
                        &preview,
                        locality,
                        release_control,
                    )?,
            )?
        } else {
            self.session.apply_point_position_from_preview(
                expected,
                point,
                model_position,
                &preview,
            )?
        };
        let outcome = MutationOutcome {
            value: retained.value().clone(),
            design: retained.design_identity(),
            attempt: retained.attempt_identity(),
            published_accepted: retained.published_accepted_identity(),
        };
        self.record_mutation(replay)?;
        Ok(MutationOutcome {
            value: EditorMutation::PointMove(outcome.value),
            design: outcome.design,
            attempt: outcome.attempt,
            published_accepted: outcome.published_accepted,
        })
    }

    fn apply_editor_effect_with_projected_release_control(
        &mut self,
        effect: &EditorEffect,
        release_control: OperationControl,
    ) -> Result<Option<MutationOutcome<EditorMutation>>, CoordinatorError> {
        match effect {
            EditorEffect::CommitPointMove {
                expected,
                point,
                model_position,
            } => self
                .commit_solved_point_move(*expected, *point, *model_position, release_control)
                .map(Some),
            EditorEffect::PreviewComputedFeatureRadius {
                expected,
                feature,
                radius,
            } => {
                if self
                    .feature_authoring_preview
                    .as_ref()
                    .is_some_and(|preview| {
                        preview.metadata.feature == *feature
                            && preview.accepts_radius_input(expected)
                    })
                {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                self.preview_computed_fillet_radius_exact(*expected, *feature, *radius)?;
                Ok(None)
            }
            EditorEffect::CommitComputedFeatureRadius {
                expected,
                feature,
                radius,
            } => {
                if self
                    .feature_authoring_preview
                    .as_ref()
                    .is_some_and(|preview| {
                        preview.metadata.feature == *feature
                            && preview.accepts_radius_input(expected)
                    })
                {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                self.set_computed_fillet_radius_exact(*expected, *feature, *radius)?;
                Ok(None)
            }
            EditorEffect::RestoreComputedFeatureRadius {
                expected,
                feature,
                radius: _,
            } => {
                if self.feature_authoring_preview.is_some() {
                    self.restore_feature_authoring_radius_preview(*expected, *feature)?;
                } else {
                    if self.computed_evaluation_input()? != *expected {
                        return Err(CoordinatorError::StaleComputedFeatureCandidate);
                    }
                    self.clear_computed_feature_preview();
                }
                Ok(None)
            }
            EditorEffect::ClearComputedFeaturePreview => {
                self.clear_computed_feature_preview();
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
            | EditorEffect::ClearPointPreview
            | EditorEffect::PreviewConstruction(_)
            | EditorEffect::ClearConstructionPreview
            | EditorEffect::PreviewInference(_)
            | EditorEffect::ClearInferencePreview => Ok(None),
        }
    }

    /// Applies one recorded transition against the identities encoded in the transcript.
    ///
    /// # Errors
    ///
    /// Returns the same applicability, stale-design, domain, history, and checkpoint
    /// errors as the corresponding coordinator operation.
    #[allow(clippy::too_many_lines)]
    pub fn replay(&mut self, action: &ReplayAction) -> Result<(), CoordinatorError> {
        if let Some(expected) = action.expected_design() {
            self.ensure_expected(expected)?;
        }
        if self.replay_m55_action(action)? {
            return Ok(());
        }
        match action {
            ReplayAction::CreateComputedFillet {
                expected,
                label,
                radius,
                corners,
            } => {
                let replay = action.clone();
                let label = label.clone();
                let corners = corners.clone();
                self.mutate_features(
                    *expected,
                    move |features| features.create_fillet_set(label, *radius, corners),
                    replay,
                )?;
            }
            ReplayAction::SetComputedFilletRadius {
                expected,
                feature,
                radius,
            } => {
                self.set_computed_fillet_radius(*expected, *feature, *radius)?;
            }
            ReplayAction::RemoveComputedFeature { expected, feature } => {
                self.remove_computed_feature(*expected, *feature)?;
            }
            ReplayAction::RemoveComputedCorner { expected, owner } => {
                self.remove_computed_corner(*expected, *owner)?;
            }
            ReplayAction::SetComputedFeatureSuppressed {
                expected,
                feature,
                suppressed,
            } => {
                self.set_computed_feature_suppressed(*expected, *feature, *suppressed)?;
            }
            ReplayAction::Edit { expected, edit } => {
                self.apply_edit(*expected, edit.clone())?;
            }
            ReplayAction::Construction { expected, proposal } => {
                self.apply_construction(*expected, proposal)?;
            }
            ReplayAction::Operation { request, .. } => self.replay_operation(request)?,
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

    fn replay_operation(
        &mut self,
        request: &SketchOperationRequest,
    ) -> Result<(), CoordinatorError> {
        let tool = match request {
            SketchOperationRequest::AssociativeFillet { .. } => OperationAuthoringTool::Fillet,
            _ => {
                return Err(CoordinatorError::InvalidActionInput(
                    "replay operation is outside the M66 authoring surface",
                ));
            }
        };
        let candidate = OperationAuthoringCandidate::explicit_replay(
            tool,
            request.clone(),
            &self.session.prepared_input(),
        );
        match self.prepare_operation_preview(&candidate)? {
            OperationAuthoringPreviewOutcome::Ready(metadata) => {
                self.apply_operation_preview(metadata.token, &candidate)?;
                Ok(())
            }
            OperationAuthoringPreviewOutcome::Warning(_) => {
                Err(CoordinatorError::InvalidActionInput(
                    "replayed helper operation did not reproduce its accepted preview",
                ))
            }
        }
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
        let checkpoint = self.history[target].clone();
        let current_checkpoint = &self.history[self.history_cursor];
        let sketch_unchanged = checkpoint.design_json == current_checkpoint.design_json
            && checkpoint.design_is_draft_v5 == current_checkpoint.design_is_draft_v5
            && checkpoint.accepted_json == current_checkpoint.accepted_json
            && checkpoint.accepted_is_draft_v5 == current_checkpoint.accepted_is_draft_v5
            && checkpoint.accepted_belongs_to_current_design
                == current_checkpoint.accepted_belongs_to_current_design;
        if !sketch_unchanged {
            let design = checkpoint_document_from_json(
                &checkpoint.design_json,
                checkpoint.design_is_draft_v5,
            )?;
            let input = self.session.last_attempt().input();
            let request = input
                .candidate_request()
                .without_temporary_targets()
                .without_previous_state_preferences();
            let revisions = self.session.revision_high_water();
            self.session = if let Some(json) = &checkpoint.accepted_json {
                let accepted =
                    checkpoint_document_from_json(json, checkpoint.accepted_is_draft_v5)?;
                if checkpoint.accepted_belongs_to_current_design {
                    RetainedSketchDocumentSession::restore_current_design_with_accepted(
                        design,
                        accepted,
                        revisions,
                        request,
                        input.solver_config(),
                    )?
                } else {
                    RetainedSketchDocumentSession::restore_design_with_accepted(
                        design,
                        accepted,
                        revisions,
                        request,
                        input.solver_config(),
                    )?
                }
            } else {
                RetainedSketchDocumentSession::restore_design(
                    design,
                    revisions,
                    request,
                    input.solver_config(),
                )?
            };
        }
        let retained_features = merge_feature_lifecycle_high_water(
            self.features.lifecycle_high_water(),
            checkpoint.feature_lifecycle,
        );
        let mut restored_features = ComputedFeatureDocument::from_json(&checkpoint.feature_json)?;
        if restored_features.sketch_document() != self.session.design_document().id() {
            return Err(ComputedFeatureSnapshotError::FeatureDocumentForDifferentSketch.into());
        }
        restored_features.rebase_after_restore(retained_features)?;
        self.computed_evaluation_allocator
            .retain_high_water(checkpoint.evaluation_allocator);
        self.features = restored_features;
        self.history_cursor = target;
        self.clear_transient();
        self.refresh_computed_features();
        self.reconcile_selection();
        Ok(())
    }

    fn record_mutation(&mut self, replay: ReplayAction) -> Result<(), CoordinatorError> {
        self.refresh_computed_features();
        let next = checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        )?;
        self.history.truncate(self.history_cursor + 1);
        self.history.push(next);
        self.history_cursor += 1;
        self.transcript.push(replay);
        self.clear_transient();
        self.reconcile_selection();
        Ok(())
    }

    fn mutate_features<T>(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        mutation: impl FnOnce(&mut ComputedFeatureDocument) -> Result<T, ComputedFeatureDocumentError>,
        replay: ReplayAction,
    ) -> Result<ComputedFeatureMutation<T>, CoordinatorError> {
        let before = self.features.identity();
        if before != expected {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let sketch_input = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        let mut candidate = self.features.clone();
        let value = mutation(&mut candidate)?;
        let outcome = evaluate_computed_features(
            &self.session,
            &candidate,
            &mut self.computed_evaluation_allocator,
            bounded_geometry_control(),
        )?;
        let OperationOutcome::Completed {
            value: snapshot, ..
        } = outcome
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if self.features.identity() != before
            || self.session.accepted_prepared_input() != Some(sketch_input)
            || snapshot.input().features != candidate.identity()
            || snapshot.input().sketch != sketch_input
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let after = candidate.identity();
        self.features = candidate;
        self.computed_input = Some(snapshot.input());
        self.computed_snapshot = Some(snapshot);
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_evaluation_problem = None;
        self.record_feature_mutation(replay)?;
        Ok(ComputedFeatureMutation {
            value,
            before,
            after,
        })
    }

    fn record_feature_mutation(&mut self, replay: ReplayAction) -> Result<(), CoordinatorError> {
        let next = checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        )?;
        self.history.truncate(self.history_cursor + 1);
        self.history.push(next);
        self.history_cursor += 1;
        self.transcript.push(replay);
        self.clear_transient();
        self.reconcile_selection();
        Ok(())
    }

    fn refresh_computed_features(&mut self) {
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_preview_evaluation_problem = None;
        match evaluate_computed_features(
            &self.session,
            &self.features,
            &mut self.computed_evaluation_allocator,
            bounded_geometry_control(),
        ) {
            Ok(OperationOutcome::Completed { value, .. }) => {
                self.computed_input = Some(value.input());
                self.computed_snapshot = Some(value);
                self.computed_evaluation_problem = None;
            }
            Ok(stopped) => {
                self.computed_input = None;
                self.computed_snapshot = None;
                self.computed_evaluation_problem = Some(format!(
                    "computed-feature evaluation stopped: {:?}",
                    stopped.report().stopping_reason
                ));
            }
            Err(error) => {
                self.computed_input = None;
                self.computed_snapshot = None;
                self.computed_evaluation_problem = Some(error.to_string());
            }
        }
    }

    fn reconcile_selection(&mut self) {
        let document = self.session.design_document();
        let retained = self
            .editor
            .selection()
            .iter()
            .copied()
            .filter(|item| composite_selection_exists(document, &self.features, *item))
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

fn computed_feature_problem(
    features: &ComputedFeatureDocument,
    feature: ComputedFeatureId,
    failure: &ComputedFeatureFailure,
) -> ComputedFeatureProblemMetadata {
    let mut corners = Vec::new();
    let mut sources = Vec::new();
    match failure {
        ComputedFeatureFailure::MissingSource {
            corner,
            span_source,
        }
        | ComputedFeatureFailure::AssociationOwnedSource {
            corner,
            span_source,
        }
        | ComputedFeatureFailure::MultiIntervalSource {
            corner,
            span_source,
        } => {
            corners.push(*corner);
            sources.push(*span_source);
        }
        ComputedFeatureFailure::InvalidParentState { corner }
        | ComputedFeatureFailure::UnsupportedCurvedPair { corner }
        | ComputedFeatureFailure::SingularParents { corner }
        | ComputedFeatureFailure::NoLocalRoot { corner }
        | ComputedFeatureFailure::AmbiguousLocalRoot { corner }
        | ComputedFeatureFailure::UncertifiedBranch { corner }
        | ComputedFeatureFailure::OffsetSingularity { corner }
        | ComputedFeatureFailure::InvalidGeometry { corner } => corners.push(*corner),
        ComputedFeatureFailure::EndpointClaimConflict {
            span_source,
            participants,
            ..
        }
        | ComputedFeatureFailure::ConsumedSourceInterval {
            span_source,
            participants,
        } => {
            sources.push(*span_source);
            corners.extend(
                participants
                    .iter()
                    .filter(|owner| owner.feature == feature)
                    .map(|owner| owner.corner),
            );
        }
        _ => {}
    }
    if corners.is_empty()
        && let Some(value) = features.feature(feature)
    {
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &value.definition;
        corners.extend(fillet.corners.iter().map(|corner| corner.id));
    }
    corners.sort_unstable();
    corners.dedup();
    for corner in &corners {
        if let Some(value) = features.corner(feature, *corner) {
            sources.extend([value.first.source, value.second.source]);
        }
    }
    sources.sort_unstable();
    sources.dedup();
    ComputedFeatureProblemMetadata {
        feature: Some(feature),
        scope: EditorProblemScope::Targeted,
        corners,
        sources,
        message: failure.to_string(),
    }
}

impl ReplayAction {
    const fn expected_design(&self) -> Option<SketchDesignIdentity> {
        match self {
            Self::Edit { expected, .. }
            | Self::Construction { expected, .. }
            | Self::Operation { expected, .. }
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
            | Self::Reattempt { expected } => Some(*expected),
            Self::CreateComputedFillet { .. }
            | Self::SetComputedFilletRadius { .. }
            | Self::RemoveComputedFeature { .. }
            | Self::RemoveComputedCorner { .. }
            | Self::SetComputedFeatureSuppressed { .. }
            | Self::Undo
            | Self::Redo => None,
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

fn operation_authoring_warning(
    candidate: &OperationAuthoringCandidate,
    kind: OperationAuthoringWarningKind,
    message: impl Into<String>,
) -> OperationAuthoringWarning {
    let stage = match (candidate.is_confirmed(), candidate.tool()) {
        (true, _) => OperationAuthoringStage::PreviewReady,
        (false, OperationAuthoringTool::Fillet) => OperationAuthoringStage::PlaceFilletRadius,
    };
    OperationAuthoringWarning {
        tool: candidate.tool(),
        stage,
        kind,
        message: message.into(),
    }
}

fn operation_preview_metadata(
    candidate: &OperationAuthoringCandidate,
    application: &SketchOperationApplication,
    scratch: &RetainedSketchDocumentSession,
    base_design: SketchDesignIdentity,
    accepted: SketchAcceptedStateIdentity,
    token: OperationAuthoringPreviewToken,
) -> Result<OperationAuthoringPreviewMetadata, CoordinatorError> {
    let accepted_document = scratch
        .accepted_state()
        .filter(|state| state.identity() == accepted)
        .ok_or(CoordinatorError::InvalidActionInput(
            "operation preview has no matching accepted document",
        ))?
        .document();
    let mut curves = BTreeSet::new();
    let mut points = BTreeSet::new();
    for change in &application.identity_changes {
        match change {
            SketchOperationIdentityChange::Proposed(DocumentElementId::Curve(curve)) => {
                curves.insert(*curve);
            }
            SketchOperationIdentityChange::Proposed(DocumentElementId::Point(point)) => {
                points.insert(*point);
            }
            _ => {}
        }
    }
    curves.retain(|curve| accepted_document.curve(*curve).is_some());
    points.retain(|point| accepted_document.point(*point).is_some());
    let primary_created_curve =
        curves
            .iter()
            .next()
            .copied()
            .ok_or(CoordinatorError::InvalidActionInput(
                "operation proposal did not publish a primary created curve",
            ))?;
    Ok(OperationAuthoringPreviewMetadata {
        token,
        tool: candidate.tool(),
        base_design,
        accepted,
        primary_created_curve,
        created_curves: curves.into_iter().collect(),
        created_points: points.into_iter().collect(),
        apply_ready: candidate.is_confirmed(),
    })
}

fn checkpoint(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    evaluation_allocator: &ComputedEvaluationAllocator,
) -> Result<RestoreCheckpoint, CoordinatorError> {
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
        accepted_belongs_to_current_design: session
            .accepted_state()
            .is_some_and(|accepted| accepted.design_identity() == session.design_identity()),
        revisions: session.revision_high_water(),
        feature_json: features.to_json()?,
        feature_lifecycle: features.lifecycle_high_water(),
        evaluation_allocator: evaluation_allocator.high_water(),
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
        SelectionItem::Feature(_) | SelectionItem::FeatureCorner(_) => false,
    }
}

fn composite_selection_exists(
    document: &SketchDocument,
    features: &ComputedFeatureDocument,
    item: SelectionItem,
) -> bool {
    match item {
        SelectionItem::Feature(feature) => features.feature(feature).is_some(),
        SelectionItem::FeatureCorner(owner) => {
            features.corner(owner.feature, owner.corner).is_some()
        }
        _ => selection_exists(document, item),
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
        let object = item.object().ok_or(DisabledReason::WrongOperandKind)?;
        if seen.insert(object) {
            objects.push(object);
        }
    }
    Ok(objects)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComputedSelectionTarget {
    Feature(ComputedFeatureId),
    Corner(ComputedCornerRef),
}

fn selected_computed_targets(
    document: &SketchDocument,
    features: &ComputedFeatureDocument,
    selection: &[SelectionItem],
) -> Result<Option<Vec<ComputedSelectionTarget>>, DisabledReason> {
    if selection.is_empty() {
        return Err(DisabledReason::EmptySelection);
    }
    let has_computed = selection.iter().any(|item| {
        matches!(
            item,
            SelectionItem::Feature(_) | SelectionItem::FeatureCorner(_)
        )
    });
    if !has_computed {
        return Ok(None);
    }
    if selection.iter().any(|item| {
        !matches!(
            item,
            SelectionItem::Feature(_) | SelectionItem::FeatureCorner(_)
        )
    }) {
        return Err(DisabledReason::WrongOperandKind);
    }
    let mut targets = Vec::new();
    for item in selection {
        if !composite_selection_exists(document, features, *item) {
            return Err(DisabledReason::MissingObject);
        }
        let target = match item {
            SelectionItem::Feature(feature) => ComputedSelectionTarget::Feature(*feature),
            SelectionItem::FeatureCorner(owner) => ComputedSelectionTarget::Corner(*owner),
            _ => unreachable!("native selections were rejected above"),
        };
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    Ok(Some(targets))
}

fn composite_delete_availability(
    document: &SketchDocument,
    features: &ComputedFeatureDocument,
    selection: &[SelectionItem],
) -> ActionState {
    match selected_computed_targets(document, features, selection) {
        Ok(Some(_)) => ActionState::Enabled,
        Ok(None) => availability(selected_objects(document, selection)),
        Err(reason) => ActionState::Disabled(reason),
    }
}

fn composite_suppression_availability(
    document: &SketchDocument,
    features: &ComputedFeatureDocument,
    selection: &[SelectionItem],
    suppressed: bool,
) -> ActionState {
    match selected_computed_targets(document, features, selection) {
        Ok(Some(targets)) => {
            let feature_ids = targets
                .into_iter()
                .map(|target| match target {
                    ComputedSelectionTarget::Feature(feature) => feature,
                    ComputedSelectionTarget::Corner(owner) => owner.feature,
                })
                .collect::<BTreeSet<_>>();
            if feature_ids.iter().any(|feature| {
                features
                    .feature(*feature)
                    .is_none_or(|value| value.suppressed == suppressed)
            }) {
                ActionState::Disabled(DisabledReason::AlreadyInRequestedState)
            } else {
                ActionState::Enabled
            }
        }
        Ok(None) => source_availability(document, selection, suppressed),
        Err(reason) => ActionState::Disabled(reason),
    }
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
            SelectionItem::Point(_)
            | SelectionItem::Curve(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => None,
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
        AuthoringOutcome, AuthoringState, EditorScene, EditorTool, FeatureAuthoringOutcome,
        FeatureAuthoringStage, FeatureAuthoringState, Modifiers, OperationAuthoringOptions,
        OperationAuthoringState, PickTolerance, PointerInput, ScreenPoint, Viewport,
    };
    use geosolve_sketch::{
        AlphaScenarioIds, AlphaScenarioKind, DocumentConstraintDefinition,
        DocumentExternalPointRef, DocumentM38DimensionDefinition, DocumentMeasurementDefinition,
        DocumentParameterKind, DocumentPointRef, ExternalLineOrientationV1, ExternalSnapshotDigest,
        ExternalSnapshotEntry, ExternalSnapshotFeatureV1, ExternalSnapshotInputError,
        ExternalSnapshotResourcesV1, ExternalSnapshotSet, OperationStopReason,
        OperationWorkCounter, ParameterBatch, ParameterBatchEntry, ParameterValue, PersistentId,
        SolverConfig, alpha_scenario, cancellation_pair,
    };
    use geosolve_sketch_features::ComputedFeatureDefinition;

    #[test]
    fn projected_drag_envelope_pins_every_m65_limit() {
        let limits = projected_drag_control().limits;
        assert_eq!(limits.document_validation_items, 16_384);
        assert_eq!(limits.document_dependency_items, 16_384);
        assert_eq!(limits.document_lowering_items, 16_384);
        assert_eq!(limits.nonlinear_iterations, 256);
        assert_eq!(limits.factorizations, 256);
        assert_eq!(limits.rank_kernels, 256);
        assert_eq!(limits.rejected_trials, 512);
        assert_eq!(limits.component_linearizations, 1_024);
        assert_eq!(limits.dense_kernel_rows, 256);
        assert_eq!(limits.dense_kernel_columns, 256);
        assert_eq!(limits.diagnostic_candidates, 512);
        assert_eq!(limits.diagnostic_trials, 1_024);
    }

    fn assert_projected_drag_work_bounded(work: &ProjectedDragWorkEvidence) {
        assert!(
            work.operation_report_complete,
            "ordinary controlled outcomes must publish complete work evidence"
        );
        let consumed = work.operation.consumed;
        assert!(consumed.document_validation_items <= PROJECTED_DRAG_MAX_DOCUMENT_ITEMS);
        assert!(consumed.document_dependency_items <= PROJECTED_DRAG_MAX_DOCUMENT_ITEMS);
        assert!(consumed.document_lowering_items <= PROJECTED_DRAG_MAX_DOCUMENT_ITEMS);
        assert!(consumed.nonlinear_iterations <= PROJECTED_DRAG_MAX_NONLINEAR_ITERATIONS);
        assert!(consumed.factorizations <= PROJECTED_DRAG_MAX_FACTORIZATIONS);
        assert!(consumed.rank_kernels <= PROJECTED_DRAG_MAX_RANK_KERNELS);
        assert!(consumed.rejected_trials <= PROJECTED_DRAG_MAX_REJECTED_TRIALS);
        assert!(consumed.component_linearizations <= PROJECTED_DRAG_MAX_COMPONENT_LINEARIZATIONS);
        assert!(consumed.dense_kernel_rows <= PROJECTED_DRAG_MAX_DENSE_DIMENSION);
        assert!(consumed.dense_kernel_columns <= PROJECTED_DRAG_MAX_DENSE_DIMENSION);
        assert!(consumed.diagnostic_candidates <= PROJECTED_DRAG_MAX_DIAGNOSTIC_CANDIDATES);
        assert!(consumed.diagnostic_trials <= PROJECTED_DRAG_MAX_DIAGNOSTIC_TRIALS);
    }

    fn circle_drag_fixture() -> (
        RetainedEditorCoordinator,
        EditorScene,
        DesignPointId,
        CurveId,
        [f64; 2],
    ) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let center = document.add_point("center", [1.0, 2.0]).expect("center");
        let radius = document
            .add_scalar(
                "circle radius",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("radius");
        let circle = document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .expect("circle");
        let radius_target = document
            .add_scalar(
                "radius target",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("radius target");
        document
            .add_dimension(
                "fixed radius",
                DocumentDimensionDefinition::Radius {
                    curve: circle,
                    target: radius_target,
                },
                DocumentDimensionMode::Driving,
            )
            .expect("radius dimension");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("circle session");
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let accepted = session.accepted_state().expect("accepted circle");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.5,
        )
        .expect("scene");
        (
            RetainedEditorCoordinator::new(session).expect("coordinator"),
            scene,
            center,
            circle,
            [1.0, 2.0],
        )
    }

    fn unannotated_circle_press(scene: &EditorScene, circle: CurveId) -> ScreenPoint {
        scene
            .curves
            .iter()
            .find(|curve| curve.span.curve == circle)
            .expect("scene circle")
            .screen_polyline
            .iter()
            .copied()
            .find(|position| {
                scene
                    .annotation_hit_test(*position, crate::PickTolerance::default(), &[], None, &[])
                    .is_none()
            })
            .expect("circle sample away from its dimension annotation")
    }

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
        coordinator.pointer_down(&scene, pointer(start));
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
    }

    #[test]
    fn typed_projected_drag_errors_mark_unrecoverable_work_reports_incomplete() {
        let cam = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("cam");
        let pantograph =
            alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).expect("pantograph");
        let AlphaScenarioIds::MotionPantograph(pantograph_ids) = pantograph.ids else {
            unreachable!()
        };
        let mut planning_error = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(cam.document, cam.request, SolverConfig::default())
                .expect("cam session"),
        )
        .expect("coordinator");
        assert!(
            planning_error
                .resolve_projected_point_move(190, 1, pantograph_ids.input, [1.0, 1.0])
                .is_empty()
        );
        let planning_work = planning_error
            .projected_drag_work_evidence()
            .expect("planning error work");
        assert_eq!(planning_work.attempts, 0);
        assert!(!planning_work.accepted);
        assert_eq!(
            planning_work.rejection_stage,
            Some(ProjectedDragRejectionStage::LocalityPlanning)
        );
        assert!(!planning_work.operation_report_complete);
        assert_eq!(
            planning_work.operation.configured,
            projected_drag_control().limits
        );

        let (mut session_error, _, center, _, _) = circle_drag_fixture();
        let _ = session_error.resolve_projected_point_move(191, 1, center, [1.1, 2.1]);
        assert!(
            session_error
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted && work.operation_report_complete)
        );
        let (foreign_preview, _, _, _) = fixed_line_session();
        session_error
            .drag_continuation
            .as_mut()
            .expect("active continuation")
            .last_accepted_preview = Some(foreign_preview);
        assert!(
            session_error
                .resolve_projected_point_move(191, 2, center, [1.2, 2.2])
                .is_empty()
        );
        let session_work = session_error
            .projected_drag_work_evidence()
            .expect("session error work");
        assert!(session_work.continued);
        assert_eq!(session_work.attempts, 0);
        assert!(!session_work.accepted);
        assert_eq!(
            session_work.rejection_stage,
            Some(ProjectedDragRejectionStage::Session)
        );
        assert!(!session_work.operation_report_complete);
        assert_eq!(
            session_work.operation.configured,
            projected_drag_control().limits
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one focused gesture proves semantic routing, bounded projection, and commit"
    )]
    fn reference_fillet_arc_drag_routes_to_semantic_center_and_commits() {
        let fixture =
            alpha_scenario(AlphaScenarioKind::FilletLineCircle, 1.0).expect("fillet fixture");
        let AlphaScenarioIds::FilletLineCircle(ids) = fixture.ids else {
            panic!("generic fillet IDs")
        };
        let session = RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .expect("fillet session");
        let center = ids.fillet.center;
        let arc = ids.fillet.arc;
        let radius = ids.fillet.radius;
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let deletion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(ids.fillet.radius_dimension),
                },
            )
            .expect("delete only the fillet radius dimension");
        assert!(deletion.published_accepted.is_some());
        let initial_accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted fillet after radius deletion");
        assert!(
            initial_accepted
                .document()
                .dimension(ids.fillet.radius_dimension)
                .is_none()
        );
        assert!(
            initial_accepted
                .document()
                .constraint(ids.fillet.constraint)
                .is_some()
        );
        let initial_center = initial_accepted
            .document()
            .point(center)
            .expect("fillet center")
            .position;
        let initial_radius = initial_accepted
            .document()
            .scalar(radius)
            .expect("fillet radius")
            .value;
        let pre_drag_document = initial_accepted.document().clone();
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted fillet");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport,
            0.5,
        )
        .expect("fillet scene");
        let arc_curve = scene
            .curves
            .iter()
            .find(|curve| curve.span == CurveSpan::line(arc))
            .expect("visible fillet arc");
        assert_eq!(arc_curve.drag_handle_point, Some(center));
        let press = arc_curve.screen_polyline[arc_curve.screen_polyline.len() / 2];
        let moved_pointer = ScreenPoint {
            x: press.x + 10.0,
            y: press.y - 7.5,
        };
        let pointer = |position| PointerInput {
            pointer_id: 201,
            position,
            modifiers: Modifiers::default(),
        };
        coordinator.editor_mut().activate_tool(EditorTool::Select);
        assert_eq!(
            coordinator.pointer_down(&scene, pointer(press)),
            vec![EditorEffect::SelectionChanged(vec![SelectionItem::Curve(
                CurveSpan::line(arc)
            )])]
        );
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved_pointer));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("fillet center drag request")
        };
        assert_eq!(*point, center);
        let expected_center = [initial_center[0] + 0.2, initial_center[1] + 0.15];
        assert!((model_position[0] - expected_center[0]).abs() <= 1.0e-12);
        assert!((model_position[1] - expected_center[1]).abs() <= 1.0e-12);

        let preview_effects = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *point,
            *model_position,
        );
        let [
            EditorEffect::PreviewPointMove {
                point: preview_point,
                model_position: preview_center,
            },
        ] = preview_effects.as_slice()
        else {
            panic!(
                "effects={preview_effects:#?} work={:#?}",
                coordinator.projected_drag_work_evidence()
            )
        };
        assert_eq!(*preview_point, center);
        assert!(preview_center.iter().all(|value| value.is_finite()));
        assert!(
            (preview_center[0] - initial_center[0]).hypot(preview_center[1] - initial_center[1])
                > 1.0e-4
        );
        assert_projected_drag_work_bounded(
            coordinator
                .projected_drag_work_evidence()
                .expect("fillet drag work"),
        );
        let requested_pointer_target = *model_position;
        let preview_accepted = coordinator
            .solved_preview_session()
            .expect("fillet preview session")
            .accepted_state()
            .expect("accepted fillet preview");
        let preview_document = preview_accepted.document().clone();
        let runtime_drag = preview_accepted
            .runtime()
            .request()
            .drag
            .expect("preview runtime retains the public drag target");
        assert_eq!(
            Some(runtime_drag.point),
            preview_accepted.mappings().runtime_point(center)
        );
        assert_eq!(
            [runtime_drag.target.x, runtime_drag.target.y].map(f64::to_bits),
            requested_pointer_target.map(f64::to_bits),
            "accepted diagnostics must correspond to the original pointer target"
        );
        let expected_design = coordinator.session().design_identity();
        let release =
            coordinator
                .editor_mut()
                .pointer_up(&scene, expected_design, pointer(moved_pointer));
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitPointMove {
                    expected,
                    point,
                    model_position,
                },
                EditorEffect::ClearPointPreview,
            ] if *expected == expected_design
                && *point == center
                && (model_position[0] - preview_center[0]).abs() <= 1.0e-8
                && (model_position[1] - preview_center[1]).abs() <= 1.0e-8
        ));
        let history_before_release = coordinator.history_len();
        let attempt_before_release = coordinator.session().last_attempt().identity();
        let mut exhausted_control = projected_drag_control();
        exhausted_control.limits.document_validation_items = 0;
        assert!(matches!(
            coordinator.apply_editor_effect_with_projected_release_control(
                &release[0],
                exhausted_control,
            ),
            Err(CoordinatorError::Session(
                DocumentSessionError::PreviewReleaseInterrupted {
                    stopping_reason: OperationStopReason::WorkExhausted {
                        counter: OperationWorkCounter::DocumentValidationItems,
                        ..
                    },
                }
            ))
        ));
        assert_eq!(coordinator.history_len(), history_before_release);
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            attempt_before_release
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("exhausted release retains accepted state")
                .document(),
            &pre_drag_document
        );
        assert_eq!(
            coordinator
                .solved_preview_session()
                .expect("exhausted release retains preview")
                .accepted_state()
                .expect("retained accepted preview")
                .document(),
            &preview_document
        );
        coordinator
            .apply_editor_effect(&release[0])
            .expect("release dispatch")
            .expect("release mutation");
        let committed = coordinator
            .session()
            .accepted_state()
            .expect("committed fillet");
        let committed_center = committed
            .document()
            .point(center)
            .expect("committed center")
            .position;
        let committed_radius = committed
            .document()
            .scalar(radius)
            .expect("committed radius")
            .value;
        assert_eq!(committed.document(), &preview_document);
        assert!(
            committed
                .document()
                .dimension(ids.fillet.radius_dimension)
                .is_none()
        );
        assert!((committed_center[0] - preview_center[0]).abs() <= 1.0e-8);
        assert!((committed_center[1] - preview_center[1]).abs() <= 1.0e-8);
        assert!((committed_radius - initial_radius).abs() > 1.0e-4);
        let committed_solve = committed
            .diagnostics()
            .solve
            .expect("committed solve diagnostics");
        assert_eq!(
            committed_solve.hard_validity,
            geosolve_sketch::SketchHardValidity::Valid
        );
        assert!(
            committed_solve
                .maximum_normalized_hard_residual
                .is_some_and(|residual| residual <= 1.0e-9)
        );
        let committed_document = committed.document().clone();
        coordinator.undo().expect("undo exact fillet release");
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("undo accepted fillet")
                .document(),
            &pre_drag_document
        );
        coordinator.redo().expect("redo exact fillet release");
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("redo accepted fillet")
                .document(),
            &committed_document
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end pointer gesture owns offset, release, history, and cancellation"
    )]
    fn circle_offset_drag_release_cancel_and_history_use_the_ordinary_lifecycle() {
        let (mut coordinator, scene, center, circle, initial_center) = circle_drag_fixture();
        coordinator.editor_mut().activate_tool(EditorTool::Select);
        let press = unannotated_circle_press(&scene, circle);
        let moved_pointer = ScreenPoint {
            x: press.x + 10.0,
            y: press.y - 5.0,
        };
        let pointer = |pointer_id, position| PointerInput {
            pointer_id,
            position,
            modifiers: Modifiers::default(),
        };
        assert_eq!(
            coordinator.pointer_down(&scene, pointer(101, press)),
            vec![EditorEffect::SelectionChanged(vec![SelectionItem::Curve(
                CurveSpan::line(circle)
            )])]
        );
        let press_plan = coordinator
            .drag_continuation
            .as_ref()
            .expect("press-time drag continuation");
        assert!(press_plan.locality.is_some());
        assert!(press_plan.planning_failure.is_none());
        assert!(press_plan.planning_operation.is_some());
        assert_eq!(press_plan.last_request_id, None);
        assert!(coordinator.projected_drag_work_evidence().is_none());
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(101, moved_pointer));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("circle projected request")
        };
        assert_eq!(*point, center);
        assert!(
            (model_position[0] - 1.2).abs() <= 1.0e-12
                && (model_position[1] - 2.1).abs() <= 1.0e-12,
            "circumference offset was not preserved: {model_position:?}"
        );
        let effects = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *point,
            *model_position,
        );
        assert!(matches!(
            effects.as_slice(),
            [EditorEffect::PreviewPointMove {
                point: preview_point,
                model_position: preview_position,
            }] if *preview_point == center
                && (preview_position[0] - 1.2).abs() <= 1.0e-8
                && (preview_position[1] - 2.1).abs() <= 1.0e-8
        ));
        assert_projected_drag_work_bounded(
            coordinator
                .projected_drag_work_evidence()
                .expect("circle drag work"),
        );
        assert!(
            coordinator
                .drag_continuation
                .as_ref()
                .is_some_and(|gesture| gesture.planning_operation.is_none()),
            "press-time planning work must be charged exactly once"
        );

        let expected = coordinator.session().design_identity();
        let release =
            coordinator
                .editor_mut()
                .pointer_up(&scene, expected, pointer(101, moved_pointer));
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitPointMove {
                    expected: effect_expected,
                    point: effect_point,
                    ..
                },
                EditorEffect::ClearPointPreview,
            ] if *effect_expected == expected && *effect_point == center
        ));
        coordinator
            .apply_editor_effect(&release[0])
            .expect("release dispatch")
            .expect("release mutation");
        assert_eq!(coordinator.history_len(), 2);
        let moved_center = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .document()
            .point(center)
            .unwrap()
            .position;
        assert!((moved_center[0] - 1.2).abs() <= 1.0e-8);
        assert!((moved_center[1] - 2.1).abs() <= 1.0e-8);

        coordinator.undo().expect("undo released drag");
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .unwrap()
                .document()
                .point(center)
                .unwrap()
                .position
                .map(f64::to_bits),
            initial_center.map(f64::to_bits)
        );
        coordinator.redo().expect("redo released drag");
        let redone_center = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .document()
            .point(center)
            .unwrap()
            .position;
        assert!((redone_center[0] - moved_center[0]).abs() <= 1.0e-10);
        assert!((redone_center[1] - moved_center[1]).abs() <= 1.0e-10);

        let viewport = scene.viewport;
        let accepted = coordinator.session().accepted_state().unwrap();
        let cancel_scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport,
            0.5,
        )
        .expect("cancel scene");
        let cancel_press = unannotated_circle_press(&cancel_scene, circle);
        let cancel_move = ScreenPoint {
            x: cancel_press.x + 5.0,
            y: cancel_press.y - 5.0,
        };
        coordinator.pointer_down(&cancel_scene, pointer(102, cancel_press));
        let cancel_request = coordinator
            .editor_mut()
            .pointer_move(&cancel_scene, pointer(102, cancel_move));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = cancel_request.as_slice()
        else {
            panic!("cancel projected request")
        };
        let _ = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *point,
            *model_position,
        );
        assert!(
            coordinator
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted)
        );
        let history_before_cancel = coordinator.history_len();
        let accepted_before_cancel = coordinator.session().accepted_state().unwrap().identity();
        assert_eq!(
            coordinator.editor_mut().cancel(),
            vec![EditorEffect::ClearPointPreview]
        );
        assert!(
            coordinator
                .resolve_projected_point_move(*pointer_id, *request_id, *point, *model_position)
                .is_empty()
        );
        coordinator.clear_transient();
        assert_eq!(coordinator.history_len(), history_before_cancel);
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            accepted_before_cancel
        );
        assert!(coordinator.solved_preview_session().is_none());
    }

    #[test]
    fn exhausted_pointer_sample_retains_complete_preview_and_recovers_in_gesture() {
        let (mut coordinator, _, center, _, _) = circle_drag_fixture();
        let _ = coordinator.resolve_projected_point_move(108, 1, center, [1.2, 2.1]);
        let first_work = *coordinator
            .projected_drag_work_evidence()
            .expect("first sample work");
        assert!(first_work.accepted, "{first_work:#?}");
        let preview_before = coordinator
            .solved_preview_session()
            .expect("accepted preview")
            .accepted_state()
            .expect("accepted preview state");
        let preview_identity = preview_before.identity();
        let preview_document = preview_before.document().clone();

        let mut exhausted_control = projected_drag_control();
        exhausted_control.limits.document_validation_items = 0;
        coordinator
            .drag_continuation
            .as_mut()
            .expect("active continuation")
            .planning_operation = Some(OperationController::new(exhausted_control).report());

        assert!(
            coordinator
                .resolve_projected_point_move(108, 2, center, [1.3, 2.2])
                .is_empty()
        );
        let exhausted = *coordinator
            .projected_drag_work_evidence()
            .expect("exhausted sample work");
        assert_eq!(exhausted.attempts, 1);
        assert!(exhausted.continued);
        assert!(!exhausted.accepted);
        assert_eq!(
            exhausted.rejection_stage,
            Some(ProjectedDragRejectionStage::ControlledOperation)
        );
        assert!(matches!(
            exhausted.operation.stopping_reason,
            Some(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DocumentValidationItems,
                ..
            })
        ));
        let retained = coordinator
            .solved_preview_session()
            .expect("retained accepted preview")
            .accepted_state()
            .expect("retained preview state");
        assert_eq!(retained.identity(), preview_identity);
        assert_eq!(retained.document(), &preview_document);

        let _ = coordinator.resolve_projected_point_move(108, 3, center, [1.4, 2.3]);
        let recovered = *coordinator
            .projected_drag_work_evidence()
            .expect("recovered sample work");
        assert!(recovered.accepted, "{recovered:#?}");
        assert!(recovered.continued);
        assert_projected_drag_work_bounded(&recovered);
        let recovered_position = coordinator
            .solved_preview_session()
            .expect("recovered preview")
            .accepted_state()
            .expect("recovered state")
            .document()
            .point(center)
            .expect("recovered center")
            .position;
        assert!((recovered_position[0] - 1.4).abs() <= 1.0e-10);
        assert!((recovered_position[1] - 2.3).abs() <= 1.0e-10);
        assert_ne!(
            recovered_position.map(f64::to_bits),
            preview_document
                .point(center)
                .expect("retained center")
                .position
                .map(f64::to_bits)
        );
    }

    #[test]
    fn exhausted_exact_release_retains_preview_and_history_for_retry() {
        let (mut coordinator, _, center, _, _) = circle_drag_fixture();
        let _ = coordinator.resolve_projected_point_move(110, 1, center, [1.2, 2.1]);
        assert!(
            coordinator
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted)
        );
        let preview = coordinator
            .solved_preview_session()
            .expect("accepted preview")
            .accepted_state()
            .expect("preview accepted state");
        let release_position = preview
            .document()
            .point(center)
            .expect("preview center")
            .position;
        let release = EditorEffect::CommitPointMove {
            expected: coordinator.session().design_identity(),
            point: center,
            model_position: release_position,
        };
        let design_before = coordinator.session().design_identity();
        let attempt_before = coordinator.session().last_attempt().identity();
        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("persisted accepted state")
            .identity();
        let history_before = coordinator.history_len();
        let transcript_before = coordinator.transcript().len();
        let preview_identity = preview.identity();
        let preview_document = preview.document().clone();

        let mut exhausted_control = projected_drag_control();
        exhausted_control.limits.document_validation_items = 0;
        assert!(matches!(
            coordinator
                .apply_editor_effect_with_projected_release_control(&release, exhausted_control,),
            Err(CoordinatorError::Session(
                DocumentSessionError::PreviewReleaseInterrupted {
                    stopping_reason: OperationStopReason::WorkExhausted {
                        counter: OperationWorkCounter::DocumentValidationItems,
                        ..
                    },
                }
            ))
        ));
        assert_eq!(coordinator.session().design_identity(), design_before);
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            attempt_before
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("persisted accepted state")
                .identity(),
            accepted_before
        );
        assert_eq!(coordinator.history_len(), history_before);
        assert_eq!(coordinator.transcript().len(), transcript_before);
        let retained_preview = coordinator
            .solved_preview_session()
            .expect("release failure retains preview")
            .accepted_state()
            .expect("retained preview state");
        assert_eq!(retained_preview.identity(), preview_identity);
        assert_eq!(retained_preview.document(), &preview_document);

        let committed = coordinator
            .apply_editor_effect(&release)
            .expect("bounded release retry")
            .expect("point mutation");
        assert!(committed.published_accepted.is_some());
        assert_eq!(coordinator.history_len(), history_before + 1);
        assert_eq!(coordinator.transcript().len(), transcript_before + 1);
    }

    #[test]
    fn stopped_press_time_planning_is_charged_once_and_never_replanned() {
        let (mut coordinator, scene, _, circle, _) = circle_drag_fixture();
        let press = unannotated_circle_press(&scene, circle);
        let pointer = |position| PointerInput {
            pointer_id: 109,
            position,
            modifiers: Modifiers::default(),
        };
        let (cancellation, token) = cancellation_pair();
        cancellation.cancel();
        let mut control = projected_drag_control();
        control.token = token;
        coordinator.pointer_down_with_problem_items_controlled(
            &scene,
            pointer(press),
            &[],
            control,
        );
        let planned = coordinator
            .drag_continuation
            .as_ref()
            .expect("tracked stopped plan");
        assert!(planned.locality.is_none());
        assert!(planned.planning_operation.is_some());
        assert!(planned.planning_failure.is_some_and(|failure| {
            failure.rejection_stage == ProjectedDragRejectionStage::LocalityPlanning
                && failure.operation_report_complete
        }));

        let moved = ScreenPoint {
            x: press.x + 10.0,
            y: press.y,
        };
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("first stopped-plan request")
        };
        assert!(
            coordinator
                .resolve_projected_point_move(*pointer_id, *request_id, *point, *model_position,)
                .is_empty()
        );
        let first = *coordinator
            .projected_drag_work_evidence()
            .expect("first stopped-plan evidence");
        assert_eq!(first.attempts, 0);
        assert_eq!(
            first.rejection_stage,
            Some(ProjectedDragRejectionStage::LocalityPlanning)
        );
        assert!(first.operation.stopping_reason.is_some());
        assert!(
            coordinator
                .drag_continuation
                .as_ref()
                .is_some_and(|gesture| gesture.planning_operation.is_none())
        );

        let request = coordinator.editor_mut().pointer_move(
            &scene,
            pointer(ScreenPoint {
                x: moved.x + 5.0,
                y: moved.y,
            }),
        );
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("second stopped-plan request")
        };
        let _ = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *point,
            *model_position,
        );
        let second = coordinator
            .projected_drag_work_evidence()
            .expect("second stopped-plan evidence");
        assert_eq!(second.attempts, 0);
        assert_eq!(
            second.rejection_stage,
            Some(ProjectedDragRejectionStage::LocalityPlanning)
        );
        assert_eq!(second.operation.stopping_reason, None);
        assert_eq!(second.operation.consumed, OperationWork::default());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one issued-result sequence covers out-of-order, duplicate, late, and final release behavior"
    )]
    fn issued_projection_results_ignore_out_of_order_duplicate_and_late_delivery() {
        fn request_tuple(effects: &[EditorEffect]) -> (u64, u64, DesignPointId, [f64; 2]) {
            let [
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point,
                    model_position,
                },
            ] = effects
            else {
                panic!("projected request")
            };
            (*pointer_id, *request_id, *point, *model_position)
        }

        let (mut coordinator, scene, center, circle, _) = circle_drag_fixture();
        let pointer = |position| PointerInput {
            pointer_id: 111,
            position,
            modifiers: Modifiers::default(),
        };
        let press = unannotated_circle_press(&scene, circle);
        let moved = |x: f64, y: f64| ScreenPoint {
            x: press.x + x,
            y: press.y + y,
        };
        coordinator.pointer_down(&scene, pointer(press));
        let first = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved(5.0, 0.0)));
        let second = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved(10.0, -5.0)));
        let first = request_tuple(&first);
        let second = request_tuple(&second);

        assert!(
            coordinator
                .resolve_projected_point_move(first.0, first.1, first.2, first.3)
                .is_empty()
        );
        assert!(coordinator.projected_drag_work_evidence().is_none());
        let _ = coordinator.resolve_projected_point_move(second.0, second.1, second.2, second.3);
        let second_work = *coordinator
            .projected_drag_work_evidence()
            .expect("current result work");
        assert!(second_work.accepted, "{second_work:#?}");
        let second_preview = coordinator
            .solved_preview_session()
            .unwrap()
            .accepted_state()
            .unwrap()
            .identity();

        assert!(
            coordinator
                .resolve_projected_point_move(second.0, second.1, second.2, second.3)
                .is_empty()
        );
        assert_eq!(
            coordinator.projected_drag_work_evidence(),
            Some(&second_work)
        );
        assert_eq!(
            coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .identity(),
            second_preview
        );

        let third = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved(15.0, -5.0)));
        let fourth = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved(17.5, -2.5)));
        let third = request_tuple(&third);
        let fourth = request_tuple(&fourth);
        assert!(
            coordinator
                .resolve_projected_point_move(third.0, third.1, third.2, third.3)
                .is_empty()
        );
        assert_eq!(
            coordinator.projected_drag_work_evidence(),
            Some(&second_work)
        );
        let _ = coordinator.resolve_projected_point_move(fourth.0, fourth.1, fourth.2, fourth.3);
        let fourth_work = *coordinator
            .projected_drag_work_evidence()
            .expect("latest result work");
        assert!(fourth_work.accepted, "{fourth_work:#?}");
        assert!(fourth_work.continued);
        assert_projected_drag_work_bounded(&fourth_work);

        let expected = coordinator.session().design_identity();
        let release =
            coordinator
                .editor_mut()
                .pointer_up(&scene, expected, pointer(moved(17.5, -2.5)));
        assert!(
            coordinator
                .resolve_projected_point_move(fourth.0, fourth.1, fourth.2, fourth.3)
                .is_empty()
        );
        assert_eq!(
            coordinator.projected_drag_work_evidence(),
            Some(&fourth_work)
        );
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitPointMove { point, .. },
                EditorEffect::ClearPointPreview,
            ] if *point == center
        ));
        coordinator
            .apply_editor_effect(&release[0])
            .expect("release")
            .expect("mutation");
    }

    #[test]
    fn tracked_request_is_stale_when_accepted_state_changes_before_first_move() {
        let (mut coordinator, scene, _, circle, _) = circle_drag_fixture();
        let press = unannotated_circle_press(&scene, circle);
        let pointer = |position| PointerInput {
            pointer_id: 112,
            position,
            modifiers: Modifiers::default(),
        };
        coordinator.pointer_down(&scene, pointer(press));
        let press_accepted = coordinator
            .drag_continuation
            .as_ref()
            .and_then(|gesture| gesture.accepted)
            .expect("press accepted stamp");
        coordinator
            .reattempt(coordinator.session().design_identity())
            .expect("fresh accepted attempt");
        assert_ne!(
            coordinator
                .session()
                .accepted_state()
                .expect("reattempt accepted")
                .identity(),
            press_accepted
        );

        let request = coordinator.editor_mut().pointer_move(
            &scene,
            pointer(ScreenPoint {
                x: press.x + 10.0,
                y: press.y,
            }),
        );
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("tracked request")
        };
        assert!(
            coordinator
                .resolve_projected_point_move(*pointer_id, *request_id, *point, *model_position,)
                .is_empty()
        );
        assert!(coordinator.projected_drag_work_evidence().is_none());
        assert!(coordinator.solved_preview_session().is_none());
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
        assert_eq!(work.attempts, 1);
        assert!(!work.continued);
        assert!(work.accepted);
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
        let continued_work = *coordinator
            .projected_drag_work_evidence()
            .expect("continued drag work");
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
        assert!(failed_work.rejection_stage.is_some());
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
        coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: coordinator.session().design_identity(),
                point: ids.left_center,
                model_position: left_preview,
            })
            .expect("commit projected drag")
            .expect("retained mutation");
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
        assert!(
            (left_after[0] - left_before_second_drag[0])
                .hypot(left_after[1] - left_before_second_drag[1])
                <= 1.0e-8,
            "first control moved while independently dragging the second"
        );
    }

    #[test]
    fn both_twin_rollers_keep_the_passive_center_fixed_across_pointer_path_shapes() {
        for active_side in 0..2 {
            let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("cam sample");
            let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
                unreachable!()
            };
            let centers = [ids.left_center, ids.right_center];
            let active = centers[active_side];
            let passive = centers[1 - active_side];
            let session = RetainedSketchDocumentSession::new(
                fixture.document,
                fixture.request,
                SolverConfig::default(),
            )
            .expect("cam session");
            let accepted = session.accepted_state().expect("accepted cam").document();
            let active_start = accepted.point(active).expect("active center").position;
            let passive_start = accepted.point(passive).expect("passive center").position;
            let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
            let path = [
                (
                    "horizontal positive",
                    [active_start[0] + 0.01, active_start[1]],
                ),
                (
                    "horizontal reversal",
                    [active_start[0] - 0.01, active_start[1]],
                ),
                (
                    "vertical positive",
                    [active_start[0], active_start[1] + 0.01],
                ),
                (
                    "vertical reversal",
                    [active_start[0], active_start[1] - 0.01],
                ),
                (
                    "diagonal positive",
                    [active_start[0] + 0.01, active_start[1] + 0.01],
                ),
                (
                    "diagonal reversal",
                    [active_start[0] - 0.01, active_start[1] - 0.01],
                ),
            ];
            let mut active_previous = active_start;

            for (index, (path_name, target)) in path.into_iter().enumerate() {
                let _ = coordinator.resolve_projected_point_move(
                    70 + u64::try_from(active_side).unwrap(),
                    u64::try_from(index + 1).unwrap(),
                    active,
                    target,
                );
                let work = coordinator
                    .projected_drag_work_evidence()
                    .expect("roller drag work");
                assert_eq!(work.attempts, 1);
                assert!(
                    work.accepted,
                    "roller {active_side}, {path_name} sample {index}: {work:#?}"
                );
                assert_projected_drag_work_bounded(work);
                let preview = coordinator
                    .solved_preview_session()
                    .expect("accepted roller preview")
                    .accepted_state()
                    .expect("accepted state")
                    .document();
                let active_position = preview.point(active).expect("active center").position;
                let passive_position = preview.point(passive).expect("passive center").position;
                let requested_delta = [
                    target[0] - active_previous[0],
                    target[1] - active_previous[1],
                ];
                let projected_delta = [
                    active_position[0] - active_previous[0],
                    active_position[1] - active_previous[1],
                ];
                assert!(
                    requested_delta[0] * projected_delta[0]
                        + requested_delta[1] * projected_delta[1]
                        > 1.0e-10,
                    "roller {active_side}, {path_name} sample {index}: projected motion \
                     {projected_delta:?} did not follow pointer motion {requested_delta:?}"
                );
                assert!(
                    (passive_position[0] - passive_start[0])
                        .hypot(passive_position[1] - passive_start[1])
                        <= 1.0e-8,
                    "roller {active_side}, {path_name} sample {index}: passive center moved from \
                     {passive_start:?} to {passive_position:?}"
                );
                active_previous = active_position;
            }
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the bounded-work gate keeps the four representative mechanism paths in one corpus"
    )]
    fn representative_mechanism_drag_corpus_is_bounded_to_one_attempt_per_sample() {
        fn run_path(
            mut coordinator: RetainedEditorCoordinator,
            point: DesignPointId,
            targets: &[[f64; 2]],
        ) {
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
                assert_projected_drag_work_bounded(work);
                let projected = coordinator
                    .solved_preview_session()
                    .expect("accepted mechanism preview")
                    .accepted_state()
                    .expect("accepted state")
                    .document()
                    .point(point)
                    .expect("driven point")
                    .position;
                assert!(
                    (projected[0] - target[0]).hypot(projected[1] - target[1]) <= 1.0e-8,
                    "driven point projected to {projected:?}, target was {target:?}"
                );
            }
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
        run_path(
            scotch,
            scotch_ids.slider,
            &[[3.2, -6.0], [3.2, -5.8], [3.4, -5.6], [3.2, -5.8]],
        );

        let scissor_fixture = alpha_scenario(AlphaScenarioKind::MotionScissor, 1.0).unwrap();
        let AlphaScenarioIds::MotionScissor(scissor_ids) = scissor_fixture.ids else {
            unreachable!()
        };
        run_path(
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
            &[[3.9, 0.0], [3.7, 0.0], [3.85, 0.0]],
        );

        let tower_fixture = alpha_scenario(AlphaScenarioKind::MotionScissorTower, 1.0).unwrap();
        let AlphaScenarioIds::MotionScissorTower(tower_ids) = tower_fixture.ids else {
            unreachable!()
        };
        run_path(
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
            &[[3.9, 0.0], [3.7, 0.0], [3.85, 0.0]],
        );

        let pantograph_fixture = alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).unwrap();
        let AlphaScenarioIds::MotionPantograph(pantograph_ids) = pantograph_fixture.ids else {
            unreachable!()
        };
        let radius = 17.0_f64.sqrt();
        let pantograph_targets =
            [0.27_f64, 0.30, 0.33].map(|angle| [radius * angle.cos(), radius * angle.sin()]);
        run_path(
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
    }

    #[test]
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
        for (index, target) in [[4.0, 2.0], [3.8, 2.2], [3.6, 2.4]].into_iter().enumerate() {
            let _ = coordinator.resolve_projected_point_move(
                92,
                u64::try_from(index + 1).unwrap(),
                ids.input,
                target,
            );
            let work = coordinator.projected_drag_work_evidence().unwrap();
            assert_eq!(work.attempts, 1);
            assert!(work.accepted, "{work:#?}");
            assert_projected_drag_work_bounded(work);
        }
    }

    #[test]
    fn off_manifold_pantograph_guide_path_projects_nearest_and_keeps_input_fixed() {
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
        let input_start = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .document()
            .point(ids.input)
            .unwrap()
            .position;
        let radius = 10.0_f64.sqrt();

        for (index, target) in [[1.2_f64, 3.0], [0.8, 3.2], [1.3, 2.8]]
            .into_iter()
            .enumerate()
        {
            let target_norm = target[0].hypot(target[1]);
            let expected = [
                radius * target[0] / target_norm,
                radius * target[1] / target_norm,
            ];
            let _ = coordinator.resolve_projected_point_move(
                93,
                u64::try_from(index + 1).unwrap(),
                ids.guide,
                target,
            );
            let work = coordinator.projected_drag_work_evidence().unwrap();
            assert_eq!(work.attempts, 1, "{work:#?}");
            assert!(work.accepted, "{work:#?}");
            assert_projected_drag_work_bounded(work);

            let preview = coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .document();
            let guide = preview.point(ids.guide).unwrap().position;
            let input = preview.point(ids.input).unwrap().position;
            assert!(
                (guide[0] - expected[0]).hypot(guide[1] - expected[1]) <= 2.0e-6,
                "guide target {target:?} projected to {guide:?}, expected {expected:?}"
            );
            assert!(
                (input[0] - input_start[0]).hypot(input[1] - input_start[1]) <= 1.0e-8,
                "guide drag moved passive input from {input_start:?} to {input:?}"
            );
        }
    }

    #[test]
    fn every_pantograph_control_projects_on_its_local_configuration() {
        let input_angle = 1.0_f64.atan2(4.0);
        let guide_angle = 3.0_f64.atan2(1.0);
        let input_radius = 17.0_f64.sqrt();
        let guide_radius = 10.0_f64.sqrt();
        let configuration = |input_delta: f64, guide_delta: f64| {
            let input = [
                input_radius * (input_angle + input_delta).cos(),
                input_radius * (input_angle + input_delta).sin(),
            ];
            let guide = [
                guide_radius * (guide_angle + guide_delta).cos(),
                guide_radius * (guide_angle + guide_delta).sin(),
            ];
            let output = [input[0] + guide[0], input[1] + guide[1]];
            (input, guide, output, [0.5 * output[0], 0.5 * output[1]])
        };
        let moved = configuration(0.015, -0.012);

        for (case, target_of) in [(0_u8, moved.0), (1, moved.1), (2, moved.2), (3, moved.3)] {
            let fixture = alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).unwrap();
            let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
                unreachable!()
            };
            let point = [ids.input, ids.guide, ids.output, ids.center][usize::from(case)];
            let mut coordinator = RetainedEditorCoordinator::new(
                RetainedSketchDocumentSession::new(
                    fixture.document,
                    fixture.request,
                    SolverConfig::default(),
                )
                .unwrap(),
            )
            .unwrap();
            let _ =
                coordinator.resolve_projected_point_move(92 + u64::from(case), 1, point, target_of);
            let work = coordinator.projected_drag_work_evidence().unwrap();
            assert_eq!(work.attempts, 1);
            assert!(work.accepted, "pantograph case {case}: {work:#?}");
            assert_projected_drag_work_bounded(work);
            let projected = coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .document()
                .point(point)
                .unwrap()
                .position;
            assert!(
                (projected[0] - target_of[0]).hypot(projected[1] - target_of[1]) <= 1.0e-8,
                "pantograph case {case} projected {projected:?}, target {target_of:?}"
            );
        }
    }

    #[test]
    fn difficult_twin_roller_projection_is_bounded_and_recovery_retains_continuation() {
        fn roller_target(parameter: f64) -> [f64; 2] {
            let tangent: [f64; 2] = [8.0, 8.0 - 16.0 * parameter];
            let tangent_norm = tangent[0].hypot(tangent[1]);
            [
                -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
                8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
            ]
        }

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

        let _ =
            coordinator.resolve_projected_point_move(93, 1, ids.left_center, roller_target(0.26));
        let first = *coordinator.projected_drag_work_evidence().unwrap();
        assert!(first.accepted, "{first:#?}");
        let retained_preview = coordinator
            .solved_preview_session()
            .unwrap()
            .accepted_state()
            .unwrap()
            .identity();

        let _ = coordinator.resolve_projected_point_move(93, 2, ids.left_center, [5.0, -5.0]);
        let difficult = *coordinator.projected_drag_work_evidence().unwrap();
        assert!(!difficult.accepted, "{difficult:#?}");
        assert_projected_drag_work_bounded(&difficult);
        assert_eq!(
            coordinator
                .solved_preview_session()
                .unwrap()
                .accepted_state()
                .unwrap()
                .identity(),
            retained_preview
        );

        let _ =
            coordinator.resolve_projected_point_move(93, 3, ids.left_center, roller_target(0.28));
        let recovered = *coordinator.projected_drag_work_evidence().unwrap();
        assert!(recovered.accepted, "{recovered:#?}");
        assert!(recovered.continued);
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
        let mut preview = coordinator.session().clone();
        let request = preview
            .last_attempt()
            .input()
            .candidate_request()
            .without_previous_state_preferences()
            .with_drag(end, [0.0, 0.0]);
        preview
            .reattempt(preview.design_identity(), request)
            .expect("accepted preview");
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
            .mark_solved_preview(&preview)
            .expect("retain exact preview");

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
        let mut preview = session.clone();
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
        preview
            .apply(preview.design_identity(), edit.clone())
            .expect("retain rejected preview design");
        coordinator
            .apply_edit(coordinator.session().design_identity(), edit)
            .expect("retain rejected coordinator design");
        coordinator
            .reattempt(coordinator.session().design_identity())
            .expect("distinct persisted attempt");
        assert_eq!(
            preview.design_identity(),
            coordinator.session().design_identity()
        );
        assert_ne!(
            preview.last_attempt().identity(),
            coordinator.session().last_attempt().identity()
        );
        assert!(preview.last_attempt().accepted_state_identity().is_none());
        let lifecycle = coordinator.lifecycle();

        assert!(matches!(
            coordinator.mark_solved_preview(&preview),
            Err(CoordinatorError::PreviewNotAccepted)
        ));
        assert_eq!(coordinator.lifecycle(), lifecycle);
        assert!(coordinator.transient.is_none());
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

    fn operation_test_line(
        document: &mut SketchDocument,
        label: &str,
        start: [f64; 2],
        end: [f64; 2],
    ) -> (CurveSpan, [DesignPointId; 2]) {
        let points = [
            document
                .add_point(format!("{label} start"), start)
                .expect("line start"),
            document
                .add_point(format!("{label} end"), end)
                .expect("line end"),
        ];
        let delta = [end[0] - start[0], end[1] - start[1]];
        let length = delta[0].hypot(delta[1]);
        let curve = document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [delta[0] / length, delta[1] / length],
                },
            )
            .expect("line");
        (CurveSpan::line(curve), points)
    }

    fn operation_test_coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted operation fixture");
        RetainedEditorCoordinator::new(session).expect("operation coordinator")
    }

    fn simple_fillet_operation_fixture() -> (
        RetainedEditorCoordinator,
        [CurveSpan; 2],
        [DesignPointId; 4],
    ) {
        let mut document = SketchDocument::new(1.0).expect("document");
        let (horizontal, horizontal_points) =
            operation_test_line(&mut document, "horizontal parent", [-2.0, 0.0], [2.0, 0.0]);
        let (vertical, vertical_points) =
            operation_test_line(&mut document, "vertical parent", [0.0, -2.0], [0.0, 2.0]);
        (
            operation_test_coordinator(document),
            [horizontal, vertical],
            [
                horizontal_points[0],
                horizontal_points[1],
                vertical_points[0],
                vertical_points[1],
            ],
        )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the test helper preserves one complete normalized pointer-to-publication gesture"
    )]
    fn drag_operation_arc_body_once(
        coordinator: &mut RetainedEditorCoordinator,
        arc: CurveId,
        pointer_id: u64,
        screen_delta: [f64; 2],
    ) -> ([f64; 2], [f64; 2]) {
        assert_eq!(coordinator.editor().tool(), EditorTool::Select);
        let (center, center_before, scene, press) = {
            let accepted = coordinator
                .session()
                .accepted_state()
                .expect("accepted Fillet before gesture");
            let CurveDefinition::CircularArc { center, .. } = coordinator
                .session()
                .design_document()
                .curve(arc)
                .expect("Fillet arc before gesture")
                .definition
            else {
                panic!("Fillet output must be circular");
            };
            let center_before = accepted
                .document()
                .point(center)
                .expect("Fillet center before gesture")
                .position;
            let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
            let scene = EditorScene::from_accepted_for_design(
                accepted.identity().revision().get(),
                coordinator.session().design_identity(),
                accepted.document(),
                coordinator.session().design_document(),
                viewport,
                0.5,
            )
            .expect("Fillet gesture scene");
            let arc_curve = scene
                .curves
                .iter()
                .find(|curve| curve.span == CurveSpan::line(arc))
                .expect("visible Fillet gesture arc");
            assert_eq!(arc_curve.drag_handle_point, Some(center));
            let press = arc_curve.screen_polyline[arc_curve.screen_polyline.len() / 2];
            (center, center_before, scene, press)
        };
        let moved = ScreenPoint {
            x: press.x + screen_delta[0],
            y: press.y + screen_delta[1],
        };
        let pointer = |position| PointerInput {
            pointer_id,
            position,
            modifiers: Modifiers::default(),
        };
        let _ = coordinator.pointer_down(&scene, pointer(press));
        assert!(matches!(
            coordinator.editor().selection(),
            [SelectionItem::Curve(span)] if span.curve == arc
        ));
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("Fillet arc must route to its semantic center");
        };
        assert_eq!(*point, center);
        let preview = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *point,
            *model_position,
        );
        assert!(matches!(
            preview.as_slice(),
            [EditorEffect::PreviewPointMove { point, .. }] if *point == center
        ));
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("Fillet arc gesture work");
        assert!(work.accepted, "{work:#?}");
        assert_eq!(work.attempts, 1);
        assert_projected_drag_work_bounded(work);
        let expected = coordinator.session().design_identity();
        let release = coordinator
            .editor_mut()
            .pointer_up(&scene, expected, pointer(moved));
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitPointMove { point, .. },
                EditorEffect::ClearPointPreview,
            ] if *point == center
        ));
        let committed = coordinator
            .apply_editor_effect(&release[0])
            .expect("release Fillet body gesture")
            .expect("Fillet center mutation");
        assert!(committed.published_accepted.is_some());
        let center_after = coordinator
            .session()
            .accepted_state()
            .expect("accepted Fillet after gesture")
            .document()
            .point(center)
            .expect("Fillet center after gesture")
            .position;
        (center_before, center_after)
    }

    fn drag_operation_point_once(
        coordinator: &mut RetainedEditorCoordinator,
        point: DesignPointId,
        pointer_id: u64,
        screen_delta: [f64; 2],
    ) -> ([f64; 2], [f64; 2]) {
        assert_eq!(coordinator.editor().tool(), EditorTool::Select);
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted point before gesture");
        let before = accepted
            .document()
            .point(point)
            .expect("gesture point")
            .position;
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("point gesture scene");
        let press = scene
            .points
            .iter()
            .find(|candidate| candidate.id == point)
            .expect("visible gesture point")
            .screen_position;
        let moved = ScreenPoint {
            x: press.x + screen_delta[0],
            y: press.y + screen_delta[1],
        };
        let pointer = |position| PointerInput {
            pointer_id,
            position,
            modifiers: Modifiers::default(),
        };
        let _ = coordinator.pointer_down(&scene, pointer(press));
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point: requested,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("point must request one projected move")
        };
        assert_eq!(*requested, point);
        let preview = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *requested,
            *model_position,
        );
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("point gesture work");
        assert!(
            matches!(
                preview.as_slice(),
                [EditorEffect::PreviewPointMove { point: previewed, .. }] if *previewed == point
            ),
            "preview={preview:#?}; work={work:#?}"
        );
        assert!(work.accepted, "{work:#?}");
        assert_eq!(work.attempts, 1);
        assert_projected_drag_work_bounded(work);
        let expected = coordinator.session().design_identity();
        let release = coordinator
            .editor_mut()
            .pointer_up(&scene, expected, pointer(moved));
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitPointMove { point: committed, .. },
                EditorEffect::ClearPointPreview,
            ] if *committed == point
        ));
        let mutation = coordinator
            .apply_editor_effect(&release[0])
            .expect("release point gesture")
            .expect("point mutation");
        assert!(mutation.published_accepted.is_some());
        let after = coordinator
            .session()
            .accepted_state()
            .expect("accepted point after gesture")
            .document()
            .point(point)
            .expect("moved gesture point")
            .position;
        (before, after)
    }

    fn staged_fillet(
        coordinator: &RetainedEditorCoordinator,
        parents: [CurveSpan; 2],
        parameters: [f64; 2],
        radius: f64,
        confirmed: bool,
    ) -> OperationAuthoringCandidate {
        let document = coordinator
            .operation_authoring_document()
            .expect("current accepted operation document")
            .clone();
        let picks = [
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(parents[0]), Some(parameters[0]))
                .expect("first stamped fillet pick"),
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(parents[1]), Some(parameters[1]))
                .expect("second stamped fillet pick"),
        ];
        let mut state = OperationAuthoringState::default();
        let _ = state.set_options(
            &document,
            OperationAuthoringOptions {
                fillet_radius: Some(radius),
                fillet_radius_mode: DocumentDimensionMode::Driving,
                ..OperationAuthoringOptions::default()
            },
        );
        let outcome = state.activate(&document, OperationAuthoringTool::Fillet, &picks);
        assert!(matches!(
            outcome,
            OperationAuthoringOutcome::PreviewRequested { .. }
        ));
        if !confirmed {
            let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = outcome else {
                unreachable!("preview outcome checked above");
            };
            assert!(!candidate.is_confirmed());
            return candidate;
        }
        let jets = picks.each_ref().map(|pick| {
            document
                .evaluate_curve_jet(pick.curve_span().expect("curve pick"), pick.curve_parameter)
                .expect("accepted operation pick jet")
        });
        let first_direction = jets[0].first_derivative;
        let second_direction = jets[1].first_derivative;
        let denominator =
            first_direction.x * second_direction.y - first_direction.y * second_direction.x;
        let between = jets[1].position - jets[0].position;
        let first_parameter =
            (between.x * second_direction.y - between.y * second_direction.x) / denominator;
        let tangent_intersection = [
            jets[0].position.x + first_parameter * first_direction.x,
            jets[0].position.y + first_parameter * first_direction.y,
        ];
        let outcome = state.confirm(&document, tangent_intersection);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = outcome else {
            panic!("expected fillet candidate: {outcome:?}");
        };
        assert!(candidate.is_confirmed());
        candidate
    }

    #[test]
    fn successful_point_edit_keeps_current_geometry_available_to_fillet_authoring() {
        let (mut coordinator, parents, points) = simple_fillet_operation_fixture();
        let edit = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: points[0],
                    position: [-2.5, 0.25],
                },
            )
            .expect("accepted point edit");
        assert!(edit.published_accepted.is_some());

        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("point-edit publication remains current");
        assert_ne!(
            accepted.input().candidate_request(),
            coordinator
                .session()
                .prepared_input()
                .attempt_input()
                .candidate_request(),
            "the command drag is intentionally absent from retained request state"
        );
        assert!(coordinator.operation_authoring_document().is_some());
        assert!(
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(parents[0]), Some(0.25))
                .is_ok(),
            "a successful drag must not disable later Fillet picks"
        );
    }

    fn commit_operation_and_assert_lifecycle(
        coordinator: &mut RetainedEditorCoordinator,
        candidate: &OperationAuthoringCandidate,
    ) -> OperationAuthoringPreviewMetadata {
        let initial_session = coordinator.session().clone();
        let original = coordinator.session().design_document().clone();
        let outcome = coordinator
            .prepare_operation_preview(candidate)
            .expect("operation preview preparation");
        let OperationAuthoringPreviewOutcome::Ready(metadata) = outcome else {
            panic!("expected accepted operation preview: {outcome:?}");
        };
        let scratch_accepted = coordinator
            .operation_preview()
            .expect("held accepted preview")
            .accepted_document()
            .clone();
        let history = coordinator.history_len();
        let mutation = coordinator
            .apply_operation_preview(metadata.token, candidate)
            .expect("exact operation commit");
        assert_eq!(coordinator.history_len(), history + 1);
        assert_eq!(
            mutation.primary_created_curve,
            metadata.primary_created_curve
        );
        assert!(matches!(
            coordinator.editor().selection(),
            [SelectionItem::Curve(span)] if span.curve == metadata.primary_created_curve
        ));
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("live accepted operation")
                .document(),
            &scratch_accepted
        );

        let committed = coordinator.session().design_document().clone();
        let action = coordinator
            .transcript()
            .last()
            .expect("operation replay action")
            .clone();
        coordinator.undo().expect("undo operation");
        assert_eq!(coordinator.session().design_document(), &original);
        coordinator.redo().expect("redo operation");
        assert_eq!(coordinator.session().design_document(), &committed);
        let mut replay = RetainedEditorCoordinator::new(initial_session).expect("replay owner");
        replay.replay(&action).expect("operation replay");
        assert_eq!(replay.session().design_document(), &committed);
        metadata
    }

    #[derive(Clone, Copy)]
    enum FilletParentEdit {
        UnlockPoint {
            constraint: geosolve_sketch::DocumentConstraintId,
            point: DesignPointId,
            position: [f64; 2],
        },
        Scalar {
            scalar: geosolve_sketch::DesignScalarId,
            value: f64,
        },
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the two grounded curved-parent fixtures keep their ordinary editable constraints visible beside the geometry"
    )]
    fn fillet_operation_fixture(
        line_circle: bool,
    ) -> (
        RetainedEditorCoordinator,
        [CurveSpan; 2],
        [f64; 2],
        f64,
        FilletParentEdit,
    ) {
        let mut document = SketchDocument::new(8.0).expect("document");
        if line_circle {
            let (line, line_points) =
                operation_test_line(&mut document, "linear parent", [2.0, 1.0], [10.0, 1.0]);
            for (index, (point, target)) in line_points
                .into_iter()
                .zip([[2.0, 1.0], [10.0, 1.0]])
                .enumerate()
            {
                document
                    .add_constraint(
                        format!("fixed line point {index}"),
                        DocumentConstraintDefinition::FixedPoint { point, target },
                    )
                    .expect("fixed line point");
            }
            let center = document
                .add_point("circle center", [6.0, 4.0])
                .expect("circle center");
            document
                .add_constraint(
                    "fixed circle center",
                    DocumentConstraintDefinition::FixedPoint {
                        point: center,
                        target: [6.0, 4.0],
                    },
                )
                .expect("fixed circle center");
            let radius = document
                .add_scalar(
                    "circle radius",
                    2.0,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .expect("circle radius");
            let circle = document
                .add_curve(
                    "circular parent",
                    CurveDefinition::Circle { center, radius },
                )
                .expect("circle");
            let source_radius_target = document
                .add_scalar(
                    "source circle radius target",
                    2.0,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .expect("source radius target");
            document
                .add_dimension(
                    "source circle radius",
                    DocumentDimensionDefinition::Radius {
                        curve: circle,
                        target: source_radius_target,
                    },
                    DocumentDimensionMode::Driving,
                )
                .expect("source radius dimension");
            (
                operation_test_coordinator(document),
                [line, CurveSpan::line(circle)],
                [0.28, 4.05],
                0.8,
                FilletParentEdit::Scalar {
                    scalar: source_radius_target,
                    value: 2.1,
                },
            )
        } else {
            let (line, line_points) =
                operation_test_line(&mut document, "linear parent", [6.0, -8.0], [6.0, 0.0]);
            for (index, (point, target)) in line_points
                .into_iter()
                .zip([[6.0, -8.0], [6.0, 0.0]])
                .enumerate()
            {
                document
                    .add_constraint(
                        format!("fixed line point {index}"),
                        DocumentConstraintDefinition::FixedPoint { point, target },
                    )
                    .expect("fixed line point");
            }
            let controls = [[1.0, -3.0], [4.0, -7.0], [8.0, -3.0]].map(|position| {
                document
                    .add_point("Bezier control", position)
                    .expect("control")
            });
            for (index, (point, target)) in [(controls[0], [1.0, -3.0]), (controls[2], [8.0, -3.0])]
                .into_iter()
                .enumerate()
            {
                document
                    .add_constraint(
                        format!("fixed Bezier endpoint {index}"),
                        DocumentConstraintDefinition::FixedPoint { point, target },
                    )
                    .expect("fixed Bezier endpoint");
            }
            let middle_lock = document
                .add_constraint(
                    "fixed Bezier edit handle",
                    DocumentConstraintDefinition::FixedPoint {
                        point: controls[1],
                        target: [4.0, -7.0],
                    },
                )
                .expect("fixed Bezier edit handle");
            let bezier = document
                .add_curve(
                    "Bezier parent",
                    CurveDefinition::QuadraticBezier { controls },
                )
                .expect("Bezier");
            (
                operation_test_coordinator(document),
                [line, CurveSpan::line(bezier)],
                [0.44, 0.74],
                0.8,
                FilletParentEdit::UnlockPoint {
                    constraint: middle_lock,
                    point: controls[1],
                    position: [3.8, -7.2],
                },
            )
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "two representative curved-parent families share one complete lifecycle matrix"
    )]
    fn curved_parent_fillet_previews_commit_exactly_and_remain_associative_after_edits() {
        for line_circle in [true, false] {
            let (mut coordinator, parents, parameters, radius, parent_edit) =
                fillet_operation_fixture(line_circle);
            let candidate = staged_fillet(&coordinator, parents, parameters, radius, true);
            let metadata = commit_operation_and_assert_lifecycle(&mut coordinator, &candidate);
            assert_eq!(metadata.tool, OperationAuthoringTool::Fillet);
            let fillet = metadata.primary_created_curve;
            let (fillet_center, fillet_radius) = match coordinator
                .session()
                .design_document()
                .curve(fillet)
                .expect("fillet arc")
                .definition
            {
                CurveDefinition::CircularArc { center, radius, .. } => (center, radius),
                ref other => panic!("fillet primary must be a circular arc: {other:?}"),
            };
            let radius_target = coordinator
                .session()
                .design_document()
                .dimensions()
                .iter()
                .find_map(|dimension| match dimension.definition {
                    DocumentDimensionDefinition::Radius { curve, target } if curve == fillet => {
                        Some(target)
                    }
                    _ => None,
                })
                .expect("fillet driving-radius target");
            assert!(
                coordinator
                    .session()
                    .design_document()
                    .constraints()
                    .iter()
                    .any(|constraint| matches!(
                        constraint.definition,
                        DocumentConstraintDefinition::CurveCurveFillet { arc, .. } if arc == fillet
                    ))
            );
            assert!(parents.iter().all(|parent| {
                coordinator
                    .session()
                    .design_document()
                    .trim_views_for_span(*parent)
                    .next()
                    .is_some()
            }));

            let edited_radius = radius * 1.2;
            let radius_edit = coordinator
                .apply_edit(
                    coordinator.session().design_identity(),
                    DocumentEdit::SetScalarValue {
                        scalar: radius_target,
                        value: edited_radius,
                    },
                )
                .expect("accepted fillet-radius edit");
            assert!(
                radius_edit.published_accepted.is_some(),
                "fillet radius edit rejected: mutation={radius_edit:?}, attempt={:?}",
                coordinator.session().last_attempt()
            );
            let accepted = coordinator
                .session()
                .accepted_state()
                .expect("accepted radius edit")
                .document();
            assert!(
                (accepted.scalar(fillet_radius).expect("fillet radius").value - edited_radius)
                    .abs()
                    <= 1.0e-7
            );
            let center_before = accepted
                .point(fillet_center)
                .expect("fillet center")
                .position;

            let edit = match parent_edit {
                FilletParentEdit::UnlockPoint {
                    constraint,
                    point,
                    position,
                } => {
                    coordinator
                        .editor_mut()
                        .set_selection([SelectionItem::Constraint(constraint)]);
                    let unlocked = coordinator
                        .set_selected_suppressed(coordinator.session().design_identity(), true)
                        .expect("suppress editable parent lock");
                    assert!(unlocked.published_accepted.is_some());
                    DocumentEdit::SetPointPosition { point, position }
                }
                FilletParentEdit::Scalar { scalar, value } => {
                    DocumentEdit::SetScalarValue { scalar, value }
                }
            };
            let parent_edit = coordinator
                .apply_edit(coordinator.session().design_identity(), edit)
                .expect("accepted fillet-parent edit");
            assert!(parent_edit.published_accepted.is_some());
            let accepted = coordinator
                .session()
                .accepted_state()
                .expect("accepted parent edit")
                .document();
            let center_after = accepted
                .point(fillet_center)
                .expect("associated fillet center")
                .position;
            assert!(
                (center_after[0] - center_before[0]).hypot(center_after[1] - center_before[1])
                    > 1.0e-6,
                "fillet center did not respond to its parent edit"
            );
            assert!(accepted.constraints().iter().any(|constraint| matches!(
                constraint.definition,
                DocumentConstraintDefinition::CurveCurveFillet { arc, .. } if arc == fillet
            )));
        }
    }

    #[test]
    fn operation_preview_is_independently_accepted_scratch_and_leaves_live_state_unchanged() {
        let (mut coordinator, parents, _) = simple_fillet_operation_fixture();
        let candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let before = retained_state_snapshot(&coordinator);
        let ready = coordinator
            .prepare_operation_preview(&candidate)
            .expect("preview preparation");
        let OperationAuthoringPreviewOutcome::Ready(metadata) = ready else {
            panic!("expected accepted preview: {ready:?}");
        };
        assert!(metadata.apply_ready);
        let preview = coordinator.operation_preview().expect("held preview");
        assert!(preview.matches_candidate(&candidate));
        assert_eq!(preview.accepted_state().identity(), metadata.accepted);
        assert_eq!(
            preview
                .scene(
                    Viewport::new([800.0, 600.0], [0.0, 0.0], 50.0).unwrap(),
                    0.5,
                )
                .unwrap()
                .design_identity,
            preview.accepted_state().design_identity()
        );
        assert!(
            preview
                .accepted_document()
                .curve(metadata.primary_created_curve)
                .is_some()
        );
        assert_retained_state_snapshot(&coordinator, &before);
    }

    #[test]
    fn operation_preview_requires_exact_token_candidate_and_confirmation() {
        let (mut coordinator, parents, _) = simple_fillet_operation_fixture();
        let unconfirmed = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.2, false);
        let OperationAuthoringPreviewOutcome::Ready(unconfirmed_metadata) = coordinator
            .prepare_operation_preview(&unconfirmed)
            .expect("unconfirmed preview")
        else {
            panic!("unconfirmed scratch should still be accepted");
        };
        assert!(!unconfirmed_metadata.apply_ready);
        assert!(matches!(
            coordinator.apply_operation_preview(unconfirmed_metadata.token, &unconfirmed),
            Err(CoordinatorError::OperationPreviewNotConfirmed)
        ));
        assert!(coordinator.operation_preview().is_some());

        let first = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.2, true);
        let OperationAuthoringPreviewOutcome::Ready(first_metadata) = coordinator
            .prepare_operation_preview(&first)
            .expect("first preview")
        else {
            panic!("first ready preview");
        };
        let second = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.4, true);
        assert!(matches!(
            coordinator.apply_operation_preview(first_metadata.token, &second),
            Err(CoordinatorError::OperationPreviewMismatch)
        ));
        assert!(coordinator.operation_preview().is_none());

        let OperationAuthoringPreviewOutcome::Ready(replaced_metadata) = coordinator
            .prepare_operation_preview(&first)
            .expect("replacement first")
        else {
            panic!("replacement first ready");
        };
        let OperationAuthoringPreviewOutcome::Ready(latest_metadata) = coordinator
            .prepare_operation_preview(&second)
            .expect("latest preview")
        else {
            panic!("latest ready");
        };
        assert!(latest_metadata.token > replaced_metadata.token);
        assert!(matches!(
            coordinator.apply_operation_preview(replaced_metadata.token, &first),
            Err(CoordinatorError::OperationPreviewMismatch)
        ));
        assert!(
            coordinator
                .operation_preview()
                .is_some_and(|preview| preview.matches_candidate(&second))
        );
    }

    #[test]
    fn cancelled_or_exhausted_operation_preparation_is_mutation_free_and_holds_no_preview() {
        let (mut coordinator, parents, _) = simple_fillet_operation_fixture();
        let candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let before = retained_state_snapshot(&coordinator);

        let (handle, token) = cancellation_pair();
        handle.cancel();
        let cancelled = coordinator
            .prepare_operation_preview_controlled(
                &candidate,
                OperationControl::new(token, bounded_geometry_control().limits),
            )
            .expect("cancelled preparation outcome");
        assert!(matches!(
            cancelled,
            OperationAuthoringPreviewOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::WorkStopped,
                ..
            })
        ));
        assert!(coordinator.operation_preview().is_none());
        assert_retained_state_snapshot(&coordinator, &before);

        let mut limits = bounded_geometry_control().limits;
        limits.document_validation_items = 1;
        let exhausted = coordinator
            .prepare_operation_preview_controlled(
                &candidate,
                OperationControl::new(geosolve_sketch::CancellationToken::default(), limits),
            )
            .expect("scratch exhaustion outcome");
        assert!(matches!(
            exhausted,
            OperationAuthoringPreviewOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::WorkStopped,
                ..
            })
        ));
        assert!(coordinator.operation_preview().is_none());
        assert_retained_state_snapshot(&coordinator, &before);
    }

    #[test]
    fn cancelled_or_exhausted_operation_commit_after_preview_is_mutation_free() {
        let (mut coordinator, parents, _) = simple_fillet_operation_fixture();

        let cancelled_candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let OperationAuthoringPreviewOutcome::Ready(cancelled_metadata) = coordinator
            .prepare_operation_preview(&cancelled_candidate)
            .expect("accepted preview before cancellation")
        else {
            panic!("accepted preview before cancellation");
        };
        let cancelled_before = retained_state_snapshot(&coordinator);
        let (handle, token) = cancellation_pair();
        handle.cancel();
        assert!(matches!(
            coordinator.apply_operation_preview_controlled(
                cancelled_metadata.token,
                &cancelled_candidate,
                OperationControl::new(token, bounded_geometry_control().limits),
            ),
            Err(CoordinatorError::OperationWorkStopped)
        ));
        assert!(coordinator.operation_preview().is_none());
        assert_retained_state_snapshot(&coordinator, &cancelled_before);

        let exhausted_candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.3, true);
        let OperationAuthoringPreviewOutcome::Ready(exhausted_metadata) = coordinator
            .prepare_operation_preview(&exhausted_candidate)
            .expect("accepted preview before exhaustion")
        else {
            panic!("accepted preview before exhaustion");
        };
        let exhausted_before = retained_state_snapshot(&coordinator);
        let mut limits = bounded_geometry_control().limits;
        limits.document_validation_items = 0;
        assert!(matches!(
            coordinator.apply_operation_preview_controlled(
                exhausted_metadata.token,
                &exhausted_candidate,
                OperationControl::new(geosolve_sketch::CancellationToken::default(), limits),
            ),
            Err(CoordinatorError::OperationWorkStopped)
        ));
        assert!(coordinator.operation_preview().is_none());
        assert_retained_state_snapshot(&coordinator, &exhausted_before);
    }

    #[test]
    fn stale_candidate_after_reattempt_or_undo_cannot_prepare() {
        let (mut coordinator, parents, points) = simple_fillet_operation_fixture();
        let stale_after_attempt = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        coordinator
            .reattempt(coordinator.session().design_identity())
            .expect("reattempt");
        assert!(matches!(
            coordinator
                .prepare_operation_preview(&stale_after_attempt)
                .expect("typed stale result"),
            OperationAuthoringPreviewOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::StalePick,
                ..
            })
        ));

        let stale_after_undo = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.3, true);
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: points[0],
                    position: [-3.0, 0.5],
                },
            )
            .expect("intervening edit");
        coordinator.undo().expect("undo intervening edit");
        assert!(matches!(
            coordinator
                .prepare_operation_preview(&stale_after_undo)
                .expect("typed post-undo stale result"),
            OperationAuthoringPreviewOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::StalePick,
                ..
            })
        ));
        assert!(coordinator.operation_preview().is_none());
    }

    #[test]
    fn older_accepted_geometry_is_not_exposed_after_same_design_attempt_failure() {
        let (mut coordinator, parents, _) = simple_fillet_operation_fixture();
        let candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let design = coordinator.session().design_identity();
        let missing = DesignPointId(PersistentId::from_u128(u128::MAX));
        let request = coordinator
            .session()
            .last_attempt()
            .input()
            .candidate_request()
            .with_drag(missing, [0.0, 0.0]);
        let attempt = coordinator
            .session
            .reattempt(design, request)
            .expect("retained failed attempt");
        assert!(attempt.failure().is_some() || attempt.accepted_state_identity().is_none());
        assert_eq!(coordinator.session().design_identity(), design);
        assert!(coordinator.session().accepted_state().is_some());
        assert!(coordinator.operation_authoring_document().is_none());
        assert!(coordinator.operation_authoring_input().is_none());
        assert!(matches!(
            coordinator
                .prepare_operation_preview(&candidate)
                .expect("typed stale preview outcome"),
            OperationAuthoringPreviewOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::StalePick,
                ..
            })
        ));
        assert!(coordinator.operation_preview().is_none());
    }

    #[test]
    fn operation_commit_is_one_history_step_selects_primary_and_round_trips_undo_redo_replay() {
        let (mut coordinator, parents, _) = simple_fillet_operation_fixture();
        let initial_session = coordinator.session().clone();
        let original = coordinator.session().design_document().clone();
        let candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let OperationAuthoringPreviewOutcome::Ready(metadata) = coordinator
            .prepare_operation_preview(&candidate)
            .expect("preview")
        else {
            panic!("ready preview");
        };
        let history = coordinator.history_len();
        let mutation = coordinator
            .apply_operation_preview(metadata.token, &candidate)
            .expect("operation commit");
        assert_eq!(coordinator.history_len(), history + 1);
        assert_eq!(
            mutation.primary_created_curve,
            metadata.primary_created_curve
        );
        assert!(matches!(
            coordinator.editor().selection(),
            [SelectionItem::Curve(span)] if span.curve == metadata.primary_created_curve
        ));
        let committed = coordinator.session().design_document().clone();
        let action = coordinator
            .transcript()
            .last()
            .expect("replay action")
            .clone();
        coordinator.undo().expect("undo operation");
        assert_eq!(coordinator.session().design_document(), &original);
        coordinator.redo().expect("redo operation");
        assert_eq!(coordinator.session().design_document(), &committed);

        let mut replay = RetainedEditorCoordinator::new(initial_session).expect("replay owner");
        replay.replay(&action).expect("operation replay");
        assert_eq!(replay.session().design_document(), &committed);
    }

    #[test]
    fn exact_commit_fails_closed_if_held_scratch_no_longer_matches_rendered_preview() {
        let (mut coordinator, parents, points) = simple_fillet_operation_fixture();
        let candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let before = retained_state_snapshot(&coordinator);
        let OperationAuthoringPreviewOutcome::Ready(metadata) = coordinator
            .prepare_operation_preview(&candidate)
            .expect("preview")
        else {
            panic!("ready preview");
        };
        let preview = coordinator
            .operation_preview
            .as_mut()
            .expect("held private preview");
        preview
            .scratch
            .apply(
                preview.scratch.design_identity(),
                DocumentEdit::SetPointPosition {
                    point: points[1],
                    position: [3.0, 0.5],
                },
            )
            .expect("simulate divergent scratch");
        assert!(matches!(
            coordinator.apply_operation_preview(metadata.token, &candidate),
            Err(CoordinatorError::OperationPreviewMismatch)
        ));
        assert_retained_state_snapshot(&coordinator, &before);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one curved-parent release proves substantial motion without changing the authored contact branch"
    )]
    fn authored_line_circle_fillet_remains_on_its_contact_branch_after_radius_release() {
        let (mut coordinator, parents, parameters, authored_radius, _) =
            fillet_operation_fixture(true);
        let candidate = staged_fillet(&coordinator, parents, parameters, authored_radius, true);
        let metadata = commit_operation_and_assert_lifecycle(&mut coordinator, &candidate);
        let arc = metadata.primary_created_curve;

        let (center, radius, radius_dimension, association, contacts, trims_before) = {
            let design = coordinator.session().design_document();
            let CurveDefinition::CircularArc { center, radius, .. } =
                design.curve(arc).expect("authored Fillet arc").definition
            else {
                panic!("primary Fillet result must be a circular arc");
            };
            let radius_dimension = design
                .dimensions()
                .iter()
                .find_map(|dimension| match dimension.definition {
                    DocumentDimensionDefinition::Radius { curve, .. } if curve == arc => {
                        Some(dimension.id)
                    }
                    _ => None,
                })
                .expect("authored radius dimension");
            let association = design
                .curve_curve_fillet_for_arc(arc)
                .expect("generic Fillet association")
                .definition
                .clone();
            let DocumentConstraintDefinition::CurveCurveFillet {
                first_contact,
                second_contact,
                ..
            } = association
            else {
                unreachable!("generic Fillet association checked above");
            };
            let contacts = [first_contact, second_contact];
            assert_eq!(
                design.contact(first_contact).expect("line contact").curve,
                parents[0]
            );
            assert_eq!(
                design
                    .contact(second_contact)
                    .expect("circle contact")
                    .curve,
                parents[1]
            );
            assert_eq!(
                design
                    .contact(first_contact)
                    .expect("line contact")
                    .neighborhood,
                ContactNeighborhood::Interior,
                "bounded authoring must retain the complete semantic support"
            );
            let circle_neighborhood = design
                .contact(second_contact)
                .expect("circle contact")
                .neighborhood;
            let ContactNeighborhood::Local { lower, upper } = circle_neighborhood else {
                panic!("periodic authoring must retain an explicit local root interval");
            };
            assert!(upper - lower < std::f64::consts::TAU);
            let trims_before = parents.map(|parent| {
                design
                    .trim_views_for_span(parent)
                    .copied()
                    .collect::<Vec<_>>()
            });
            (
                center,
                radius,
                radius_dimension,
                association,
                contacts,
                trims_before,
            )
        };

        let deletion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(radius_dimension),
                },
            )
            .expect("delete only the Fillet radius dimension");
        assert!(deletion.published_accepted.is_some());
        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted free curved-parent Fillet");
        let document_before = accepted_before.document();
        let center_before = document_before
            .point(center)
            .expect("Fillet center")
            .position;
        let contact_state_before = contacts.map(|contact| {
            let slot = document_before.contact(contact).expect("Fillet contact");
            (
                slot.curve,
                slot.parameter,
                slot.domain,
                slot.winding,
                slot.neighborhood,
                slot.tangent_orientation,
                document_before
                    .scalar(slot.parameter)
                    .expect("contact parameter")
                    .value,
            )
        });

        // On this retained branch the center is above the horizontal line and outside the
        // circle.  A radius of 2 therefore has y = 1 + 2 and
        // `(x - 6)^2 + (y - 4)^2 = (2 + 2)^2`; the left-hand root preserves the authored pick.
        let released_radius = 2.0;
        let target = [6.0 - 15.0_f64.sqrt(), 1.0 + released_radius];
        let movement = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: center,
                    position: target,
                },
            )
            .expect("substantially move the free curved-parent Fillet");
        assert!(
            movement.published_accepted.is_some(),
            "large compatible Fillet move must publish: attempt={:#?}",
            coordinator.session().last_attempt()
        );

        let accepted_after = coordinator
            .session()
            .accepted_state()
            .expect("accepted large curved-parent Fillet move");
        let document_after = accepted_after.document();
        let center_after = document_after
            .point(center)
            .expect("moved Fillet center")
            .position;
        let radius_after = document_after
            .scalar(radius)
            .expect("released Fillet radius")
            .value;
        assert!(
            (center_after[0] - center_before[0]).hypot(center_after[1] - center_before[1]) > 1.5,
            "the regression must exercise a substantial curved-parent move"
        );
        assert!((center_after[0] - target[0]).abs() <= 1.0e-7);
        assert!((center_after[1] - target[1]).abs() <= 1.0e-7);
        assert!((radius_after - released_radius).abs() <= 1.0e-7);
        assert!(document_after.dimension(radius_dimension).is_none());
        assert_eq!(
            document_after
                .curve_curve_fillet_for_arc(arc)
                .expect("retained Fillet association")
                .definition,
            association,
            "a free drag must not silently rewrite side, endpoint, or arc-order branch state"
        );
        assert_eq!(
            parents.map(|parent| {
                document_after
                    .trim_views_for_span(parent)
                    .copied()
                    .collect::<Vec<_>>()
            }),
            trims_before,
            "the contact-owned visible-interval topology must remain stable"
        );

        let DocumentConstraintDefinition::CurveCurveFillet {
            first_side,
            second_side,
            ..
        } = association
        else {
            unreachable!("generic Fillet association checked above");
        };
        let sides = [first_side, second_side];
        let contact_state_after = contacts.map(|contact| {
            let slot = document_after
                .contact(contact)
                .expect("retained Fillet contact");
            (
                slot.curve,
                slot.parameter,
                slot.domain,
                slot.winding,
                slot.neighborhood,
                slot.tangent_orientation,
                document_after
                    .scalar(slot.parameter)
                    .expect("moved contact parameter")
                    .value,
            )
        });
        for (index, ((before, after), side)) in contact_state_before
            .into_iter()
            .zip(contact_state_after)
            .zip(sides)
            .enumerate()
        {
            assert_eq!(after.0, before.0);
            assert_eq!(after.1, before.1);
            assert_eq!(after.2, before.2);
            assert_eq!(after.3, before.3, "contact {index} changed winding");
            assert_eq!(after.4, before.4, "contact {index} changed neighborhood");
            assert_eq!(after.5, before.5);
            let parameter_delta = after.6 - before.6;
            assert!(
                parameter_delta.abs() > 0.2,
                "contact {index} must move materially in this regression"
            );
            assert!(
                parameter_delta.abs() < std::f64::consts::PI,
                "contact {index} jumped to a remote root"
            );

            let jet = document_after
                .evaluate_contact_jet(contacts[index])
                .expect("moved contact jet");
            let differential = jet.differential().expect("regular moved contact");
            let center_offset = [
                center_after[0] - jet.position.x,
                center_after[1] - jet.position.y,
            ];
            let signed_normal_offset = center_offset[0] * differential.left_normal.x
                + center_offset[1] * differential.left_normal.y;
            let expected_sign = match side {
                geosolve_sketch::DocumentCurveNormalSide::Left => 1.0,
                geosolve_sketch::DocumentCurveNormalSide::Right => -1.0,
            };
            assert!(
                expected_sign * signed_normal_offset > 0.99 * radius_after,
                "contact {index} crossed its explicitly retained normal side"
            );
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one authored-feature lifecycle proves removal of the former hidden contact window"
    )]
    fn authored_fillet_moves_past_the_old_quarter_span_after_radius_dimension_deletion() {
        let mut document = SketchDocument::new(2.0).expect("document");
        let (horizontal, horizontal_points) =
            operation_test_line(&mut document, "horizontal parent", [-2.0, 0.0], [2.0, 0.0]);
        let (vertical, vertical_points) =
            operation_test_line(&mut document, "vertical parent", [0.0, -2.0], [0.0, 2.0]);
        for (index, (point, target)) in horizontal_points
            .into_iter()
            .zip([[-2.0, 0.0], [2.0, 0.0]])
            .chain(vertical_points.into_iter().zip([[0.0, -2.0], [0.0, 2.0]]))
            .enumerate()
        {
            document
                .add_constraint(
                    format!("fixed fillet parent {index}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("fixed parent point");
        }
        let mut coordinator = operation_test_coordinator(document);
        let parents = [horizontal, vertical];
        let candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let metadata = commit_operation_and_assert_lifecycle(&mut coordinator, &candidate);
        let arc = metadata.primary_created_curve;

        let (center, radius, radius_dimension, contacts) = {
            let design = coordinator.session().design_document();
            let CurveDefinition::CircularArc { center, radius, .. } =
                design.curve(arc).expect("fillet arc").definition
            else {
                panic!("primary Fillet result must be a circular arc");
            };
            let radius_dimension = design
                .dimensions()
                .iter()
                .find_map(|dimension| match dimension.definition {
                    DocumentDimensionDefinition::Radius { curve, .. } if curve == arc => {
                        Some(dimension.id)
                    }
                    _ => None,
                })
                .expect("authored driving radius dimension");
            let contacts = design
                .constraints()
                .iter()
                .find_map(|constraint| match constraint.definition {
                    DocumentConstraintDefinition::CurveCurveFillet {
                        arc: candidate_arc,
                        first_contact,
                        second_contact,
                        ..
                    } if candidate_arc == arc => Some([first_contact, second_contact]),
                    _ => None,
                })
                .expect("authored Fillet association");
            for contact in contacts {
                assert_eq!(
                    design
                        .contact(contact)
                        .expect("fillet contact")
                        .neighborhood,
                    ContactNeighborhood::Interior,
                    "bounded authored parents must persist semantic support, not a search window"
                );
            }
            (center, radius, radius_dimension, contacts)
        };

        let deletion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(radius_dimension),
                },
            )
            .expect("delete Fillet radius dimension");
        assert!(deletion.published_accepted.is_some());
        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted flexible Fillet");
        let center_before = accepted_before
            .document()
            .point(center)
            .expect("fillet center")
            .position;
        let radius_before = accepted_before
            .document()
            .scalar(radius)
            .expect("fillet radius")
            .value;
        let parameters_before = contacts.map(|contact| {
            let contact = accepted_before
                .document()
                .contact(contact)
                .expect("contact");
            accepted_before
                .document()
                .scalar(contact.parameter)
                .expect("contact parameter")
                .value
        });
        let target = [6.0 * center_before[0], 6.0 * center_before[1]];
        let movement = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: center,
                    position: target,
                },
            )
            .expect("move flexible Fillet beyond its former local window");
        assert!(movement.published_accepted.is_some());

        let accepted_after = coordinator
            .session()
            .accepted_state()
            .expect("accepted large Fillet move");
        let center_after = accepted_after
            .document()
            .point(center)
            .expect("moved center")
            .position;
        let radius_after = accepted_after
            .document()
            .scalar(radius)
            .expect("moved radius")
            .value;
        let parameters_after = contacts.map(|contact| {
            let contact = accepted_after.document().contact(contact).expect("contact");
            assert_eq!(contact.neighborhood, ContactNeighborhood::Interior);
            accepted_after
                .document()
                .scalar(contact.parameter)
                .expect("contact parameter")
                .value
        });
        assert!(
            (center_after[0] - center_before[0]).hypot(center_after[1] - center_before[1]) > 1.0,
            "the center must move materially beyond the old quarter-span envelope"
        );
        assert!(radius_after > radius_before + 1.0);
        assert!(
            parameters_after
                .into_iter()
                .zip(parameters_before)
                .all(|(after, before)| (after - before).abs() > 0.25),
            "both contacts must cross the former ±quarter-span hard bounds"
        );
        assert!(
            accepted_after
                .document()
                .dimension(radius_dimension)
                .is_none()
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one positive multi-sample gesture proves free Fillet travel through the ordinary pointer lifecycle"
    )]
    fn authored_free_line_line_fillet_center_gesture_crosses_old_contact_window_and_commits() {
        let (mut coordinator, parents, points) = simple_fillet_operation_fixture();
        for (index, (point, target)) in points
            .into_iter()
            .zip([[-2.0, 0.0], [2.0, 0.0], [0.0, -2.0], [0.0, 2.0]])
            .enumerate()
        {
            let fixed = coordinator
                .apply_edit(
                    coordinator.session().design_identity(),
                    DocumentEdit::CreateConstraint {
                        label: format!("fixed Fillet parent point {index}"),
                        definition: DocumentConstraintDefinition::FixedPoint { point, target },
                    },
                )
                .expect("fix Fillet parent point");
            assert!(fixed.published_accepted.is_some());
        }

        let candidate = staged_fillet(&coordinator, parents, [0.25, 0.75], 0.25, true);
        let metadata = commit_operation_and_assert_lifecycle(&mut coordinator, &candidate);
        let arc = metadata.primary_created_curve;
        let (center, radius_dimension, association, contacts) = {
            let document = coordinator.session().design_document();
            let CurveDefinition::CircularArc { center, .. } =
                document.curve(arc).expect("authored Fillet arc").definition
            else {
                panic!("primary Fillet result must be a circular arc");
            };
            let radius_dimension = document
                .dimensions()
                .iter()
                .find_map(|dimension| match dimension.definition {
                    DocumentDimensionDefinition::Radius { curve, .. } if curve == arc => {
                        Some(dimension.id)
                    }
                    _ => None,
                })
                .expect("authored Fillet radius dimension");
            let association = document
                .curve_curve_fillet_for_arc(arc)
                .expect("authored Fillet association")
                .definition
                .clone();
            let contacts = match &association {
                DocumentConstraintDefinition::CurveCurveFillet {
                    first_contact,
                    second_contact,
                    ..
                } => [*first_contact, *second_contact],
                _ => unreachable!("generic Fillet association checked above"),
            };
            (center, radius_dimension, association, contacts)
        };

        let deletion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(radius_dimension),
                },
            )
            .expect("delete the Fillet radius dimension");
        assert!(deletion.published_accepted.is_some());
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted free Fillet");
        let initial_center = accepted
            .document()
            .point(center)
            .expect("Fillet center")
            .position;
        let parameters_before = contacts.map(|contact| {
            let contact = accepted
                .document()
                .contact(contact)
                .expect("Fillet contact");
            assert_eq!(contact.neighborhood, ContactNeighborhood::Interior);
            accepted
                .document()
                .scalar(contact.parameter)
                .expect("Fillet contact parameter")
                .value
        });

        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport,
            0.5,
        )
        .expect("free Fillet scene");
        let arc_curve = scene
            .curves
            .iter()
            .find(|curve| curve.span == CurveSpan::line(arc))
            .expect("visible Fillet arc");
        assert_eq!(arc_curve.drag_handle_point, Some(center));
        let press = arc_curve.screen_polyline[arc_curve.screen_polyline.len() / 2];
        let initial_center_screen = viewport.model_to_screen(initial_center);
        let pointer = |position| PointerInput {
            pointer_id: 202,
            position,
            modifiers: Modifiers::default(),
        };
        coordinator.editor_mut().activate_tool(EditorTool::Select);
        assert!(matches!(
            coordinator.pointer_down(&scene, pointer(press)).as_slice(),
            [EditorEffect::SelectionChanged(selection)]
                if selection == &[SelectionItem::Curve(CurveSpan::line(arc))]
        ));

        let mut final_pointer = press;
        let mut final_preview_document = None;
        for (index, multiplier) in [2.0, 4.0, 6.0].into_iter().enumerate() {
            let target = [
                multiplier * initial_center[0],
                multiplier * initial_center[1],
            ];
            let target_screen = viewport.model_to_screen(target);
            final_pointer = ScreenPoint {
                x: press.x + target_screen.x - initial_center_screen.x,
                y: press.y + target_screen.y - initial_center_screen.y,
            };
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
                panic!("Fillet center projected request");
            };
            assert_eq!(*point, center);
            let preview_effects = coordinator.resolve_projected_point_move(
                *pointer_id,
                *request_id,
                *point,
                *model_position,
            );
            assert!(matches!(
                preview_effects.as_slice(),
                [EditorEffect::PreviewPointMove { point, .. }] if *point == center
            ));
            let work = coordinator
                .projected_drag_work_evidence()
                .expect("Fillet gesture work");
            assert!(work.accepted, "sample {index} rejected: {work:#?}");
            assert_eq!(work.attempts, 1);
            assert_eq!(work.continued, index > 0);
            assert_projected_drag_work_bounded(work);

            let preview = coordinator
                .solved_preview_session()
                .expect("accepted Fillet preview")
                .accepted_state()
                .expect("independently accepted Fillet state");
            let solve = preview
                .diagnostics()
                .solve
                .expect("accepted Fillet solve diagnostics");
            assert_eq!(
                solve.hard_validity,
                geosolve_sketch::SketchHardValidity::Valid
            );
            assert!(
                solve
                    .maximum_normalized_hard_residual
                    .is_some_and(|residual| residual <= 1.0e-9)
            );
            assert_eq!(
                preview
                    .document()
                    .curve_curve_fillet_for_arc(arc)
                    .expect("retained preview association")
                    .definition,
                association
            );
            final_preview_document = Some(preview.document().clone());
        }

        let final_preview_document = final_preview_document.expect("final accepted Fillet preview");
        let parameters_after = contacts.map(|contact| {
            let contact = final_preview_document
                .contact(contact)
                .expect("moved Fillet contact");
            final_preview_document
                .scalar(contact.parameter)
                .expect("moved Fillet contact parameter")
                .value
        });
        assert!(
            parameters_after
                .into_iter()
                .zip(parameters_before)
                .all(|(after, before)| (after - before).abs() > 0.25),
            "the gesture must carry both contacts beyond the former ±0.25 span window"
        );

        let expected = coordinator.session().design_identity();
        let release = coordinator
            .editor_mut()
            .pointer_up(&scene, expected, pointer(final_pointer));
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitPointMove { point, .. },
                EditorEffect::ClearPointPreview,
            ] if *point == center
        ));
        let committed = coordinator
            .apply_editor_effect(&release[0])
            .expect("release free Fillet gesture")
            .expect("Fillet point mutation");
        assert!(committed.published_accepted.is_some());
        let committed = coordinator
            .session()
            .accepted_state()
            .expect("committed Fillet gesture")
            .document();
        assert_eq!(committed, &final_preview_document);
        assert_eq!(
            committed
                .curve_curve_fillet_for_arc(arc)
                .expect("committed Fillet association")
                .definition,
            association
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact authoring-to-drag lifecycle guards Reference Fillet interaction before and after dimension deletion"
    )]
    fn default_reference_fillet_exits_and_resizes_before_and_after_dimension_deletion() {
        let (mut coordinator, parents, points) = simple_fillet_operation_fixture();
        for (index, (point, target)) in points
            .into_iter()
            .zip([[-2.0, 0.0], [2.0, 0.0], [0.0, -2.0], [0.0, 2.0]])
            .enumerate()
        {
            let fixed = coordinator
                .apply_edit(
                    coordinator.session().design_identity(),
                    DocumentEdit::CreateConstraint {
                        label: format!("fixed immediate-drag parent point {index}"),
                        definition: DocumentConstraintDefinition::FixedPoint { point, target },
                    },
                )
                .expect("fix immediate-drag Fillet parent point");
            assert!(fixed.published_accepted.is_some());
        }

        let document = coordinator
            .operation_authoring_document()
            .expect("current operation document")
            .clone();
        let picks = [
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(parents[0]), Some(0.25))
                .expect("first stamped Fillet pick"),
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(parents[1]), Some(0.75))
                .expect("second stamped Fillet pick"),
        ];
        let mut authoring = OperationAuthoringState::default();
        assert!(matches!(
            authoring.activate(&document, OperationAuthoringTool::Fillet, &picks),
            OperationAuthoringOutcome::PreviewRequested { .. }
        ));
        let confirmed = authoring.confirm(&document, [0.25, 0.25]);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = confirmed else {
            panic!("pointer placement must confirm a default Fillet candidate");
        };
        let SketchOperationRequest::AssociativeFillet { request, .. } = candidate.request() else {
            panic!("Fillet request");
        };
        assert_eq!(request.radius_mode, DocumentDimensionMode::Reference);
        let OperationAuthoringPreviewOutcome::Ready(metadata) = coordinator
            .prepare_operation_preview(&candidate)
            .expect("prepare immediate-drag Fillet")
        else {
            panic!("confirmed Fillet preview must be accepted");
        };
        assert!(metadata.apply_ready);
        let mutation = coordinator
            .apply_operation_preview(metadata.token, &candidate)
            .expect("publish immediate-drag Fillet");
        let arc = mutation.primary_created_curve;
        assert_eq!(arc, metadata.primary_created_curve);
        assert_eq!(
            authoring.publication_succeeded(),
            OperationAuthoringOutcome::ModeExited
        );
        assert_eq!(authoring.active_tool(), None);

        let (radius, radius_dimension) = {
            let design = coordinator.session().design_document();
            let CurveDefinition::CircularArc { radius, .. } =
                design.curve(arc).expect("published Fillet arc").definition
            else {
                panic!("published Fillet must be circular");
            };
            let radius_dimension = design
                .dimensions()
                .iter()
                .find(|dimension| {
                    matches!(
                        dimension.definition,
                        DocumentDimensionDefinition::Radius { curve, .. } if curve == arc
                    )
                })
                .expect("published Fillet radius dimension");
            assert_eq!(radius_dimension.mode, DocumentDimensionMode::Reference);
            (radius, radius_dimension.id)
        };
        let radius_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted published Fillet")
            .document()
            .scalar(radius)
            .expect("Fillet radius")
            .value;
        let (center_before, center_after) =
            drag_operation_arc_body_once(&mut coordinator, arc, 203, [-10.0, -10.0]);
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted immediately resized Fillet");
        let radius_after = accepted
            .document()
            .scalar(radius)
            .expect("immediately resized Fillet radius")
            .value;
        assert!(
            (center_after[0] - center_before[0]).hypot(center_after[1] - center_before[1]) > 1.0e-4
        );
        assert!((radius_after - radius_before).abs() > 1.0e-4);
        assert_eq!(
            accepted
                .document()
                .dimension(radius_dimension)
                .expect("Reference dimension remains after drag")
                .mode,
            DocumentDimensionMode::Reference
        );

        let deletion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(radius_dimension),
                },
            )
            .expect("delete the authored Reference radius dimension");
        assert!(deletion.published_accepted.is_some());
        assert!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted Fillet after dimension deletion")
                .document()
                .dimension(radius_dimension)
                .is_none()
        );
        let radius_before_second_drag = coordinator
            .session()
            .accepted_state()
            .expect("accepted dimension-free Fillet")
            .document()
            .scalar(radius)
            .expect("dimension-free Fillet radius")
            .value;
        let (second_center_before, second_center_after) =
            drag_operation_arc_body_once(&mut coordinator, arc, 204, [-10.0, -10.0]);
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted second Fillet resize");
        let radius_after_second_drag = accepted
            .document()
            .scalar(radius)
            .expect("twice-resized Fillet radius")
            .value;
        assert!(
            (second_center_after[0] - second_center_before[0])
                .hypot(second_center_after[1] - second_center_before[1])
                > 1.0e-4
        );
        assert!((radius_after_second_drag - radius_before_second_drag).abs() > 1.0e-4);
        assert!(accepted.document().dimension(radius_dimension).is_none());
        assert!(matches!(
            &accepted
                .document()
                .curve_curve_fillet_for_arc(arc)
                .expect("association survives both drags and dimension deletion")
                .definition,
            DocumentConstraintDefinition::CurveCurveFillet { arc: retained, .. }
                if *retained == arc
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one real pointer-authoring lifecycle qualifies three independent Fillet gestures on both sides of UI deletion"
    )]
    fn drawn_polyline_fillet_remains_interactive_after_ui_style_radius_deletion() {
        let document = SketchDocument::new(10.0).expect("empty sketch");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("empty retained session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let viewport =
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("authoring viewport");
        let empty_scene = {
            let accepted = coordinator
                .session()
                .accepted_state()
                .expect("accepted empty sketch");
            EditorScene::from_accepted_for_design(
                accepted.identity().revision().get(),
                coordinator.session().design_identity(),
                accepted.document(),
                coordinator.session().design_document(),
                viewport,
                0.5,
            )
            .expect("empty authoring scene")
        };
        let pointer = |position| PointerInput {
            pointer_id: 301,
            position: viewport.model_to_screen(position),
            modifiers: Modifiers::default(),
        };

        let _ = coordinator.editor_mut().activate_tool(EditorTool::Polyline);
        for position in [[-4.0, 0.0], [4.0, 0.0], [4.0, 4.0]] {
            let effects = coordinator
                .editor_mut()
                .pointer_down(&empty_scene, pointer(position));
            assert!(
                effects
                    .iter()
                    .all(|effect| !matches!(effect, EditorEffect::CommitConstruction { .. }))
            );
        }
        let expected = coordinator.session().design_identity();
        let completion = coordinator.editor_mut().complete_draft(expected);
        let construction = completion
            .iter()
            .find(|effect| matches!(effect, EditorEffect::CommitConstruction { .. }))
            .expect("complete two-segment polyline");
        let mutation = coordinator
            .apply_editor_effect(construction)
            .expect("apply polyline construction")
            .expect("polyline mutation");
        let EditorMutation::Construction(construction) = mutation.value else {
            panic!("polyline construction mutation expected");
        };
        let points: [DesignPointId; 3] = construction
            .points
            .try_into()
            .expect("three newly drawn polyline points");
        let [polyline] = construction.curves.as_slice() else {
            panic!("one newly drawn polyline");
        };
        let spans = [
            CurveSpan {
                curve: *polyline,
                segment: 0,
            },
            CurveSpan {
                curve: *polyline,
                segment: 1,
            },
        ];

        let operation_document = coordinator
            .operation_authoring_document()
            .expect("accepted drawn polyline")
            .clone();
        let picks = coordinator
            .operation_picks_for_item(
                OperationAuthoringTool::Fillet,
                SelectionItem::Point(points[1]),
                None,
            )
            .expect("corner expands to adjacent spans");
        assert_eq!(
            picks
                .iter()
                .filter_map(OperationAuthoringPick::curve_span)
                .collect::<Vec<_>>(),
            spans
        );
        let mut authoring = OperationAuthoringState::default();
        let activated =
            authoring.activate(&operation_document, OperationAuthoringTool::Fillet, &picks);
        let OperationAuthoringOutcome::PreviewRequested {
            candidate: unconfirmed,
            ..
        } = activated
        else {
            panic!("drawn corner must stage a Fillet preview: {activated:?}");
        };
        assert!(!unconfirmed.is_confirmed());
        let _ = coordinator
            .prepare_operation_preview(&unconfirmed)
            .expect("prepare unconfirmed drawn-corner preview");

        let confirmed = authoring.confirm(&operation_document, [3.8, 0.2]);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = confirmed else {
            panic!("drawn corner must confirm a Fillet preview: {confirmed:?}");
        };
        assert!(candidate.is_confirmed());
        let SketchOperationRequest::AssociativeFillet { request, .. } = candidate.request() else {
            panic!("drawn corner must resolve an associative Fillet request");
        };
        assert_eq!(request.radius_mode, DocumentDimensionMode::Reference);
        let OperationAuthoringPreviewOutcome::Ready(metadata) = coordinator
            .prepare_operation_preview(&candidate)
            .expect("prepare confirmed drawn-corner preview")
        else {
            panic!("confirmed drawn-corner preview must be accepted");
        };
        assert!(metadata.apply_ready);
        let mutation = coordinator
            .apply_operation_preview(metadata.token, &candidate)
            .expect("apply drawn-corner Fillet");
        let arc = mutation.primary_created_curve;
        assert_eq!(arc, metadata.primary_created_curve);
        assert_eq!(
            authoring.publication_succeeded(),
            OperationAuthoringOutcome::ModeExited
        );
        assert_eq!(authoring.active_tool(), None);
        let _ = coordinator.editor_mut().activate_tool(EditorTool::Select);
        assert_eq!(coordinator.editor().tool(), EditorTool::Select);

        let radius_dimension = coordinator
            .session()
            .design_document()
            .dimensions()
            .iter()
            .find(|dimension| {
                matches!(
                    dimension.definition,
                    DocumentDimensionDefinition::Radius { curve, .. } if curve == arc
                )
            })
            .expect("generated Fillet radius dimension");
        assert_eq!(radius_dimension.mode, DocumentDimensionMode::Reference);
        let radius_dimension = radius_dimension.id;

        let before_radius_deletion = coordinator.session().clone();
        let mut arc_probe = RetainedEditorCoordinator::new(before_radius_deletion.clone())
            .expect("pre-deletion arc probe");
        let (before, after) = drag_operation_arc_body_once(&mut arc_probe, arc, 302, [-5.0, -5.0]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);

        let mut outer_point_probe = RetainedEditorCoordinator::new(before_radius_deletion.clone())
            .expect("pre-deletion outer-point probe");
        let (before, after) =
            drag_operation_point_once(&mut outer_point_probe, points[0], 303, [4.0, -2.8]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);

        let mut corner_probe = RetainedEditorCoordinator::new(before_radius_deletion.clone())
            .expect("pre-deletion shared-corner probe");
        let (before, after) =
            drag_operation_point_once(&mut corner_probe, points[1], 304, [4.0, -2.8]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);

        let mut second_outer_point_probe = RetainedEditorCoordinator::new(before_radius_deletion)
            .expect("pre-deletion second outer-point probe");
        let (before, after) =
            drag_operation_point_once(&mut second_outer_point_probe, points[2], 305, [-2.8, 4.0]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Dimension(radius_dimension)]);
        let expected = coordinator.session().design_identity();
        let deletion = coordinator
            .delete_selected(expected)
            .expect("UI-style radius deletion");
        assert!(deletion.published_accepted.is_some());
        assert_eq!(
            deletion.value,
            [DocumentObjectId::Dimension(radius_dimension)]
        );
        let design = coordinator.session().design_document();
        assert!(design.dimension(radius_dimension).is_none());
        assert!(matches!(
            &design
                .curve_curve_fillet_for_arc(arc)
                .expect("Fillet association survives radius deletion")
                .definition,
            DocumentConstraintDefinition::CurveCurveFillet { arc: retained, .. }
                if *retained == arc
        ));

        let after_radius_deletion = coordinator.session().clone();
        let mut arc_probe = RetainedEditorCoordinator::new(after_radius_deletion.clone())
            .expect("post-deletion arc probe");
        let (before, after) = drag_operation_arc_body_once(&mut arc_probe, arc, 306, [-5.0, -5.0]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);

        let mut outer_point_probe = RetainedEditorCoordinator::new(after_radius_deletion.clone())
            .expect("post-deletion outer-point probe");
        let (before, after) =
            drag_operation_point_once(&mut outer_point_probe, points[0], 307, [4.0, -2.8]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);

        let mut corner_probe = RetainedEditorCoordinator::new(after_radius_deletion.clone())
            .expect("post-deletion shared-corner probe");
        let (before, after) =
            drag_operation_point_once(&mut corner_probe, points[1], 308, [4.0, -2.8]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);

        let mut second_outer_point_probe = RetainedEditorCoordinator::new(after_radius_deletion)
            .expect("post-deletion second outer-point probe");
        let (before, after) =
            drag_operation_point_once(&mut second_outer_point_probe, points[2], 309, [-2.8, 4.0]);
        assert!((after[0] - before[0]).hypot(after[1] - before[1]) > 1.0e-6);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the hostile curved-parent regression keeps the complete public authoring and edit lifecycle visible"
    )]
    fn hostile_bezier_parent_edit_never_publishes_fillet_contact_outside_authored_local_neighborhood()
     {
        let mut document = SketchDocument::new(8.0).expect("document");
        let (line, line_points) = operation_test_line(
            &mut document,
            "fixed horizontal line",
            [-10.0, 0.0],
            [10.0, 0.0],
        );
        for (index, (point, target)) in line_points
            .into_iter()
            .zip([[-10.0, 0.0], [10.0, 0.0]])
            .enumerate()
        {
            document
                .add_constraint(
                    format!("fixed line point {index}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("fixed line point");
        }

        let control_positions = [[-6.0, 1.0], [-1.0, 8.0], [1.0, 8.0], [6.0, 1.0]];
        let controls = control_positions.map(|position| {
            document
                .add_point("Bezier control", position)
                .expect("Bezier control")
        });
        let control_locks = controls
            .iter()
            .copied()
            .zip(control_positions)
            .enumerate()
            .map(|(index, (point, target))| {
                document
                    .add_constraint(
                        format!("fixed Bezier control {index}"),
                        DocumentConstraintDefinition::FixedPoint { point, target },
                    )
                    .expect("fixed Bezier control")
            })
            .collect::<Vec<_>>();
        let bezier = CurveSpan::line(
            document
                .add_curve(
                    "hostile cubic Bezier parent",
                    CurveDefinition::CubicBezier { controls },
                )
                .expect("cubic Bezier"),
        );

        let expected_bezier_parameter = 0.361_804_407_541_642_45;
        let authored_radius = 3.108_415_523_528_273_5;
        let mut coordinator = operation_test_coordinator(document);
        // These are deliberately discarded-side picks, not the inferred contacts:
        // each one identifies trim ownership while the local root search resolves
        // the exact line/Bezier contact pair above.
        let accepted_document = coordinator
            .operation_authoring_document()
            .expect("current accepted operation document")
            .clone();
        let picks = [
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(line), Some(0.25))
                .expect("stamped line pick"),
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(bezier), Some(0.15))
                .expect("stamped Bezier pick"),
        ];
        let mut authoring = OperationAuthoringState::default();
        let _ = authoring.set_options(
            &accepted_document,
            OperationAuthoringOptions {
                fillet_radius: Some(authored_radius),
                fillet_radius_mode: DocumentDimensionMode::Driving,
                fillet_flip_second_side: true,
                ..OperationAuthoringOptions::default()
            },
        );
        let seeded = authoring.activate(&accepted_document, OperationAuthoringTool::Fillet, &picks);
        assert!(matches!(
            seeded,
            OperationAuthoringOutcome::PreviewRequested { .. }
        ));
        let jets = picks.each_ref().map(|pick| {
            accepted_document
                .evaluate_curve_jet(pick.curve_span().expect("curve pick"), pick.curve_parameter)
                .expect("accepted operation pick jet")
        });
        let first_direction = jets[0].first_derivative;
        let second_direction = jets[1].first_derivative;
        let denominator =
            first_direction.x * second_direction.y - first_direction.y * second_direction.x;
        let between = jets[1].position - jets[0].position;
        let first_parameter =
            (between.x * second_direction.y - between.y * second_direction.x) / denominator;
        let tangent_intersection = [
            jets[0].position.x + first_parameter * first_direction.x,
            jets[0].position.y + first_parameter * first_direction.y,
        ];
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } =
            authoring.confirm(&accepted_document, tangent_intersection)
        else {
            panic!("expected confirmed hostile Fillet candidate");
        };
        assert!(candidate.is_confirmed());
        let SketchOperationRequest::AssociativeFillet { request, .. } = candidate.request() else {
            panic!("Fillet authoring must publish an associative request");
        };
        assert_eq!(
            request.first.side,
            geosolve_sketch::DocumentCurveNormalSide::Left
        );
        assert_eq!(
            request.second.side,
            geosolve_sketch::DocumentCurveNormalSide::Right
        );
        assert!((request.first.parameter - 0.5).abs() <= 1.0e-9);
        assert!((request.second.parameter - expected_bezier_parameter).abs() <= 1.0e-8);

        let metadata = commit_operation_and_assert_lifecycle(&mut coordinator, &candidate);
        let arc = metadata.primary_created_curve;
        let (radius_dimension, contacts, bezier_neighborhood) = {
            let accepted = coordinator.session().design_document();
            let radius_dimension = accepted
                .dimensions()
                .iter()
                .find_map(|dimension| match dimension.definition {
                    DocumentDimensionDefinition::Radius { curve, .. } if curve == arc => {
                        Some(dimension.id)
                    }
                    _ => None,
                })
                .expect("authored Fillet radius dimension");
            let association = accepted
                .curve_curve_fillet_for_arc(arc)
                .expect("authored Fillet association");
            let DocumentConstraintDefinition::CurveCurveFillet {
                first_contact,
                second_contact,
                ..
            } = association.definition
            else {
                unreachable!("generic Fillet association checked above");
            };
            let contacts = [first_contact, second_contact];
            let line_contact = accepted.contact(first_contact).expect("line contact");
            let bezier_contact = accepted.contact(second_contact).expect("Bezier contact");
            assert_eq!(line_contact.curve, line);
            assert_eq!(bezier_contact.curve, bezier);
            assert_eq!(line_contact.neighborhood, ContactNeighborhood::Interior);
            let bezier_neighborhood = bezier_contact.neighborhood;
            let ContactNeighborhood::Local { lower, upper } = bezier_neighborhood else {
                panic!("curved Fillet contact must retain an authored local root cell");
            };
            let initial_parameter = accepted
                .scalar(bezier_contact.parameter)
                .expect("Bezier contact parameter")
                .value;
            assert!((initial_parameter - expected_bezier_parameter).abs() <= 1.0e-8);
            assert!(lower < initial_parameter && initial_parameter < upper);
            (radius_dimension, contacts, bezier_neighborhood)
        };

        let deletion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(radius_dimension),
                },
            )
            .expect("delete the Fillet radius dimension");
        assert!(deletion.published_accepted.is_some());
        let unlock = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetElementUserSuppressed {
                    element: DocumentElementId::Constraint(control_locks[1]),
                    suppressed: true,
                },
            )
            .expect("suppress the second Bezier control lock");
        assert!(unlock.published_accepted.is_some());

        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted free-radius Fillet");
        let accepted_identity_before = accepted_before.identity();
        let accepted_json_before = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON before hostile edit");
        let hostile = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: controls[1],
                    position: [-8.0, -5.825],
                },
            )
            .expect("submit hostile Bezier control edit");

        if hostile.published_accepted.is_none() {
            assert_eq!(
                coordinator
                    .session()
                    .accepted_state()
                    .expect("retained accepted Fillet")
                    .identity(),
                accepted_identity_before
            );
            assert_eq!(
                coordinator
                    .session()
                    .export_accepted_json()
                    .expect("accepted JSON after rejected hostile edit"),
                accepted_json_before
            );
            return;
        }

        let accepted_after = coordinator
            .session()
            .accepted_state()
            .expect("accepted hostile Fillet edit")
            .document();
        let line_contact = accepted_after
            .contact(contacts[0])
            .expect("retained line contact");
        let bezier_contact = accepted_after
            .contact(contacts[1])
            .expect("retained Bezier contact");
        assert_eq!(line_contact.neighborhood, ContactNeighborhood::Interior);
        assert_eq!(bezier_contact.neighborhood, bezier_neighborhood);
        let ContactNeighborhood::Local { lower, upper } = bezier_neighborhood else {
            unreachable!("authored Bezier neighborhood checked above");
        };
        let parameter = accepted_after
            .scalar(bezier_contact.parameter)
            .expect("edited Bezier contact parameter")
            .value;
        assert!(
            lower < parameter && parameter < upper,
            "accepted Bezier contact escaped its authored local root cell: {parameter} not in ({lower}, {upper})"
        );
    }

    struct ComputedFilletEditorFixture {
        coordinator: RetainedEditorCoordinator,
        points: [DesignPointId; 4],
        spans: [CurveSpan; 3],
    }

    fn computed_fillet_editor_fixture() -> ComputedFilletEditorFixture {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [
            document.add_point("p0", [0.0, 0.0]).expect("p0"),
            document.add_point("p1", [4.0, 0.0]).expect("p1"),
            document.add_point("p2", [4.0, 4.0]).expect("p2"),
            document.add_point("p3", [8.0, 4.0]).expect("p3"),
        ];
        let curve = document
            .add_curve(
                "three-span polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
                },
            )
            .expect("polyline");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        ComputedFilletEditorFixture {
            coordinator: RetainedEditorCoordinator::new(session).expect("coordinator"),
            points,
            spans: [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
        }
    }

    fn grouped_fillet_candidate(
        coordinator: &RetainedEditorCoordinator,
        corners: impl IntoIterator<Item = DesignPointId>,
    ) -> FeatureAuthoringCandidate {
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let selection = corners
            .into_iter()
            .map(|point| (SelectionItem::Point(point), None))
            .collect::<Vec<_>>();
        let mut authoring = FeatureAuthoringState::default();
        match authoring.activate(
            &snapshot,
            coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &selection,
        ) {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
            other => panic!("expected grouped Fillet candidate, got {other:?}"),
        }
    }

    fn feature_candidate(outcome: FeatureAuthoringOutcome) -> FeatureAuthoringCandidate {
        match outcome {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
            | FeatureAuthoringOutcome::Apply(candidate) => candidate,
            other => panic!("expected complete feature candidate, got {other:?}"),
        }
    }

    fn apply_grouped_fillet(
        coordinator: &mut RetainedEditorCoordinator,
        candidate: &FeatureAuthoringCandidate,
    ) -> ComputedFeatureId {
        let metadata = coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                candidate,
                "adjacent corners",
            )
            .expect("computed preview");
        coordinator
            .apply_feature_authoring_preview(metadata.token, candidate)
            .expect("computed publication")
            .value
    }

    fn fillet_radius(coordinator: &RetainedEditorCoordinator, feature: ComputedFeatureId) -> f64 {
        let ComputedFeatureDefinition::FilletSet(fillet) = &coordinator
            .feature_document()
            .feature(feature)
            .expect("Fillet feature")
            .definition;
        fillet.radius
    }

    #[derive(Debug, Eq, PartialEq)]
    struct AcceptedSketchEquationInvariants {
        hard_residual_bits: Vec<u64>,
        maximum_hard_residual_bits: Option<u64>,
        hard_residual_l2_bits: Option<u64>,
        numerical_rank: Option<usize>,
        equality_degrees_of_freedom: Option<usize>,
        bidirectional_degrees_of_freedom: Option<usize>,
    }

    fn accepted_sketch_equation_invariants(
        coordinator: &RetainedEditorCoordinator,
    ) -> AcceptedSketchEquationInvariants {
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted sketch invariants");
        let diagnostics = accepted.diagnostics();
        let solve = diagnostics.solve.expect("solve diagnostics");
        let rank = diagnostics.rank.expect("rank diagnostics");
        let mobility = diagnostics.mobility.expect("mobility diagnostics");
        let hard_residual_bits = accepted
            .solve_result()
            .display_audit
            .sources
            .iter()
            .flat_map(|source| source.rows.iter())
            .map(|row| row.normalized_residual.to_bits())
            .collect();
        AcceptedSketchEquationInvariants {
            hard_residual_bits,
            maximum_hard_residual_bits: solve.maximum_normalized_hard_residual.map(f64::to_bits),
            hard_residual_l2_bits: solve.normalized_hard_residual_l2.map(f64::to_bits),
            numerical_rank: rank.numerical_rank,
            equality_degrees_of_freedom: mobility.equality_degrees_of_freedom,
            bidirectional_degrees_of_freedom: mobility.bidirectional_bounded_degrees_of_freedom,
        }
    }

    fn computed_feature_state(
        snapshot: &ComputedFeatureSnapshot,
        feature: ComputedFeatureId,
    ) -> &ComputedFeatureEvaluationState {
        &snapshot
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == feature)
            .expect("computed feature evaluation")
            .state
    }

    fn current_computed_scene(coordinator: &RetainedEditorCoordinator) -> EditorScene {
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted computed scene");
        let expected = coordinator
            .computed_evaluation_input()
            .expect("exact computed input");
        EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &coordinator
                .session()
                .accepted_prepared_input()
                .expect("current accepted computed input"),
            &expected,
            coordinator
                .computed_snapshot()
                .expect("current computed output"),
            Viewport::new([900.0, 700.0], [1.0, 1.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("current composite scene")
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn grouped_computed_fillet_preview_composes_scene_and_never_mutates_the_sketch() {
        let mut fixture = computed_fillet_editor_fixture();
        let coordinator = &mut fixture.coordinator;
        let design_before = coordinator.session().design_identity();
        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted sketch")
            .identity();
        let design_json_before = coordinator
            .session()
            .export_design_json()
            .expect("design JSON");
        let accepted_json_before = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON");
        let right_nullity_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted sketch")
            .solve_result()
            .unstable_core_report()
            .right_nullity;
        let candidate =
            grouped_fillet_candidate(coordinator, fixture.points[1..=2].iter().copied());
        assert_eq!(candidate.corners().len(), 2);
        let metadata = coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "two adjacent corners",
            )
            .expect("whole-batch preview");
        {
            let preview = coordinator
                .feature_authoring_preview()
                .expect("held preview");
            assert_eq!(preview.metadata(), &metadata);
            assert_eq!(preview.snapshot().edges().len(), 5);
            assert!(matches!(
                &preview.snapshot().feature_evaluations()[0].state,
                ComputedFeatureEvaluationState::Current { corner_edges }
                    if corner_edges.len() == 2
            ));
            let middle = preview
                .snapshot()
                .source_fragment_edges(NativeCurveSpanSource {
                    span: fixture.spans[1],
                })
                .next()
                .expect("middle replacement");
            let ComputedEdgeProvenance::SourceFragment {
                start_claim,
                end_claim,
                ..
            } = middle.provenance
            else {
                panic!("middle edge must retain source-fragment provenance");
            };
            assert!(start_claim.is_some() && end_claim.is_some());
        }
        let feature = coordinator
            .apply_feature_authoring_preview(metadata.token, &candidate)
            .expect("publish exact preview")
            .value;
        assert_eq!(coordinator.session().design_identity(), design_before);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted sketch")
                .identity(),
            accepted_before
        );
        assert_eq!(
            coordinator
                .session()
                .export_design_json()
                .expect("design JSON"),
            design_json_before
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON"),
            accepted_json_before
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted sketch")
                .solve_result()
                .unstable_core_report()
                .right_nullity,
            right_nullity_before
        );
        assert_eq!(
            fillet_radius(coordinator, feature).to_bits(),
            candidate.radius().to_bits()
        );
        assert_eq!(
            coordinator.computed_profile_boundary(),
            ComputedProfileBoundary::Withheld { active_features: 1 }
        );

        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted sketch");
        let viewport = Viewport::new([900.0, 700.0], [1.0, 1.0], 50.0).expect("viewport");
        let scene = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &coordinator
                .session()
                .accepted_prepared_input()
                .expect("current accepted computed input"),
            &coordinator
                .computed_evaluation_input()
                .expect("exact computed input"),
            coordinator.computed_snapshot().expect("computed output"),
            viewport,
            0.5,
        )
        .expect("composite scene");
        assert_eq!(scene.computed_curves.len(), 2);
        assert_eq!(
            scene
                .curves
                .iter()
                .filter(|curve| curve.span == fixture.spans[1])
                .count(),
            1,
            "the shared middle span must be replaced once and trimmed at both ends"
        );
        for curve in &scene.computed_curves {
            assert_eq!(
                coordinator.selection_for_computed_edge(curve.edge),
                Some(SelectionItem::FeatureCorner(curve.owner))
            );
        }
        let arc = &scene.computed_curves[0];
        let arc_sample = arc.screen_polyline[arc.screen_polyline.len() / 2];
        assert_eq!(
            scene
                .hit_test(arc_sample, PickTolerance::default())
                .expect("computed arc hit")
                .item,
            SelectionItem::FeatureCorner(arc.owner)
        );
        assert!(!matches!(
            scene
                .native_authoring_hit_test(arc_sample, PickTolerance::default())
                .map(|hit| hit.item),
            Some(SelectionItem::FeatureCorner(_))
        ));
        let middle = scene
            .curves
            .iter()
            .find(|curve| curve.span == fixture.spans[1])
            .expect("middle source fragment");
        let first = middle.screen_polyline[0];
        let last = *middle.screen_polyline.last().expect("middle end");
        let middle_sample = ScreenPoint {
            x: 0.5 * (first.x + last.x),
            y: 0.5 * (first.y + last.y),
        };
        assert_eq!(
            scene
                .native_authoring_hit_test(middle_sample, PickTolerance::default())
                .expect("native replacement hit")
                .item,
            SelectionItem::Curve(fixture.spans[1])
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn computed_radius_cas_history_and_reload_preserve_sketch_and_never_reuse_output_ids() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&fixture.coordinator, fixture.points[1..=2].iter().copied());
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let coordinator = &mut fixture.coordinator;
        let design_identity = coordinator.session().design_identity();
        let accepted_identity = coordinator
            .session()
            .accepted_state()
            .expect("accepted sketch")
            .identity();
        let sketch_json = coordinator
            .session()
            .export_design_json()
            .expect("design JSON");
        let accepted_json = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON");
        let sketch_equation_invariants = accepted_sketch_equation_invariants(coordinator);
        let initial_snapshot = coordinator.computed_snapshot().expect("initial output");
        let initial_input = initial_snapshot.input();
        let initial_edges = coordinator
            .computed_snapshot()
            .expect("initial output")
            .edges()
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let initial_evaluation = initial_snapshot.evaluation_revision();
        coordinator
            .set_computed_fillet_radius_exact(initial_input, feature, 0.75)
            .expect("exact radius edit");
        assert_eq!(
            fillet_radius(coordinator, feature).to_bits(),
            0.75_f64.to_bits()
        );
        assert_eq!(coordinator.session().design_identity(), design_identity);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted sketch")
                .identity(),
            accepted_identity
        );
        assert_eq!(
            coordinator
                .session()
                .export_design_json()
                .expect("design JSON"),
            sketch_json
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON"),
            accepted_json
        );
        assert_eq!(
            accepted_sketch_equation_invariants(coordinator),
            sketch_equation_invariants
        );
        let resized_evaluation = coordinator
            .computed_snapshot()
            .expect("resized output")
            .evaluation_revision();
        assert!(resized_evaluation.raw() > initial_evaluation.raw());
        assert!(matches!(
            coordinator.set_computed_fillet_radius_exact(initial_input, feature, 0.5),
            Err(CoordinatorError::StaleComputedFeatureCandidate)
        ));

        coordinator.undo().expect("undo radius");
        assert_eq!(
            fillet_radius(coordinator, feature).to_bits(),
            candidate.radius().to_bits()
        );
        let undo_evaluation = coordinator
            .computed_snapshot()
            .expect("undo output")
            .evaluation_revision();
        assert!(undo_evaluation.raw() > resized_evaluation.raw());
        coordinator.redo().expect("redo radius");
        assert_eq!(
            fillet_radius(coordinator, feature).to_bits(),
            0.75_f64.to_bits()
        );
        let redo_evaluation = coordinator
            .computed_snapshot()
            .expect("redo output")
            .evaluation_revision();
        assert!(redo_evaluation.raw() > undo_evaluation.raw());
        let saved = coordinator.checkpoint().clone();
        let current_input = coordinator
            .computed_snapshot()
            .expect("redo output")
            .input();
        coordinator
            .set_computed_fillet_radius_exact(current_input, feature, 0.6)
            .expect("later radius edit");
        let before_reload = coordinator
            .computed_snapshot()
            .expect("later output")
            .evaluation_revision();
        coordinator.reload(&saved).expect("composite reload");
        assert_eq!(
            fillet_radius(coordinator, feature).to_bits(),
            0.75_f64.to_bits()
        );
        let reloaded = coordinator.computed_snapshot().expect("reloaded output");
        assert!(reloaded.evaluation_revision().raw() > before_reload.raw());
        assert!(
            initial_edges
                .iter()
                .all(|edge| reloaded.edge(*edge).is_none())
        );
        assert_eq!(coordinator.history_len(), 1);
        assert_eq!(coordinator.history_cursor(), 0);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn authoring_radius_refresh_is_multi_sample_exact_and_failure_transactional() {
        let mut fixture = computed_fillet_editor_fixture();
        let snapshot = fixture
            .coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let selection = fixture.points[1..=2]
            .iter()
            .copied()
            .map(|point| (SelectionItem::Point(point), None))
            .collect::<Vec<_>>();
        let mut authoring = FeatureAuthoringState::default();
        let initial = feature_candidate(authoring.activate(
            &snapshot,
            fixture.coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &selection,
        ));
        let prepared = fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &initial,
                "dragged batch",
            )
            .expect("initial preview");
        let initial_bindings = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("initial held preview")
            .corner_bindings();
        let gesture_origin = prepared.input;
        let first = feature_candidate(authoring.set_options(
            &snapshot,
            crate::FeatureAuthoringOptions {
                fillet_radius: Some(0.8),
                ..authoring.options()
            },
        ));
        let first_metadata = fixture
            .coordinator
            .refresh_feature_authoring_preview(gesture_origin, &first)
            .expect("first sample");
        assert_eq!(first_metadata.feature, prepared.feature);
        assert_ne!(first_metadata.token, prepared.token);
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("first refreshed preview")
                .corner_bindings(),
            initial_bindings
        );
        assert!(fixture.coordinator.feature_document().features().is_empty());

        let held_before_generic_effect = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("held refreshed preview")
            .candidate()
            .clone();
        assert!(matches!(
            fixture
                .coordinator
                .apply_editor_effect(&EditorEffect::PreviewComputedFeatureRadius {
                    expected: gesture_origin,
                    feature: prepared.feature,
                    radius: 0.7,
                }),
            Err(CoordinatorError::FeatureAuthoringPreviewMismatch)
        ));
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("generic effect retained preview")
                .candidate(),
            &held_before_generic_effect
        );

        let mut invalid_origin = gesture_origin;
        invalid_origin.policy.max_root_iterations += 1;
        assert!(matches!(
            fixture
                .coordinator
                .refresh_feature_authoring_preview(invalid_origin, &first),
            Err(CoordinatorError::FeatureAuthoringPreviewMismatch)
        ));
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("prior preview retained")
                .metadata(),
            &first_metadata
        );

        let second = feature_candidate(authoring.set_options(
            &snapshot,
            crate::FeatureAuthoringOptions {
                fillet_radius: Some(0.6),
                ..authoring.options()
            },
        ));
        let second_metadata = fixture
            .coordinator
            .refresh_feature_authoring_preview(gesture_origin, &second)
            .expect("second sample still accepts pointer-down input");
        assert_eq!(second_metadata.feature, prepared.feature);
        assert_ne!(second_metadata.token, first_metadata.token);
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("second refreshed preview")
                .corner_bindings(),
            initial_bindings
        );
        assert!(matches!(
            fixture
                .coordinator
                .apply_feature_authoring_preview(first_metadata.token, &second),
            Err(CoordinatorError::FeatureAuthoringPreviewMismatch)
        ));
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("stale token retains preview")
                .metadata(),
            &second_metadata
        );
        assert!(matches!(
            fixture
                .coordinator
                .apply_feature_authoring_preview(second_metadata.token, &first),
            Err(CoordinatorError::FeatureAuthoringPreviewMismatch)
        ));
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("stale candidate retains preview")
                .metadata(),
            &second_metadata
        );
        fixture
            .coordinator
            .apply_editor_effect(&EditorEffect::RestoreComputedFeatureRadius {
                expected: gesture_origin,
                feature: prepared.feature,
                radius: initial.radius(),
            })
            .expect("restore exact pointer-down preview");
        let restored = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("restored preview");
        assert_eq!(restored.candidate(), &initial);
        assert_eq!(restored.metadata(), &prepared);

        let final_metadata = fixture
            .coordinator
            .refresh_feature_authoring_preview(gesture_origin, &second)
            .expect("rebuild final sample after rollback");
        fixture
            .coordinator
            .accept_feature_authoring_radius_preview(gesture_origin, prepared.feature)
            .expect("rebase gesture origin");
        let published = fixture
            .coordinator
            .apply_feature_authoring_preview(final_metadata.token, &second)
            .expect("publish latest exact candidate");
        assert_eq!(published.value, prepared.feature);
        assert_eq!(
            fillet_radius(&fixture.coordinator, published.value).to_bits(),
            0.6_f64.to_bits()
        );
    }

    #[test]
    fn clearing_feature_authoring_preview_removes_only_temporary_selection() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &candidate,
                "temporary corner",
            )
            .expect("preview");
        let owner = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("held preview")
            .corner_bindings()[0]
            .owner;
        let native = SelectionItem::Point(fixture.points[0]);
        fixture
            .coordinator
            .editor_mut()
            .set_selection([native, SelectionItem::FeatureCorner(owner)]);

        fixture.coordinator.clear_feature_authoring_preview();

        assert!(fixture.coordinator.feature_authoring_preview().is_none());
        assert_eq!(fixture.coordinator.editor().selection(), &[native]);
    }

    #[test]
    fn replacing_feature_authoring_preview_clears_reused_temporary_selection() {
        let mut fixture = computed_fillet_editor_fixture();
        let first = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let first_metadata = fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &first,
                "first temporary corner",
            )
            .expect("first preview");
        let first_owner = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("first held preview")
            .corner_bindings()[0]
            .owner;
        let native = SelectionItem::Curve(fixture.spans[0]);
        fixture
            .coordinator
            .editor_mut()
            .set_selection([native, SelectionItem::FeatureCorner(first_owner)]);

        let second = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[2]]);
        let second_metadata = fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &second,
                "replacement temporary corner",
            )
            .expect("replacement preview");

        assert_eq!(
            second_metadata.feature, first_metadata.feature,
            "a replacement preview from the unchanged base document reuses its provisional feature ID"
        );
        assert!(fixture.coordinator.feature_authoring_preview().is_some());
        assert_eq!(fixture.coordinator.editor().selection(), &[native]);
    }

    #[test]
    fn refreshing_held_feature_authoring_batch_preserves_temporary_selection() {
        let mut fixture = computed_fillet_editor_fixture();
        let snapshot = fixture
            .coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let mut authoring = FeatureAuthoringState::default();
        let initial = feature_candidate(authoring.activate(
            &snapshot,
            fixture.coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Point(fixture.points[1]), None)],
        ));
        let prepared = fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &initial,
                "refreshable corner",
            )
            .expect("initial preview");
        let owner = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("initial held preview")
            .corner_bindings()[0]
            .owner;
        let native = SelectionItem::Point(fixture.points[0]);
        let expected_selection = [native, SelectionItem::FeatureCorner(owner)];
        fixture
            .coordinator
            .editor_mut()
            .set_selection(expected_selection);
        let resized = feature_candidate(authoring.set_options(
            &snapshot,
            crate::FeatureAuthoringOptions {
                fillet_radius: Some(0.8),
                ..authoring.options()
            },
        ));

        let refreshed = fixture
            .coordinator
            .refresh_feature_authoring_preview(prepared.input, &resized)
            .expect("refreshed preview");

        assert_eq!(refreshed.feature, prepared.feature);
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("refreshed held preview")
                .corner_bindings()[0]
                .owner,
            owner
        );
        assert_eq!(
            fixture.coordinator.editor().selection(),
            &expected_selection
        );
    }

    #[test]
    fn clearing_transient_state_removes_temporary_feature_selection() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &candidate,
                "transient corner",
            )
            .expect("preview");
        let owner = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("held preview")
            .corner_bindings()[0]
            .owner;
        let native = SelectionItem::Curve(fixture.spans[0]);
        fixture
            .coordinator
            .editor_mut()
            .set_selection([native, SelectionItem::FeatureCorner(owner)]);

        fixture.coordinator.clear_transient();

        assert!(fixture.coordinator.feature_authoring_preview().is_none());
        assert_eq!(fixture.coordinator.editor().selection(), &[native]);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn clearing_feature_preview_revokes_hover_context_and_active_radius_gesture() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let metadata = fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &candidate,
                "interactive temporary corner",
            )
            .expect("preview");
        let scene = {
            let accepted = fixture
                .coordinator
                .session()
                .accepted_state()
                .expect("accepted sketch");
            let preview = fixture
                .coordinator
                .feature_authoring_preview()
                .expect("held preview");
            EditorScene::from_accepted_with_computed(
                accepted.identity().revision().get(),
                accepted.design_identity(),
                accepted.document(),
                fixture.coordinator.session().design_document(),
                &fixture
                    .coordinator
                    .session()
                    .accepted_prepared_input()
                    .expect("accepted input"),
                &metadata.input,
                preview.snapshot(),
                Viewport::new([900.0, 700.0], [1.0, 1.0], 50.0).expect("viewport"),
                0.5,
            )
            .expect("composite preview scene")
        };
        let arc = scene
            .computed_curves
            .iter()
            .find(|curve| curve.owner.feature == metadata.feature)
            .expect("temporary arc")
            .clone();
        let owner = SelectionItem::FeatureCorner(arc.owner);
        let press = arc.screen_polyline[arc.screen_polyline.len() / 2];
        let press_model = scene.viewport.screen_to_model(press);
        let direction = [
            (press_model[0] - arc.center[0]) / arc.radius,
            (press_model[1] - arc.center[1]) / arc.radius,
        ];
        let moved = scene.viewport.model_to_screen([
            0.2_f64.mul_add(direction[0], press_model[0]),
            0.2_f64.mul_add(direction[1], press_model[1]),
        ]);
        let pointer = |position| PointerInput {
            pointer_id: 1_337,
            position,
            modifiers: Modifiers::default(),
        };

        let hover = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(press));
        assert!(matches!(
            hover.as_slice(),
            [EditorEffect::HoverChanged(state)]
                if state.target.is_some() && state.context_owner == Some(owner)
        ));
        assert_eq!(fixture.coordinator.editor().hovered(), Some(owner));
        assert_eq!(
            fixture.coordinator.editor().hover_state().context_owner,
            Some(owner)
        );

        fixture.coordinator.pointer_down(&scene, pointer(press));
        let active_move = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(moved));
        assert!(matches!(
            active_move.as_slice(),
            [EditorEffect::PreviewComputedFeatureRadius { feature, .. }]
                if *feature == metadata.feature
        ));

        fixture.coordinator.clear_feature_authoring_preview();

        assert!(fixture.coordinator.feature_authoring_preview().is_none());
        assert_eq!(
            fixture.coordinator.editor().hover_state(),
            crate::EditorHoverState::default()
        );
        let base_scene = {
            let accepted = fixture
                .coordinator
                .session()
                .accepted_state()
                .expect("accepted sketch");
            EditorScene::from_accepted_for_design(
                accepted.identity().revision().get(),
                accepted.design_identity(),
                accepted.document(),
                fixture.coordinator.session().design_document(),
                scene.viewport,
                0.5,
            )
            .expect("base scene after preview clear")
        };
        let move_after_clear = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&base_scene, pointer(moved));
        let expected_design = fixture.coordinator.session().design_identity();
        let up_after_clear = fixture.coordinator.editor_mut().pointer_up(
            &base_scene,
            expected_design,
            pointer(moved),
        );
        assert!(
            move_after_clear
                .iter()
                .chain(&up_after_clear)
                .all(|effect| !matches!(
                    effect,
                    EditorEffect::PreviewComputedFeatureRadius { feature, .. }
                        | EditorEffect::CommitComputedFeatureRadius { feature, .. }
                        | EditorEffect::RestoreComputedFeatureRadius { feature, .. }
                        if *feature == metadata.feature
                )),
            "a destroyed preview owner must not emit later radius lifecycle effects"
        );
        assert!(up_after_clear.is_empty());
    }

    #[test]
    fn transactional_feature_pick_rolls_back_duplicate_and_keeps_prior_preview() {
        let mut fixture = computed_fillet_editor_fixture();
        let snapshot = fixture
            .coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let mut state = FeatureAuthoringState::default();
        assert!(matches!(
            state.activate(
                &snapshot,
                fixture.coordinator.session().design_document(),
                FeatureAuthoringTool::Fillet,
                &[],
            ),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));
        let first = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Point(fixture.points[1]), None)],
                "first corner",
            )
            .expect("first preview transaction");
        assert!(matches!(
            first.outcome,
            FeatureAuthoringOutcome::PreviewRequested {
                ref candidate,
                ..
            } if candidate.corners().len() == 1
        ));
        let prior_metadata = first.preview.expect("first preview metadata");
        let prior_state = state.clone();

        let duplicate = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Point(fixture.points[1]), None)],
                "duplicate corner",
            )
            .expect_err("duplicate candidate must be rejected");

        assert!(matches!(
            duplicate,
            CoordinatorError::ComputedFeatureDocument(ComputedFeatureDocumentError::InvalidField {
                field: "corner parents",
                ..
            })
        ));
        assert_eq!(state, prior_state);
        assert_eq!(state.completed_corner_count(), 1);
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("prior preview retained")
                .metadata(),
            &prior_metadata
        );

        let retry = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Point(fixture.points[2]), None)],
                "two valid corners",
            )
            .expect("retry with a distinct corner");
        assert!(matches!(
            retry.outcome,
            FeatureAuthoringOutcome::PreviewRequested {
                ref candidate,
                ..
            } if candidate.corners().len() == 2
        ));
        assert!(retry.preview.is_some());
        assert_eq!(state.completed_corner_count(), 2);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn transactional_feature_pick_restores_pending_support_after_crossed_claim_rejection() {
        let mut fixture = computed_fillet_editor_fixture();
        let snapshot = fixture
            .coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let mut state = FeatureAuthoringState::default();
        assert!(matches!(
            state.activate(
                &snapshot,
                fixture.coordinator.session().design_document(),
                FeatureAuthoringTool::Fillet,
                &[],
            ),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));
        assert!(matches!(
            state.set_options(
                &snapshot,
                crate::FeatureAuthoringOptions {
                    fillet_radius: Some(3.0),
                    ..crate::FeatureAuthoringOptions::default()
                },
            ),
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Point(fixture.points[1]), None)],
                "large first corner",
            )
            .expect("locally valid first corner");
        let pending = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Curve(fixture.spans[1]), Some(0.75))],
                "pending second corner",
            )
            .expect("first support of second corner");
        assert!(matches!(
            pending.outcome,
            FeatureAuthoringOutcome::Collecting {
                ref pending,
                ref guidance,
            } if pending.len() == 1
                && guidance.completed_corners == 1
                && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
        ));
        assert!(fixture.coordinator.feature_authoring_preview().is_none());
        let state_before_rejection = state.clone();

        let crossed = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Curve(fixture.spans[2]), Some(0.25))],
                "crossed second corner",
            )
            .expect_err("crossed source claims must reject the whole preview");

        assert!(matches!(
            crossed,
            CoordinatorError::FeatureAuthoringPreviewRejected(
                ComputedFeatureFailure::ConsumedSourceInterval { .. }
                    | ComputedFeatureFailure::EndpointClaimConflict { .. }
            )
        ));
        assert_eq!(state, state_before_rejection);
        assert_eq!(state.completed_corner_count(), 1);
        assert_eq!(
            state.guidance().stage,
            FeatureAuthoringStage::PickSecondFilletCurve
        );
        assert!(fixture.coordinator.feature_authoring_preview().is_none());

        assert!(matches!(
            state.set_options(
                &snapshot,
                crate::FeatureAuthoringOptions {
                    fillet_radius: Some(1.0),
                    ..state.options()
                },
            ),
            FeatureAuthoringOutcome::Collecting {
                ref pending,
                ..
            } if pending.len() == 1
        ));
        let recovered = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Curve(fixture.spans[2]), Some(0.25))],
                "recovered second corner",
            )
            .expect("retry after reducing the shared radius");
        assert!(matches!(
            recovered.outcome,
            FeatureAuthoringOutcome::PreviewRequested {
                ref candidate,
                ..
            } if candidate.corners().len() == 2
        ));
        assert!(recovered.preview.is_some());
        assert_eq!(state.completed_corner_count(), 2);
        assert_eq!(state.guidance().stage, FeatureAuthoringStage::PreviewReady);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn computed_selection_mutations_and_native_failure_recovery_are_atomic() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&fixture.coordinator, fixture.points[1..=2].iter().copied());
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let coordinator = &mut fixture.coordinator;
        let corners = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &coordinator
                .feature_document()
                .feature(feature)
                .expect("feature")
                .definition;
            fillet
                .corners
                .iter()
                .map(|corner| corner.id)
                .collect::<Vec<_>>()
        };
        let first_owner = ComputedCornerRef {
            feature,
            corner: corners[0],
        };
        let sketch_identity = coordinator.session().design_identity();
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(first_owner)]);
        coordinator
            .set_selected_suppressed(sketch_identity, true)
            .expect("set-wide suppression from corner selection");
        assert!(matches!(
            coordinator
                .computed_snapshot()
                .expect("suppressed output")
                .feature_evaluations()[0]
                .state,
            ComputedFeatureEvaluationState::Suppressed
        ));
        assert_eq!(coordinator.session().design_identity(), sketch_identity);
        coordinator.undo().expect("undo suppression");
        assert!(matches!(
            coordinator
                .computed_snapshot()
                .expect("restored output")
                .feature_evaluations()[0]
                .state,
            ComputedFeatureEvaluationState::Current { .. }
        ));

        let feature_identity = coordinator.feature_document().identity();
        coordinator.editor_mut().set_selection([
            SelectionItem::FeatureCorner(first_owner),
            SelectionItem::Curve(fixture.spans[0]),
        ]);
        assert!(coordinator.delete_selected(sketch_identity).is_err());
        assert_eq!(coordinator.feature_document().identity(), feature_identity);
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(first_owner)]);
        coordinator
            .delete_selected(sketch_identity)
            .expect("delete one corner");
        let ComputedFeatureDefinition::FilletSet(remaining) = &coordinator
            .feature_document()
            .feature(feature)
            .expect("remaining set")
            .definition;
        assert_eq!(remaining.corners.len(), 1);
        coordinator.undo().expect("restore deleted corner");
        let ComputedFeatureDefinition::FilletSet(restored) = &coordinator
            .feature_document()
            .feature(feature)
            .expect("restored set")
            .definition;
        assert_eq!(restored.corners.len(), 2);
        let source_delete_edges = coordinator
            .computed_snapshot()
            .expect("output before source deletion")
            .edges()
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let source_delete_evaluation = coordinator
            .computed_snapshot()
            .expect("output before source deletion")
            .evaluation_revision();

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(fixture.spans[0])]);
        coordinator
            .delete_selected(sketch_identity)
            .expect("delete native parent");
        assert!(matches!(
            coordinator
                .computed_snapshot()
                .expect("failed output")
                .feature_evaluations()[0]
                .state,
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::MissingSource { .. }
            }
        ));
        let problems = coordinator.computed_feature_problems();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].feature, Some(feature));
        assert_eq!(problems[0].scope, EditorProblemScope::Targeted);
        assert!(!problems[0].corners.is_empty());
        assert!(!problems[0].sources.is_empty());
        coordinator.undo().expect("restore native parent");
        assert!(coordinator.computed_feature_problems().is_empty());
        let recovered = coordinator.computed_snapshot().expect("recovered output");
        assert!(matches!(
            computed_feature_state(recovered, feature),
            ComputedFeatureEvaluationState::Current { .. }
        ));
        let ComputedFeatureDefinition::FilletSet(recovered_set) = &coordinator
            .feature_document()
            .feature(feature)
            .expect("same restored feature")
            .definition;
        assert_eq!(
            recovered_set
                .corners
                .iter()
                .map(|corner| corner.id)
                .collect::<Vec<_>>(),
            corners
        );
        assert!(recovered.evaluation_revision().raw() > source_delete_evaluation.raw());
        assert!(
            source_delete_edges
                .iter()
                .all(|old| recovered.edge(*old).is_none())
        );

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(feature)]);
        coordinator
            .delete_selected(coordinator.session().design_identity())
            .expect("delete complete set");
        assert!(coordinator.feature_document().feature(feature).is_none());
        assert_eq!(
            coordinator.computed_profile_boundary(),
            ComputedProfileBoundary::BaseOnly
        );
    }

    #[test]
    fn last_corner_delete_history_preserves_ids_and_allocator_high_water() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let ComputedFeatureDefinition::FilletSet(set) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("single-corner set")
            .definition;
        let [corner] = set.corners.as_slice() else {
            panic!("expected one Fillet corner")
        };
        let corner = corner.id;
        let persistent_high_water = fixture
            .coordinator
            .feature_document()
            .lifecycle_high_water();
        let evaluation_high_water = fixture
            .coordinator
            .computed_evaluation_allocator
            .high_water();

        fixture
            .coordinator
            .remove_computed_corner(
                fixture.coordinator.feature_document().identity(),
                ComputedCornerRef { feature, corner },
            )
            .expect("delete final corner");
        assert!(
            fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .is_none()
        );

        for recovery in ["first Undo", "second Undo"] {
            fixture.coordinator.undo().expect(recovery);
            let ComputedFeatureDefinition::FilletSet(restored) = &fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .expect("same set restored")
                .definition;
            assert_eq!(restored.corners.len(), 1);
            assert_eq!(restored.corners[0].id, corner);
            assert!(matches!(
                computed_feature_state(
                    fixture
                        .coordinator
                        .computed_snapshot()
                        .expect("restored output"),
                    feature
                ),
                ComputedFeatureEvaluationState::Current { .. }
            ));
            let feature_high_water = fixture
                .coordinator
                .feature_document()
                .lifecycle_high_water();
            assert!(
                feature_high_water.allocator.next_feature_id.raw()
                    >= persistent_high_water.allocator.next_feature_id.raw()
            );
            assert!(
                feature_high_water.allocator.next_corner_id.raw()
                    >= persistent_high_water.allocator.next_corner_id.raw()
            );
            assert!(
                fixture
                    .coordinator
                    .computed_evaluation_allocator
                    .high_water()
                    .next_revision
                    .raw()
                    >= evaluation_high_water.next_revision.raw()
            );
            fixture
                .coordinator
                .redo()
                .expect("redo final-corner deletion");
            assert!(
                fixture
                    .coordinator
                    .feature_document()
                    .feature(feature)
                    .is_none()
            );
        }
    }

    #[test]
    fn persistence_checkpoint_captures_rebased_history_and_transient_evaluation_high_water() {
        let mut fixture = computed_fillet_editor_fixture();
        let first_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        apply_grouped_fillet(&mut fixture.coordinator, &first_candidate);
        let second_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[2]]);
        let removed_by_undo = apply_grouped_fillet(&mut fixture.coordinator, &second_candidate);

        fixture.coordinator.undo().expect("undo second feature");
        assert!(
            fixture
                .coordinator
                .feature_document()
                .feature(removed_by_undo)
                .is_none()
        );
        let frozen_history = fixture.coordinator.checkpoint().clone();
        let retry_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[2]]);
        fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &retry_candidate,
                "cancelled preview",
            )
            .expect("transient computed preview");
        fixture.coordinator.clear_feature_authoring_preview();

        let persisted = fixture
            .coordinator
            .persistence_checkpoint()
            .expect("current persistence checkpoint");
        assert_eq!(
            persisted.revisions(),
            fixture.coordinator.session().revision_high_water()
        );
        assert_eq!(
            ComputedFeatureDocument::from_json(persisted.feature_json())
                .expect("persisted feature document"),
            *fixture.coordinator.feature_document()
        );
        assert_eq!(
            persisted.feature_lifecycle_high_water(),
            fixture
                .coordinator
                .feature_document()
                .lifecycle_high_water()
        );
        assert_eq!(
            persisted.computed_evaluation_high_water(),
            fixture
                .coordinator
                .computed_evaluation_allocator
                .high_water()
        );
        assert!(
            persisted
                .feature_lifecycle_high_water()
                .allocator
                .next_feature_id
                .raw()
                > frozen_history
                    .feature_lifecycle_high_water()
                    .allocator
                    .next_feature_id
                    .raw()
        );
        assert!(
            persisted
                .computed_evaluation_high_water()
                .next_revision
                .raw()
                > frozen_history
                    .computed_evaluation_high_water()
                    .next_revision
                    .raw()
        );
    }

    #[test]
    fn adjacent_independent_sets_isolate_suppression_and_deletion() {
        let mut fixture = computed_fillet_editor_fixture();
        let first_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let first = apply_grouped_fillet(&mut fixture.coordinator, &first_candidate);
        let second_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[2]]);
        let second = apply_grouped_fillet(&mut fixture.coordinator, &second_candidate);
        let assert_current = |coordinator: &RetainedEditorCoordinator, feature| {
            assert!(matches!(
                computed_feature_state(
                    coordinator.computed_snapshot().expect("computed output"),
                    feature
                ),
                ComputedFeatureEvaluationState::Current { .. }
            ));
        };
        assert_current(&fixture.coordinator, first);
        assert_current(&fixture.coordinator, second);

        fixture
            .coordinator
            .set_computed_feature_suppressed(
                fixture.coordinator.feature_document().identity(),
                first,
                true,
            )
            .expect("suppress first independent set");
        assert!(matches!(
            computed_feature_state(
                fixture
                    .coordinator
                    .computed_snapshot()
                    .expect("suppressed output"),
                first
            ),
            ComputedFeatureEvaluationState::Suppressed
        ));
        assert_current(&fixture.coordinator, second);

        fixture
            .coordinator
            .set_computed_feature_suppressed(
                fixture.coordinator.feature_document().identity(),
                first,
                false,
            )
            .expect("restore first independent set");
        fixture
            .coordinator
            .remove_computed_feature(fixture.coordinator.feature_document().identity(), second)
            .expect("delete second independent set");
        assert!(
            fixture
                .coordinator
                .feature_document()
                .feature(second)
                .is_none()
        );
        assert_current(&fixture.coordinator, first);
        fixture
            .coordinator
            .undo()
            .expect("undo independent deletion");
        assert_current(&fixture.coordinator, first);
        assert_current(&fixture.coordinator, second);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn every_source_point_drag_previews_commits_and_recovers_current_computed_output() {
        let cases = [
            (0_usize, [12.0, -6.0]),
            (1, [-8.0, 10.0]),
            (2, [9.0, -8.0]),
            (3, [-11.0, 7.0]),
        ];
        for (case, (point_index, screen_delta)) in cases.into_iter().enumerate() {
            let mut fixture = computed_fillet_editor_fixture();
            let candidate = grouped_fillet_candidate(
                &fixture.coordinator,
                fixture.points[1..=2].iter().copied(),
            );
            let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
            let point = fixture.points[point_index];
            let ComputedFeatureDefinition::FilletSet(set) = &fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .expect("published Fillet set")
                .definition;
            let persistent_corners = set
                .corners
                .iter()
                .map(|corner| corner.id)
                .collect::<Vec<_>>();
            let initial_evaluation = fixture
                .coordinator
                .computed_snapshot()
                .expect("initial computed output")
                .evaluation_revision();
            let scene = current_computed_scene(&fixture.coordinator);
            let press = scene
                .points
                .iter()
                .find(|candidate| candidate.id == point)
                .expect("source point in scene")
                .screen_position;
            let moved = ScreenPoint {
                x: press.x + screen_delta[0],
                y: press.y + screen_delta[1],
            };
            let pointer_id = 800 + u64::try_from(case).expect("small case index");
            let pointer = |position| PointerInput {
                pointer_id,
                position,
                modifiers: Modifiers::default(),
            };
            let _ = fixture.coordinator.pointer_down(&scene, pointer(press));
            let requests = fixture
                .coordinator
                .editor_mut()
                .pointer_move(&scene, pointer(moved));
            let [
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point: requested,
                    model_position,
                },
            ] = requests.as_slice()
            else {
                panic!("case {case} must request one projected source move")
            };
            assert_eq!(*requested, point);
            let preview_effects = fixture.coordinator.resolve_projected_point_move(
                *pointer_id,
                *request_id,
                *requested,
                *model_position,
            );
            assert!(matches!(
                preview_effects.as_slice(),
                [EditorEffect::PreviewPointMove { point: previewed, .. }] if *previewed == point
            ));
            let preview_evaluation = match fixture.coordinator.computed_scene_state() {
                ComputedSceneState::Current { expected, snapshot } => {
                    assert_eq!(*expected, snapshot.input());
                    assert_eq!(
                        expected.sketch,
                        fixture
                            .coordinator
                            .solved_preview_session()
                            .expect("solved source preview")
                            .prepared_input()
                    );
                    assert!(matches!(
                        computed_feature_state(snapshot, feature),
                        ComputedFeatureEvaluationState::Current { .. }
                    ));
                    snapshot.evaluation_revision()
                }
                state => panic!("case {case} withheld valid computed preview: {state:?}"),
            };
            assert!(preview_evaluation.raw() > initial_evaluation.raw());

            let expected_design = fixture.coordinator.session().design_identity();
            let release = fixture.coordinator.editor_mut().pointer_up(
                &scene,
                expected_design,
                pointer(moved),
            );
            assert!(matches!(
                release.as_slice(),
                [
                    EditorEffect::CommitPointMove { point: committed, .. },
                    EditorEffect::ClearPointPreview,
                ] if *committed == point
            ));
            let committed_mutation = fixture
                .coordinator
                .apply_editor_effect(&release[0])
                .expect("commit source point preview")
                .expect("source point mutation");
            assert!(
                committed_mutation.published_accepted.is_some(),
                "case {case} did not publish accepted source geometry"
            );
            assert!(
                fixture
                    .coordinator
                    .session()
                    .accepted_state_for_current_input()
                    .is_some(),
                "case {case} published an accepted state for stale input"
            );
            fixture
                .coordinator
                .apply_editor_effect(&release[1])
                .expect("clear source point preview");
            let committed_evaluation = match fixture.coordinator.computed_scene_state() {
                ComputedSceneState::Current { expected, snapshot } => {
                    assert_eq!(*expected, snapshot.input());
                    assert!(matches!(
                        computed_feature_state(snapshot, feature),
                        ComputedFeatureEvaluationState::Current { .. }
                    ));
                    snapshot.evaluation_revision()
                }
                state => panic!(
                    "case {case} withheld committed computed output: {state:?}; problems={:#?}",
                    fixture.coordinator.computed_feature_problems()
                ),
            };
            assert!(committed_evaluation.raw() > preview_evaluation.raw());
            let committed_edges = fixture
                .coordinator
                .computed_snapshot()
                .expect("committed output")
                .edges()
                .iter()
                .map(|edge| edge.id)
                .collect::<Vec<_>>();

            fixture.coordinator.undo().expect("undo source point move");
            let recovered = fixture
                .coordinator
                .computed_snapshot()
                .expect("recovered computed output");
            assert!(matches!(
                computed_feature_state(recovered, feature),
                ComputedFeatureEvaluationState::Current { .. }
            ));
            assert!(recovered.evaluation_revision().raw() > committed_evaluation.raw());
            assert!(
                committed_edges
                    .iter()
                    .all(|old| recovered.edge(*old).is_none())
            );
            let ComputedFeatureDefinition::FilletSet(recovered_set) = &fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .expect("same feature after source Undo")
                .definition;
            assert_eq!(
                recovered_set
                    .corners
                    .iter()
                    .map(|corner| corner.id)
                    .collect::<Vec<_>>(),
                persistent_corners
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn accepted_and_rejected_reattempts_refresh_or_withhold_exact_computed_output() {
        let mut accepted_fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&accepted_fixture.coordinator, [accepted_fixture.points[1]]);
        apply_grouped_fillet(&mut accepted_fixture.coordinator, &candidate);
        let old_snapshot = accepted_fixture
            .coordinator
            .computed_snapshot()
            .expect("old output")
            .clone();
        let design = accepted_fixture.coordinator.session().design_identity();
        accepted_fixture
            .coordinator
            .reattempt(design)
            .expect("accepted reattempt");
        let current = accepted_fixture
            .coordinator
            .computed_snapshot()
            .expect("refreshed output");
        assert_ne!(current.input(), old_snapshot.input());
        assert!(current.evaluation_revision().raw() > old_snapshot.evaluation_revision().raw());
        let expected = accepted_fixture
            .coordinator
            .computed_evaluation_input()
            .expect("current exact input");
        assert_eq!(current.input(), expected);
        let accepted = accepted_fixture
            .coordinator
            .session()
            .accepted_state()
            .expect("accepted reattempt state");
        let viewport = Viewport::new([900.0, 700.0], [1.0, 1.0], 50.0).expect("viewport");
        assert!(matches!(
            EditorScene::from_accepted_with_computed(
                accepted.identity().revision().get(),
                accepted.design_identity(),
                accepted.document(),
                accepted_fixture.coordinator.session().design_document(),
                &accepted_fixture
                    .coordinator
                    .session()
                    .accepted_prepared_input()
                    .expect("current accepted reattempt input"),
                &expected,
                &old_snapshot,
                viewport,
                0.5,
            ),
            Err(crate::EditorError::StaleComputedFeatureSnapshot)
        ));
        let mut wrong_policy = expected;
        wrong_policy.policy.max_root_iterations += 1;
        assert!(matches!(
            EditorScene::from_accepted_with_computed(
                accepted.identity().revision().get(),
                accepted.design_identity(),
                accepted.document(),
                accepted_fixture.coordinator.session().design_document(),
                &accepted_fixture
                    .coordinator
                    .session()
                    .accepted_prepared_input()
                    .expect("current accepted reattempt input"),
                &wrong_policy,
                current,
                viewport,
                0.5,
            ),
            Err(crate::EditorError::StaleComputedFeatureSnapshot)
        ));

        let mut rejected_fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&rejected_fixture.coordinator, [rejected_fixture.points[1]]);
        apply_grouped_fillet(&mut rejected_fixture.coordinator, &candidate);
        let old_snapshot = rejected_fixture
            .coordinator
            .computed_snapshot()
            .expect("pre-rejection output")
            .clone();
        let accepted_before = rejected_fixture
            .coordinator
            .session()
            .accepted_state()
            .expect("accepted before rejection")
            .identity();
        let invalid_point = DesignPointId(PersistentId::from_u128(0xffff_ffff_ffff));
        let design = rejected_fixture.coordinator.session().design_identity();
        let invalid_request = rejected_fixture
            .coordinator
            .session()
            .last_attempt()
            .input()
            .candidate_request()
            .with_drag(invalid_point, [1.0, 1.0]);
        rejected_fixture
            .coordinator
            .session
            .reattempt(design, invalid_request)
            .expect("seed rejected request");
        assert!(
            rejected_fixture
                .coordinator
                .session()
                .last_attempt()
                .accepted_state_identity()
                .is_none()
        );
        rejected_fixture
            .coordinator
            .reattempt(design)
            .expect("coordinator rejected reattempt");
        assert_eq!(
            rejected_fixture
                .coordinator
                .session()
                .accepted_state()
                .expect("retained accepted")
                .identity(),
            accepted_before
        );
        assert!(rejected_fixture.coordinator.computed_snapshot().is_none());
        assert!(matches!(
            rejected_fixture.coordinator.computed_scene_state(),
            ComputedSceneState::Withheld
        ));
        assert!(matches!(
            rejected_fixture
                .coordinator
                .computed_feature_problems()
                .as_slice(),
            [ComputedFeatureProblemMetadata {
                scope: EditorProblemScope::Global,
                ..
            }]
        ));
        let retained = rejected_fixture
            .coordinator
            .session()
            .accepted_state()
            .expect("retained accepted geometry");
        assert!(matches!(
            EditorScene::from_accepted_with_computed(
                retained.identity().revision().get(),
                retained.design_identity(),
                retained.document(),
                rejected_fixture.coordinator.session().design_document(),
                &rejected_fixture.coordinator.session().prepared_input(),
                &old_snapshot.input(),
                &old_snapshot,
                viewport,
                0.5,
            ),
            Err(crate::EditorError::StaleComputedFeatureSnapshot)
        ));
    }

    #[test]
    fn failed_source_preview_evaluation_withholds_base_ghost_and_keeps_native_preview() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let mut preview = fixture.coordinator.session().clone();
        let request = preview
            .last_attempt()
            .input()
            .candidate_request()
            .with_drag(fixture.points[0], [-0.5, 0.0]);
        preview
            .reattempt(preview.design_identity(), request)
            .expect("accepted native preview");
        assert!(preview.accepted_state_for_current_input().is_some());

        let mut exhausted = bounded_geometry_control();
        exhausted.limits.document_validation_items = 0;
        fixture
            .coordinator
            .mark_solved_preview_controlled(&preview, exhausted)
            .expect("native preview remains publishable");
        assert!(fixture.coordinator.visible_preview_session().is_some());
        assert!(fixture.coordinator.computed_snapshot().is_none());
        assert!(matches!(
            fixture.coordinator.computed_scene_state(),
            ComputedSceneState::Withheld
        ));
        let visible = fixture
            .coordinator
            .visible_preview_session()
            .and_then(RetainedSketchDocumentSession::accepted_state)
            .expect("visible native preview");
        EditorScene::from_accepted_for_design(
            visible.identity().revision().get(),
            visible.design_identity(),
            visible.document(),
            fixture.coordinator.session().design_document(),
            Viewport::new([900.0, 700.0], [1.0, 1.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("native-only preview scene");

        fixture.coordinator.clear_transient();
        assert!(matches!(
            fixture.coordinator.computed_scene_state(),
            ComputedSceneState::Current { .. }
        ));
        fixture.coordinator.computed_evaluation_allocator =
            ComputedEvaluationAllocator::from_high_water(ComputedEvaluationAllocatorHighWater {
                next_revision: geosolve_sketch_features::ComputedEvaluationRevision::from_raw(
                    u64::MAX,
                ),
            });
        fixture
            .coordinator
            .mark_solved_preview_controlled(&preview, bounded_geometry_control())
            .expect("native preview survives computed setup error");
        assert!(fixture.coordinator.computed_snapshot().is_none());
        assert!(matches!(
            fixture.coordinator.computed_scene_state(),
            ComputedSceneState::Withheld
        ));
    }

    #[test]
    fn feature_preview_token_exhaustion_and_stale_candidates_publish_nothing() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&fixture.coordinator, fixture.points[1..=2].iter().copied());
        let feature_identity = fixture.coordinator.feature_document().identity();
        let history = fixture.coordinator.history_len();
        fixture.coordinator.next_feature_authoring_preview_token = u64::MAX;
        assert!(matches!(
            fixture.coordinator.prepare_feature_authoring_preview(
                feature_identity,
                &candidate,
                "exhausted preview"
            ),
            Err(CoordinatorError::FeatureAuthoringPreviewTokenExhausted)
        ));
        assert_eq!(
            fixture.coordinator.feature_document().identity(),
            feature_identity
        );
        assert_eq!(fixture.coordinator.history_len(), history);
        assert!(fixture.coordinator.feature_authoring_preview().is_none());

        fixture
            .coordinator
            .apply_edit(
                fixture.coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: fixture.points[0],
                    position: [-1.0, 0.0],
                },
            )
            .expect("native edit");
        assert!(matches!(
            fixture.coordinator.prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &candidate,
                "stale preview"
            ),
            Err(CoordinatorError::StaleComputedFeatureCandidate)
        ));
        assert!(fixture.coordinator.feature_document().features().is_empty());
    }

    #[test]
    fn computed_radius_drag_preserves_the_pointer_offset_on_first_move() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let accepted = fixture
            .coordinator
            .session()
            .accepted_state()
            .expect("accepted sketch");
        let viewport = Viewport::new([900.0, 700.0], [1.0, 1.0], 50.0).expect("viewport");
        let scene = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            fixture.coordinator.session().design_document(),
            &fixture
                .coordinator
                .session()
                .accepted_prepared_input()
                .expect("current accepted computed input"),
            &fixture
                .coordinator
                .computed_evaluation_input()
                .expect("exact computed input"),
            fixture
                .coordinator
                .computed_snapshot()
                .expect("computed output"),
            viewport,
            0.5,
        )
        .expect("scene");
        let arc = &scene.computed_curves[0];
        let sample = scene
            .viewport
            .screen_to_model(arc.screen_polyline[arc.screen_polyline.len() / 2]);
        let direction = [
            (sample[0] - arc.center[0]) / arc.radius,
            (sample[1] - arc.center[1]) / arc.radius,
        ];
        let press_model = [
            (arc.radius - 0.05).mul_add(direction[0], arc.center[0]),
            (arc.radius - 0.05).mul_add(direction[1], arc.center[1]),
        ];
        let move_model = [
            0.2_f64.mul_add(direction[0], press_model[0]),
            0.2_f64.mul_add(direction[1], press_model[1]),
        ];
        let pointer = |position| PointerInput {
            pointer_id: 741,
            position,
            modifiers: Modifiers::default(),
        };
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(&scene, pointer(scene.viewport.model_to_screen(press_model)));
        let effects =
            editor.pointer_move(&scene, pointer(scene.viewport.model_to_screen(move_model)));
        let radius = effects
            .iter()
            .find_map(|effect| match effect {
                EditorEffect::PreviewComputedFeatureRadius { radius, .. } => Some(*radius),
                _ => None,
            })
            .expect("radius preview effect");
        assert!(
            (radius - (arc.radius + 0.2)).abs() <= 1.0e-10,
            "the first drag sample jumped from {} to {radius}",
            arc.radius
        );
        let cancelled = editor.cancel();
        assert!(cancelled.iter().any(|effect| matches!(
            effect,
            EditorEffect::RestoreComputedFeatureRadius {
                expected,
                feature,
                radius,
            } if *expected == scene.computed_input.expect("scene input")
                && *feature == arc.owner.feature
                && radius.to_bits() == arc.radius.to_bits()
        )));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn published_computed_arc_pointer_preview_commits_and_undoes_one_history_step() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let initial_radius = fillet_radius(&fixture.coordinator, feature);
        let initial_history = fixture.coordinator.history_len();
        let initial_edges = fixture
            .coordinator
            .computed_snapshot()
            .expect("initial computed output")
            .edges()
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let scene = current_computed_scene(&fixture.coordinator);
        let arc = scene.computed_curves[0].clone();
        let sample = scene
            .viewport
            .screen_to_model(arc.screen_polyline[arc.screen_polyline.len() / 2]);
        let direction = [
            (sample[0] - arc.center[0]) / arc.radius,
            (sample[1] - arc.center[1]) / arc.radius,
        ];
        let press_model = [
            (arc.radius - 0.05).mul_add(direction[0], arc.center[0]),
            (arc.radius - 0.05).mul_add(direction[1], arc.center[1]),
        ];
        let move_model = [
            0.2_f64.mul_add(direction[0], press_model[0]),
            0.2_f64.mul_add(direction[1], press_model[1]),
        ];
        let pointer = |position| PointerInput {
            pointer_id: 991,
            position,
            modifiers: Modifiers::default(),
        };
        let _ = fixture
            .coordinator
            .pointer_down(&scene, pointer(scene.viewport.model_to_screen(press_model)));
        let preview_effects = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(scene.viewport.model_to_screen(move_model)));
        let [
            EditorEffect::PreviewComputedFeatureRadius {
                expected,
                feature: preview_feature,
                radius: preview_radius,
            },
        ] = preview_effects.as_slice()
        else {
            panic!("published computed arc must emit one radius preview")
        };
        assert_eq!(*preview_feature, feature);
        assert_eq!(
            *expected,
            scene.computed_input.expect("scene computed input")
        );
        fixture
            .coordinator
            .apply_editor_effect(&preview_effects[0])
            .expect("apply non-persistent radius preview");
        assert_eq!(
            fillet_radius(&fixture.coordinator, feature).to_bits(),
            initial_radius.to_bits(),
            "preview must not publish persistent intent"
        );
        let preview_evaluation = match fixture.coordinator.computed_scene_state() {
            ComputedSceneState::Current { expected, snapshot } => {
                assert_eq!(*expected, snapshot.input());
                let radius = snapshot
                    .edges()
                    .iter()
                    .find_map(|edge| match (&edge.geometry, &edge.provenance) {
                        (
                            geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc),
                            ComputedEdgeProvenance::FilletArc { owner, .. },
                        ) if owner.feature == feature => Some(arc.radius),
                        _ => None,
                    })
                    .expect("preview Fillet arc");
                assert_eq!(radius.to_bits(), preview_radius.to_bits());
                snapshot.evaluation_revision()
            }
            state => panic!("valid published radius preview was withheld: {state:?}"),
        };

        let expected_design = fixture.coordinator.session().design_identity();
        let release = fixture.coordinator.editor_mut().pointer_up(
            &scene,
            expected_design,
            pointer(scene.viewport.model_to_screen(move_model)),
        );
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitComputedFeatureRadius { feature: committed, .. },
                EditorEffect::ClearComputedFeaturePreview,
            ] if *committed == feature
        ));
        fixture
            .coordinator
            .apply_editor_effect(&release[0])
            .expect("commit exact published radius");
        fixture
            .coordinator
            .apply_editor_effect(&release[1])
            .expect("clear published radius preview");
        assert_eq!(fixture.coordinator.history_len(), initial_history + 1);
        assert_eq!(
            fillet_radius(&fixture.coordinator, feature).to_bits(),
            preview_radius.to_bits()
        );
        let committed = fixture
            .coordinator
            .computed_snapshot()
            .expect("committed computed output");
        assert!(committed.evaluation_revision().raw() > preview_evaluation.raw());
        assert!(matches!(
            computed_feature_state(committed, feature),
            ComputedFeatureEvaluationState::Current { .. }
        ));
        let committed_evaluation = committed.evaluation_revision();
        let committed_edges = committed
            .edges()
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();

        fixture.coordinator.undo().expect("undo radius gesture");
        assert_eq!(
            fillet_radius(&fixture.coordinator, feature).to_bits(),
            initial_radius.to_bits()
        );
        let restored = fixture
            .coordinator
            .computed_snapshot()
            .expect("Undo restored computed output");
        assert!(matches!(
            computed_feature_state(restored, feature),
            ComputedFeatureEvaluationState::Current { .. }
        ));
        assert!(restored.evaluation_revision().raw() > committed_evaluation.raw());
        assert!(
            initial_edges
                .iter()
                .all(|old| restored.edge(*old).is_none())
        );
        assert!(
            committed_edges
                .iter()
                .all(|old| restored.edge(*old).is_none())
        );
    }
}
