use std::f64::consts::FRAC_PI_2;

use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};

use crate::{
    SpatialAssembly, SpatialAssemblyError, SpatialAxisFeatureId, SpatialAxisParity, SpatialBodyId,
    SpatialCoordinateId, SpatialFrameFeatureId, SpatialHingeTarget, SpatialModeMonitorId,
    SpatialModeSign, SpatialPlanarTranslationAxis, SpatialPlaneFeatureId, SpatialPointFeatureId,
    SpatialSourceId,
};

/// Reusable driven spatial assemblies intended for examples and renderers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialExampleKind {
    ShaftBearing,
    BlockBase,
}

impl SpatialExampleKind {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::ShaftBearing => "shaft-bearing",
            Self::BlockBase => "block-base",
        }
    }
}

/// Renderer-facing identities in the shaft/bearing example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShaftBearingExampleIds {
    /// Bearing, then shaft.
    pub bodies: [SpatialBodyId; 2],
    /// Bearing, then shaft drawing frames.
    pub frames: [SpatialFrameFeatureId; 2],
    /// Bearing, then shaft cylindrical axes.
    pub axes: [SpatialAxisFeatureId; 2],
    pub translation_plane: SpatialPlaneFeatureId,
    pub translation_witness: SpatialPointFeatureId,
    pub joint: SpatialSourceId,
    /// Hinge, then axial translation.
    pub coordinates: [SpatialCoordinateId; 2],
    /// Hinge, then axial translation.
    pub drivers: [SpatialSourceId; 2],
    /// Axis parity, hinge winding, then positive translation side.
    pub monitors: [SpatialModeMonitorId; 3],
}

/// Renderer-facing identities in the block/base example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockBaseExampleIds {
    /// Base, then block.
    pub bodies: [SpatialBodyId; 2],
    /// Base, then block drawing frames.
    pub frames: [SpatialFrameFeatureId; 2],
    /// Base, then block normal axes.
    pub axes: [SpatialAxisFeatureId; 2],
    /// Base, then block joint planes.
    pub planes: [SpatialPlaneFeatureId; 2],
    pub side_witness: SpatialPointFeatureId,
    pub joint: SpatialSourceId,
    /// Hinge, plane X, then plane Y.
    pub coordinates: [SpatialCoordinateId; 3],
    /// Hinge, plane X, then plane Y.
    pub drivers: [SpatialSourceId; 3],
    /// Normal parity, hinge winding, then positive plane side.
    pub monitors: [SpatialModeMonitorId; 3],
}

/// Scenario-specific renderer identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialExampleIds {
    ShaftBearing(ShaftBearingExampleIds),
    BlockBase(BlockBaseExampleIds),
}

/// One fully constructed spatial example and its renderer-facing identities.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialExampleFixture {
    pub assembly: SpatialAssembly,
    pub ids: SpatialExampleIds,
}

/// Stable identities for the embedded displacement-driven slider-crank fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedSpatialSliderCrankIds {
    /// Ground, crank, connecting rod, then slider.
    pub bodies: [SpatialBodyId; 4],
    /// Crank pin, rod crank pin, rod slider pin, then slider pin.
    pub points: [SpatialPointFeatureId; 4],
    /// Ground guide, slider guide, ground normal, rod transverse, then rod normal.
    pub axes: [SpatialAxisFeatureId; 5],
    pub driver: SpatialSourceId,
    pub crank_hinge: SpatialCoordinateId,
    pub slider_translation: SpatialCoordinateId,
    /// Crank winding, rod normal parity, then positive-X slider side.
    pub monitors: [SpatialModeMonitorId; 3],
}

/// Exact embedded spatial slider-crank assembly and its stable identities.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedSpatialSliderCrankFixture {
    pub assembly: SpatialAssembly,
    pub ids: EmbeddedSpatialSliderCrankIds,
}

