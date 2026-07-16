use std::f64::consts::PI;

use geosolve_geometry::{PlaneFrame, Point2, Point3, Pose2, Vector2, Vector3};

use crate::{
    AxisDirectionBranch, AxisFeatureId, BodyId, BranchMonitorId, BranchSign, DriverId, JointId,
    Linkage, LinkageError, PointFeatureId,
};

/// Explicit four-bar circle-intersection root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FourBarAssemblyMode {
    Open,
    Crossed,
}

/// Stable IDs needed to inspect, drive, and render canonical L1/L2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FourBarIds {
    pub ground: BodyId,
    pub input: BodyId,
    pub coupler: BodyId,
    pub rocker: BodyId,
    pub ground_o2: PointFeatureId,
    pub ground_o4: PointFeatureId,
    pub input_o2: PointFeatureId,
    pub input_a: PointFeatureId,
    pub coupler_a: PointFeatureId,
    pub coupler_b: PointFeatureId,
    pub rocker_b: PointFeatureId,
    pub rocker_o4: PointFeatureId,
    pub o2_joint: JointId,
    pub a_joint: JointId,
    pub b_joint: JointId,
    pub o4_joint: JointId,
    pub driver: DriverId,
    pub orientation_monitor: BranchMonitorId,
    pub assembly_mode: FourBarAssemblyMode,
    pub orientation_sign: BranchSign,
}

/// Explicit slider-crank assembly choice retained independently of coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SliderCrankAssemblyMode {
    PositiveX,
}

/// Stable IDs needed to inspect, drive, and render canonical L3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SliderCrankIds {
    pub ground: BodyId,
    pub crank: BodyId,
    pub rod: BodyId,
    pub slider: BodyId,
    pub ground_o: PointFeatureId,
    pub ground_guide_origin: PointFeatureId,
    pub crank_o: PointFeatureId,
    pub crank_a: PointFeatureId,
    pub rod_a: PointFeatureId,
    pub rod_slider: PointFeatureId,
    pub slider_pin: PointFeatureId,
    pub ground_guide_axis: AxisFeatureId,
    pub slider_axis: AxisFeatureId,
    pub o_joint: JointId,
    pub a_joint: JointId,
    pub slider_joint: JointId,
    pub guide_joint: JointId,
    pub driver: DriverId,
    pub positive_x_monitor: BranchMonitorId,
    pub assembly_mode: SliderCrankAssemblyMode,
}

/// Canonical orthonormal world XY plane.
///
/// # Panics
///
/// Panics only if the geometry crate rejects canonical finite unit axes.
#[must_use]
pub fn xy_plane_frame() -> PlaneFrame {
    PlaneFrame::try_new(Point3::origin(), Vector3::x(), Vector3::y())
        .expect("canonical XY workplane is valid")
}

/// Builds canonical L1 or L2 at model scale one.
///
/// # Errors
///
/// Returns an error if canonical finite geometry cannot be constructed.
pub fn four_bar(mode: FourBarAssemblyMode) -> Result<(Linkage, FourBarIds), LinkageError> {
    four_bar_with_scale(mode, 1.0)
}

/// Builds canonical L1.
///
/// # Errors
///
/// Returns an error if canonical finite geometry cannot be constructed.
pub fn four_bar_open() -> Result<(Linkage, FourBarIds), LinkageError> {
    four_bar(FourBarAssemblyMode::Open)
}

/// Builds canonical L2.
///
/// # Errors
///
/// Returns an error if canonical finite geometry cannot be constructed.
pub fn four_bar_crossed() -> Result<(Linkage, FourBarIds), LinkageError> {
    four_bar(FourBarAssemblyMode::Crossed)
}

