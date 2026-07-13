//! Planar rigid-body linkage model built on the shared solver kernel.

use geosolve_geometry::Pose2;
use slotmap::new_key_type;

new_key_type! {
    pub struct BodyId;
    pub struct JointId;
    pub struct DriverId;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigidBody2 {
    pub pose: Pose2,
    pub grounded: bool,
}

/// Hardcoded scenarios required by the browser demonstration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkageScenario {
    FourBarOpen,
    FourBarCrossed,
    SliderCrank,
}
