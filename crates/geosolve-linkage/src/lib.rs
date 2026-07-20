//! Planar and spatial rigid-body kinematics compiled into the shared constraint solver.

mod compiler;
mod continuation;
mod model;
mod planar;
mod residuals;
mod scenarios;
mod spatial;
mod spatial_residuals;
mod spatial_scenarios;
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
    CompiledSpatialAssembly, SPATIAL_ASSEMBLY_DOCUMENT_VERSION, SPATIAL_BOUNDARY_ENTER_CLEARANCE,
    SPATIAL_BOUNDARY_LEAVE_CLEARANCE, SpatialAdaptiveContinuationRequest,
    SpatialAdaptiveContinuationResult, SpatialAdaptiveContinuationSample,
    SpatialAdaptiveContinuationStatus, SpatialAssembly, SpatialAssemblyDocument,
    SpatialAssemblyDocumentSession, SpatialAssemblyEdit, SpatialAssemblyError,
    SpatialAssemblyModeChange, SpatialAssemblyRuntimeMap, SpatialAssemblySession,
    SpatialAssemblyTransaction, SpatialAxisFeature, SpatialAxisFeatureId, SpatialAxisParity,
    SpatialAxisVelocity, SpatialBody, SpatialBodyId, SpatialBodyVariableMapping,
    SpatialBodyVelocity, SpatialBoundaryHysteresisState, SpatialBoundaryObservation,
    SpatialBoundaryTransition, SpatialBranchBoundary, SpatialBranchBoundaryEvaluation,
    SpatialBranchBoundaryEvent, SpatialComponentGaugeReport, SpatialCoordinate,
    SpatialCoordinateId, SpatialCoordinateKind, SpatialCoordinateRate, SpatialCoordinateRateKind,
    SpatialCoordinateValue, SpatialCoordinateValueKind, SpatialDocumentError, SpatialDocumentId,
    SpatialDriverRate, SpatialFrameAxis, SpatialFrameFeature, SpatialFrameFeatureId,
    SpatialFrameVelocity, SpatialGaugePolicy, SpatialGaugeReference, SpatialGaugeReport,
    SpatialGeometry, SpatialHingeCoordinateValue, SpatialHingeTarget, SpatialModeChangeTransaction,
    SpatialModeEvaluation, SpatialModeFeature, SpatialModeMonitor, SpatialModeMonitorId,
    SpatialModeMonitorKind, SpatialModeSign, SpatialMotionBasisVector,
    SpatialNormalizedBodyTangent, SpatialPatch, SpatialPersistentId, SpatialPlanarTranslationAxis,
    SpatialPlaneFeature, SpatialPlaneFeatureId, SpatialPlaneVelocity, SpatialPointFeature,
    SpatialPointFeatureId, SpatialPointVelocity, SpatialPrincipalCutDirection,
    SpatialRuntimeFeature, SpatialSolveResult, SpatialSolvedBody, SpatialSource, SpatialSourceId,
    SpatialSourceKind, SpatialSourceMapping, SpatialTransformedAxisFeature,
    SpatialTransformedFrameFeature, SpatialTransformedPlaneFeature, SpatialTransformedPointFeature,
    SpatialVelocityInconsistency, SpatialVelocityOptions, SpatialVelocityOutcome,
    SpatialVelocitySolution, SpatialWorldActionCertification,
};
pub use spatial_scenarios::{
    BlockBaseExampleIds, EmbeddedSpatialSliderCrankFixture, EmbeddedSpatialSliderCrankIds,
    ShaftBearingExampleIds, SpatialExampleFixture, SpatialExampleIds, SpatialExampleKind,
    embedded_spatial_slider_crank, spatial_example,
};
pub use velocity::{BodyVelocity, VelocityResult};

/// Hardcoded scenarios required by native tests and the browser demonstration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkageScenario {
    FourBarOpen,
    FourBarCrossed,
    SliderCrank,
}