/// Builds one exact driven spatial example at a uniform model scale.
///
/// Every length is multiplied by `scale`; angle targets, winding, parity, and
/// monitor state are scale independent.
///
/// # Errors
///
/// Returns a spatial construction error for an invalid scale or geometry.
pub fn spatial_example(
    kind: SpatialExampleKind,
    scale: f64,
) -> Result<SpatialExampleFixture, SpatialAssemblyError> {
    match kind {
        SpatialExampleKind::ShaftBearing => shaft_bearing_example(scale),
        SpatialExampleKind::BlockBase => block_base_example(scale),
    }
}

/// Builds the M23 displacement-driven slider-crank under one static `SE(3)` embedding.
///
/// The mechanism has crank radius `1.25 * scale`, rod length `3.5 * scale`,
/// positive-X assembly mode and a slider displacement position driver. The
/// supplied crank phase must remain on winding zero's canonical interval.
///
/// # Errors
///
/// Returns a spatial construction error for invalid scale, pose, phase or any
/// generated feature/source geometry.
#[allow(clippy::too_many_lines)]
pub fn embedded_spatial_slider_crank(
    scale: f64,
    embedding: Pose3,
    crank_phase: f64,
) -> Result<EmbeddedSpatialSliderCrankFixture, SpatialAssemblyError> {
    let crank_length = 1.25 * scale;
    let rod_length = 3.5 * scale;
    let crank_pin = Point3::new(
        crank_length * crank_phase.cos(),
        crank_length * crank_phase.sin(),
        0.0,
    );
    let squared_horizontal = rod_length * rod_length - crank_pin.y * crank_pin.y;
    if !squared_horizontal.is_finite() || squared_horizontal <= 0.0 {
        return Err(SpatialAssemblyError::InvalidField {
            field: "embedded_slider_crank.crank_phase",
            message: "crank phase does not produce a regular positive-X rod branch".to_owned(),
        });
    }
    let horizontal = squared_horizontal.sqrt();
    let slider_x = crank_pin.x + horizontal;
    let rod_angle = (-crank_pin.y).atan2(horizontal);
    let planar_pose = |translation: Vector3<f64>, angle: f64| {
        let half = 0.5 * angle;
        Pose3::try_new(translation, [half.cos(), 0.0, 0.0, half.sin()])
    };
    let transformed = |pose: Pose3| embedding.compose(&pose);
    let identity = Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z())?;
    let x_axis = Frame3::try_new(Point3::origin(), Vector3::y(), Vector3::z(), Vector3::x())?;
    let y_axis = Frame3::try_new(Point3::origin(), Vector3::z(), Vector3::x(), Vector3::y())?;

    let mut assembly = SpatialAssembly::new(scale)?;
    let ground = assembly.add_body("ground", transformed(Pose3::identity())?)?;
    let crank = assembly.add_body(
        "crank",
        transformed(planar_pose(Vector3::zeros(), crank_phase)?)?,
    )?;
    let rod = assembly.add_body(
        "connecting rod",
        transformed(planar_pose(crank_pin.coords, rod_angle)?)?,
    )?;
    let slider = assembly.add_body(
        "slider",
        transformed(planar_pose(Vector3::new(slider_x, 0.0, 0.0), 0.0)?)?,
    )?;

    let ground_hinge = assembly.add_frame_feature("ground crank hinge", ground, identity)?;
    let crank_hinge_feature = assembly.add_frame_feature("crank hinge", crank, identity)?;
    let crank_pin_feature =
        assembly.add_point_feature("crank pin", crank, Point3::new(crank_length, 0.0, 0.0))?;
    let rod_crank_pin = assembly.add_point_feature("rod crank pin", rod, Point3::origin())?;
    let rod_slider_pin =
        assembly.add_point_feature("rod slider pin", rod, Point3::new(rod_length, 0.0, 0.0))?;
    let slider_pin = assembly.add_point_feature("slider pin", slider, Point3::origin())?;
    let ground_guide = assembly.add_axis_feature("ground guide", ground, x_axis)?;
    let slider_guide = assembly.add_axis_feature("slider guide", slider, x_axis)?;
    let ground_normal = assembly.add_axis_feature("ground normal", ground, identity)?;
    let rod_transverse = assembly.add_axis_feature("rod transverse", rod, y_axis)?;
    let rod_normal = assembly.add_axis_feature("rod normal", rod, identity)?;
    let positive_x_plane = assembly.add_plane_feature("positive X datum", ground, x_axis)?;

    assembly.add_physical_ground("ground fixed", ground)?;
    let revolute = assembly.add_revolute_joint(
        "crank revolute",
        ground_hinge,
        crank_hinge_feature,
        SpatialAxisParity::Aligned,
    )?;
    assembly.add_ball_joint("crank-rod ball", crank_pin_feature, rod_crank_pin)?;
    assembly.add_ball_joint("rod-slider ball", rod_slider_pin, slider_pin)?;
    let prismatic = assembly.add_prismatic_joint(
        "slider prismatic",
        ground_guide,
        slider_guide,
        SpatialAxisParity::Aligned,
    )?;
    assembly.add_axis_angle_mate("rod planar roll", ground_normal, rod_transverse, FRAC_PI_2)?;
    let crank_coordinate = assembly.add_hinge_coordinate("crank phase", revolute, 0)?;
    let translation =
        assembly.add_axial_translation_coordinate("slider displacement", prismatic)?;
    let driver = assembly.add_translation_position_driver(
        "slider displacement driver",
        translation,
        slider_x,
    )?;
    let winding_monitor =
        assembly.add_hinge_winding_monitor("crank winding zero", crank_coordinate, 0)?;
    let normal_monitor = assembly.add_axis_parity_monitor(
        "rod normal retained",
        ground_normal,
        rod_normal,
        SpatialAxisParity::Aligned,
    )?;
    let side_monitor = assembly.add_plane_side_monitor(
        "positive X slider branch",
        positive_x_plane,
        slider_pin,
        SpatialModeSign::Positive,
    )?;

    Ok(EmbeddedSpatialSliderCrankFixture {
        assembly,
        ids: EmbeddedSpatialSliderCrankIds {
            bodies: [ground, crank, rod, slider],
            points: [crank_pin_feature, rod_crank_pin, rod_slider_pin, slider_pin],
            axes: [
                ground_guide,
                slider_guide,
                ground_normal,
                rod_transverse,
                rod_normal,
            ],
            driver,
            crank_hinge: crank_coordinate,
            slider_translation: translation,
            monitors: [winding_monitor, normal_monitor, side_monitor],
        },
    })
}

