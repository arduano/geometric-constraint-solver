use geosolve_geometry::Point2;

use crate::{
    CenterDirectionBranch, CircleId, CircleTangencyMode, DimensionMode, PointId, SegmentId, Sketch,
    SketchConstraintId, SketchDimensionId, SketchError,
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

/// Stable IDs for canonical branch-sensitive scenario S3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TangentCirclesIds {
    pub center_a: PointId,
    pub center_b: PointId,
    pub centers: SegmentId,
    pub circle_a: CircleId,
    pub circle_b: CircleId,
    pub fixed_center_a: SketchConstraintId,
    pub horizontal_centers: SketchConstraintId,
    pub radius_a: SketchDimensionId,
    pub radius_b: SketchDimensionId,
    pub tangency: SketchConstraintId,
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

/// Builds canonical S3 exactly as specified in `docs/SCENARIOS.md`.
///
/// Circle radii are solver scalars fixed by driving dimensions. The tangency
/// source starts in external mode and retains an explicit positive-x
/// center-direction branch; callers can switch it to internal mode with
/// [`Sketch::set_circle_tangency_mode`].
///
/// # Errors
///
/// Returns an error if the canonical finite geometry cannot be constructed.
pub fn tangent_circles() -> Result<(Sketch, TangentCirclesIds), SketchError> {
    let mut sketch = Sketch::new(1.0)?;
    let center_a = sketch.add_named_point("A center", Point2::new(0.0, 0.0))?;
    let center_b = sketch.add_named_point("B center", Point2::new(5.0, 0.5))?;
    let centers = sketch.add_named_segment("A-B centers", center_a, center_b)?;
    let circle_a = sketch.add_named_circle("circle A", center_a, 2.0)?;
    let circle_b = sketch.add_named_circle("circle B", center_b, 1.0)?;
    let fixed_center_a = sketch.add_fixed_point(center_a)?;
    let horizontal_centers = sketch.add_horizontal(centers)?;
    let radius_a = sketch.add_circle_radius(circle_a, 2.0, DimensionMode::Driving)?;
    let radius_b = sketch.add_circle_radius(circle_b, 1.0, DimensionMode::Driving)?;
    let tangency = sketch.add_circle_circle_tangency(
        circle_a,
        circle_b,
        CircleTangencyMode::External,
        CenterDirectionBranch::positive_x(),
    )?;
    Ok((
        sketch,
        TangentCirclesIds {
            center_a,
            center_b,
            centers,
            circle_a,
            circle_b,
            fixed_center_a,
            horizontal_centers,
            radius_a,
            radius_b,
            tangency,
        },
    ))
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