/// Builds geometrically similar L1/L2 data for scale-invariance verification.
///
/// # Errors
///
/// Returns an error for an invalid scale or non-constructible geometry.
pub fn four_bar_with_scale(
    mode: FourBarAssemblyMode,
    scale: f64,
) -> Result<(Linkage, FourBarIds), LinkageError> {
    let mut linkage = Linkage::new(scale, xy_plane_frame())?;
    let o2 = Point2::new(0.0, 0.0);
    let o4 = Point2::new(4.0 * scale, 0.0);
    let input_length = 1.5 * scale;
    let coupler_length = 3.0 * scale;
    let rocker_length = 2.5 * scale;
    let input_angle = 60.0 * PI / 180.0;
    let a = Point2::new(
        input_length * input_angle.cos(),
        input_length * input_angle.sin(),
    );
    let (upper, lower) = circle_intersections(a, coupler_length, o4, rocker_length)?;
    let b = match mode {
        FourBarAssemblyMode::Open => upper,
        FourBarAssemblyMode::Crossed => lower,
    };
    let orientation_value = cross(o4 - a, b - a);
    let orientation_sign = BranchSign::from_nonzero(orientation_value)?;

    let ground = linkage.add_body("ground", Pose2::identity(), true)?;
    let input = linkage.add_body(
        "input",
        Pose2 {
            translation: o2.coords,
            angle: input_angle,
        },
        false,
    )?;
    let coupler = linkage.add_body(
        "coupler",
        Pose2 {
            translation: a.coords,
            angle: (b.y - a.y).atan2(b.x - a.x),
        },
        false,
    )?;
    let rocker = linkage.add_body(
        "rocker",
        Pose2 {
            translation: o4.coords,
            angle: (b.y - o4.y).atan2(b.x - o4.x),
        },
        false,
    )?;

    let ground_o2 = linkage.add_point_feature("ground.O2", ground, o2)?;
    let ground_o4 = linkage.add_point_feature("ground.O4", ground, o4)?;
    let input_o2 = linkage.add_point_feature("input.O2", input, Point2::origin())?;
    let input_a = linkage.add_point_feature("input.A", input, Point2::new(input_length, 0.0))?;
    let coupler_a = linkage.add_point_feature("coupler.A", coupler, Point2::origin())?;
    let coupler_b =
        linkage.add_point_feature("coupler.B", coupler, Point2::new(coupler_length, 0.0))?;
    let rocker_o4 = linkage.add_point_feature("rocker.O4", rocker, Point2::origin())?;
    let rocker_b =
        linkage.add_point_feature("rocker.B", rocker, Point2::new(rocker_length, 0.0))?;

    let o2_joint = linkage.add_revolute_joint("O2 revolute", ground_o2, input_o2)?;
    let a_joint = linkage.add_revolute_joint("A revolute", input_a, coupler_a)?;
    let b_joint = linkage.add_revolute_joint("B revolute", coupler_b, rocker_b)?;
    let o4_joint = linkage.add_revolute_joint("O4 revolute", rocker_o4, ground_o4)?;
    let driver =
        linkage.add_angular_driver("input angle", ground, input, input_angle, 2.0 * PI / 180.0)?;
    let orientation_monitor =
        linkage.add_orientation_branch_monitor(input_a, ground_o4, coupler_b, orientation_sign)?;

    Ok((
        linkage,
        FourBarIds {
            ground,
            input,
            coupler,
            rocker,
            ground_o2,
            ground_o4,
            input_o2,
            input_a,
            coupler_a,
            coupler_b,
            rocker_b,
            rocker_o4,
            o2_joint,
            a_joint,
            b_joint,
            o4_joint,
            driver,
            orientation_monitor,
            assembly_mode: mode,
            orientation_sign,
        },
    ))
}

/// Builds canonical L3 at model scale one.
///
/// # Errors
///
/// Returns an error if canonical finite geometry cannot be constructed.
pub fn slider_crank() -> Result<(Linkage, SliderCrankIds), LinkageError> {
    slider_crank_with_scale(1.0)
}

