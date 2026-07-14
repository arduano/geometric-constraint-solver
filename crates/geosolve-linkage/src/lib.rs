//! Planar rigid-body linkages compiled into the shared constraint solver.

mod compiler;
mod model;
mod residuals;
mod scenarios;
mod velocity;

pub use compiler::{
    BodyVariableMapping, CompiledLinkage, DriveResult, DriveSample, LinkageGeometry,
    LinkageSolveDiagnostics, LinkageSolveResult, LinkageSource, LinkageSourceMapping,
    SolveRejection, SolvedBody, TransformedAxisFeature, TransformedPointFeature,
};
pub use model::{
    AxisDirectionBranch, AxisFeature, AxisFeatureId, BodyId, BranchMonitor, BranchMonitorId,
    BranchSign, BranchViolation, Driver, DriverId, DriverKind, DriverUnit, Joint, JointId,
    JointKind, Linkage, LinkageError, PointFeature, PointFeatureId, RigidBody,
};
pub use scenarios::{
    FourBarAssemblyMode, FourBarIds, SliderCrankAssemblyMode, SliderCrankIds, four_bar,
    four_bar_crossed, four_bar_open, four_bar_with_scale, slider_crank, slider_crank_with_scale,
    xy_plane_frame,
};
pub use velocity::{BodyVelocity, VelocityResult};

/// Hardcoded scenarios required by native tests and the browser demonstration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkageScenario {
    FourBarOpen,
    FourBarCrossed,
    SliderCrank,
}
