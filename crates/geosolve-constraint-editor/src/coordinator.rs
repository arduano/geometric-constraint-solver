// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained-design lifecycle coordination for presentation adapters.

use std::collections::{BTreeSet, HashSet};

use geosolve_sketch::{
    ContactBranchEdit, ContactDomain, ContactId, ContactNeighborhood, CurveDefinition, CurveId,
    CurveSpan, DesignPointId, DesignScalarId, DocumentAngleOrientation, DocumentArcSweep,
    DocumentCommandEffect, DocumentConstraintDefinition, DocumentCurveContinuity,
    DocumentCurveControlAvailability, DocumentCurveControlError, DocumentCurveControlId,
    DocumentCurveControlProjection, DocumentCurveControlTarget,
    DocumentCurveControlWithholdingReason, DocumentCurveCurvatureRelation,
    DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode,
    DocumentDragLocalityPlan, DocumentEdit, DocumentElementId, DocumentExternalBindingId,
    DocumentFilletTrimEndpoint, DocumentHyperbolaBranch, DocumentMeasurementCatalog,
    DocumentMeasurementProvenance, DocumentMeasurementValue, DocumentObjectId,
    DocumentParameterTarget, DocumentRationalConicControl, DocumentRuntimeMap,
    DocumentSessionError, DocumentSolveRequest, DocumentSourceId, DocumentSourceOwner,
    ExternalFeatureKindV1, ExternalSnapshotSet, ExternalTopologyDigest, GeometryRole,
    GeometryRoleEdit, OperationCheckpoint, OperationControl, OperationController, OperationLimits,
    OperationOutcome, OperationReport, OperationWork, ParameterBatch, PreparedSketchInput,
    PreparedSketchOperation, PreparedSketchPatch, PreparedSketchSnapshot,
    RetainedSketchDocumentSession, RuntimeCurve, ScalarDomain, ScalarUnit,
    SketchAcceptedDocumentRedundancy, SketchAcceptedStateIdentity, SketchAttemptFailure,
    SketchAttemptFailureKind, SketchAttemptIdentity, SketchBound, SketchDatum,
    SketchDesignIdentity, SketchDocument, SketchLifecycleRevisionHighWater,
    SketchPersistentIdentityHighWater, SketchSolveResult, SketchSource, SolveRejection,
    TangentOrientation,
};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedEdgeId, ComputedEdgeProvenance, ComputedEvaluationAllocator,
    ComputedEvaluationAllocatorHighWater, ComputedFeatureAuthoringError,
    ComputedFeatureAuthoringSnapshot, ComputedFeatureCornerId, ComputedFeatureDocument,
    ComputedFeatureDocumentError, ComputedFeatureDocumentIdentity, ComputedFeatureEvaluationError,
    ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureId,
    ComputedFeatureLifecycleHighWater, ComputedFeatureReanchorError, ComputedFeatureSnapshot,
    ComputedFeatureSnapshotError, ComputedFilletContactReseedRequest,
    ComputedFilletCornerAlternative, ComputedFilletCornerAlternativeKind,
    ComputedFilletParentIndex, ContinuedComputedFilletCorner, NativeCurveSpanSource,
    NewComputedFilletCorner,
};
use thiserror::Error;

use crate::feature_authoring::resolve_feature_item_picks;
use crate::{
    ActionChoice, AuthoringApplication, AuthoringOperand, AuthoringOptions, AuthoringState,
    AuthoringTool, ComputedFilletContinuationLimit, ComputedFilletContinuationLimitKind,
    ComputedFilletContinuationStatus, ComputedFilletInteractionSample, ConstraintActionRequest,
    ConstraintEditor, ConstraintIntent, ConstraintRelationChoice, ConstructionCommitPlan,
    ConstructionCommitResult, ConstructionCommitToken, ConstructionProposal,
    ConstructionRelationProvenance, ConstructionResult, CurveControlPreviewRequestDisposition,
    DimensionActionRequest, DimensionKind, DraftAuthoringInput, DraftInferenceInput, EditorEffect,
    EditorScene, FeatureAuthoringCandidate, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringPick, FeatureAuthoringState, FeatureAuthoringTool, FeatureAuthoringWarningKind,
    GeometryInteractionPolicy, PickTolerance, PointGestureSnapshot, PointerInput,
    ProjectedDragRequestDisposition, ResolvedConstraintKind, SceneFilletAction,
    SceneFilletActionAvailability, SceneFilletActionControlGeometry, SceneFilletActionId,
    ScreenPoint, SelectionItem,
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
const FILLET_RETAINED_ARROW_LENGTH_PIXELS: f64 = 22.0;

/// Shared finite work envelope for accepted preview and computed-feature
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
    evaluate_computed_features_continuing(session, features, allocator, control, None)
}

fn evaluate_computed_features_continuing(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    allocator: &mut ComputedEvaluationAllocator,
    control: OperationControl,
    previous: Option<&ComputedFeatureSnapshot>,
) -> Result<OperationOutcome<ComputedFeatureSnapshot>, CoordinatorError> {
    let captured = if let Some(previous) = previous {
        ComputedFeatureEvaluationSnapshot::capture_continuing_from(
            session,
            features,
            ComputedFeatureEvaluationPolicy::default(),
            previous,
        )?
    } else {
        ComputedFeatureEvaluationSnapshot::capture(
            session,
            features,
            ComputedFeatureEvaluationPolicy::default(),
        )?
    };
    Ok(captured.prepare(allocator)?.execute(control)?)
}

fn capture_computed_features_continuing(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    previous: Option<&ComputedFeatureSnapshot>,
) -> Result<ComputedFeatureEvaluationSnapshot, CoordinatorError> {
    Ok(if let Some(previous) = previous {
        ComputedFeatureEvaluationSnapshot::capture_continuing_from(
            session,
            features,
            ComputedFeatureEvaluationPolicy::default(),
            previous,
        )?
    } else {
        ComputedFeatureEvaluationSnapshot::capture(
            session,
            features,
            ComputedFeatureEvaluationPolicy::default(),
        )?
    })
}

/// A recorded native edit may carry only the contact-frame refresh derived
/// from that same edit. It must never smuggle an ordinary feature mutation
/// (radius, suppression, ownership, topology, side, trim direction or sweep)
/// into deterministic replay.
fn recorded_transition_is_reanchor_only(
    before: &ComputedFeatureDocument,
    after: &ComputedFeatureDocument,
) -> bool {
    if after.validate().is_err()
        || before.id() != after.id()
        || before.sketch_document() != after.sketch_document()
        || after.revision() <= before.revision()
        || before.allocator_high_water() != after.allocator_high_water()
        || before.features().len() != after.features().len()
    {
        return false;
    }
    before
        .features()
        .iter()
        .zip(after.features())
        .all(|(before_feature, after_feature)| {
            if before_feature.id != after_feature.id
                || before_feature.label != after_feature.label
                || before_feature.suppressed != after_feature.suppressed
            {
                return false;
            }
            let (
                geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(before_fillet),
                geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(after_fillet),
            ) = (&before_feature.definition, &after_feature.definition);
            before_fillet.radius.to_bits() == after_fillet.radius.to_bits()
                && before_fillet.corners.len() == after_fillet.corners.len()
                && before_fillet.corners.iter().zip(&after_fillet.corners).all(
                    |(before_corner, after_corner)| {
                        before_corner.id == after_corner.id
                            && before_corner.endpoint_order == after_corner.endpoint_order
                            && before_corner.sweep == after_corner.sweep
                            && [
                                (before_corner.first, after_corner.first),
                                (before_corner.second, after_corner.second),
                            ]
                            .into_iter()
                            .all(|(before_parent, after_parent)| {
                                before_parent.source == after_parent.source
                                    && before_parent.normal_side == after_parent.normal_side
                                    && before_parent.retained_endpoint
                                        == after_parent.retained_endpoint
                            })
                    },
                )
        })
}

fn computed_feature_states_match_for_durable_reanchor(
    continued: &ComputedFeatureSnapshot,
    cold: &ComputedFeatureSnapshot,
) -> bool {
    continued.edges().len() == cold.edges().len()
        && continued
            .edges()
            .iter()
            .zip(cold.edges())
            .all(|(continued, cold)| {
                continued.id.ordinal == cold.id.ordinal
                    && continued.role == cold.role
                    && continued.geometry == cold.geometry
                    && continued.provenance == cold.provenance
            })
        && continued.construction_fragments().len() == cold.construction_fragments().len()
        && continued
            .construction_fragments()
            .iter()
            .zip(cold.construction_fragments())
            .all(|(continued, cold)| {
                continued.id.ordinal == cold.id.ordinal
                    && continued.source == cold.source
                    && continued.interval == cold.interval
                    && continued.source_role == cold.source_role
                    && continued.provenance == cold.provenance
            })
        && continued.replaced_sources() == cold.replaced_sources()
        && continued.feature_evaluations().len() == cold.feature_evaluations().len()
        && continued
            .feature_evaluations()
            .iter()
            .zip(cold.feature_evaluations())
            .all(|(continued, cold)| {
                if continued.feature != cold.feature {
                    return false;
                }
                match (&continued.state, &cold.state) {
                    (
                        ComputedFeatureEvaluationState::Current {
                            corner_edges: continued,
                        },
                        ComputedFeatureEvaluationState::Current { corner_edges: cold },
                    ) => {
                        continued.len() == cold.len()
                            && continued.iter().zip(cold).all(
                                |((continued_corner, continued_edge), (cold_corner, cold_edge))| {
                                    continued_corner == cold_corner
                                        && continued_edge.ordinal == cold_edge.ordinal
                                },
                            )
                    }
                    (
                        ComputedFeatureEvaluationState::Suppressed,
                        ComputedFeatureEvaluationState::Suppressed,
                    ) => true,
                    (
                        ComputedFeatureEvaluationState::Failed { failure: continued },
                        ComputedFeatureEvaluationState::Failed { failure: cold },
                    ) => continued == cold,
                    _ => false,
                }
            })
}

fn computed_feature_document_semantics_match(
    expected: &ComputedFeatureDocument,
    candidate: &ComputedFeatureDocument,
) -> bool {
    expected.id() == candidate.id()
        && expected.sketch_document() == candidate.sketch_document()
        && expected.allocator_high_water() == candidate.allocator_high_water()
        && expected.features() == candidate.features()
}

#[derive(Clone, Debug, PartialEq)]
enum RecordedComputedFeatureDisposition {
    Current,
    Suppressed,
    Failed(ComputedFeatureFailure),
}

fn recorded_computed_feature_dispositions(
    snapshot: &ComputedFeatureSnapshot,
) -> Vec<(ComputedFeatureId, RecordedComputedFeatureDisposition)> {
    snapshot
        .feature_evaluations()
        .iter()
        .map(|evaluation| {
            let disposition = match &evaluation.state {
                ComputedFeatureEvaluationState::Current { .. } => {
                    RecordedComputedFeatureDisposition::Current
                }
                ComputedFeatureEvaluationState::Suppressed => {
                    RecordedComputedFeatureDisposition::Suppressed
                }
                ComputedFeatureEvaluationState::Failed { failure } => {
                    RecordedComputedFeatureDisposition::Failed(failure.clone())
                }
            };
            (evaluation.feature, disposition)
        })
        .collect()
}

/// Transcript replay must reproduce every durable prepared-input semantic while
/// deliberately rebinding the process-local prepared-state epoch and the
/// candidate solver's previous-state-preference bit. The epoch authenticates
/// off-thread work in one live coordinator, while the preference records how a
/// projected release reached geometry that replay independently reconstructs.
/// Neither is persisted intent. The exact drag target, retained publication
/// request, solver policy and every activation/parameter/external stamp remain
/// bound.
fn prepared_sketch_inputs_match_for_replay(
    expected: &PreparedSketchInput,
    candidate: &PreparedSketchInput,
) -> bool {
    let expected_input = (*expected).attempt_input();
    let candidate_input = (*candidate).attempt_input();
    expected_input.design_identity() == candidate_input.design_identity()
        && expected_input.candidate_request().drag == candidate_input.candidate_request().drag
        && expected_input.publication_request() == candidate_input.publication_request()
        && expected_input.solver_config() == candidate_input.solver_config()
        && expected_input.effective_activation_revision()
            == candidate_input.effective_activation_revision()
        && expected_input.activation_digest() == candidate_input.activation_digest()
        && expected_input.parameter_revision() == candidate_input.parameter_revision()
        && expected_input.parameter_digest() == candidate_input.parameter_digest()
        && expected_input.external_snapshot_set_revision()
            == candidate_input.external_snapshot_set_revision()
        && expected_input.external_snapshot_set_digest()
            == candidate_input.external_snapshot_set_digest()
        && (*expected).latest_attempt_identity() == (*candidate).latest_attempt_identity()
        && (*expected).accepted_state_identity() == (*candidate).accepted_state_identity()
        && (*expected).accepted_revision_high_water() == (*candidate).accepted_revision_high_water()
}

/// Turns one authenticated continued preview into ordinary persistent feature
/// state and proves that an evaluation with no transient continuation hints
/// reproduces the same feature/corner dispositions and exact contact metadata.
fn evaluate_durable_computed_reanchor(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    allocator: &mut ComputedEvaluationAllocator,
    continued: &ComputedFeatureSnapshot,
) -> Result<(ComputedFeatureDocument, ComputedFeatureSnapshot), CoordinatorError> {
    if session.accepted_prepared_input() != Some(continued.input().sketch)
        || continued.input().features != features.identity()
    {
        return Err(CoordinatorError::StaleComputedFeatureCandidate);
    }
    let reanchored = continued.reanchored_feature_document(features)?;
    let evaluated =
        evaluate_computed_features(session, &reanchored, allocator, bounded_geometry_control())?;
    let OperationOutcome::Completed { value: cold, .. } = evaluated else {
        return Err(CoordinatorError::ComputedFeatureWorkStopped);
    };
    if !computed_feature_states_match_for_durable_reanchor(continued, &cold) {
        return Err(CoordinatorError::ComputedFeatureReanchorNotDurable);
    }
    let independently_reanchored = cold.reanchored_feature_document(&reanchored)?;
    if !computed_feature_document_semantics_match(&reanchored, &independently_reanchored) {
        return Err(CoordinatorError::ComputedFeatureReanchorNotDurable);
    }
    Ok((reanchored, cold))
}

fn computed_feature_preview_invalidations(
    features: &ComputedFeatureDocument,
    previous: &ComputedFeatureSnapshot,
    candidate: &ComputedFeatureSnapshot,
) -> Vec<ComputedFeatureProblemMetadata> {
    previous
        .feature_evaluations()
        .iter()
        .filter(|evaluation| {
            matches!(
                evaluation.state,
                ComputedFeatureEvaluationState::Current { .. }
            )
        })
        .filter_map(|previous| {
            let candidate = candidate
                .feature_evaluations()
                .iter()
                .find(|candidate| candidate.feature == previous.feature);
            match candidate.map(|candidate| &candidate.state) {
                Some(ComputedFeatureEvaluationState::Current { .. }) => None,
                Some(ComputedFeatureEvaluationState::Failed { failure }) => {
                    let mut problem =
                        computed_feature_problem(features, previous.feature, failure);
                    let prefix = if computed_fillet_failure_limit(failure).kind
                        == ComputedFilletContinuationLimitKind::DomainBoundary
                    {
                        "Parent limit"
                    } else {
                        "Fillet movement limit"
                    };
                    problem.message = format!(
                        "{prefix}: holding the last valid position because {failure}"
                    );
                    Some(problem)
                }
                Some(ComputedFeatureEvaluationState::Suppressed) | None => {
                    Some(ComputedFeatureProblemMetadata {
                        feature: Some(previous.feature),
                        corners: Vec::new(),
                        sources: Vec::new(),
                        scope: EditorProblemScope::Targeted,
                        message: "Fillet movement is held at the last valid position because complete computed output is unavailable"
                            .into(),
                    })
                }
            }
        })
        .collect()
}

fn computed_preview_global_limit(message: impl Into<String>) -> ComputedFeatureProblemMetadata {
    ComputedFeatureProblemMetadata {
        feature: None,
        corners: Vec::new(),
        sources: Vec::new(),
        scope: EditorProblemScope::Global,
        message: message.into(),
    }
}

fn computed_preview_stopped_problems(
    features: &ComputedFeatureDocument,
    previous: Option<&ComputedFeatureSnapshot>,
    message: impl Into<String>,
) -> Vec<ComputedFeatureProblemMetadata> {
    let message = message.into();
    let Some(previous) = previous else {
        return vec![computed_preview_global_limit(message)];
    };
    let current = previous
        .feature_evaluations()
        .iter()
        .filter(|evaluation| {
            matches!(
                evaluation.state,
                ComputedFeatureEvaluationState::Current { .. }
            )
        })
        .map(|evaluation| evaluation.feature)
        .collect::<Vec<_>>();
    let [feature] = current.as_slice() else {
        return vec![computed_preview_global_limit(message)];
    };
    let Some(feature_value) = features.feature(*feature) else {
        return vec![computed_preview_global_limit(message)];
    };
    let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
        &feature_value.definition;
    let mut corners = fillet
        .corners
        .iter()
        .map(|corner| corner.id)
        .collect::<Vec<_>>();
    corners.sort_unstable();
    corners.dedup();
    let mut sources = fillet
        .corners
        .iter()
        .flat_map(|corner| [corner.first.source, corner.second.source])
        .collect::<Vec<_>>();
    sources.sort_unstable();
    sources.dedup();
    vec![ComputedFeatureProblemMetadata {
        feature: Some(*feature),
        corners,
        sources,
        scope: EditorProblemScope::Targeted,
        message,
    }]
}

fn evaluate_computed_features_in_controller_continuing(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    allocator: &mut ComputedEvaluationAllocator,
    controller: &mut OperationController,
    previous: Option<&ComputedFeatureSnapshot>,
) -> Result<Option<ComputedFeatureSnapshot>, CoordinatorError> {
    let value = capture_computed_features_continuing(session, features, previous)?
        .prepare(allocator)?
        .execute_in_controller(controller)?;
    Ok(value)
}

fn evaluate_computed_features_in_controller(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    allocator: &mut ComputedEvaluationAllocator,
    controller: &mut OperationController,
) -> Result<Option<ComputedFeatureSnapshot>, CoordinatorError> {
    evaluate_computed_features_in_controller_continuing(
        session, features, allocator, controller, None,
    )
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

fn computed_fillet_limit(
    kind: ComputedFilletContinuationLimitKind,
    message: impl Into<String>,
) -> ComputedFilletContinuationLimit {
    ComputedFilletContinuationLimit {
        kind,
        message: message.into(),
    }
}

fn computed_fillet_authoring_limit(
    error: &ComputedFeatureAuthoringError,
) -> ComputedFilletContinuationLimit {
    let kind = match error {
        ComputedFeatureAuthoringError::SingularParents
        | ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity => {
            ComputedFilletContinuationLimitKind::BranchFold
        }
        ComputedFeatureAuthoringError::InvalidRadius
        | ComputedFeatureAuthoringError::NoLocalRoot
        | ComputedFeatureAuthoringError::SideCorrectionUnavailable
        | ComputedFeatureAuthoringError::AmbiguousRetainedEndpoint
        | ComputedFeatureAuthoringError::InvalidContactReseed => {
            ComputedFilletContinuationLimitKind::DomainBoundary
        }
        ComputedFeatureAuthoringError::OffsetSingularity => {
            ComputedFilletContinuationLimitKind::OffsetSingularity
        }
        ComputedFeatureAuthoringError::AmbiguousLocalRoot => {
            ComputedFilletContinuationLimitKind::AmbiguousLocalRoot
        }
        ComputedFeatureAuthoringError::Evaluation(_) => {
            ComputedFilletContinuationLimitKind::WorkStopped
        }
        ComputedFeatureAuthoringError::NonFinitePick
        | ComputedFeatureAuthoringError::StalePick
        | ComputedFeatureAuthoringError::DuplicateSource
        | ComputedFeatureAuthoringError::UnsupportedSameCurvePair
        | ComputedFeatureAuthoringError::UnsupportedCurvedPair
        | ComputedFeatureAuthoringError::UnsupportedSourceTopology
        | ComputedFeatureAuthoringError::UncertifiedCurvedBranch
        | ComputedFeatureAuthoringError::InvalidResolvedGeometry
        | ComputedFeatureAuthoringError::InvalidContinuationState
        | ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity => {
            ComputedFilletContinuationLimitKind::LossOfRegularity
        }
        _ => ComputedFilletContinuationLimitKind::LossOfRegularity,
    };
    computed_fillet_limit(kind, error.to_string())
}

fn computed_fillet_failure_limit(
    failure: &ComputedFeatureFailure,
) -> ComputedFilletContinuationLimit {
    let kind = match failure {
        ComputedFeatureFailure::NoLocalRoot { .. }
        | ComputedFeatureFailure::ConsumedSourceInterval { .. } => {
            ComputedFilletContinuationLimitKind::DomainBoundary
        }
        ComputedFeatureFailure::AmbiguousLocalRoot { .. } => {
            ComputedFilletContinuationLimitKind::AmbiguousLocalRoot
        }
        ComputedFeatureFailure::SingularParents { .. }
        | ComputedFeatureFailure::UncertifiedBranch { .. } => {
            ComputedFilletContinuationLimitKind::BranchFold
        }
        ComputedFeatureFailure::OffsetSingularity { .. } => {
            ComputedFilletContinuationLimitKind::OffsetSingularity
        }
        ComputedFeatureFailure::MissingSource { .. }
        | ComputedFeatureFailure::AssociationOwnedSource { .. }
        | ComputedFeatureFailure::MultiIntervalSource { .. }
        | ComputedFeatureFailure::InvalidParentState { .. }
        | ComputedFeatureFailure::UnsupportedCurvedPair { .. }
        | ComputedFeatureFailure::InvalidGeometry { .. }
        | ComputedFeatureFailure::EndpointClaimConflict { .. } => {
            ComputedFilletContinuationLimitKind::LossOfRegularity
        }
        _ => ComputedFilletContinuationLimitKind::LossOfRegularity,
    };
    computed_fillet_limit(kind, failure.to_string())
}

fn feature_authoring_warning_limit(
    kind: FeatureAuthoringWarningKind,
    message: &str,
) -> Option<ComputedFilletContinuationLimit> {
    let limit = match kind {
        FeatureAuthoringWarningKind::StalePick | FeatureAuthoringWarningKind::MissingObject => {
            return None;
        }
        FeatureAuthoringWarningKind::InvalidRadius
        | FeatureAuthoringWarningKind::AmbiguousTrimSide => {
            ComputedFilletContinuationLimitKind::DomainBoundary
        }
        FeatureAuthoringWarningKind::SingularFillet => {
            ComputedFilletContinuationLimitKind::BranchFold
        }
        FeatureAuthoringWarningKind::AmbiguousFilletRoot => {
            ComputedFilletContinuationLimitKind::AmbiguousLocalRoot
        }
        FeatureAuthoringWarningKind::WorkStopped => {
            ComputedFilletContinuationLimitKind::WorkStopped
        }
        FeatureAuthoringWarningKind::WrongOperandKind
        | FeatureAuthoringWarningKind::NonFinitePick
        | FeatureAuthoringWarningKind::DuplicateSupport
        | FeatureAuthoringWarningKind::IncompleteCorner
        | FeatureAuthoringWarningKind::UnsupportedCurveFamily
        | FeatureAuthoringWarningKind::UnsupportedFilletPair => {
            ComputedFilletContinuationLimitKind::LossOfRegularity
        }
    };
    Some(computed_fillet_limit(limit, message))
}

fn coordinator_computed_fillet_limit(
    error: &CoordinatorError,
) -> Option<ComputedFilletContinuationLimit> {
    match error {
        CoordinatorError::ComputedFeatureAuthoring(error) => {
            Some(computed_fillet_authoring_limit(error))
        }
        CoordinatorError::FeatureAuthoringPreviewRejected(failure) => {
            Some(computed_fillet_failure_limit(failure))
        }
        CoordinatorError::ComputedFeatureWorkStopped
        | CoordinatorError::ComputedFeatureEvaluation(_) => Some(computed_fillet_limit(
            ComputedFilletContinuationLimitKind::WorkStopped,
            error.to_string(),
        )),
        _ => None,
    }
}

fn computed_fillet_alternative_action_id(
    kind: ComputedFilletCornerAlternativeKind,
) -> Option<SceneFilletActionId> {
    match kind {
        ComputedFilletCornerAlternativeKind::Current => None,
        ComputedFilletCornerAlternativeKind::NormalSides { first, second } => {
            Some(SceneFilletActionId::LocalAlternative { first, second })
        }
        ComputedFilletCornerAlternativeKind::RetainedEndpoint { parent, .. } => {
            Some(match parent {
                ComputedFilletParentIndex::First => {
                    SceneFilletActionId::ReverseFirstRetainedDirection
                }
                ComputedFilletParentIndex::Second => {
                    SceneFilletActionId::ReverseSecondRetainedDirection
                }
            })
        }
        ComputedFilletCornerAlternativeKind::ComplementaryArc => {
            Some(SceneFilletActionId::ComplementaryArc)
        }
    }
}

fn select_computed_fillet_alternative(
    alternatives: &[ComputedFilletCornerAlternative],
    requested: SceneFilletActionId,
) -> Option<&ComputedFilletCornerAlternative> {
    alternatives.iter().find(|alternative| {
        computed_fillet_alternative_action_id(alternative.kind) == Some(requested)
    })
}

fn computed_fillet_alternative_is_current(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    owner: ComputedCornerRef,
    radius: f64,
    current_corners: &[(ComputedFeatureCornerId, NewComputedFilletCorner)],
    replacement: NewComputedFilletCorner,
) -> bool {
    let corners = current_corners
        .iter()
        .map(|(id, corner)| {
            (
                *id,
                if *id == owner.corner {
                    replacement
                } else {
                    *corner
                },
            )
        })
        .collect::<Vec<_>>();
    let mut candidate = features.clone();
    if candidate
        .replace_fillet_set(owner.feature, radius, corners)
        .is_err()
    {
        return false;
    }
    let mut allocator = ComputedEvaluationAllocator::default();
    let Ok(outcome) = evaluate_computed_features(
        session,
        &candidate,
        &mut allocator,
        computed_feature_authoring_control(),
    ) else {
        return false;
    };
    let OperationOutcome::Completed {
        value: evaluated, ..
    } = outcome
    else {
        return false;
    };
    matches!(
        evaluated
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == owner.feature)
            .map(|evaluation| &evaluation.state),
        Some(ComputedFeatureEvaluationState::Current { .. })
    )
}

fn computed_fillet_retained_control_geometry(
    scene: &EditorScene,
    snapshot: &ComputedFeatureAuthoringSnapshot,
    continued: &ContinuedComputedFilletCorner,
    parent: ComputedFilletParentIndex,
) -> Option<SceneFilletActionControlGeometry> {
    let (intent, contact) = match parent {
        ComputedFilletParentIndex::First => (continued.corner.first, continued.arc.contacts[0]),
        ComputedFilletParentIndex::Second => (continued.corner.second, continued.arc.contacts[1]),
    };
    let jet = snapshot
        .sketch_document()
        .evaluate_curve_jet(intent.source.span, contact.total_parameter)
        .ok()?;
    let tangent = [jet.first_derivative.x, jet.first_derivative.y];
    let tangent_norm = tangent[0].hypot(tangent[1]);
    if !tangent_norm.is_finite() || tangent_norm <= 0.0 {
        return None;
    }
    let sign = match intent.retained_endpoint {
        DocumentFilletTrimEndpoint::Start => -1.0,
        DocumentFilletTrimEndpoint::End => 1.0,
    };
    let model_direction = [
        sign * tangent[0] / tangent_norm,
        sign * tangent[1] / tangent_norm,
    ];
    let model_length = FILLET_RETAINED_ARROW_LENGTH_PIXELS / scene.viewport.pixels_per_model_unit;
    let model_end = [
        model_length.mul_add(model_direction[0], contact.position[0]),
        model_length.mul_add(model_direction[1], contact.position[1]),
    ];
    Some(SceneFilletActionControlGeometry {
        model_anchor: contact.position,
        model_direction,
        screen_start: scene.viewport.model_to_screen(contact.position),
        screen_end: scene.viewport.model_to_screen(model_end),
    })
}

fn computed_fillet_alternative_control_geometry(
    scene: &EditorScene,
    arc: &geosolve_sketch_features::ComputedCircularArc,
) -> Option<SceneFilletActionControlGeometry> {
    const CONTROL_LENGTH_PIXELS: f64 = 22.0;

    let tau = std::f64::consts::TAU;
    let delta = match arc.sweep {
        geosolve_sketch::DocumentArcSweep::CounterClockwise => {
            (arc.end_angle - arc.start_angle).rem_euclid(tau)
        }
        geosolve_sketch::DocumentArcSweep::Clockwise => {
            -(arc.start_angle - arc.end_angle).rem_euclid(tau)
        }
    };
    if !arc.center.into_iter().all(f64::is_finite)
        || !arc.radius.is_finite()
        || arc.radius <= 0.0
        || !delta.is_finite()
        || delta.abs() <= f64::EPSILON
    {
        return None;
    }
    let middle_angle = (0.5 * delta).mul_add(1.0, arc.start_angle);
    let model_direction = [middle_angle.cos(), middle_angle.sin()];
    let model_anchor = [
        arc.radius.mul_add(model_direction[0], arc.center[0]),
        arc.radius.mul_add(model_direction[1], arc.center[1]),
    ];
    let model_length = CONTROL_LENGTH_PIXELS / scene.viewport.pixels_per_model_unit;
    let model_end = [
        model_length.mul_add(model_direction[0], model_anchor[0]),
        model_length.mul_add(model_direction[1], model_anchor[1]),
    ];
    Some(SceneFilletActionControlGeometry {
        model_anchor,
        model_direction,
        screen_start: scene.viewport.model_to_screen(model_anchor),
        screen_end: scene.viewport.model_to_screen(model_end),
    })
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
    sketch_identity_high_water: SketchPersistentIdentityHighWater,
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

    /// Never-reuse persistent sketch object and spline-span allocator metadata.
    #[must_use]
    pub const fn sketch_identity_high_water(&self) -> &SketchPersistentIdentityHighWater {
        &self.sketch_identity_high_water
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
    /// Point-observable hard freedom preserved by the gesture's locality anchors.
    ///
    /// Scalar-only nullspace freedom cannot be represented by a point target and is excluded.
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
    /// Intrinsic Cartesian datums may participate as relation operands but are
    /// immutable reference geometry rather than editable document objects.
    ProtectedDatum,
    MissingObject,
    InvalidSpan,
    /// The selected semantic operands already resolve to one underlying object,
    /// so adding the requested relation would be tautological.
    SameSemanticOperand,
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
    CurveControl(DocumentCommandEffect),
    Construction(ConstructionResult),
    InferredConstruction(ConstructionCommitResult),
}

/// Typed retained mutation emitted by one complete headless authoring application.
#[derive(Clone, Debug)]
pub enum AuthoringMutation {
    Constraint(MutationOutcome<geosolve_sketch::DocumentConstraintId>),
    Dimension(MutationOutcome<DocumentDimensionId>),
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

/// Coordinator-owned result of one pointer press while computed-feature
/// authoring is active.
///
/// A rendered current preview arc may explicitly own a radius gesture even
/// where its native parent is also inside the authoring hit tolerance. Every
/// other painted target retains the existing bounded native-pick transaction.
#[derive(Clone, Debug, PartialEq)]
pub enum FeatureAuthoringPointerDownOutcome {
    RadiusGesture {
        effects: Vec<EditorEffect>,
    },
    NativePick {
        transaction: Box<FeatureAuthoringTransaction>,
    },
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
    /// Collector state paired with `radius_origin` only while a pointer owns a
    /// live authoring-radius gesture.
    radius_origin_state: Option<Box<FeatureAuthoringState>>,
    accepted_contact_sample: Option<ComputedFilletEditSample>,
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

/// Stable selected-curve family used by presentation-neutral inspectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurvePropertyFamily {
    Line,
    Polyline,
    Circle,
    CircularArc,
    QuadraticBezier,
    CubicBezier,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    BSpline,
    Nurbs,
}

impl CurvePropertyFamily {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Line => "Line",
            Self::Polyline => "Polyline",
            Self::Circle => "Circle",
            Self::CircularArc => "Circular arc",
            Self::QuadraticBezier => "Quadratic Bezier",
            Self::CubicBezier => "Cubic Bezier",
            Self::Ellipse => "Ellipse",
            Self::EllipticalArc => "Elliptical arc",
            Self::RationalQuadraticConic => "Rational quadratic conic",
            Self::Parabola => "Parabola segment",
            Self::Hyperbola => "Hyperbola segment",
            Self::BSpline => "B-spline",
            Self::Nurbs => "NURBS",
        }
    }
}

/// Semantic role of one exact numeric selected-curve property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveNumericPropertyKind {
    Radius,
    MinorAxisRatio,
    TrimStart,
    TrimEnd,
    SemiConjugate,
    RationalWeight,
    NurbsWeight { ordinal: u32 },
}

impl CurveNumericPropertyKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Radius => "Radius",
            Self::MinorAxisRatio => "Minor axis ratio",
            Self::TrimStart => "Start trim",
            Self::TrimEnd => "End trim",
            Self::SemiConjugate => "Conjugate size",
            Self::RationalWeight => "Middle weight",
            Self::NurbsWeight { .. } => "Control weight",
        }
    }
}

/// Exact persisted scalar exposed by the selected-curve inspector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveNumericPropertyMetadata {
    pub kind: CurveNumericPropertyKind,
    pub scalar: DesignScalarId,
    pub value: f64,
    pub unit: ScalarUnit,
    pub domain: ScalarDomain,
    /// Exact current ownership of this scalar mutation.
    pub availability: DocumentCurveControlAvailability,
}

/// Complete exact property surface for one selected native curve.
///
/// Canvas controls remain transient. This DTO exposes only existing persistent parameters and
/// explicit discrete branch state, so a presentation adapter never needs to inspect curve
/// definitions or infer which scalar a field edits.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectedCurvePropertyMetadata {
    pub curve: CurveId,
    pub label: String,
    pub family: CurvePropertyFamily,
    /// Curve-wide ownership used by rational-middle and discrete branch edits.
    pub direct_edit_availability: DocumentCurveControlAvailability,
    pub numeric: Vec<CurveNumericPropertyMetadata>,
    pub sweep: Option<DocumentArcSweep>,
    pub hyperbola_branch: Option<DocumentHyperbolaBranch>,
    pub rational_control: Option<DocumentRationalConicControl>,
    pub nurbs_gauge: Option<DesignScalarId>,
    /// Whether changing the NURBS gauge may rescale every owned weight now.
    pub nurbs_gauge_availability: Option<DocumentCurveControlAvailability>,
    pub degree: Option<u32>,
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

/// Aggregate persistent role state for the complete native curves represented
/// by the current span selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryRoleSelectionState {
    Profile,
    Construction,
    Mixed,
}

/// Exact derived branch-state promotion paired with one recorded native edit.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedComputedFeatureTransition {
    edit: DocumentEdit,
    before_sketch: PreparedSketchInput,
    after_sketch: PreparedSketchInput,
    before: ComputedFeatureDocumentIdentity,
    after: ComputedFeatureDocument,
    dispositions: Vec<(ComputedFeatureId, RecordedComputedFeatureDisposition)>,
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
    /// Absolute branch-preserving whole-set replacement. This is the durable
    /// replay form for radius, contact and explicit branch manipulation.
    SetComputedFilletConfiguration {
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
        radius: f64,
        corners: Vec<(ComputedFeatureCornerId, NewComputedFilletCorner)>,
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
        computed_features: Option<Box<RecordedComputedFeatureTransition>>,
    },
    Construction {
        expected: SketchDesignIdentity,
        proposal: ConstructionProposal,
        role: GeometryRole,
    },
    ConstructionPlan {
        expected: Box<PreparedSketchInput>,
        plan: ConstructionCommitPlan,
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
    CurveControl(#[from] DocumentCurveControlError),
    #[error(transparent)]
    Editor(#[from] crate::EditorError),
    #[error(transparent)]
    ComputedFeatureDocument(#[from] ComputedFeatureDocumentError),
    #[error(transparent)]
    ComputedFeatureEvaluation(#[from] ComputedFeatureEvaluationError),
    #[error(transparent)]
    ComputedFeatureSnapshot(#[from] ComputedFeatureSnapshotError),
    #[error(transparent)]
    ComputedFeatureAuthoring(#[from] ComputedFeatureAuthoringError),
    #[error(transparent)]
    ComputedFeatureReanchor(#[from] ComputedFeatureReanchorError),
    #[error("selected operands cannot construct the requested dimension")]
    IncompatibleDimension,
    #[error("invalid typed action input: {0}")]
    InvalidActionInput(&'static str),
    #[error("curve property action does not match the exact current curve selection")]
    CurvePropertySelectionMismatch,
    #[error("selected curve property is unavailable: {0:?}")]
    CurvePropertyUnavailable(DocumentCurveControlWithholdingReason),
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
    #[error("auto-constrained construction did not publish a newly accepted state")]
    InferredConstructionNotAccepted,
    #[error("auto-constrained construction would add redundant source {inferred_source:?}")]
    RedundantInferredConstruction { inferred_source: DocumentSourceId },
    #[error("auto-constrained construction effect does not match the editor's pending plan")]
    InferredConstructionCommitMismatch,
    #[error("auto-constrained construction input is no longer the current accepted input")]
    StaleInferredConstructionInput,
    #[error("computed-feature authoring pick is unavailable: {0:?}")]
    FeatureAuthoringPick(crate::FeatureAuthoringWarningKind),
    #[error("computed-feature candidate does not match the current exact input")]
    StaleComputedFeatureCandidate,
    #[error("computed-feature evaluation stopped before publication")]
    ComputedFeatureWorkStopped,
    #[error("computed-feature preview would invalidate previously Current output")]
    ComputedFeaturePreviewInvalidated,
    #[error("computed-feature contact re-anchor is not independently cold-reproducible")]
    ComputedFeatureReanchorNotDurable,
    #[error("computed-feature authoring preview was rejected: {0}")]
    FeatureAuthoringPreviewRejected(ComputedFeatureFailure),
    #[error("computed-feature authoring preview is missing or does not match")]
    FeatureAuthoringPreviewMismatch,
    #[error("computed-feature authoring preview identity space is exhausted")]
    FeatureAuthoringPreviewTokenExhausted,
    #[error("computed-feature authoring transition was rejected: {0}")]
    FeatureAuthoringTransitionRejected(String),
    #[error("computed Fillet action is unavailable: {0}")]
    ComputedFilletActionUnavailable(String),
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
    computed_fillet_preview: Option<ComputedFilletEditPreview>,
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
    curve_control_continuation: Option<CurveControlContinuation>,
    projected_drag_work: Option<ProjectedDragWorkEvidence>,
    feature_authoring_preview: Option<FeatureAuthoringPreview>,
    next_feature_authoring_preview_token: u64,
}

/// Fully prepared, infallibly publishable state for one accepted inferred
/// construction.  Building this value performs every fallible checkpoint and
/// computed-output step against clones, so the live coordinator remains exact
/// if staging fails.
struct StagedConstructionPublication {
    session: RetainedSketchDocumentSession,
    computed_evaluation_allocator: ComputedEvaluationAllocator,
    computed_input: Option<geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    computed_snapshot: Option<ComputedFeatureSnapshot>,
    computed_evaluation_problem: Option<String>,
    checkpoint: RestoreCheckpoint,
}

/// Exact non-persistent whole-feature candidate accepted during one published
/// Fillet manipulation. Publication consumes this value; requested-but-invalid
/// pointer samples can never be reconstructed at release time.
#[derive(Clone, Debug)]
struct ComputedFilletEditPreview {
    origin: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    feature: ComputedFeatureId,
    radius: f64,
    sample: ComputedFilletEditSample,
    corners: Vec<(ComputedFeatureCornerId, NewComputedFilletCorner)>,
    features: ComputedFeatureDocument,
    snapshot: ComputedFeatureSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ComputedFilletEditSample {
    Radius(f64),
    Contact {
        owner: ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    },
}

struct ComputedFilletAlternatives {
    radius: f64,
    corners: Vec<(ComputedFeatureCornerId, NewComputedFilletCorner)>,
    values: Vec<ComputedFilletCornerAlternative>,
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
    last_valid_computed_snapshot: Option<ComputedFeatureSnapshot>,
    computed_problems: Vec<ComputedFeatureProblemMetadata>,
}

/// One exact independently accepted prepared candidate retained for pointer release.
#[derive(Debug)]
struct CurveControlPreparedPreview {
    request_id: u64,
    control: DocumentCurveControlId,
    model_position: [f64; 2],
    edit: DocumentEdit,
    patch: PreparedSketchPatch,
    computed_snapshot: ComputedFeatureSnapshot,
    computed_allocator: ComputedEvaluationAllocator,
}

/// Latest independently accepted semantic result of one pointer sample.
#[derive(Debug)]
enum CurveControlAcceptedSample {
    /// The pointer inverse-mapped exactly to the gesture's retained starting value.
    Unchanged {
        request_id: u64,
        control: DocumentCurveControlId,
        model_position: [f64; 2],
    },
    /// A changed retained input produced a complete independently accepted candidate.
    Changed(Box<CurveControlPreparedPreview>),
}

impl CurveControlAcceptedSample {
    const fn request_id(&self) -> u64 {
        match self {
            Self::Unchanged { request_id, .. } => *request_id,
            Self::Changed(preview) => preview.request_id,
        }
    }

    const fn control(&self) -> DocumentCurveControlId {
        match self {
            Self::Unchanged { control, .. } => *control,
            Self::Changed(preview) => preview.control,
        }
    }

    const fn model_position(&self) -> [f64; 2] {
        match self {
            Self::Unchanged { model_position, .. } => *model_position,
            Self::Changed(preview) => preview.model_position,
        }
    }
}

#[derive(Debug)]
enum CurveControlPreparedSample {
    Rejected,
    Accepted(CurveControlAcceptedSample),
}

/// Gesture-local immutable base snapshot plus its latest accepted prepared patch.
#[derive(Debug)]
struct CurveControlContinuation {
    pointer_id: u64,
    control: DocumentCurveControlId,
    expected: SketchDesignIdentity,
    base: PreparedSketchInput,
    snapshot: PreparedSketchSnapshot,
    computed_allocator: ComputedEvaluationAllocator,
    last_request_id: Option<u64>,
    last_accepted: Option<CurveControlAcceptedSample>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SolvedPreviewPublicationPolicy {
    /// Publish the independently accepted native preview even when computed
    /// output must be withheld. This preserves the general preview API's
    /// established native-only fallback contract.
    AllowNativeOnly,
    /// A projected source drag advances only when native and every previously
    /// Current computed feature form one complete scene.
    RequireCompleteComputedScene,
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
            computed_fillet_preview: None,
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
            curve_control_continuation: None,
            projected_drag_work: None,
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

    /// Adds exact branch-preserving Fillet manipulation affordances to an
    /// already constructed composite scene.
    ///
    /// Radius sensitivity and all local action applicability come from the
    /// feature domain. The scene derives only screen-space rails, handles and
    /// dashed arc polylines. Corners near a fold remain renderable, but no
    /// unvalidated radius rail is invented for them.
    ///
    /// `action_items` controls the comparatively expensive bounded continuation
    /// and alternative enumeration. Radius/contact handles, actions and ghosts
    /// are attached only for selected corners, selected feature sets or the
    /// current grouped-authoring candidate.
    ///
    /// # Errors
    ///
    /// Rejects stale scene provenance or malformed presentation geometry.
    #[allow(
        clippy::too_many_lines,
        reason = "one auditable scene boundary joins exact feature intent, validated rails and action DTOs"
    )]
    pub fn populate_computed_fillet_affordances(
        &self,
        scene: &mut EditorScene,
        action_items: &[SelectionItem],
        chord_tolerance_pixels: f64,
    ) -> Result<(), CoordinatorError> {
        let expected = scene
            .computed_input
            .ok_or(CoordinatorError::StaleComputedFeatureCandidate)?;
        if matches!(
            self.editor.active_pointer_gesture(),
            Some(crate::ActivePointerGesture {
                kind: crate::ActivePointerGestureKind::FilletRadius
                    | crate::ActivePointerGestureKind::FilletContact,
                ..
            })
        ) {
            let interaction_origin = self
                .computed_fillet_preview
                .as_ref()
                .map(|preview| preview.origin)
                .or_else(|| {
                    self.feature_authoring_preview
                        .as_ref()
                        .filter(|preview| preview.radius_origin_state.is_some())
                        .map(|preview| preview.radius_origin.metadata.input)
                });
            if let Some(origin) = interaction_origin
                && origin != expected
            {
                scene.set_computed_fillet_interaction_origin(origin)?;
            }
        }
        let features = self
            .computed_feature_document_for_input(&expected)
            .ok_or(CoordinatorError::StaleComputedFeatureCandidate)?;
        let source = self.visible_preview_session().unwrap_or(&self.session);
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(source)?;
        let active_continuation_status = self.editor.computed_fillet_continuation_status().cloned();
        let owners = scene
            .computed_curves
            .iter()
            .map(|curve| curve.owner)
            .collect::<Vec<_>>();
        for owner in owners {
            let selected = action_items.iter().any(|item| {
                matches!(item, SelectionItem::FeatureCorner(current) if *current == owner)
                    || matches!(item, SelectionItem::Feature(current) if *current == owner.feature)
            });
            let authoring = self
                .feature_authoring_preview
                .as_ref()
                .is_some_and(|preview| {
                    preview.snapshot.input() == expected && preview.corner_index(owner).is_some()
                });
            if !selected && !authoring {
                continue;
            }
            let Some(feature) = features.feature(owner.feature) else {
                return Err(CoordinatorError::StaleComputedFeatureCandidate);
            };
            let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
                &feature.definition;
            let Some(corner) = fillet
                .corners
                .iter()
                .find(|corner| corner.id == owner.corner)
            else {
                return Err(CoordinatorError::StaleComputedFeatureCandidate);
            };
            let continuation = match snapshot.continue_fillet_corner(
                corner.without_id(),
                fillet.radius,
                fillet.radius,
                ComputedFeatureEvaluationPolicy::default(),
                bounded_geometry_control(),
            ) {
                Ok(OperationOutcome::Completed { value, .. }) => value,
                Ok(stopped) => {
                    let status = active_continuation_status
                        .as_ref()
                        .filter(|status| status.owner == owner)
                        .cloned()
                        .unwrap_or_else(|| ComputedFilletContinuationStatus {
                            expected,
                            owner,
                            sample: ComputedFilletInteractionSample::Radius(fillet.radius),
                            limit: computed_fillet_limit(
                                ComputedFilletContinuationLimitKind::WorkStopped,
                                format!(
                                    "Fillet continuation stopped before a current rail: {:?}",
                                    stopped.report().stopping_reason
                                ),
                            ),
                        });
                    scene.set_computed_fillet_continuation_status(owner, Some(status))?;
                    continue;
                }
                Err(error) => {
                    let status = active_continuation_status
                        .as_ref()
                        .filter(|status| status.owner == owner)
                        .cloned()
                        .unwrap_or_else(|| ComputedFilletContinuationStatus {
                            expected,
                            owner,
                            sample: ComputedFilletInteractionSample::Radius(fillet.radius),
                            limit: computed_fillet_authoring_limit(&error),
                        });
                    scene.set_computed_fillet_continuation_status(owner, Some(status))?;
                    continue;
                }
            };
            let mut affected_owners = fillet
                .corners
                .iter()
                .map(|corner| ComputedCornerRef {
                    feature: owner.feature,
                    corner: corner.id,
                })
                .collect::<Vec<_>>();
            affected_owners.sort_unstable();
            scene.attach_computed_fillet_radius_rail(
                owner,
                continuation.sensitivity.center_derivative,
                affected_owners,
            )?;
            if let Some(status) = active_continuation_status
                .as_ref()
                .filter(|status| status.owner == owner)
                .cloned()
            {
                scene.set_computed_fillet_continuation_status(owner, Some(status))?;
            }
            let actions_requested = selected || authoring;
            if !actions_requested {
                continue;
            }
            let alternatives = match snapshot.local_fillet_corner_alternatives(
                corner.without_id(),
                fillet.radius,
                ComputedFeatureEvaluationPolicy::default(),
                computed_feature_authoring_control(),
            ) {
                Ok(OperationOutcome::Completed { value, .. }) => value,
                Ok(_) | Err(_) => Vec::new(),
            };
            let current_corners = fillet
                .corners
                .iter()
                .map(|corner| (corner.id, corner.without_id()))
                .collect::<Vec<_>>();
            let mut actions = Vec::new();
            for alternative in alternatives {
                let Some(id) = computed_fillet_alternative_action_id(alternative.kind) else {
                    continue;
                };
                if !computed_fillet_alternative_is_current(
                    source,
                    features,
                    owner,
                    fillet.radius,
                    &current_corners,
                    alternative.resolved.corner,
                ) {
                    continue;
                }
                let (label, control_geometry) = match id {
                    SceneFilletActionId::ReverseFirstRetainedDirection => (
                        "Reverse first retained direction".into(),
                        computed_fillet_retained_control_geometry(
                            scene,
                            &snapshot,
                            &continuation,
                            ComputedFilletParentIndex::First,
                        ),
                    ),
                    SceneFilletActionId::ReverseSecondRetainedDirection => (
                        "Reverse second retained direction".into(),
                        computed_fillet_retained_control_geometry(
                            scene,
                            &snapshot,
                            &continuation,
                            ComputedFilletParentIndex::Second,
                        ),
                    ),
                    SceneFilletActionId::ComplementaryArc => (
                        "Use complementary arc".into(),
                        computed_fillet_alternative_control_geometry(
                            scene,
                            &alternative.resolved.arc,
                        ),
                    ),
                    SceneFilletActionId::LocalAlternative { first, second } => (
                        format!("Use local side branch {first:?}/{second:?}"),
                        computed_fillet_alternative_control_geometry(
                            scene,
                            &alternative.resolved.arc,
                        ),
                    ),
                };
                let polyline = scene.tessellate_computed_fillet_arc(
                    &alternative.resolved.arc,
                    chord_tolerance_pixels,
                )?;
                actions.push(SceneFilletAction {
                    id,
                    owner,
                    label,
                    availability: SceneFilletActionAvailability::Applicable,
                    control_geometry,
                    dashed_alternative_arc: Some(polyline),
                });
            }
            scene.set_fillet_corner_actions(owner, actions)?;
        }
        Ok(())
    }

    fn computed_feature_document_for_input(
        &self,
        input: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
    ) -> Option<&ComputedFeatureDocument> {
        let ComputedSceneState::Current { expected, snapshot } = self.computed_scene_state() else {
            return None;
        };
        if *expected != *input || snapshot.input() != *input {
            return None;
        }
        if let Some(preview) = self.feature_authoring_preview.as_ref()
            && preview.snapshot.input() == *input
            && preview.features.identity() == input.features
        {
            return Some(&preview.features);
        }
        if let Some(preview) = self.computed_fillet_preview.as_ref()
            && preview.snapshot.input() == *input
            && preview.features.identity() == input.features
        {
            return Some(&preview.features);
        }
        (self.features.identity() == input.features).then_some(&self.features)
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

    /// Replaces application selection and immediately revokes any prepared
    /// curve-control candidate owned by the previous selection.
    pub fn set_selection(&mut self, selection: impl IntoIterator<Item = SelectionItem>) {
        let previous = self.editor.selection().to_vec();
        self.editor.set_selection(selection);
        if self.editor.selection() != previous {
            self.clear_curve_control_preview();
        }
    }

    /// Applies one selection click and immediately revokes any prepared
    /// curve-control candidate whose owner was replaced or deselected.
    pub fn select_item(&mut self, item: SelectionItem, modifiers: crate::Modifiers) {
        let previous = self.editor.selection().to_vec();
        self.editor.select_item(item, modifiers);
        if self.editor.selection() != previous {
            self.clear_curve_control_preview();
        }
    }

    /// Atomically replaces the editor's complete geometry interaction policy.
    ///
    /// A point press owns coordinator-local continuation state from pointer-down,
    /// before the editor crosses its drag threshold or emits preview effects. If
    /// the policy transition cancels that press, clear the matching transient
    /// continuation here while preserving durable selection and history.
    pub fn set_geometry_interaction_policy(
        &mut self,
        policy: GeometryInteractionPolicy,
    ) -> Vec<EditorEffect> {
        let before = self.editor.point_gesture_snapshot();
        let effects = self.editor.set_geometry_interaction_policy(policy);
        if before.is_some() && self.editor.point_gesture_snapshot().is_none() {
            self.clear_transient();
        }
        effects
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
        if let Some(preview) = self
            .current_curve_control_continuation()
            .and_then(|gesture| gesture.last_accepted.as_ref())
            .and_then(|sample| match sample {
                CurveControlAcceptedSample::Changed(preview) => Some(preview),
                CurveControlAcceptedSample::Unchanged { .. } => None,
            })
        {
            return preview.patch.preview().accepted_session();
        }
        self.drag_continuation
            .as_ref()
            .and_then(|gesture| gesture.last_accepted_preview.as_ref())
            .or(self.solved_preview.as_ref())
    }

    fn current_curve_control_continuation(&self) -> Option<&CurveControlContinuation> {
        self.curve_control_continuation.as_ref().filter(|gesture| {
            gesture.last_request_id.is_some_and(|request_id| {
                self.editor.curve_control_preview_request_disposition(
                    gesture.pointer_id,
                    request_id,
                    gesture.expected,
                    gesture.control,
                ) == CurveControlPreviewRequestDisposition::Current
            })
        })
    }

    /// Work evidence for the latest projected pointer sample, if one is active.
    #[must_use]
    pub const fn projected_drag_work_evidence(&self) -> Option<&ProjectedDragWorkEvidence> {
        self.projected_drag_work.as_ref()
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

    /// Publishes the exact compatible native item that an unchanged ordinary
    /// constraint/dimension authoring press would consume.
    pub fn pointer_move_authoring(
        &mut self,
        state: &AuthoringState,
        scene: &EditorScene,
        input: PointerInput,
        tolerance: PickTolerance,
    ) -> Vec<EditorEffect> {
        let target = state.hover_item_at_with_policy(
            self.session.design_document(),
            scene,
            input.position,
            tolerance,
            self.editor.geometry_interaction_policy(),
        );
        self.editor.set_authoring_hover_target(target)
    }

    /// Publishes the exact compatible item that an unchanged grouped Fillet
    /// authoring press would consume, without changing the candidate or
    /// retained feature preview. A painted computed corner remains only an
    /// intent hint and must pass the same retained-preview and scene checks as
    /// pointer-down before it can precede native operand picking.
    ///
    /// # Errors
    ///
    /// Rejects when no current accepted feature-authoring snapshot exists or
    /// when a painted computed-corner hint does not match the exact retained
    /// preview, scene provenance, and independently resolved radius hit.
    pub fn pointer_move_feature_authoring(
        &mut self,
        state: &FeatureAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
        painted_item: Option<SelectionItem>,
        tolerance: PickTolerance,
    ) -> Result<Vec<EditorEffect>, CoordinatorError> {
        if let Some(owner) =
            self.validated_feature_authoring_radius_owner(state, scene, painted_item)?
        {
            let target = self
                .editor
                .feature_radius_hover_item(scene, input.position, owner, tolerance)
                .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
            return Ok(self.editor.set_authoring_hover_target(Some(target)));
        }
        let snapshot = self.feature_authoring_snapshot()?;
        let target = state.hover_item_at_with_policy(
            &snapshot,
            snapshot.sketch_document(),
            scene,
            input.position,
            tolerance,
            self.editor.geometry_interaction_policy(),
        );
        Ok(self.editor.set_authoring_hover_target(target))
    }

    fn validated_feature_authoring_radius_owner(
        &self,
        state: &FeatureAuthoringState,
        scene: &EditorScene,
        painted_item: Option<SelectionItem>,
    ) -> Result<Option<ComputedCornerRef>, CoordinatorError> {
        let Some(SelectionItem::FeatureCorner(owner)) = painted_item else {
            return Ok(None);
        };
        let candidate = match state.apply() {
            FeatureAuthoringOutcome::Apply(candidate) => candidate,
            FeatureAuthoringOutcome::ModeEntered(_)
            | FeatureAuthoringOutcome::NoNativeHit(_)
            | FeatureAuthoringOutcome::Collecting { .. }
            | FeatureAuthoringOutcome::PreviewRequested { .. }
            | FeatureAuthoringOutcome::Warning(_)
            | FeatureAuthoringOutcome::CandidateCleared(_)
            | FeatureAuthoringOutcome::ModeExited
            | FeatureAuthoringOutcome::Inactive => {
                return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
            }
        };
        let preview = self
            .feature_authoring_preview
            .as_ref()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        let accepted = self
            .session
            .accepted_state_for_current_input()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        if preview.metadata.feature != owner.feature
            || preview.corner_index(owner).is_none()
            || preview.candidate() != &candidate
            || scene.accepted_revision != accepted.identity().revision().get()
            || scene.design_identity != accepted.design_identity()
            || scene.computed_input != Some(preview.snapshot().input())
        {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        Ok(Some(owner))
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
        let document = snapshot.sketch_document();
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
                    | SelectionItem::Datum(_)
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
        let document = snapshot.sketch_document();
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
        let document = snapshot.sketch_document().clone();
        let mut trial = state.clone();
        let outcome = trial.pick_items(&snapshot, &document, items);
        self.finish_feature_authoring_transaction(state, trial, outcome, label.into())
    }

    /// Arbitrates one pointer press between a painted current-preview radius
    /// grip and the ordinary bounded native Fillet collector.
    ///
    /// `painted_item` is only an interaction-intent hint. A computed corner is
    /// admitted only when it belongs to the exact held preview, the collector's
    /// complete candidate still matches that preview, the scene has current
    /// accepted/computed provenance, and the pointer independently hits that
    /// owner's computed curve. Invalid computed-corner hints are rejected
    /// state-neutrally rather than being reinterpreted as native picks.
    ///
    /// # Errors
    ///
    /// Returns a snapshot, document, evaluation, provisional-feature, or exact
    /// preview mismatch without changing `state` or replacing a held preview.
    #[allow(
        clippy::too_many_arguments,
        reason = "one atomic pointer transaction carries exact scene, hit, policy and label inputs"
    )]
    pub fn transact_feature_authoring_pointer_down(
        &mut self,
        state: &mut FeatureAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
        painted_item: Option<SelectionItem>,
        tolerance: PickTolerance,
        label: impl Into<String>,
    ) -> Result<FeatureAuthoringPointerDownOutcome, CoordinatorError> {
        if let Some(owner) =
            self.validated_feature_authoring_radius_owner(state, scene, painted_item)?
        {
            if self.editor.active_pointer_gesture().is_some() {
                return Ok(FeatureAuthoringPointerDownOutcome::RadiusGesture {
                    effects: Vec::new(),
                });
            }
            let effects = self
                .editor
                .pointer_down_feature_radius(scene, input, owner, tolerance)
                .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
            let preview = self
                .feature_authoring_preview
                .as_mut()
                .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
            *preview.radius_origin = FeatureAuthoringRadiusOrigin {
                candidate: preview.candidate.clone(),
                features: preview.features.clone(),
                snapshot: preview.snapshot.clone(),
                metadata: preview.metadata.clone(),
            };
            preview.radius_origin_state = Some(Box::new(state.clone()));
            preview.accepted_contact_sample = None;
            return Ok(FeatureAuthoringPointerDownOutcome::RadiusGesture { effects });
        }

        self.transact_feature_authoring_pick_at(state, scene, input.position, tolerance, label)
            .map(
                |transaction| FeatureAuthoringPointerDownOutcome::NativePick {
                    transaction: Box::new(transaction),
                },
            )
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
        let document = snapshot.sketch_document().clone();
        let mut trial = state.clone();
        let outcome = trial.pick_at_with_policy(
            &snapshot,
            &document,
            scene,
            position,
            tolerance,
            self.editor.geometry_interaction_policy(),
        );
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

    /// Applies only the shared numeric Fillet radius through absolute
    /// same-branch continuation. A missing host value retains the collector's
    /// initialized/remembered radius.
    ///
    /// This is the M68 numeric counterpart to radius dragging. It deliberately
    /// bypasses relative flip booleans so an explicit branch action cannot be
    /// undone by reconstructing completed corners from their old picks.
    ///
    /// # Errors
    ///
    /// Rejects an invalid radius, stale accepted input, stopped absolute
    /// continuation or non-current whole-feature preview state-neutrally.
    pub fn transact_feature_authoring_radius(
        &mut self,
        state: &mut FeatureAuthoringState,
        radius: Option<f64>,
        label: impl Into<String>,
    ) -> Result<FeatureAuthoringTransaction, CoordinatorError> {
        let radius = radius
            .or(state.options().fillet_radius)
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| {
                CoordinatorError::FeatureAuthoringTransitionRejected(
                    "Fillet radius must be finite and positive".into(),
                )
            })?;
        let snapshot = self.feature_authoring_snapshot()?;
        let mut trial = state.clone();
        let outcome = trial.continue_radius_absolute(&snapshot, radius);
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
            radius_origin_state: None,
            accepted_contact_sample: None,
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
        let radius_origin_state = current.radius_origin_state.clone();
        let accepted_contact_sample = current.accepted_contact_sample;
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
            radius_origin_state,
            accepted_contact_sample,
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

    /// Applies one editor-emitted radius effect to the active grouped Fillet
    /// collector and its exact whole-feature preview as one headless
    /// transaction.
    ///
    /// Presentation adapters must not synchronize authoring options or rebuild
    /// corners themselves. A preview effect advances completed corners by
    /// absolute same-branch continuation, evaluates the whole temporary set,
    /// and acknowledges the exact sample to the pointer state only after both
    /// layers are current. Commit merely accepts that already-held sample;
    /// restore reinstates the exact pointer-down collector and preview.
    ///
    /// # Errors
    ///
    /// Rejects stale ownership, invalid/folded continuation, bounded work or a
    /// non-current whole-feature preview without changing the supplied state or
    /// the previously held exact preview.
    #[allow(
        clippy::too_many_lines,
        reason = "the closed authoring interaction transition table keeps preview, ack, commit and rollback auditable together"
    )]
    pub fn apply_feature_authoring_editor_effect(
        &mut self,
        state: &mut FeatureAuthoringState,
        effect: &EditorEffect,
    ) -> Result<(), CoordinatorError> {
        match effect {
            EditorEffect::PreviewComputedFeatureRadius {
                expected,
                feature,
                radius,
            } => {
                let held = self
                    .feature_authoring_preview
                    .as_ref()
                    .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                if held.metadata.feature != *feature || !held.accepts_radius_input(expected) {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                let mut trial = held
                    .radius_origin_state
                    .as_deref()
                    .cloned()
                    .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                let snapshot = self.feature_authoring_snapshot()?;
                let outcome = trial.continue_radius_absolute(&snapshot, *radius);
                let candidate = match outcome {
                    FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
                    FeatureAuthoringOutcome::Warning(warning) => {
                        if let Some(limit) =
                            feature_authoring_warning_limit(warning.kind, &warning.message)
                        {
                            self.editor.reject_computed_feature_radius_preview(
                                expected, *feature, *radius, limit,
                            );
                        }
                        return Err(CoordinatorError::FeatureAuthoringTransitionRejected(
                            warning.message,
                        ));
                    }
                    FeatureAuthoringOutcome::ModeEntered(_)
                    | FeatureAuthoringOutcome::NoNativeHit(_)
                    | FeatureAuthoringOutcome::Collecting { .. }
                    | FeatureAuthoringOutcome::Apply(_)
                    | FeatureAuthoringOutcome::CandidateCleared(_)
                    | FeatureAuthoringOutcome::ModeExited
                    | FeatureAuthoringOutcome::Inactive => {
                        return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                    }
                };
                let prior_preview = self.feature_authoring_preview.clone();
                if let Err(error) = self.refresh_feature_authoring_preview(*expected, &candidate) {
                    if let Some(limit) = coordinator_computed_fillet_limit(&error) {
                        self.editor.reject_computed_feature_radius_preview(
                            expected, *feature, *radius, limit,
                        );
                    }
                    return Err(error);
                }
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    preview.accepted_contact_sample = None;
                }
                if !self
                    .editor
                    .accept_computed_feature_radius_preview(expected, *feature, *radius)
                {
                    self.feature_authoring_preview = prior_preview;
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                *state = trial;
                Ok(())
            }
            EditorEffect::CommitComputedFeatureRadius {
                expected,
                feature,
                radius,
            } => {
                let preview = self
                    .feature_authoring_preview
                    .as_ref()
                    .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                if preview.metadata.feature != *feature
                    || !preview.accepts_radius_input(expected)
                    || preview.candidate.radius().to_bits() != radius.to_bits()
                {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                self.accept_feature_authoring_radius_preview(*expected, *feature)?;
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    preview.radius_origin_state = None;
                }
                Ok(())
            }
            EditorEffect::RestoreComputedFeatureRadius {
                expected, feature, ..
            } => {
                let origin_state = self
                    .feature_authoring_preview
                    .as_ref()
                    .and_then(|preview| preview.radius_origin_state.clone())
                    .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                self.restore_feature_authoring_radius_preview(*expected, *feature)?;
                *state = *origin_state;
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    preview.radius_origin_state = None;
                }
                Ok(())
            }
            EditorEffect::ClearComputedFeaturePreview => {
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    preview.radius_origin_state = None;
                }
                Ok(())
            }
            EditorEffect::PreviewComputedFeatureContact {
                expected,
                owner,
                parent,
                source,
                parameter,
            } => {
                let (corner_index, prior, radius, mut trial) = {
                    let held = self
                        .feature_authoring_preview
                        .as_ref()
                        .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                    if held.metadata.feature != owner.feature
                        || !held.accepts_radius_input(expected)
                    {
                        return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                    }
                    let corner_index = held
                        .corner_index(*owner)
                        .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                    let prior = held
                        .radius_origin
                        .candidate
                        .corners()
                        .get(corner_index)
                        .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?
                        .corner;
                    let radius = held.radius_origin.candidate.radius();
                    let trial = held
                        .radius_origin_state
                        .as_deref()
                        .cloned()
                        .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                    (corner_index, prior, radius, trial)
                };
                let parent_source = match parent {
                    ComputedFilletParentIndex::First => prior.first.source,
                    ComputedFilletParentIndex::Second => prior.second.source,
                };
                if parent_source != *source || !parameter.is_finite() {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                let snapshot = self.feature_authoring_snapshot()?;
                let outcome = match snapshot.reseed_fillet_contact(
                    ComputedFilletContactReseedRequest {
                        prior,
                        parent: *parent,
                        parameter: *parameter,
                    },
                    radius,
                    ComputedFeatureEvaluationPolicy::default(),
                    computed_feature_authoring_control(),
                ) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        self.editor.reject_computed_feature_contact_preview(
                            expected,
                            *owner,
                            *parent,
                            *source,
                            *parameter,
                            computed_fillet_authoring_limit(&error),
                        );
                        return Err(error.into());
                    }
                };
                let OperationOutcome::Completed {
                    value: continued, ..
                } = outcome
                else {
                    self.editor.reject_computed_feature_contact_preview(
                        expected,
                        *owner,
                        *parent,
                        *source,
                        *parameter,
                        computed_fillet_limit(
                            ComputedFilletContinuationLimitKind::WorkStopped,
                            "Fillet contact continuation exhausted its bounded work envelope",
                        ),
                    );
                    return Err(CoordinatorError::ComputedFeatureWorkStopped);
                };
                let outcome = trial.replace_corner_absolute(&snapshot, corner_index, continued);
                let candidate = match outcome {
                    FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
                    FeatureAuthoringOutcome::Warning(warning) => {
                        if let Some(limit) =
                            feature_authoring_warning_limit(warning.kind, &warning.message)
                        {
                            self.editor.reject_computed_feature_contact_preview(
                                expected, *owner, *parent, *source, *parameter, limit,
                            );
                        }
                        return Err(CoordinatorError::FeatureAuthoringTransitionRejected(
                            warning.message,
                        ));
                    }
                    FeatureAuthoringOutcome::ModeEntered(_)
                    | FeatureAuthoringOutcome::NoNativeHit(_)
                    | FeatureAuthoringOutcome::Collecting { .. }
                    | FeatureAuthoringOutcome::Apply(_)
                    | FeatureAuthoringOutcome::CandidateCleared(_)
                    | FeatureAuthoringOutcome::ModeExited
                    | FeatureAuthoringOutcome::Inactive => {
                        return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                    }
                };
                let prior_preview = self.feature_authoring_preview.clone();
                if let Err(error) = self.refresh_feature_authoring_preview(*expected, &candidate) {
                    if let Some(limit) = coordinator_computed_fillet_limit(&error) {
                        self.editor.reject_computed_feature_contact_preview(
                            expected, *owner, *parent, *source, *parameter, limit,
                        );
                    }
                    return Err(error);
                }
                let sample = ComputedFilletEditSample::Contact {
                    owner: *owner,
                    parent: *parent,
                    source: *source,
                    parameter: *parameter,
                };
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    preview.accepted_contact_sample = Some(sample);
                }
                if !self.editor.accept_computed_feature_contact_preview(
                    expected, *owner, *parent, *source, *parameter,
                ) {
                    self.feature_authoring_preview = prior_preview;
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                *state = trial;
                Ok(())
            }
            EditorEffect::CommitComputedFeatureContact {
                expected,
                owner,
                parent,
                source,
                parameter,
            } => {
                let sample = ComputedFilletEditSample::Contact {
                    owner: *owner,
                    parent: *parent,
                    source: *source,
                    parameter: *parameter,
                };
                let preview = self
                    .feature_authoring_preview
                    .as_ref()
                    .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                if preview.metadata.feature != owner.feature
                    || !preview.accepts_radius_input(expected)
                    || preview.accepted_contact_sample != Some(sample)
                {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    *preview.radius_origin = FeatureAuthoringRadiusOrigin {
                        candidate: preview.candidate.clone(),
                        features: preview.features.clone(),
                        snapshot: preview.snapshot.clone(),
                        metadata: preview.metadata.clone(),
                    };
                    preview.radius_origin_state = None;
                    preview.accepted_contact_sample = None;
                }
                Ok(())
            }
            EditorEffect::RestoreComputedFeatureContact {
                expected, owner, ..
            } => {
                let origin_state = self
                    .feature_authoring_preview
                    .as_ref()
                    .and_then(|preview| preview.radius_origin_state.clone())
                    .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
                self.restore_feature_authoring_radius_preview(*expected, owner.feature)?;
                *state = *origin_state;
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    preview.radius_origin_state = None;
                    preview.accepted_contact_sample = None;
                }
                Ok(())
            }
            EditorEffect::ClearComputedFeatureContactPreview => {
                if let Some(preview) = self.feature_authoring_preview.as_mut() {
                    preview.radius_origin_state = None;
                    preview.accepted_contact_sample = None;
                }
                Ok(())
            }
            EditorEffect::CommitComputedFilletAction { target } => self
                .apply_feature_authoring_fillet_action(
                    state,
                    target.expected,
                    target.owner,
                    target.action,
                )
                .map(|_| ()),
            EditorEffect::FilletBranchPreviewChanged { .. } => Ok(()),
            _ => Err(CoordinatorError::FeatureAuthoringPreviewMismatch),
        }
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
        let next = self.stage_feature_mutation_checkpoint(&preview.features)?;
        let replay = ReplayAction::CreateComputedFillet {
            expected: preview.expected,
            label: preview.label.clone(),
            radius: preview.candidate.radius(),
            corners: preview.candidate.persistent_corners(),
        };
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
        self.record_feature_mutation(next, replay);
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

    /// Changes one Fillet set's shared radius by absolute same-branch
    /// continuation of every completed corner.
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
        if self.features.identity() != expected {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let origin = self.computed_evaluation_input()?;
        self.prepare_computed_fillet_numeric_radius_preview(&origin, feature, radius)?;
        self.publish_computed_fillet_preview(&origin, feature, radius)
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
        let matches_held = self
            .computed_fillet_preview
            .as_ref()
            .is_some_and(|preview| {
                preview.origin == expected
                    && preview.feature == feature
                    && preview.radius.to_bits() == radius.to_bits()
            });
        if !matches_held {
            self.prepare_computed_fillet_radius_preview(&expected, feature, radius)?;
        }
        self.publish_computed_fillet_preview(&expected, feature, radius)
    }

    fn apply_computed_fillet_configuration(
        &mut self,
        expected: ComputedFeatureDocumentIdentity,
        feature: ComputedFeatureId,
        radius: f64,
        corners: Vec<(ComputedFeatureCornerId, NewComputedFilletCorner)>,
        replay: ReplayAction,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        let before = self.features.identity();
        if before != expected {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let sketch_input = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        let mut candidate = self.features.clone();
        candidate.replace_fillet_set(feature, radius, corners)?;
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
        require_current_feature_authoring_evaluation(&snapshot, feature)?;
        let after = candidate.identity();
        let next = self.stage_feature_mutation_checkpoint(&candidate)?;
        self.features = candidate;
        self.computed_input = Some(snapshot.input());
        self.computed_snapshot = Some(snapshot);
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_fillet_preview = None;
        self.computed_evaluation_problem = None;
        self.computed_preview_evaluation_problem = None;
        self.record_feature_mutation(next, replay);
        Ok(ComputedFeatureMutation {
            value: (),
            before,
            after,
        })
    }

    /// Applies one explicit, bounded local branch/retention action to a
    /// published Fillet corner as one absolute configuration/history change.
    ///
    /// # Errors
    ///
    /// Rejects stale input, a missing/disabled action or any non-current whole-
    /// feature result without changing durable feature intent.
    pub fn apply_computed_fillet_action(
        &mut self,
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: ComputedCornerRef,
        action: SceneFilletActionId,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        if self.computed_evaluation_input()? != expected
            || self.features.identity() != expected.features
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        if !self.editor.selection().iter().any(|item| {
            matches!(item, SelectionItem::FeatureCorner(selected) if *selected == owner)
                || matches!(item, SelectionItem::Feature(feature) if *feature == owner.feature)
        }) {
            return Err(CoordinatorError::ComputedFilletActionUnavailable(
                "the Fillet corner or its owning feature is not selected".into(),
            ));
        }
        let alternatives = self.computed_fillet_alternatives(&self.features, owner)?;
        let replacement = select_computed_fillet_alternative(&alternatives.values, action)
            .ok_or_else(|| {
                CoordinatorError::ComputedFilletActionUnavailable(
                    "the selected local branch is not available from this exact corner".into(),
                )
            })?
            .resolved
            .corner;
        let corners = alternatives
            .corners
            .into_iter()
            .map(|(id, corner)| {
                (
                    id,
                    if id == owner.corner {
                        replacement
                    } else {
                        corner
                    },
                )
            })
            .collect::<Vec<_>>();
        self.apply_computed_fillet_configuration(
            expected.features,
            owner.feature,
            alternatives.radius,
            corners.clone(),
            ReplayAction::SetComputedFilletConfiguration {
                expected: expected.features,
                feature: owner.feature,
                radius: alternatives.radius,
                corners,
            },
        )
    }

    /// Applies the same explicit local action to a held grouped-authoring
    /// candidate. The temporary feature and collector advance atomically, but
    /// no durable history entry exists until ordinary feature Apply succeeds.
    ///
    /// # Errors
    ///
    /// Rejects stale preview ownership, unavailable branch identity, bounded
    /// feature work or a non-current replacement without changing the state.
    pub fn apply_feature_authoring_fillet_action(
        &mut self,
        state: &mut FeatureAuthoringState,
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: ComputedCornerRef,
        action: SceneFilletActionId,
    ) -> Result<FeatureAuthoringPreviewMetadata, CoordinatorError> {
        let preview = self
            .feature_authoring_preview
            .as_ref()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        if preview.metadata.feature != owner.feature || !preview.accepts_radius_input(&expected) {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        let corner_index = preview
            .corner_index(owner)
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        let alternatives = self.computed_fillet_alternatives(&preview.features, owner)?;
        let replacement = select_computed_fillet_alternative(&alternatives.values, action)
            .ok_or_else(|| {
                CoordinatorError::ComputedFilletActionUnavailable(
                    "the selected local branch is not available from this exact corner".into(),
                )
            })?
            .resolved
            .clone();
        let snapshot = self.feature_authoring_snapshot()?;
        let mut trial = state.clone();
        let outcome = trial.replace_corner_absolute(&snapshot, corner_index, replacement);
        let candidate = match outcome {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
            FeatureAuthoringOutcome::Warning(warning) => {
                return Err(CoordinatorError::FeatureAuthoringTransitionRejected(
                    warning.message,
                ));
            }
            FeatureAuthoringOutcome::ModeEntered(_)
            | FeatureAuthoringOutcome::NoNativeHit(_)
            | FeatureAuthoringOutcome::Collecting { .. }
            | FeatureAuthoringOutcome::Apply(_)
            | FeatureAuthoringOutcome::CandidateCleared(_)
            | FeatureAuthoringOutcome::ModeExited
            | FeatureAuthoringOutcome::Inactive => {
                return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
            }
        };
        let metadata = self.refresh_feature_authoring_preview(expected, &candidate)?;
        *state = trial;
        Ok(metadata)
    }

    fn computed_fillet_alternatives(
        &self,
        features: &ComputedFeatureDocument,
        owner: ComputedCornerRef,
    ) -> Result<ComputedFilletAlternatives, CoordinatorError> {
        let feature = features
            .feature(owner.feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(owner.feature))?;
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &feature.definition;
        let prior = fillet
            .corners
            .iter()
            .find(|corner| corner.id == owner.corner)
            .copied()
            .ok_or(ComputedFeatureDocumentError::UnknownCorner(owner.corner))?;
        let current_corners = fillet
            .corners
            .iter()
            .map(|corner| (corner.id, corner.without_id()))
            .collect::<Vec<_>>();
        let snapshot = self.feature_authoring_snapshot()?;
        let outcome = snapshot.local_fillet_corner_alternatives(
            prior.without_id(),
            fillet.radius,
            ComputedFeatureEvaluationPolicy::default(),
            computed_feature_authoring_control(),
        )?;
        let OperationOutcome::Completed {
            value: alternatives,
            ..
        } = outcome
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        Ok(ComputedFilletAlternatives {
            radius: fillet.radius,
            corners: current_corners,
            values: alternatives,
        })
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
        let origin = self.computed_evaluation_input()?;
        self.prepare_computed_fillet_radius_preview(&origin, feature, radius)?;
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
        self.prepare_computed_fillet_radius_preview(&expected, feature, radius)?;
        self.computed_preview_snapshot
            .as_ref()
            .ok_or(CoordinatorError::ComputedFeatureWorkStopped)
    }

    pub fn clear_computed_feature_preview(&mut self) {
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_fillet_preview = None;
        self.computed_preview_evaluation_problem = None;
    }

    fn prepare_computed_fillet_radius_preview(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<(), CoordinatorError> {
        self.prepare_computed_fillet_radius_preview_with_mode(origin, feature, radius, false)
    }

    fn prepare_computed_fillet_numeric_radius_preview(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<(), CoordinatorError> {
        self.prepare_computed_fillet_radius_preview_with_mode(origin, feature, radius, true)
    }

    fn prepare_computed_fillet_radius_preview_with_mode(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        radius: f64,
        explicit_numeric_edit: bool,
    ) -> Result<(), CoordinatorError> {
        if self.computed_evaluation_input()? != *origin {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let current_snapshot = self
            .computed_snapshot
            .as_ref()
            .ok_or(CoordinatorError::ComputedFeatureWorkStopped)?;
        if current_snapshot.input() != *origin {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        require_current_feature_authoring_evaluation(current_snapshot, feature)?;
        let feature_value = self
            .features
            .feature(feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &feature_value.definition;
        let corner_ids = fillet
            .corners
            .iter()
            .map(|corner| corner.id)
            .collect::<Vec<_>>();
        let priors = fillet
            .corners
            .iter()
            .copied()
            .map(geosolve_sketch_features::ComputedFilletCorner::without_id)
            .collect::<Vec<_>>();
        let authoring = self.feature_authoring_snapshot()?;
        let outcome = if explicit_numeric_edit {
            authoring.continue_fillet_corners_numeric(
                &priors,
                fillet.radius,
                radius,
                ComputedFeatureEvaluationPolicy::default(),
                computed_feature_authoring_control(),
            )?
        } else {
            authoring.continue_fillet_corners(
                &priors,
                fillet.radius,
                radius,
                ComputedFeatureEvaluationPolicy::default(),
                computed_feature_authoring_control(),
            )?
        };
        let OperationOutcome::Completed {
            value: continued, ..
        } = outcome
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if continued.len() != corner_ids.len()
            || continued.iter().any(|value| {
                value.sketch_input != origin.sketch
                    || value.accepted != origin.accepted
                    || value.arc.radius.to_bits() != radius.to_bits()
            })
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let corners = corner_ids
            .into_iter()
            .zip(continued.into_iter().map(|value| value.corner))
            .collect::<Vec<_>>();
        self.install_computed_fillet_preview(
            origin,
            feature,
            radius,
            corners,
            ComputedFilletEditSample::Radius(radius),
        )
    }

    fn install_computed_fillet_preview(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        radius: f64,
        corners: Vec<(ComputedFeatureCornerId, NewComputedFilletCorner)>,
        sample: ComputedFilletEditSample,
    ) -> Result<(), CoordinatorError> {
        let mut candidate = self.features.clone();
        candidate.replace_fillet_set(feature, radius, corners.clone())?;
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
        if self.computed_evaluation_input()? != *origin
            || snapshot.input().sketch != origin.sketch
            || snapshot.input().features != candidate.identity()
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        require_current_feature_authoring_evaluation(&snapshot, feature)?;
        self.computed_preview_input = Some(snapshot.input());
        self.computed_preview_snapshot = Some(snapshot.clone());
        self.computed_preview_evaluation_problem = None;
        self.computed_fillet_preview = Some(ComputedFilletEditPreview {
            origin: *origin,
            feature,
            radius,
            sample,
            corners,
            features: candidate,
            snapshot,
        });
        Ok(())
    }

    fn prepare_computed_fillet_contact_preview(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    ) -> Result<(), CoordinatorError> {
        if self.computed_evaluation_input()? != *origin || owner.feature.raw() == 0 {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let feature_value = self
            .features
            .feature(owner.feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(owner.feature))?;
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &feature_value.definition;
        let current = fillet
            .corners
            .iter()
            .find(|corner| corner.id == owner.corner)
            .copied()
            .ok_or(ComputedFeatureDocumentError::UnknownCorner(owner.corner))?;
        let parent_source = match parent {
            ComputedFilletParentIndex::First => current.first.source,
            ComputedFilletParentIndex::Second => current.second.source,
        };
        if parent_source != source || !parameter.is_finite() {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let authoring = self.feature_authoring_snapshot()?;
        let outcome = authoring.reseed_fillet_contact(
            ComputedFilletContactReseedRequest {
                prior: current.without_id(),
                parent,
                parameter,
            },
            fillet.radius,
            ComputedFeatureEvaluationPolicy::default(),
            computed_feature_authoring_control(),
        )?;
        let OperationOutcome::Completed {
            value: continued, ..
        } = outcome
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if continued.sketch_input != origin.sketch
            || continued.accepted != origin.accepted
            || continued.arc.radius.to_bits() != fillet.radius.to_bits()
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let radius = fillet.radius;
        let corners = fillet
            .corners
            .iter()
            .map(|corner| {
                (
                    corner.id,
                    if corner.id == owner.corner {
                        continued.corner
                    } else {
                        corner.without_id()
                    },
                )
            })
            .collect::<Vec<_>>();
        self.install_computed_fillet_preview(
            origin,
            owner.feature,
            radius,
            corners,
            ComputedFilletEditSample::Contact {
                owner,
                parent,
                source,
                parameter,
            },
        )
    }

    fn publish_computed_fillet_contact_preview(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        self.publish_held_computed_fillet_preview(
            origin,
            owner.feature,
            ComputedFilletEditSample::Contact {
                owner,
                parent,
                source,
                parameter,
            },
        )
    }

    fn publish_computed_fillet_preview(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        self.publish_held_computed_fillet_preview(
            origin,
            feature,
            ComputedFilletEditSample::Radius(radius),
        )
    }

    fn publish_held_computed_fillet_preview(
        &mut self,
        origin: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: ComputedFeatureId,
        sample: ComputedFilletEditSample,
    ) -> Result<ComputedFeatureMutation<()>, CoordinatorError> {
        if self.computed_evaluation_input()? != *origin {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let preview = self
            .computed_fillet_preview
            .as_ref()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        if preview.origin != *origin
            || preview.feature != feature
            || preview.sample != sample
            || preview.features.identity() != preview.snapshot.input().features
        {
            return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
        }
        require_current_feature_authoring_evaluation(&preview.snapshot, feature)?;
        let next = self.stage_feature_mutation_checkpoint(&preview.features)?;
        let preview = self
            .computed_fillet_preview
            .take()
            .ok_or(CoordinatorError::FeatureAuthoringPreviewMismatch)?;
        let before = self.features.identity();
        let after = preview.features.identity();
        let radius = preview.radius;
        let corners = preview.corners.clone();
        self.features = preview.features;
        self.computed_input = Some(preview.snapshot.input());
        self.computed_snapshot = Some(preview.snapshot);
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_evaluation_problem = None;
        self.computed_preview_evaluation_problem = None;
        self.record_feature_mutation(
            next,
            ReplayAction::SetComputedFilletConfiguration {
                expected: origin.features,
                feature,
                radius,
                corners,
            },
        );
        Ok(ComputedFeatureMutation {
            value: (),
            before,
            after,
        })
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
        let mut problems = self
            .computed_snapshot()
            .into_iter()
            .flat_map(ComputedFeatureSnapshot::feature_evaluations)
            .filter_map(|evaluation| match &evaluation.state {
                ComputedFeatureEvaluationState::Failed { failure } => Some(
                    computed_feature_problem(features, evaluation.feature, failure),
                ),
                ComputedFeatureEvaluationState::Current { .. }
                | ComputedFeatureEvaluationState::Suppressed => None,
            })
            .collect::<Vec<_>>();
        if let Some(gesture) = self.drag_continuation.as_ref() {
            problems.extend(gesture.computed_problems.iter().cloned());
        }
        problems
    }

    /// Returns the independently accepted drag preview visible to a presentation adapter.
    #[must_use]
    pub fn visible_preview_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.solved_preview_session()
    }

    /// Whether the visible preview is a non-authoritative prepared curve-control candidate.
    ///
    /// Presentation adapters retain the pointer-down scene stamp for this candidate while
    /// rendering its independently accepted geometry; only compare-and-swap publication may
    /// advance the authoritative design identity.
    #[must_use]
    pub fn curve_control_preview_active(&self) -> bool {
        self.current_curve_control_continuation()
            .and_then(|gesture| gesture.last_accepted.as_ref())
            .is_some_and(|sample| matches!(sample, CurveControlAcceptedSample::Changed(_)))
    }

    /// Retains the authenticated pointer-down origin on an exact prepared
    /// curve-control candidate scene.
    ///
    /// Candidate geometry keeps its own advanced design and accepted revision;
    /// this gesture-local origin only allows the already-live pointer sequence
    /// to sample and release that candidate. It grants no drafting-inference or
    /// detached mutation authority.
    ///
    /// # Errors
    ///
    /// Rejects a scene that is not the exact visible prepared candidate, whose
    /// computed provenance is stale, whose selected control layer was altered,
    /// or whose pointer-down base is no longer current.
    pub fn retain_curve_control_preview_interaction_origin(
        &self,
        scene: &mut EditorScene,
    ) -> Result<(), CoordinatorError> {
        let gesture = self
            .current_curve_control_continuation()
            .filter(|gesture| {
                gesture.expected == self.session.design_identity()
                    && gesture.base == self.session.prepared_input()
            })
            .ok_or(crate::EditorError::StalePreparedSketchInput)?;
        let CurveControlAcceptedSample::Changed(preview) = gesture
            .last_accepted
            .as_ref()
            .ok_or(crate::EditorError::StalePreparedSketchInput)?
        else {
            return Err(crate::EditorError::StalePreparedSketchInput.into());
        };
        let candidate = preview
            .patch
            .preview()
            .accepted_session()
            .ok_or(crate::EditorError::StalePreparedSketchInput)?;
        let candidate_input = candidate
            .accepted_prepared_input()
            .ok_or(crate::EditorError::StalePreparedSketchInput)?;
        let candidate_accepted = candidate
            .accepted_state_for_current_input()
            .ok_or(crate::EditorError::StalePreparedSketchInput)?;
        let origin_accepted = gesture
            .base
            .accepted_state_identity()
            .ok_or(crate::EditorError::StalePreparedSketchInput)?;
        if scene.authenticated_prepared_input().is_some()
            || scene.accepted_revision != candidate_accepted.identity().revision().get()
            || scene.design_identity != candidate.design_identity()
            || scene.accepted_document != *candidate_accepted.document()
            || candidate_input != preview.computed_snapshot.input().sketch
            || scene.computed_input != Some(preview.computed_snapshot.input())
            || scene.feature_identity != Some(preview.computed_snapshot.input().features)
        {
            return Err(crate::EditorError::StalePreparedSketchInput.into());
        }
        let controls = scene.curve_controls.clone();
        let guides = scene.curve_control_guides.clone();
        let mut rebuilt = scene.clone();
        self.editor.populate_curve_controls(&mut rebuilt)?;
        if rebuilt.curve_controls != controls
            || rebuilt.curve_control_guides != guides
            || !curve_control_point_aliases_match_scene(scene)
        {
            return Err(crate::EditorError::StalePreparedSketchInput.into());
        }
        scene.set_curve_control_interaction_origin(
            origin_accepted.revision().get(),
            gesture.expected,
            preview.request_id,
            preview.model_position,
        );
        Ok(())
    }

    /// Resolves a pointer press and captures any point gesture's locality plan from
    /// the exact accepted state visible at press time.
    pub fn pointer_down(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items(scene, input, &[])
    }

    /// Resolves a pointer press with explicit host-normalized drafting
    /// inference input and captures any point-drag locality state.
    pub fn pointer_down_with_draft_inference(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        inference: DraftInferenceInput,
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items_and_draft_inference(scene, input, &[], inference)
    }

    /// Resolves a pointer press with independent ambient-inference and recipe
    /// regularization intent while retaining point-drag locality ownership.
    pub fn pointer_down_with_draft_authoring(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items_and_draft_authoring(scene, input, &[], authoring)
    }

    /// Resolves a pointer press with diagnostic annotation forcing and explicit
    /// host-normalized drafting inference input while retaining point-drag
    /// locality ownership in the coordinator.
    pub fn pointer_down_with_problem_items_and_draft_inference(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
        inference: DraftInferenceInput,
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items_and_draft_authoring(
            scene,
            input,
            problem_items,
            DraftAuthoringInput {
                inference,
                regularized: false,
            },
        )
    }

    /// Resolves a pointer press with diagnostic annotation forcing and the
    /// complete semantic geometry-authoring input while retaining point-drag
    /// locality ownership in the coordinator.
    pub fn pointer_down_with_problem_items_and_draft_authoring(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        if self
            .editor
            .curve_control_press_target(scene, input.position, problem_items)
            .is_some()
            && !self.curve_control_scene_is_authenticated(scene)
        {
            return Vec::new();
        }
        let before = self.editor.point_gesture_snapshot();
        let effects = self
            .editor
            .pointer_down_with_problem_items_and_draft_authoring(
                scene,
                input,
                problem_items,
                authoring,
            );
        let after = self.editor.point_gesture_snapshot();
        if after != before {
            self.clear_transient();
            if let Some(gesture) = after {
                self.drag_continuation =
                    Some(self.plan_projected_drag_start(gesture, projected_drag_control()));
            }
        }
        effects
    }

    /// Reports the retained publication result for one tokenized construction
    /// plan back to the editor state machine.
    pub fn acknowledge_construction_commit(
        &mut self,
        token: ConstructionCommitToken,
        accepted: bool,
    ) -> Vec<EditorEffect> {
        let prepared_input = self.session.accepted_prepared_input();
        self.editor.acknowledge_construction_commit_for_input(
            token,
            accepted,
            prepared_input.as_ref(),
            true,
        )
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
        if self
            .editor
            .curve_control_press_target(scene, input.position, problem_items)
            .is_some()
            && !self.curve_control_scene_is_authenticated(scene)
        {
            return Vec::new();
        }
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

    fn curve_control_scene_is_authenticated(&self, scene: &EditorScene) -> bool {
        let Some(current) = self.session.accepted_prepared_input() else {
            return false;
        };
        if scene.authenticated_prepared_input() != Some(current) {
            return false;
        }
        let mut expected = scene.clone();
        if self.editor.populate_curve_controls(&mut expected).is_err() {
            return false;
        }
        expected.curve_controls == scene.curve_controls
            && expected.curve_control_guides == scene.curve_control_guides
            && curve_control_point_aliases_match_scene(scene)
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
            last_valid_computed_snapshot: self
                .computed_snapshot
                .as_ref()
                .filter(|snapshot| snapshot.input().features == self.features.identity())
                .cloned(),
            computed_problems: Vec::new(),
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
        gesture.computed_problems.clear();
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
                    } else if self
                        .mark_solved_preview_continuing_controlled(
                            &candidate,
                            bounded_geometry_control(),
                            gesture.last_valid_computed_snapshot.as_ref(),
                            SolvedPreviewPublicationPolicy::RequireCompleteComputedScene,
                            &mut gesture.computed_problems,
                        )
                        .is_err()
                    {
                        accepted_position = None;
                        rejection_stage = Some(ProjectedDragRejectionStage::PreviewPublication);
                    } else {
                        if let Some(snapshot) = self.computed_preview_snapshot.as_ref() {
                            gesture.last_valid_computed_snapshot = Some(snapshot.clone());
                        }
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

    /// Executes one inverse selected-curve control sample from a frozen prepared snapshot.
    ///
    /// Every sample in the gesture starts from the same exact accepted input. Only a finite,
    /// independently accepted sketch plus complete computed-feature scene replaces the retained
    /// preview. Rejection therefore leaves the prior valid patch available for release.
    pub fn resolve_curve_control_preview(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
        model_position: [f64; 2],
    ) -> Vec<EditorEffect> {
        match self
            .editor
            .curve_control_preview_request_disposition(pointer_id, request_id, expected, control)
        {
            CurveControlPreviewRequestDisposition::Current => {}
            CurveControlPreviewRequestDisposition::Stale
            | CurveControlPreviewRequestDisposition::Untracked => return Vec::new(),
        }
        if expected != self.session.design_identity()
            || !model_position.iter().all(|value| value.is_finite())
            || self.session.accepted_state_for_current_input().is_none()
        {
            return self
                .editor
                .curve_control_preview_result(pointer_id, request_id, expected, control, None);
        }
        let current = self.session.prepared_input();
        let same_gesture = self
            .curve_control_continuation
            .as_ref()
            .is_some_and(|gesture| {
                gesture.pointer_id == pointer_id
                    && gesture.control == control
                    && gesture.expected == expected
                    && gesture.base == current
            });
        if !same_gesture {
            self.transient = None;
            self.solved_preview = None;
            self.drag_continuation = None;
            self.projected_drag_work = None;
            self.computed_preview_snapshot = None;
            self.computed_preview_input = None;
            self.computed_preview_evaluation_problem = None;
            let snapshot = self.session.prepared_snapshot();
            self.curve_control_continuation = Some(CurveControlContinuation {
                pointer_id,
                control,
                expected,
                base: snapshot.input(),
                snapshot,
                computed_allocator: self.computed_evaluation_allocator.clone(),
                last_request_id: None,
                last_accepted: None,
            });
        }
        let Some(mut gesture) = self.curve_control_continuation.take() else {
            return Vec::new();
        };
        if gesture
            .last_request_id
            .is_some_and(|last_request_id| request_id <= last_request_id)
        {
            self.curve_control_continuation = Some(gesture);
            return Vec::new();
        }
        gesture.last_request_id = Some(request_id);

        let accepted = self.prepare_curve_control_sample(
            &gesture.snapshot,
            &gesture.computed_allocator,
            request_id,
            control,
            model_position,
        );
        let accepted_position = match accepted {
            Ok(CurveControlPreparedSample::Accepted(sample)) => {
                let position = sample.model_position();
                match &sample {
                    CurveControlAcceptedSample::Changed(preview) => {
                        let proposed = preview.patch.preview().proposed_commit();
                        self.computed_preview_input = Some(preview.computed_snapshot.input());
                        self.computed_preview_snapshot = Some(preview.computed_snapshot.clone());
                        self.computed_preview_evaluation_problem = None;
                        self.transient = proposed.accepted_state_identity().map(|accepted| {
                            TransientLifecycle::SolvedPreview {
                                attempt: proposed.attempt_identity(),
                                accepted,
                            }
                        });
                    }
                    CurveControlAcceptedSample::Unchanged { .. } => {
                        self.transient = None;
                        self.computed_preview_snapshot = None;
                        self.computed_preview_input = None;
                        self.computed_preview_evaluation_problem = None;
                    }
                }
                gesture.last_accepted = Some(sample);
                Some(position)
            }
            Ok(CurveControlPreparedSample::Rejected) | Err(_) => None,
        };
        self.curve_control_continuation = Some(gesture);
        self.editor.curve_control_preview_result(
            pointer_id,
            request_id,
            expected,
            control,
            accepted_position,
        )
    }

    fn prepare_curve_control_sample(
        &mut self,
        snapshot: &PreparedSketchSnapshot,
        base_computed_allocator: &ComputedEvaluationAllocator,
        request_id: u64,
        control: DocumentCurveControlId,
        model_position: [f64; 2],
    ) -> Result<CurveControlPreparedSample, CoordinatorError> {
        let accepted_document = snapshot
            .accepted_state()
            .filter(|accepted| {
                snapshot.input().accepted_state_identity() == Some(accepted.identity())
            })
            .map(geosolve_sketch::SketchAcceptedDocumentState::document)
            .ok_or(CoordinatorError::PreviewNotAccepted)?;
        let projection = accepted_document.project_curve_control(control, model_position)?;
        let Some(edit) = curve_control_projection_edit(projection) else {
            return Ok(CurveControlPreparedSample::Rejected);
        };
        let retained_position = accepted_document
            .curve_controls(control.curve)?
            .into_iter()
            .find(|candidate| candidate.id == control)
            .map(|candidate| candidate.position)
            .filter(|position| position.iter().all(|value| value.is_finite()))
            .ok_or(CoordinatorError::PreviewNotAccepted)?;
        if curve_control_edit_is_noop(snapshot.design_document(), &edit) {
            return Ok(CurveControlPreparedSample::Accepted(
                CurveControlAcceptedSample::Unchanged {
                    request_id,
                    control,
                    model_position: retained_position,
                },
            ));
        }
        let outcome = snapshot
            .clone()
            .prepare(PreparedSketchOperation::Apply(edit.clone()))
            .execute(bounded_geometry_control())?;
        let OperationOutcome::Completed { value: patch, .. } = outcome else {
            return Ok(CurveControlPreparedSample::Rejected);
        };
        if patch.proposed_commit().accepted_state_identity().is_none() {
            return Ok(CurveControlPreparedSample::Rejected);
        }
        let patch_preview = patch.preview();
        let candidate_session = patch_preview
            .accepted_session()
            .ok_or(CoordinatorError::PreviewNotAccepted)?;
        let previous = self
            .computed_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.input().features == self.features.identity());
        let mut computed_allocator = base_computed_allocator.clone();
        let evaluated = evaluate_computed_features_continuing(
            candidate_session,
            &self.features,
            &mut computed_allocator,
            bounded_geometry_control(),
            previous,
        )?;
        let OperationOutcome::Completed {
            value: computed_snapshot,
            ..
        } = evaluated
        else {
            return Ok(CurveControlPreparedSample::Rejected);
        };
        if previous.is_some_and(|previous| {
            !computed_feature_preview_invalidations(&self.features, previous, &computed_snapshot)
                .is_empty()
        }) {
            return Ok(CurveControlPreparedSample::Rejected);
        }
        let accepted_document = patch_preview
            .accepted_document()
            .ok_or(CoordinatorError::PreviewNotAccepted)?;
        let accepted_position = accepted_document
            .curve_controls(control.curve)?
            .into_iter()
            .find(|candidate| candidate.id == control)
            .map(|candidate| candidate.position)
            .filter(|position| position.iter().all(|value| value.is_finite()))
            .ok_or(CoordinatorError::PreviewNotAccepted)?;
        Ok(CurveControlPreparedSample::Accepted(
            CurveControlAcceptedSample::Changed(Box::new(CurveControlPreparedPreview {
                request_id,
                control,
                model_position: accepted_position,
                edit,
                patch,
                computed_snapshot,
                computed_allocator,
            })),
        ))
    }

    /// Explicitly marks an outstanding solve. It does not mutate lifecycle history.
    pub fn mark_solving(&mut self) {
        self.transient = Some(TransientLifecycle::Solving);
        self.solved_preview = None;
        self.drag_continuation = None;
        self.curve_control_continuation = None;
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
        let previous = self.computed_snapshot.clone();
        let mut projected_problems = Vec::new();
        self.mark_solved_preview_continuing_controlled(
            preview,
            control,
            previous.as_ref(),
            SolvedPreviewPublicationPolicy::AllowNativeOnly,
            &mut projected_problems,
        )
    }

    fn mark_solved_preview_continuing_controlled(
        &mut self,
        preview: &RetainedSketchDocumentSession,
        control: OperationControl,
        previous: Option<&ComputedFeatureSnapshot>,
        publication: SolvedPreviewPublicationPolicy,
        projected_problems: &mut Vec<ComputedFeatureProblemMetadata>,
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
        let evaluated = evaluate_computed_features_continuing(
            preview,
            &self.features,
            &mut self.computed_evaluation_allocator,
            control,
            previous.filter(|snapshot| snapshot.input().features == self.features.identity()),
        );
        match evaluated {
            Ok(OperationOutcome::Completed { value, .. }) => {
                if publication == SolvedPreviewPublicationPolicy::RequireCompleteComputedScene
                    && let Some(previous) = previous
                {
                    let problems =
                        computed_feature_preview_invalidations(&self.features, previous, &value);
                    if !problems.is_empty() {
                        *projected_problems = problems;
                        return Err(CoordinatorError::ComputedFeaturePreviewInvalidated);
                    }
                }
                self.computed_preview_input = Some(value.input());
                self.computed_preview_snapshot = Some(value);
                self.computed_preview_evaluation_problem = None;
                projected_problems.clear();
            }
            Ok(stopped) => {
                let message = format!(
                    "computed-feature preview evaluation stopped: {:?}",
                    stopped.report().stopping_reason
                );
                if publication == SolvedPreviewPublicationPolicy::RequireCompleteComputedScene {
                    *projected_problems = computed_preview_stopped_problems(
                        &self.features,
                        previous,
                        format!(
                            "Fillet movement limit: holding the last valid position because {message}"
                        ),
                    );
                    return Err(CoordinatorError::ComputedFeatureWorkStopped);
                }
                self.computed_preview_snapshot = None;
                self.computed_preview_input = None;
                self.computed_preview_evaluation_problem = Some(message);
            }
            Err(error) => {
                if publication == SolvedPreviewPublicationPolicy::RequireCompleteComputedScene {
                    *projected_problems = vec![computed_preview_global_limit(format!(
                        "Movement is held at the last valid position because computed-feature evaluation failed: {error}"
                    ))];
                    return Err(error);
                }
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
        self.curve_control_continuation = None;
        self.projected_drag_work = None;
        self.clear_feature_authoring_preview();
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_fillet_preview = None;
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
    /// any revision already observed by either lifecycle. When the checkpoint's
    /// native design and accepted payload are byte-identical to a healthy current
    /// retained session, only the feature sidecar is restored; this preserves the
    /// exact accepted sketch identity, audit rows and rank/DOF across feature-only
    /// save/reload. A failed live attempt is reconstructed from the checkpoint so
    /// stale attempt metadata cannot survive the reload shortcut.
    ///
    /// # Errors
    ///
    /// Returns JSON, foreign-document, accepted-snapshot, solve-setup, or revision errors.
    pub fn reload(&mut self, saved_checkpoint: &RestoreCheckpoint) -> Result<(), CoordinatorError> {
        let current_checkpoint = checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        )?;
        let sketch_unchanged = self.session.accepted_state_for_current_input().is_some()
            && saved_checkpoint.design_json == current_checkpoint.design_json
            && saved_checkpoint.design_is_draft_v5 == current_checkpoint.design_is_draft_v5
            && saved_checkpoint.accepted_json == current_checkpoint.accepted_json
            && saved_checkpoint.accepted_is_draft_v5 == current_checkpoint.accepted_is_draft_v5
            && saved_checkpoint.accepted_belongs_to_current_design
                == current_checkpoint.accepted_belongs_to_current_design;
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
        let restored = if sketch_unchanged {
            None
        } else {
            Some(restore_sketch_checkpoint(
                &self.session,
                saved_checkpoint,
                revisions,
                AcceptedCheckpointRestore::RequireExact,
            )?)
        };
        let retained_features = merge_feature_lifecycle_high_water(
            self.features.lifecycle_high_water(),
            saved_checkpoint.feature_lifecycle,
        );
        let mut restored_features =
            ComputedFeatureDocument::from_json(&saved_checkpoint.feature_json)?;
        let restored_sketch = restored.as_ref().map_or_else(
            || self.session.design_document().id(),
            |session| session.design_document().id(),
        );
        if restored_features.sketch_document() != restored_sketch {
            return Err(ComputedFeatureSnapshotError::FeatureDocumentForDifferentSketch.into());
        }
        restored_features.rebase_after_restore(retained_features)?;
        self.computed_evaluation_allocator
            .retain_high_water(saved_checkpoint.evaluation_allocator);
        if let Some(restored) = restored {
            self.session = restored;
        } else {
            let high_water = self
                .session
                .persistent_identity_high_water()
                .merged(&saved_checkpoint.sketch_identity_high_water)?;
            self.session
                .retain_persistent_identity_high_water(&high_water)?;
        }
        self.features = restored_features;
        self.editor.invalidate_for_retained_state_change(true);
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
        let accepted_document = self.session.accepted_state().map_or(
            document,
            geosolve_sketch::SketchAcceptedDocumentState::document,
        );
        let selection = self.editor.selection();
        match action {
            CoordinatorActionKind::Constraint(intent) => {
                let Some(resolved) = self.resolved_constraint(intent) else {
                    return Vec::new();
                };
                if resolved == ResolvedConstraintKind::RadialLine {
                    return selected_radial_line(document, selection)
                        .and_then(|(line, center, operand)| {
                            radial_line_contact_action_choice(
                                document,
                                accepted_document,
                                operand,
                                line,
                                center,
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
            computed_features: None,
        };
        let previous_session = self.session.clone();
        let outcome = self.session.apply(expected, edit)?;
        let result = MutationOutcome {
            value: outcome.value().clone(),
            design: outcome.design_identity(),
            attempt: outcome.attempt_identity(),
            published_accepted: outcome.published_accepted_identity(),
        };
        self.record_mutation(replay, previous_session)?;
        Ok(result)
    }

    fn apply_recorded_edit_with_computed_features(
        &mut self,
        expected: SketchDesignIdentity,
        edit: DocumentEdit,
        transition: &RecordedComputedFeatureTransition,
        replay: ReplayAction,
    ) -> Result<(), CoordinatorError> {
        self.ensure_expected(expected)?;
        let before_sketch = self
            .session
            .accepted_prepared_input()
            .ok_or(CoordinatorError::StaleComputedFeatureCandidate)?;
        if transition.edit != edit
            || !prepared_sketch_inputs_match_for_replay(&transition.before_sketch, &before_sketch)
            || self.features.identity() != transition.before
            || transition.after.sketch_document() != self.session.design_document().id()
            || !recorded_transition_is_reanchor_only(&self.features, &transition.after)
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        self.computed_snapshot
            .as_ref()
            .filter(|snapshot| {
                snapshot.input().sketch == before_sketch
                    && snapshot.input().features == transition.before
            })
            .ok_or(CoordinatorError::StaleComputedFeatureCandidate)?;
        let mut candidate_session = self.session.clone();
        let retained = candidate_session.apply(expected, edit)?;
        let after_sketch = candidate_session
            .accepted_prepared_input()
            .filter(|_| retained.published_accepted_identity().is_some())
            .ok_or(CoordinatorError::StaleComputedFeatureCandidate)?;
        if !prepared_sketch_inputs_match_for_replay(&transition.after_sketch, &after_sketch) {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let mut candidate_allocator = self.computed_evaluation_allocator.clone();
        let evaluated = evaluate_computed_features(
            &candidate_session,
            &transition.after,
            &mut candidate_allocator,
            bounded_geometry_control(),
        )?;
        let OperationOutcome::Completed {
            value: snapshot, ..
        } = evaluated
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if recorded_computed_feature_dispositions(&snapshot) != transition.dispositions {
            return Err(CoordinatorError::ComputedFeatureReanchorNotDurable);
        }
        let independently_reanchored = snapshot.reanchored_feature_document(&transition.after)?;
        if !computed_feature_document_semantics_match(&transition.after, &independently_reanchored)
        {
            return Err(CoordinatorError::ComputedFeatureReanchorNotDurable);
        }
        let next = checkpoint(&candidate_session, &transition.after, &candidate_allocator)?;

        self.session = candidate_session;
        self.features = transition.after.clone();
        self.computed_evaluation_allocator = candidate_allocator;
        self.computed_input = Some(snapshot.input());
        self.computed_snapshot = Some(snapshot);
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_fillet_preview = None;
        self.computed_evaluation_problem = None;
        self.record_feature_mutation(next, replay);
        Ok(())
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

    /// Returns the aggregate role of selected native curves. Repeated spans of
    /// one polyline count once; non-curve selection items are ignored.
    #[must_use]
    pub fn selected_geometry_role_state(&self) -> Option<GeometryRoleSelectionState> {
        let curves = self
            .editor
            .selection()
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Curve(span) => Some(span.curve),
                SelectionItem::Point(_)
                | SelectionItem::Constraint(_)
                | SelectionItem::Dimension(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let mut roles = curves
            .into_iter()
            .filter_map(|curve| self.session.design_document().geometry_role(curve));
        let first = roles.next()?;
        if roles.all(|role| role == first) {
            Some(match first {
                GeometryRole::Profile => GeometryRoleSelectionState::Profile,
                GeometryRole::Construction => GeometryRoleSelectionState::Construction,
            })
        } else {
            Some(GeometryRoleSelectionState::Mixed)
        }
    }

    /// Returns exact persistent properties for exactly one selected native curve.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive family match keeps the inspector catalog aligned with every native curve definition"
    )]
    pub fn curve_property_metadata_for(
        &self,
        selection: &[SelectionItem],
    ) -> Option<SelectedCurvePropertyMetadata> {
        let [SelectionItem::Curve(span)] = selection else {
            return None;
        };
        let document = self.session.design_document();
        if !document.curve_spans(span.curve).ok()?.contains(span) {
            return None;
        }
        let curve = document.curve(span.curve)?;
        let accepted = self.session.accepted_state_for_current_input()?;
        let direct_edit_availability = curve_direct_edit_availability(
            accepted.document(),
            span.curve,
            accepted.effective_activity(),
        );
        let mut numeric = Vec::new();
        let mut push_scalar =
            |kind: CurveNumericPropertyKind, scalar: DesignScalarId, gauge_owned: bool| {
                let value = document.scalar(scalar)?;
                numeric.push(CurveNumericPropertyMetadata {
                    kind,
                    scalar,
                    value: value.value,
                    unit: value.unit,
                    domain: value.domain,
                    availability: curve_numeric_property_availability(
                        accepted.document(),
                        span.curve,
                        scalar,
                        direct_edit_availability,
                        gauge_owned,
                    ),
                });
                Some(())
            };
        let (family, sweep, hyperbola_branch, rational_control, nurbs_gauge, degree) =
            match &curve.definition {
                CurveDefinition::Line { .. } => {
                    (CurvePropertyFamily::Line, None, None, None, None, None)
                }
                CurveDefinition::Polyline { .. } => {
                    (CurvePropertyFamily::Polyline, None, None, None, None, None)
                }
                CurveDefinition::Circle { radius, .. } => {
                    push_scalar(CurveNumericPropertyKind::Radius, *radius, false)?;
                    (CurvePropertyFamily::Circle, None, None, None, None, None)
                }
                CurveDefinition::CircularArc {
                    radius,
                    start_angle,
                    end_angle,
                    sweep,
                    ..
                } => {
                    push_scalar(CurveNumericPropertyKind::Radius, *radius, false)?;
                    push_scalar(CurveNumericPropertyKind::TrimStart, *start_angle, false)?;
                    push_scalar(CurveNumericPropertyKind::TrimEnd, *end_angle, false)?;
                    (
                        CurvePropertyFamily::CircularArc,
                        Some(*sweep),
                        None,
                        None,
                        None,
                        None,
                    )
                }
                CurveDefinition::QuadraticBezier { .. } => (
                    CurvePropertyFamily::QuadraticBezier,
                    None,
                    None,
                    None,
                    None,
                    Some(2),
                ),
                CurveDefinition::CubicBezier { .. } => (
                    CurvePropertyFamily::CubicBezier,
                    None,
                    None,
                    None,
                    None,
                    Some(3),
                ),
                CurveDefinition::Ellipse {
                    minor_axis_ratio, ..
                } => {
                    push_scalar(
                        CurveNumericPropertyKind::MinorAxisRatio,
                        *minor_axis_ratio,
                        false,
                    )?;
                    (CurvePropertyFamily::Ellipse, None, None, None, None, None)
                }
                CurveDefinition::EllipticalArc {
                    minor_axis_ratio,
                    start_angle,
                    end_angle,
                    sweep,
                    ..
                } => {
                    push_scalar(
                        CurveNumericPropertyKind::MinorAxisRatio,
                        *minor_axis_ratio,
                        false,
                    )?;
                    push_scalar(CurveNumericPropertyKind::TrimStart, *start_angle, false)?;
                    push_scalar(CurveNumericPropertyKind::TrimEnd, *end_angle, false)?;
                    (
                        CurvePropertyFamily::EllipticalArc,
                        Some(*sweep),
                        None,
                        None,
                        None,
                        None,
                    )
                }
                CurveDefinition::RationalQuadraticConic { middle_weight, .. } => {
                    push_scalar(
                        CurveNumericPropertyKind::RationalWeight,
                        *middle_weight,
                        false,
                    )?;
                    (
                        CurvePropertyFamily::RationalQuadraticConic,
                        None,
                        None,
                        Some(document.rational_conic_control(span.curve).ok()?),
                        None,
                        Some(2),
                    )
                }
                CurveDefinition::ParabolaSegment {
                    trim_start,
                    trim_end,
                    ..
                } => {
                    push_scalar(CurveNumericPropertyKind::TrimStart, *trim_start, false)?;
                    push_scalar(CurveNumericPropertyKind::TrimEnd, *trim_end, false)?;
                    (CurvePropertyFamily::Parabola, None, None, None, None, None)
                }
                CurveDefinition::HyperbolaSegment {
                    semi_conjugate,
                    branch,
                    trim_start,
                    trim_end,
                    ..
                } => {
                    push_scalar(
                        CurveNumericPropertyKind::SemiConjugate,
                        *semi_conjugate,
                        false,
                    )?;
                    push_scalar(CurveNumericPropertyKind::TrimStart, *trim_start, false)?;
                    push_scalar(CurveNumericPropertyKind::TrimEnd, *trim_end, false)?;
                    (
                        CurvePropertyFamily::Hyperbola,
                        None,
                        Some(*branch),
                        None,
                        None,
                        None,
                    )
                }
                CurveDefinition::BSpline { degree, .. } => (
                    CurvePropertyFamily::BSpline,
                    None,
                    None,
                    None,
                    None,
                    Some(*degree),
                ),
                CurveDefinition::Nurbs {
                    degree,
                    weights,
                    gauge_weight,
                    ..
                } => {
                    for (ordinal, scalar) in weights.iter().copied().enumerate() {
                        push_scalar(
                            CurveNumericPropertyKind::NurbsWeight {
                                ordinal: u32::try_from(ordinal).ok()?,
                            },
                            scalar,
                            scalar == *gauge_weight,
                        )?;
                    }
                    (
                        CurvePropertyFamily::Nurbs,
                        None,
                        None,
                        None,
                        Some(*gauge_weight),
                        Some(*degree),
                    )
                }
            };
        let nurbs_gauge_availability = nurbs_gauge.map(|_| {
            if direct_edit_availability != DocumentCurveControlAvailability::Editable {
                direct_edit_availability
            } else if numeric.iter().any(|property| {
                property.availability
                    == DocumentCurveControlAvailability::ReadOnly(
                        DocumentCurveControlWithholdingReason::HostParameterOwned,
                    )
            }) {
                DocumentCurveControlAvailability::ReadOnly(
                    DocumentCurveControlWithholdingReason::HostParameterOwned,
                )
            } else {
                DocumentCurveControlAvailability::Editable
            }
        });
        Some(SelectedCurvePropertyMetadata {
            curve: span.curve,
            label: curve.label.clone(),
            family,
            direct_edit_availability,
            numeric,
            sweep,
            hyperbola_branch,
            rational_control,
            nurbs_gauge,
            nurbs_gauge_availability,
            degree,
        })
    }

    /// Returns exact properties for the current application selection.
    #[must_use]
    pub fn selected_curve_property_metadata(&self) -> Option<SelectedCurvePropertyMetadata> {
        self.curve_property_metadata_for(self.editor.selection())
    }

    fn selected_curve_property_for_action(
        &self,
        curve: CurveId,
    ) -> Result<SelectedCurvePropertyMetadata, CoordinatorError> {
        let metadata = self
            .selected_curve_property_metadata()
            .ok_or(CoordinatorError::CurvePropertySelectionMismatch)?;
        if metadata.curve != curve {
            return Err(CoordinatorError::CurvePropertySelectionMismatch);
        }
        Ok(metadata)
    }

    /// Retains one exact selected-curve numeric property through ordinary document history.
    ///
    /// Rational nonzero-weight edits preserve the visible Euclidean middle control atomically.
    /// Entering or leaving the exact zero-weight projective mode retains the stored `Qh` vector
    /// explicitly, so no division-by-zero or fictitious point is introduced.
    ///
    /// # Errors
    ///
    /// Rejects a stale design, missing/mismatched property, invalid scalar domain or solve.
    pub fn set_curve_numeric_property(
        &mut self,
        expected: SketchDesignIdentity,
        curve: CurveId,
        kind: CurveNumericPropertyKind,
        value: f64,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let metadata = self.selected_curve_property_for_action(curve)?;
        let property = metadata
            .numeric
            .iter()
            .find(|property| property.kind == kind)
            .ok_or(CoordinatorError::InvalidActionInput(
                "numeric property does not belong to the selected curve",
            ))?;
        ensure_curve_property_available(property.availability)?;
        let edit = if kind == CurveNumericPropertyKind::RationalWeight {
            let document = self.session.design_document();
            let control = document.rational_conic_control(curve).map_err(|_| {
                CoordinatorError::InvalidActionInput("curve has no rational middle control")
            })?;
            let next = match control {
                DocumentRationalConicControl::Euclidean { middle, .. } if value != 0.0 => {
                    DocumentRationalConicControl::Euclidean {
                        middle,
                        weight: value,
                    }
                }
                DocumentRationalConicControl::Euclidean { .. } => {
                    let CurveDefinition::RationalQuadraticConic {
                        weighted_middle, ..
                    } = &document
                        .curve(curve)
                        .ok_or(CoordinatorError::ActionUnavailable(
                            DisabledReason::MissingObject,
                        ))?
                        .definition
                    else {
                        return Err(CoordinatorError::InvalidActionInput(
                            "curve has no rational middle control",
                        ));
                    };
                    DocumentRationalConicControl::Projective {
                        weighted_middle: *weighted_middle,
                        weight: 0.0,
                    }
                }
                DocumentRationalConicControl::Projective {
                    weighted_middle, ..
                } if value == 0.0 => DocumentRationalConicControl::Projective {
                    weighted_middle,
                    weight: 0.0,
                },
                DocumentRationalConicControl::Projective {
                    weighted_middle, ..
                } => DocumentRationalConicControl::Euclidean {
                    middle: [weighted_middle[0] / value, weighted_middle[1] / value],
                    weight: value,
                },
                _ => {
                    return Err(CoordinatorError::InvalidActionInput(
                        "unsupported rational middle control mode",
                    ));
                }
            };
            DocumentEdit::SetRationalConicControl {
                curve,
                control: next,
            }
        } else {
            DocumentEdit::SetScalarValue {
                scalar: property.scalar,
                value,
            }
        };
        self.apply_edit(expected, edit)
    }

    /// Retains the exact visible middle coordinate of one rational quadratic conic.
    ///
    /// `middle` is the ordinary Euclidean `P1` while the curve has nonzero weight and the
    /// projective `Qh` vector while its weight is exactly zero. The current weight and control
    /// mode are preserved, so a presentation adapter never needs to reconstruct homogeneous
    /// storage or choose a zero-weight transition.
    ///
    /// # Errors
    ///
    /// Rejects a stale design, non-rational owner, invalid coordinate or failed retained edit.
    pub fn set_curve_rational_middle(
        &mut self,
        expected: SketchDesignIdentity,
        curve: CurveId,
        middle: [f64; 2],
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let metadata = self.selected_curve_property_for_action(curve)?;
        if metadata.rational_control.is_none() {
            return Err(CoordinatorError::InvalidActionInput(
                "curve has no rational middle control",
            ));
        }
        ensure_curve_property_available(metadata.direct_edit_availability)?;
        let current = self
            .session
            .design_document()
            .rational_conic_control(curve)
            .map_err(|_| {
                CoordinatorError::InvalidActionInput("curve has no rational middle control")
            })?;
        let control = match current {
            DocumentRationalConicControl::Euclidean { weight, .. } => {
                DocumentRationalConicControl::Euclidean { middle, weight }
            }
            DocumentRationalConicControl::Projective { weight, .. } => {
                DocumentRationalConicControl::Projective {
                    weighted_middle: middle,
                    weight,
                }
            }
            _ => {
                return Err(CoordinatorError::InvalidActionInput(
                    "unsupported rational middle control mode",
                ));
            }
        };
        self.apply_edit(
            expected,
            DocumentEdit::SetRationalConicControl { curve, control },
        )
    }

    /// Retains one explicit selected arc traversal choice.
    ///
    /// # Errors
    ///
    /// Rejects a stale design, incompatible curve or failed retained edit.
    pub fn set_curve_sweep(
        &mut self,
        expected: SketchDesignIdentity,
        curve: CurveId,
        sweep: DocumentArcSweep,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let metadata = self.selected_curve_property_for_action(curve)?;
        if metadata.sweep.is_none() {
            return Err(CoordinatorError::InvalidActionInput(
                "curve has no explicit arc sweep",
            ));
        }
        ensure_curve_property_available(metadata.direct_edit_availability)?;
        self.apply_edit(expected, DocumentEdit::SetArcSweep { curve, sweep })
    }

    /// Retains one explicit selected hyperbola branch choice.
    ///
    /// # Errors
    ///
    /// Rejects a stale design, incompatible curve or failed retained edit.
    pub fn set_curve_hyperbola_branch(
        &mut self,
        expected: SketchDesignIdentity,
        curve: CurveId,
        branch: DocumentHyperbolaBranch,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let metadata = self.selected_curve_property_for_action(curve)?;
        if metadata.hyperbola_branch.is_none() {
            return Err(CoordinatorError::InvalidActionInput(
                "curve has no explicit hyperbola branch",
            ));
        }
        ensure_curve_property_available(metadata.direct_edit_availability)?;
        self.apply_edit(expected, DocumentEdit::SetHyperbolaBranch { curve, branch })
    }

    /// Makes one existing selected NURBS weight the explicit gauge without moving the curve.
    ///
    /// # Errors
    ///
    /// Rejects a stale design, incompatible curve/weight or failed retained edit.
    pub fn set_curve_nurbs_gauge(
        &mut self,
        expected: SketchDesignIdentity,
        curve: CurveId,
        gauge_weight: DesignScalarId,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let metadata = self.selected_curve_property_for_action(curve)?;
        let availability =
            metadata
                .nurbs_gauge_availability
                .ok_or(CoordinatorError::InvalidActionInput(
                    "curve has no NURBS gauge",
                ))?;
        if metadata.nurbs_gauge == Some(gauge_weight) {
            return Err(CoordinatorError::CurvePropertyUnavailable(
                DocumentCurveControlWithholdingReason::GaugeOwned,
            ));
        }
        if !metadata
            .numeric
            .iter()
            .any(|property| property.scalar == gauge_weight)
        {
            return Err(CoordinatorError::InvalidActionInput(
                "gauge weight does not belong to the selected curve",
            ));
        }
        ensure_curve_property_available(availability)?;
        self.apply_edit(
            expected,
            DocumentEdit::SetNurbsWeightGauge {
                curve,
                gauge_weight,
            },
        )
    }

    /// Atomically toggles every selected complete native curve. An all-
    /// Construction selection becomes Profile; Profile or mixed selections
    /// become Construction.
    ///
    /// # Errors
    ///
    /// Returns an action-availability, stale-design, document-validation,
    /// solve-setup or checkpoint error without partial role changes.
    pub fn toggle_selected_geometry_role(
        &mut self,
        expected: SketchDesignIdentity,
    ) -> Result<MutationOutcome<DocumentCommandEffect>, CoordinatorError> {
        self.ensure_expected(expected)?;
        if selection_contains_datum(self.editor.selection()) {
            return Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum,
            ));
        }
        let target = match self.selected_geometry_role_state() {
            Some(GeometryRoleSelectionState::Construction) => GeometryRole::Profile,
            Some(GeometryRoleSelectionState::Profile | GeometryRoleSelectionState::Mixed) => {
                GeometryRole::Construction
            }
            None => {
                return Err(CoordinatorError::ActionUnavailable(
                    DisabledReason::WrongOperandKind,
                ));
            }
        };
        let edits = self
            .editor
            .selection()
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Curve(span) => Some(span.curve),
                SelectionItem::Point(_)
                | SelectionItem::Constraint(_)
                | SelectionItem::Dimension(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_) => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|curve| GeometryRoleEdit::new(curve, target))
            .collect();
        self.apply_edit(expected, DocumentEdit::SetGeometryRoles { edits })
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
        let previous_session = self.session.clone();
        let outcome = self.session.transact(expected, |document| {
            document.rebind_external_binding(binding, expected_kind, expected_topology)
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(
            ReplayAction::RebindExternalBinding {
                expected,
                binding,
                expected_kind,
                expected_topology,
            },
            previous_session,
        )?;
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
        self.editor.invalidate_for_retained_state_change(true);
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
        self.editor.invalidate_for_retained_state_change(true);
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
        self.apply_construction_with_role(expected, proposal, GeometryRole::Profile)
    }

    /// Applies one role-aware construction proposal as one retained transaction
    /// and checkpoint.
    ///
    /// # Errors
    ///
    /// Returns stale-design, construction, solve-setup or checkpoint errors.
    pub fn apply_construction_with_role(
        &mut self,
        expected: SketchDesignIdentity,
        proposal: &ConstructionProposal,
        role: GeometryRole,
    ) -> Result<MutationOutcome<ConstructionResult>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let replay = ReplayAction::Construction {
            expected,
            proposal: proposal.clone(),
            role,
        };
        let previous_session = self.session.clone();
        let outcome = self.session.transact(expected, |document| {
            proposal.apply_with_role(document, role)
        })?;
        let result = MutationOutcome {
            value: outcome.value().clone(),
            design: outcome.design_identity(),
            attempt: outcome.attempt_identity(),
            published_accepted: outcome.published_accepted_identity(),
        };
        self.record_mutation(replay, previous_session)?;
        Ok(result)
    }

    /// Atomically applies one construction and its constraint-backed drafting
    /// inferences, publishing only a newly accepted, non-redundant candidate.
    ///
    /// Unlike ordinary retained design edits, automatic inference is
    /// fail-closed: a rejected solve or a newly redundant inferred source never
    /// enters retained design, history, or the allocator lifecycle. The exact
    /// trial session is swapped in after validation, so the accepted candidate
    /// is not reconstructed or solved a second time.
    ///
    /// # Errors
    ///
    /// Returns stale-design, document, solve, non-acceptance, redundancy, or
    /// checkpoint errors without mutating the live coordinator.
    pub fn apply_construction_plan(
        &mut self,
        expected: &PreparedSketchInput,
        plan: &ConstructionCommitPlan,
    ) -> Result<MutationOutcome<ConstructionCommitResult>, CoordinatorError> {
        self.ensure_pending_construction_plan_compatible(expected, plan)?;
        self.ensure_construction_plan_input(expected)?;
        plan.validate_relation_count()?;
        let design = expected.design_identity();
        let mut trial = self.session.clone();
        let outcome = trial.transact(design, |document| plan.apply(document))?;
        self.publish_construction_plan_trial(expected, plan, trial, &outcome)
    }

    /// Controlled counterpart to [`Self::apply_construction_plan`].
    ///
    /// Cancellation or deterministic work exhaustion returns its exact
    /// [`OperationOutcome`] and leaves the live retained design, accepted state,
    /// allocator lifecycle, history, transcript, and pending draft untouched.
    ///
    /// # Errors
    ///
    /// Returns the same stale-design, document, solve, non-acceptance,
    /// redundancy, or checkpoint errors as [`Self::apply_construction_plan`].
    pub fn apply_construction_plan_controlled(
        &mut self,
        expected: &PreparedSketchInput,
        plan: &ConstructionCommitPlan,
        control: OperationControl,
    ) -> Result<OperationOutcome<MutationOutcome<ConstructionCommitResult>>, CoordinatorError> {
        self.ensure_pending_construction_plan_compatible(expected, plan)?;
        self.ensure_construction_plan_input(expected)?;
        plan.validate_relation_count()?;
        let design = expected.design_identity();
        let mut controller = OperationController::new(control);
        let mut trial = self.session.clone();
        let Some(outcome) = trial.transact_controlled_edit_in_controller(
            design,
            |document, controller| plan.apply_in_controller(document, controller),
            &mut controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let value = Self::validate_construction_plan_trial(&trial, &outcome)?;
        let Some(staged) =
            self.stage_construction_publication_in_controller(trial, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        if !self.publish_staged_construction_in_controller(
            staged,
            ReplayAction::ConstructionPlan {
                expected: Box::new(*expected),
                plan: plan.clone(),
            },
            &mut controller,
        ) {
            return Ok(controller.outcome_unchecked());
        }
        Ok(controller.outcome(value))
    }

    fn publish_construction_plan_trial(
        &mut self,
        expected: &PreparedSketchInput,
        plan: &ConstructionCommitPlan,
        trial: RetainedSketchDocumentSession,
        outcome: &geosolve_sketch::RetainedDocumentTransactionOutcome<ConstructionCommitResult>,
    ) -> Result<MutationOutcome<ConstructionCommitResult>, CoordinatorError> {
        let result = Self::validate_construction_plan_trial(&trial, outcome)?;
        let staged = self.stage_construction_publication(trial)?;
        self.publish_staged_construction(
            staged,
            ReplayAction::ConstructionPlan {
                expected: Box::new(*expected),
                plan: plan.clone(),
            },
        );
        Ok(result)
    }

    fn validate_construction_plan_trial(
        trial: &RetainedSketchDocumentSession,
        outcome: &geosolve_sketch::RetainedDocumentTransactionOutcome<ConstructionCommitResult>,
    ) -> Result<MutationOutcome<ConstructionCommitResult>, CoordinatorError> {
        if outcome.published_accepted_identity().is_none() {
            return Err(CoordinatorError::InferredConstructionNotAccepted);
        }
        let accepted = trial
            .accepted_state_for_current_input()
            .ok_or(CoordinatorError::InferredConstructionNotAccepted)?;
        let redundancy = accepted.accepted_redundancy();
        if let Some(source) = outcome
            .value()
            .constraints
            .iter()
            .filter(|constraint| {
                constraint.provenance == ConstructionRelationProvenance::AutoInference
            })
            .map(|constraint| constraint.source)
            .find(|source| {
                redundancy.fully_redundant_sources().contains(source)
                    || redundancy
                        .sources_containing_redundant_rows()
                        .contains(source)
            })
        {
            return Err(CoordinatorError::RedundantInferredConstruction {
                inferred_source: source,
            });
        }
        Ok(mutation_from(outcome))
    }

    fn ensure_construction_plan_input(
        &self,
        expected: &PreparedSketchInput,
    ) -> Result<(), CoordinatorError> {
        if self.session.accepted_prepared_input().as_ref() != Some(expected) {
            return Err(CoordinatorError::StaleInferredConstructionInput);
        }
        Ok(())
    }

    fn ensure_pending_construction_plan_compatible(
        &self,
        expected: &PreparedSketchInput,
        plan: &ConstructionCommitPlan,
    ) -> Result<(), CoordinatorError> {
        if self.editor.pending_construction_commit_token().is_some()
            && !self
                .editor
                .pending_construction_plan_matches(expected, plan)
        {
            return Err(CoordinatorError::InferredConstructionCommitMismatch);
        }
        Ok(())
    }

    fn stage_construction_publication(
        &self,
        session: RetainedSketchDocumentSession,
    ) -> Result<StagedConstructionPublication, CoordinatorError> {
        let mut computed_evaluation_allocator = self.computed_evaluation_allocator.clone();
        let (computed_input, computed_snapshot, computed_evaluation_problem) =
            match evaluate_computed_features(
                &session,
                &self.features,
                &mut computed_evaluation_allocator,
                bounded_geometry_control(),
            ) {
                Ok(OperationOutcome::Completed { value, .. }) => {
                    (Some(value.input()), Some(value), None)
                }
                Ok(stopped) => (
                    None,
                    None,
                    Some(format!(
                        "computed-feature evaluation stopped: {:?}",
                        stopped.report().stopping_reason
                    )),
                ),
                Err(error) => (None, None, Some(error.to_string())),
            };
        let next = checkpoint(&session, &self.features, &computed_evaluation_allocator)?;
        Ok(StagedConstructionPublication {
            session,
            computed_evaluation_allocator,
            computed_input,
            computed_snapshot,
            computed_evaluation_problem,
            checkpoint: next,
        })
    }

    fn stage_construction_publication_in_controller(
        &self,
        session: RetainedSketchDocumentSession,
        controller: &mut OperationController,
    ) -> Result<Option<StagedConstructionPublication>, CoordinatorError> {
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(None);
        }
        let mut computed_evaluation_allocator = self.computed_evaluation_allocator.clone();
        let (computed_input, computed_snapshot, computed_evaluation_problem) =
            match evaluate_computed_features_in_controller(
                &session,
                &self.features,
                &mut computed_evaluation_allocator,
                controller,
            ) {
                Ok(Some(value)) => (Some(value.input()), Some(value), None),
                Ok(None) => return Ok(None),
                Err(error) => (None, None, Some(error.to_string())),
            };
        let next = checkpoint(&session, &self.features, &computed_evaluation_allocator)?;
        Ok(Some(StagedConstructionPublication {
            session,
            computed_evaluation_allocator,
            computed_input,
            computed_snapshot,
            computed_evaluation_problem,
            checkpoint: next,
        }))
    }

    fn publish_staged_construction(
        &mut self,
        staged: StagedConstructionPublication,
        replay: ReplayAction,
    ) {
        let published_plan = match &replay {
            ReplayAction::ConstructionPlan { expected, plan } => Some((expected.as_ref(), plan)),
            _ => None,
        };
        self.session = staged.session;
        self.computed_evaluation_allocator = staged.computed_evaluation_allocator;
        self.computed_input = staged.computed_input;
        self.computed_snapshot = staged.computed_snapshot;
        self.computed_evaluation_problem = staged.computed_evaluation_problem;
        self.history.truncate(self.history_cursor + 1);
        self.history.push(staged.checkpoint);
        self.history_cursor += 1;
        if let Some((expected, plan)) = published_plan {
            let _ = self
                .editor
                .mark_construction_commit_published(expected, plan);
        }
        self.transcript.push(replay);
        self.editor.invalidate_for_retained_state_change(false);
        self.clear_transient();
        self.reconcile_selection();
    }

    fn publish_staged_construction_in_controller(
        &mut self,
        staged: StagedConstructionPublication,
        replay: ReplayAction,
        controller: &mut OperationController,
    ) -> bool {
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return false;
        }
        self.publish_staged_construction(staged, replay);
        true
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
        let previous_session = self.session.clone();
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
                let definition = simple_constraint_definition(
                    self.session.design_document(),
                    &selection,
                    resolved,
                )
                .ok_or(CoordinatorError::InvalidActionInput(
                    "contextual relation did not resolve to a simple constraint",
                ))?;
                let label = request.label;
                self.session.transact(expected, move |document| {
                    document.add_constraint(label, definition)
                })?
            }
        };
        let result = mutation_from(&outcome);
        self.record_mutation(
            ReplayAction::ConstraintAction {
                expected,
                selection,
                request: replay_request,
            },
            previous_session,
        )?;
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
        if choice.domain != ContactDomain::SupportingLine
            || choice.neighborhood != ContactNeighborhood::Interior
        {
            return Err(CoordinatorError::InvalidActionInput(
                "circle normal requires the complete supporting line",
            ));
        }
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
        let previous_session = self.session.clone();
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
        self.record_mutation(
            ReplayAction::DimensionAction {
                expected,
                selection,
                request: replay_request,
            },
            previous_session,
        )?;
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

    #[allow(
        clippy::too_many_lines,
        reason = "one auditable request builder authenticates every contextual constraint family and its contact metadata"
    )]
    fn authoring_constraint_request(
        &self,
        intent: ConstraintIntent,
        resolved: ResolvedConstraintKind,
        selection: &[SelectionItem],
        operands: &[AuthoringOperand],
        options: AuthoringOptions,
    ) -> Result<ConstraintActionRequest, CoordinatorError> {
        let document = self.session.design_document();
        let accepted_document = self.session.accepted_state().map_or(
            document,
            geosolve_sketch::SketchAcceptedDocumentState::document,
        );
        if resolved == ResolvedConstraintKind::RadialLine {
            return radial_line_authoring_request(document, accepted_document, intent, selection);
        }
        let contact_operands = match resolved {
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
                    | SelectionItem::Datum(_)
                    | SelectionItem::Feature(_)
                    | SelectionItem::FeatureCorner(_) => None,
                })
                .collect(),
            ResolvedConstraintKind::FixedPoint
            | ResolvedConstraintKind::CoincidentWithOrigin
            | ResolvedConstraintKind::PointOnDatumAxis
            | ResolvedConstraintKind::CoincidentPoints
            | ResolvedConstraintKind::HorizontalLine
            | ResolvedConstraintKind::VerticalLine
            | ResolvedConstraintKind::HorizontalPoints
            | ResolvedConstraintKind::VerticalPoints
            | ResolvedConstraintKind::ConcentricCurves
            | ResolvedConstraintKind::CollinearSupports
            | ResolvedConstraintKind::CollinearWithDatumAxis
            | ResolvedConstraintKind::ParallelLines
            | ResolvedConstraintKind::PerpendicularLines
            | ResolvedConstraintKind::EqualLength
            | ResolvedConstraintKind::EqualRadius
            | ResolvedConstraintKind::Midpoint
            | ResolvedConstraintKind::SymmetricAboutLine
            | ResolvedConstraintKind::SymmetricAboutDatumAxis
            | ResolvedConstraintKind::RadialLine => Vec::new(),
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
        let previous_session = self.session.clone();
        let outcome = self
            .session
            .apply(expected, DocumentEdit::SetContactBranches { edits })?;
        let result = mutation_from(&outcome);
        self.record_mutation(
            ReplayAction::SetContactBranches {
                expected,
                selection,
                edits: replay_edits,
            },
            previous_session,
        )?;
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
        let previous_session = self.session.clone();
        let outcome = self.session.apply(
            expected,
            DocumentEdit::SetOrientedAngleOrientation {
                dimension,
                orientation,
            },
        )?;
        let result = mutation_from(&outcome);
        self.record_mutation(
            ReplayAction::SetAngleOrientation {
                expected,
                dimension,
                orientation,
            },
            previous_session,
        )?;
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
        let previous_session = self.session.clone();
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
        self.record_mutation(
            ReplayAction::PointDistance {
                expected,
                points: [first, second],
                mode,
                label: replay_label,
            },
            previous_session,
        )?;
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
        let previous_session = self.session.clone();
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
        self.record_mutation(
            ReplayAction::SegmentLength {
                expected,
                curve,
                mode,
                label: replay_label,
            },
            previous_session,
        )?;
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
        let previous_session = self.session.clone();
        let outcome = self
            .session
            .apply(expected, DocumentEdit::SetDimensionMode { dimension, mode })?;
        let result = mutation_from(&outcome);
        self.record_mutation(
            ReplayAction::SetDimensionMode {
                expected,
                dimension,
                mode,
            },
            previous_session,
        )?;
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
        if selection_contains_datum(&selection) {
            return Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum,
            ));
        }
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
        let previous_session = self.session.clone();
        let outcome = self.session.transact(expected, move |document| {
            document.remove_many_with_dependents(&objects)?;
            Ok(objects)
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(
            ReplayAction::Delete {
                expected,
                selection,
            },
            previous_session,
        )?;
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
        if selection_contains_datum(&selection) {
            return Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum,
            ));
        }
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
        let previous_session = self.session.clone();
        let outcome = self.session.transact(expected, move |document| {
            for source in &sources {
                document.set_source_suppressed(*source, suppressed)?;
            }
            Ok(sources)
        })?;
        let result = mutation_from(&outcome);
        self.record_mutation(
            ReplayAction::SetSuppressed {
                expected,
                selection,
                suppressed,
            },
            previous_session,
        )?;
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
        self.editor.invalidate_for_retained_state_change(true);
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

    #[allow(
        clippy::too_many_lines,
        reason = "exact preview authentication, native release, durable feature re-anchor and atomic publication form one transaction boundary"
    )]
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
        let before_sketch = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        let before_features = self.features.identity();
        let computed_continuation = self
            .computed_preview_snapshot
            .as_ref()
            .filter(|snapshot| {
                self.computed_preview_input == Some(snapshot.input())
                    && snapshot.input().sketch == preview.prepared_input()
                    && snapshot.input().features == before_features
            })
            .cloned()
            .or_else(|| {
                self.drag_continuation
                    .as_ref()
                    .and_then(|gesture| gesture.last_valid_computed_snapshot.clone())
                    .filter(|snapshot| {
                        snapshot.input().sketch == preview.prepared_input()
                            && snapshot.input().features == before_features
                    })
            })
            .ok_or(CoordinatorError::ComputedFeaturePreviewInvalidated)?;
        let edit = DocumentEdit::SetPointPosition {
            point,
            position: model_position,
        };
        let mut candidate_session = self.session.clone();
        let retained = if let Some(locality) = locality.as_ref() {
            complete_projected_drag_release(
                candidate_session.apply_point_position_from_preview_with_drag_locality_controlled(
                    expected,
                    point,
                    model_position,
                    &preview,
                    locality,
                    release_control,
                )?,
            )?
        } else {
            candidate_session.apply_point_position_from_preview(
                expected,
                point,
                model_position,
                &preview,
            )?
        };
        let after_sketch = candidate_session
            .accepted_prepared_input()
            .filter(|_| retained.published_accepted_identity().is_some())
            .ok_or(CoordinatorError::SolvedPreviewMismatch)?;
        if candidate_session.last_attempt().continuation_parent_input()
            != Some(computed_continuation.input().sketch)
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }

        let mut candidate_allocator = self.computed_evaluation_allocator.clone();
        let continued = evaluate_computed_features_continuing(
            &candidate_session,
            &self.features,
            &mut candidate_allocator,
            bounded_geometry_control(),
            Some(&computed_continuation),
        )?;
        let OperationOutcome::Completed {
            value: continued, ..
        } = continued
        else {
            return Err(CoordinatorError::ComputedFeatureWorkStopped);
        };
        if !computed_feature_preview_invalidations(
            &self.features,
            &computed_continuation,
            &continued,
        )
        .is_empty()
        {
            return Err(CoordinatorError::ComputedFeaturePreviewInvalidated);
        }
        let (candidate_features, cold) = evaluate_durable_computed_reanchor(
            &candidate_session,
            &self.features,
            &mut candidate_allocator,
            &continued,
        )?;
        if cold.input().sketch != after_sketch
            || cold.input().features != candidate_features.identity()
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let computed_features = (candidate_features.identity() != before_features)
            .then(|| {
                if !recorded_transition_is_reanchor_only(&self.features, &candidate_features) {
                    return Err(CoordinatorError::ComputedFeatureReanchorNotDurable);
                }
                Ok(Box::new(RecordedComputedFeatureTransition {
                    edit: edit.clone(),
                    before_sketch,
                    after_sketch,
                    before: before_features,
                    after: candidate_features.clone(),
                    dispositions: recorded_computed_feature_dispositions(&cold),
                }))
            })
            .transpose()?;
        let replay = ReplayAction::Edit {
            expected,
            edit,
            computed_features,
        };
        let next = checkpoint(
            &candidate_session,
            &candidate_features,
            &candidate_allocator,
        )?;
        let outcome = MutationOutcome {
            value: retained.value().clone(),
            design: retained.design_identity(),
            attempt: retained.attempt_identity(),
            published_accepted: retained.published_accepted_identity(),
        };

        self.session = candidate_session;
        self.features = candidate_features;
        self.computed_evaluation_allocator = candidate_allocator;
        self.computed_input = Some(cold.input());
        self.computed_snapshot = Some(cold);
        self.computed_evaluation_problem = None;
        self.record_feature_mutation(next, replay);
        Ok(MutationOutcome {
            value: EditorMutation::PointMove(outcome.value),
            design: outcome.design,
            attempt: outcome.attempt,
            published_accepted: outcome.published_accepted,
        })
    }

    fn commit_curve_control_preview(
        &mut self,
        expected: SketchDesignIdentity,
        pointer_id: u64,
        request_id: u64,
        control: DocumentCurveControlId,
    ) -> Result<Option<MutationOutcome<EditorMutation>>, CoordinatorError> {
        let outcome =
            self.commit_curve_control_preview_inner(expected, pointer_id, request_id, control);
        if outcome.is_err() {
            self.clear_curve_control_preview();
        }
        outcome
    }

    #[allow(
        clippy::too_many_lines,
        reason = "exact patch authentication, computed-feature re-anchor and atomic history publication form one transaction boundary"
    )]
    fn commit_curve_control_preview_inner(
        &mut self,
        expected: SketchDesignIdentity,
        pointer_id: u64,
        request_id: u64,
        control: DocumentCurveControlId,
    ) -> Result<Option<MutationOutcome<EditorMutation>>, CoordinatorError> {
        self.ensure_expected(expected)?;
        let Some(gesture) = self.curve_control_continuation.as_ref() else {
            return Err(CoordinatorError::MissingSolvedPreview);
        };
        if gesture.pointer_id != pointer_id
            || gesture.control != control
            || gesture.expected != expected
            || gesture.base != self.session.prepared_input()
        {
            return Err(CoordinatorError::SolvedPreviewMismatch);
        }
        let Some(sample) = gesture.last_accepted.as_ref() else {
            return Err(CoordinatorError::MissingSolvedPreview);
        };
        if sample.request_id() != request_id || sample.control() != control {
            return Err(CoordinatorError::SolvedPreviewMismatch);
        }
        let CurveControlAcceptedSample::Changed(preview) = sample else {
            self.clear_curve_control_preview();
            return Ok(None);
        };

        let before_sketch = self
            .session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        let before_features = self.features.identity();
        let proposed = preview.patch.proposed_commit();
        let patch_preview = preview.patch.preview();
        let candidate_session = patch_preview
            .accepted_session()
            .ok_or(CoordinatorError::PreviewNotAccepted)?;
        let after_sketch = candidate_session
            .accepted_prepared_input()
            .filter(|input| {
                input.design_identity() == proposed.design_identity()
                    && input.latest_attempt_identity() == proposed.attempt_identity()
                    && input.accepted_state_identity() == proposed.accepted_state_identity()
            })
            .ok_or(CoordinatorError::SolvedPreviewMismatch)?;
        if preview.computed_snapshot.input().sketch != after_sketch
            || preview.computed_snapshot.input().features != before_features
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }

        let mut candidate_allocator = preview.computed_allocator.clone();
        let (candidate_features, cold) = evaluate_durable_computed_reanchor(
            candidate_session,
            &self.features,
            &mut candidate_allocator,
            &preview.computed_snapshot,
        )?;
        if cold.input().sketch != after_sketch
            || cold.input().features != candidate_features.identity()
        {
            return Err(CoordinatorError::StaleComputedFeatureCandidate);
        }
        let computed_features = (candidate_features.identity() != before_features)
            .then(|| {
                if !recorded_transition_is_reanchor_only(&self.features, &candidate_features) {
                    return Err(CoordinatorError::ComputedFeatureReanchorNotDurable);
                }
                Ok(Box::new(RecordedComputedFeatureTransition {
                    edit: preview.edit.clone(),
                    before_sketch,
                    after_sketch,
                    before: before_features,
                    after: candidate_features.clone(),
                    dispositions: recorded_computed_feature_dispositions(&cold),
                }))
            })
            .transpose()?;
        let replay = ReplayAction::Edit {
            expected,
            edit: preview.edit.clone(),
            computed_features,
        };
        let next = checkpoint(candidate_session, &candidate_features, &candidate_allocator)?;
        let effect = curve_control_command_effect(&preview.edit)?;

        let mut gesture = self
            .curve_control_continuation
            .take()
            .ok_or(CoordinatorError::MissingSolvedPreview)?;
        let sample = gesture
            .last_accepted
            .take()
            .ok_or(CoordinatorError::MissingSolvedPreview)?;
        let CurveControlAcceptedSample::Changed(preview) = sample else {
            return Err(CoordinatorError::SolvedPreviewMismatch);
        };

        let committed = match self.session.commit_prepared_patch(preview.patch) {
            Ok(committed) => committed,
            Err(error) => {
                // A stale compare-and-swap patch cannot be reconstructed after consumption. The
                // live session remains unchanged and the editor will revoke the gesture through
                // its retained-state invalidation path.
                return Err(error.into());
            }
        };
        if committed != proposed {
            return Err(CoordinatorError::SolvedPreviewMismatch);
        }
        self.features = candidate_features;
        self.computed_evaluation_allocator = candidate_allocator;
        self.computed_input = Some(cold.input());
        self.computed_snapshot = Some(cold);
        self.computed_evaluation_problem = None;
        self.record_feature_mutation(next, replay);
        Ok(Some(MutationOutcome {
            value: EditorMutation::CurveControl(effect),
            design: committed.design_identity(),
            attempt: committed.attempt_identity(),
            published_accepted: committed.accepted_state_identity(),
        }))
    }

    fn clear_curve_control_preview(&mut self) {
        if self.curve_control_continuation.take().is_none() {
            return;
        }
        self.transient = None;
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_preview_evaluation_problem = None;
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one closed effect dispatcher keeps all editor preview/commit/rollback variants exhaustive"
    )]
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
            EditorEffect::CommitCurveControl {
                expected,
                pointer_id,
                request_id,
                control,
            } => self.commit_curve_control_preview(*expected, *pointer_id, *request_id, *control),
            EditorEffect::ClearCurveControlPreview => {
                self.clear_curve_control_preview();
                Ok(None)
            }
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
                let prior_snapshot = self.computed_preview_snapshot.clone();
                let prior_input = self.computed_preview_input;
                let prior_preview = self.computed_fillet_preview.clone();
                if let Err(error) =
                    self.preview_computed_fillet_radius_exact(*expected, *feature, *radius)
                {
                    if let Some(limit) = coordinator_computed_fillet_limit(&error) {
                        self.editor.reject_computed_feature_radius_preview(
                            expected, *feature, *radius, limit,
                        );
                    }
                    return Err(error);
                }
                if !self
                    .editor
                    .accept_computed_feature_radius_preview(expected, *feature, *radius)
                {
                    self.computed_preview_snapshot = prior_snapshot;
                    self.computed_preview_input = prior_input;
                    self.computed_fillet_preview = prior_preview;
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
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
                self.publish_computed_fillet_preview(expected, *feature, *radius)?;
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
            EditorEffect::PreviewComputedFeatureContact {
                expected,
                owner,
                parent,
                source,
                parameter,
            } => {
                if self.feature_authoring_preview.is_some() {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                let prior_snapshot = self.computed_preview_snapshot.clone();
                let prior_input = self.computed_preview_input;
                let prior_preview = self.computed_fillet_preview.clone();
                if let Err(error) = self.prepare_computed_fillet_contact_preview(
                    expected, *owner, *parent, *source, *parameter,
                ) {
                    if let Some(limit) = coordinator_computed_fillet_limit(&error) {
                        self.editor.reject_computed_feature_contact_preview(
                            expected, *owner, *parent, *source, *parameter, limit,
                        );
                    }
                    return Err(error);
                }
                if !self.editor.accept_computed_feature_contact_preview(
                    expected, *owner, *parent, *source, *parameter,
                ) {
                    self.computed_preview_snapshot = prior_snapshot;
                    self.computed_preview_input = prior_input;
                    self.computed_fillet_preview = prior_preview;
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                Ok(None)
            }
            EditorEffect::CommitComputedFeatureContact {
                expected,
                owner,
                parent,
                source,
                parameter,
            } => {
                if self.feature_authoring_preview.is_some() {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                self.publish_computed_fillet_contact_preview(
                    expected, *owner, *parent, *source, *parameter,
                )?;
                Ok(None)
            }
            EditorEffect::RestoreComputedFeatureContact { expected, .. } => {
                if self.feature_authoring_preview.is_some()
                    || self.computed_evaluation_input()? != *expected
                {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                self.clear_computed_feature_preview();
                Ok(None)
            }
            EditorEffect::ClearComputedFeaturePreview
            | EditorEffect::ClearComputedFeatureContactPreview => {
                self.clear_computed_feature_preview();
                Ok(None)
            }
            EditorEffect::CommitComputedFilletAction { target } => {
                if self.feature_authoring_preview.is_some() {
                    return Err(CoordinatorError::FeatureAuthoringPreviewMismatch);
                }
                self.apply_computed_fillet_action(target.expected, target.owner, target.action)?;
                Ok(None)
            }
            EditorEffect::CommitConstruction {
                expected,
                proposal,
                role,
            } => {
                self.ensure_expected(*expected)?;
                let outcome = self.apply_construction_with_role(*expected, proposal, *role)?;
                Ok(Some(MutationOutcome {
                    value: EditorMutation::Construction(outcome.value),
                    design: outcome.design,
                    attempt: outcome.attempt,
                    published_accepted: outcome.published_accepted,
                }))
            }
            EditorEffect::CommitConstructionPlan {
                expected,
                token,
                plan,
            } => {
                if !self
                    .editor
                    .authenticates_construction_commit(*token, expected.as_ref(), plan)
                {
                    return Err(CoordinatorError::InferredConstructionCommitMismatch);
                }
                let outcome = self.apply_construction_plan(expected.as_ref(), plan)?;
                Ok(Some(MutationOutcome {
                    value: EditorMutation::InferredConstruction(outcome.value),
                    design: outcome.design,
                    attempt: outcome.attempt,
                    published_accepted: outcome.published_accepted,
                }))
            }
            EditorEffect::SelectionChanged(_)
            | EditorEffect::HoverChanged(_)
            | EditorEffect::FilletBranchPreviewChanged { .. }
            | EditorEffect::PreviewPointMove { .. }
            | EditorEffect::RequestProjectedPointMove { .. }
            | EditorEffect::ClearPointPreview
            | EditorEffect::RequestCurveControlPreview { .. }
            | EditorEffect::PreviewCurveControl { .. }
            | EditorEffect::PreviewConstruction(_)
            | EditorEffect::ClearConstructionPreview
            | EditorEffect::DraftInferenceChanged(_) => Ok(None),
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
            ReplayAction::SetComputedFilletConfiguration {
                expected,
                feature,
                radius,
                corners,
            } => {
                self.apply_computed_fillet_configuration(
                    *expected,
                    *feature,
                    *radius,
                    corners.clone(),
                    action.clone(),
                )?;
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
            ReplayAction::Edit {
                expected,
                edit,
                computed_features,
            } => {
                if let Some(computed_features) = computed_features {
                    self.apply_recorded_edit_with_computed_features(
                        *expected,
                        edit.clone(),
                        computed_features,
                        action.clone(),
                    )?;
                } else {
                    self.apply_edit(*expected, edit.clone())?;
                }
            }
            ReplayAction::Construction {
                expected,
                proposal,
                role,
            } => {
                self.apply_construction_with_role(*expected, proposal, *role)?;
            }
            ReplayAction::ConstructionPlan { expected, plan } => {
                self.apply_construction_plan(expected.as_ref(), plan)?;
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
        let checkpoint = self.history[target].clone();
        let current_checkpoint = &self.history[self.history_cursor];
        let sketch_unchanged = self.session.accepted_state_for_current_input().is_some()
            && checkpoint.design_json == current_checkpoint.design_json
            && checkpoint.design_is_draft_v5 == current_checkpoint.design_is_draft_v5
            && checkpoint.accepted_json == current_checkpoint.accepted_json
            && checkpoint.accepted_is_draft_v5 == current_checkpoint.accepted_is_draft_v5
            && checkpoint.accepted_belongs_to_current_design
                == current_checkpoint.accepted_belongs_to_current_design;
        if sketch_unchanged {
            let high_water = self
                .session
                .persistent_identity_high_water()
                .merged(&checkpoint.sketch_identity_high_water)?;
            self.session
                .retain_persistent_identity_high_water(&high_water)?;
        } else {
            let revisions = self.session.revision_high_water();
            self.session = restore_sketch_checkpoint(
                &self.session,
                &checkpoint,
                revisions,
                AcceptedCheckpointRestore::PreferCurrentInputTruth,
            )?;
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
        self.editor.invalidate_for_retained_state_change(true);
        self.clear_transient();
        self.refresh_computed_features();
        self.reconcile_selection();
        Ok(())
    }

    fn record_mutation(
        &mut self,
        replay: ReplayAction,
        previous_session: RetainedSketchDocumentSession,
    ) -> Result<(), CoordinatorError> {
        let previous = self.computed_snapshot.clone();
        self.record_mutation_continuing(replay, previous.as_ref(), previous_session)
    }

    fn record_mutation_continuing(
        &mut self,
        mut replay: ReplayAction,
        previous: Option<&ComputedFeatureSnapshot>,
        previous_session: RetainedSketchDocumentSession,
    ) -> Result<(), CoordinatorError> {
        let preserve_pending_plan = matches!(&replay, ReplayAction::ConstructionPlan { .. });
        let recorded_edit = match &replay {
            ReplayAction::Edit { edit, .. } => Some(edit.clone()),
            _ => None,
        };
        let previous_features = self.features.clone();
        let previous_allocator = self.computed_evaluation_allocator.clone();
        let previous_computed_input = self.computed_input;
        let previous_computed_snapshot = self.computed_snapshot.clone();
        let previous_computed_preview_input = self.computed_preview_input;
        let previous_computed_preview_snapshot = self.computed_preview_snapshot.clone();
        let previous_computed_fillet_preview = self.computed_fillet_preview.clone();
        let previous_computed_evaluation_problem = self.computed_evaluation_problem.clone();
        let previous_computed_preview_evaluation_problem =
            self.computed_preview_evaluation_problem.clone();
        let computed_features =
            self.refresh_computed_features_continuing(previous, true, recorded_edit.as_ref());
        if let ReplayAction::Edit {
            computed_features: recorded,
            ..
        } = &mut replay
            && recorded.is_none()
        {
            *recorded = computed_features.map(Box::new);
        }
        let next = match checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        ) {
            Ok(next) => next,
            Err(error) => {
                self.session = previous_session;
                self.features = previous_features;
                self.computed_evaluation_allocator = previous_allocator;
                self.computed_input = previous_computed_input;
                self.computed_snapshot = previous_computed_snapshot;
                self.computed_preview_input = previous_computed_preview_input;
                self.computed_preview_snapshot = previous_computed_preview_snapshot;
                self.computed_fillet_preview = previous_computed_fillet_preview;
                self.computed_evaluation_problem = previous_computed_evaluation_problem;
                self.computed_preview_evaluation_problem =
                    previous_computed_preview_evaluation_problem;
                return Err(error);
            }
        };
        self.history.truncate(self.history_cursor + 1);
        self.history.push(next);
        self.history_cursor += 1;
        self.transcript.push(replay);
        let preserve_pending_ack = self.editor.pending_construction_commit_token().is_some();
        self.editor
            .invalidate_for_retained_state_change(!preserve_pending_plan && !preserve_pending_ack);
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
        let next = self.stage_feature_mutation_checkpoint(&candidate)?;
        self.features = candidate;
        self.computed_input = Some(snapshot.input());
        self.computed_snapshot = Some(snapshot);
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_fillet_preview = None;
        self.computed_evaluation_problem = None;
        self.record_feature_mutation(next, replay);
        Ok(ComputedFeatureMutation {
            value,
            before,
            after,
        })
    }

    fn stage_feature_mutation_checkpoint(
        &self,
        features: &ComputedFeatureDocument,
    ) -> Result<RestoreCheckpoint, CoordinatorError> {
        checkpoint(&self.session, features, &self.computed_evaluation_allocator)
    }

    fn record_feature_mutation(&mut self, next: RestoreCheckpoint, replay: ReplayAction) {
        self.history.truncate(self.history_cursor + 1);
        self.history.push(next);
        self.history_cursor += 1;
        self.transcript.push(replay);
        self.editor.invalidate_for_retained_state_change(true);
        self.clear_transient();
        self.reconcile_selection();
    }

    fn refresh_computed_features(&mut self) {
        self.refresh_computed_features_continuing(None, false, None);
    }

    fn begin_durable_computed_refresh(&mut self) {
        self.computed_evaluation_problem = None;
        self.computed_preview_snapshot = None;
        self.computed_preview_input = None;
        self.computed_fillet_preview = None;
        self.computed_preview_evaluation_problem = None;
    }

    fn refresh_computed_features_continuing(
        &mut self,
        previous: Option<&ComputedFeatureSnapshot>,
        promote_current_branch: bool,
        recorded_edit: Option<&DocumentEdit>,
    ) -> Option<RecordedComputedFeatureTransition> {
        self.begin_durable_computed_refresh();
        let before = self.features.identity();
        let previous = previous.filter(|snapshot| snapshot.input().features == before);
        let before_sketch = previous.map(|snapshot| snapshot.input().sketch);
        let continued = (promote_current_branch && recorded_edit.is_some())
            .then_some(previous)
            .flatten()
            .and_then(|previous| {
                match evaluate_computed_features_continuing(
                    &self.session,
                    &self.features,
                    &mut self.computed_evaluation_allocator,
                    bounded_geometry_control(),
                    Some(previous),
                ) {
                    Ok(OperationOutcome::Completed { value, .. }) => Some(value),
                    Ok(stopped) => {
                        self.computed_evaluation_problem = Some(format!(
                            "computed-feature branch continuation stopped: {:?}",
                            stopped.report().stopping_reason
                        ));
                        None
                    }
                    Err(error) => {
                        self.computed_evaluation_problem = Some(format!(
                            "computed-feature branch continuation failed: {error}"
                        ));
                        None
                    }
                }
            });

        let promoted = continued.and_then(|continued| {
            match evaluate_durable_computed_reanchor(
                &self.session,
                &self.features,
                &mut self.computed_evaluation_allocator,
                &continued,
            ) {
                Ok((features, cold)) => Some((features, cold)),
                Err(error) => {
                    self.computed_evaluation_problem = Some(format!(
                        "computed-feature branch promotion was not durable: {error}"
                    ));
                    None
                }
            }
        });

        if let Some((features, snapshot)) = promoted {
            self.features = features;
            self.computed_input = Some(snapshot.input());
            self.computed_snapshot = Some(snapshot);
            self.computed_evaluation_problem = None;
        } else {
            let prior_problem = self.computed_evaluation_problem.take();
            match evaluate_computed_features(
                &self.session,
                &self.features,
                &mut self.computed_evaluation_allocator,
                bounded_geometry_control(),
            ) {
                Ok(OperationOutcome::Completed { value, .. }) => {
                    self.computed_input = Some(value.input());
                    self.computed_snapshot = Some(value);
                    self.computed_evaluation_problem = prior_problem;
                }
                Ok(stopped) => {
                    self.computed_input = None;
                    self.computed_snapshot = None;
                    self.computed_evaluation_problem = Some(format!(
                        "computed-feature evaluation stopped: {:?}{}",
                        stopped.report().stopping_reason,
                        prior_problem
                            .as_deref()
                            .map_or(String::new(), |problem| format!(" after {problem}"))
                    ));
                }
                Err(error) => {
                    self.computed_input = None;
                    self.computed_snapshot = None;
                    self.computed_evaluation_problem = Some(match prior_problem {
                        Some(problem) => format!("{problem}; cold evaluation failed: {error}"),
                        None => error.to_string(),
                    });
                }
            }
        }
        if self.features.identity() == before {
            return None;
        }
        Some(RecordedComputedFeatureTransition {
            edit: recorded_edit?.clone(),
            before_sketch: before_sketch?,
            after_sketch: self.session.accepted_prepared_input()?,
            before,
            after: self.features.clone(),
            dispositions: recorded_computed_feature_dispositions(self.computed_snapshot.as_ref()?),
        })
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
            Self::ConstructionPlan { expected, .. } => Some(expected.design_identity()),
            Self::CreateComputedFillet { .. }
            | Self::SetComputedFilletRadius { .. }
            | Self::SetComputedFilletConfiguration { .. }
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
        sketch_identity_high_water: session.persistent_identity_high_water().clone(),
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

fn restore_sketch_checkpoint(
    current: &RetainedSketchDocumentSession,
    checkpoint: &RestoreCheckpoint,
    revisions: SketchLifecycleRevisionHighWater,
    accepted_restore: AcceptedCheckpointRestore,
) -> Result<RetainedSketchDocumentSession, CoordinatorError> {
    let high_water = current
        .persistent_identity_high_water()
        .merged(&checkpoint.sketch_identity_high_water)?;
    let mut design =
        checkpoint_document_from_json(&checkpoint.design_json, checkpoint.design_is_draft_v5)?;
    design.retain_persistent_identity_high_water(&high_water)?;
    let input = current.last_attempt().input();
    let request = input
        .candidate_request()
        .without_temporary_targets()
        .without_previous_state_preferences();
    let parameters = current.parameter_batch().clone();
    let snapshots = current.external_snapshot_set().clone();
    let mut restored = if let Some(json) = &checkpoint.accepted_json {
        let mut accepted = checkpoint_document_from_json(json, checkpoint.accepted_is_draft_v5)?;
        accepted.retain_persistent_identity_high_water(&high_water)?;
        let exact = if checkpoint.accepted_belongs_to_current_design {
            RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
                design.clone(),
                accepted,
                revisions,
                parameters.clone(),
                snapshots.clone(),
                request,
                input.solver_config(),
            )
        } else {
            RetainedSketchDocumentSession::restore_design_with_accepted_and_inputs(
                design.clone(),
                accepted,
                revisions,
                parameters.clone(),
                snapshots.clone(),
                request,
                input.solver_config(),
            )
        };
        match (exact, accepted_restore) {
            (Ok(restored), _) => restored,
            (Err(error), AcceptedCheckpointRestore::RequireExact) => return Err(error.into()),
            (Err(_), AcceptedCheckpointRestore::PreferCurrentInputTruth) => {
                // Parameter values and external snapshots are host state rather than
                // sketch history. A historical accepted materialization can therefore
                // cease to certify after those inputs change. Restore the historical
                // design as a fresh attempt under the exact current inputs; it may
                // publish a newly accepted state or retain a typed input failure, but
                // the older accepted geometry must never masquerade as current.
                RetainedSketchDocumentSession::restore_design_with_inputs(
                    design,
                    revisions,
                    parameters,
                    snapshots,
                    request,
                    input.solver_config(),
                )?
            }
        }
    } else {
        RetainedSketchDocumentSession::restore_design_with_inputs(
            design,
            revisions,
            parameters,
            snapshots,
            request,
            input.solver_config(),
        )?
    };
    restored.retain_persistent_identity_high_water(&high_water)?;
    Ok(restored)
}

#[derive(Clone, Copy)]
enum AcceptedCheckpointRestore {
    /// Persistence reload treats the stored accepted payload as a strict contract.
    RequireExact,
    /// History restores design intent under host inputs that are not historical state.
    PreferCurrentInputTruth,
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
        ConstraintIntent::Concentric,
        ConstraintIntent::Collinear,
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
    if intent == ConstraintIntent::Lock && selection_contains_datum(selection) {
        return Err(DisabledReason::ProtectedDatum);
    }
    if intent == ConstraintIntent::Symmetric {
        return match symmetric_operands(document, selection)? {
            (_, _, SymmetricReference::Line(_)) => Ok(ResolvedConstraintKind::SymmetricAboutLine),
            (_, _, SymmetricReference::DatumAxis(_)) => {
                Ok(ResolvedConstraintKind::SymmetricAboutDatumAxis)
            }
        };
    }
    let resolved = match (intent, selection) {
        (ConstraintIntent::Lock, [SelectionItem::Point(_)]) => ResolvedConstraintKind::FixedPoint,
        (
            ConstraintIntent::Coincident,
            [
                SelectionItem::Point(_),
                SelectionItem::Datum(SketchDatum::Origin),
            ]
            | [
                SelectionItem::Datum(SketchDatum::Origin),
                SelectionItem::Point(_),
            ],
        ) => ResolvedConstraintKind::CoincidentWithOrigin,
        (
            ConstraintIntent::Coincident,
            [
                SelectionItem::Point(_),
                SelectionItem::Datum(SketchDatum::XAxis | SketchDatum::YAxis),
            ]
            | [
                SelectionItem::Datum(SketchDatum::XAxis | SketchDatum::YAxis),
                SelectionItem::Point(_),
            ],
        ) => ResolvedConstraintKind::PointOnDatumAxis,
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
            ConstraintIntent::Horizontal,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) if first != second => ResolvedConstraintKind::HorizontalPoints,
        (
            ConstraintIntent::Horizontal,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) if first == second => return Err(DisabledReason::SameSemanticOperand),
        (
            ConstraintIntent::Vertical,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) if first != second => ResolvedConstraintKind::VerticalPoints,
        (
            ConstraintIntent::Vertical,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) if first == second => return Err(DisabledReason::SameSemanticOperand),
        (
            ConstraintIntent::Concentric,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if first.curve != second.curve => {
            let first = document
                .resolve_center_ref(geosolve_sketch::DocumentCenterRef { curve: first.curve })
                .map_err(|_| DisabledReason::WrongOperandKind)?;
            let second = document
                .resolve_center_ref(geosolve_sketch::DocumentCenterRef {
                    curve: second.curve,
                })
                .map_err(|_| DisabledReason::WrongOperandKind)?;
            if first == second {
                return Err(DisabledReason::SameSemanticOperand);
            }
            ResolvedConstraintKind::ConcentricCurves
        }
        (
            ConstraintIntent::Concentric,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if first.curve == second.curve => return Err(DisabledReason::SameSemanticOperand),
        (
            ConstraintIntent::Collinear,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if first != second
            && line_endpoints(document, *first).is_ok()
            && line_endpoints(document, *second).is_ok() =>
        {
            ResolvedConstraintKind::CollinearSupports
        }
        (
            ConstraintIntent::Collinear,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) if first == second => return Err(DisabledReason::SameSemanticOperand),
        (
            ConstraintIntent::Collinear,
            [
                SelectionItem::Curve(line),
                SelectionItem::Datum(SketchDatum::XAxis | SketchDatum::YAxis),
            ]
            | [
                SelectionItem::Datum(SketchDatum::XAxis | SketchDatum::YAxis),
                SelectionItem::Curve(line),
            ],
        ) if line_endpoints(document, *line).is_ok() => {
            ResolvedConstraintKind::CollinearWithDatumAxis
        }
        (
            ConstraintIntent::Parallel,
            [
                SelectionItem::Curve(line),
                SelectionItem::Datum(SketchDatum::XAxis),
            ]
            | [
                SelectionItem::Datum(SketchDatum::XAxis),
                SelectionItem::Curve(line),
            ],
        ) if line_endpoints(document, *line).is_ok() => ResolvedConstraintKind::HorizontalLine,
        (
            ConstraintIntent::Parallel,
            [
                SelectionItem::Curve(line),
                SelectionItem::Datum(SketchDatum::YAxis),
            ]
            | [
                SelectionItem::Datum(SketchDatum::YAxis),
                SelectionItem::Curve(line),
            ],
        ) if line_endpoints(document, *line).is_ok() => ResolvedConstraintKind::VerticalLine,
        (
            ConstraintIntent::Perpendicular,
            [
                SelectionItem::Curve(line),
                SelectionItem::Datum(SketchDatum::XAxis),
            ]
            | [
                SelectionItem::Datum(SketchDatum::XAxis),
                SelectionItem::Curve(line),
            ],
        ) if line_endpoints(document, *line).is_ok() => ResolvedConstraintKind::VerticalLine,
        (
            ConstraintIntent::Perpendicular,
            [
                SelectionItem::Curve(line),
                SelectionItem::Datum(SketchDatum::YAxis),
            ]
            | [
                SelectionItem::Datum(SketchDatum::YAxis),
                SelectionItem::Curve(line),
            ],
        ) if line_endpoints(document, *line).is_ok() => ResolvedConstraintKind::HorizontalLine,
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
            let valid_arity = match intent {
                ConstraintIntent::Lock => selection.len() == 1,
                ConstraintIntent::Horizontal | ConstraintIntent::Vertical => {
                    matches!(selection.len(), 1 | 2)
                }
                ConstraintIntent::Coincident
                | ConstraintIntent::Parallel
                | ConstraintIntent::Perpendicular
                | ConstraintIntent::Equal
                | ConstraintIntent::Midpoint
                | ConstraintIntent::Tangent
                | ConstraintIntent::Continuity
                | ConstraintIntent::Concentric
                | ConstraintIntent::Collinear => selection.len() == 2,
                ConstraintIntent::Symmetric => selection.len() == 3,
            };
            return Err(if valid_arity {
                DisabledReason::WrongOperandKind
            } else {
                DisabledReason::WrongArity
            });
        }
    };
    Ok(resolved)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SymmetricReference {
    Line(CurveSpan),
    DatumAxis(geosolve_sketch::DocumentCoordinateAxis),
}

fn symmetric_operands(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Result<(DesignPointId, DesignPointId, SymmetricReference), DisabledReason> {
    if selection.len() != 3 {
        if let [SelectionItem::Point(first), SelectionItem::Point(second)] = selection
            && first == second
        {
            return Err(DisabledReason::SameSemanticOperand);
        }
        return Err(DisabledReason::WrongArity);
    }
    let mut points = selection.iter().filter_map(|item| match item {
        SelectionItem::Point(point) => Some(*point),
        _ => None,
    });
    let first = points.next().ok_or(DisabledReason::WrongOperandKind)?;
    let second = points.next().ok_or(DisabledReason::WrongOperandKind)?;
    if points.next().is_some() {
        return Err(DisabledReason::WrongOperandKind);
    }
    if first == second {
        return Err(DisabledReason::SameSemanticOperand);
    }
    let mut references = selection.iter().filter_map(|item| match item {
        SelectionItem::Curve(line) if line_endpoints(document, *line).is_ok() => {
            Some(SymmetricReference::Line(*line))
        }
        SelectionItem::Datum(datum) => datum.coordinate_axis().map(SymmetricReference::DatumAxis),
        SelectionItem::Point(_)
        | SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Curve(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => None,
    });
    let reference = references.next().ok_or(DisabledReason::WrongOperandKind)?;
    if references.next().is_some() {
        return Err(DisabledReason::WrongOperandKind);
    }
    Ok((first, second, reference))
}

pub(crate) fn selection_exists(document: &SketchDocument, item: SelectionItem) -> bool {
    match item {
        SelectionItem::Point(id) => document.point(id).is_some(),
        SelectionItem::Curve(span) => document
            .curve_spans(span.curve)
            .is_ok_and(|spans| spans.contains(&span)),
        SelectionItem::Constraint(id) => document.constraints().iter().any(|value| value.id == id),
        SelectionItem::Dimension(id) => document.dimensions().iter().any(|value| value.id == id),
        SelectionItem::Datum(_) => true,
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

pub(crate) fn line_endpoints(
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

fn curve_control_point_aliases_match_scene(scene: &EditorScene) -> bool {
    scene.curve_controls.iter().all(|control| {
        let DocumentCurveControlTarget::Point(point_id) = control.target else {
            return true;
        };
        let mut matches = scene.points.iter().filter(|point| point.id == point_id);
        matches.next().is_some_and(|point| {
            point.model_position.map(f64::to_bits) == control.model_position.map(f64::to_bits)
                && point.screen_position.x.to_bits() == control.screen_position.x.to_bits()
                && point.screen_position.y.to_bits() == control.screen_position.y.to_bits()
        }) && matches.next().is_none()
    })
}

fn curve_control_projection_edit(
    projection: DocumentCurveControlProjection,
) -> Option<DocumentEdit> {
    match projection {
        DocumentCurveControlProjection::Point { point, position } => {
            Some(DocumentEdit::SetPointPosition { point, position })
        }
        DocumentCurveControlProjection::Scalar { scalar, value } => {
            Some(DocumentEdit::SetScalarValue { scalar, value })
        }
        DocumentCurveControlProjection::RationalMiddle { curve, control } => {
            let weighted_middle = match control {
                DocumentRationalConicControl::Euclidean { middle, weight } => {
                    [middle[0] * weight, middle[1] * weight]
                }
                DocumentRationalConicControl::Projective {
                    weighted_middle, ..
                } => weighted_middle,
                _ => return None,
            };
            weighted_middle
                .iter()
                .all(|value| value.is_finite())
                .then_some(DocumentEdit::SetConicWeightedMiddle {
                    curve,
                    weighted_middle,
                })
        }
        _ => None,
    }
}

fn curve_control_command_effect(
    edit: &DocumentEdit,
) -> Result<DocumentCommandEffect, CoordinatorError> {
    match edit {
        DocumentEdit::SetPointPosition { point, .. } => {
            Ok(DocumentCommandEffect::UpdatedPoint(*point))
        }
        DocumentEdit::SetScalarValue { scalar, .. } => {
            Ok(DocumentCommandEffect::UpdatedScalar(*scalar))
        }
        DocumentEdit::SetConicWeightedMiddle { curve, .. } => {
            Ok(DocumentCommandEffect::UpdatedConicWeightedMiddle(*curve))
        }
        DocumentEdit::SetRationalConicControl { curve, .. } => {
            Ok(DocumentCommandEffect::UpdatedRationalConicControl(*curve))
        }
        _ => Err(CoordinatorError::InvalidActionInput(
            "prepared curve-control patch carries an unsupported edit",
        )),
    }
}

fn curve_direct_edit_availability(
    document: &SketchDocument,
    curve: CurveId,
    activity: &geosolve_sketch::EffectiveActivity,
) -> DocumentCurveControlAvailability {
    if !activity.is_active(curve) {
        return DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::InactiveCurve,
        );
    }
    if document.constraints().iter().any(|constraint| {
        activity.is_active(constraint.id)
            && matches!(
                constraint.definition,
                DocumentConstraintDefinition::LineLineFillet { arc, .. }
                    | DocumentConstraintDefinition::CurveCurveFillet { arc, .. }
                    if arc == curve
            )
    }) {
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::AssociativeFilletOutput,
        )
    } else {
        DocumentCurveControlAvailability::Editable
    }
}

fn ensure_curve_property_available(
    availability: DocumentCurveControlAvailability,
) -> Result<(), CoordinatorError> {
    match availability {
        DocumentCurveControlAvailability::Editable => Ok(()),
        DocumentCurveControlAvailability::ReadOnly(reason) => {
            Err(CoordinatorError::CurvePropertyUnavailable(reason))
        }
    }
}

fn curve_numeric_property_availability(
    document: &SketchDocument,
    curve: CurveId,
    scalar: DesignScalarId,
    direct_edit_availability: DocumentCurveControlAvailability,
    gauge_owned: bool,
) -> DocumentCurveControlAvailability {
    if direct_edit_availability != DocumentCurveControlAvailability::Editable {
        return direct_edit_availability;
    }
    if document.parameter_bindings().iter().any(|binding| {
        matches!(
            binding.target,
            DocumentParameterTarget::DimensionlessFixedScalar(property)
                if property.scalar == scalar
        )
    }) {
        return DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::HostParameterOwned,
        );
    }
    if let Ok(controls) = document.curve_controls(curve)
        && let Some(availability) = controls.iter().find_map(|control| {
            matches!(
                control.target,
                geosolve_sketch::DocumentCurveControlTarget::Scalar(owner) if owner == scalar
            )
            .then_some(control.availability)
        })
        && availability != DocumentCurveControlAvailability::Editable
    {
        return availability;
    }
    if gauge_owned {
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::GaugeOwned,
        )
    } else {
        DocumentCurveControlAvailability::Editable
    }
}

fn curve_control_edit_is_noop(document: &SketchDocument, edit: &DocumentEdit) -> bool {
    match edit {
        DocumentEdit::SetPointPosition { point, position } => document
            .point(*point)
            .is_some_and(|current| point_bits_equal(current.position, *position)),
        DocumentEdit::SetScalarValue { scalar, value } => document
            .scalar(*scalar)
            .is_some_and(|current| current.value.to_bits() == value.to_bits()),
        DocumentEdit::SetConicWeightedMiddle {
            curve,
            weighted_middle,
        } => document.curve(*curve).is_some_and(|current| {
            matches!(
                &current.definition,
                CurveDefinition::RationalQuadraticConic {
                    weighted_middle: current,
                    ..
                } if point_bits_equal(*current, *weighted_middle)
            )
        }),
        DocumentEdit::SetRationalConicControl { curve, control } => document
            .rational_conic_control(*curve)
            .is_ok_and(|current| rational_control_bits_equal(current, *control)),
        _ => false,
    }
}

fn point_bits_equal(first: [f64; 2], second: [f64; 2]) -> bool {
    first.map(f64::to_bits) == second.map(f64::to_bits)
}

fn rational_control_bits_equal(
    first: DocumentRationalConicControl,
    second: DocumentRationalConicControl,
) -> bool {
    match (first, second) {
        (
            DocumentRationalConicControl::Euclidean {
                middle: first_middle,
                weight: first_weight,
            },
            DocumentRationalConicControl::Euclidean {
                middle: second_middle,
                weight: second_weight,
            },
        )
        | (
            DocumentRationalConicControl::Projective {
                weighted_middle: first_middle,
                weight: first_weight,
            },
            DocumentRationalConicControl::Projective {
                weighted_middle: second_middle,
                weight: second_weight,
            },
        ) => {
            point_bits_equal(first_middle, second_middle)
                && first_weight.to_bits() == second_weight.to_bits()
        }
        _ => false,
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

/// Lowers an already-resolved simple contextual relation without repeating
/// applicability checks. [`resolve_constraint`] is the sole owner of operand
/// existence, kind, arity and semantic-compatibility policy.
#[allow(
    clippy::too_many_lines,
    reason = "the closed resolved-kind-to-definition matrix is clearest as one exhaustive dispatch"
)]
fn simple_constraint_definition(
    document: &SketchDocument,
    selection: &[SelectionItem],
    resolved: ResolvedConstraintKind,
) -> Option<DocumentConstraintDefinition> {
    Some(match (resolved, selection) {
        (ResolvedConstraintKind::FixedPoint, [SelectionItem::Point(point)]) => {
            DocumentConstraintDefinition::FixedPoint {
                point: *point,
                target: document.point(*point)?.position,
            }
        }
        (
            ResolvedConstraintKind::CoincidentWithOrigin,
            [
                SelectionItem::Point(point),
                SelectionItem::Datum(SketchDatum::Origin),
            ]
            | [
                SelectionItem::Datum(SketchDatum::Origin),
                SelectionItem::Point(point),
            ],
        ) => DocumentConstraintDefinition::CoincidentWithOrigin { point: *point },
        (
            ResolvedConstraintKind::PointOnDatumAxis,
            [SelectionItem::Point(point), SelectionItem::Datum(datum)]
            | [SelectionItem::Datum(datum), SelectionItem::Point(point)],
        ) => DocumentConstraintDefinition::PointOnDatumAxis {
            point: *point,
            axis: datum.coordinate_axis()?,
        },
        (
            ResolvedConstraintKind::CoincidentPoints,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) => DocumentConstraintDefinition::Coincident {
            first: *first,
            second: *second,
        },
        (
            ResolvedConstraintKind::HorizontalLine,
            [SelectionItem::Curve(line)]
            | [SelectionItem::Curve(line), SelectionItem::Datum(_)]
            | [SelectionItem::Datum(_), SelectionItem::Curve(line)],
        ) => DocumentConstraintDefinition::Horizontal { line: *line },
        (
            ResolvedConstraintKind::VerticalLine,
            [SelectionItem::Curve(line)]
            | [SelectionItem::Curve(line), SelectionItem::Datum(_)]
            | [SelectionItem::Datum(_), SelectionItem::Curve(line)],
        ) => DocumentConstraintDefinition::Vertical { line: *line },
        (
            ResolvedConstraintKind::HorizontalPoints,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) => DocumentConstraintDefinition::HorizontalPoints {
            first: *first,
            second: *second,
        },
        (
            ResolvedConstraintKind::VerticalPoints,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) => DocumentConstraintDefinition::VerticalPoints {
            first: *first,
            second: *second,
        },
        (
            ResolvedConstraintKind::ConcentricCurves,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::Concentric {
            first: geosolve_sketch::DocumentCenterRef { curve: first.curve },
            second: geosolve_sketch::DocumentCenterRef {
                curve: second.curve,
            },
        },
        (
            ResolvedConstraintKind::CollinearSupports,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::Collinear {
            first: geosolve_sketch::DocumentLineSupportRef {
                span: *first,
                direction: geosolve_sketch::DocumentDirectionSense::Forward,
            },
            second: geosolve_sketch::DocumentLineSupportRef {
                span: *second,
                direction: geosolve_sketch::DocumentDirectionSense::Forward,
            },
        },
        (
            ResolvedConstraintKind::CollinearWithDatumAxis,
            [SelectionItem::Curve(line), SelectionItem::Datum(datum)]
            | [SelectionItem::Datum(datum), SelectionItem::Curve(line)],
        ) => DocumentConstraintDefinition::CollinearWithDatumAxis {
            line: geosolve_sketch::DocumentLineSupportRef {
                span: *line,
                direction: geosolve_sketch::DocumentDirectionSense::Forward,
            },
            axis: datum.coordinate_axis()?,
        },
        (
            ResolvedConstraintKind::ParallelLines,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::Parallel {
            first: *first,
            second: *second,
        },
        (
            ResolvedConstraintKind::PerpendicularLines,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::Perpendicular {
            first: *first,
            second: *second,
        },
        (
            ResolvedConstraintKind::EqualLength,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::EqualLength {
            first: *first,
            second: *second,
        },
        (
            ResolvedConstraintKind::EqualRadius,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::EqualRadius {
            first: first.curve,
            second: second.curve,
        },
        (
            ResolvedConstraintKind::Midpoint,
            [SelectionItem::Point(point), SelectionItem::Curve(line)]
            | [SelectionItem::Curve(line), SelectionItem::Point(point)],
        ) => DocumentConstraintDefinition::Midpoint {
            point: *point,
            line: *line,
        },
        (ResolvedConstraintKind::SymmetricAboutLine, selection) => {
            let (first, second, SymmetricReference::Line(line)) =
                symmetric_operands(document, selection).ok()?
            else {
                return None;
            };
            DocumentConstraintDefinition::SymmetricAboutLine {
                first,
                second,
                line,
            }
        }
        (ResolvedConstraintKind::SymmetricAboutDatumAxis, selection) => {
            let (first, second, SymmetricReference::DatumAxis(axis)) =
                symmetric_operands(document, selection).ok()?
            else {
                return None;
            };
            DocumentConstraintDefinition::SymmetricAboutDatumAxis {
                first,
                second,
                axis,
            }
        }
        // Specialized contact-bearing kinds and any mismatched operand bundle
        // remain fail-closed at this simple lowerer.
        _ => return None,
    })
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

fn radial_line_contact_action_choice(
    topology: &SketchDocument,
    geometry: &SketchDocument,
    operand: u8,
    line: CurveSpan,
    center: DesignPointId,
) -> Option<ActionChoice> {
    topology
        .curve_contact_domains(line)
        .ok()?
        .contains(&ContactDomain::SupportingLine)
        .then_some(())?;
    let default_parameter = supporting_line_projection_parameter(geometry, line, center)?;
    Some(ActionChoice::Contact {
        operand,
        span: line,
        domains: vec![ContactDomain::SupportingLine],
        default_parameter,
        neighborhoods: vec![ContactNeighborhood::Interior],
        tangent_orientations: Vec::new(),
        default_winding: 0,
    })
}

fn radial_line_authoring_request(
    topology: &SketchDocument,
    geometry: &SketchDocument,
    intent: ConstraintIntent,
    selection: &[SelectionItem],
) -> Result<ConstraintActionRequest, CoordinatorError> {
    let (line, center, operand) =
        selected_radial_line(topology, selection).ok_or(CoordinatorError::InvalidActionInput(
            "circle normal requires one line and one circle or circular arc",
        ))?;
    let ActionChoice::Contact {
        domains,
        default_parameter,
        neighborhoods,
        default_winding,
        ..
    } = radial_line_contact_action_choice(topology, geometry, operand, line, center).ok_or(
        CoordinatorError::InvalidActionInput(
            "circle normal has no finite supporting-line projection",
        ),
    )?
    else {
        unreachable!("radial-line choice emits contact metadata");
    };
    let domain = *domains.first().ok_or(CoordinatorError::InvalidActionInput(
        "circle normal has no supporting-line domain",
    ))?;
    let neighborhood = *neighborhoods
        .first()
        .ok_or(CoordinatorError::InvalidActionInput(
            "circle normal has no supporting-line neighborhood",
        ))?;
    Ok(ConstraintActionRequest {
        intent,
        label: ResolvedConstraintKind::RadialLine.label().to_owned(),
        contacts: vec![crate::ContactActionChoice {
            support: geosolve_sketch::DocumentCurveSpanRef {
                span: line,
                winding: default_winding,
            },
            domain,
            parameter: default_parameter,
            neighborhood,
            tangent_orientation: None,
        }],
        relation: None,
    })
}

fn supporting_line_projection_parameter(
    document: &SketchDocument,
    line: CurveSpan,
    point: DesignPointId,
) -> Option<f64> {
    let (start, end) = line_endpoints(document, line).ok()?;
    let start = document.point(start)?.position;
    let end = document.point(end)?.position;
    let point = document.point(point)?.position;
    let direction = [end[0] - start[0], end[1] - start[1]];
    let offset = [point[0] - start[0], point[1] - start[1]];
    if !direction.into_iter().chain(offset).all(f64::is_finite) {
        return None;
    }
    let length = direction[0].hypot(direction[1]);
    if !length.is_finite() || length <= 0.0 {
        return None;
    }
    let unit = [direction[0] / length, direction[1] / length];
    let distance = offset[0].mul_add(unit[0], offset[1] * unit[1]);
    let parameter = distance / length;
    parameter.is_finite().then_some(parameter)
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
    if selection_contains_datum(selection) {
        return Err(DisabledReason::ProtectedDatum);
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
    if selection_contains_datum(selection) {
        return ActionState::Disabled(DisabledReason::ProtectedDatum);
    }
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
    if selection_contains_datum(selection) {
        return ActionState::Disabled(DisabledReason::ProtectedDatum);
    }
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
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => None,
        })
        .collect()
}

const fn selection_contains_datum(selection: &[SelectionItem]) -> bool {
    let mut index = 0;
    while index < selection.len() {
        if matches!(selection[index], SelectionItem::Datum(_)) {
            return true;
        }
        index += 1;
    }
    false
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
        AuthoringOutcome, AuthoringState, ConstructionPoint, EditorScene, EditorTool,
        FeatureAuthoringOutcome, FeatureAuthoringStage, FeatureAuthoringState, Modifiers,
        PickTolerance, PointerInput, ScreenPoint, Viewport,
    };
    use geosolve_sketch::{
        AlphaScenarioIds, AlphaScenarioKind, DocumentBSplineForm, DocumentConstraintDefinition,
        DocumentCurveNormalSide, DocumentExternalPointRef, DocumentM38DimensionDefinition,
        DocumentMeasurementDefinition, DocumentParameterKind, DocumentPointRef,
        ExternalLineOrientationV1, ExternalSnapshotDigest, ExternalSnapshotEntry,
        ExternalSnapshotFeatureV1, ExternalSnapshotInputError, ExternalSnapshotResourcesV1,
        ExternalSnapshotSet, OperationStopReason, OperationWorkCounter, ParameterBatch,
        ParameterBatchEntry, ParameterValue, PersistentId, SolverConfig, alpha_scenario,
        cancellation_pair,
    };
    use geosolve_sketch_features::{
        ComputedFeatureDefinition, ComputedFilletAuthoringOptions,
        ComputedFilletCornerAuthoringRequest, ComputedFilletCurvePick,
    };

    macro_rules! auto_relations {
        ($relation:expr; $count:expr) => {
            vec![crate::ConstructionRelationDefinition::auto_inference($relation); $count]
        };
        ($($relation:expr),* $(,)?) => {
            vec![$(crate::ConstructionRelationDefinition::auto_inference($relation)),*]
        };
    }

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

    #[test]
    fn geometry_policy_transition_clears_prethreshold_drag_continuation_only() {
        for policy in [
            GeometryInteractionPolicy {
                scope: crate::GeometryPickScope::Profile,
                ..GeometryInteractionPolicy::default()
            },
            GeometryInteractionPolicy {
                visibility: crate::GeometryVisibility {
                    explicit_construction: false,
                    implicit_construction: true,
                    reference_geometry: true,
                },
                ..GeometryInteractionPolicy::default()
            },
        ] {
            let (mut coordinator, scene, center, circle, _) = circle_drag_fixture();
            let retained = retained_state_snapshot(&coordinator);
            let history_cursor = coordinator.history_cursor();
            let press = unannotated_circle_press(&scene, circle);
            let effects = coordinator.pointer_down(
                &scene,
                PointerInput {
                    pointer_id: 94,
                    position: press,
                    modifiers: crate::Modifiers::default(),
                },
            );
            assert!(matches!(
                effects.as_slice(),
                [EditorEffect::SelectionChanged(selection)]
                    if selection == &[SelectionItem::Curve(CurveSpan::line(circle))]
            ));
            assert_eq!(
                coordinator
                    .editor
                    .point_gesture_snapshot()
                    .map(|gesture| gesture.point),
                Some(center)
            );
            assert!(coordinator.drag_continuation.is_some());
            let selection = coordinator.editor.selection().to_vec();
            let continuation = coordinator.drag_continuation.as_ref().map(|gesture| {
                (
                    gesture.gesture_epoch,
                    gesture.pointer_id,
                    gesture.point,
                    gesture.design,
                    gesture.accepted,
                    gesture.last_request_id,
                )
            });
            let unchanged = coordinator.editor.geometry_interaction_policy();
            assert!(
                coordinator
                    .set_geometry_interaction_policy(unchanged)
                    .is_empty()
            );
            assert!(coordinator.editor.point_gesture_snapshot().is_some());
            assert_eq!(
                coordinator.drag_continuation.as_ref().map(|gesture| {
                    (
                        gesture.gesture_epoch,
                        gesture.pointer_id,
                        gesture.point,
                        gesture.design,
                        gesture.accepted,
                        gesture.last_request_id,
                    )
                }),
                continuation,
                "an identical policy must retain the exact press-time continuation"
            );
            assert_eq!(coordinator.editor.selection(), selection);

            assert!(
                coordinator
                    .set_geometry_interaction_policy(policy)
                    .is_empty(),
                "a pre-threshold press emits no preview-clear effect"
            );
            assert!(coordinator.editor.point_gesture_snapshot().is_none());
            assert!(coordinator.drag_continuation.is_none());
            assert!(coordinator.transient.is_none());
            assert!(coordinator.solved_preview.is_none());
            assert!(coordinator.projected_drag_work.is_none());
            assert_eq!(coordinator.editor.selection(), selection);
            assert_eq!(coordinator.history_cursor(), history_cursor);
            assert_retained_state_snapshot(&coordinator, &retained);
        }
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

    fn inferred_normal_plan(reference: CurveSpan) -> ConstructionCommitPlan {
        ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.5, 0.0]),
                end: ConstructionPoint::New([0.5, 2.5]),
            },
            curve_roles: vec![GeometryRole::Construction],
            relations: auto_relations![
                crate::InferredRelation::PointOnCurve {
                    point: crate::DraftPointSlot::Created { point_index: 0 },
                    contact: crate::DraftContactDescriptor {
                        span: crate::DraftSpanSlot::Existing(reference),
                        domain: ContactDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        },
                        parameter: 0.25,
                        winding: 0,
                        neighborhood: ContactNeighborhood::Interior,
                    },
                },
                crate::InferredRelation::Perpendicular {
                    first: crate::DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    },
                    second: crate::DraftSpanSlot::Existing(reference),
                },
            ],
        }
    }

    fn redundant_inferred_plan(reference: CurveSpan) -> ConstructionCommitPlan {
        let created = crate::DraftSpanSlot::Created {
            curve_index: 0,
            segment: 0,
        };
        ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.0, 1.0]),
                end: ConstructionPoint::New([2.0, 1.0]),
            },
            curve_roles: vec![GeometryRole::Profile],
            relations: auto_relations![
                crate::InferredRelation::Horizontal { line: created },
                crate::InferredRelation::Parallel {
                    first: created,
                    second: crate::DraftSpanSlot::Existing(reference),
                },
            ],
        }
    }

    fn retained_document_payloads(
        coordinator: &RetainedEditorCoordinator,
    ) -> ((String, bool), Option<(String, bool)>) {
        let design = checkpoint_document_to_json(coordinator.session().design_document())
            .expect("design checkpoint payload");
        let accepted = coordinator.session().accepted_state().map(|accepted| {
            checkpoint_document_to_json(accepted.document()).expect("accepted checkpoint payload")
        });
        (design, accepted)
    }

    fn construction_commit_persistent_ids(
        result: &ConstructionCommitResult,
        document: &SketchDocument,
    ) -> BTreeSet<PersistentId> {
        let mut ids = result
            .construction
            .points
            .iter()
            .map(|id| id.0)
            .chain(result.construction.curves.iter().map(|id| id.0))
            .chain(result.contacts.iter().map(|result| result.contact.0))
            .chain(
                result
                    .constraints
                    .iter()
                    .flat_map(|result| [result.constraint.0, result.source.0]),
            )
            .collect::<BTreeSet<_>>();
        for contact in &result.contacts {
            ids.insert(
                document
                    .contact(contact.contact)
                    .expect("committed contact")
                    .parameter
                    .0,
            );
        }
        ids
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one transaction fixture keeps solve count, atomic contents, history, replay, and reload evidence together"
    )]
    fn inferred_construction_is_one_solve_checkpoint_with_exact_undo_redo_and_replay() {
        let (session, _, reference, _) = fixed_line_session();
        let replay_session = session.clone();
        let reload_session = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        let expected_input = coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");
        let initial_attempt = coordinator.session().last_attempt().identity();
        let initial_accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted baseline")
            .identity();
        let initial_history = coordinator.history_len();
        let initial_cursor = coordinator.history_cursor();
        let initial_points = coordinator.session().design_document().points().len();
        let initial_curves = coordinator.session().design_document().curves().len();
        let initial_contacts = coordinator.session().design_document().contacts().len();
        let initial_constraints = coordinator.session().design_document().constraints().len();
        let plan = inferred_normal_plan(reference);

        let outcome = coordinator
            .apply_construction_plan(&expected_input, &plan)
            .expect("accepted inferred construction");
        let published = outcome.published_accepted.expect("new accepted state");
        assert_eq!(
            outcome.design.revision().get(),
            expected.revision().get() + 1
        );
        assert_eq!(
            outcome.attempt.revision().get(),
            initial_attempt.revision().get() + 1
        );
        assert_eq!(
            published.revision().get(),
            initial_accepted.revision().get() + 1
        );
        assert_eq!(outcome.value.contacts.len(), 1);
        assert_eq!(outcome.value.constraints.len(), 2);
        assert_eq!(coordinator.history_len(), initial_history + 1);
        assert_eq!(coordinator.history_cursor(), initial_cursor + 1);
        assert_eq!(
            coordinator.session().design_document().points().len(),
            initial_points + 2
        );
        assert_eq!(
            coordinator.session().design_document().curves().len(),
            initial_curves + 1
        );
        assert_eq!(
            coordinator.session().design_document().contacts().len(),
            initial_contacts + 1
        );
        assert_eq!(
            coordinator.session().design_document().constraints().len(),
            initial_constraints + 2
        );
        let created_curve = outcome.value.construction.curves[0];
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .geometry_role(created_curve),
            Some(GeometryRole::Construction)
        );
        assert!(matches!(
            coordinator.transcript().last(),
            Some(ReplayAction::ConstructionPlan { plan: retained, .. }) if retained == &plan
        ));
        let replay = coordinator.transcript().last().expect("replay").clone();
        let committed_payloads = retained_document_payloads(&coordinator);
        let committed_identity_high_water = coordinator
            .session()
            .persistent_identity_high_water()
            .clone();

        coordinator.undo().expect("one-step undo");
        assert_eq!(coordinator.history_len(), initial_history + 1);
        assert_eq!(coordinator.history_cursor(), initial_cursor);
        let undone = coordinator.session().design_document();
        assert_eq!(undone.points().len(), initial_points);
        assert_eq!(undone.curves().len(), initial_curves);
        assert_eq!(undone.contacts().len(), initial_contacts);
        assert_eq!(undone.constraints().len(), initial_constraints);
        assert!(undone.curve(created_curve).is_none());
        assert!(undone.contact(outcome.value.contacts[0].contact).is_none());
        assert_eq!(
            coordinator.session().persistent_identity_high_water(),
            &committed_identity_high_water,
            "Undo removes objects but must not rewind their allocator lifecycle"
        );

        coordinator.redo().expect("one-step redo");
        assert_eq!(coordinator.history_cursor(), initial_cursor + 1);
        assert_eq!(retained_document_payloads(&coordinator), committed_payloads);
        for result in &outcome.value.constraints {
            assert!(
                coordinator
                    .session()
                    .design_document()
                    .constraint(result.constraint)
                    .is_some()
            );
        }
        assert!(
            coordinator
                .session()
                .design_document()
                .contact(outcome.value.contacts[0].contact)
                .is_some()
        );
        let saved = coordinator
            .persistence_checkpoint()
            .expect("committed persistence checkpoint");

        let mut replayed =
            RetainedEditorCoordinator::new(replay_session).expect("replay coordinator");
        replayed.replay(&replay).expect("construction-plan replay");
        assert_eq!(retained_document_payloads(&replayed), committed_payloads);
        assert_eq!(replayed.session().design_identity(), outcome.design);
        assert_eq!(
            replayed.session().last_attempt().identity(),
            outcome.attempt
        );
        assert_eq!(
            replayed
                .session()
                .accepted_state()
                .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
            outcome.published_accepted
        );

        let mut reloaded =
            RetainedEditorCoordinator::new(reload_session).expect("reload coordinator");
        reloaded.reload(&saved).expect("construction-plan reload");
        assert_eq!(retained_document_payloads(&reloaded), committed_payloads);
        assert_eq!(reloaded.history_len(), 1);
        assert_eq!(reloaded.history_cursor(), 0);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the focused lifecycle regression verifies every identity created by one atomic circle/contact plan"
    )]
    fn created_circle_constrains_an_existing_fixed_point_in_one_undoable_publication() {
        let (session, points, _, _) = fixed_line_session();
        let fixed_rim_point = points[1];
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");
        let initial_history = coordinator.history_len();
        let initial_cursor = coordinator.history_cursor();
        let initial_document = coordinator.session().design_document();
        let initial_points = initial_document.points().len();
        let initial_scalars = initial_document.scalars().len();
        let initial_curves = initial_document.curves().len();
        let initial_contacts = initial_document.contacts().len();
        let initial_constraints = initial_document.constraints().len();
        let initial_sources = initial_document.sources().count();
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Circle {
                center: ConstructionPoint::New([0.25, 0.0]),
                radius: 1.75,
            },
            curve_roles: vec![GeometryRole::Profile],
            relations: auto_relations![crate::InferredRelation::PointOnCurve {
                point: crate::DraftPointSlot::Existing(fixed_rim_point),
                contact: crate::DraftContactDescriptor {
                    span: crate::DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    },
                    domain: ContactDomain::Periodic {
                        period: std::f64::consts::TAU,
                    },
                    parameter: 0.0,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                },
            }],
        };

        let outcome = coordinator
            .apply_construction_plan(&expected, &plan)
            .expect("accepted circle/contact plan");
        assert!(outcome.published_accepted.is_some());
        assert_eq!(coordinator.history_len(), initial_history + 1);
        assert_eq!(coordinator.history_cursor(), initial_cursor + 1);
        assert_eq!(outcome.value.construction.points.len(), 1);
        assert_eq!(outcome.value.construction.scalars.len(), 1);
        assert_eq!(outcome.value.construction.curves.len(), 1);
        assert_eq!(outcome.value.contacts.len(), 1);
        assert_eq!(outcome.value.constraints.len(), 1);

        let center = outcome.value.construction.points[0];
        let radius = outcome.value.construction.scalars[0];
        let circle = outcome.value.construction.curves[0];
        let contact = outcome.value.contacts[0].contact;
        let constraint = outcome.value.constraints[0].constraint;
        let source = outcome.value.constraints[0].source;
        let committed = coordinator.session().design_document();
        let contact_parameter = committed.contact(contact).expect("contact").parameter;
        assert_eq!(committed.points().len(), initial_points + 1);
        assert_eq!(committed.scalars().len(), initial_scalars + 2);
        assert_eq!(committed.curves().len(), initial_curves + 1);
        assert_eq!(committed.contacts().len(), initial_contacts + 1);
        assert_eq!(committed.constraints().len(), initial_constraints + 1);
        assert_eq!(committed.sources().count(), initial_sources + 1);
        assert!(matches!(
            &committed.constraint(constraint).expect("constraint").definition,
            DocumentConstraintDefinition::PointOnCurve {
                point,
                contact: resolved_contact,
            } if *point == fixed_rim_point && *resolved_contact == contact
        ));
        assert_eq!(
            committed.contact(contact).expect("contact").curve,
            CurveSpan::line(circle)
        );
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted publication")
            .document();
        assert!(accepted.curve(circle).is_some());
        assert!(accepted.contact(contact).is_some());
        assert!(accepted.constraint(constraint).is_some());
        let committed_payloads = retained_document_payloads(&coordinator);

        coordinator.undo().expect("one-step Undo");
        assert_eq!(coordinator.history_cursor(), initial_cursor);
        let undone = coordinator.session().design_document();
        assert_eq!(undone.points().len(), initial_points);
        assert_eq!(undone.scalars().len(), initial_scalars);
        assert_eq!(undone.curves().len(), initial_curves);
        assert_eq!(undone.contacts().len(), initial_contacts);
        assert_eq!(undone.constraints().len(), initial_constraints);
        assert_eq!(undone.sources().count(), initial_sources);
        assert!(undone.point(center).is_none());
        assert!(undone.scalar(radius).is_none());
        assert!(undone.scalar(contact_parameter).is_none());
        assert!(undone.curve(circle).is_none());
        assert!(undone.contact(contact).is_none());
        assert!(undone.constraint(constraint).is_none());
        assert!(undone.source(source).is_none());
        assert_eq!(
            undone
                .point(fixed_rim_point)
                .expect("retained fixed point")
                .position
                .map(f64::to_bits),
            [2.0_f64.to_bits(), 0.0_f64.to_bits()]
        );
        let undone_accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted state after Undo")
            .document();
        assert!(undone_accepted.curve(circle).is_none());
        assert!(undone_accepted.contact(contact).is_none());
        assert!(undone_accepted.constraint(constraint).is_none());

        coordinator.redo().expect("one-step Redo");
        assert_eq!(coordinator.history_cursor(), initial_cursor + 1);
        assert_eq!(retained_document_payloads(&coordinator), committed_payloads);
        let redone = coordinator.session().design_document();
        assert!(redone.point(center).is_some());
        assert!(redone.scalar(radius).is_some());
        assert!(redone.scalar(contact_parameter).is_some());
        assert!(redone.curve(circle).is_some());
        assert!(redone.contact(contact).is_some());
        assert!(redone.constraint(constraint).is_some());
        assert!(redone.source(source).is_some());
    }

    #[test]
    fn undo_then_divergent_inferred_construction_never_reuses_persistent_ids() {
        let (session, _, reference, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let plan = inferred_normal_plan(reference);
        let first_input = coordinator
            .session()
            .accepted_prepared_input()
            .expect("first exact input");
        let first = coordinator
            .apply_construction_plan(&first_input, &plan)
            .expect("first inferred construction");
        let first_ids = construction_commit_persistent_ids(
            &first.value,
            coordinator.session().design_document(),
        );

        coordinator.undo().expect("undo first construction");
        let divergent_input = coordinator
            .session()
            .accepted_prepared_input()
            .expect("divergent exact input");
        let second = coordinator
            .apply_construction_plan(&divergent_input, &plan)
            .expect("divergent inferred construction");
        let second_ids = construction_commit_persistent_ids(
            &second.value,
            coordinator.session().design_document(),
        );

        assert!(first_ids.is_disjoint(&second_ids));
        assert!(!coordinator.can_redo());
    }

    #[test]
    fn reloading_an_undone_checkpoint_never_reuses_retired_persistent_ids() {
        let (session, _, reference, _) = fixed_line_session();
        let reload_seed = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let plan = inferred_normal_plan(reference);
        let first_input = coordinator
            .session()
            .accepted_prepared_input()
            .expect("first exact input");
        let first = coordinator
            .apply_construction_plan(&first_input, &plan)
            .expect("first inferred construction");
        let retired_ids = construction_commit_persistent_ids(
            &first.value,
            coordinator.session().design_document(),
        );

        coordinator.undo().expect("undo first construction");
        let saved = coordinator
            .persistence_checkpoint()
            .expect("undone persistence checkpoint");
        let mut reloaded = RetainedEditorCoordinator::new(reload_seed).expect("reload coordinator");
        reloaded.reload(&saved).expect("reload undone checkpoint");

        let second_input = reloaded
            .session()
            .accepted_prepared_input()
            .expect("reloaded exact input");
        let second = reloaded
            .apply_construction_plan(&second_input, &plan)
            .expect("post-reload inferred construction");
        let allocated_ids =
            construction_commit_persistent_ids(&second.value, reloaded.session().design_document());

        assert!(retired_ids.is_disjoint(&allocated_ids));
    }

    #[test]
    fn retained_history_never_reuses_an_undone_spline_span_id() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let controls = [[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [3.0, 1.5], [4.0, 0.0]]
            .map(|position| {
                document
                    .add_point("clamped control", position)
                    .expect("control")
            })
            .to_vec();
        let curve = document
            .add_curve(
                "clamped cubic",
                CurveDefinition::BSpline {
                    form: DocumentBSplineForm::Clamped,
                    degree: 3,
                    controls,
                    knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
                    span_ids: vec![41, 73],
                    next_span_id: 100,
                },
            )
            .expect("B-spline");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let first = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::InsertBSplineKnot {
                    curve,
                    parameter: 0.25,
                },
            )
            .expect("first insertion");
        let DocumentCommandEffect::InsertedBSplineKnot(first) = first.value else {
            panic!("expected first B-spline insertion");
        };
        assert_eq!(first.new_span_id, Some(100));

        coordinator.undo().expect("undo insertion");
        let divergent = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::InsertBSplineKnot {
                    curve,
                    parameter: 0.75,
                },
            )
            .expect("divergent insertion");
        let DocumentCommandEffect::InsertedBSplineKnot(divergent) = divergent.value else {
            panic!("expected divergent B-spline insertion");
        };
        assert_eq!(divergent.new_span_id, Some(101));
        assert!(!coordinator.can_redo());
    }

    #[test]
    fn redundant_inferred_bundle_is_rejected_without_design_or_history_mutation() {
        let (session, _, reference, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator.session().design_identity();
        let expected_input = coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");
        let before = coordinator
            .session()
            .export_design_json()
            .expect("design JSON");
        let accepted_before = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON");
        let identity_before = coordinator.session().last_attempt().identity();
        let history = coordinator.history_len();
        let cursor = coordinator.history_cursor();
        let transcript = coordinator.transcript().len();
        let evaluation_high_water = coordinator
            .persistence_checkpoint()
            .expect("persistence checkpoint")
            .computed_evaluation_high_water();
        let plan = redundant_inferred_plan(reference);

        let result = coordinator.apply_construction_plan(&expected_input, &plan);
        assert!(
            matches!(
                result,
                Err(CoordinatorError::RedundantInferredConstruction { .. })
            ),
            "unexpected redundant-bundle result: {result:?}"
        );
        assert_eq!(coordinator.session().design_identity(), expected);
        assert_eq!(
            coordinator
                .session()
                .export_design_json()
                .expect("design JSON"),
            before
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON"),
            accepted_before
        );
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            identity_before
        );
        assert_eq!(coordinator.history_len(), history);
        assert_eq!(coordinator.history_cursor(), cursor);
        assert_eq!(coordinator.transcript().len(), transcript);
        assert_eq!(
            coordinator
                .persistence_checkpoint()
                .expect("persistence checkpoint")
                .computed_evaluation_high_water(),
            evaluation_high_water
        );
    }

    #[test]
    fn partially_redundant_inferred_contact_is_rejected_exactly() {
        let (session, _, reference, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected_input = coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");
        let retained = retained_state_snapshot(&coordinator);
        let cursor = coordinator.history_cursor();
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Point {
                point: ConstructionPoint::New([1.0, 0.0]),
            },
            curve_roles: Vec::new(),
            relations: auto_relations![
                crate::InferredRelation::Midpoint {
                    point: crate::DraftPointSlot::Created { point_index: 0 },
                    line: crate::DraftSpanSlot::Existing(reference),
                },
                crate::InferredRelation::PointOnCurve {
                    point: crate::DraftPointSlot::Created { point_index: 0 },
                    contact: crate::DraftContactDescriptor {
                        span: crate::DraftSpanSlot::Existing(reference),
                        domain: ContactDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        },
                        parameter: 0.5,
                        winding: 0,
                        neighborhood: ContactNeighborhood::Interior,
                    },
                },
            ],
        };

        let result = coordinator.apply_construction_plan(&expected_input, &plan);
        assert!(
            matches!(
                result,
                Err(CoordinatorError::RedundantInferredConstruction { .. })
            ),
            "unexpected partial-redundancy result: {result:?}"
        );
        assert_retained_state_snapshot(&coordinator, &retained);
        assert_eq!(coordinator.history_cursor(), cursor);
    }

    #[test]
    fn oversized_inferred_plan_rejects_before_controlled_trial_state() {
        let (session, _, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");
        let retained = retained_state_snapshot(&coordinator);
        let cursor = coordinator.history_cursor();
        let created = crate::DraftSpanSlot::Created {
            curve_index: 0,
            segment: 0,
        };
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.0, 1.0]),
                end: ConstructionPoint::New([2.0, 1.0]),
            },
            curve_roles: vec![GeometryRole::Profile],
            relations: auto_relations![
                crate::InferredRelation::Horizontal { line: created };
                crate::MAX_CONSTRUCTION_PLAN_RELATIONS + 1
            ],
        };

        assert!(matches!(
            coordinator.apply_construction_plan_controlled(
                &expected,
                &plan,
                OperationControl::unlimited(),
            ),
            Err(CoordinatorError::Document(
                geosolve_sketch::DocumentError::ResourceLimit {
                    resource: "construction plan relations",
                    actual,
                    limit: crate::MAX_CONSTRUCTION_PLAN_RELATIONS,
                }
            )) if actual == crate::MAX_CONSTRUCTION_PLAN_RELATIONS + 1
        ));
        assert_retained_state_snapshot(&coordinator, &retained);
        assert_eq!(coordinator.history_cursor(), cursor);
    }

    #[test]
    fn cancelled_and_exhausted_inferred_construction_are_exact_and_retryable() {
        let (session, _, reference, _) = fixed_line_session();
        let control_session = session.clone();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let mut control =
            RetainedEditorCoordinator::new(control_session).expect("control coordinator");
        let expected = coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");
        let retained = retained_state_snapshot(&coordinator);
        let cursor = coordinator.history_cursor();
        let evaluation_high_water = coordinator
            .persistence_checkpoint()
            .expect("persistence checkpoint")
            .computed_evaluation_high_water();
        let plan = inferred_normal_plan(reference);

        let (cancellation, token) = cancellation_pair();
        cancellation.cancel();
        let mut cancelled_control = OperationControl::unlimited();
        cancelled_control.token = token;
        assert!(matches!(
            coordinator
                .apply_construction_plan_controlled(&expected, &plan, cancelled_control)
                .expect("controlled cancellation"),
            OperationOutcome::Cancelled { .. }
        ));
        assert_retained_state_snapshot(&coordinator, &retained);
        assert_eq!(coordinator.history_cursor(), cursor);

        let mut exhausted_control = OperationControl::unlimited();
        exhausted_control.limits.document_validation_items = 1;
        let OperationOutcome::WorkExhausted { report } = coordinator
            .apply_construction_plan_controlled(&expected, &plan, exhausted_control)
            .expect("controlled exhaustion")
        else {
            panic!("the second inferred relation must exhaust the plan work envelope");
        };
        assert_eq!(report.consumed.document_validation_items, 1);
        assert_eq!(
            report.stopping_reason,
            Some(OperationStopReason::WorkExhausted {
                counter: geosolve_sketch::OperationWorkCounter::DocumentValidationItems,
                checkpoint: OperationCheckpoint::DocumentValidation,
            })
        );
        assert_retained_state_snapshot(&coordinator, &retained);
        assert_eq!(coordinator.history_cursor(), cursor);
        assert_eq!(
            coordinator
                .persistence_checkpoint()
                .expect("persistence checkpoint")
                .computed_evaluation_high_water(),
            evaluation_high_water
        );

        let retry = coordinator
            .apply_construction_plan(&expected, &plan)
            .expect("retry after stopped work");
        let control_expected = control
            .session()
            .accepted_prepared_input()
            .expect("control accepted input");
        let direct = control
            .apply_construction_plan(&control_expected, &plan)
            .expect("direct control publication");
        assert_eq!(retry.value, direct.value);
        assert_eq!(retry.design, direct.design);
        assert_eq!(retry.attempt, direct.attempt);
        assert_eq!(retry.published_accepted, direct.published_accepted);
        assert_eq!(
            retained_document_payloads(&coordinator),
            retained_document_payloads(&control)
        );
    }

    #[test]
    fn cancellation_after_complete_inferred_staging_cannot_publish_live_state() {
        let (session, _, reference, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let expected = coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");
        let plan = inferred_normal_plan(reference);
        let retained = retained_state_snapshot(&coordinator);
        let evaluation_high_water = coordinator
            .persistence_checkpoint()
            .expect("persistence checkpoint")
            .computed_evaluation_high_water();

        let (cancellation, token) = cancellation_pair();
        let mut control = OperationControl::unlimited();
        control.token = token;
        let mut controller = OperationController::new(control);
        let mut trial = coordinator.session().clone();
        let outcome = trial
            .transact_in_controller(
                expected.design_identity(),
                |document| plan.apply(document),
                &mut controller,
            )
            .expect("trial transaction")
            .expect("trial completed");
        RetainedEditorCoordinator::validate_construction_plan_trial(&trial, &outcome)
            .expect("trial validation");
        let staged = coordinator
            .stage_construction_publication_in_controller(trial, &mut controller)
            .expect("staging")
            .expect("staging completed");

        cancellation.cancel();
        assert!(!coordinator.publish_staged_construction_in_controller(
            staged,
            ReplayAction::ConstructionPlan {
                expected: Box::new(expected),
                plan,
            },
            &mut controller,
        ));
        assert!(matches!(
            controller.outcome_unchecked::<()>(),
            OperationOutcome::Cancelled {
                report: OperationReport {
                    stopping_reason: Some(OperationStopReason::Cancelled {
                        checkpoint: OperationCheckpoint::BeforeCommit,
                    }),
                    ..
                }
            }
        ));
        assert_retained_state_snapshot(&coordinator, &retained);
        assert_eq!(
            coordinator
                .persistence_checkpoint()
                .expect("persistence checkpoint")
                .computed_evaluation_high_water(),
            evaluation_high_water
        );
    }

    #[test]
    fn controlled_inferred_report_includes_computed_staging_work() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Point {
                point: ConstructionPoint::New([10.0, 10.0]),
            },
            curve_roles: Vec::new(),
            relations: Vec::new(),
        };
        let expected = fixture
            .coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input");

        let mut trial = fixture.coordinator.session().clone();
        let mut trial_controller = OperationController::new(OperationControl::unlimited());
        trial
            .transact_in_controller(
                expected.design_identity(),
                |document| plan.apply(document),
                &mut trial_controller,
            )
            .expect("trial transaction")
            .expect("trial completion");
        let trial_work = trial_controller.report().consumed.document_validation_items;

        let OperationOutcome::Completed { report, .. } = fixture
            .coordinator
            .apply_construction_plan_controlled(&expected, &plan, OperationControl::unlimited())
            .expect("controlled inferred publication")
        else {
            panic!("controlled inferred publication did not complete");
        };
        assert!(
            report.consumed.document_validation_items > trial_work,
            "computed-feature staging work must be included in the compound report"
        );
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

    fn external_line_entry(
        binding: DocumentExternalBindingId,
        topology_digest: ExternalTopologyDigest,
    ) -> ExternalSnapshotEntry {
        ExternalSnapshotEntry {
            binding,
            source_revision: 1,
            source_digest: ExternalSnapshotDigest::from_bytes([18; 32]),
            feature: ExternalSnapshotFeatureV1::LineSegment {
                start: [0.0, 0.0],
                end: [4.0, 0.0],
                domain: [0.0, 1.0],
                orientation: ExternalLineOrientationV1::StartToEnd,
                scale: 1.0,
                topology_digest,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 2,
                    control_count: 0,
                    span_count: 1,
                },
            },
        }
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
    fn undo_redo_preserve_the_current_nondefault_parameter_batch() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                geosolve_sketch::DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("binding");
        let initial = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(4.0),
            }],
        )
        .expect("initial batch");
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            initial,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        assert!(session.accepted_state().is_some());
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "history point".into(),
                    position: [8.0, 5.0],
                },
            )
            .expect("history edit");
        let current = ParameterBatch::new(
            2,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(6.0),
            }],
        )
        .expect("current batch");
        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                current.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("replace parameter batch");
        let assert_current_width = |coordinator: &RetainedEditorCoordinator| {
            let accepted = coordinator
                .session()
                .accepted_state_for_current_input()
                .expect("accepted current parameter input");
            let left = accepted
                .document()
                .point(rectangle.points[0])
                .expect("bottom-left point")
                .position;
            let right = accepted
                .document()
                .point(rectangle.points[1])
                .expect("bottom-right point")
                .position;
            assert!(((right[0] - left[0]) - 6.0).abs() < 1.0e-9);
        };

        coordinator.undo().expect("undo with current parameters");
        assert_eq!(coordinator.session().parameter_batch(), &current);
        assert_current_width(&coordinator);
        assert_eq!(
            coordinator
                .session()
                .last_attempt()
                .input()
                .parameter_revision(),
            current.revision()
        );
        coordinator.redo().expect("redo with current parameters");
        assert_eq!(coordinator.session().parameter_batch(), &current);
        assert_current_width(&coordinator);
        assert_eq!(
            coordinator
                .session()
                .last_attempt()
                .input()
                .parameter_digest(),
            current.digest()
        );
    }

    #[test]
    fn undo_redo_keep_current_parameter_contract_without_stale_accepted_geometry() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        let target =
            geosolve_sketch::DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]);
        document
            .add_parameter_binding(parameter, target)
            .expect("binding");
        let initial = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(4.0),
            }],
        )
        .expect("initial batch");
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            initial,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::RemoveParameterBinding { parameter, target },
            )
            .expect("remove parameter binding");
        let current = ParameterBatch::new(2, Vec::new()).expect("empty current batch");
        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                current.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("replace parameter batch");
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );

        coordinator
            .undo()
            .expect("undo with incompatible current batch");
        assert_eq!(coordinator.session().parameter_batch(), &current);
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .parameter_bindings()
                .len(),
            1
        );
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        assert!(matches!(
            coordinator
                .session()
                .last_attempt()
                .failure()
                .and_then(SketchAttemptFailure::parameter_input_issue),
            Some(geosolve_sketch::SketchParameterInputIssue::Missing(actual))
                if actual == parameter
        ));

        coordinator
            .redo()
            .expect("redo with compatible current batch");
        assert_eq!(coordinator.session().parameter_batch(), &current);
        assert!(
            coordinator
                .session()
                .design_document()
                .parameter_bindings()
                .is_empty()
        );
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );
    }

    #[test]
    fn undo_redo_preserve_the_current_nondefault_external_snapshot_set() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let point = document.add_point("point", [1.0, 2.0]).expect("point");
        let binding = document
            .add_external_binding("external", ExternalFeatureKindV1::Point, None)
            .expect("binding");
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
            SolverConfig::default(),
        )
        .expect("session");
        assert!(session.accepted_state().is_some());
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "history point".into(),
                    position: [8.0, 5.0],
                },
            )
            .expect("history edit");
        let current = ExternalSnapshotSet::new(2, vec![external_point_entry(binding, [3.0, 4.0])])
            .expect("current snapshots");
        coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                current.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("replace snapshots");
        let assert_current_external_point = |coordinator: &RetainedEditorCoordinator| {
            let accepted = coordinator
                .session()
                .accepted_state_for_current_input()
                .expect("accepted current external input");
            let position = accepted
                .document()
                .point(point)
                .expect("externally constrained point")
                .position;
            assert!((position[0] - 3.0).abs() < 1.0e-9);
            assert!((position[1] - 4.0).abs() < 1.0e-9);
        };

        coordinator.undo().expect("undo with current snapshots");
        assert_eq!(coordinator.session().external_snapshot_set(), &current);
        assert_current_external_point(&coordinator);
        assert_eq!(
            coordinator
                .session()
                .last_attempt()
                .input()
                .external_snapshot_set_revision(),
            current.revision()
        );
        coordinator.redo().expect("redo with current snapshots");
        assert_eq!(coordinator.session().external_snapshot_set(), &current);
        assert_current_external_point(&coordinator);
        assert_eq!(
            coordinator
                .session()
                .last_attempt()
                .input()
                .external_snapshot_set_digest(),
            current.digest()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn undo_redo_keep_current_external_contract_without_stale_accepted_geometry() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("start");
        let end = document.add_point("end", [4.0, 0.0]).expect("end");
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        let first_topology = ExternalTopologyDigest::from_bytes([21; 32]);
        let second_topology = ExternalTopologyDigest::from_bytes([22; 32]);
        let binding = document
            .add_external_binding(
                "external line",
                ExternalFeatureKindV1::LineSegment,
                Some(first_topology),
            )
            .expect("binding");
        document
            .add_constraint(
                "external collinear",
                DocumentConstraintDefinition::ExternalLineCollinear {
                    line: geosolve_sketch::DocumentLineSupportRef {
                        span: CurveSpan::line(line),
                        direction: geosolve_sketch::DocumentDirectionSense::Forward,
                    },
                    external: geosolve_sketch::DocumentExternalLineSupportRef {
                        binding,
                        direction: geosolve_sketch::DocumentDirectionSense::Forward,
                    },
                },
            )
            .expect("constraint");
        let initial =
            ExternalSnapshotSet::new(1, vec![external_line_entry(binding, first_topology)])
                .expect("initial snapshots");
        let session = RetainedSketchDocumentSession::new_with_inputs(
            document,
            ParameterBatch::default(),
            initial,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .rebind_external_binding(
                coordinator.session().design_identity(),
                binding,
                ExternalFeatureKindV1::LineSegment,
                Some(second_topology),
            )
            .expect("rebind topology");
        let current =
            ExternalSnapshotSet::new(2, vec![external_line_entry(binding, second_topology)])
                .expect("current snapshots");
        coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                current.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("replace snapshots");
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );

        coordinator
            .undo()
            .expect("undo with incompatible current snapshots");
        assert_eq!(coordinator.session().external_snapshot_set(), &current);
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .external_binding(binding)
                .expect("historical binding")
                .expected_topology,
            Some(first_topology)
        );
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        assert!(matches!(
            coordinator
                .session()
                .last_attempt()
                .failure()
                .and_then(SketchAttemptFailure::external_snapshot_error),
            Some(ExternalSnapshotInputError::TopologyMismatch { binding: actual })
                if *actual == binding
        ));

        coordinator
            .redo()
            .expect("redo with compatible current snapshots");
        assert_eq!(coordinator.session().external_snapshot_set(), &current);
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .external_binding(binding)
                .expect("current binding")
                .expected_topology,
            Some(second_topology)
        );
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );
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
        assert!(matches!(
            coordinator.computed_feature_problems().as_slice(),
            [ComputedFeatureProblemMetadata {
                scope: EditorProblemScope::Global,
                ..
            }]
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
        assert!(matches!(
            coordinator.computed_feature_problems().as_slice(),
            [ComputedFeatureProblemMetadata {
                scope: EditorProblemScope::Global,
                ..
            }]
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
        assert!(matches!(
            coordinator.computed_feature_problems().as_slice(),
            [ComputedFeatureProblemMetadata {
                scope: EditorProblemScope::Global,
                ..
            }]
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
        assert!(coordinator.computed_feature_problems().is_empty());
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
        assert!(coordinator.current_problem_metadata().is_some());
        assert!(matches!(
            coordinator.computed_feature_problems().as_slice(),
            [ComputedFeatureProblemMetadata {
                scope: EditorProblemScope::Global,
                ..
            }]
        ));

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
        assert_eq!(coordinator.lifecycle().status, LifecycleStatus::Accepted);
        assert!(coordinator.current_problem_metadata().is_none());
        assert!(coordinator.computed_feature_problems().is_empty());
        coordinator.redo().expect("redo");
        assert_eq!(
            coordinator.session().design_document().dimensions().len(),
            1
        );
        assert_eq!(
            coordinator.lifecycle().status,
            LifecycleStatus::RejectedAttempt
        );
        assert!(coordinator.current_problem_metadata().is_some());
        assert!(matches!(
            coordinator.computed_feature_problems().as_slice(),
            [ComputedFeatureProblemMetadata {
                scope: EditorProblemScope::Global,
                ..
            }]
        ));
        coordinator.undo().expect("second undo");
        assert_eq!(coordinator.lifecycle().status, LifecycleStatus::Accepted);
        assert!(coordinator.current_problem_metadata().is_none());
        assert!(coordinator.computed_feature_problems().is_empty());
    }

    #[test]
    fn rejected_dimension_suppression_repairs_and_publishes_a_new_accepted_state() {
        let (session, points, _, target) = fixed_line_session();
        let retained = session.accepted_state().expect("accepted").identity();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let outcome = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
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
            .expect("retained conflicting dimension");
        let DocumentCommandEffect::CreatedDimension(dimension) = outcome.value else {
            panic!("conflicting dimension returned the wrong effect");
        };
        assert!(outcome.published_accepted.is_none());
        assert_eq!(
            coordinator.lifecycle().status,
            LifecycleStatus::RejectedAttempt
        );
        assert!(coordinator.current_problem_metadata().is_some());
        assert!(!coordinator.computed_feature_problems().is_empty());
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("retained accepted state")
                .identity(),
            retained
        );

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Dimension(dimension)]);
        let repair = coordinator
            .set_selected_suppressed(coordinator.session().design_identity(), true)
            .expect("suppression repair");
        assert!(repair.published_accepted.is_some());
        assert_eq!(coordinator.lifecycle().status, LifecycleStatus::Accepted);
        assert!(coordinator.current_problem_metadata().is_none());
        assert!(coordinator.computed_feature_problems().is_empty());
        assert_ne!(
            coordinator
                .session()
                .accepted_state()
                .expect("repaired accepted state")
                .identity(),
            retained
        );
        assert_eq!(repair.value.len(), 1);
        assert!(
            coordinator
                .session()
                .design_document()
                .source(repair.value[0])
                .expect("repaired source")
                .suppressed
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
                    point: ConstructionPoint::New([7.0, 2.0]),
                },
                role: GeometryRole::Profile,
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
    fn ordinary_construction_proposals_publish_and_reload_through_the_coordinator() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(4.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("empty accepted session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let proposals = [
            ConstructionProposal::Point {
                point: ConstructionPoint::New([-8.0, -8.0]),
            },
            ConstructionProposal::Line {
                start: ConstructionPoint::New([-6.0, -6.0]),
                end: ConstructionPoint::New([-4.0, -6.0]),
            },
            ConstructionProposal::Polyline {
                points: vec![
                    ConstructionPoint::New([-2.0, -6.0]),
                    ConstructionPoint::New([0.0, -6.0]),
                    ConstructionPoint::New([0.0, -4.0]),
                ],
            },
            ConstructionProposal::Rectangle {
                first: [2.0, -6.0],
                second: [4.0, -4.0],
            },
            ConstructionProposal::Circle {
                center: ConstructionPoint::New([6.0, -5.0]),
                radius: 1.0,
            },
            ConstructionProposal::CounterClockwiseArc {
                center: ConstructionPoint::New([8.0, -5.0]),
                start: [9.0, -5.0],
                end: [8.0, -4.0],
            },
        ];

        for proposal in proposals {
            let outcome = coordinator
                .apply_construction(coordinator.session().design_identity(), &proposal)
                .expect("ordinary construction");
            assert!(outcome.published_accepted.is_some());
            assert_eq!(coordinator.lifecycle().status, LifecycleStatus::Accepted);
        }

        let saved = coordinator.checkpoint().clone();
        let canonical_design = saved.design_json().to_owned();
        coordinator.reload(&saved).expect("checkpoint reload");
        assert_eq!(coordinator.checkpoint().design_json(), canonical_design);
        assert_eq!(coordinator.lifecycle().status, LifecycleStatus::Accepted);
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
        let other_radius = document
            .add_scalar(
                "other radius",
                1.5,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let other_circle = CurveSpan::line(
            document
                .add_curve(
                    "other circle",
                    CurveDefinition::Circle {
                        center: points[0],
                        radius: other_radius,
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
                ConstraintIntent::Horizontal,
                ResolvedConstraintKind::HorizontalPoints,
                vec![point(0), point(1)],
                0,
            ),
            (
                ConstraintIntent::Vertical,
                ResolvedConstraintKind::VerticalPoints,
                vec![point(0), point(2)],
                0,
            ),
            (
                ConstraintIntent::Concentric,
                ResolvedConstraintKind::ConcentricCurves,
                vec![curve(circle), curve(other_circle)],
                0,
            ),
            (
                ConstraintIntent::Collinear,
                ResolvedConstraintKind::CollinearSupports,
                vec![curve(first_line), curve(second_line)],
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
        assert_eq!(cases.len(), 20);
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
                assert_eq!(request.contacts[0].domain, ContactDomain::SupportingLine);
                assert_eq!(
                    request.contacts[0].neighborhood,
                    ContactNeighborhood::Interior
                );
                assert_eq!(request.contacts[0].parameter.to_bits(), 0.0_f64.to_bits());
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
    fn intrinsic_datums_are_selectable_but_protected_from_every_object_mutation() {
        let (session, points, span, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let original_design = coordinator.session().design_identity();
        let original_history = coordinator.history_len();

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Datum(SketchDatum::Origin)]);
        for action in [
            CoordinatorActionKind::Delete,
            CoordinatorActionKind::Suppress,
            CoordinatorActionKind::Unsuppress,
            CoordinatorActionKind::Constraint(ConstraintIntent::Lock),
        ] {
            assert!(coordinator.actions().contains(&ActionAvailability {
                action,
                state: ActionState::Disabled(DisabledReason::ProtectedDatum),
            }));
        }
        assert!(matches!(
            coordinator.delete_selected(original_design),
            Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum
            ))
        ));
        assert!(matches!(
            coordinator.set_selected_suppressed(original_design, true),
            Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum
            ))
        ));
        assert!(matches!(
            coordinator.set_selected_suppressed(original_design, false),
            Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum
            ))
        ));
        assert!(matches!(
            coordinator.toggle_selected_geometry_role(original_design),
            Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum
            ))
        ));

        coordinator.editor_mut().set_selection([
            SelectionItem::Curve(span),
            SelectionItem::Datum(SketchDatum::XAxis),
        ]);
        assert!(matches!(
            coordinator.toggle_selected_geometry_role(original_design),
            Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum
            ))
        ));
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .geometry_role(span.curve),
            Some(GeometryRole::Profile)
        );

        coordinator.editor_mut().set_selection([
            SelectionItem::Point(points[0]),
            SelectionItem::Datum(SketchDatum::YAxis),
        ]);
        assert!(matches!(
            coordinator.delete_selected(original_design),
            Err(CoordinatorError::ActionUnavailable(
                DisabledReason::ProtectedDatum
            ))
        ));
        assert_eq!(coordinator.session().design_identity(), original_design);
        assert_eq!(coordinator.history_len(), original_history);
        assert!(
            coordinator
                .session()
                .design_document()
                .point(points[0])
                .is_some()
        );
    }

    #[test]
    fn datum_pointer_drag_selects_without_starting_a_gesture_or_history() {
        let (session, points, _, _) = fixed_line_session();
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let accepted = session.accepted_state().expect("accepted line");
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
        let design = coordinator.session().design_identity();
        let history = coordinator.history_len();
        let press = viewport.model_to_screen([8.0, 0.0]);

        assert!(matches!(
            coordinator.pointer_down(
                &scene,
                PointerInput {
                    pointer_id: 740,
                    position: press,
                    modifiers: Modifiers::default(),
                },
            )
            .as_slice(),
            [EditorEffect::SelectionChanged(selection)]
                if selection == &[SelectionItem::Datum(SketchDatum::XAxis)]
        ));
        assert!(coordinator.editor().point_gesture_snapshot().is_none());
        assert!(coordinator.drag_continuation.is_none());
        assert!(
            coordinator
                .editor_mut()
                .pointer_move(
                    &scene,
                    PointerInput {
                        pointer_id: 740,
                        position: ScreenPoint {
                            x: press.x + 40.0,
                            y: press.y + 40.0,
                        },
                        modifiers: Modifiers::default(),
                    },
                )
                .iter()
                .all(|effect| !matches!(
                    effect,
                    EditorEffect::PreviewPointMove { .. }
                        | EditorEffect::RequestProjectedPointMove { .. }
                ))
        );
        assert_eq!(coordinator.session().design_identity(), design);
        assert_eq!(coordinator.history_len(), history);

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Datum(SketchDatum::YAxis)]);
        let point_press = viewport.model_to_screen([2.0, 0.0]);
        assert!(matches!(
            coordinator.pointer_down(
                &scene,
                PointerInput {
                    pointer_id: 741,
                    position: point_press,
                    modifiers: Modifiers {
                        shift: true,
                        ..Modifiers::default()
                    },
                },
            )
            .as_slice(),
            [EditorEffect::SelectionChanged(selection)]
                if selection == &[
                    SelectionItem::Datum(SketchDatum::YAxis),
                    SelectionItem::Point(points[1]),
                ]
        ));
        assert!(coordinator.editor().point_gesture_snapshot().is_none());
        assert!(coordinator.drag_continuation.is_none());
        assert_eq!(coordinator.session().design_identity(), design);
        assert_eq!(coordinator.history_len(), history);
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
    fn initial_conflict_projects_design_unsolved_without_accepted_provenance() {
        let mut document = SketchDocument::new(4.0).expect("document");
        let point = document.add_point("conflicted", [0.0, 0.0]).expect("point");
        for target in [[0.0, 0.0], [1.0, 0.0]] {
            document
                .add_constraint(
                    "conflicting fixed point",
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("constraint");
        }
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained unsolved design");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let lifecycle = coordinator.lifecycle();
        assert_eq!(lifecycle.status, LifecycleStatus::DesignUnsolved);
        assert!(lifecycle.accepted.is_none());
        assert!(lifecycle.parent_accepted.is_none());
        assert!(lifecycle.preview_attempt.is_none());
        assert!(lifecycle.preview_accepted.is_none());
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
            [EditorEffect::CommitPointMove {
                expected,
                point,
                model_position,
            }] if *expected == expected_design
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
            [EditorEffect::CommitPointMove {
                expected: effect_expected,
                point: effect_point,
                ..
            }] if *effect_expected == expected && *effect_point == center
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
            [EditorEffect::CommitPointMove { point, .. }] if *point == center
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
    #[allow(
        clippy::too_many_lines,
        reason = "the prospective, stale and accepted-state assertions form one transaction scenario"
    )]
    fn contextual_authoring_resolution_is_prospective_until_one_coordinator_apply() {
        let (session, points, _, _) = fixed_line_session();
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        assert!(coordinator.actions().contains(&ActionAvailability {
            action: CoordinatorActionKind::Constraint(ConstraintIntent::Lock),
            state: ActionState::Disabled(DisabledReason::EmptySelection),
        }));
        assert_eq!(
            coordinator.resolved_constraint(ConstraintIntent::Lock),
            None
        );
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
        assert_eq!(
            coordinator.resolved_constraint(ConstraintIntent::Lock),
            Some(ResolvedConstraintKind::FixedPoint)
        );
        let application = AuthoringState::default().activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Lock),
            &[AuthoringOperand::selected(SelectionItem::Point(points[0]))],
        );
        let AuthoringOutcome::Apply(application) = application else {
            panic!("prospective contextual application");
        };
        assert_eq!(
            application.resolved_constraint,
            Some(ResolvedConstraintKind::FixedPoint)
        );
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

        let prospective = retained_state_snapshot(&coordinator);
        let mut stale_application = application.clone();
        stale_application.resolved_constraint = Some(ResolvedConstraintKind::CoincidentPoints);
        assert!(matches!(
            coordinator.apply_authoring(design, &stale_application),
            Err(CoordinatorError::InvalidActionInput(
                "authoring resolution is stale"
            ))
        ));
        assert_retained_state_snapshot(&coordinator, &prospective);

        let AuthoringMutation::Constraint(outcome) = coordinator
            .apply_authoring(design, &application)
            .expect("contextual apply")
        else {
            panic!("constraint mutation");
        };
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

        let applied = retained_state_snapshot(&coordinator);
        assert!(matches!(
            coordinator.apply_authoring(design, &application),
            Err(CoordinatorError::Session(
                DocumentSessionError::StaleDesign { .. }
            ))
        ));
        assert_retained_state_snapshot(&coordinator, &applied);
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

    fn seed_failed_no_history_temporary_attempt(coordinator: &mut RetainedEditorCoordinator) {
        let expected = coordinator.session().design_identity();
        let invalid_point = DesignPointId(PersistentId::from_u128(0xffff_ffff_ffff));
        let invalid_request = coordinator
            .session()
            .last_attempt()
            .input()
            .candidate_request()
            .with_drag(invalid_point, [1.0, 1.0]);
        coordinator
            .session
            .reattempt(expected, invalid_request)
            .expect("seed failed temporary request");
        coordinator
            .reattempt(expected)
            .expect("publish failed no-history attempt");
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        assert!(coordinator.current_problem_metadata().is_some());
        assert!(matches!(
            coordinator.computed_feature_problems().as_slice(),
            [ComputedFeatureProblemMetadata {
                scope: EditorProblemScope::Global,
                ..
            }]
        ));
    }

    #[test]
    fn feature_only_undo_restores_a_fresh_session_after_a_failed_no_history_attempt() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        assert_eq!(fixture.coordinator.history_len(), 2);
        assert!(
            fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .is_some()
        );

        seed_failed_no_history_temporary_attempt(&mut fixture.coordinator);
        fixture.coordinator.undo().expect("feature-only undo");

        assert!(
            fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .is_none()
        );
        assert_eq!(
            fixture.coordinator.lifecycle().status,
            LifecycleStatus::Accepted
        );
        assert!(
            fixture
                .coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );
        assert!(fixture.coordinator.current_problem_metadata().is_none());
        assert!(fixture.coordinator.computed_feature_problems().is_empty());
    }

    #[test]
    fn same_sketch_reload_restores_a_fresh_session_after_a_failed_no_history_attempt() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let saved = fixture
            .coordinator
            .persistence_checkpoint()
            .expect("healthy feature checkpoint");

        seed_failed_no_history_temporary_attempt(&mut fixture.coordinator);
        fixture
            .coordinator
            .reload(&saved)
            .expect("same-sketch reload");

        assert!(
            fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .is_some()
        );
        assert_eq!(
            fixture.coordinator.lifecycle().status,
            LifecycleStatus::Accepted
        );
        assert!(
            fixture
                .coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );
        assert!(fixture.coordinator.current_problem_metadata().is_none());
        assert!(fixture.coordinator.computed_feature_problems().is_empty());
    }

    #[test]
    fn recorded_native_edit_transition_accepts_contact_refresh_but_rejects_branch_or_radius_edits()
    {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let before = fixture.coordinator.feature_document().clone();
        let ComputedFeatureDefinition::FilletSet(fillet) =
            &before.feature(feature).expect("Fillet feature").definition;
        let corner = fillet.corners[0];

        let mut refreshed_corner = corner.without_id();
        refreshed_corner.first.picked_parameter += 1.0e-4;
        let mut contact_refresh = before.clone();
        contact_refresh
            .set_fillet_corner(feature, corner.id, refreshed_corner)
            .expect("valid contact refresh");
        assert!(recorded_transition_is_reanchor_only(
            &before,
            &contact_refresh
        ));

        let mut changed_branch = corner.without_id();
        changed_branch.first.normal_side = match changed_branch.first.normal_side {
            DocumentCurveNormalSide::Left => DocumentCurveNormalSide::Right,
            DocumentCurveNormalSide::Right => DocumentCurveNormalSide::Left,
        };
        let mut branch_transition = before.clone();
        branch_transition
            .set_fillet_corner(feature, corner.id, changed_branch)
            .expect("structurally valid branch mutation");
        assert!(!recorded_transition_is_reanchor_only(
            &before,
            &branch_transition
        ));

        let mut radius_transition = before.clone();
        radius_transition
            .set_fillet_radius(feature, fillet.radius * 1.1)
            .expect("structurally valid radius mutation");
        assert!(!recorded_transition_is_reanchor_only(
            &before,
            &radius_transition
        ));
    }

    #[test]
    fn non_edit_replay_actions_never_persist_an_unrecorded_feature_reanchor() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let before_identity = fixture.coordinator.feature_document().identity();
        let before_json = fixture.coordinator.feature_document().to_json().unwrap();

        fixture
            .coordinator
            .apply_constraint_action_for(
                fixture.coordinator.session().design_identity(),
                &[SelectionItem::Curve(fixture.spans[0])],
                ConstraintActionRequest {
                    intent: ConstraintIntent::Horizontal,
                    label: "horizontal source".into(),
                    contacts: Vec::new(),
                    relation: None,
                },
            )
            .expect("ordinary non-Edit constraint action");

        assert_eq!(
            fixture.coordinator.feature_document().identity(),
            before_identity,
            "a non-Edit replay action cannot silently advance persistent feature intent"
        );
        assert_eq!(
            fixture.coordinator.feature_document().to_json().unwrap(),
            before_json
        );
        assert!(matches!(
            fixture.coordinator.transcript().last(),
            Some(ReplayAction::ConstraintAction { .. })
        ));
        assert!(matches!(
            computed_feature_state(
                fixture
                    .coordinator
                    .computed_snapshot()
                    .expect("cold-evaluated feature output"),
                feature,
            ),
            ComputedFeatureEvaluationState::Current { .. }
        ));
    }

    #[test]
    fn direct_edit_replay_preserves_new_failure_beside_reanchored_current_feature() {
        let mut fixture = computed_fillet_editor_fixture();
        let first_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let first = apply_grouped_fillet(&mut fixture.coordinator, &first_candidate);
        let second_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[2]]);
        let second = apply_grouped_fillet(&mut fixture.coordinator, &second_candidate);
        let mut replayed = RetainedEditorCoordinator::with_features(
            fixture.coordinator.session().clone(),
            fixture.coordinator.feature_document().clone(),
        )
        .expect("exact pre-edit replay coordinator");
        let before_features = fixture.coordinator.feature_document().identity();

        fixture
            .coordinator
            .apply_edit(
                fixture.coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: fixture.points[1],
                    position: [0.1, 0.0],
                },
            )
            .expect("direct edit may retain one failed Fillet beside one current Fillet");
        let original_snapshot = fixture
            .coordinator
            .computed_snapshot()
            .expect("mixed direct-edit computed state");
        assert!(matches!(
            computed_feature_state(original_snapshot, first),
            ComputedFeatureEvaluationState::Failed { .. }
        ));
        assert!(matches!(
            computed_feature_state(original_snapshot, second),
            ComputedFeatureEvaluationState::Current { .. }
        ));
        assert_ne!(
            fixture.coordinator.feature_document().identity(),
            before_features,
            "the surviving Current feature must durably refresh its contact frame"
        );
        let action = fixture
            .coordinator
            .transcript()
            .last()
            .expect("direct edit replay action")
            .clone();
        assert!(matches!(
            action,
            ReplayAction::Edit {
                computed_features: Some(_),
                ..
            }
        ));
        let expected_dispositions = recorded_computed_feature_dispositions(original_snapshot);
        let expected_features = fixture.coordinator.feature_document().to_json().unwrap();
        let expected_design = fixture.coordinator.session().export_design_json().unwrap();
        let expected_accepted = fixture
            .coordinator
            .session()
            .export_accepted_json()
            .unwrap();

        replayed
            .replay(&action)
            .expect("mixed direct edit must replay its recorded cold state");
        let replayed_snapshot = replayed
            .computed_snapshot()
            .expect("replayed mixed computed state");
        assert_eq!(
            recorded_computed_feature_dispositions(replayed_snapshot),
            expected_dispositions
        );
        assert!(computed_feature_states_match_for_durable_reanchor(
            original_snapshot,
            replayed_snapshot
        ));
        assert_eq!(
            replayed.feature_document().to_json().unwrap(),
            expected_features
        );
        assert_eq!(
            replayed.session().export_design_json().unwrap(),
            expected_design
        );
        assert_eq!(
            replayed.session().export_accepted_json().unwrap(),
            expected_accepted
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the complete-scene regression keeps two feature dispositions, preview retention, attribution, recovery and release in one real pointer transaction"
    )]
    fn projected_drag_rejects_one_new_failure_beside_another_current_fillet() {
        let mut fixture = computed_fillet_editor_fixture();
        let first_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let first = apply_grouped_fillet(&mut fixture.coordinator, &first_candidate);
        let second_candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[2]]);
        let second = apply_grouped_fillet(&mut fixture.coordinator, &second_candidate);
        let initial_snapshot = fixture
            .coordinator
            .computed_snapshot()
            .expect("two initially Current Fillets");
        assert!(matches!(
            computed_feature_state(initial_snapshot, first),
            ComputedFeatureEvaluationState::Current { .. }
        ));
        assert!(matches!(
            computed_feature_state(initial_snapshot, second),
            ComputedFeatureEvaluationState::Current { .. }
        ));

        let (first_corner, mut first_sources) = {
            let ComputedFeatureDefinition::FilletSet(set) = &fixture
                .coordinator
                .feature_document()
                .feature(first)
                .expect("first Fillet")
                .definition;
            let corner = set.corners[0];
            (corner.id, vec![corner.first.source, corner.second.source])
        };
        first_sources.sort_unstable();

        // Establish independently that this exact native candidate fails only
        // the first set. The projected transaction below must reject that
        // partial candidate instead of publishing the surviving second set.
        let mut failure_probe = RetainedEditorCoordinator::with_features(
            fixture.coordinator.session().clone(),
            fixture.coordinator.feature_document().clone(),
        )
        .expect("failure probe");
        failure_probe
            .apply_edit(
                failure_probe.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: fixture.points[1],
                    position: [0.1, 0.0],
                },
            )
            .expect("mixed candidate is an accepted native edit");
        let candidate_snapshot = failure_probe
            .computed_snapshot()
            .expect("mixed candidate computed state");
        assert!(matches!(
            computed_feature_state(candidate_snapshot, first),
            ComputedFeatureEvaluationState::Failed { .. }
        ));
        assert!(matches!(
            computed_feature_state(candidate_snapshot, second),
            ComputedFeatureEvaluationState::Current { .. }
        ));

        let point = fixture.points[1];
        let scene = current_computed_scene(&fixture.coordinator);
        let press = scene
            .points
            .iter()
            .find(|candidate| candidate.id == point)
            .expect("shared source point")
            .screen_position;
        let pointer_id = 9_070;
        let pointer = |position| PointerInput {
            pointer_id,
            position,
            modifiers: Modifiers::default(),
        };
        let expected_design = fixture.coordinator.session().design_identity();
        let history_before = fixture.coordinator.history_len();
        fixture.coordinator.pointer_down(&scene, pointer(press));

        let computed_scene_fingerprint = |coordinator: &RetainedEditorCoordinator| {
            let ComputedSceneState::Current { expected, snapshot } =
                coordinator.computed_scene_state()
            else {
                panic!(
                    "complete computed-scene fingerprint requested from {:?}",
                    coordinator.computed_scene_state()
                )
            };
            assert_eq!(*expected, snapshot.input());
            (
                *expected,
                snapshot.evaluation_revision(),
                snapshot.edges().to_vec(),
                snapshot.construction_fragments().to_vec(),
                snapshot.feature_evaluations().to_vec(),
                snapshot.replaced_sources().to_vec(),
            )
        };

        let resolve_sample = |coordinator: &mut RetainedEditorCoordinator,
                              model_position: [f64; 2]| {
            let scene = visible_computed_scene(coordinator);
            let screen = scene.viewport.model_to_screen(model_position);
            let request = coordinator
                .editor_mut()
                .pointer_move(&scene, pointer(screen));
            let [
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point: requested,
                    model_position,
                },
            ] = request.as_slice()
            else {
                panic!("projected sample did not emit one request: {request:?}")
            };
            assert_eq!(*requested, point);
            let effects = coordinator.resolve_projected_point_move(
                *pointer_id,
                *request_id,
                *requested,
                *model_position,
            );
            (screen, effects)
        };

        let (_, valid_effects) = resolve_sample(&mut fixture.coordinator, [3.5, 0.0]);
        let [
            EditorEffect::PreviewPointMove {
                point: previewed,
                model_position: first_valid_position,
            },
        ] = valid_effects.as_slice()
        else {
            panic!("first regular sample did not publish one preview: {valid_effects:?}")
        };
        assert_eq!(*previewed, point);
        let first_valid_position = *first_valid_position;
        let first_valid_session = fixture
            .coordinator
            .solved_preview_session()
            .expect("first valid native preview");
        let first_valid_design_json = first_valid_session.export_design_json().unwrap();
        let first_valid_accepted_json = first_valid_session.export_accepted_json().unwrap();
        let first_valid_computed = computed_scene_fingerprint(&fixture.coordinator);
        let first_valid_snapshot = match fixture.coordinator.computed_scene_state() {
            ComputedSceneState::Current { expected, snapshot } => {
                assert_eq!(*expected, snapshot.input());
                snapshot
            }
            state => panic!("first regular sample withheld computed output: {state:?}"),
        };
        for feature in [first, second] {
            assert!(matches!(
                computed_feature_state(first_valid_snapshot, feature),
                ComputedFeatureEvaluationState::Current { .. }
            ));
        }

        let (_, blocked_effects) = resolve_sample(&mut fixture.coordinator, [0.1, 0.0]);
        assert!(
            blocked_effects.is_empty(),
            "a partial computed candidate must not publish a native preview"
        );
        let blocked_work = fixture
            .coordinator
            .projected_drag_work_evidence()
            .expect("blocked mixed-candidate evidence");
        assert!(!blocked_work.accepted);
        assert_eq!(
            blocked_work.rejection_stage,
            Some(ProjectedDragRejectionStage::PreviewPublication)
        );
        let retained_session = fixture
            .coordinator
            .solved_preview_session()
            .expect("blocked sample retains the paired native preview");
        assert_eq!(
            retained_session.export_design_json().unwrap(),
            first_valid_design_json
        );
        assert_eq!(
            retained_session.export_accepted_json().unwrap(),
            first_valid_accepted_json
        );
        assert_eq!(
            computed_scene_fingerprint(&fixture.coordinator),
            first_valid_computed,
            "the blocked candidate must retain the exact computed snapshot"
        );
        let problems = fixture.coordinator.computed_feature_problems();
        let [problem] = problems.as_slice() else {
            panic!("one newly failed Fillet must publish one targeted problem: {problems:?}")
        };
        assert_eq!(problem.feature, Some(first));
        assert_ne!(problem.feature, Some(second));
        assert_eq!(problem.corners, vec![first_corner]);
        assert_eq!(problem.sources, first_sources);
        assert_eq!(problem.scope, EditorProblemScope::Targeted);

        let (_, recovered_effects) = resolve_sample(&mut fixture.coordinator, [3.0, 0.0]);
        let [
            EditorEffect::PreviewPointMove {
                point: recovered,
                model_position: recovered_position,
            },
        ] = recovered_effects.as_slice()
        else {
            panic!("reverse sample did not recover one complete preview: {recovered_effects:?}")
        };
        assert_eq!(*recovered, point);
        assert_ne!(
            recovered_position.map(f64::to_bits),
            first_valid_position.map(f64::to_bits),
            "reverse recovery must publish a fresh valid sample"
        );
        assert!(fixture.coordinator.computed_feature_problems().is_empty());
        let recovered_position = *recovered_position;
        let recovered_snapshot = match fixture.coordinator.computed_scene_state() {
            ComputedSceneState::Current { expected, snapshot } => {
                assert_eq!(*expected, snapshot.input());
                snapshot
            }
            state => panic!("reverse sample withheld computed output: {state:?}"),
        };
        for feature in [first, second] {
            assert!(matches!(
                computed_feature_state(recovered_snapshot, feature),
                ComputedFeatureEvaluationState::Current { .. }
            ));
        }
        let recovered_computed = computed_scene_fingerprint(&fixture.coordinator);

        let (blocked_screen, terminal_effects) =
            resolve_sample(&mut fixture.coordinator, [0.1, 0.0]);
        assert!(terminal_effects.is_empty());
        assert_eq!(
            computed_scene_fingerprint(&fixture.coordinator),
            recovered_computed
        );
        let release_scene = visible_computed_scene(&fixture.coordinator);
        let release = fixture.coordinator.editor_mut().pointer_up(
            &release_scene,
            expected_design,
            pointer(blocked_screen),
        );
        let [
            commit @ EditorEffect::CommitPointMove {
                point: committed,
                model_position,
                ..
            },
        ] = release.as_slice()
        else {
            panic!("terminal blocked sample did not release the last valid preview: {release:?}")
        };
        assert_eq!(*committed, point);
        assert_eq!(
            model_position.map(f64::to_bits),
            recovered_position.map(f64::to_bits)
        );
        fixture
            .coordinator
            .apply_editor_effect(commit)
            .expect("commit retained complete preview")
            .expect("one native point mutation");
        assert_eq!(fixture.coordinator.history_len(), history_before + 1);
        assert_eq!(
            fixture
                .coordinator
                .session()
                .accepted_state_for_current_input()
                .expect("committed accepted state")
                .document()
                .point(point)
                .expect("committed point")
                .position
                .map(f64::to_bits),
            recovered_position.map(f64::to_bits)
        );
        assert!(fixture.coordinator.computed_feature_problems().is_empty());
        let committed_snapshot = fixture
            .coordinator
            .computed_snapshot()
            .expect("committed complete computed output");
        for feature in [first, second] {
            assert!(matches!(
                computed_feature_state(committed_snapshot, feature),
                ComputedFeatureEvaluationState::Current { .. }
            ));
        }
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
        let mut scene = EditorScene::from_accepted_with_computed(
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
        .expect("current composite scene");
        coordinator
            .populate_computed_fillet_affordances(&mut scene, coordinator.editor().selection(), 0.5)
            .expect("current Fillet affordances");
        scene
    }

    fn visible_computed_scene(coordinator: &RetainedEditorCoordinator) -> EditorScene {
        let source = coordinator
            .solved_preview_session()
            .unwrap_or_else(|| coordinator.session());
        let accepted = source
            .accepted_state()
            .expect("accepted visible computed scene");
        let (expected, snapshot) = match coordinator.computed_scene_state() {
            ComputedSceneState::Current { expected, snapshot } => (*expected, snapshot),
            state => panic!("visible computed scene is not Current: {state:?}"),
        };
        let mut scene = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            source.design_document(),
            &source
                .accepted_prepared_input()
                .expect("current accepted visible input"),
            &expected,
            snapshot,
            Viewport::new([900.0, 700.0], [1.0, 1.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("visible composite scene");
        coordinator
            .populate_computed_fillet_affordances(&mut scene, coordinator.editor().selection(), 0.5)
            .expect("visible Fillet affordances");
        scene
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
                .filter(|curve| {
                    curve.span == fixture.spans[1] && !curve.origin.is_implicit_construction()
                })
                .count(),
            1,
            "the shared middle span must be replaced once and trimmed at both ends"
        );
        assert_eq!(
            scene
                .curves
                .iter()
                .filter(|curve| {
                    curve.span == fixture.spans[1] && curve.origin.is_implicit_construction()
                })
                .count(),
            2,
            "both discarded middle-span complements remain implicit construction"
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
    #[allow(
        clippy::too_many_lines,
        reason = "the exact-fold regression keeps branch, scene, history and native-sketch invariants in one transaction fixture"
    )]
    fn explicit_numeric_radius_can_depart_a_rail_less_fold_on_its_persisted_branch() {
        let mut document = SketchDocument::new(10.0).expect("fold document");
        let line_start = document.add_point("line start", [-5.0, 0.0]).unwrap();
        let line_end = document.add_point("line end", [5.0, 0.0]).unwrap();
        let line = CurveSpan::line(
            document
                .add_curve(
                    "line",
                    CurveDefinition::Line {
                        start: line_start,
                        end: line_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let circle_center = document.add_point("circle center", [0.0, 2.0]).unwrap();
        let circle_radius = document
            .add_scalar(
                "circle radius",
                1.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let circle = CurveSpan::line(
            document
                .add_curve(
                    "circle",
                    CurveDefinition::Circle {
                        center: circle_center,
                        radius: circle_radius,
                    },
                )
                .unwrap(),
        );
        let document_id = document.id();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted fold session");
        let authoring =
            ComputedFeatureAuthoringSnapshot::capture(&session).expect("fold authoring snapshot");
        let branch_center_x = -1.5_f64.sqrt();
        let line_parameter = (branch_center_x + 5.0) / 10.0;
        let circle_parameter = (-1.25_f64)
            .atan2(branch_center_x)
            .rem_euclid(std::f64::consts::TAU);
        let line_jet = authoring
            .sketch_document()
            .evaluate_curve_jet(line, line_parameter)
            .unwrap();
        let circle_jet = authoring
            .sketch_document()
            .evaluate_curve_jet(circle, circle_parameter)
            .unwrap();
        let fold_outcome = authoring
            .resolve_fillet_corner(
                ComputedFilletCornerAuthoringRequest {
                    first: ComputedFilletCurvePick {
                        source: NativeCurveSpanSource { span: line },
                        parameter: line_parameter,
                        model_position: [line_jet.position.x, line_jet.position.y],
                        retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
                    },
                    second: ComputedFilletCurvePick {
                        source: NativeCurveSpanSource { span: circle },
                        parameter: circle_parameter,
                        model_position: [circle_jet.position.x, circle_jet.position.y],
                        retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
                    },
                    options: ComputedFilletAuthoringOptions::default(),
                },
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .expect("fold authoring");
        let OperationOutcome::Completed { value: fold, .. } = fold_outcome else {
            panic!("bounded fold authoring did not complete: {fold_outcome:?}");
        };
        assert!(fold.arc.center[0].abs() <= 1.0e-6);

        let mut features = ComputedFeatureDocument::new(document_id);
        let feature = features
            .create_fillet_set("persisted fold branch", 0.5, vec![fold.corner])
            .expect("fold feature");
        let corner = match &features.feature(feature).unwrap().definition {
            ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
        };
        let owner = ComputedCornerRef { feature, corner };
        let mut coordinator =
            RetainedEditorCoordinator::with_features(session, features).expect("fold coordinator");
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
        let design_identity = coordinator.session().design_identity();
        let accepted_identity = coordinator
            .session()
            .accepted_state()
            .expect("accepted fold sketch")
            .identity();
        let sketch_equations = accepted_sketch_equation_invariants(&coordinator);
        let history_before = coordinator.history_len();
        let fold_feature_identity = coordinator.feature_document().identity();

        let fold_scene = current_computed_scene(&coordinator);
        assert!(
            fold_scene
                .fillet_affordances
                .iter()
                .all(|affordance| affordance.owner != owner),
            "an exact fold must not invent a finite radius rail"
        );
        assert!(matches!(
            fold_scene.computed_fillet_continuation_statuses.as_slice(),
            [ComputedFilletContinuationStatus {
                owner: current,
                limit: ComputedFilletContinuationLimit {
                    kind: ComputedFilletContinuationLimitKind::BranchFold,
                    ..
                },
                ..
            }] if *current == owner
        ));

        coordinator
            .set_computed_fillet_radius(fold_feature_identity, feature, 0.55)
            .expect("explicit numeric edit departs the persisted fold branch");
        assert_eq!(coordinator.history_len(), history_before + 1);
        assert_eq!(
            fillet_radius(&coordinator, feature).to_bits(),
            0.55_f64.to_bits()
        );
        let regular_corner = match &coordinator
            .feature_document()
            .feature(feature)
            .expect("regular feature")
            .definition
        {
            ComputedFeatureDefinition::FilletSet(fillet) => {
                assert_eq!(fillet.corners[0].id, corner);
                fillet.corners[0].without_id()
            }
        };
        assert_eq!(regular_corner.first.source, fold.corner.first.source);
        assert_eq!(regular_corner.second.source, fold.corner.second.source);
        assert_eq!(
            regular_corner.first.normal_side,
            fold.corner.first.normal_side
        );
        assert_eq!(
            regular_corner.second.normal_side,
            fold.corner.second.normal_side
        );
        let regular_center_x = coordinator
            .computed_snapshot()
            .expect("regular computed snapshot")
            .edges()
            .iter()
            .find_map(|edge| match (&edge.provenance, &edge.geometry) {
                (
                    ComputedEdgeProvenance::FilletArc { owner: current, .. },
                    geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc),
                ) if *current == owner => Some(arc.center[0]),
                _ => None,
            })
            .expect("regular persisted-root arc");
        assert!(
            regular_center_x < 0.0,
            "numeric edit hopped to the other root"
        );
        let regular_scene = current_computed_scene(&coordinator);
        assert!(
            regular_scene
                .fillet_affordances
                .iter()
                .any(|affordance| affordance.owner == owner),
            "the regular branch must expose a finite rail"
        );
        assert!(
            regular_scene
                .computed_fillet_continuation_statuses
                .is_empty()
        );
        assert_eq!(coordinator.session().design_identity(), design_identity);
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            accepted_identity
        );
        assert_eq!(
            accepted_sketch_equation_invariants(&coordinator),
            sketch_equations
        );

        coordinator.undo().expect("undo numeric fold departure");
        assert_eq!(
            fillet_radius(&coordinator, feature).to_bits(),
            0.5_f64.to_bits()
        );
        let restored_corner = match &coordinator
            .feature_document()
            .feature(feature)
            .expect("restored fold feature")
            .definition
        {
            ComputedFeatureDefinition::FilletSet(fillet) => {
                assert_eq!(fillet.corners[0].id, corner);
                fillet.corners[0].without_id()
            }
        };
        assert_eq!(restored_corner, fold.corner);
        let restored_scene = current_computed_scene(&coordinator);
        assert!(restored_scene.fillet_affordances.is_empty());
        assert!(matches!(
            restored_scene
                .computed_fillet_continuation_statuses
                .as_slice(),
            [ComputedFilletContinuationStatus {
                limit: ComputedFilletContinuationLimit {
                    kind: ComputedFilletContinuationLimitKind::BranchFold,
                    ..
                },
                ..
            }]
        ));

        coordinator.redo().expect("redo numeric fold departure");
        assert_eq!(
            fillet_radius(&coordinator, feature).to_bits(),
            0.55_f64.to_bits()
        );
        let redo_feature = coordinator
            .feature_document()
            .feature(feature)
            .expect("redone regular feature");
        let ComputedFeatureDefinition::FilletSet(redo_fillet) = &redo_feature.definition;
        assert_eq!(redo_fillet.corners[0].id, corner);
        assert_eq!(redo_fillet.corners[0].without_id(), regular_corner);
        assert_eq!(coordinator.session().design_identity(), design_identity);
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            accepted_identity
        );
        assert_eq!(
            accepted_sketch_equation_invariants(&coordinator),
            sketch_equations
        );
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
            let mut scene = EditorScene::from_accepted_with_computed(
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
            .expect("composite preview scene");
            fixture
                .coordinator
                .populate_computed_fillet_affordances(&mut scene, &[], 0.5)
                .expect("temporary Fillet affordances");
            scene
        };
        let arc = scene
            .computed_curves
            .iter()
            .find(|curve| curve.owner.feature == metadata.feature)
            .expect("temporary arc")
            .clone();
        let owner = SelectionItem::FeatureCorner(arc.owner);
        let rail = arc.radius_rail.expect("validated temporary radius rail");
        let press = rail.screen_grip;
        let press_model = scene.viewport.screen_to_model(press);
        let moved = scene.viewport.model_to_screen([
            0.2_f64.mul_add(rail.model_derivative[0], press_model[0]),
            0.2_f64.mul_add(rail.model_derivative[1], press_model[1]),
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
            // This first move shortens the outer span enough to move the
            // unique line-line contact outside its old one-eighth parameter
            // neighbourhood while keeping the right-angle Fillet regular.
            (0_usize, [100.0, 0.0]),
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
            let intermediate = ScreenPoint {
                x: press.x + 0.5 * screen_delta[0],
                y: press.y + 0.5 * screen_delta[1],
            };
            let pointer_id = 800 + u64::try_from(case).expect("small case index");
            let pointer = |position| PointerInput {
                pointer_id,
                position,
                modifiers: Modifiers::default(),
            };
            let _ = fixture.coordinator.pointer_down(&scene, pointer(press));
            let first_requests = fixture
                .coordinator
                .editor_mut()
                .pointer_move(&scene, pointer(intermediate));
            let [
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point: requested,
                    model_position,
                },
            ] = first_requests.as_slice()
            else {
                panic!("case {case} must request one intermediate projected source move")
            };
            assert_eq!(*requested, point);
            let first_preview_effects = fixture.coordinator.resolve_projected_point_move(
                *pointer_id,
                *request_id,
                *requested,
                *model_position,
            );
            assert!(matches!(
                first_preview_effects.as_slice(),
                [EditorEffect::PreviewPointMove { point: previewed, .. }] if *previewed == point
            ));
            let intermediate_scene = visible_computed_scene(&fixture.coordinator);
            let requests = fixture
                .coordinator
                .editor_mut()
                .pointer_move(&intermediate_scene, pointer(moved));
            let [
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point: requested,
                    model_position,
                },
            ] = requests.as_slice()
            else {
                panic!("case {case} must request one final projected source move")
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
                [EditorEffect::CommitPointMove { point: committed, .. }] if *committed == point
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
            fixture
                .coordinator
                .editor_mut()
                .set_selection([SelectionItem::Feature(feature)]);
            let editable_scene = current_computed_scene(&fixture.coordinator);
            assert_eq!(editable_scene.fillet_affordances.len(), 2);
            assert!(
                editable_scene
                    .computed_fillet_continuation_statuses
                    .is_empty(),
                "case {case} rendered Current output but withheld a regular Fillet rail"
            );
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
    #[allow(
        clippy::too_many_lines,
        reason = "the rollback fingerprint deliberately covers every retained, computed, allocator, history and transient authority field"
    )]
    fn projected_release_discards_native_and_feature_state_when_durable_reanchor_fails() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let point = fixture.points[0];
        let scene = current_computed_scene(&fixture.coordinator);
        let press = scene
            .points
            .iter()
            .find(|candidate| candidate.id == point)
            .expect("movable source point")
            .screen_position;
        let moved = ScreenPoint {
            x: press.x - 24.0,
            y: press.y + 12.0,
        };
        let pointer = |position| PointerInput {
            pointer_id: 8_901,
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
            panic!("source drag must request one projected move")
        };
        let preview = fixture.coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *requested,
            *model_position,
        );
        assert!(matches!(
            preview.as_slice(),
            [EditorEffect::PreviewPointMove { point: previewed, .. }] if *previewed == point
        ));
        assert!(matches!(
            computed_feature_state(
                fixture
                    .coordinator
                    .computed_preview_snapshot
                    .as_ref()
                    .expect("complete computed preview"),
                feature,
            ),
            ComputedFeatureEvaluationState::Current { .. }
        ));
        let expected = fixture.coordinator.session().design_identity();
        let release = fixture
            .coordinator
            .editor_mut()
            .pointer_up(&scene, expected, pointer(moved));
        let [commit @ EditorEffect::CommitPointMove { .. }] = release.as_slice() else {
            panic!("valid source preview must offer one exact release")
        };

        // The first staged evaluation may allocate `MAX - 1`; the cold
        // durability proof then fails while trying to allocate `MAX`. Because
        // all release work is staged, neither allocation can leak into live
        // state and the held preview remains retryable.
        let exhausted = ComputedEvaluationAllocatorHighWater {
            next_revision: geosolve_sketch_features::ComputedEvaluationRevision::from_raw(
                u64::MAX - 1,
            ),
        };
        fixture
            .coordinator
            .computed_evaluation_allocator
            .retain_high_water(exhausted);
        let before_design = fixture.coordinator.session().design_identity();
        let before_prepared = fixture
            .coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input before rejected release");
        let before_design_json = fixture.coordinator.session().export_design_json().unwrap();
        let before_accepted_json = fixture
            .coordinator
            .session()
            .export_accepted_json()
            .unwrap();
        let before_features = fixture.coordinator.feature_document().to_json().unwrap();
        let before_feature_identity = fixture.coordinator.feature_document().identity();
        let before_computed_input = fixture.coordinator.computed_input;
        let before_computed_revision = fixture
            .coordinator
            .computed_snapshot
            .as_ref()
            .expect("retained computed snapshot")
            .evaluation_revision();
        let before_preview_input = fixture.coordinator.computed_preview_input;
        let before_preview_revision = fixture
            .coordinator
            .computed_preview_snapshot
            .as_ref()
            .expect("held computed preview")
            .evaluation_revision();
        let before_solved_preview = fixture
            .coordinator
            .solved_preview_session()
            .expect("held solved preview")
            .prepared_input();
        let before_history_len = fixture.coordinator.history_len();
        let before_history_cursor = fixture.coordinator.history_cursor();
        let before_transcript = fixture.coordinator.transcript().to_vec();
        let before_checkpoint = fixture.coordinator.persistence_checkpoint().unwrap();

        assert!(matches!(
            fixture.coordinator.apply_editor_effect(commit),
            Err(CoordinatorError::ComputedFeatureEvaluation(
                ComputedFeatureEvaluationError::EvaluationIdentityExhausted
            ))
        ));
        assert_eq!(
            fixture.coordinator.session().design_identity(),
            before_design
        );
        assert_eq!(
            fixture.coordinator.session().accepted_prepared_input(),
            Some(before_prepared)
        );
        assert_eq!(
            fixture.coordinator.session().export_design_json().unwrap(),
            before_design_json
        );
        assert_eq!(
            fixture
                .coordinator
                .session()
                .export_accepted_json()
                .unwrap(),
            before_accepted_json
        );
        assert_eq!(
            fixture.coordinator.feature_document().to_json().unwrap(),
            before_features
        );
        assert_eq!(
            fixture.coordinator.feature_document().identity(),
            before_feature_identity
        );
        assert_eq!(fixture.coordinator.computed_input, before_computed_input);
        assert_eq!(
            fixture
                .coordinator
                .computed_snapshot
                .as_ref()
                .expect("same retained computed snapshot")
                .evaluation_revision(),
            before_computed_revision
        );
        assert_eq!(
            fixture.coordinator.computed_preview_input,
            before_preview_input
        );
        assert_eq!(
            fixture
                .coordinator
                .computed_preview_snapshot
                .as_ref()
                .expect("same held computed preview")
                .evaluation_revision(),
            before_preview_revision
        );
        assert_eq!(
            fixture
                .coordinator
                .solved_preview_session()
                .expect("same held solved preview")
                .prepared_input(),
            before_solved_preview
        );
        assert_eq!(fixture.coordinator.history_len(), before_history_len);
        assert_eq!(fixture.coordinator.history_cursor(), before_history_cursor);
        assert_eq!(fixture.coordinator.transcript(), before_transcript);
        assert_eq!(
            fixture
                .coordinator
                .persistence_checkpoint()
                .unwrap()
                .computed_evaluation_high_water(),
            before_checkpoint.computed_evaluation_high_water()
        );
        assert_eq!(
            fixture
                .coordinator
                .persistence_checkpoint()
                .unwrap()
                .feature_json(),
            before_checkpoint.feature_json()
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the UAT regression keeps source edits, rails, grouped publication and native-sketch invariants in one scenario"
    )]
    fn large_affine_source_edits_keep_grouped_fillet_affordances_adjustable() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&fixture.coordinator, fixture.points[1..=2].iter().copied());
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        fixture
            .coordinator
            .set_computed_fillet_radius(
                fixture.coordinator.feature_document().identity(),
                feature,
                0.5,
            )
            .expect("establish edited shared radius");

        // Shortening both outer spans keeps the two right-angle Fillets regular,
        // but moves their unique affine contacts well outside the old one-eighth
        // parameter neighbourhoods retained by the feature intent.
        for (point, position) in [
            (fixture.points[0], [3.0, 0.0]),
            (fixture.points[3], [5.0, 4.0]),
        ] {
            let moved = fixture
                .coordinator
                .apply_edit(
                    fixture.coordinator.session().design_identity(),
                    DocumentEdit::SetPointPosition { point, position },
                )
                .expect("accepted large affine source edit");
            assert!(moved.published_accepted.is_some());
            assert!(matches!(
                computed_feature_state(
                    fixture
                        .coordinator
                        .computed_snapshot()
                        .expect("current output after source edit"),
                    feature,
                ),
                ComputedFeatureEvaluationState::Current { .. }
            ));
        }

        let design_before_radius = fixture.coordinator.session().design_identity();
        let accepted_before_radius = fixture
            .coordinator
            .session()
            .accepted_state()
            .expect("accepted edited sketch")
            .identity();
        let design_json_before_radius = fixture
            .coordinator
            .session()
            .export_design_json()
            .expect("edited design JSON");
        let accepted_json_before_radius = fixture
            .coordinator
            .session()
            .export_accepted_json()
            .expect("edited accepted JSON");
        let equation_invariants_before_radius =
            accepted_sketch_equation_invariants(&fixture.coordinator);
        let ComputedFeatureDefinition::FilletSet(set_before_radius) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("edited Fillet set")
            .definition;
        let corner_ids_before_radius = set_before_radius
            .corners
            .iter()
            .map(|corner| corner.id)
            .collect::<Vec<_>>();

        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(feature)]);
        let scene = current_computed_scene(&fixture.coordinator);
        assert_eq!(scene.fillet_affordances.len(), 2);
        assert!(scene.computed_fillet_continuation_statuses.is_empty());
        assert!(scene.fillet_affordances.iter().all(|affordances| {
            let derivative = affordances.radius_rail.model_derivative;
            derivative.into_iter().all(f64::is_finite) && derivative[0].hypot(derivative[1]) > 0.0
        }));

        let origin = scene.computed_input.expect("current edited computed input");
        let history_before = fixture.coordinator.history_len();
        let preview = fixture
            .coordinator
            .preview_computed_fillet_radius_exact(origin, feature, 0.6)
            .expect("preview re-anchored grouped radius");
        assert!(matches!(
            computed_feature_state(preview, feature),
            ComputedFeatureEvaluationState::Current { .. }
        ));
        fixture
            .coordinator
            .set_computed_fillet_radius_exact(origin, feature, 0.6)
            .expect("publish re-anchored grouped radius");
        assert_eq!(
            fillet_radius(&fixture.coordinator, feature).to_bits(),
            0.6_f64.to_bits()
        );
        assert_eq!(fixture.coordinator.history_len(), history_before + 1);
        assert_eq!(
            fixture.coordinator.session().design_identity(),
            design_before_radius
        );
        assert_eq!(
            fixture
                .coordinator
                .session()
                .accepted_state()
                .expect("accepted sketch after feature edit")
                .identity(),
            accepted_before_radius
        );
        assert_eq!(
            fixture
                .coordinator
                .session()
                .export_design_json()
                .expect("design JSON after feature edit"),
            design_json_before_radius
        );
        assert_eq!(
            fixture
                .coordinator
                .session()
                .export_accepted_json()
                .expect("accepted JSON after feature edit"),
            accepted_json_before_radius
        );
        assert_eq!(
            accepted_sketch_equation_invariants(&fixture.coordinator),
            equation_invariants_before_radius
        );
        let ComputedFeatureDefinition::FilletSet(set_after_radius) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("re-anchored Fillet set")
            .definition;
        assert_eq!(
            set_after_radius
                .corners
                .iter()
                .map(|corner| corner.id)
                .collect::<Vec<_>>(),
            corner_ids_before_radius
        );

        let published = current_computed_scene(&fixture.coordinator);
        assert_eq!(published.fillet_affordances.len(), 2);
        assert!(published.computed_fillet_continuation_statuses.is_empty());
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
    #[allow(clippy::too_many_lines)]
    fn published_computed_arc_pointer_preview_commits_and_undoes_one_history_step() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("published Fillet")
            .definition;
        let owner = ComputedCornerRef {
            feature,
            corner: fillet.corners[0].id,
        };
        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
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
        let rail = arc.radius_rail.expect("validated radius rail");
        let press_model = rail.model_grip;
        let move_model = [
            0.2_f64.mul_add(rail.model_derivative[0], press_model[0]),
            0.2_f64.mul_add(rail.model_derivative[1], press_model[1]),
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
        assert!(
            (*preview_radius - (arc.radius + 0.2)).abs() <= 1.0e-10,
            "the first drag sample jumped from {} to {preview_radius}",
            arc.radius
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
        assert_eq!(
            committed.evaluation_revision(),
            preview_evaluation,
            "release must publish the exact last Current preview rather than re-evaluate a request"
        );
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

    #[derive(Debug, PartialEq)]
    struct NativeSketchInvariants {
        design_identity: SketchDesignIdentity,
        accepted_identity: SketchAcceptedStateIdentity,
        design_json: String,
        accepted_json: Option<String>,
        point_position_bits: Vec<[u64; 2]>,
        equations: AcceptedSketchEquationInvariants,
    }

    fn native_sketch_invariants(
        coordinator: &RetainedEditorCoordinator,
        points: &[DesignPointId],
    ) -> NativeSketchInvariants {
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted native sketch");
        NativeSketchInvariants {
            design_identity: coordinator.session().design_identity(),
            accepted_identity: accepted.identity(),
            design_json: coordinator
                .session()
                .export_design_json()
                .expect("native design JSON"),
            accepted_json: coordinator
                .session()
                .export_accepted_json()
                .expect("native accepted JSON"),
            point_position_bits: points
                .iter()
                .map(|point| {
                    let position = accepted
                        .document()
                        .point(*point)
                        .expect("accepted native point")
                        .position;
                    [position[0].to_bits(), position[1].to_bits()]
                })
                .collect(),
            equations: accepted_sketch_equation_invariants(coordinator),
        }
    }

    fn computed_feature_semantics(
        coordinator: &RetainedEditorCoordinator,
        feature: ComputedFeatureId,
    ) -> (String, bool, ComputedFeatureDefinition) {
        let feature = coordinator
            .feature_document()
            .feature(feature)
            .expect("computed feature semantics");
        (
            feature.label.clone(),
            feature.suppressed,
            feature.definition.clone(),
        )
    }

    fn authoring_radius_path(
        session: RetainedSketchDocumentSession,
        corner: DesignPointId,
        deltas: &[f64],
    ) -> FeatureAuthoringCandidate {
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let mut authoring = FeatureAuthoringState::default();
        let candidate = feature_candidate(authoring.activate(
            &snapshot,
            coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Point(corner), None)],
        ));
        coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "authoring radius path",
            )
            .expect("initial authoring radius preview");
        let owner = coordinator
            .feature_authoring_preview()
            .expect("held authoring radius preview")
            .corner_bindings()[0]
            .owner;
        let mut scene = visible_computed_scene(&coordinator);
        let rail = scene
            .fillet_affordances
            .iter()
            .find(|candidate| candidate.owner == owner)
            .expect("authoring radius affordances")
            .radius_rail;
        coordinator
            .transact_feature_authoring_pointer_down(
                &mut authoring,
                &scene,
                PointerInput {
                    pointer_id: 905,
                    position: rail.screen_grip,
                    modifiers: Modifiers::default(),
                },
                Some(SelectionItem::FeatureCorner(owner)),
                PickTolerance::default(),
                "authoring radius press",
            )
            .expect("start authoring radius path");
        for delta in deltas {
            let model_position = [
                delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
                delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
            ];
            let effects = coordinator.editor_mut().pointer_move(
                &scene,
                PointerInput {
                    pointer_id: 905,
                    position: scene.viewport.model_to_screen(model_position),
                    modifiers: Modifiers::default(),
                },
            );
            assert!(matches!(
                effects.as_slice(),
                [EditorEffect::PreviewComputedFeatureRadius { .. }]
            ));
            coordinator
                .apply_feature_authoring_editor_effect(&mut authoring, &effects[0])
                .expect("accept authoring radius sample");
            scene = visible_computed_scene(&coordinator);
        }
        coordinator
            .feature_authoring_preview()
            .expect("final authoring radius preview")
            .candidate()
            .clone()
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum RadiusModelSample {
        A,
        B,
    }

    impl RadiusModelSample {
        const fn index(self) -> usize {
            match self {
                Self::A => 0,
                Self::B => 1,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum RadiusModelPointerSample {
        Origin,
        Valid(RadiusModelSample),
        Invalid,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum RadiusModelPhase {
        Idle,
        Live,
        Cancelled,
        Released,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    struct RadiusReferenceState {
        phase: RadiusModelPhase,
        moved: bool,
        pointer_sample: RadiusModelPointerSample,
        last_sampled: Option<RadiusModelSample>,
        pending: Option<RadiusModelSample>,
        current: Option<RadiusModelSample>,
        held_preview: Option<RadiusModelSample>,
        emitted: [bool; 2],
        durable: Option<RadiusModelSample>,
    }

    impl RadiusReferenceState {
        const fn idle() -> Self {
            Self {
                phase: RadiusModelPhase::Idle,
                moved: false,
                pointer_sample: RadiusModelPointerSample::Origin,
                last_sampled: None,
                pending: None,
                current: None,
                held_preview: None,
                emitted: [false; 2],
                durable: None,
            }
        }

        const fn terminal(phase: RadiusModelPhase, durable: Option<RadiusModelSample>) -> Self {
            Self {
                phase,
                durable,
                ..Self::idle()
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RadiusModelEvent {
        OwnerDown,
        ForeignDown,
        OwnerMoveA,
        OwnerMoveB,
        ForeignMove,
        OwnerMoveInvalid,
        AcknowledgeA,
        AcknowledgeB,
        OwnerReleaseLatest,
        OwnerReleaseInvalid,
        ForeignRelease,
        Cancel,
    }

    const RADIUS_MODEL_EVENTS: [RadiusModelEvent; 12] = [
        RadiusModelEvent::OwnerDown,
        RadiusModelEvent::ForeignDown,
        RadiusModelEvent::OwnerMoveA,
        RadiusModelEvent::OwnerMoveB,
        RadiusModelEvent::ForeignMove,
        RadiusModelEvent::OwnerMoveInvalid,
        RadiusModelEvent::AcknowledgeA,
        RadiusModelEvent::AcknowledgeB,
        RadiusModelEvent::OwnerReleaseLatest,
        RadiusModelEvent::OwnerReleaseInvalid,
        RadiusModelEvent::ForeignRelease,
        RadiusModelEvent::Cancel,
    ];

    fn radius_reference_transition(
        state: RadiusReferenceState,
        event: RadiusModelEvent,
    ) -> Option<RadiusReferenceState> {
        if event == RadiusModelEvent::OwnerDown {
            return (state.phase == RadiusModelPhase::Idle).then_some(RadiusReferenceState {
                phase: RadiusModelPhase::Live,
                ..state
            });
        }
        if state.phase != RadiusModelPhase::Live {
            return None;
        }
        let mut next = state;
        match event {
            RadiusModelEvent::OwnerDown => unreachable!(),
            RadiusModelEvent::ForeignDown
            | RadiusModelEvent::ForeignMove
            | RadiusModelEvent::ForeignRelease => {}
            RadiusModelEvent::OwnerMoveA | RadiusModelEvent::OwnerMoveB => {
                let sample = if event == RadiusModelEvent::OwnerMoveA {
                    RadiusModelSample::A
                } else {
                    RadiusModelSample::B
                };
                next.moved = true;
                next.pointer_sample = RadiusModelPointerSample::Valid(sample);
                if next.last_sampled != Some(sample) {
                    next.last_sampled = Some(sample);
                    next.pending = Some(sample);
                    next.current = None;
                    next.emitted[sample.index()] = true;
                }
            }
            RadiusModelEvent::OwnerMoveInvalid => {
                next.moved = true;
                next.pointer_sample = RadiusModelPointerSample::Invalid;
                next.last_sampled = None;
                next.pending = None;
                next.current = None;
            }
            RadiusModelEvent::AcknowledgeA | RadiusModelEvent::AcknowledgeB => {
                let sample = if event == RadiusModelEvent::AcknowledgeA {
                    RadiusModelSample::A
                } else {
                    RadiusModelSample::B
                };
                if !next.emitted[sample.index()] {
                    return None;
                }
                if next.pending == Some(sample) {
                    next.pending = None;
                    next.current = Some(sample);
                    next.held_preview = Some(sample);
                }
            }
            RadiusModelEvent::OwnerReleaseLatest => {
                let durable = next.current.filter(|sample| {
                    next.pointer_sample == RadiusModelPointerSample::Valid(*sample)
                });
                return Some(RadiusReferenceState::terminal(
                    RadiusModelPhase::Released,
                    durable,
                ));
            }
            RadiusModelEvent::OwnerReleaseInvalid => {
                return Some(RadiusReferenceState::terminal(
                    RadiusModelPhase::Released,
                    None,
                ));
            }
            RadiusModelEvent::Cancel => {
                return Some(RadiusReferenceState::terminal(
                    RadiusModelPhase::Cancelled,
                    None,
                ));
            }
        }
        Some(next)
    }

    fn bounded_radius_model_transitions() -> Vec<(
        Vec<RadiusModelEvent>,
        RadiusModelEvent,
        RadiusReferenceState,
    )> {
        let initial = RadiusReferenceState::idle();
        let mut paths = std::collections::BTreeMap::from([(initial, Vec::new())]);
        let mut queue = std::collections::VecDeque::from([initial]);
        let mut transitions = Vec::new();
        while let Some(state) = queue.pop_front() {
            if !matches!(state.phase, RadiusModelPhase::Idle | RadiusModelPhase::Live) {
                continue;
            }
            let prefix = paths.get(&state).expect("known model state").clone();
            for event in RADIUS_MODEL_EVENTS {
                let Some(next) = radius_reference_transition(state, event) else {
                    continue;
                };
                transitions.push((prefix.clone(), event, next));
                if let std::collections::btree_map::Entry::Vacant(entry) = paths.entry(next) {
                    let mut next_prefix = prefix.clone();
                    next_prefix.push(event);
                    entry.insert(next_prefix);
                    queue.push_back(next);
                }
            }
        }
        let phase_count = |phase| paths.keys().filter(|state| state.phase == phase).count();
        assert_eq!(paths.len(), 28, "bounded model state count changed");
        assert_eq!(phase_count(RadiusModelPhase::Idle), 1);
        assert_eq!(phase_count(RadiusModelPhase::Live), 23);
        assert_eq!(phase_count(RadiusModelPhase::Cancelled), 1);
        assert_eq!(phase_count(RadiusModelPhase::Released), 3);
        assert_eq!(transitions.len(), 240, "bounded transition count changed");
        assert!(
            paths.values().all(|path| path.len() <= 5),
            "canonical model prefix escaped its fixed bound"
        );
        transitions
    }

    #[derive(Clone, Debug)]
    struct RadiusCurrentEvidence {
        radius_bits: u64,
        feature_json: String,
        evaluation_revision: u64,
    }

    struct RadiusTransitionHarness {
        fixture: ComputedFilletEditorFixture,
        feature: ComputedFeatureId,
        scene: EditorScene,
        grip: ScreenPoint,
        valid_positions: [ScreenPoint; 2],
        invalid_position: ScreenPoint,
        expected_design: SketchDesignIdentity,
        model: RadiusReferenceState,
        emitted_effects: [Option<EditorEffect>; 2],
        current_evidence: [Option<RadiusCurrentEvidence>; 2],
        owner_position: ScreenPoint,
        origin_feature_json: String,
        origin_radius_bits: u64,
        origin_evaluation_revision: u64,
        origin_history_len: usize,
        origin_history_cursor: usize,
        origin_corner_ids: Vec<ComputedFeatureCornerId>,
        native_before: NativeSketchInvariants,
    }

    impl RadiusTransitionHarness {
        const OWNER_POINTER: u64 = 9_701;
        const FOREIGN_POINTER: u64 = 9_702;

        fn new() -> Self {
            let mut fixture = computed_fillet_editor_fixture();
            let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
            let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
            let (owner, origin_corner_ids) = {
                let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
                    .coordinator
                    .feature_document()
                    .feature(feature)
                    .expect("published model Fillet")
                    .definition;
                (
                    ComputedCornerRef {
                        feature,
                        corner: fillet.corners[0].id,
                    },
                    fillet.corners.iter().map(|corner| corner.id).collect(),
                )
            };
            fixture
                .coordinator
                .editor_mut()
                .set_selection([SelectionItem::FeatureCorner(owner)]);
            let native_before = native_sketch_invariants(&fixture.coordinator, &fixture.points);
            let origin_feature_json = fixture
                .coordinator
                .feature_document()
                .to_json()
                .expect("model origin feature JSON");
            let origin_radius = fillet_radius(&fixture.coordinator, feature);
            let origin_evaluation_revision = fixture
                .coordinator
                .computed_snapshot()
                .expect("model origin computed snapshot")
                .evaluation_revision()
                .raw();
            let scene = current_computed_scene(&fixture.coordinator);
            let rail = scene
                .computed_curves
                .iter()
                .find(|curve| curve.owner == owner)
                .and_then(|curve| curve.radius_rail)
                .expect("model radius rail");
            let model_position = |delta: f64| {
                [
                    delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
                    delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
                ]
            };
            let valid_positions =
                [0.2, 0.35].map(|delta| scene.viewport.model_to_screen(model_position(delta)));
            let invalid_position = scene
                .viewport
                .model_to_screen(model_position(-(origin_radius + 1.0)));
            let grip = rail.screen_grip;
            let expected_design = fixture.coordinator.session().design_identity();
            let origin_history_len = fixture.coordinator.history_len();
            let origin_history_cursor = fixture.coordinator.history_cursor();
            Self {
                fixture,
                feature,
                scene,
                grip,
                valid_positions,
                invalid_position,
                expected_design,
                model: RadiusReferenceState::idle(),
                emitted_effects: [None, None],
                current_evidence: [None, None],
                owner_position: grip,
                origin_feature_json,
                origin_radius_bits: origin_radius.to_bits(),
                origin_evaluation_revision,
                origin_history_len,
                origin_history_cursor,
                origin_corner_ids,
                native_before,
            }
        }

        fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
            PointerInput {
                pointer_id,
                position,
                modifiers: Modifiers::default(),
            }
        }

        fn sample_position(&self, sample: RadiusModelSample) -> ScreenPoint {
            self.valid_positions[sample.index()]
        }

        fn sample_effect_radius_bits(&self, sample: RadiusModelSample) -> u64 {
            match self.emitted_effects[sample.index()]
                .as_ref()
                .expect("model sample effect")
            {
                EditorEffect::PreviewComputedFeatureRadius { radius, .. } => radius.to_bits(),
                effect => panic!("unexpected model sample effect: {effect:?}"),
            }
        }

        #[allow(clippy::too_many_lines)]
        fn execute(&mut self, event: RadiusModelEvent) {
            let before = self.model;
            let next = radius_reference_transition(before, event)
                .expect("transition graph supplied an applicable event");
            match event {
                RadiusModelEvent::OwnerDown => {
                    let effects = self
                        .fixture
                        .coordinator
                        .pointer_down(&self.scene, Self::pointer(Self::OWNER_POINTER, self.grip));
                    assert!(effects.is_empty(), "owner down effects: {effects:?}");
                }
                RadiusModelEvent::ForeignDown => {
                    let effects = self
                        .fixture
                        .coordinator
                        .pointer_down(&self.scene, Self::pointer(Self::FOREIGN_POINTER, self.grip));
                    assert!(effects.is_empty(), "foreign down effects: {effects:?}");
                }
                RadiusModelEvent::OwnerMoveA | RadiusModelEvent::OwnerMoveB => {
                    let sample = if event == RadiusModelEvent::OwnerMoveA {
                        RadiusModelSample::A
                    } else {
                        RadiusModelSample::B
                    };
                    let position = self.sample_position(sample);
                    self.owner_position = position;
                    let input = Self::pointer(Self::OWNER_POINTER, position);
                    let effects = self
                        .fixture
                        .coordinator
                        .editor_mut()
                        .pointer_move(&self.scene, input);
                    if before.last_sampled == Some(sample) {
                        assert!(effects.is_empty(), "duplicate sample effects: {effects:?}");
                    } else {
                        assert!(matches!(
                            effects.as_slice(),
                            [EditorEffect::PreviewComputedFeatureRadius {
                                feature,
                                ..
                            }] if *feature == self.feature
                        ));
                        self.emitted_effects[sample.index()] = Some(effects[0].clone());
                    }
                }
                RadiusModelEvent::ForeignMove => {
                    let input = Self::pointer(Self::FOREIGN_POINTER, self.valid_positions[0]);
                    let effects = self
                        .fixture
                        .coordinator
                        .editor_mut()
                        .pointer_move(&self.scene, input);
                    assert!(effects.is_empty(), "foreign move effects: {effects:?}");
                }
                RadiusModelEvent::OwnerMoveInvalid => {
                    self.owner_position = self.invalid_position;
                    let input = Self::pointer(Self::OWNER_POINTER, self.invalid_position);
                    let effects = self
                        .fixture
                        .coordinator
                        .editor_mut()
                        .pointer_move(&self.scene, input);
                    assert!(effects.is_empty(), "invalid move effects: {effects:?}");
                }
                RadiusModelEvent::AcknowledgeA | RadiusModelEvent::AcknowledgeB => {
                    let sample = if event == RadiusModelEvent::AcknowledgeA {
                        RadiusModelSample::A
                    } else {
                        RadiusModelSample::B
                    };
                    let effect = self.emitted_effects[sample.index()]
                        .as_ref()
                        .expect("emitted acknowledgement effect")
                        .clone();
                    let result = self.fixture.coordinator.apply_editor_effect(&effect);
                    if before.pending == Some(sample) {
                        result.expect("exact Current acknowledgement");
                        let preview = self
                            .fixture
                            .coordinator
                            .computed_fillet_preview
                            .as_ref()
                            .expect("acknowledged held preview");
                        self.current_evidence[sample.index()] = Some(RadiusCurrentEvidence {
                            radius_bits: preview.radius.to_bits(),
                            feature_json: preview
                                .features
                                .to_json()
                                .expect("acknowledged feature JSON"),
                            evaluation_revision: preview.snapshot.evaluation_revision().raw(),
                        });
                    } else {
                        assert!(
                            result.is_err(),
                            "stale/duplicate acknowledgement unexpectedly succeeded"
                        );
                    }
                }
                RadiusModelEvent::OwnerReleaseLatest | RadiusModelEvent::OwnerReleaseInvalid => {
                    let release_position = if event == RadiusModelEvent::OwnerReleaseLatest {
                        self.owner_position
                    } else {
                        self.invalid_position
                    };
                    let input = Self::pointer(Self::OWNER_POINTER, release_position);
                    let effects = self.fixture.coordinator.editor_mut().pointer_up(
                        &self.scene,
                        self.expected_design,
                        input,
                    );
                    let expected_commit = next.durable;
                    match expected_commit {
                        Some(sample) => assert!(matches!(
                            effects.as_slice(),
                            [
                                EditorEffect::CommitComputedFeatureRadius { radius, .. },
                                EditorEffect::ClearComputedFeaturePreview,
                            ] if radius.to_bits() == self.sample_effect_radius_bits(sample)
                        )),
                        None if before.moved => {
                            assert_eq!(effects, vec![EditorEffect::ClearComputedFeaturePreview]);
                        }
                        None => assert!(effects.is_empty(), "click release effects: {effects:?}"),
                    }
                    for effect in &effects {
                        self.fixture
                            .coordinator
                            .apply_editor_effect(effect)
                            .expect("model release effect");
                    }
                    self.emitted_effects = [None, None];
                }
                RadiusModelEvent::ForeignRelease => {
                    let input = Self::pointer(Self::FOREIGN_POINTER, self.valid_positions[0]);
                    let effects = self.fixture.coordinator.editor_mut().pointer_up(
                        &self.scene,
                        self.expected_design,
                        input,
                    );
                    assert!(effects.is_empty(), "foreign release effects: {effects:?}");
                }
                RadiusModelEvent::Cancel => {
                    let effects = self.fixture.coordinator.editor_mut().cancel();
                    assert!(matches!(
                        effects.as_slice(),
                        [EditorEffect::RestoreComputedFeatureRadius {
                            feature,
                            radius,
                            ..
                        }] if *feature == self.feature
                            && radius.to_bits() == self.origin_radius_bits
                    ));
                    for effect in &effects {
                        self.fixture
                            .coordinator
                            .apply_editor_effect(effect)
                            .expect("model cancellation effect");
                    }
                    self.emitted_effects = [None, None];
                }
            }
            self.model = next;
            self.assert_observation();
        }

        #[allow(clippy::too_many_lines)]
        fn assert_observation(&self) {
            let coordinator = &self.fixture.coordinator;
            let expected_durable = self.model.durable.map(|sample| {
                self.current_evidence[sample.index()]
                    .as_ref()
                    .expect("durable Current evidence")
            });
            let expected_feature_json = expected_durable
                .map_or(self.origin_feature_json.as_str(), |evidence| {
                    evidence.feature_json.as_str()
                });
            assert_eq!(
                coordinator
                    .feature_document()
                    .to_json()
                    .expect("observed feature JSON"),
                expected_feature_json
            );
            let expected_radius_bits =
                expected_durable.map_or(self.origin_radius_bits, |evidence| evidence.radius_bits);
            assert_eq!(
                fillet_radius(coordinator, self.feature).to_bits(),
                expected_radius_bits
            );
            let ComputedFeatureDefinition::FilletSet(fillet) = &coordinator
                .feature_document()
                .feature(self.feature)
                .expect("stable model feature")
                .definition;
            assert_eq!(
                fillet
                    .corners
                    .iter()
                    .map(|corner| corner.id)
                    .collect::<Vec<_>>(),
                self.origin_corner_ids
            );
            let history_delta = usize::from(self.model.durable.is_some());
            assert_eq!(
                coordinator.history_len(),
                self.origin_history_len + history_delta
            );
            assert_eq!(
                coordinator.history_cursor(),
                self.origin_history_cursor + history_delta
            );
            assert_eq!(
                native_sketch_invariants(coordinator, &self.fixture.points),
                self.native_before
            );

            if let Some(sample) = self.model.held_preview {
                let expected = self.current_evidence[sample.index()]
                    .as_ref()
                    .expect("held Current evidence");
                let held = coordinator
                    .computed_fillet_preview
                    .as_ref()
                    .expect("held coordinator preview");
                assert_eq!(held.radius.to_bits(), expected.radius_bits);
                assert_eq!(
                    held.features.to_json().expect("held feature JSON"),
                    expected.feature_json
                );
                assert_eq!(
                    held.snapshot.evaluation_revision().raw(),
                    expected.evaluation_revision
                );
                assert_eq!(
                    coordinator
                        .computed_preview_snapshot
                        .as_ref()
                        .expect("held preview snapshot")
                        .evaluation_revision()
                        .raw(),
                    expected.evaluation_revision
                );
                assert_eq!(
                    coordinator.computed_preview_input,
                    Some(held.snapshot.input())
                );
            } else {
                assert!(coordinator.computed_fillet_preview.is_none());
                assert!(coordinator.computed_preview_snapshot.is_none());
                assert!(coordinator.computed_preview_input.is_none());
            }

            let expected_visible_revision = self
                .model
                .held_preview
                .and_then(|sample| self.current_evidence[sample.index()].as_ref())
                .or(expected_durable)
                .map_or(self.origin_evaluation_revision, |evidence| {
                    evidence.evaluation_revision
                });
            assert_eq!(
                coordinator
                    .computed_snapshot()
                    .expect("model visible computed snapshot")
                    .evaluation_revision()
                    .raw(),
                expected_visible_revision
            );
            assert!(matches!(
                coordinator.computed_scene_state(),
                ComputedSceneState::Current { .. }
            ));
            if self.model.phase == RadiusModelPhase::Live {
                assert_eq!(
                    coordinator.editor().active_pointer_gesture(),
                    Some(crate::ActivePointerGesture {
                        pointer_id: Self::OWNER_POINTER,
                        kind: crate::ActivePointerGestureKind::FilletRadius,
                    })
                );
            } else {
                assert_eq!(coordinator.editor().active_pointer_gesture(), None);
            }
        }
    }

    #[test]
    fn bounded_radius_transition_model_matches_every_reachable_state_event_pair() {
        let transitions = bounded_radius_model_transitions();
        for (prefix, event, expected) in transitions {
            let mut harness = RadiusTransitionHarness::new();
            harness.assert_observation();
            for prefix_event in prefix {
                harness.execute(prefix_event);
            }
            harness.execute(event);
            assert_eq!(harness.model, expected);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn authoring_and_published_radius_transactions_share_one_atomic_absolute_configuration() {
        let mut authoring_fixture = computed_fillet_editor_fixture();
        let native_authoring_before =
            native_sketch_invariants(&authoring_fixture.coordinator, &authoring_fixture.points);
        let snapshot = authoring_fixture
            .coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let selection = authoring_fixture.points[1..=2]
            .iter()
            .copied()
            .map(|point| (SelectionItem::Point(point), None))
            .collect::<Vec<_>>();
        let mut authoring = FeatureAuthoringState::default();
        let initial = feature_candidate(authoring.activate(
            &snapshot,
            authoring_fixture.coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &selection,
        ));
        assert_eq!(initial.corners().len(), 2);
        let history_before = authoring_fixture.coordinator.history_len();

        let transaction = authoring_fixture
            .coordinator
            .transact_feature_authoring_radius(&mut authoring, Some(0.75), "numeric batch")
            .expect("Current-only authoring radius transaction");
        let resized = match transaction.outcome {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
            other => panic!("expected resized authoring preview, got {other:?}"),
        };
        let metadata = transaction.preview.expect("exact resized preview metadata");
        assert_eq!(resized.radius().to_bits(), 0.75_f64.to_bits());
        assert_eq!(resized.corners().len(), 2);
        assert!(
            authoring_fixture
                .coordinator
                .feature_document()
                .features()
                .is_empty()
        );
        assert_eq!(authoring_fixture.coordinator.history_len(), history_before);
        let held_before_invalid = authoring_fixture
            .coordinator
            .feature_authoring_preview()
            .expect("held Current preview")
            .candidate()
            .clone();
        assert_eq!(held_before_invalid, resized);
        assert!(matches!(
            computed_feature_state(
                authoring_fixture
                    .coordinator
                    .feature_authoring_preview()
                    .expect("Current whole-set preview")
                    .snapshot(),
                metadata.feature,
            ),
            ComputedFeatureEvaluationState::Current { corner_edges }
                if corner_edges.len() == 2
        ));

        let state_before_invalid = authoring.clone();
        let invalid = authoring_fixture
            .coordinator
            .transact_feature_authoring_radius(&mut authoring, Some(3.0), "crossed shared span");
        match invalid {
            Err(_)
            | Ok(FeatureAuthoringTransaction {
                outcome: FeatureAuthoringOutcome::Warning(_),
                ..
            }) => {}
            other => panic!("invalid whole-set radius unexpectedly advanced: {other:?}"),
        }
        assert_eq!(authoring, state_before_invalid);
        let held_after_invalid = authoring_fixture
            .coordinator
            .feature_authoring_preview()
            .expect("last Current preview retained");
        assert_eq!(held_after_invalid.metadata(), &metadata);
        assert_eq!(held_after_invalid.candidate(), &held_before_invalid);
        assert_eq!(authoring_fixture.coordinator.history_len(), history_before);
        assert_eq!(
            native_sketch_invariants(&authoring_fixture.coordinator, &authoring_fixture.points),
            native_authoring_before
        );

        let created = authoring_fixture
            .coordinator
            .apply_feature_authoring_preview(metadata.token, &resized)
            .expect("publish exact authoring preview")
            .value;
        assert_eq!(
            authoring_fixture.coordinator.history_len(),
            history_before + 1
        );
        let (created_ids, created_corners) = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &authoring_fixture
                .coordinator
                .feature_document()
                .feature(created)
                .expect("published grouped Fillet")
                .definition;
            (
                fillet
                    .corners
                    .iter()
                    .map(|corner| corner.id)
                    .collect::<Vec<_>>(),
                fillet
                    .corners
                    .iter()
                    .map(|corner| corner.without_id())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(created_ids.len(), 2);
        let created_snapshot = authoring_fixture
            .coordinator
            .computed_snapshot()
            .expect("published grouped output");
        assert!(matches!(
            computed_feature_state(created_snapshot, created),
            ComputedFeatureEvaluationState::Current { corner_edges }
                if corner_edges.len() == 2
        ));
        let created_arc_radii = created_snapshot
            .edges()
            .iter()
            .filter_map(|edge| match (&edge.geometry, &edge.provenance) {
                (
                    geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc),
                    ComputedEdgeProvenance::FilletArc { owner, .. },
                ) if owner.feature == created => Some(arc.radius.to_bits()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(created_arc_radii, vec![0.75_f64.to_bits(); 2]);
        assert_eq!(
            native_sketch_invariants(&authoring_fixture.coordinator, &authoring_fixture.points),
            native_authoring_before
        );

        // A feature created at the initial radius and then edited numerically
        // must land on the same absolute corners as the authoring transaction.
        let mut published_fixture = ComputedFilletEditorFixture {
            coordinator: RetainedEditorCoordinator::new(
                authoring_fixture.coordinator.session().clone(),
            )
            .expect("parallel published coordinator"),
            points: authoring_fixture.points,
            spans: authoring_fixture.spans,
        };
        let native_published_before =
            native_sketch_invariants(&published_fixture.coordinator, &published_fixture.points);
        let initial = grouped_fillet_candidate(
            &published_fixture.coordinator,
            published_fixture.points[1..=2].iter().copied(),
        );
        let published = apply_grouped_fillet(&mut published_fixture.coordinator, &initial);
        let ids_before = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &published_fixture
                .coordinator
                .feature_document()
                .feature(published)
                .expect("initial published grouped Fillet")
                .definition;
            fillet
                .corners
                .iter()
                .map(|corner| corner.id)
                .collect::<Vec<_>>()
        };
        let published_history_before = published_fixture.coordinator.history_len();
        let exact = published_fixture
            .coordinator
            .computed_evaluation_input()
            .expect("exact published input");
        published_fixture
            .coordinator
            .set_computed_fillet_radius_exact(exact, published, 0.75)
            .expect("Current-only published numeric radius");
        assert_eq!(
            published_fixture.coordinator.history_len(),
            published_history_before + 1
        );
        let (ids_after, published_corners) = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &published_fixture
                .coordinator
                .feature_document()
                .feature(published)
                .expect("resized published grouped Fillet")
                .definition;
            (
                fillet
                    .corners
                    .iter()
                    .map(|corner| corner.id)
                    .collect::<Vec<_>>(),
                fillet
                    .corners
                    .iter()
                    .map(|corner| corner.without_id())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(ids_after, ids_before);
        assert_eq!(published_corners, created_corners);
        assert_eq!(
            native_sketch_invariants(&published_fixture.coordinator, &published_fixture.points),
            native_published_before
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn published_contact_gesture_previews_cancels_commits_and_replays_one_exact_history_step() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let (owner, corner_ids_before) = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .expect("published Fillet")
                .definition;
            (
                ComputedCornerRef {
                    feature,
                    corner: fillet.corners[0].id,
                },
                fillet
                    .corners
                    .iter()
                    .map(|corner| corner.id)
                    .collect::<Vec<_>>(),
            )
        };
        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
        let native_before = native_sketch_invariants(&fixture.coordinator, &fixture.points);
        let feature_json_before = fixture
            .coordinator
            .feature_document()
            .to_json()
            .expect("feature JSON before contact gesture");
        let feature_semantics_before = computed_feature_semantics(&fixture.coordinator, feature);
        let history_before = fixture.coordinator.history_len();
        let scene = current_computed_scene(&fixture.coordinator);
        let affordances = scene
            .fillet_affordances
            .iter()
            .find(|candidate| candidate.owner == owner)
            .expect("selected Fillet affordances");
        let handle = affordances.contacts[0];
        let source_curve = scene
            .curves
            .iter()
            .find(|curve| curve.span == handle.source.span)
            .expect("named native parent");
        let far_endpoint = [
            source_curve.screen_polyline[0],
            *source_curve
                .screen_polyline
                .last()
                .expect("source polyline endpoint"),
        ]
        .into_iter()
        .max_by(|first, second| {
            first
                .distance(handle.screen_position)
                .total_cmp(&second.distance(handle.screen_position))
        })
        .expect("far source endpoint");
        let target = ScreenPoint {
            x: 0.15_f64.mul_add(
                far_endpoint.x - handle.screen_position.x,
                handle.screen_position.x,
            ),
            y: 0.15_f64.mul_add(
                far_endpoint.y - handle.screen_position.y,
                handle.screen_position.y,
            ),
        };
        let pointer = |pointer_id, position| PointerInput {
            pointer_id,
            position,
            modifiers: Modifiers::default(),
        };
        let expected_design = fixture.coordinator.session().design_identity();

        // A Current contact preview remains feature-only and cancellation
        // restores the exact durable origin.
        let _ = fixture
            .coordinator
            .editor_mut()
            .pointer_down_feature_contact_handle(
                &scene,
                pointer(801, handle.screen_position),
                handle,
            );
        let preview = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(801, target));
        let parameter = match preview.as_slice() {
            [
                EditorEffect::PreviewComputedFeatureContact {
                    owner: preview_owner,
                    parent,
                    source,
                    parameter,
                    ..
                },
            ] => {
                assert_eq!(*preview_owner, owner);
                assert_eq!(*parent, handle.parent);
                assert_eq!(*source, handle.source);
                *parameter
            }
            effects => panic!("expected one contact preview, got {effects:?}"),
        };
        fixture
            .coordinator
            .apply_editor_effect(&preview[0])
            .expect("accept Current contact preview");
        assert!(matches!(
            fixture.coordinator.computed_scene_state(),
            ComputedSceneState::Current { .. }
        ));
        assert_eq!(
            fixture
                .coordinator
                .feature_document()
                .to_json()
                .expect("durable feature JSON during preview"),
            feature_json_before
        );
        let cancelled = fixture.coordinator.editor_mut().cancel();
        assert!(matches!(
            cancelled.as_slice(),
            [EditorEffect::RestoreComputedFeatureContact {
                owner: restored,
                parent,
                source,
                ..
            }] if *restored == owner && *parent == handle.parent && *source == handle.source
        ));
        fixture
            .coordinator
            .apply_editor_effect(&cancelled[0])
            .expect("restore cancelled contact preview");
        assert_eq!(fixture.coordinator.history_len(), history_before);
        assert_eq!(
            fixture
                .coordinator
                .feature_document()
                .to_json()
                .expect("cancelled feature JSON"),
            feature_json_before
        );
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );

        // Repeating the same sample and releasing publishes the exact held
        // evaluation as one history entry.
        let _ = fixture
            .coordinator
            .editor_mut()
            .pointer_down_feature_contact_handle(
                &scene,
                pointer(802, handle.screen_position),
                handle,
            );
        let preview = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(802, target));
        fixture
            .coordinator
            .apply_editor_effect(&preview[0])
            .expect("accept committable contact preview");
        let preview_revision = match fixture.coordinator.computed_scene_state() {
            ComputedSceneState::Current { snapshot, .. } => snapshot.evaluation_revision(),
            state => panic!("expected Current contact preview, got {state:?}"),
        };
        let release = fixture.coordinator.editor_mut().pointer_up(
            &scene,
            expected_design,
            pointer(802, target),
        );
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitComputedFeatureContact {
                    owner: committed,
                    parameter: committed_parameter,
                    ..
                },
                EditorEffect::ClearComputedFeatureContactPreview,
            ] if *committed == owner && committed_parameter.to_bits() == parameter.to_bits()
        ));
        for effect in &release {
            fixture
                .coordinator
                .apply_editor_effect(effect)
                .expect("publish contact effect");
        }
        assert_eq!(fixture.coordinator.history_len(), history_before + 1);
        assert_eq!(
            fixture
                .coordinator
                .computed_snapshot()
                .expect("published contact snapshot")
                .evaluation_revision(),
            preview_revision
        );
        let feature_json_after = fixture
            .coordinator
            .feature_document()
            .to_json()
            .expect("feature JSON after contact commit");
        assert_eq!(feature_json_after, feature_json_before);
        let feature_semantics_after = computed_feature_semantics(&fixture.coordinator, feature);
        assert_eq!(
            feature_semantics_after, feature_semantics_before,
            "re-seeding this bounded line-line branch resolves back to the same absolute corner"
        );
        let corner_ids_after = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .expect("contact-edited Fillet")
                .definition;
            fillet
                .corners
                .iter()
                .map(|corner| corner.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(corner_ids_after, corner_ids_before);
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );

        let saved = fixture
            .coordinator
            .persistence_checkpoint()
            .expect("contact checkpoint");
        fixture.coordinator.undo().expect("Undo contact action");
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_semantics_before
        );
        fixture.coordinator.redo().expect("Redo contact action");
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_semantics_after
        );
        fixture.coordinator.undo().expect("Undo before reload");
        fixture
            .coordinator
            .reload(&saved)
            .expect("reload contact state");
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_semantics_after
        );
        assert_eq!(fixture.coordinator.history_len(), 1);
        assert_eq!(fixture.coordinator.history_cursor(), 0);
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );

        // The old composite input/effect is stale after publication/reload.
        let history_after_reload = fixture.coordinator.history_len();
        assert!(matches!(
            fixture
                .coordinator
                .apply_editor_effect(&EditorEffect::PreviewComputedFeatureContact {
                    expected: scene.computed_input.expect("origin input"),
                    owner,
                    parent: handle.parent,
                    source: handle.source,
                    parameter,
                }),
            Err(CoordinatorError::StaleComputedFeatureCandidate)
        ));
        assert_eq!(fixture.coordinator.history_len(), history_after_reload);
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn published_radius_gesture_survives_current_rerenders_and_recovers_after_an_invalid_sample() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let owner = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .expect("published Fillet")
                .definition;
            ComputedCornerRef {
                feature,
                corner: fillet.corners[0].id,
            }
        };
        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
        let native_before = native_sketch_invariants(&fixture.coordinator, &fixture.points);
        let history_before = fixture.coordinator.history_len();
        let origin_scene = current_computed_scene(&fixture.coordinator);
        let origin_input = origin_scene.computed_input.expect("origin computed input");
        let rail = origin_scene
            .fillet_affordances
            .iter()
            .find(|candidate| candidate.owner == owner)
            .expect("selected Fillet affordances")
            .radius_rail;
        let origin_radius = fillet_radius(&fixture.coordinator, feature);
        let sample_model = |delta: f64| {
            [
                delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
                delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
            ]
        };
        let pointer = |model_position| PointerInput {
            pointer_id: 901,
            position: origin_scene.viewport.model_to_screen(model_position),
            modifiers: Modifiers::default(),
        };
        let expected_design = fixture.coordinator.session().design_identity();

        let _ = fixture
            .coordinator
            .pointer_down(&origin_scene, pointer(rail.model_grip));
        let first = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&origin_scene, pointer(sample_model(0.1)));
        assert!(matches!(
            first.as_slice(),
            [EditorEffect::PreviewComputedFeatureRadius { radius, .. }]
                if (*radius - (origin_radius + 0.1)).abs() <= 1.0e-10
        ));
        fixture
            .coordinator
            .apply_editor_effect(&first[0])
            .expect("accept first Current sample");
        let first_scene = visible_computed_scene(&fixture.coordinator);
        assert_ne!(first_scene.computed_input, Some(origin_input));

        let invalid_delta = -(origin_radius + 1.0);
        assert!(
            fixture
                .coordinator
                .editor_mut()
                .pointer_move(&first_scene, pointer(sample_model(invalid_delta)))
                .is_empty(),
            "an invalid sample must revoke acknowledgement without replacing the Current scene"
        );
        let recovered = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&first_scene, pointer(sample_model(0.25)));
        let recovered_radius = match recovered.as_slice() {
            [
                EditorEffect::PreviewComputedFeatureRadius {
                    radius, expected, ..
                },
            ] => {
                assert_eq!(*expected, origin_input);
                *radius
            }
            effects => panic!("rerendered scene did not recover radius gesture: {effects:?}"),
        };
        assert!((recovered_radius - (origin_radius + 0.25)).abs() <= 1.0e-10);
        fixture
            .coordinator
            .apply_editor_effect(&recovered[0])
            .expect("accept recovered Current sample");
        let recovered_scene = visible_computed_scene(&fixture.coordinator);
        assert_ne!(recovered_scene.computed_input, Some(origin_input));
        let preview_revision = match fixture.coordinator.computed_scene_state() {
            ComputedSceneState::Current { snapshot, .. } => snapshot.evaluation_revision(),
            state => panic!("recovered preview was not Current: {state:?}"),
        };

        let release = fixture.coordinator.editor_mut().pointer_up(
            &recovered_scene,
            expected_design,
            pointer(sample_model(0.25)),
        );
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitComputedFeatureRadius {
                    expected,
                    feature: committed,
                    radius,
                },
                EditorEffect::ClearComputedFeaturePreview,
            ] if *expected == origin_input
                && *committed == feature
                && radius.to_bits() == recovered_radius.to_bits()
        ));
        for effect in &release {
            fixture
                .coordinator
                .apply_editor_effect(effect)
                .expect("publish recovered radius gesture");
        }
        assert_eq!(fixture.coordinator.history_len(), history_before + 1);
        assert_eq!(
            fillet_radius(&fixture.coordinator, feature).to_bits(),
            recovered_radius.to_bits()
        );
        assert_eq!(
            fixture
                .coordinator
                .computed_snapshot()
                .expect("exact recovered publication")
                .evaluation_revision(),
            preview_revision
        );
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );
    }

    #[test]
    fn valid_authoring_owner_with_a_far_non_hit_is_exactly_state_neutral() {
        let mut fixture = computed_fillet_editor_fixture();
        let snapshot = fixture
            .coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let mut authoring = FeatureAuthoringState::default();
        let candidate = feature_candidate(authoring.activate(
            &snapshot,
            fixture.coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Point(fixture.points[1]), None)],
        ));
        let metadata = fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &candidate,
                "far authoring press",
            )
            .expect("held authoring preview");
        let owner = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("authoring preview")
            .corner_bindings()[0]
            .owner;
        let scene = visible_computed_scene(&fixture.coordinator);
        let authoring_before = authoring.clone();
        let feature_identity_before = fixture.coordinator.feature_document().identity();
        let history_before = fixture.coordinator.history_len();
        {
            let preview = fixture
                .coordinator
                .feature_authoring_preview()
                .expect("origin preview");
            assert!(preview.radius_origin_state.is_none());
            assert!(preview.accepted_contact_sample.is_none());
            assert_eq!(preview.radius_origin.metadata, metadata);
        }

        let rejected = fixture.coordinator.transact_feature_authoring_pointer_down(
            &mut authoring,
            &scene,
            PointerInput {
                pointer_id: 9_007,
                position: ScreenPoint {
                    x: -1_000_000.0,
                    y: -1_000_000.0,
                },
                modifiers: Modifiers::default(),
            },
            Some(SelectionItem::FeatureCorner(owner)),
            PickTolerance::default(),
            "rejected far authoring press",
        );
        assert!(matches!(
            rejected,
            Err(CoordinatorError::FeatureAuthoringPreviewMismatch)
        ));
        assert_eq!(authoring, authoring_before);
        assert_eq!(fixture.coordinator.history_len(), history_before);
        assert_eq!(
            fixture.coordinator.feature_document().identity(),
            feature_identity_before
        );
        assert_eq!(fixture.coordinator.editor().active_pointer_gesture(), None);
        let preview = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("rejected press retained exact preview");
        assert_eq!(preview.metadata, metadata);
        assert_eq!(preview.candidate, candidate);
        assert!(preview.radius_origin_state.is_none());
        assert!(preview.accepted_contact_sample.is_none());
        assert_eq!(preview.radius_origin.metadata, metadata);
        assert_eq!(preview.radius_origin.candidate, candidate);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn authoring_radius_gesture_survives_current_rerenders_before_one_atomic_apply() {
        let mut fixture = computed_fillet_editor_fixture();
        let dense = authoring_radius_path(
            fixture.coordinator.session().clone(),
            fixture.points[1],
            &[0.1, 0.25],
        );
        let direct = authoring_radius_path(
            fixture.coordinator.session().clone(),
            fixture.points[1],
            &[0.25],
        );
        assert_eq!(
            dense, direct,
            "absolute authoring radius samples must be invariant to intermediate rerenders"
        );
        let native_before = native_sketch_invariants(&fixture.coordinator, &fixture.points);
        let snapshot = fixture
            .coordinator
            .feature_authoring_snapshot()
            .expect("authoring snapshot");
        let mut authoring = FeatureAuthoringState::default();
        let candidate = feature_candidate(authoring.activate(
            &snapshot,
            fixture.coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Point(fixture.points[1]), None)],
        ));
        let metadata = fixture
            .coordinator
            .prepare_feature_authoring_preview(
                fixture.coordinator.feature_document().identity(),
                &candidate,
                "authoring rerender",
            )
            .expect("initial authoring preview");
        let owner = fixture
            .coordinator
            .feature_authoring_preview()
            .expect("held authoring preview")
            .corner_bindings()[0]
            .owner;
        let history_before = fixture.coordinator.history_len();
        let origin_scene = visible_computed_scene(&fixture.coordinator);
        let rail = origin_scene
            .fillet_affordances
            .iter()
            .find(|candidate| candidate.owner == owner)
            .expect("authoring Fillet affordances")
            .radius_rail;
        let initial_radius = candidate.radius();
        let sample_model = |delta: f64| {
            [
                delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
                delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
            ]
        };
        let pointer = |model_position| PointerInput {
            pointer_id: 902,
            position: origin_scene.viewport.model_to_screen(model_position),
            modifiers: Modifiers::default(),
        };
        let expected_design = fixture.coordinator.session().design_identity();
        let down = fixture
            .coordinator
            .transact_feature_authoring_pointer_down(
                &mut authoring,
                &origin_scene,
                pointer(rail.model_grip),
                Some(SelectionItem::FeatureCorner(owner)),
                PickTolerance::default(),
                "authoring radius press",
            )
            .expect("start authoring radius gesture");
        assert!(matches!(
            down,
            FeatureAuthoringPointerDownOutcome::RadiusGesture { .. }
        ));

        let first = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&origin_scene, pointer(sample_model(0.1)));
        assert!(matches!(
            first.as_slice(),
            [EditorEffect::PreviewComputedFeatureRadius { expected, radius, .. }]
                if *expected == metadata.input
                    && (*radius - (initial_radius + 0.1)).abs() <= 1.0e-10
        ));
        fixture
            .coordinator
            .apply_feature_authoring_editor_effect(&mut authoring, &first[0])
            .expect("accept first authoring Current sample");
        let first_scene = visible_computed_scene(&fixture.coordinator);
        assert_ne!(first_scene.computed_input, Some(metadata.input));

        let second = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&first_scene, pointer(sample_model(0.25)));
        let final_radius = match second.as_slice() {
            [
                EditorEffect::PreviewComputedFeatureRadius {
                    expected, radius, ..
                },
            ] => {
                assert_eq!(*expected, metadata.input);
                *radius
            }
            effects => panic!("authoring rerender stranded radius gesture: {effects:?}"),
        };
        assert!((final_radius - (initial_radius + 0.25)).abs() <= 1.0e-10);
        fixture
            .coordinator
            .apply_feature_authoring_editor_effect(&mut authoring, &second[0])
            .expect("accept second authoring Current sample");
        let final_scene = visible_computed_scene(&fixture.coordinator);
        let release = fixture.coordinator.editor_mut().pointer_up(
            &final_scene,
            expected_design,
            pointer(sample_model(0.25)),
        );
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitComputedFeatureRadius { expected, radius, .. },
                EditorEffect::ClearComputedFeaturePreview,
            ] if *expected == metadata.input && radius.to_bits() == final_radius.to_bits()
        ));
        for effect in &release {
            fixture
                .coordinator
                .apply_feature_authoring_editor_effect(&mut authoring, effect)
                .expect("finish authoring radius gesture");
        }
        assert!(fixture.coordinator.feature_document().features().is_empty());
        assert_eq!(fixture.coordinator.history_len(), history_before);
        let (final_metadata, final_candidate) = {
            let preview = fixture
                .coordinator
                .feature_authoring_preview()
                .expect("held final authoring preview");
            (preview.metadata().clone(), preview.candidate().clone())
        };
        assert_eq!(final_candidate.radius().to_bits(), final_radius.to_bits());
        assert!(matches!(
            computed_feature_state(
                fixture
                    .coordinator
                    .feature_authoring_preview()
                    .expect("Current final authoring preview")
                    .snapshot(),
                owner.feature,
            ),
            ComputedFeatureEvaluationState::Current { .. }
        ));

        let published = fixture
            .coordinator
            .apply_feature_authoring_preview(final_metadata.token, &final_candidate)
            .expect("publish exact final authoring preview")
            .value;
        assert_eq!(published, owner.feature);
        assert_eq!(fixture.coordinator.history_len(), history_before + 1);
        assert_eq!(
            fillet_radius(&fixture.coordinator, published).to_bits(),
            final_radius.to_bits()
        );
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn line_circle_contact_gesture_keeps_its_origin_across_current_branch_rerenders() {
        let mut document = SketchDocument::new(5.0).expect("line-circle document");
        let line_start = document.add_point("line start", [1.0, 2.5]).unwrap();
        let line_end = document.add_point("line end", [11.0, 2.5]).unwrap();
        let line = document
            .add_curve(
                "linear parent",
                CurveDefinition::Line {
                    start: line_start,
                    end: line_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let center = document.add_point("circle center", [6.0, 4.0]).unwrap();
        let source_radius = document
            .add_scalar(
                "source radius",
                1.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let circle = document
            .add_curve(
                "circular parent",
                CurveDefinition::Circle {
                    center,
                    radius: source_radius,
                },
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted line-circle session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let native_points = [line_start, line_end, center];
        let native_before = native_sketch_invariants(&coordinator, &native_points);
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("line-circle authoring snapshot");
        let line_span = CurveSpan::line(line);
        let circle_span = CurveSpan::line(circle);
        let mut authoring = FeatureAuthoringState::default();
        let candidate = feature_candidate(authoring.activate(
            &snapshot,
            coordinator.session().design_document(),
            FeatureAuthoringTool::Fillet,
            &[
                (SelectionItem::Curve(line_span), Some(0.4)),
                (SelectionItem::Curve(circle_span), Some(4.0)),
            ],
        ));
        let feature = apply_grouped_fillet(&mut coordinator, &candidate);
        let owner = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &coordinator
                .feature_document()
                .feature(feature)
                .expect("line-circle Fillet")
                .definition;
            ComputedCornerRef {
                feature,
                corner: fillet.corners[0].id,
            }
        };
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
        let history_before = coordinator.history_len();
        let origin_semantics = computed_feature_semantics(&coordinator, feature);
        let origin_scene = current_computed_scene(&coordinator);
        let origin_input = origin_scene
            .computed_input
            .expect("line-circle origin input");
        let handle = origin_scene
            .fillet_affordances
            .iter()
            .find(|candidate| candidate.owner == owner)
            .expect("line-circle affordances")
            .contacts
            .into_iter()
            .find(|candidate| candidate.source.span == circle_span)
            .expect("circular contact handle");
        let target = |parameter: f64| {
            let jet = snapshot
                .sketch_document()
                .evaluate_curve_jet(circle_span, parameter)
                .expect("circle target jet");
            origin_scene
                .viewport
                .model_to_screen([jet.position.x, jet.position.y])
        };
        let pointer = |position| PointerInput {
            pointer_id: 903,
            position,
            modifiers: Modifiers::default(),
        };
        let expected_design = coordinator.session().design_identity();

        let _ = coordinator
            .editor_mut()
            .pointer_down_feature_contact_handle(
                &origin_scene,
                pointer(handle.screen_position),
                handle,
            );
        let first = coordinator
            .editor_mut()
            .pointer_move(&origin_scene, pointer(target(5.3)));
        assert!(matches!(
            first.as_slice(),
            [EditorEffect::PreviewComputedFeatureContact {
                expected,
                owner: preview_owner,
                source,
                ..
            }] if *expected == origin_input
                && *preview_owner == owner
                && source.span == circle_span
        ));
        coordinator
            .apply_editor_effect(&first[0])
            .expect("accept first line-circle contact branch");
        let first_scene = visible_computed_scene(&coordinator);
        assert_ne!(first_scene.computed_input, Some(origin_input));

        let second = coordinator
            .editor_mut()
            .pointer_move(&first_scene, pointer(target(5.5)));
        let final_parameter = match second.as_slice() {
            [
                EditorEffect::PreviewComputedFeatureContact {
                    expected,
                    owner: preview_owner,
                    source,
                    parameter,
                    ..
                },
            ] => {
                assert_eq!(*expected, origin_input);
                assert_eq!(*preview_owner, owner);
                assert_eq!(source.span, circle_span);
                *parameter
            }
            effects => panic!("contact gesture was stranded after rerender: {effects:?}"),
        };
        coordinator
            .apply_editor_effect(&second[0])
            .expect("accept second line-circle contact sample");
        let final_scene = visible_computed_scene(&coordinator);
        let preview_revision = match coordinator.computed_scene_state() {
            ComputedSceneState::Current { snapshot, .. } => snapshot.evaluation_revision(),
            state => panic!("line-circle contact preview was not Current: {state:?}"),
        };
        let release = coordinator.editor_mut().pointer_up(
            &final_scene,
            expected_design,
            pointer(target(5.5)),
        );
        assert!(matches!(
            release.as_slice(),
            [
                EditorEffect::CommitComputedFeatureContact {
                    expected,
                    owner: committed,
                    parameter,
                    ..
                },
                EditorEffect::ClearComputedFeatureContactPreview,
            ] if *expected == origin_input
                && *committed == owner
                && parameter.to_bits() == final_parameter.to_bits()
        ));
        for effect in &release {
            coordinator
                .apply_editor_effect(effect)
                .expect("publish rerendered line-circle contact gesture");
        }
        assert_eq!(coordinator.history_len(), history_before + 1);
        assert_ne!(
            computed_feature_semantics(&coordinator, feature),
            origin_semantics
        );
        assert_eq!(
            coordinator
                .computed_snapshot()
                .expect("published line-circle contact output")
                .evaluation_revision(),
            preview_revision
        );
        assert_eq!(
            native_sketch_invariants(&coordinator, &native_points),
            native_before
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn published_radius_release_never_recreates_a_missing_or_replaced_current_preview() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let owner = {
            let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
                .coordinator
                .feature_document()
                .feature(feature)
                .expect("published Fillet")
                .definition;
            ComputedCornerRef {
                feature,
                corner: fillet.corners[0].id,
            }
        };
        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
        let scene = current_computed_scene(&fixture.coordinator);
        let origin_input = scene.computed_input.expect("origin input");
        let rail = scene
            .fillet_affordances
            .iter()
            .find(|candidate| candidate.owner == owner)
            .expect("Fillet affordances")
            .radius_rail;
        let origin_radius = fillet_radius(&fixture.coordinator, feature);
        let sample_model = |delta: f64| {
            [
                delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
                delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
            ]
        };
        let pointer = |pointer_id, delta| PointerInput {
            pointer_id,
            position: scene.viewport.model_to_screen(sample_model(delta)),
            modifiers: Modifiers::default(),
        };
        let expected_design = fixture.coordinator.session().design_identity();
        let feature_json_before = fixture
            .coordinator
            .feature_document()
            .to_json()
            .expect("origin feature JSON");
        let history_before = fixture.coordinator.history_len();
        let native_before = native_sketch_invariants(&fixture.coordinator, &fixture.points);

        for (case, replace_with) in [None, Some(origin_radius + 0.3)].into_iter().enumerate() {
            let pointer_id = 910 + u64::try_from(case).expect("small case index");
            let _ = fixture.coordinator.pointer_down(
                &scene,
                PointerInput {
                    pointer_id,
                    position: rail.screen_grip,
                    modifiers: Modifiers::default(),
                },
            );
            let preview = fixture
                .coordinator
                .editor_mut()
                .pointer_move(&scene, pointer(pointer_id, 0.2));
            let acknowledged_radius = match preview.as_slice() {
                [EditorEffect::PreviewComputedFeatureRadius { radius, .. }] => *radius,
                effects => panic!("expected acknowledged radius request, got {effects:?}"),
            };
            fixture
                .coordinator
                .apply_editor_effect(&preview[0])
                .expect("acknowledge exact Current preview");
            if let Some(replacement) = replace_with {
                fixture
                    .coordinator
                    .preview_computed_fillet_radius_exact(origin_input, feature, replacement)
                    .expect("replace held preview without acknowledging it");
            } else {
                fixture.coordinator.clear_computed_feature_preview();
            }
            let release = fixture.coordinator.editor_mut().pointer_up(
                &scene,
                expected_design,
                pointer(pointer_id, 0.2),
            );
            assert!(matches!(
                release.as_slice(),
                [
                    EditorEffect::CommitComputedFeatureRadius { radius, .. },
                    EditorEffect::ClearComputedFeaturePreview,
                ] if radius.to_bits() == acknowledged_radius.to_bits()
            ));
            assert!(matches!(
                fixture.coordinator.apply_editor_effect(&release[0]),
                Err(CoordinatorError::FeatureAuthoringPreviewMismatch)
            ));
            fixture
                .coordinator
                .apply_editor_effect(&release[1])
                .expect("clear rejected release preview");
            assert_eq!(
                fixture
                    .coordinator
                    .feature_document()
                    .to_json()
                    .expect("unchanged feature JSON"),
                feature_json_before
            );
            assert_eq!(fixture.coordinator.history_len(), history_before);
            assert_eq!(
                fillet_radius(&fixture.coordinator, feature).to_bits(),
                origin_radius.to_bits()
            );
            assert_eq!(
                native_sketch_invariants(&fixture.coordinator, &fixture.points),
                native_before
            );
        }
    }

    #[test]
    fn computed_fillet_affordances_are_selection_gated_and_keep_shared_ownership() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&fixture.coordinator, fixture.points[1..=2].iter().copied());
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("published grouped Fillet")
            .definition;
        let mut owners = fillet
            .corners
            .iter()
            .map(|corner| ComputedCornerRef {
                feature,
                corner: corner.id,
            })
            .collect::<Vec<_>>();
        owners.sort_unstable();

        fixture
            .coordinator
            .editor_mut()
            .set_selection(std::iter::empty());
        assert!(
            current_computed_scene(&fixture.coordinator)
                .fillet_affordances
                .is_empty(),
            "ordinary unselected computed geometry must not advertise edit handles"
        );

        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owners[0])]);
        let corner_scene = current_computed_scene(&fixture.coordinator);
        let [corner_affordances] = corner_scene.fillet_affordances.as_slice() else {
            panic!("one selected corner must own exactly one affordance bundle");
        };
        assert_eq!(corner_affordances.owner, owners[0]);
        assert_eq!(corner_affordances.affected_owners, owners);
        assert_eq!(
            corner_affordances.contacts.map(|contact| contact.owner),
            [owners[0]; 2]
        );
        assert!(!corner_affordances.actions.is_empty());
        assert!(corner_affordances.actions.iter().all(|action| {
            action.owner == owners[0]
                && !action.label.trim().is_empty()
                && corner_scene
                    .fillet_action_target(owners[0], action.id)
                    .is_some_and(|target| {
                        target.expected == corner_scene.computed_input.expect("computed input")
                            && target.owner == owners[0]
                    })
        }));
        let stable_action_ids = corner_affordances
            .actions
            .iter()
            .map(|action| action.id)
            .collect::<Vec<_>>();
        assert_eq!(
            current_computed_scene(&fixture.coordinator).fillet_affordances[0]
                .actions
                .iter()
                .map(|action| action.id)
                .collect::<Vec<_>>(),
            stable_action_ids,
            "an unchanged scene must retain exact semantic action identities"
        );

        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(feature)]);
        let feature_scene = current_computed_scene(&fixture.coordinator);
        assert_eq!(
            feature_scene
                .fillet_affordances
                .iter()
                .map(|affordances| affordances.owner)
                .collect::<Vec<_>>(),
            owners
        );
        assert!(
            feature_scene
                .fillet_affordances
                .iter()
                .all(|affordances| affordances.affected_owners == owners),
            "every shared-radius rail must disclose every corner it changes"
        );
    }

    #[test]
    fn grouped_fillet_omits_retained_direction_actions_rejected_by_whole_composition() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate =
            grouped_fillet_candidate(&fixture.coordinator, fixture.points[1..=2].iter().copied());
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("published grouped Fillet")
            .definition;
        let owners = fillet
            .corners
            .iter()
            .map(|corner| ComputedCornerRef {
                feature,
                corner: corner.id,
            })
            .collect::<Vec<_>>();
        assert_eq!(owners.len(), 2);

        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(feature)]);
        let scene = current_computed_scene(&fixture.coordinator);
        let first_actions = &scene
            .fillet_affordances
            .iter()
            .find(|affordances| affordances.owner == owners[0])
            .expect("first corner affordances")
            .actions;
        let second_actions = &scene
            .fillet_affordances
            .iter()
            .find(|affordances| affordances.owner == owners[1])
            .expect("second corner affordances")
            .actions;

        assert!(first_actions.iter().any(|action| {
            action.id == SceneFilletActionId::ReverseFirstRetainedDirection
                && matches!(
                    action.availability,
                    SceneFilletActionAvailability::Applicable
                )
        }));
        assert!(
            !first_actions
                .iter()
                .any(|action| { action.id == SceneFilletActionId::ReverseSecondRetainedDirection })
        );
        assert!(
            !second_actions
                .iter()
                .any(|action| { action.id == SceneFilletActionId::ReverseFirstRetainedDirection })
        );
        assert!(second_actions.iter().any(|action| {
            action.id == SceneFilletActionId::ReverseSecondRetainedDirection
                && matches!(
                    action.availability,
                    SceneFilletActionAvailability::Applicable
                )
        }));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn rejected_radius_sample_exposes_typed_limit_and_retains_last_current_scene() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("published Fillet")
            .definition;
        let owner = ComputedCornerRef {
            feature,
            corner: fillet.corners[0].id,
        };
        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
        let scene = current_computed_scene(&fixture.coordinator);
        let origin_input = scene.computed_input.expect("origin computed input");
        let rail = scene.fillet_affordances[0].radius_rail;
        let origin_radius = fillet_radius(&fixture.coordinator, feature);
        let rejected_radius = 5.0;
        let rejected_delta = rejected_radius - origin_radius;
        let rejected_model = [
            rejected_delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
            rejected_delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
        ];
        let pointer = |model_position| PointerInput {
            pointer_id: 920,
            position: scene.viewport.model_to_screen(model_position),
            modifiers: Modifiers::default(),
        };
        let native_before = native_sketch_invariants(&fixture.coordinator, &fixture.points);
        let feature_before = fixture
            .coordinator
            .feature_document()
            .to_json()
            .expect("origin feature JSON");
        let history_before = fixture.coordinator.history_len();

        let _ = fixture
            .coordinator
            .pointer_down(&scene, pointer(rail.model_grip));
        let request = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(rejected_model));
        let requested_radius = match request.as_slice() {
            [
                EditorEffect::PreviewComputedFeatureRadius {
                    expected,
                    feature: requested_feature,
                    radius,
                },
            ] => {
                assert_eq!(*expected, origin_input);
                assert_eq!(*requested_feature, feature);
                assert!((*radius - rejected_radius).abs() <= 1.0e-10);
                *radius
            }
            effects => panic!("expected one rejected radius request, got {effects:?}"),
        };
        assert!(
            fixture
                .coordinator
                .apply_editor_effect(&request[0])
                .is_err()
        );
        let status = fixture
            .coordinator
            .editor()
            .computed_fillet_continuation_status()
            .expect("typed rejected-sample status")
            .clone();
        assert_eq!(status.expected, origin_input);
        assert_eq!(status.owner, owner);
        assert!(matches!(
            status.sample,
            ComputedFilletInteractionSample::Radius(radius)
                if radius.to_bits() == requested_radius.to_bits()
        ));
        assert_eq!(
            status.limit.kind,
            ComputedFilletContinuationLimitKind::BranchFold
        );
        assert!(!status.limit.message.trim().is_empty());

        let retained_scene = visible_computed_scene(&fixture.coordinator);
        assert_eq!(retained_scene.computed_input, Some(origin_input));
        assert_eq!(
            retained_scene.computed_fillet_continuation_statuses,
            vec![status.clone()]
        );
        assert_eq!(
            retained_scene.fillet_affordances[0].continuation_status,
            Some(status)
        );
        assert_eq!(
            fixture
                .coordinator
                .feature_document()
                .to_json()
                .expect("retained feature JSON"),
            feature_before
        );
        assert_eq!(fixture.coordinator.history_len(), history_before);
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );

        let recovered_delta = 0.2_f64;
        let recovered_model = [
            recovered_delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
            recovered_delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
        ];
        let recovered = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&retained_scene, pointer(recovered_model));
        assert!(matches!(
            recovered.as_slice(),
            [EditorEffect::PreviewComputedFeatureRadius { .. }]
        ));
        fixture
            .coordinator
            .apply_editor_effect(&recovered[0])
            .expect("recover with a Current radius sample");
        assert!(
            fixture
                .coordinator
                .editor()
                .computed_fillet_continuation_status()
                .is_none()
        );
        assert!(
            visible_computed_scene(&fixture.coordinator)
                .computed_fillet_continuation_statuses
                .is_empty()
        );
        for effect in fixture.coordinator.editor_mut().cancel() {
            fixture
                .coordinator
                .apply_editor_effect(&effect)
                .expect("cancel recovered preview");
        }
        assert_eq!(fixture.coordinator.history_len(), history_before);
        assert_eq!(
            fixture
                .coordinator
                .feature_document()
                .to_json()
                .expect("cancelled feature JSON"),
            feature_before
        );
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn computed_fillet_action_effect_is_exact_atomic_and_sketch_invariant() {
        let mut fixture = computed_fillet_editor_fixture();
        let candidate = grouped_fillet_candidate(&fixture.coordinator, [fixture.points[1]]);
        let feature = apply_grouped_fillet(&mut fixture.coordinator, &candidate);
        let ComputedFeatureDefinition::FilletSet(fillet) = &fixture
            .coordinator
            .feature_document()
            .feature(feature)
            .expect("published Fillet")
            .definition;
        let owner = ComputedCornerRef {
            feature,
            corner: fillet.corners[0].id,
        };
        fixture
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::FeatureCorner(owner)]);
        let scene = current_computed_scene(&fixture.coordinator);
        let action = scene.fillet_affordances[0]
            .actions
            .iter()
            .find(|action| {
                matches!(
                    action.availability,
                    SceneFilletActionAvailability::Applicable
                )
            })
            .expect("at least one independently current branch action");
        let target = scene
            .fillet_action_target(owner, action.id)
            .expect("exact scene-stamped target");
        let history_before = fixture.coordinator.history_len();
        let native_before = native_sketch_invariants(&fixture.coordinator, &fixture.points);
        let feature_before = computed_feature_semantics(&fixture.coordinator, feature);
        let feature_json_before = fixture
            .coordinator
            .feature_document()
            .to_json()
            .expect("origin feature JSON");

        let preview_effects = fixture
            .coordinator
            .editor_mut()
            .preview_fillet_action(&scene, crate::SceneFilletActionInput::Accessible(target));
        assert_eq!(
            preview_effects,
            vec![EditorEffect::FilletBranchPreviewChanged {
                target: Some(target)
            }]
        );
        for effect in &preview_effects {
            assert!(
                fixture
                    .coordinator
                    .apply_editor_effect(effect)
                    .expect("branch preview is coordinator-neutral")
                    .is_none()
            );
        }
        assert_eq!(fixture.coordinator.history_len(), history_before);
        assert_eq!(
            fixture
                .coordinator
                .feature_document()
                .to_json()
                .expect("preview feature JSON"),
            feature_json_before
        );

        let mut stale = target;
        stale.expected.policy.max_root_iterations += 1;
        assert!(matches!(
            fixture
                .coordinator
                .apply_editor_effect(&EditorEffect::CommitComputedFilletAction { target: stale }),
            Err(CoordinatorError::StaleComputedFeatureCandidate)
        ));
        assert_eq!(fixture.coordinator.history_len(), history_before);
        assert_eq!(
            fixture
                .coordinator
                .feature_document()
                .to_json()
                .expect("stale target retained feature JSON"),
            feature_json_before
        );

        let activation = fixture
            .coordinator
            .editor_mut()
            .activate_fillet_action(&scene, crate::SceneFilletActionInput::Accessible(target));
        assert!(matches!(
            activation.as_slice(),
            [
                EditorEffect::CommitComputedFilletAction { target: committed },
                EditorEffect::FilletBranchPreviewChanged { target: None },
            ] if *committed == target
        ));
        for effect in &activation {
            assert!(
                fixture
                    .coordinator
                    .apply_editor_effect(effect)
                    .expect("exact branch activation")
                    .is_none()
            );
        }
        assert_eq!(fixture.coordinator.history_len(), history_before + 1);
        let feature_after = computed_feature_semantics(&fixture.coordinator, feature);
        assert_ne!(feature_after, feature_before);
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );

        assert!(matches!(
            fixture
                .coordinator
                .apply_editor_effect(&EditorEffect::CommitComputedFilletAction { target }),
            Err(CoordinatorError::StaleComputedFeatureCandidate)
        ));
        assert_eq!(fixture.coordinator.history_len(), history_before + 1);
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_after
        );

        fixture.coordinator.undo().expect("Undo branch action");
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_before
        );
        fixture.coordinator.redo().expect("Redo branch action");
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_after
        );
        let saved = fixture.coordinator.checkpoint().clone();
        fixture.coordinator.undo().expect("Undo before reload");
        fixture
            .coordinator
            .reload(&saved)
            .expect("reload branch action");
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_after
        );
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );

        let history_after_reload = fixture.coordinator.history_len();
        let current_scene = current_computed_scene(&fixture.coordinator);
        let current_target = current_scene.fillet_affordances[0]
            .actions
            .iter()
            .find(|action| {
                matches!(
                    action.availability,
                    SceneFilletActionAvailability::Applicable
                )
            })
            .and_then(|action| current_scene.fillet_action_target(owner, action.id))
            .expect("current target before selection removal");
        fixture
            .coordinator
            .editor_mut()
            .set_selection(std::iter::empty());
        assert!(matches!(
            fixture
                .coordinator
                .apply_editor_effect(&EditorEffect::CommitComputedFilletAction {
                    target: current_target
                }),
            Err(CoordinatorError::ComputedFilletActionUnavailable(_))
        ));
        assert_eq!(fixture.coordinator.history_len(), history_after_reload);
        assert_eq!(
            computed_feature_semantics(&fixture.coordinator, feature),
            feature_after
        );
        assert_eq!(
            native_sketch_invariants(&fixture.coordinator, &fixture.points),
            native_before
        );
    }
}
