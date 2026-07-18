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