#[allow(clippy::too_many_lines)]
fn shaft_bearing_example(scale: f64) -> Result<SpatialExampleFixture, SpatialAssemblyError> {
    let mut assembly = SpatialAssembly::new(scale)?;
    let bearing_pose = Pose3::exp([3.0 * scale, -1.5 * scale, 0.8 * scale, 0.23, -0.17, 0.31])?;
    let shaft_pose = Pose3::exp([-1.2 * scale, 0.7 * scale, -0.4 * scale, -0.18, 0.29, 0.16])?;
    let joint_pose = Pose3::exp([0.5 * scale, -0.2 * scale, 0.4 * scale, 0.31, -0.24, 0.19])?;
    let bearing_world = frame_from_pose(joint_pose)?;
    let shaft_world = rotated_translated_frame(bearing_world, 0.48, 1.9 * scale)?;
    let bearing_local = local_frame(bearing_pose, bearing_world)?;
    let shaft_local = local_frame(shaft_pose, shaft_world)?;

    let bearing = assembly.add_body("Grounded bearing", bearing_pose)?;
    let shaft = assembly.add_body("Driven shaft", shaft_pose)?;
    let bearing_frame =
        assembly.add_frame_feature("Bearing drawing frame", bearing, bearing_local)?;
    let shaft_frame = assembly.add_frame_feature("Shaft drawing frame", shaft, shaft_local)?;
    let bearing_axis =
        assembly.add_axis_feature("Bearing cylindrical axis", bearing, bearing_local)?;
    let shaft_axis = assembly.add_axis_feature("Shaft cylindrical axis", shaft, shaft_local)?;
    let translation_plane =
        assembly.add_plane_feature("Bearing translation datum", bearing, bearing_local)?;
    let translation_witness =
        assembly.add_point_feature("Shaft translation witness", shaft, shaft_local.origin())?;
    assembly.add_physical_ground("Bearing fixed to world", bearing)?;
    let joint = assembly.add_cylindrical_joint(
        "Aligned shaft/bearing cylindrical joint",
        bearing_axis,
        shaft_axis,
        SpatialAxisParity::Aligned,
    )?;
    let hinge = assembly.add_hinge_coordinate("Shaft hinge coordinate", joint, 2)?;
    let translation =
        assembly.add_axial_translation_coordinate("Shaft axial translation", joint)?;
    let hinge_driver = assembly.add_hinge_position_driver(
        "Shaft angle driver",
        hinge,
        SpatialHingeTarget {
            principal_phase: 0.48,
            winding: 2,
        },
    )?;
    let translation_driver = assembly.add_translation_position_driver(
        "Shaft axial position driver",
        translation,
        1.9 * scale,
    )?;
    let parity_monitor = assembly.add_axis_parity_monitor(
        "Shaft aligned-axis mode",
        bearing_axis,
        shaft_axis,
        SpatialAxisParity::Aligned,
    )?;
    let winding_monitor = assembly.add_hinge_winding_monitor("Shaft winding 2 mode", hinge, 2)?;
    let side_monitor = assembly.add_plane_side_monitor(
        "Shaft positive translation side",
        translation_plane,
        translation_witness,
        SpatialModeSign::Positive,
    )?;

    Ok(SpatialExampleFixture {
        assembly,
        ids: SpatialExampleIds::ShaftBearing(ShaftBearingExampleIds {
            bodies: [bearing, shaft],
            frames: [bearing_frame, shaft_frame],
            axes: [bearing_axis, shaft_axis],
            translation_plane,
            translation_witness,
            joint,
            coordinates: [hinge, translation],
            drivers: [hinge_driver, translation_driver],
            monitors: [parity_monitor, winding_monitor, side_monitor],
        }),
    })
}

