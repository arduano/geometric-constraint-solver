use geosolve_geometry::Point2;

use crate::{
    DimensionMode, PointId, SegmentId, Sketch, SketchConstraintId, SketchDimensionId, SketchError,
};

/// Stable IDs needed to render and drag canonical scenario S1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnderconstrainedTriangleIds {
    pub a: PointId,
    pub b: PointId,
    pub c: PointId,
    pub ab: SegmentId,
    pub fixed_a: SketchConstraintId,
    pub horizontal_ab: SketchConstraintId,
    pub length_ab: SketchDimensionId,
    pub distance_ac: SketchDimensionId,
}

/// Builds canonical S1 exactly as specified in `docs/SCENARIOS.md`.
///
/// # Errors
///
/// Returns an error if the canonical finite geometry cannot be constructed.
pub fn underconstrained_triangle() -> Result<(Sketch, UnderconstrainedTriangleIds), SketchError> {
    let mut sketch = Sketch::new(1.0)?;
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0))?;
    let b = sketch.add_named_point("B", Point2::new(4.0, 0.0))?;
    let c = sketch.add_named_point("C", Point2::new(2.2, 2.0))?;
    let ab = sketch.add_named_segment("AB", a, b)?;
    let fixed_a = sketch.add_fixed_point(a)?;
    let horizontal_ab = sketch.add_horizontal(ab)?;
    let length_ab = sketch.add_segment_length(ab, 4.0, DimensionMode::Driving)?;
    let distance_ac = sketch.add_point_distance(a, c, 3.0, DimensionMode::Driving)?;
    Ok((
        sketch,
        UnderconstrainedTriangleIds {
            a,
            b,
            c,
            ab,
            fixed_a,
            horizontal_ab,
            length_ab,
            distance_ac,
        },
    ))
}
