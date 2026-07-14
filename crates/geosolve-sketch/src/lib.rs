//! A small, stable-ID CAD sketch model compiled into `geosolve-core`.

mod compiler;
mod curves;
mod model;
mod residuals;
mod scenarios;

pub use compiler::{
    ArcRadiusVariableMapping, CircleRadiusVariableMapping, CompiledSketch, DragTarget,
    LatentVariableMapping, LatentVariableRole, PointVariableMapping, ReferenceDimensionValue,
    SketchGeometry, SketchSolveRequest, SketchSolveResult, SketchSource, SketchSourceMapping,
    SolveRejection, SolvedArc, SolvedCircle, SolvedPoint,
};
pub use curves::{
    AngleOrientation, ArcSweep, CENTER_DIRECTION_COSINE_MARGIN,
    CONTACT_PARAMETER_ROUNDOFF_TOLERANCE, CenterDirectionBranch, Circle, CircleContainment,
    CircleTangencyMode, CircularArc, ContactState, LineParameterDomain, LineSide,
};
pub use model::{
    ArcId, CircleId, CoordinateAxis, DimensionKind, DimensionMode, LineSegment, PointId,
    SegmentBranch, SegmentId, Sketch, SketchConstraint, SketchConstraintId, SketchConstraintKind,
    SketchDimension, SketchDimensionId, SketchError, SketchPoint,
};
pub use scenarios::{
    ConflictingRectangleIds, TangentCirclesIds, UnderconstrainedTriangleIds, conflicting_rectangle,
    redundant_rectangle, tangent_circles, underconstrained_triangle,
};

/// The first end-to-end acceptance scenes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchScenario {
    UnderconstrainedTriangle,
    ConflictingRectangle,
    TangentCircles,
}
