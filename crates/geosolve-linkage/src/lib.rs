//! Planar and spatial rigid-body kinematics compiled into the shared constraint solver.

mod compiler;
mod continuation;
mod model;
mod planar;
mod residuals;
mod scenarios;
mod spatial;
mod spatial_residuals;
mod velocity;

pub use compiler::{
    BodyVariableMapping, BranchEvaluation, BranchMonitorKind, CompiledLinkage, DriveResult,
    DriveSample, LinkageGeometry, LinkageSolveDiagnostics, LinkageSolveResult, LinkageSource,
    LinkageSourceMapping, SolveRejection, SolvedBody, TransformedAxisFeature,
    TransformedPointFeature,
};
pub use continuation::{
    AdaptiveContinuationMode, AdaptiveContinuationRequest, AdaptiveContinuationResult,
    AdaptiveContinuationSample, AdaptiveContinuationStatus, ContinuationDirection,
};
pub use model::{
    AxisDirectionBranch, AxisFeature, AxisFeatureId, BodyId, BranchMonitor, BranchMonitorId,
    BranchSign, BranchViolation, Driver, DriverId, DriverKind, DriverUnit, Joint, JointId,
    JointKind, Linkage, LinkageError, PointFeature, PointFeatureId, RigidBody,
};
pub use planar::{
    PLANAR_LINKAGE_DOCUMENT_VERSION, PlanarAxisFeature, PlanarBody, PlanarBodyId, PlanarBodyState,
    PlanarComponentGaugeReport, PlanarDocumentId, PlanarDriverState, PlanarFeatureId,
    PlanarGaugePolicy, PlanarGaugeReference, PlanarGaugeReport, PlanarLinkageAcceptedState,
    PlanarLinkageDocument, PlanarLinkageError, PlanarLinkageRuntimeMap, PlanarLinkageSession,
    PlanarLinkageTopology, PlanarPointFeature, PlanarRuntimeFeature, PlanarRuntimeSource,
    PlanarSource, PlanarSourceId, PlanarSourceKind, PlanarWorldActionCertification,
};
pub use scenarios::{
    FourBarAssemblyMode, FourBarIds, SliderCrankAssemblyMode, SliderCrankIds, four_bar,
    four_bar_crossed, four_bar_open, four_bar_with_scale, slider_crank,
    slider_crank_displacement_driven, slider_crank_displacement_driven_with_scale,
    slider_crank_with_scale, xy_plane_frame,
};
pub use spatial::{
    CompiledSpatialAssembly, SpatialAssembly, SpatialAssemblyError, SpatialAssemblySession,
    SpatialAxisParity, SpatialBody, SpatialBodyId, SpatialBodyVariableMapping,
    SpatialComponentGaugeReport, SpatialFrameFeature, SpatialFrameFeatureId, SpatialGaugePolicy,
    SpatialGaugeReference, SpatialGaugeReport, SpatialGeometry, SpatialPatch, SpatialPointFeature,
    SpatialPointFeatureId, SpatialSolveResult, SpatialSolvedBody, SpatialSource, SpatialSourceId,
    SpatialSourceKind, SpatialSourceMapping, SpatialTransformedFrameFeature,
    SpatialTransformedPointFeature, SpatialWorldActionCertification,
};
pub use velocity::{BodyVelocity, VelocityResult};

/// Hardcoded scenarios required by native tests and the browser demonstration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkageScenario {
    FourBarOpen,
    FourBarCrossed,
    SliderCrank,
}
