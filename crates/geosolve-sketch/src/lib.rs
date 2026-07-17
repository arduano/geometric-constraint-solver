//! A small, stable-ID CAD sketch model compiled into `geosolve-core`.

mod alpha_scenarios;
mod beziers;
mod compiler;
mod curves;
mod document;
mod document_lowering;
mod document_session;
mod generic_curves;
mod model;
mod residuals;
mod scenarios;
mod session;

pub use alpha_scenarios::{
    A1ScenarioIds, A2ScenarioIds, A3ScenarioIds, A4ScenarioIds, A5ScenarioIds, A8ScenarioIds,
    AlphaPerformanceSize, AlphaScenarioFixture, AlphaScenarioIds, AlphaScenarioKind,
    DiagnosticEndpointBoundIds, DiagnosticRankDropIds, DiagnosticRedundancyIds, MotionCamIds,
    MotionOrbitIds, MotionPeaucellierIds, MotionRotatingSquareIds, MotionScissorIds,
    MotionScissorTowerIds, MotionScotchYokeIds, MotionTrammelIds, StressBridgeIds,
    StressCompassIds, alpha_performance_document, alpha_scenario,
};
pub use beziers::{BezierCurve, BezierEvaluationError, BezierKind};
pub use compiler::{
    ArcRadiusVariableMapping, CircleRadiusVariableMapping, CompiledSketch, DragTarget,
    LatentVariableMapping, LatentVariableRole, MIN_REPRESENTABLE_RADIUS, PointVariableMapping,
    ReferenceDimensionValue, SketchBound, SketchBoundMapping, SketchGeometry, SketchSolveRequest,
    SketchSolveResult, SketchSource, SketchSourceMapping, SolveRejection, SolvedArc, SolvedCircle,
    SolvedPoint,
};
pub use curves::{
    AngleOrientation, ArcCircleTangencySide, ArcSweep, CENTER_DIRECTION_COSINE_MARGIN,
    CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE, CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE,
    CIRCLE_ARC_TANGENCY_SCALE_UNCERTAINTY_MULTIPLIER, CONTACT_PARAMETER_ROUNDOFF_TOLERANCE,
    CenterDirectionBranch, Circle, CircleContainment, CircleTangencyMode, CircularArc,
    ContactState, LineParameterDomain, LineSide,
};
pub use document::{
    ContactDefinition, ContactDomain, ContactId, ContactNeighborhood, ContactSlot,
    ContactStateEdit, CurveDefinition, CurveId, CurveSpan, DesignCurve, DesignPoint, DesignPointId,
    DesignScalar, DesignScalarId, DocumentAngleOrientation, DocumentArcSweep,
    DocumentArcTangencySide, DocumentCircleContainment, DocumentCircleTangencyMode,
    DocumentConstraint, DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis,
    DocumentCurveEvaluationError, DocumentDimension, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentDimensionMode, DocumentError, DocumentId, DocumentLineSide,
    DocumentObjectId, DocumentSourceId, FeatureEndpoint, FeatureRef, MAX_DOCUMENT_JSON_BYTES,
    MAX_DOCUMENT_OBJECTS, MAX_LABEL_BYTES, MAX_POLYLINE_POINTS, PersistentId, RectangleIds,
    SKETCH_DOCUMENT_VERSION, ScalarDomain, ScalarUnit, SketchDocument, TangentOrientation,
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
pub use model::{
    ArcId, BezierId, CircleId, CoordinateAxis, CurveContactNeighborhood, CurveTangentOrientation,
    DimensionKind, DimensionMode, LineSegment, PointId, SegmentBranch, SegmentEndpoint, SegmentId,
    Sketch, SketchConstraint, SketchConstraintId, SketchConstraintKind, SketchCurve,
    SketchCurveContact, SketchDimension, SketchDimensionId, SketchError, SketchPoint,
};
pub use scenarios::{
    ConflictingRectangleIds, TangentCirclesIds, UnderconstrainedTriangleIds, conflicting_rectangle,
    redundant_rectangle, tangent_circles, underconstrained_triangle,
};
pub use session::{
    SketchPatch, SketchSession, SketchSessionError, SketchSessionPatch, SketchSessionRevisions,
};

/// The first end-to-end acceptance scenes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchScenario {
    UnderconstrainedTriangle,
    ConflictingRectangle,
    TangentCircles,
}
