//! Persistent 2D CAD sketches over the validated `GeoSolve` numerical kernel.
//!
//! [`SketchDocument`] stores stable external IDs, editable analytic and parametric
//! curves, constraints, dimensions, explicit branch state and canonical JSON.
//! [`RetainedSketchDocumentSession`] is the production lifecycle entry point for
//! separate retained design intent, non-authoritative attempts and independently
//! accepted solved state. [`SketchDocumentSession`] remains the accepted-only
//! command/history workflow used by embedders, sample builders and compatibility clients.
//!
//! The supported curve surface includes lines, circles/arcs, conics, Beziers,
//! clamped/periodic B-splines and NURBS. Generic contact, tangency, curvature,
//! continuity, associative constructions, fillets and persistent visible trim views
//! compile through shared curve jets. Span, side, winding, neighborhood and sweep
//! choices remain explicit state outside differentiation.
//!
//! See the `persistent_sketch` example for construct, solve, edit and JSON restore:
//!
//! ```text
//! cargo run --locked -p geosolve-sketch --example persistent_sketch
//! ```
//!
//! Direct [`Sketch`] and compiler/mapping exports remain advanced diagnostic and
//! compatibility APIs during the `0.1` preview. Runtime IDs must not be persisted;
//! use document IDs and document/source audit mappings for application identity.

mod alpha_scenarios;
mod attributes;
mod beziers;
mod bsplines;
mod characterization;
mod compiler;
mod conics;
mod curves;
mod diagnostics;
mod document;
mod document_lowering;
mod document_session;
mod generic_curves;
mod m38;
mod model;
mod nurbs;
mod offsets;
mod profile_offset_construction;
mod profiles;
mod residuals;
mod scenarios;
mod semantic;
mod session;

