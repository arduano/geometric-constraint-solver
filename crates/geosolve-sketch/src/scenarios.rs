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

/// Stable IDs for canonical scenario S2 and its equal-width redundancy variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConflictingRectangleIds {
    pub a: PointId,
    pub b: PointId,
    pub c: PointId,
    pub d: PointId,
    pub ab: SegmentId,
    pub bc: SegmentId,
    pub cd: SegmentId,
    pub da: SegmentId,
    pub fixed_a: SketchConstraintId,
    pub horizontal_ab: SketchConstraintId,
    pub horizontal_cd: SketchConstraintId,
    pub vertical_bc: SketchConstraintId,
    pub vertical_da: SketchConstraintId,
    pub width_4: SketchDimensionId,
    pub width_5: SketchDimensionId,
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

/// Builds canonical conflicting scenario S2 exactly as specified in `docs/SCENARIOS.md`.
///
/// # Errors
///
/// Returns an error if the canonical finite geometry cannot be constructed.
pub fn conflicting_rectangle() -> Result<(Sketch, ConflictingRectangleIds), SketchError> {
    rectangle_with_second_width(5.0)
}

/// Builds the S2 redundancy variant with two equal driving width dimensions.
///
/// # Errors
///
/// Returns an error if the canonical finite geometry cannot be constructed.
pub fn redundant_rectangle() -> Result<(Sketch, ConflictingRectangleIds), SketchError> {
    rectangle_with_second_width(4.0)
}

fn rectangle_with_second_width(
    second_width: f64,
) -> Result<(Sketch, ConflictingRectangleIds), SketchError> {
    let mut sketch = Sketch::new(1.0)?;
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0))?;
    let b = sketch.add_named_point("B", Point2::new(4.0, 0.0))?;
    let c = sketch.add_named_point("C", Point2::new(4.0, 3.0))?;
    let d = sketch.add_named_point("D", Point2::new(0.0, 3.0))?;
    let ab = sketch.add_named_segment("AB", a, b)?;
    let bc = sketch.add_named_segment("BC", b, c)?;
    let cd = sketch.add_named_segment("CD", c, d)?;
    let da = sketch.add_named_segment("DA", d, a)?;
    let fixed_a = sketch.add_fixed_point(a)?;
    let horizontal_ab = sketch.add_horizontal(ab)?;
    let horizontal_cd = sketch.add_horizontal(cd)?;
    let vertical_bc = sketch.add_vertical(bc)?;
    let vertical_da = sketch.add_vertical(da)?;
    let width_4 = sketch.add_segment_length(ab, 4.0, DimensionMode::Driving)?;
    let width_5 = sketch.add_segment_length(ab, second_width, DimensionMode::Driving)?;
    Ok((
        sketch,
        ConflictingRectangleIds {
            a,
            b,
            c,
            d,
            ab,
            bc,
            cd,
            da,
            fixed_a,
            horizontal_ab,
            horizontal_cd,
            vertical_bc,
            vertical_da,
            width_4,
            width_5,
        },
    ))
}
