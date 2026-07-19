//! A small, stable-ID CAD sketch model compiled into `geosolve-core`.

mod alpha_scenarios;
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
mod residuals;
mod scenarios;
mod session;

pub use alpha_scenarios::{
    A1ScenarioIds, A2ScenarioIds, A3ScenarioIds, A4ScenarioIds, A5ScenarioIds, A8ScenarioIds,
    AlphaPerformanceSize, AlphaScenarioFixture, AlphaScenarioIds, AlphaScenarioKind,
    ConicCircleLimitIds, ConicGalleryIds, ConicTangencyIds, DiagnosticEndpointBoundIds,
    DiagnosticRankDropIds, DiagnosticRedundancyIds, MotionCamIds, MotionOrbitIds,
    MotionPeaucellierIds, MotionRotatingSquareIds, MotionScissorIds, MotionScissorTowerIds,
    MotionScotchYokeIds, MotionTrammelIds, StressBridgeIds, StressCompassIds,
    alpha_performance_document, alpha_scenario,
};
pub use beziers::{BezierCurve, BezierEvaluationError, BezierKind};
pub use bsplines::BSplineCurve;
pub use compiler::{
    ArcRadiusVariableMapping, CircleRadiusVariableMapping, CompiledSketch, ConicScalarRole,
    ConicScalarVariableMapping, ConicVectorRole, ConicVectorVariableMapping, DragTarget,
    LatentVariableMapping, LatentVariableRole, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
    MIN_REPRESENTABLE_RADIUS, NurbsWeightVariableMapping, PointVariableMapping,
    ReferenceDimensionValue, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, SketchBound, SketchBoundMapping,
    SketchGeometry, SketchSolveRequest, SketchSolveResult, SketchSource, SketchSourceMapping,
    SolveRejection, SolvedArc, SolvedCircle, SolvedConic, SolvedConicKind, SolvedNurbs,
    SolvedPoint,
};
pub use conics::{ConicCurve, ConicGeometry, ConicKind};
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
    DocumentArcTangencySide, DocumentBSplineForm, DocumentBSplineInsertion,
    DocumentBSplineSpanDirection, DocumentCircleContainment, DocumentCircleTangencyMode,
    DocumentConicFeature, DocumentConicMeasurement, DocumentConicQueryError, DocumentConstraint,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
    DocumentCurveEvaluationError, DocumentCurveMeasurementError, DocumentCurveMeasurementKind,
    DocumentCurveNormalSide, DocumentDimension, DocumentDimensionDefinition, DocumentDimensionId,
    DocumentDimensionMode, DocumentError, DocumentHyperbolaBranch, DocumentId, DocumentLineSide,
    DocumentNurbsInsertion, DocumentObjectId, DocumentSourceId, DocumentTrimProjection,
    DocumentTrimProjectionError, FeatureEndpoint, FeatureRef, MAX_BSPLINE_CONTROLS,
    MAX_DOCUMENT_JSON_BYTES, MAX_DOCUMENT_OBJECTS, MAX_LABEL_BYTES, MAX_POLYLINE_POINTS,
    PersistentId, RectangleIds, SKETCH_DOCUMENT_VERSION, ScalarDomain, ScalarUnit, SketchDocument,
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
    CurveNormalSide, CurveTangentOrientation, DimensionKind, DimensionMode, LineSegment, NurbsId,
    PointId, SegmentBranch, SegmentEndpoint, SegmentId, Sketch, SketchConstraint,
    SketchConstraintId, SketchConstraintKind, SketchCurve, SketchCurveContact, SketchDimension,
    SketchDimensionId, SketchError, SketchPoint,
};
pub use nurbs::NurbsCurve;
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