pub use alpha_scenarios::{
    A1ScenarioIds, A2ScenarioIds, A3ScenarioIds, A4ScenarioIds, A5ScenarioIds, A8ScenarioIds,
    AlphaPerformanceSize, AlphaProfileScenarioUat, AlphaScenarioFixture, AlphaScenarioIds,
    AlphaScenarioKind, AlphaScenarioUat, ConicCircleLimitIds, ConicGalleryIds, ConicTangencyIds,
    DiagnosticEndpointBoundIds, DiagnosticRankDropIds, DiagnosticRedundancyIds, DirectedAngleIds,
    EntityMirrorIds, ExactTranslatedOffsetIds, GenericFilletLabIds, M27ReferenceFilletIds,
    M28TrimmedFilletIds, MotionCamIds, MotionDrawingArmIds, MotionFourBarCouplerIds,
    MotionOrbitIds, MotionPantographIds, MotionPeaucellierIds, MotionRotatingSquareIds,
    MotionScissorIds, MotionScissorTowerIds, MotionScotchYokeIds, MotionTrammelIds,
    NurbsDifferentialIds, NurbsLabIds, ProfileScenarioIds, StressBridgeIds, StressCompassIds,
    SupportingOffsetIds, alpha_performance_document, alpha_scenario,
};
pub use attributes::{SketchAttributeError, SketchAttributes};
pub use beziers::{BezierCurve, BezierEvaluationError, BezierKind};
pub use bsplines::BSplineCurve;
pub use characterization::{
    CurrentDocumentCommandKind, CurrentDocumentEffectKind, CurrentMeasurementKind,
};
pub use compiler::{
    ArcAngleRole, ArcAngleVariableMapping, ArcRadiusVariableMapping, CircleRadiusVariableMapping,
    CompiledSketch, ConicScalarVariableMapping, ConicVectorRole, ConicVectorVariableMapping,
    DragTarget, LatentVariableMapping, LatentVariableRole, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
    MIN_REPRESENTABLE_RADIUS, NurbsWeightVariableMapping, PointVariableMapping,
    ReferenceDimensionValue, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, SketchAcceptedRedundancy,
    SketchBound, SketchBoundMapping, SketchGeometry, SketchSolveRequest, SketchSolveResult,
    SketchSolveWorkSummary, SketchSource, SketchSourceMapping, SolveRejection, SolvedArc,
    SolvedCircle, SolvedConic, SolvedConicKind, SolvedNurbs, SolvedPoint,
};
pub use conics::{ConicCurve, ConicGeometry, ConicKind};
pub use curves::{
    AngleOrientation, ArcCircleTangencySide, ArcSweep, CENTER_DIRECTION_COSINE_MARGIN,
    CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE, CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE,
    CIRCLE_ARC_TANGENCY_SCALE_UNCERTAINTY_MULTIPLIER, CONTACT_PARAMETER_ROUNDOFF_TOLERANCE,
    CenterDirectionBranch, Circle, CircleContainment, CircleTangencyMode, CircularArc,
    ContactState, LineOffsetOrientation, LineParameterDomain, LineSide,
};
pub use diagnostics::{
    SketchBoundDiagnostic, SketchBoundStatus, SketchComponentDiagnostic,
    SketchDependencyDiagnostic, SketchDiagnosticBudget, SketchDiagnosticComponentIdentity,
    SketchDiagnosticIncompleteReason, SketchDiagnosticInputIdentity, SketchDiagnosticProvenance,
    SketchDiagnosticSearch, SketchDiagnosticSearchStatus, SketchDiagnosticSnapshot,
    SketchDiagnosticWork, SketchExternalReferenceDiagnostic, SketchExternalReferenceState,
    SketchHardValidity, SketchMobilityDiagnostic, SketchOneSidedMobility,
    SketchParameterDiagnostic, SketchParameterInputIssue, SketchParameterState,
    SketchRankDiagnostic, SketchRepairSuggestion, SketchSolveDiagnostic, SketchSolveTermination,
    SketchSolverPolicyDigest, SketchSourceDiagnostic, SketchStructuralClassification,
};
pub use document::{
    ActivationDigest, ContactBranchEdit, ContactDefinition, ContactDomain, ContactId,
    ContactNeighborhood, ContactSlot, ContactStateEdit, CurveCurveFilletIds,
    CurveCurveFilletRequest, CurveDefinition, CurveFilletParentRequest, CurveId, CurveSpan,
    DesignCurve, DesignPoint, DesignPointId, DesignScalar, DesignScalarId,
    DocumentAngleOrientation, DocumentArcSweep, DocumentArcTangencySide, DocumentBSplineForm,
    DocumentBSplineInsertion, DocumentBSplineSpanDirection, DocumentCenterRef,
    DocumentCircleContainment, DocumentCircleTangencyMode, DocumentConicFeature,
    DocumentConicMeasurement, DocumentConicQueryError, DocumentConstraint,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentControlRef, DocumentCoordinateAxis,
    DocumentCurveContinuity, DocumentCurveControl, DocumentCurveControlAvailability,
    DocumentCurveControlError, DocumentCurveControlId, DocumentCurveControlKind,
    DocumentCurveControlProjection, DocumentCurveControlTarget,
    DocumentCurveControlWithholdingReason, DocumentCurveCurvatureRelation,
    DocumentCurveDirectionRelation, DocumentCurveEvaluationError, DocumentCurveMeasurementError,
    DocumentCurveMeasurementKind, DocumentCurveNormalSide, DocumentCurveSpanRef,
    DocumentCurveTrimView, DocumentDimension, DocumentDimensionDefinition, DocumentDimensionId,
    DocumentDimensionMode, DocumentDirectedProfileOffsetCurve, DocumentDirectionRef,
    DocumentDirectionSense, DocumentElementActivity, DocumentElementId, DocumentEndpointRef,
    DocumentError, DocumentExternalBinding, DocumentExternalBindingId,
    DocumentExternalLineSupportRef, DocumentExternalPointRef, DocumentFaceOffsetDirection,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentHyperbolaBranch, DocumentId,
    DocumentLineOffsetOrientation, DocumentLineSide, DocumentLineSupportRef,
    DocumentMirroredBSplineInsertion, DocumentNurbsInsertion, DocumentObjectId,
    DocumentOffsetTraversal, DocumentParameter, DocumentParameterBinding, DocumentParameterId,
    DocumentParameterKind, DocumentParameterOutput, DocumentParameterTarget, DocumentPointRef,
    DocumentProfileOffsetChain, DocumentProfileOffsetEdgePair, DocumentProfileOffsetIds,
    DocumentProfileOffsetJunction, DocumentProfileOffsetJunctionBranch,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetLoop, DocumentProfileOffsetOperand,
    DocumentProfileOffsetTerminalPolicy, DocumentProfileOffsetTurn, DocumentRationalConicControl,
    DocumentRationalConicControlMode, DocumentSourceId, DocumentSourceOwner, DocumentSourceRef,
    DocumentTrimBoundary, DocumentTrimParameter, DocumentTrimProjection,
    DocumentTrimProjectionError, DocumentVisibleCurveInterval, EffectiveActivity,
    ExternalFeatureKindV1, ExternalTopologyDigest, FeatureEndpoint, FeatureRef, GeometryRole,
    GeometryRoleEdit, HostActivationOverride, HostConfigurationActivation, InactivityReason,
    LineLineFilletIds, LineLineFilletRequest, MAX_BSPLINE_CONTROLS, MAX_DOCUMENT_JSON_BYTES,
    MAX_DOCUMENT_OBJECTS, MAX_DOCUMENT_PARAMETERS, MAX_EXTERNAL_BINDINGS, MAX_LABEL_BYTES,
    MAX_PERSISTENT_SPLINE_SPAN_CURSORS, MAX_POLYLINE_POINTS, MirroredCurveIds, PersistentId,
    RectangleIds, SKETCH_DOCUMENT_VERSION, ScalarDomain, ScalarUnit, SketchDatum, SketchDocument,
    SketchPersistentIdentityHighWater, TangentOrientation,
};
pub use document_lowering::{
    ContactRuntimeMapping, CurveRuntimeMapping, DocumentContactRole,
    DocumentParameterRuntimeBinding, DocumentRuntimeMap, DocumentSourceRuntimeMapping,
    LoweredDocument, PointRuntimeMapping, RuntimeCurve, RuntimeSource,
};
pub use document_session::{
    DocumentCommand, DocumentCommandEffect, DocumentCommandOutcome, DocumentDragLocalityPlan,
    DocumentDragTarget, DocumentEdit, DocumentParameterOutputProposal, DocumentSessionError,
    DocumentSolveRequest, DocumentSolveResult, DocumentTransactionOutcome,
    EXTERNAL_SNAPSHOT_SET_VERSION_V1, ExternalLineOrientationV1, ExternalSnapshotDigest,
    ExternalSnapshotEntry, ExternalSnapshotFeatureV1, ExternalSnapshotInputError,
    ExternalSnapshotResourcesV1, ExternalSnapshotSet, ExternalSnapshotSetDigest,
    ExternalSnapshotSetV1, MAX_EXTERNAL_SNAPSHOT_CONTROLS, MAX_EXTERNAL_SNAPSHOT_ENTRIES,
    MAX_EXTERNAL_SNAPSHOT_POINTS, MAX_EXTERNAL_SNAPSHOT_SPANS, MAX_PARAMETER_BATCH_ENTRIES,
    ParameterBatch, ParameterBatchEntry, ParameterDigest, ParameterValue, PreparedSketchCommit,
    PreparedSketchInput, PreparedSketchJob, PreparedSketchOperation, PreparedSketchOperationKind,
    PreparedSketchPatch, PreparedSketchPreview, PreparedSketchSnapshot,
    RetainedDocumentTransactionOutcome, RetainedSketchDocumentSession,
    SketchAcceptedDocumentRedundancy, SketchAcceptedDocumentState, SketchAcceptedRevision,
    SketchAcceptedStateIdentity, SketchAttemptFailure, SketchAttemptFailureKind,
    SketchAttemptIdentity, SketchAttemptInput, SketchAttemptRevision, SketchDesignIdentity,
    SketchDesignRevision, SketchDocumentAttempt, SketchDocumentSession,
    SketchLifecycleRevisionHighWater,
};
pub use geosolve_core::{
    CancellationHandle, CancellationToken, OperationCheckpoint, OperationControl,
    OperationController, OperationLimits, OperationOutcome, OperationReport, OperationStopReason,
    OperationWork, OperationWorkCounter, SolverConfig, cancellation_pair,
};
pub use geosolve_geometry::{
    BSplineBasis, BSplineContinuity, BSplineDefinitionError, BSplineEvaluationError, BSplineForm,
    BSplineInsertionError, BSplineKnotSide, BSplineSpanIndex, ConicDefinitionError,
    ConicEvaluationError, CurveDifferential2, CurveDifferentialError, DirectedParameterTrim,
    EllipseAxisObservability, HyperbolaBranch, MAX_BSPLINE_DEGREE, NurbsDefinitionError,
    NurbsEvaluationError, NurbsInsertionError, ProperConicKind,
};
pub use m38::{
    DocumentBoundedCurveInterval, DocumentDatumAxis, DocumentM38DimensionDefinition,
    DocumentM38SolveResult, DocumentMeasurementAudit, DocumentMeasurementCatalog,
    DocumentMeasurementDefinition, DocumentMeasurementProvenance, DocumentMeasurementValue,
    DocumentMeasurementWork,
};
pub use model::{
    ArcAngleEndpoint, ArcId, BSplineId, BezierId, CircleId, ConicId, ConicScalarRole,
    CoordinateAxis, CurveContactNeighborhood, CurveContinuity, CurveCurvatureRelation,
    CurveDirectionRelation, CurveMeasurementKind, CurveNormalSide, CurveTangentOrientation,
    DimensionKind, DimensionMode, ExternalConstraintProvenance, FilletEndpointOrder, LineSegment,
    M38ConicProperty, NurbsId, PointId, ProfileOffsetId, SegmentBranch, SegmentEndpoint, SegmentId,
    Sketch, SketchConstraint, SketchConstraintId, SketchConstraintKind, SketchCurve,
    SketchCurveContact, SketchDimension, SketchDimensionId, SketchError, SketchPoint,
    SketchScalarRef,
};
pub use nurbs::NurbsCurve;
pub use offsets::{
    DirectedProfileOffsetCurve, FaceOffsetDirection, OffsetTraversal, ProfileOffsetAssociation,
    ProfileOffsetChain, ProfileOffsetCurve, ProfileOffsetEdgePair, ProfileOffsetJunctionBranch,
    ProfileOffsetLoop, ProfileOffsetOperand, ProfileOffsetTerminalPolicy, ProfileOffsetTurn,
};
pub use profile_offset_construction::{
    DocumentPreparedProfileOffsetGeometry, DocumentProfileOffsetCreationJunction,
    DocumentProfileOffsetCreationOperand, DocumentProfileOffsetCreationPath,
    DocumentProfileOffsetCreationRequest,
};
pub use profiles::{
    LineCurveFilletBranchCellError, VisualProfileAnalysis, VisualProfileBudgetCounter,
    VisualProfileBudgetReport, VisualProfileContour, VisualProfileCurveFamily, VisualProfileEdge,
    VisualProfileFace, VisualProfileGeometryScope, VisualProfileIntersection, VisualProfileIssue,
    VisualProfileIssueKind, VisualProfileOptions, VisualProfileOrientation, VisualProfileStatus,
};
pub use scenarios::{
    ConflictingRectangleIds, TangentCirclesIds, UnderconstrainedTriangleIds, conflicting_rectangle,
    redundant_rectangle, tangent_circles, underconstrained_triangle,
};
pub use semantic::{
    DocumentAngleOperand, DocumentCircleArcTangentRequest, DocumentConstructedRelation,
    DocumentContactSeed, DocumentLineCircleTangentRequest, DocumentPlanarAudit,
    DocumentPlanarRelation, DocumentPlanarSource, DocumentScalarAudit, DocumentScalarAuditBinding,
    DocumentScalarBranch, DocumentScalarPropertyRef, DocumentScalarRelation, DocumentScalarRow,
    DocumentScalarSource, DocumentScalarUnit, DocumentSemanticCatalogSession,
    DocumentSemanticSolveResult, DocumentSemanticSourceCatalog, DocumentSignedLengthProvenance,
    LoweredDocumentScalarSource,
};
pub use session::{
    SketchPatch, SketchProductionScaleAssessment, SketchRankAuthority, SketchSession,
    SketchSessionError, SketchSessionExecutionKind, SketchSessionExecutionSummary,
    SketchSessionPatch, SketchSessionRevisions,
};
