//! CAD sketch domain model and compiler into `geosolve-core` residuals.

use geosolve_geometry::Point2;
use slotmap::new_key_type;

new_key_type! {
    pub struct PointId;
    pub struct EntityId;
    pub struct SketchConstraintId;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchPoint {
    pub position: Point2<f64>,
}

/// The first end-to-end acceptance scenes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchScenario {
    UnderconstrainedTriangle,
    ConflictingRectangle,
    TangentCircles,
}
