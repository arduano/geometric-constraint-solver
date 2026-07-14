//! A small, stable-ID CAD sketch model compiled into `geosolve-core`.

mod compiler;
mod model;
mod residuals;
mod scenarios;

pub use compiler::{
    CompiledSketch, DragTarget, PointVariableMapping, ReferenceDimensionValue, SketchGeometry,
    SketchSolveRequest, SketchSolveResult, SketchSource, SketchSourceMapping, SolveRejection,
    SolvedPoint,
};
pub use model::{
    CoordinateAxis, DimensionKind, DimensionMode, LineSegment, PointId, SegmentBranch, SegmentId,
    Sketch, SketchConstraint, SketchConstraintId, SketchConstraintKind, SketchDimension,
    SketchDimensionId, SketchError, SketchPoint,
};
pub use scenarios::{
    ConflictingRectangleIds, UnderconstrainedTriangleIds, conflicting_rectangle,
    redundant_rectangle, underconstrained_triangle,
};

/// The first end-to-end acceptance scenes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchScenario {
    UnderconstrainedTriangle,
    ConflictingRectangle,
    TangentCircles,
}