#[allow(clippy::too_many_lines)]
fn block_base_example(scale: f64) -> Result<SpatialExampleFixture, SpatialAssemblyError> {
    let mut assembly = SpatialAssembly::new(scale)?;
    let base_pose = Pose3::exp([-2.4 * scale, 1.1 * scale, 0.5 * scale, -0.21, 0.16, 0.28])?;
    let block_pose = Pose3::exp([1.3 * scale, -0.9 * scale, 0.7 * scale, 0.19, -0.27, 0.22])?;
    let datum_pose = Pose3::exp([0.4 * scale, 0.3 * scale, -0.2 * scale, 0.26, 0.18, -0.23])?;
    let base_world = frame_from_pose(datum_pose)?;
    let block_world = planar_offset_frame(base_world, 0.37, 1.25 * scale, -0.8 * scale)?;
    let base_local = local_frame(base_pose, base_world)?;
    let block_local = local_frame(block_pose, block_world)?;
    let witness_world = block_world.origin() + base_world.z_axis() * (0.9 * scale);
    let witness_local = block_pose.try_inverse_transform_point(witness_world)?;

    let base = assembly.add_body("Grounded base", base_pose)?;
    let block = assembly.add_body("Driven planar block", block_pose)?;
    let base_frame = assembly.add_frame_feature("Base drawing frame", base, base_local)?;
    let block_frame = assembly.add_frame_feature("Block drawing frame", block, block_local)?;
    let base_axis = assembly.add_axis_feature("Base normal axis", base, base_local)?;
    let block_axis = assembly.add_axis_feature("Block normal axis", block, block_local)?;
    let base_plane = assembly.add_plane_feature("Base mounting plane", base, base_local)?;
    let block_plane = assembly.add_plane_feature("Block mounting plane", block, block_local)?;
    let side_witness =
        assembly.add_point_feature("Block positive-side witness", block, witness_local)?;
    assembly.add_physical_ground("Base fixed to world", base)?;
    let joint = assembly.add_planar_joint(
        "Aligned block/base planar joint",
        base_plane,
        block_plane,
        SpatialAxisParity::Aligned,
    )?;
    let hinge = assembly.add_hinge_coordinate("Block normal rotation", joint, 3)?;
    let plane_x = assembly.add_planar_translation_coordinate(
        "Block plane-X coordinate",
        joint,
        SpatialPlanarTranslationAxis::X,
    )?;
    let plane_y = assembly.add_planar_translation_coordinate(
        "Block plane-Y coordinate",
        joint,
        SpatialPlanarTranslationAxis::Y,
    )?;
    let hinge_driver = assembly.add_hinge_position_driver(
        "Block rotation driver",
        hinge,
        SpatialHingeTarget {
            principal_phase: 0.37,
            winding: 3,
        },
    )?;
    let translation_drivers = [
        assembly.add_translation_position_driver("Block plane-X driver", plane_x, 1.25 * scale)?,
        assembly.add_translation_position_driver("Block plane-Y driver", plane_y, -0.8 * scale)?,
    ];
    let parity_monitor = assembly.add_axis_parity_monitor(
        "Block aligned-normal mode",
        base_axis,
        block_axis,
        SpatialAxisParity::Aligned,
    )?;
    let winding_monitor = assembly.add_hinge_winding_monitor("Block winding 3 mode", hinge, 3)?;
    let side_monitor = assembly.add_plane_side_monitor(
        "Block positive mounting side",
        base_plane,
        side_witness,
        SpatialModeSign::Positive,
    )?;

    Ok(SpatialExampleFixture {
        assembly,
        ids: SpatialExampleIds::BlockBase(BlockBaseExampleIds {
            bodies: [base, block],
            frames: [base_frame, block_frame],
            axes: [base_axis, block_axis],
            planes: [base_plane, block_plane],
            side_witness,
            joint,
            coordinates: [hinge, plane_x, plane_y],
            drivers: [hinge_driver, translation_drivers[0], translation_drivers[1]],
            monitors: [parity_monitor, winding_monitor, side_monitor],
        }),
    })
}