/// Builds geometrically similar L3 data for scale-invariance verification.
///
/// # Errors
///
/// Returns an error for an invalid scale or non-constructible geometry.
pub fn slider_crank_with_scale(scale: f64) -> Result<(Linkage, SliderCrankIds), LinkageError> {
    let mut linkage = Linkage::new(scale, xy_plane_frame())?;
    let crank_length = 1.25 * scale;
    let rod_length = 3.5 * scale;
    let crank_angle = 45.0 * PI / 180.0;
    let a = Point2::new(
        crank_length * crank_angle.cos(),
        crank_length * crank_angle.sin(),
    );
    let horizontal = (rod_length * rod_length - a.y * a.y).sqrt();
    if !horizontal.is_finite() || horizontal <= 0.0 {
        return Err(LinkageError::NonFiniteValue {
            context: "slider-crank circle intersection",
            value: horizontal,
        });
    }
    let slider_point = Point2::new(a.x + horizontal, 0.0);
    let rod_angle = (slider_point.y - a.y).atan2(slider_point.x - a.x);

    let ground = linkage.add_body("ground", Pose2::identity(), true)?;
    let crank = linkage.add_body(
        "crank",
        Pose2 {
            translation: Vector2::zeros(),
            angle: crank_angle,
        },
        false,
    )?;
    let rod = linkage.add_body(
        "rod",
        Pose2 {
            translation: a.coords,
            angle: rod_angle,
        },
        false,
    )?;
    let slider = linkage.add_body(
        "slider",
        Pose2 {
            translation: slider_point.coords,
            angle: 0.0,
        },
        false,
    )?;

    let ground_o = linkage.add_point_feature("ground.O", ground, Point2::origin())?;
    let ground_guide_origin =
        linkage.add_point_feature("ground.guide-origin", ground, Point2::origin())?;
    let ground_guide_axis = linkage.add_axis_feature("ground.guide-axis", ground, Vector2::x())?;
    let crank_o = linkage.add_point_feature("crank.O", crank, Point2::origin())?;
    let crank_a = linkage.add_point_feature("crank.A", crank, Point2::new(crank_length, 0.0))?;
    let rod_a = linkage.add_point_feature("rod.A", rod, Point2::origin())?;
    let rod_slider =
        linkage.add_point_feature("rod.slider-pin", rod, Point2::new(rod_length, 0.0))?;
    let slider_pin = linkage.add_point_feature("slider.pin", slider, Point2::origin())?;
    let slider_axis = linkage.add_axis_feature("slider.axis", slider, Vector2::x())?;

    let o_joint = linkage.add_revolute_joint("O revolute", ground_o, crank_o)?;
    let a_joint = linkage.add_revolute_joint("A revolute", crank_a, rod_a)?;
    let slider_pin_joint =
        linkage.add_revolute_joint("slider-pin revolute", rod_slider, slider_pin)?;
    let guide_joint = linkage.add_prismatic_joint(
        "slider guide",
        ground_guide_origin,
        ground_guide_axis,
        slider_pin,
        slider_axis,
        AxisDirectionBranch::Same,
    )?;
    let driver =
        linkage.add_angular_driver("crank angle", ground, crank, crank_angle, 2.0 * PI / 180.0)?;
    let positive_x_monitor = linkage.add_directed_displacement_branch_monitor(
        ground_guide_origin,
        slider_pin,
        ground_guide_axis,
        BranchSign::Positive,
    )?;

    Ok((
        linkage,
        SliderCrankIds {
            ground,
            crank,
            rod,
            slider,
            ground_o,
            ground_guide_origin,
            crank_o,
            crank_a,
            rod_a,
            rod_slider,
            slider_pin,
            ground_guide_axis,
            slider_axis,
            o_joint,
            a_joint,
            slider_joint: slider_pin_joint,
            guide_joint,
            driver,
            positive_x_monitor,
            assembly_mode: SliderCrankAssemblyMode::PositiveX,
        },
    ))
}

fn circle_intersections(
    first_center: Point2<f64>,
    first_radius: f64,
    second_center: Point2<f64>,
    second_radius: f64,
) -> Result<(Point2<f64>, Point2<f64>), LinkageError> {
    let center_delta = second_center - first_center;
    let distance = center_delta.x.hypot(center_delta.y);
    if !distance.is_finite() || distance <= 0.0 {
        return Err(LinkageError::NonFiniteValue {
            context: "four-bar circle-center distance",
            value: distance,
        });
    }
    let along = (first_radius * first_radius - second_radius * second_radius + distance * distance)
        / (2.0 * distance);
    let height_squared = first_radius * first_radius - along * along;
    if !height_squared.is_finite() || height_squared <= 0.0 {
        return Err(LinkageError::NonFiniteValue {
            context: "four-bar circle-intersection height",
            value: height_squared,
        });
    }
    let height = height_squared.sqrt();
    let direction = center_delta / distance;
    let normal = Vector2::new(-direction.y, direction.x);
    let base = first_center + direction * along;
    Ok((base + normal * height, base - normal * height))
}

fn cross(first: Vector2<f64>, second: Vector2<f64>) -> f64 {
    first.x * second.y - first.y * second.x
}
