//! Persistent 2D CAD sketches over the validated `GeoSolve` numerical kernel.
//!
//! [`SketchDocument`] stores stable external IDs, editable analytic and parametric
//! curves, constraints, dimensions, explicit branch state and canonical JSON.
//! [`SketchDocumentSession`] is the recommended application entry point: commands
//! clone, solve, independently validate and atomically publish one accepted document,
//! while rejection retains the previous finite geometry and history.
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
mod compiler;
mod conics;
mod curves;
mod document;
mod document_lowering;
mod document_session;
mod generic_curves;
mod model;
mod nurbs;
mod profiles;
mod residuals;
mod scenarios;
mod session;

pub use alpha_scenarios::{
    A1ScenarioIds, A2ScenarioIds, A3ScenarioIds, A4ScenarioIds, A5ScenarioIds, A8ScenarioIds,
    AlphaPerformanceSize, AlphaProfileScenarioUat, AlphaScenarioFixture, AlphaScenarioIds,
    AlphaScenarioKind, AlphaScenarioUat, ConicCircleLimitIds, ConicGalleryIds, ConicTangencyIds,
    DiagnosticEndpointBoundIds, DiagnosticRankDropIds, DiagnosticRedundancyIds, DirectedAngleIds,
    EntityMirrorIds, ExactTranslatedOffsetIds, GenericFilletLabIds, M27ReferenceFilletIds,
    M28TrimmedFilletIds, MotionCamIds, MotionOrbitIds, MotionPeaucellierIds,
    MotionRotatingSquareIds, MotionScissorIds, MotionScissorTowerIds, MotionScotchYokeIds,
    MotionTrammelIds, NurbsDifferentialIds, NurbsLabIds, ProfileScenarioIds, StressBridgeIds,
    StressCompassIds, SupportingOffsetIds, alpha_performance_document, alpha_scenario,
};
pub use attributes::{SketchAttributeError, SketchAttributes};
pub use beziers::{BezierCurve, BezierEvaluationError, BezierKind};
pub use bsplines::BSplineCurve;
pub use compiler::{
    ArcAngleRole, ArcAngleVariableMapping, ArcRadiusVariableMapping, CircleRadiusVariableMapping,
    CompiledSketch, ConicScalarRole, ConicScalarVariableMapping, ConicVectorRole,
    ConicVectorVariableMapping, DragTarget, LatentVariableMapping, LatentVariableRole,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, MIN_REPRESENTABLE_RADIUS, NurbsWeightVariableMapping,
    PointVariableMapping, ReferenceDimensionValue, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE,
    SketchBound, SketchBoundMapping, SketchGeometry, SketchSolveRequest, SketchSolveResult,
    SketchSource, SketchSourceMapping, SolveRejection, SolvedArc, SolvedCircle, SolvedConic,
    SolvedConicKind, SolvedNurbs, SolvedPoint,
};
pub use conics::{ConicCurve, ConicGeometry, ConicKind};
pub use curves::{
    AngleOrientation, ArcCircleTangencySide, ArcSweep, CENTER_DIRECTION_COSINE_MARGIN,
    CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE, CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE,
    CIRCLE_ARC_TANGENCY_SCALE_UNCERTAINTY_MULTIPLIER, CONTACT_PARAMETER_ROUNDOFF_TOLERANCE,
    CenterDirectionBranch, Circle, CircleContainment, CircleTangencyMode, CircularArc,
    ContactState, LineOffsetOrientation, LineParameterDomain, LineSide,
};
pub use document::{
    ContactDefinition, ContactDomain, ContactId, ContactNeighborhood, ContactSlot,
    ContactStateEdit, CurveCurveFilletIds, CurveCurveFilletRequest, CurveDefinition,
    CurveFilletParentRequest, CurveId, CurveSpan, DesignCurve, DesignPoint, DesignPointId,
    DesignScalar, DesignScalarId, DocumentAngleOrientation, DocumentArcSweep,
    DocumentArcTangencySide, DocumentBSplineForm, DocumentBSplineInsertion,
    DocumentBSplineSpanDirection, DocumentCircleContainment, DocumentCircleTangencyMode,
    DocumentConicFeature, DocumentConicMeasurement, DocumentConicQueryError, DocumentConstraint,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
    DocumentCurveEvaluationError, DocumentCurveMeasurementError, DocumentCurveMeasurementKind,
    DocumentCurveNormalSide, DocumentCurveTrimView, DocumentDimension, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentDimensionMode, DocumentElementId, DocumentError,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentHyperbolaBranch, DocumentId,
    DocumentLineOffsetOrientation, DocumentLineSide, DocumentMirroredBSplineInsertion,
    DocumentNurbsInsertion, DocumentObjectId, DocumentSourceId, DocumentSourceOwner,
    DocumentSourceRef, DocumentTrimBoundary, DocumentTrimParameter, DocumentTrimProjection,
    DocumentTrimProjectionError, DocumentVisibleCurveInterval, FeatureEndpoint, FeatureRef,
    LineLineFilletIds, LineLineFilletRequest, MAX_BSPLINE_CONTROLS, MAX_DOCUMENT_JSON_BYTES,
    MAX_DOCUMENT_OBJECTS, MAX_LABEL_BYTES, MAX_POLYLINE_POINTS, MirroredCurveIds, PersistentId,
    RectangleIds, SKETCH_DOCUMENT_VERSION, ScalarDomain, ScalarUnit, SketchDocument,
    TangentOrientation,
};
pub use document_lowering::{
    ContactRuntimeMapping, CurveRuntimeMapping, DocumentContactRole, DocumentRuntimeMap,
    DocumentSourceRuntimeMapping, LoweredDocument, PointRuntimeMapping, RuntimeCurve,
    RuntimeSource,
};
pub use document_session::{
    DocumentCommand, DocumentCommandEffect, DocumentCommandOutcome, DocumentDragTarget,
    DocumentEdit, DocumentSessionError, DocumentSolveRequest, DocumentSolveResult,
    DocumentTransactionOutcome, SketchDocumentSession,
};
pub use geosolve_geometry::{
    BSplineBasis, BSplineContinuity, BSplineDefinitionError, BSplineEvaluationError, BSplineForm,
    BSplineInsertionError, BSplineKnotSide, BSplineSpanIndex, ConicDefinitionError,
    ConicEvaluationError, CurveDifferential2, CurveDifferentialError, DirectedParameterTrim,
    EllipseAxisObservability, HyperbolaBranch, MAX_BSPLINE_DEGREE, NurbsDefinitionError,
    NurbsEvaluationError, NurbsInsertionError, ProperConicKind,
};
pub use model::{
    ArcId, BSplineId, BezierId, CircleId, ConicId, CoordinateAxis, CurveContactNeighborhood,
    CurveContinuity, CurveCurvatureRelation, CurveDirectionRelation, CurveMeasurementKind,
    CurveNormalSide, CurveTangentOrientation, DimensionKind, DimensionMode, FilletEndpointOrder,
    LineSegment, NurbsId, PointId, SegmentBranch, SegmentEndpoint, SegmentId, Sketch,
    SketchConstraint, SketchConstraintId, SketchConstraintKind, SketchCurve, SketchCurveContact,
    SketchDimension, SketchDimensionId, SketchError, SketchPoint,
};
pub use nurbs::NurbsCurve;
pub use profiles::{
    VisualProfileAnalysis, VisualProfileBudgetCounter, VisualProfileBudgetReport,
    VisualProfileContour, VisualProfileCurveFamily, VisualProfileEdge, VisualProfileFace,
    VisualProfileGeometryScope, VisualProfileIntersection, VisualProfileIssue,
    VisualProfileIssueKind, VisualProfileOptions, VisualProfileOrientation, VisualProfileStatus,
};
pub use scenarios::{
    ConflictingRectangleIds, TangentCirclesIds, UnderconstrainedTriangleIds, conflicting_rectangle,
    redundant_rectangle, tangent_circles, underconstrained_triangle,
};
pub use session::{
    SketchPatch, SketchSession, SketchSessionError, SketchSessionPatch, SketchSessionRevisions,
};