fn frame_from_pose(pose: Pose3) -> Result<Frame3, SpatialAssemblyError> {
    Ok(Frame3::try_new(
        Point3::from(pose.translation()),
        pose.try_transform_vector(Vector3::x())?,
        pose.try_transform_vector(Vector3::y())?,
        pose.try_transform_vector(Vector3::z())?,
    )?)
}

fn local_frame(body: Pose3, world: Frame3) -> Result<Frame3, SpatialAssemblyError> {
    Ok(Frame3::try_new(
        body.try_inverse_transform_point(world.origin())?,
        body.try_inverse_transform_vector(world.x_axis())?,
        body.try_inverse_transform_vector(world.y_axis())?,
        body.try_inverse_transform_vector(world.z_axis())?,
    )?)
}

fn rotated_translated_frame(
    frame: Frame3,
    angle: f64,
    translation: f64,
) -> Result<Frame3, SpatialAssemblyError> {
    let (sine, cosine) = angle.sin_cos();
    Ok(Frame3::try_new(
        frame.origin() + frame.z_axis() * translation,
        frame.x_axis() * cosine + frame.y_axis() * sine,
        -frame.x_axis() * sine + frame.y_axis() * cosine,
        frame.z_axis(),
    )?)
}

fn planar_offset_frame(
    frame: Frame3,
    angle: f64,
    x: f64,
    y: f64,
) -> Result<Frame3, SpatialAssemblyError> {
    let (sine, cosine) = angle.sin_cos();
    Ok(Frame3::try_new(
        frame.origin() + frame.x_axis() * x + frame.y_axis() * y,
        frame.x_axis() * cosine + frame.y_axis() * sine,
        -frame.x_axis() * sine + frame.y_axis() * cosine,
        frame.z_axis(),
    )?)
}
