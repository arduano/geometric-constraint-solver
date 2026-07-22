use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, AuditSnapshot, ContinuationError, CoreError, HardValidity,
    LinearSolveBackend, Problem, ResidualBlock, ResidualCategory, ResidualId, ResidualRowAudit,
    SessionError, SolveReport, SolveSession, SolverConfig, SourceConstraint, SourceConstraintId,
    SparseFallbackReason, VariableBlock, VariableId, VariableKind, VariableValue,
};
use geosolve_geometry::{Frame3, GeometryError, Point3, Pose3, Vector3};
use thiserror::Error;

use crate::spatial_residuals::{
    ParameterizedSpatialHingePositionResidual, ParameterizedSpatialTranslationPositionResidual,
    SpatialAxisAlignmentResidual, SpatialAxisAngleResidual, SpatialBallResidual,
    SpatialFixedFrameResidual, SpatialHingePositionResidual, SpatialPointDistanceResidual,
    SpatialRelationKind, SpatialRelationResidual, SpatialRevoluteResidual,
    SpatialTranslationPositionResidual,
};

#[path = "spatial_continuation.rs"]
mod continuation;
#[path = "spatial_document.rs"]
mod document;
#[path = "spatial_velocity.rs"]
mod velocity;

pub use continuation::{
    SpatialAdaptiveContinuationRequest, SpatialAdaptiveContinuationResult,
    SpatialAdaptiveContinuationSample, SpatialAdaptiveContinuationStatus,
};
pub use document::{
    SPATIAL_ASSEMBLY_DOCUMENT_VERSION, SpatialAssemblyDocument, SpatialAssemblyDocumentSession,
    SpatialAssemblyRuntimeMap, SpatialDocumentError, SpatialDocumentId, SpatialPersistentId,
    SpatialRuntimeFeature,
};
pub use velocity::{
    SpatialAxisVelocity, SpatialBodyVelocity, SpatialCoordinateRate, SpatialCoordinateRateKind,
    SpatialDriverRate, SpatialFrameVelocity, SpatialMotionBasisVector,
    SpatialNormalizedBodyTangent, SpatialPlaneVelocity, SpatialPointVelocity,
    SpatialVelocityInconsistency, SpatialVelocityOptions, SpatialVelocityOutcome,
    SpatialVelocitySolution,
};

pub(crate) const ORIENTATION_BRANCH_MARGIN: f64 = 1.0e-3;
const SPATIAL_ACCEPTANCE_TOLERANCE: f64 = 1.0e-9;
/// Clearance at or below which a retained spatial branch enters the event band.
pub const SPATIAL_BOUNDARY_ENTER_CLEARANCE: f64 = 2.0e-3;
/// Clearance at or above which a near-boundary spatial branch becomes clear again.
pub const SPATIAL_BOUNDARY_LEAVE_CLEARANCE: f64 = 4.0e-3;
static NEXT_SPATIAL_ASSEMBLY_NAMESPACE: AtomicU64 = AtomicU64::new(1);

trait SpatialIdValue: Copy {
    fn ordinal(self) -> u64;
    fn belongs_to_namespace(self, namespace: u64) -> bool;
}

macro_rules! spatial_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            namespace: u64,
            ordinal: u64,
        }

        impl $name {
            const fn new(namespace: u64, ordinal: u64) -> Self {
                Self { namespace, ordinal }
            }

            #[must_use]
            pub const fn as_u64(self) -> u64 {
                self.ordinal
            }

            const fn belongs_to(self, namespace: u64) -> bool {
                self.namespace == namespace
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.ordinal)
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.ordinal.fmt(formatter)
            }
        }

        impl SpatialIdValue for $name {
            fn ordinal(self) -> u64 {
                self.ordinal
            }

            fn belongs_to_namespace(self, namespace: u64) -> bool {
                self.belongs_to(namespace)
            }
        }
    };
}

spatial_id!(SpatialBodyId, "Opaque spatial rigid-body identity.");
spatial_id!(
    SpatialPointFeatureId,
    "Opaque body-local spatial point-feature identity."
);
spatial_id!(
    SpatialFrameFeatureId,
    "Opaque body-local spatial frame-feature identity."
);
spatial_id!(
    SpatialAxisFeatureId,
    "Opaque body-local spatial axis-feature identity."
);
spatial_id!(
    SpatialPlaneFeatureId,
    "Opaque body-local spatial plane-feature identity."
);
spatial_id!(SpatialSourceId, "Opaque spatial physical-source identity.");
spatial_id!(
    SpatialCoordinateId,
    "Opaque spatial position-coordinate identity in the assembly-wide ID space."
);
spatial_id!(
    SpatialModeMonitorId,
    "Opaque monitor-only spatial assembly-mode identity in the assembly-wide ID space."
);

/// Construction, compilation, gauge, solve, or independent-validation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SpatialAssemblyError {
    #[error("spatial model scale must be positive and finite, got {value}")]
    InvalidModelScale { value: f64 },
    #[error("spatial {field} label must not be empty")]
    InvalidLabel { field: &'static str },
    #[error("invalid spatial assembly field {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("unknown spatial body reference {0}")]
    UnknownBody(SpatialBodyId),
    #[error("unknown spatial point-feature reference {0}")]
    UnknownPointFeature(SpatialPointFeatureId),
    #[error("unknown spatial frame-feature reference {0}")]
    UnknownFrameFeature(SpatialFrameFeatureId),
    #[error("unknown spatial axis-feature reference {0}")]
    UnknownAxisFeature(SpatialAxisFeatureId),
    #[error("unknown spatial plane-feature reference {0}")]
    UnknownPlaneFeature(SpatialPlaneFeatureId),
    #[error("unknown spatial source reference {0}")]
    UnknownSource(SpatialSourceId),
    #[error("unknown spatial coordinate reference {0}")]
    UnknownCoordinate(SpatialCoordinateId),
    #[error("unknown spatial mode-monitor reference {0}")]
    UnknownModeMonitor(SpatialModeMonitorId),
    #[error("spatial source {source_id} is not a valid {expected} coordinate parent")]
    WrongCoordinateParent {
        source_id: SpatialSourceId,
        expected: &'static str,
    },
    #[error("spatial coordinate {coordinate} is not a {expected} coordinate")]
    WrongCoordinateKind {
        coordinate: SpatialCoordinateId,
        expected: &'static str,
    },
    #[error("spatial source {source_id} does not support {expected}")]
    WrongSourceKind {
        source_id: SpatialSourceId,
        expected: &'static str,
    },
    #[error("spatial mode monitor {monitor_id} does not support {expected}")]
    WrongModeMonitorKind {
        monitor_id: SpatialModeMonitorId,
        expected: &'static str,
    },
    #[error(
        "hinge winding mismatch for coordinate {coordinate}: coordinate {coordinate_winding}, target {target_winding}"
    )]
    WindingMismatch {
        coordinate: SpatialCoordinateId,
        coordinate_winding: i64,
        target_winding: i64,
    },
    #[error("spatial transaction repeats {role} edit for {id}")]
    DuplicateEdit { role: &'static str, id: String },
    #[error("spatial coordinate {coordinate} has incompatible simultaneous driver targets")]
    IncompatibleDriverTargets { coordinate: SpatialCoordinateId },
    #[error("spatial body {0} is physically grounded more than once")]
    DuplicateGround(SpatialBodyId),
    #[error("spatial joint endpoints must belong to different bodies, got {0}")]
    SameBodyJointEndpoints(SpatialBodyId),
    #[error("spatial ID space is exhausted")]
    IdExhausted,
    #[error("invalid spatial gauge policy: {0}")]
    InvalidGaugePolicy(String),
    #[error("spatial gauge certification failed: {0}")]
    GaugeCertification(String),
    #[error("stale spatial assembly revision: expected {expected}, actual {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("spatial assembly revision is exhausted")]
    RevisionExhausted,
    #[error("spatial independent validation failed: {0}")]
    IndependentValidation(String),
    #[error("initial spatial assembly was rejected: {0}")]
    InitialRejected(String),
    #[error(transparent)]
    Continuation(#[from] ContinuationError),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

/// Explicit directed-axis or plane-normal relationship retained by a spatial source.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialAxisParity {
    Aligned,
    Opposed,
}

impl SpatialAxisParity {
    #[must_use]
    pub const fn multiplier(self) -> f64 {
        match self {
            Self::Aligned => 1.0,
            Self::Opposed => -1.0,
        }
    }
}

/// Explicit positive or negative side/orientation state for a spatial mode monitor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialModeSign {
    Positive,
    Negative,
}

impl SpatialModeSign {
    #[must_use]
    pub const fn multiplier(self) -> f64 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

/// Canonical target for a winding-retaining hinge coordinate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialHingeTarget {
    pub principal_phase: f64,
    pub winding: i64,
}

/// First-plane in-plane axis selected by a planar translation coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialPlanarTranslationAxis {
    X,
    Y,
}

/// One topology-only coordinate definition. Coordinates add no core rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialCoordinateKind {
    Hinge {
        parent: SpatialSourceId,
        winding: i64,
    },
    AxialTranslation {
        parent: SpatialSourceId,
    },
    PlanarTranslation {
        parent: SpatialSourceId,
        axis: SpatialPlanarTranslationAxis,
    },
}

/// One spatial coordinate in deterministic insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialCoordinate {
    id: SpatialCoordinateId,
    label: String,
    kind: SpatialCoordinateKind,
}

impl SpatialCoordinate {
    #[must_use]
    pub const fn id(&self) -> SpatialCoordinateId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> SpatialCoordinateKind {
        self.kind
    }

    #[must_use]
    pub const fn definition(&self) -> SpatialCoordinateKind {
        self.kind
    }
}

/// Accepted finite hinge value with winding retained outside differentiation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialHingeCoordinateValue {
    pub principal_phase: f64,
    pub winding: i64,
}

/// Accepted value payload for one concrete spatial coordinate kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialCoordinateValueKind {
    Hinge(SpatialHingeCoordinateValue),
    AxialTranslation(f64),
    PlanarTranslation {
        axis: SpatialPlanarTranslationAxis,
        value: f64,
    },
}

/// One accepted coordinate value in coordinate insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialCoordinateValue {
    pub coordinate: SpatialCoordinateId,
    pub coordinate_label: String,
    pub value: SpatialCoordinateValueKind,
}

/// One explicit monitor-only spatial assembly-mode definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialModeMonitorKind {
    AxisParity {
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    },
    HingeWinding {
        coordinate: SpatialCoordinateId,
        winding: i64,
    },
    PlaneSide {
        plane: SpatialPlaneFeatureId,
        point: SpatialPointFeatureId,
        side: SpatialModeSign,
    },
    SignedVolume {
        points: [SpatialPointFeatureId; 4],
        orientation: SpatialModeSign,
    },
}

/// One explicit monitor-only spatial assembly mode in deterministic insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialModeMonitor {
    id: SpatialModeMonitorId,
    label: String,
    kind: SpatialModeMonitorKind,
}

impl SpatialModeMonitor {
    #[must_use]
    pub const fn id(&self) -> SpatialModeMonitorId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> SpatialModeMonitorKind {
        self.kind
    }

    #[must_use]
    pub const fn definition(&self) -> SpatialModeMonitorKind {
        self.kind
    }
}

/// Typed feature identity involved in an accepted spatial mode evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialModeFeature {
    Point(SpatialPointFeatureId),
    Frame(SpatialFrameFeatureId),
    Axis(SpatialAxisFeatureId),
    Plane(SpatialPlaneFeatureId),
}

/// Fresh finite evaluation of one retained monitor-only assembly mode.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialModeEvaluation {
    pub monitor_id: SpatialModeMonitorId,
    pub monitor_label: String,
    pub kind: SpatialModeMonitorKind,
    /// Fresh dot, signed distance, principal phase, or normalized triple product.
    pub fresh_raw_metric: Option<f64>,
    /// Selected signed metric, or clock-projection magnitude for winding state.
    pub retained_normalized_metric: f64,
    pub retained: bool,
    pub involved_bodies: Vec<SpatialBodyId>,
    pub involved_features: Vec<SpatialModeFeature>,
    pub coordinate: Option<SpatialCoordinateId>,
    pub winding: Option<i64>,
}

/// Accepted hysteresis latch for one finite spatial branch-boundary metric.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialBoundaryHysteresisState {
    Clear,
    Near,
}

/// One local frame axis used to identify an orientation false-root boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialFrameAxis {
    X,
    Y,
    Z,
}

/// Typed spatial branch or false-root boundary monitored without adding equations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialBranchBoundary {
    FixedFrameDiagonal {
        source: SpatialSourceId,
        axis: SpatialFrameAxis,
    },
    FrameOffsetDiagonal {
        source: SpatialSourceId,
        axis: SpatialFrameAxis,
    },
    SourceAxisParity {
        source: SpatialSourceId,
        parity: SpatialAxisParity,
    },
    PrismaticClockRoot {
        source: SpatialSourceId,
    },
    HingeDriverPositiveRoot {
        source: SpatialSourceId,
        coordinate: SpatialCoordinateId,
    },
    HingePrincipalCut {
        coordinate: SpatialCoordinateId,
        winding: i64,
    },
    MonitorAxisParity {
        monitor: SpatialModeMonitorId,
        parity: SpatialAxisParity,
    },
    MonitorPlaneSide {
        monitor: SpatialModeMonitorId,
        side: SpatialModeSign,
    },
    MonitorSignedVolume {
        monitor: SpatialModeMonitorId,
        orientation: SpatialModeSign,
    },
}

/// One finite accepted boundary metric and its retained hysteresis latch.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialBranchBoundaryEvaluation {
    pub boundary: SpatialBranchBoundary,
    pub raw_metric: f64,
    pub clearance: f64,
    pub hysteresis_state: SpatialBoundaryHysteresisState,
}

/// Kind of one spatial branch-boundary event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialBoundaryTransition {
    Entered,
    Left,
    /// A predictor attempted to cross a known periodic cut without a mode change.
    CrossingAttempted,
}

/// Endpoint at which one spatial boundary transition was observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialBoundaryObservation {
    PredictorEndpoint,
    CorrectedPhysicalEndpoint,
}

/// One typed finite boundary event emitted by spatial continuation.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialBranchBoundaryEvent {
    pub boundary: SpatialBranchBoundary,
    pub transition: SpatialBoundaryTransition,
    pub observation: SpatialBoundaryObservation,
    pub previous_clearance: f64,
    pub clearance: f64,
    pub raw_metric: f64,
}

/// Numerical coordinate policy, separate from physical grounding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SpatialGaugePolicy {
    /// Fix the lowest body ID in each certified floating component privately.
    #[default]
    LowestPersistentBody,
    /// Select exactly one reference for every floating component.
    ExplicitReferences { bodies: Vec<SpatialBodyId> },
}

/// One spatial rigid body and its current accepted or staged pose guess.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialBody {
    id: SpatialBodyId,
    label: String,
    pose_guess: Pose3,
}

impl SpatialBody {
    #[must_use]
    pub const fn id(&self) -> SpatialBodyId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn pose_guess(&self) -> Pose3 {
        self.pose_guess
    }
}

/// One body-local point feature.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialPointFeature {
    id: SpatialPointFeatureId,
    label: String,
    body: SpatialBodyId,
    local_point: Point3<f64>,
}

impl SpatialPointFeature {
    #[must_use]
    pub const fn id(&self) -> SpatialPointFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> SpatialBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_point(&self) -> Point3<f64> {
        self.local_point
    }
}

/// One validated body-local coordinate frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialFrameFeature {
    id: SpatialFrameFeatureId,
    label: String,
    body: SpatialBodyId,
    local_frame: Frame3,
}

impl SpatialFrameFeature {
    #[must_use]
    pub const fn id(&self) -> SpatialFrameFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> SpatialBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_frame(&self) -> Frame3 {
        self.local_frame
    }
}

/// One body-local directed axis with a persistent transverse clock.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialAxisFeature {
    id: SpatialAxisFeatureId,
    label: String,
    body: SpatialBodyId,
    local_frame: Frame3,
}

impl SpatialAxisFeature {
    #[must_use]
    pub const fn id(&self) -> SpatialAxisFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> SpatialBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_frame(&self) -> Frame3 {
        self.local_frame
    }

    #[must_use]
    pub fn local_origin(&self) -> Point3<f64> {
        self.local_frame.origin()
    }

    #[must_use]
    pub fn local_axis(&self) -> Vector3<f64> {
        self.local_frame.z_axis()
    }

    #[must_use]
    pub fn local_x_clock(&self) -> Vector3<f64> {
        self.local_frame.x_axis()
    }

    #[must_use]
    pub fn local_y_clock(&self) -> Vector3<f64> {
        self.local_frame.y_axis()
    }
}

/// One body-local directed plane normal with a persistent in-plane clock.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialPlaneFeature {
    id: SpatialPlaneFeatureId,
    label: String,
    body: SpatialBodyId,
    local_frame: Frame3,
}

impl SpatialPlaneFeature {
    #[must_use]
    pub const fn id(&self) -> SpatialPlaneFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> SpatialBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_frame(&self) -> Frame3 {
        self.local_frame
    }

    #[must_use]
    pub fn local_origin(&self) -> Point3<f64> {
        self.local_frame.origin()
    }

    #[must_use]
    pub fn local_normal(&self) -> Vector3<f64> {
        self.local_frame.z_axis()
    }

    #[must_use]
    pub fn local_x_clock(&self) -> Vector3<f64> {
        self.local_frame.x_axis()
    }

    #[must_use]
    pub fn local_y_clock(&self) -> Vector3<f64> {
        self.local_frame.y_axis()
    }
}

/// One physical spatial equation source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialSourceKind {
    PhysicalGround {
        body: SpatialBodyId,
        target_pose: Pose3,
    },
    BallJoint {
        first: SpatialPointFeatureId,
        second: SpatialPointFeatureId,
    },
    FixedFrame {
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
    },
    RevoluteJoint {
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
        parity: SpatialAxisParity,
    },
    PrismaticJoint {
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    },
    CylindricalJoint {
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    },
    PlanarJoint {
        first: SpatialPlaneFeatureId,
        second: SpatialPlaneFeatureId,
        parity: SpatialAxisParity,
    },
    UniversalJoint {
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
    },
    PointDistanceMate {
        first: SpatialPointFeatureId,
        second: SpatialPointFeatureId,
        distance: f64,
    },
    AxisAngleMate {
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        angle: f64,
    },
    AxisAlignmentMate {
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    },
    FrameOffsetMate {
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
        offset: Frame3,
    },
    HingePositionDriver {
        coordinate: SpatialCoordinateId,
        target: SpatialHingeTarget,
    },
    TranslationPositionDriver {
        coordinate: SpatialCoordinateId,
        target: f64,
    },
}

/// One physical source in deterministic insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialSource {
    id: SpatialSourceId,
    label: String,
    kind: SpatialSourceKind,
}

impl SpatialSource {
    #[must_use]
    pub const fn id(&self) -> SpatialSourceId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> SpatialSourceKind {
        self.kind
    }

    #[must_use]
    pub const fn definition(&self) -> SpatialSourceKind {
        self.kind
    }
}

/// Minimal in-memory spatial assembly definition and accepted pose state.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialAssembly {
    namespace: u64,
    model_scale: f64,
    revision: u64,
    next_id: u64,
    gauge_policy: SpatialGaugePolicy,
    bodies: Vec<SpatialBody>,
    point_features: Vec<SpatialPointFeature>,
    frame_features: Vec<SpatialFrameFeature>,
    axis_features: Vec<SpatialAxisFeature>,
    plane_features: Vec<SpatialPlaneFeature>,
    coordinates: Vec<SpatialCoordinate>,
    mode_monitors: Vec<SpatialModeMonitor>,
    sources: Vec<SpatialSource>,
}

impl SpatialAssembly {
    /// Creates an empty spatial assembly.
    ///
    /// # Errors
    ///
    /// Returns an error unless `model_scale` is positive and finite.
    pub fn new(model_scale: f64) -> Result<Self, SpatialAssemblyError> {
        validate_model_scale(model_scale)?;
        Ok(Self {
            namespace: allocate_spatial_assembly_namespace()?,
            model_scale,
            revision: 0,
            next_id: 1,
            gauge_policy: SpatialGaugePolicy::default(),
            bodies: Vec::new(),
            point_features: Vec::new(),
            frame_features: Vec::new(),
            axis_features: Vec::new(),
            plane_features: Vec::new(),
            coordinates: Vec::new(),
            mode_monitors: Vec::new(),
            sources: Vec::new(),
        })
    }

    #[must_use]
    pub const fn model_scale(&self) -> f64 {
        self.model_scale
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn gauge_policy(&self) -> &SpatialGaugePolicy {
        &self.gauge_policy
    }

    #[must_use]
    pub fn bodies(&self) -> &[SpatialBody] {
        &self.bodies
    }

    #[must_use]
    pub fn point_features(&self) -> &[SpatialPointFeature] {
        &self.point_features
    }

    #[must_use]
    pub fn frame_features(&self) -> &[SpatialFrameFeature] {
        &self.frame_features
    }

    #[must_use]
    pub fn axis_features(&self) -> &[SpatialAxisFeature] {
        &self.axis_features
    }

    #[must_use]
    pub fn plane_features(&self) -> &[SpatialPlaneFeature] {
        &self.plane_features
    }

    #[must_use]
    pub fn coordinates(&self) -> &[SpatialCoordinate] {
        &self.coordinates
    }

    #[must_use]
    pub fn mode_monitors(&self) -> &[SpatialModeMonitor] {
        &self.mode_monitors
    }

    #[must_use]
    pub fn sources(&self) -> &[SpatialSource] {
        &self.sources
    }

    #[must_use]
    pub fn body(&self, id: SpatialBodyId) -> Option<&SpatialBody> {
        self.bodies.iter().find(|body| body.id == id)
    }

    #[must_use]
    pub fn point_feature(&self, id: SpatialPointFeatureId) -> Option<&SpatialPointFeature> {
        self.point_features.iter().find(|feature| feature.id == id)
    }

    #[must_use]
    pub fn frame_feature(&self, id: SpatialFrameFeatureId) -> Option<&SpatialFrameFeature> {
        self.frame_features.iter().find(|feature| feature.id == id)
    }

    #[must_use]
    pub fn axis_feature(&self, id: SpatialAxisFeatureId) -> Option<&SpatialAxisFeature> {
        self.axis_features.iter().find(|feature| feature.id == id)
    }

    #[must_use]
    pub fn plane_feature(&self, id: SpatialPlaneFeatureId) -> Option<&SpatialPlaneFeature> {
        self.plane_features.iter().find(|feature| feature.id == id)
    }

    #[must_use]
    pub fn source(&self, id: SpatialSourceId) -> Option<&SpatialSource> {
        self.sources.iter().find(|source| source.id == id)
    }

    #[must_use]
    pub fn coordinate(&self, id: SpatialCoordinateId) -> Option<&SpatialCoordinate> {
        self.coordinates
            .iter()
            .find(|coordinate| coordinate.id == id)
    }

    #[must_use]
    pub fn mode_monitor(&self, id: SpatialModeMonitorId) -> Option<&SpatialModeMonitor> {
        self.mode_monitors.iter().find(|monitor| monitor.id == id)
    }

    /// Adds one body with a finite manifold pose guess.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, invalid pose, or exhausted ID space.
    pub fn add_body(
        &mut self,
        label: impl Into<String>,
        pose_guess: Pose3,
    ) -> Result<SpatialBodyId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "body")?;
        validate_pose(pose_guess)?;
        let id = SpatialBodyId::new(self.namespace, self.allocate_id()?);
        self.bodies.push(SpatialBody {
            id,
            label,
            pose_guess,
        });
        Ok(id)
    }

    /// Adds one finite body-local point feature.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, non-finite point, or exhausted ID space.
    pub fn add_point_feature(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
        local_point: Point3<f64>,
    ) -> Result<SpatialPointFeatureId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "point feature")?;
        self.require_body(body)?;
        validate_point(local_point, "point_feature.local_point")?;
        let id = SpatialPointFeatureId::new(self.namespace, self.allocate_id()?);
        self.point_features.push(SpatialPointFeature {
            id,
            label,
            body,
            local_point,
        });
        Ok(id)
    }

    /// Adds one validated body-local frame feature.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, invalid frame, or exhausted ID space.
    pub fn add_frame_feature(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
        local_frame: Frame3,
    ) -> Result<SpatialFrameFeatureId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "frame feature")?;
        self.require_body(body)?;
        let local_frame = revalidate_frame(local_frame)?;
        let id = SpatialFrameFeatureId::new(self.namespace, self.allocate_id()?);
        self.frame_features.push(SpatialFrameFeature {
            id,
            label,
            body,
            local_frame,
        });
        Ok(id)
    }

    /// Adds one validated body-local directed axis and persistent transverse clock.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, invalid frame, or exhausted ID space.
    pub fn add_axis_feature(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
        local_frame: Frame3,
    ) -> Result<SpatialAxisFeatureId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "axis feature")?;
        self.require_body(body)?;
        let local_frame = revalidate_frame(local_frame)?;
        let id = SpatialAxisFeatureId::new(self.namespace, self.allocate_id()?);
        self.axis_features.push(SpatialAxisFeature {
            id,
            label,
            body,
            local_frame,
        });
        Ok(id)
    }

    /// Adds one validated body-local plane and persistent in-plane clock.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, invalid frame, or exhausted ID space.
    pub fn add_plane_feature(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
        local_frame: Frame3,
    ) -> Result<SpatialPlaneFeatureId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "plane feature")?;
        self.require_body(body)?;
        let local_frame = revalidate_frame(local_frame)?;
        let id = SpatialPlaneFeatureId::new(self.namespace, self.allocate_id()?);
        self.plane_features.push(SpatialPlaneFeature {
            id,
            label,
            body,
            local_frame,
        });
        Ok(id)
    }

    /// Adds a physical fixed-pose source whose target is captured immediately.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, duplicate physical ground, or exhausted ID space.
    pub fn add_physical_ground(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "physical ground")?;
        let target_pose = self.require_body(body)?.pose_guess;
        if self.sources.iter().any(|source| {
            matches!(source.kind, SpatialSourceKind::PhysicalGround { body: existing, .. } if existing == body)
        }) {
            return Err(SpatialAssemblyError::DuplicateGround(body));
        }
        self.add_source_record(
            label,
            SpatialSourceKind::PhysicalGround { body, target_pose },
        )
    }

    /// Adds a coincident-point ball joint.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_ball_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialPointFeatureId,
        second: SpatialPointFeatureId,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "ball joint")?;
        let first_body = self.require_point_feature(first)?.body;
        let second_body = self.require_point_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(label, SpatialSourceKind::BallJoint { first, second })
    }

    /// Adds a coincident, identically oriented fixed-frame relationship.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_fixed_frame(
        &mut self,
        label: impl Into<String>,
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "fixed frame")?;
        let first_body = self.require_frame_feature(first)?.body;
        let second_body = self.require_frame_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(label, SpatialSourceKind::FixedFrame { first, second })
    }

    /// Adds a revolute joint about the local frame z axes with explicit parity.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_revolute_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
        parity: SpatialAxisParity,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "revolute joint")?;
        let first_body = self.require_frame_feature(first)?.body;
        let second_body = self.require_frame_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(
            label,
            SpatialSourceKind::RevoluteJoint {
                first,
                second,
                parity,
            },
        )
    }

    /// Adds a one-translation-DOF joint between two clocked axis features.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_prismatic_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "prismatic joint")?;
        let first_body = self.require_axis_feature(first)?.body;
        let second_body = self.require_axis_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(
            label,
            SpatialSourceKind::PrismaticJoint {
                first,
                second,
                parity,
            },
        )
    }

    /// Adds a translation-and-rotation joint between two clocked axis features.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_cylindrical_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "cylindrical joint")?;
        let first_body = self.require_axis_feature(first)?.body;
        let second_body = self.require_axis_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(
            label,
            SpatialSourceKind::CylindricalJoint {
                first,
                second,
                parity,
            },
        )
    }

    /// Adds a three-DOF in-plane relationship between two clocked planes.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_planar_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialPlaneFeatureId,
        second: SpatialPlaneFeatureId,
        parity: SpatialAxisParity,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "planar joint")?;
        let first_body = self.require_plane_feature(first)?.body;
        let second_body = self.require_plane_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(
            label,
            SpatialSourceKind::PlanarJoint {
                first,
                second,
                parity,
            },
        )
    }

    /// Adds a coincident-origin, orthogonal-axis universal joint.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_universal_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "universal joint")?;
        let first_body = self.require_axis_feature(first)?.body;
        let second_body = self.require_axis_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(label, SpatialSourceKind::UniversalJoint { first, second })
    }

    /// Adds one regular point-to-point distance equation.
    ///
    /// Zero is intentionally excluded because a ball joint is the explicit
    /// codimension-three coincidence API.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, a nonpositive or
    /// non-finite target, coincident/non-finite candidate points, or exhausted ID space.
    pub fn add_point_distance_mate(
        &mut self,
        label: impl Into<String>,
        first: SpatialPointFeatureId,
        second: SpatialPointFeatureId,
        distance: f64,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "point distance mate")?;
        let first_feature = self.require_point_feature(first)?;
        let second_feature = self.require_point_feature(second)?;
        require_distinct_bodies(first_feature.body, second_feature.body)?;
        validate_positive_distance(distance)?;
        validate_point_distance_candidate(self, first_feature, second_feature)?;
        self.add_source_record(
            label,
            SpatialSourceKind::PointDistanceMate {
                first,
                second,
                distance,
            },
        )
    }

    /// Adds one directed-axis interior-angle equation in explicit feature order.
    ///
    /// The exactly representable endpoints `0.0` and `PI` are excluded because
    /// their dot-product derivative is singular. Use an axis-alignment mate with
    /// explicit parity for those relationships; this constructor never infers a branch.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, an angle outside
    /// the finite open interval `(0, PI)`, or exhausted ID space.
    pub fn add_axis_angle_mate(
        &mut self,
        label: impl Into<String>,
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        angle: f64,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "axis angle mate")?;
        let first_body = self.require_axis_feature(first)?.body;
        let second_body = self.require_axis_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        validate_interior_angle(angle)?;
        self.add_source_record(
            label,
            SpatialSourceKind::AxisAngleMate {
                first,
                second,
                angle,
            },
        )
    }

    /// Adds two direction-only axis-alignment rows with explicit directed parity.
    ///
    /// This rank-two mate does not impose coaxial placement and is distinct from
    /// a cylindrical joint.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_axis_alignment_mate(
        &mut self,
        label: impl Into<String>,
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "axis alignment mate")?;
        let first_body = self.require_axis_feature(first)?.body;
        let second_body = self.require_axis_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(
            label,
            SpatialSourceKind::AxisAlignmentMate {
                first,
                second,
                parity,
            },
        )
    }

    /// Adds a complete relative frame target expressed in the first feature frame.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, an invalid or
    /// non-composable offset frame, or exhausted ID space.
    pub fn add_frame_offset_mate(
        &mut self,
        label: impl Into<String>,
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
        offset: Frame3,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "frame offset mate")?;
        let first_feature = self.require_frame_feature(first)?;
        let second_feature = self.require_frame_feature(second)?;
        require_distinct_bodies(first_feature.body, second_feature.body)?;
        let offset = revalidate_frame(offset)?;
        compose_frames(first_feature.local_frame, offset)?;
        self.add_source_record(
            label,
            SpatialSourceKind::FrameOffsetMate {
                first,
                second,
                offset,
            },
        )
    }

    /// Adds a topology-only hinge coordinate over an ordered revolute, cylindrical, or planar source.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale or wrong-kind parent source, or exhausted ID space.
    pub fn add_hinge_coordinate(
        &mut self,
        label: impl Into<String>,
        parent: SpatialSourceId,
        winding: i64,
    ) -> Result<SpatialCoordinateId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "hinge coordinate")?;
        require_hinge_parent(self.require_source(parent)?)?;
        self.add_coordinate_record(label, SpatialCoordinateKind::Hinge { parent, winding })
    }

    /// Adds a topology-only axial translation coordinate over an ordered prismatic or cylindrical source.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale or wrong-kind parent source, or exhausted ID space.
    pub fn add_axial_translation_coordinate(
        &mut self,
        label: impl Into<String>,
        parent: SpatialSourceId,
    ) -> Result<SpatialCoordinateId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "axial translation coordinate")?;
        require_axial_translation_parent(self.require_source(parent)?)?;
        self.add_coordinate_record(label, SpatialCoordinateKind::AxialTranslation { parent })
    }

    /// Adds a topology-only first-plane X or Y translation coordinate over a planar joint.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale or non-planar parent source, or exhausted ID space.
    pub fn add_planar_translation_coordinate(
        &mut self,
        label: impl Into<String>,
        parent: SpatialSourceId,
        axis: SpatialPlanarTranslationAxis,
    ) -> Result<SpatialCoordinateId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "planar translation coordinate")?;
        require_planar_translation_parent(self.require_source(parent)?)?;
        self.add_coordinate_record(
            label,
            SpatialCoordinateKind::PlanarTranslation { parent, axis },
        )
    }

    /// Adds one hard hinge-position source over an existing hinge coordinate.
    ///
    /// # Errors
    ///
    /// Rejects invalid labels, stale or wrong-kind coordinates, noncanonical targets,
    /// winding mismatch, incompatible existing targets, or exhausted ID space.
    pub fn add_hinge_position_driver(
        &mut self,
        label: impl Into<String>,
        coordinate: SpatialCoordinateId,
        target: SpatialHingeTarget,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "hinge position driver")?;
        validate_hinge_target(target)?;
        let coordinate_definition = self.require_coordinate(coordinate)?;
        let SpatialCoordinateKind::Hinge { winding, .. } = coordinate_definition.kind else {
            return Err(SpatialAssemblyError::WrongCoordinateKind {
                coordinate,
                expected: "hinge",
            });
        };
        require_matching_winding(coordinate, winding, target.winding)?;
        require_compatible_hinge_driver_targets(self, coordinate, target)?;
        self.add_source_record(
            label,
            SpatialSourceKind::HingePositionDriver { coordinate, target },
        )
    }

    /// Adds one hard translation-position source over an existing translation coordinate.
    ///
    /// # Errors
    ///
    /// Rejects invalid labels, stale or wrong-kind coordinates, non-finite targets,
    /// incompatible existing targets, or exhausted ID space.
    pub fn add_translation_position_driver(
        &mut self,
        label: impl Into<String>,
        coordinate: SpatialCoordinateId,
        target: f64,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "translation position driver")?;
        validate_translation_target(target)?;
        if !matches!(
            self.require_coordinate(coordinate)?.kind,
            SpatialCoordinateKind::AxialTranslation { .. }
                | SpatialCoordinateKind::PlanarTranslation { .. }
        ) {
            return Err(SpatialAssemblyError::WrongCoordinateKind {
                coordinate,
                expected: "translation",
            });
        }
        require_compatible_translation_driver_targets(self, coordinate, target)?;
        self.add_source_record(
            label,
            SpatialSourceKind::TranslationPositionDriver { coordinate, target },
        )
    }

    /// Adds a row-free directed-axis parity monitor.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale axis feature, or exhausted ID space.
    pub fn add_axis_parity_monitor(
        &mut self,
        label: impl Into<String>,
        first: SpatialAxisFeatureId,
        second: SpatialAxisFeatureId,
        parity: SpatialAxisParity,
    ) -> Result<SpatialModeMonitorId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "axis parity monitor")?;
        self.require_axis_feature(first)?;
        self.require_axis_feature(second)?;
        self.add_mode_monitor_record(
            label,
            SpatialModeMonitorKind::AxisParity {
                first,
                second,
                parity,
            },
        )
    }

    /// Adds a row-free explicit winding monitor over a hinge coordinate.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale or non-hinge coordinate, mismatched winding,
    /// or exhausted ID space.
    pub fn add_hinge_winding_monitor(
        &mut self,
        label: impl Into<String>,
        coordinate: SpatialCoordinateId,
        winding: i64,
    ) -> Result<SpatialModeMonitorId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "hinge winding monitor")?;
        let coordinate_definition = self.require_coordinate(coordinate)?;
        let SpatialCoordinateKind::Hinge {
            winding: coordinate_winding,
            ..
        } = coordinate_definition.kind
        else {
            return Err(SpatialAssemblyError::WrongCoordinateKind {
                coordinate,
                expected: "hinge",
            });
        };
        require_matching_winding(coordinate, coordinate_winding, winding)?;
        self.add_mode_monitor_record(
            label,
            SpatialModeMonitorKind::HingeWinding {
                coordinate,
                winding,
            },
        )
    }

    /// Adds a row-free selected-side monitor for a point and directed plane.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale plane/point feature, or exhausted ID space.
    pub fn add_plane_side_monitor(
        &mut self,
        label: impl Into<String>,
        plane: SpatialPlaneFeatureId,
        point: SpatialPointFeatureId,
        side: SpatialModeSign,
    ) -> Result<SpatialModeMonitorId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "plane side monitor")?;
        self.require_plane_feature(plane)?;
        self.require_point_feature(point)?;
        self.add_mode_monitor_record(
            label,
            SpatialModeMonitorKind::PlaneSide { plane, point, side },
        )
    }

    /// Adds a row-free selected orientation for four ordered point features.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale or repeated point features, or exhausted ID space.
    pub fn add_signed_volume_monitor(
        &mut self,
        label: impl Into<String>,
        points: [SpatialPointFeatureId; 4],
        orientation: SpatialModeSign,
    ) -> Result<SpatialModeMonitorId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "signed volume monitor")?;
        require_distinct_volume_points(points)?;
        for point in points {
            self.require_point_feature(point)?;
        }
        self.add_mode_monitor_record(
            label,
            SpatialModeMonitorKind::SignedVolume {
                points,
                orientation,
            },
        )
    }

    /// Compiles only physical equations in deterministic insertion order.
    ///
    /// # Errors
    ///
    /// Rejects invalid assembly state or any core declaration failure.
    pub fn compile(&self) -> Result<CompiledSpatialAssembly, SpatialAssemblyError> {
        self.validate_structure()?;
        self.compile_validated()
    }

    fn compile_with_parameterized_driver(
        &self,
        driver: SpatialSourceId,
        parameter: f64,
    ) -> Result<(CompiledSpatialAssembly, VariableId), SpatialAssemblyError> {
        self.validate_structure()?;
        let source = self.require_source(driver)?;
        match source.kind {
            SpatialSourceKind::HingePositionDriver { .. } => {
                validate_hinge_target(SpatialHingeTarget {
                    principal_phase: parameter,
                    winding: 0,
                })?;
            }
            SpatialSourceKind::TranslationPositionDriver { .. } => {
                validate_translation_target(parameter)?;
            }
            _ => {
                return Err(SpatialAssemblyError::WrongSourceKind {
                    source_id: driver,
                    expected: "a position driver",
                });
            }
        }
        let (compiled, parameter_variable) =
            self.compile_validated_internal(Some((driver, parameter)))?;
        Ok((
            compiled,
            parameter_variable.ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "parameterized spatial compilation omitted its scalar variable".to_owned(),
                )
            })?,
        ))
    }

    fn compile_validated(&self) -> Result<CompiledSpatialAssembly, SpatialAssemblyError> {
        self.compile_validated_internal(None)
            .map(|(compiled, _)| compiled)
    }

    #[allow(clippy::too_many_lines)]
    fn compile_validated_internal(
        &self,
        parameterized_driver: Option<(SpatialSourceId, f64)>,
    ) -> Result<(CompiledSpatialAssembly, Option<VariableId>), SpatialAssemblyError> {
        let mut problem = Problem::new();
        let mut body_variables = Vec::with_capacity(self.bodies.len());
        let mut variables = HashMap::with_capacity(self.bodies.len());
        let pose_scales = [
            self.model_scale,
            self.model_scale,
            self.model_scale,
            1.0,
            1.0,
            1.0,
        ];
        for body in &self.bodies {
            let variable_id = problem.add_variable(VariableBlock::pose3(
                body.pose_guess.ambient(),
                pose_scales,
            )?);
            body_variables.push(SpatialBodyVariableMapping {
                body_id: body.id,
                variable_id,
            });
            variables.insert(body.id, variable_id);
        }
        let parameter_variable = if let Some((driver, parameter)) = parameterized_driver {
            let source = self.require_source(driver)?;
            let scale = match source.kind {
                SpatialSourceKind::HingePositionDriver { .. } => 1.0,
                SpatialSourceKind::TranslationPositionDriver { .. } => self.model_scale,
                _ => {
                    return Err(SpatialAssemblyError::WrongSourceKind {
                        source_id: driver,
                        expected: "a position driver",
                    });
                }
            };
            Some(problem.add_variable(VariableBlock::scalar(parameter, scale)?))
        } else {
            None
        };

        let mut source_mappings = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            let core_source_id = problem.add_source(SourceConstraint::new(&source.label)?);
            let residual = match source.kind {
                SpatialSourceKind::PhysicalGround { body, target_pose } => {
                    let variable = variable_for_body(&variables, body)?;
                    let residual = ResidualBlock::fixed_variable(
                        core_source_id,
                        variable,
                        VariableValue::Pose3(target_pose.ambient()),
                        pose_scales.to_vec(),
                        ground_audit_rows(body, target_pose),
                    )?;
                    let residual_id = problem.add_residual(residual)?;
                    problem.declare_fixed_variable(
                        variable,
                        VariableValue::Pose3(target_pose.ambient()),
                        residual_id,
                    )?;
                    residual_id
                }
                SpatialSourceKind::BallJoint { first, second } => {
                    let first_feature = self.require_point_feature(first)?;
                    let second_feature = self.require_point_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        3,
                        vec![self.model_scale; 3],
                        point_joint_audit_rows("ball joint", first_feature, second_feature),
                        SpatialBallResidual {
                            first_local: point_array(first_feature.local_point),
                            second_local: point_array(second_feature.local_point),
                        },
                    )?)?
                }
                SpatialSourceKind::PointDistanceMate {
                    first,
                    second,
                    distance,
                } => {
                    let first_feature = self.require_point_feature(first)?;
                    let second_feature = self.require_point_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        1,
                        vec![self.model_scale],
                        vec![point_distance_audit_row(
                            first_feature,
                            second_feature,
                            distance,
                        )],
                        SpatialPointDistanceResidual {
                            first_local: point_array(first_feature.local_point),
                            second_local: point_array(second_feature.local_point),
                            distance,
                        },
                    )?)?
                }
                SpatialSourceKind::FixedFrame { first, second } => {
                    let first_feature = self.require_frame_feature(first)?;
                    let second_feature = self.require_frame_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        6,
                        vec![
                            self.model_scale,
                            self.model_scale,
                            self.model_scale,
                            1.0,
                            1.0,
                            1.0,
                        ],
                        frame_joint_audit_rows(
                            "fixed frame",
                            first_feature,
                            second_feature,
                            None,
                            &[
                                "world origin x difference",
                                "world origin y difference",
                                "world origin z difference",
                                "first y dot second x",
                                "first z dot second x",
                                "first z dot second y",
                            ],
                        ),
                        SpatialFixedFrameResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                        },
                    )?)?
                }
                SpatialSourceKind::FrameOffsetMate {
                    first,
                    second,
                    offset,
                } => {
                    let first_feature = self.require_frame_feature(first)?;
                    let second_feature = self.require_frame_feature(second)?;
                    let expected_local = compose_frames(first_feature.local_frame, offset)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        6,
                        vec![
                            self.model_scale,
                            self.model_scale,
                            self.model_scale,
                            1.0,
                            1.0,
                            1.0,
                        ],
                        frame_offset_audit_rows(first_feature, second_feature, offset),
                        SpatialFixedFrameResidual {
                            first_local: expected_local,
                            second_local: second_feature.local_frame,
                        },
                    )?)?
                }
                SpatialSourceKind::RevoluteJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first_feature = self.require_frame_feature(first)?;
                    let second_feature = self.require_frame_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        5,
                        vec![
                            self.model_scale,
                            self.model_scale,
                            self.model_scale,
                            1.0,
                            1.0,
                        ],
                        frame_joint_audit_rows(
                            "revolute joint",
                            first_feature,
                            second_feature,
                            Some(parity),
                            &[
                                "world origin x difference",
                                "world origin y difference",
                                "world origin z difference",
                                "first x dot parity-adjusted second z",
                                "first y dot parity-adjusted second z",
                            ],
                        ),
                        SpatialRevoluteResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            parity_multiplier: parity.multiplier(),
                        },
                    )?)?
                }
                SpatialSourceKind::PrismaticJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first_feature = self.require_axis_feature(first)?;
                    let second_feature = self.require_axis_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        5,
                        vec![self.model_scale, self.model_scale, 1.0, 1.0, 1.0],
                        axis_joint_audit_rows(
                            "prismatic joint",
                            first_feature,
                            second_feature,
                            Some(parity),
                            &[
                                ("first x dot (second origin - first origin)", "model-unit"),
                                ("first y dot (second origin - first origin)", "model-unit"),
                                ("first x dot parity-adjusted second z", "dimensionless"),
                                ("first y dot parity-adjusted second z", "dimensionless"),
                                ("first y dot second x", "dimensionless"),
                            ],
                        ),
                        SpatialRelationResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            parity_multiplier: parity.multiplier(),
                            kind: SpatialRelationKind::Prismatic,
                        },
                    )?)?
                }
                SpatialSourceKind::CylindricalJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first_feature = self.require_axis_feature(first)?;
                    let second_feature = self.require_axis_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        4,
                        vec![self.model_scale, self.model_scale, 1.0, 1.0],
                        axis_joint_audit_rows(
                            "cylindrical joint",
                            first_feature,
                            second_feature,
                            Some(parity),
                            &[
                                ("first x dot (second origin - first origin)", "model-unit"),
                                ("first y dot (second origin - first origin)", "model-unit"),
                                ("first x dot parity-adjusted second z", "dimensionless"),
                                ("first y dot parity-adjusted second z", "dimensionless"),
                            ],
                        ),
                        SpatialRelationResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            parity_multiplier: parity.multiplier(),
                            kind: SpatialRelationKind::Cylindrical,
                        },
                    )?)?
                }
                SpatialSourceKind::PlanarJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first_feature = self.require_plane_feature(first)?;
                    let second_feature = self.require_plane_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        3,
                        vec![self.model_scale, 1.0, 1.0],
                        plane_joint_audit_rows(
                            "planar joint",
                            first_feature,
                            second_feature,
                            parity,
                            &[
                                ("first z dot (second origin - first origin)", "model-unit"),
                                ("first x dot parity-adjusted second z", "dimensionless"),
                                ("first y dot parity-adjusted second z", "dimensionless"),
                            ],
                        ),
                        SpatialRelationResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            parity_multiplier: parity.multiplier(),
                            kind: SpatialRelationKind::Planar,
                        },
                    )?)?
                }
                SpatialSourceKind::UniversalJoint { first, second } => {
                    let first_feature = self.require_axis_feature(first)?;
                    let second_feature = self.require_axis_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        4,
                        vec![self.model_scale, self.model_scale, self.model_scale, 1.0],
                        axis_joint_audit_rows(
                            "universal joint",
                            first_feature,
                            second_feature,
                            None,
                            &[
                                ("second origin x - first origin x", "model-unit"),
                                ("second origin y - first origin y", "model-unit"),
                                ("second origin z - first origin z", "model-unit"),
                                ("first z dot second z", "dimensionless"),
                            ],
                        ),
                        SpatialRelationResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            parity_multiplier: 1.0,
                            kind: SpatialRelationKind::Universal,
                        },
                    )?)?
                }
                SpatialSourceKind::AxisAngleMate {
                    first,
                    second,
                    angle,
                } => {
                    let first_feature = self.require_axis_feature(first)?;
                    let second_feature = self.require_axis_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        1,
                        vec![1.0],
                        vec![axis_angle_audit_row(first_feature, second_feature, angle)],
                        SpatialAxisAngleResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            angle,
                        },
                    )?)?
                }
                SpatialSourceKind::AxisAlignmentMate {
                    first,
                    second,
                    parity,
                } => {
                    let first_feature = self.require_axis_feature(first)?;
                    let second_feature = self.require_axis_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        2,
                        vec![1.0, 1.0],
                        axis_joint_audit_rows(
                            "axis alignment mate",
                            first_feature,
                            second_feature,
                            Some(parity),
                            &[
                                ("first x dot parity-adjusted second z", "dimensionless"),
                                ("first y dot parity-adjusted second z", "dimensionless"),
                            ],
                        ),
                        SpatialAxisAlignmentResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            parity_multiplier: parity.multiplier(),
                        },
                    )?)?
                }
                SpatialSourceKind::HingePositionDriver { coordinate, target } => {
                    let coordinate_definition = self.require_coordinate(coordinate)?;
                    let resolved =
                        resolve_hinge_coordinate_definition(self, coordinate_definition)?;
                    if parameterized_driver.is_some_and(|(selected, _)| selected == source.id) {
                        let parameter_variable = parameter_variable.ok_or_else(|| {
                            SpatialAssemblyError::IndependentValidation(
                                "parameterized hinge driver omitted its scalar variable".to_owned(),
                            )
                        })?;
                        problem.add_residual(ResidualBlock::new(
                            core_source_id,
                            ResidualCategory::Hard,
                            vec![
                                variable_for_body(&variables, resolved.first_body)?,
                                variable_for_body(&variables, resolved.second_body)?,
                                parameter_variable,
                            ],
                            1,
                            vec![1.0],
                            vec![parameterized_hinge_driver_audit_row(
                                coordinate_definition,
                                resolved,
                                target.winding,
                            )],
                            ParameterizedSpatialHingePositionResidual {
                                first_local: resolved.first_local,
                                second_local: resolved.second_local,
                                parity_multiplier: resolved.parity.multiplier(),
                            },
                        )?)?
                    } else {
                        problem.add_residual(ResidualBlock::new(
                            core_source_id,
                            ResidualCategory::Hard,
                            vec![
                                variable_for_body(&variables, resolved.first_body)?,
                                variable_for_body(&variables, resolved.second_body)?,
                            ],
                            1,
                            vec![1.0],
                            vec![hinge_driver_audit_row(
                                coordinate_definition,
                                resolved,
                                target,
                            )],
                            SpatialHingePositionResidual {
                                first_local: resolved.first_local,
                                second_local: resolved.second_local,
                                parity_multiplier: resolved.parity.multiplier(),
                                target_principal_phase: target.principal_phase,
                            },
                        )?)?
                    }
                }
                SpatialSourceKind::TranslationPositionDriver { coordinate, target } => {
                    let coordinate_definition = self.require_coordinate(coordinate)?;
                    let resolved =
                        resolve_translation_coordinate_definition(self, coordinate_definition)?;
                    if parameterized_driver.is_some_and(|(selected, _)| selected == source.id) {
                        let parameter_variable = parameter_variable.ok_or_else(|| {
                            SpatialAssemblyError::IndependentValidation(
                                "parameterized translation driver omitted its scalar variable"
                                    .to_owned(),
                            )
                        })?;
                        problem.add_residual(ResidualBlock::new(
                            core_source_id,
                            ResidualCategory::Hard,
                            vec![
                                variable_for_body(&variables, resolved.first_body)?,
                                variable_for_body(&variables, resolved.second_body)?,
                                parameter_variable,
                            ],
                            1,
                            vec![self.model_scale],
                            vec![parameterized_translation_driver_audit_row(
                                coordinate_definition,
                                resolved,
                            )],
                            ParameterizedSpatialTranslationPositionResidual {
                                first_local: resolved.first_local,
                                second_local: resolved.second_local,
                                first_local_axis: translation_local_axis(
                                    coordinate_definition,
                                    resolved,
                                )?,
                                parity_multiplier: resolved.parity.multiplier(),
                            },
                        )?)?
                    } else {
                        problem.add_residual(ResidualBlock::new(
                            core_source_id,
                            ResidualCategory::Hard,
                            vec![
                                variable_for_body(&variables, resolved.first_body)?,
                                variable_for_body(&variables, resolved.second_body)?,
                            ],
                            1,
                            vec![self.model_scale],
                            vec![translation_driver_audit_row(
                                coordinate_definition,
                                resolved,
                                target,
                            )],
                            SpatialTranslationPositionResidual {
                                first_local: resolved.first_local,
                                second_local: resolved.second_local,
                                first_local_axis: translation_local_axis(
                                    coordinate_definition,
                                    resolved,
                                )?,
                                parity_multiplier: resolved.parity.multiplier(),
                                target,
                            },
                        )?)?
                    }
                }
            };
            source_mappings.push(SpatialSourceMapping {
                source: source.id,
                source_label: source.label.clone(),
                core_source_id,
                residual_ids: vec![residual],
            });
        }

        Ok((
            CompiledSpatialAssembly {
                problem,
                body_variables,
                source_mappings,
                point_features: self.point_features.clone(),
                frame_features: self.frame_features.clone(),
                axis_features: self.axis_features.clone(),
                plane_features: self.plane_features.clone(),
            },
            parameter_variable,
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn validate_structure(&self) -> Result<(), SpatialAssemblyError> {
        validate_model_scale(self.model_scale)?;
        if self.namespace == 0 {
            return invalid_field("namespace", "assembly namespace must be nonzero");
        }
        let mut raw_ids = BTreeSet::new();
        for body in &self.bodies {
            validate_label(&body.label, "body")?;
            validate_pose(body.pose_guess)?;
            require_owned_unique_id(&mut raw_ids, self.namespace, body.id)?;
        }
        for feature in &self.point_features {
            validate_label(&feature.label, "point feature")?;
            self.require_body(feature.body)?;
            validate_point(feature.local_point, "point_feature.local_point")?;
            require_owned_unique_id(&mut raw_ids, self.namespace, feature.id)?;
        }
        for feature in &self.frame_features {
            validate_label(&feature.label, "frame feature")?;
            self.require_body(feature.body)?;
            revalidate_frame(feature.local_frame)?;
            require_owned_unique_id(&mut raw_ids, self.namespace, feature.id)?;
        }
        for feature in &self.axis_features {
            validate_label(&feature.label, "axis feature")?;
            self.require_body(feature.body)?;
            revalidate_frame(feature.local_frame)?;
            require_owned_unique_id(&mut raw_ids, self.namespace, feature.id)?;
        }
        for feature in &self.plane_features {
            validate_label(&feature.label, "plane feature")?;
            self.require_body(feature.body)?;
            revalidate_frame(feature.local_frame)?;
            require_owned_unique_id(&mut raw_ids, self.namespace, feature.id)?;
        }
        for coordinate in &self.coordinates {
            validate_label(&coordinate.label, "coordinate")?;
            require_owned_unique_id(&mut raw_ids, self.namespace, coordinate.id)?;
            match coordinate.kind {
                SpatialCoordinateKind::Hinge { parent, .. } => {
                    require_hinge_parent(self.require_source(parent)?)?;
                }
                SpatialCoordinateKind::AxialTranslation { parent } => {
                    require_axial_translation_parent(self.require_source(parent)?)?;
                }
                SpatialCoordinateKind::PlanarTranslation { parent, .. } => {
                    require_planar_translation_parent(self.require_source(parent)?)?;
                }
            }
        }
        for monitor in &self.mode_monitors {
            validate_label(&monitor.label, "mode monitor")?;
            require_owned_unique_id(&mut raw_ids, self.namespace, monitor.id)?;
            match monitor.kind {
                SpatialModeMonitorKind::AxisParity { first, second, .. } => {
                    self.require_axis_feature(first)?;
                    self.require_axis_feature(second)?;
                }
                SpatialModeMonitorKind::HingeWinding {
                    coordinate,
                    winding,
                } => {
                    let coordinate_definition = self.require_coordinate(coordinate)?;
                    let SpatialCoordinateKind::Hinge {
                        winding: coordinate_winding,
                        ..
                    } = coordinate_definition.kind
                    else {
                        return Err(SpatialAssemblyError::WrongCoordinateKind {
                            coordinate,
                            expected: "hinge",
                        });
                    };
                    require_matching_winding(coordinate, coordinate_winding, winding)?;
                    resolve_hinge_coordinate_definition(self, coordinate_definition)?;
                }
                SpatialModeMonitorKind::PlaneSide { plane, point, .. } => {
                    self.require_plane_feature(plane)?;
                    self.require_point_feature(point)?;
                }
                SpatialModeMonitorKind::SignedVolume { points, .. } => {
                    require_distinct_volume_points(points)?;
                    for point in points {
                        self.require_point_feature(point)?;
                    }
                }
            }
        }
        let mut grounded = BTreeSet::new();
        for source in &self.sources {
            validate_label(&source.label, "source")?;
            require_owned_unique_id(&mut raw_ids, self.namespace, source.id)?;
            match source.kind {
                SpatialSourceKind::PhysicalGround { body, target_pose } => {
                    self.require_body(body)?;
                    validate_pose(target_pose)?;
                    if !grounded.insert(body) {
                        return Err(SpatialAssemblyError::DuplicateGround(body));
                    }
                }
                SpatialSourceKind::BallJoint { first, second } => {
                    let first = self.require_point_feature(first)?;
                    let second = self.require_point_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                }
                SpatialSourceKind::PointDistanceMate {
                    first,
                    second,
                    distance,
                } => {
                    let first = self.require_point_feature(first)?;
                    let second = self.require_point_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                    validate_positive_distance(distance)?;
                    validate_point_distance_candidate(self, first, second)?;
                }
                SpatialSourceKind::FixedFrame { first, second }
                | SpatialSourceKind::RevoluteJoint { first, second, .. } => {
                    let first = self.require_frame_feature(first)?;
                    let second = self.require_frame_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                }
                SpatialSourceKind::FrameOffsetMate {
                    first,
                    second,
                    offset,
                } => {
                    let first = self.require_frame_feature(first)?;
                    let second = self.require_frame_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                    let offset = revalidate_frame(offset)?;
                    compose_frames(first.local_frame, offset)?;
                }
                SpatialSourceKind::PrismaticJoint { first, second, .. }
                | SpatialSourceKind::CylindricalJoint { first, second, .. }
                | SpatialSourceKind::UniversalJoint { first, second }
                | SpatialSourceKind::AxisAlignmentMate { first, second, .. } => {
                    let first = self.require_axis_feature(first)?;
                    let second = self.require_axis_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                }
                SpatialSourceKind::AxisAngleMate {
                    first,
                    second,
                    angle,
                } => {
                    let first = self.require_axis_feature(first)?;
                    let second = self.require_axis_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                    validate_interior_angle(angle)?;
                }
                SpatialSourceKind::PlanarJoint { first, second, .. } => {
                    let first = self.require_plane_feature(first)?;
                    let second = self.require_plane_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                }
                SpatialSourceKind::HingePositionDriver { coordinate, target } => {
                    validate_hinge_target(target)?;
                    let coordinate_definition = self.require_coordinate(coordinate)?;
                    let SpatialCoordinateKind::Hinge { winding, .. } = coordinate_definition.kind
                    else {
                        return Err(SpatialAssemblyError::WrongCoordinateKind {
                            coordinate,
                            expected: "hinge",
                        });
                    };
                    require_matching_winding(coordinate, winding, target.winding)?;
                    resolve_hinge_coordinate_definition(self, coordinate_definition)?;
                }
                SpatialSourceKind::TranslationPositionDriver { coordinate, target } => {
                    validate_translation_target(target)?;
                    let coordinate_definition = self.require_coordinate(coordinate)?;
                    if !matches!(
                        coordinate_definition.kind,
                        SpatialCoordinateKind::AxialTranslation { .. }
                            | SpatialCoordinateKind::PlanarTranslation { .. }
                    ) {
                        return Err(SpatialAssemblyError::WrongCoordinateKind {
                            coordinate,
                            expected: "translation",
                        });
                    }
                    resolve_translation_coordinate_definition(self, coordinate_definition)?;
                }
            }
        }
        validate_driver_consistency(self)?;
        if raw_ids
            .iter()
            .next_back()
            .is_some_and(|maximum| self.next_id <= *maximum)
        {
            return invalid_field("next_id", "must exceed every allocated ID");
        }
        let components = certified_components(self)?;
        resolve_gauge_references(&self.gauge_policy, &components)?;
        Ok(())
    }

    fn allocate_id(&mut self) -> Result<u64, SpatialAssemblyError> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(SpatialAssemblyError::IdExhausted)?;
        Ok(id)
    }

    fn add_source_record(
        &mut self,
        label: String,
        kind: SpatialSourceKind,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let id = SpatialSourceId::new(self.namespace, self.allocate_id()?);
        self.sources.push(SpatialSource { id, label, kind });
        Ok(id)
    }

    fn add_coordinate_record(
        &mut self,
        label: String,
        kind: SpatialCoordinateKind,
    ) -> Result<SpatialCoordinateId, SpatialAssemblyError> {
        let id = SpatialCoordinateId::new(self.namespace, self.allocate_id()?);
        self.coordinates.push(SpatialCoordinate { id, label, kind });
        Ok(id)
    }

    fn add_mode_monitor_record(
        &mut self,
        label: String,
        kind: SpatialModeMonitorKind,
    ) -> Result<SpatialModeMonitorId, SpatialAssemblyError> {
        let id = SpatialModeMonitorId::new(self.namespace, self.allocate_id()?);
        self.mode_monitors
            .push(SpatialModeMonitor { id, label, kind });
        Ok(id)
    }

    fn require_body(&self, id: SpatialBodyId) -> Result<&SpatialBody, SpatialAssemblyError> {
        self.body(id).ok_or(SpatialAssemblyError::UnknownBody(id))
    }

    fn require_point_feature(
        &self,
        id: SpatialPointFeatureId,
    ) -> Result<&SpatialPointFeature, SpatialAssemblyError> {
        self.point_feature(id)
            .ok_or(SpatialAssemblyError::UnknownPointFeature(id))
    }

    fn require_frame_feature(
        &self,
        id: SpatialFrameFeatureId,
    ) -> Result<&SpatialFrameFeature, SpatialAssemblyError> {
        self.frame_feature(id)
            .ok_or(SpatialAssemblyError::UnknownFrameFeature(id))
    }

    fn require_axis_feature(
        &self,
        id: SpatialAxisFeatureId,
    ) -> Result<&SpatialAxisFeature, SpatialAssemblyError> {
        self.axis_feature(id)
            .ok_or(SpatialAssemblyError::UnknownAxisFeature(id))
    }

    fn require_plane_feature(
        &self,
        id: SpatialPlaneFeatureId,
    ) -> Result<&SpatialPlaneFeature, SpatialAssemblyError> {
        self.plane_feature(id)
            .ok_or(SpatialAssemblyError::UnknownPlaneFeature(id))
    }

    fn require_source(&self, id: SpatialSourceId) -> Result<&SpatialSource, SpatialAssemblyError> {
        self.source(id)
            .ok_or(SpatialAssemblyError::UnknownSource(id))
    }

    fn require_coordinate(
        &self,
        id: SpatialCoordinateId,
    ) -> Result<&SpatialCoordinate, SpatialAssemblyError> {
        self.coordinate(id)
            .ok_or(SpatialAssemblyError::UnknownCoordinate(id))
    }

    fn require_mode_monitor(
        &self,
        id: SpatialModeMonitorId,
    ) -> Result<&SpatialModeMonitor, SpatialAssemblyError> {
        self.mode_monitor(id)
            .ok_or(SpatialAssemblyError::UnknownModeMonitor(id))
    }
}

#[derive(Clone, Copy, Debug)]
struct ResolvedSpatialCoordinateDefinition {
    parent_source: SpatialSourceId,
    first_body: SpatialBodyId,
    second_body: SpatialBodyId,
    first_local: Frame3,
    second_local: Frame3,
    parity: SpatialAxisParity,
}

fn require_hinge_parent(source: &SpatialSource) -> Result<(), SpatialAssemblyError> {
    if matches!(
        source.kind,
        SpatialSourceKind::RevoluteJoint { .. }
            | SpatialSourceKind::CylindricalJoint { .. }
            | SpatialSourceKind::PlanarJoint { .. }
    ) {
        Ok(())
    } else {
        Err(SpatialAssemblyError::WrongCoordinateParent {
            source_id: source.id,
            expected: "hinge",
        })
    }
}

fn require_axial_translation_parent(source: &SpatialSource) -> Result<(), SpatialAssemblyError> {
    if matches!(
        source.kind,
        SpatialSourceKind::PrismaticJoint { .. } | SpatialSourceKind::CylindricalJoint { .. }
    ) {
        Ok(())
    } else {
        Err(SpatialAssemblyError::WrongCoordinateParent {
            source_id: source.id,
            expected: "axial translation",
        })
    }
}

fn require_planar_translation_parent(source: &SpatialSource) -> Result<(), SpatialAssemblyError> {
    if matches!(source.kind, SpatialSourceKind::PlanarJoint { .. }) {
        Ok(())
    } else {
        Err(SpatialAssemblyError::WrongCoordinateParent {
            source_id: source.id,
            expected: "planar translation",
        })
    }
}

fn resolve_hinge_coordinate_definition(
    assembly: &SpatialAssembly,
    coordinate: &SpatialCoordinate,
) -> Result<ResolvedSpatialCoordinateDefinition, SpatialAssemblyError> {
    let SpatialCoordinateKind::Hinge { parent, .. } = coordinate.kind else {
        return Err(SpatialAssemblyError::WrongCoordinateKind {
            coordinate: coordinate.id,
            expected: "hinge",
        });
    };
    let source = assembly.require_source(parent)?;
    match source.kind {
        SpatialSourceKind::RevoluteJoint {
            first,
            second,
            parity,
        } => {
            let first = assembly.require_frame_feature(first)?;
            let second = assembly.require_frame_feature(second)?;
            Ok(ResolvedSpatialCoordinateDefinition {
                parent_source: parent,
                first_body: first.body,
                second_body: second.body,
                first_local: first.local_frame,
                second_local: second.local_frame,
                parity,
            })
        }
        SpatialSourceKind::CylindricalJoint {
            first,
            second,
            parity,
        } => {
            let first = assembly.require_axis_feature(first)?;
            let second = assembly.require_axis_feature(second)?;
            Ok(ResolvedSpatialCoordinateDefinition {
                parent_source: parent,
                first_body: first.body,
                second_body: second.body,
                first_local: first.local_frame,
                second_local: second.local_frame,
                parity,
            })
        }
        SpatialSourceKind::PlanarJoint {
            first,
            second,
            parity,
        } => {
            let first = assembly.require_plane_feature(first)?;
            let second = assembly.require_plane_feature(second)?;
            Ok(ResolvedSpatialCoordinateDefinition {
                parent_source: parent,
                first_body: first.body,
                second_body: second.body,
                first_local: first.local_frame,
                second_local: second.local_frame,
                parity,
            })
        }
        _ => Err(SpatialAssemblyError::WrongCoordinateParent {
            source_id: parent,
            expected: "hinge",
        }),
    }
}

fn resolve_translation_coordinate_definition(
    assembly: &SpatialAssembly,
    coordinate: &SpatialCoordinate,
) -> Result<ResolvedSpatialCoordinateDefinition, SpatialAssemblyError> {
    let parent = match coordinate.kind {
        SpatialCoordinateKind::AxialTranslation { parent }
        | SpatialCoordinateKind::PlanarTranslation { parent, .. } => parent,
        SpatialCoordinateKind::Hinge { .. } => {
            return Err(SpatialAssemblyError::WrongCoordinateKind {
                coordinate: coordinate.id,
                expected: "translation",
            });
        }
    };
    let source = assembly.require_source(parent)?;
    match (coordinate.kind, source.kind) {
        (
            SpatialCoordinateKind::AxialTranslation { .. },
            SpatialSourceKind::PrismaticJoint {
                first,
                second,
                parity,
            }
            | SpatialSourceKind::CylindricalJoint {
                first,
                second,
                parity,
            },
        ) => {
            let first = assembly.require_axis_feature(first)?;
            let second = assembly.require_axis_feature(second)?;
            Ok(ResolvedSpatialCoordinateDefinition {
                parent_source: parent,
                first_body: first.body,
                second_body: second.body,
                first_local: first.local_frame,
                second_local: second.local_frame,
                parity,
            })
        }
        (
            SpatialCoordinateKind::PlanarTranslation { .. },
            SpatialSourceKind::PlanarJoint {
                first,
                second,
                parity,
            },
        ) => {
            let first = assembly.require_plane_feature(first)?;
            let second = assembly.require_plane_feature(second)?;
            Ok(ResolvedSpatialCoordinateDefinition {
                parent_source: parent,
                first_body: first.body,
                second_body: second.body,
                first_local: first.local_frame,
                second_local: second.local_frame,
                parity,
            })
        }
        (SpatialCoordinateKind::AxialTranslation { .. }, _) => {
            Err(SpatialAssemblyError::WrongCoordinateParent {
                source_id: parent,
                expected: "axial translation",
            })
        }
        (SpatialCoordinateKind::PlanarTranslation { .. }, _) => {
            Err(SpatialAssemblyError::WrongCoordinateParent {
                source_id: parent,
                expected: "planar translation",
            })
        }
        (SpatialCoordinateKind::Hinge { .. }, _) => unreachable!("kind checked above"),
    }
}

fn translation_local_axis(
    coordinate: &SpatialCoordinate,
    resolved: ResolvedSpatialCoordinateDefinition,
) -> Result<Vector3<f64>, SpatialAssemblyError> {
    match coordinate.kind {
        SpatialCoordinateKind::AxialTranslation { .. } => Ok(resolved.first_local.z_axis()),
        SpatialCoordinateKind::PlanarTranslation {
            axis: SpatialPlanarTranslationAxis::X,
            ..
        } => Ok(resolved.first_local.x_axis()),
        SpatialCoordinateKind::PlanarTranslation {
            axis: SpatialPlanarTranslationAxis::Y,
            ..
        } => Ok(resolved.first_local.y_axis()),
        SpatialCoordinateKind::Hinge { .. } => Err(SpatialAssemblyError::WrongCoordinateKind {
            coordinate: coordinate.id,
            expected: "translation",
        }),
    }
}

fn require_matching_winding(
    coordinate: SpatialCoordinateId,
    coordinate_winding: i64,
    target_winding: i64,
) -> Result<(), SpatialAssemblyError> {
    if coordinate_winding == target_winding {
        Ok(())
    } else {
        Err(SpatialAssemblyError::WindingMismatch {
            coordinate,
            coordinate_winding,
            target_winding,
        })
    }
}

fn require_compatible_hinge_driver_targets(
    assembly: &SpatialAssembly,
    coordinate: SpatialCoordinateId,
    target: SpatialHingeTarget,
) -> Result<(), SpatialAssemblyError> {
    if assembly.sources.iter().any(|source| {
        matches!(
            source.kind,
            SpatialSourceKind::HingePositionDriver {
                coordinate: existing_coordinate,
                target: existing_target,
            } if existing_coordinate == coordinate
                && !same_hinge_target(existing_target, target)
        )
    }) {
        Err(SpatialAssemblyError::IncompatibleDriverTargets { coordinate })
    } else {
        Ok(())
    }
}

fn require_compatible_translation_driver_targets(
    assembly: &SpatialAssembly,
    coordinate: SpatialCoordinateId,
    target: f64,
) -> Result<(), SpatialAssemblyError> {
    if assembly.sources.iter().any(|source| {
        matches!(
            source.kind,
            SpatialSourceKind::TranslationPositionDriver {
                coordinate: existing_coordinate,
                target: existing_target,
            } if existing_coordinate == coordinate
                && existing_target.to_bits() != target.to_bits()
        )
    }) {
        Err(SpatialAssemblyError::IncompatibleDriverTargets { coordinate })
    } else {
        Ok(())
    }
}

fn validate_driver_consistency(assembly: &SpatialAssembly) -> Result<(), SpatialAssemblyError> {
    let mut hinge_targets = BTreeMap::<SpatialCoordinateId, SpatialHingeTarget>::new();
    let mut translation_targets = BTreeMap::<SpatialCoordinateId, f64>::new();
    for source in &assembly.sources {
        match source.kind {
            SpatialSourceKind::HingePositionDriver { coordinate, target } => {
                if hinge_targets
                    .insert(coordinate, target)
                    .is_some_and(|existing| !same_hinge_target(existing, target))
                {
                    return Err(SpatialAssemblyError::IncompatibleDriverTargets { coordinate });
                }
            }
            SpatialSourceKind::TranslationPositionDriver { coordinate, target }
                if translation_targets
                    .insert(coordinate, target)
                    .is_some_and(|existing| existing.to_bits() != target.to_bits()) =>
            {
                return Err(SpatialAssemblyError::IncompatibleDriverTargets { coordinate });
            }
            _ => {}
        }
    }
    Ok(())
}

fn same_hinge_target(first: SpatialHingeTarget, second: SpatialHingeTarget) -> bool {
    first.winding == second.winding
        && first.principal_phase.to_bits() == second.principal_phase.to_bits()
}

/// Exact mapping from one physical spatial source to core identity and rows.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialSourceMapping {
    pub source: SpatialSourceId,
    pub source_label: String,
    pub core_source_id: SourceConstraintId,
    pub residual_ids: Vec<ResidualId>,
}

/// Exact spatial body-to-Pose3-variable relationship in body insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpatialBodyVariableMapping {
    pub body_id: SpatialBodyId,
    pub variable_id: VariableId,
}

/// Read-only compiled physical spatial problem and exact domain mappings.
#[derive(Clone, Debug)]
pub struct CompiledSpatialAssembly {
    problem: Problem,
    body_variables: Vec<SpatialBodyVariableMapping>,
    source_mappings: Vec<SpatialSourceMapping>,
    point_features: Vec<SpatialPointFeature>,
    frame_features: Vec<SpatialFrameFeature>,
    axis_features: Vec<SpatialAxisFeature>,
    plane_features: Vec<SpatialPlaneFeature>,
}

impl CompiledSpatialAssembly {
    #[must_use]
    pub fn body_variables(&self) -> &[SpatialBodyVariableMapping] {
        &self.body_variables
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[SpatialSourceMapping] {
        &self.source_mappings
    }

    #[must_use]
    pub fn axis_features(&self) -> &[SpatialAxisFeature] {
        &self.axis_features
    }

    #[must_use]
    pub fn plane_features(&self) -> &[SpatialPlaneFeature] {
        &self.plane_features
    }

    #[must_use]
    pub fn variable_for_body(&self, body: SpatialBodyId) -> Option<VariableId> {
        self.body_variables
            .iter()
            .find_map(|mapping| (mapping.body_id == body).then_some(mapping.variable_id))
    }

    #[must_use]
    pub fn source_mapping(&self, source: SpatialSourceId) -> Option<&SpatialSourceMapping> {
        self.source_mappings
            .iter()
            .find(|mapping| mapping.source == source)
    }

    /// Checks every physical residual Jacobian against central right-retraction differences.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid finite-difference step or failed residual evaluation.
    pub fn check_jacobians(
        &self,
        normalized_step: f64,
    ) -> Result<geosolve_core::JacobianCheckReport, SpatialAssemblyError> {
        Ok(self.problem.check_jacobians(normalized_step)?)
    }

    pub(crate) fn add_numerical_pose_gauge(
        &mut self,
        body: SpatialBodyId,
        target: Pose3,
        model_scale: f64,
    ) -> Result<(), SpatialAssemblyError> {
        validate_pose(target)?;
        validate_model_scale(model_scale)?;
        let variable = self
            .variable_for_body(body)
            .ok_or(SpatialAssemblyError::UnknownBody(body))?;
        let source = self.problem.add_source(SourceConstraint::new(format!(
            "private spatial numerical gauge for body {body}"
        ))?);
        let value = VariableValue::Pose3(target.ambient());
        let residual = self.problem.add_residual(ResidualBlock::fixed_variable(
            source,
            variable,
            value,
            vec![model_scale, model_scale, model_scale, 1.0, 1.0, 1.0],
            private_gauge_audit_rows(body),
        )?)?;
        self.problem
            .declare_fixed_variable(variable, value, residual)?;
        Ok(())
    }
}

/// One solved spatial body pose in deterministic body order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialSolvedBody {
    pub body_id: SpatialBodyId,
    pub pose: Pose3,
}

/// One transformed spatial point feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTransformedPointFeature {
    pub feature_id: SpatialPointFeatureId,
    pub body_id: SpatialBodyId,
    pub world: Point3<f64>,
}

/// One transformed spatial frame feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTransformedFrameFeature {
    pub feature_id: SpatialFrameFeatureId,
    pub body_id: SpatialBodyId,
    pub world: Frame3,
}

/// One transformed directed axis with its persistent transverse clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTransformedAxisFeature {
    pub feature_id: SpatialAxisFeatureId,
    pub body_id: SpatialBodyId,
    pub world: Frame3,
}

impl SpatialTransformedAxisFeature {
    #[must_use]
    pub const fn world_frame(self) -> Frame3 {
        self.world
    }

    #[must_use]
    pub fn origin(self) -> Point3<f64> {
        self.world.origin()
    }

    #[must_use]
    pub fn direction(self) -> Vector3<f64> {
        self.world.z_axis()
    }

    #[must_use]
    pub fn axis(self) -> Vector3<f64> {
        self.direction()
    }

    #[must_use]
    pub fn x_clock(self) -> Vector3<f64> {
        self.world.x_axis()
    }

    #[must_use]
    pub fn y_clock(self) -> Vector3<f64> {
        self.world.y_axis()
    }
}

/// One transformed directed plane normal with its persistent in-plane clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTransformedPlaneFeature {
    pub feature_id: SpatialPlaneFeatureId,
    pub body_id: SpatialBodyId,
    pub world: Frame3,
}

impl SpatialTransformedPlaneFeature {
    #[must_use]
    pub const fn world_frame(self) -> Frame3 {
        self.world
    }

    #[must_use]
    pub fn origin(self) -> Point3<f64> {
        self.world.origin()
    }

    #[must_use]
    pub fn normal(self) -> Vector3<f64> {
        self.world.z_axis()
    }

    #[must_use]
    pub fn x_clock(self) -> Vector3<f64> {
        self.world.x_axis()
    }

    #[must_use]
    pub fn y_clock(self) -> Vector3<f64> {
        self.world.y_axis()
    }
}

/// Accepted finite body and transformed feature geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialGeometry {
    pub bodies: Vec<SpatialSolvedBody>,
    pub points: Vec<SpatialTransformedPointFeature>,
    pub frames: Vec<SpatialTransformedFrameFeature>,
    pub axes: Vec<SpatialTransformedAxisFeature>,
    pub planes: Vec<SpatialTransformedPlaneFeature>,
}

impl SpatialGeometry {
    #[must_use]
    pub fn body_pose(&self, body: SpatialBodyId) -> Option<Pose3> {
        self.bodies
            .iter()
            .find_map(|item| (item.body_id == body).then_some(item.pose))
    }

    #[must_use]
    pub fn world_point(&self, feature: SpatialPointFeatureId) -> Option<Point3<f64>> {
        self.points
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.world))
    }

    #[must_use]
    pub fn point(&self, feature: SpatialPointFeatureId) -> Option<Point3<f64>> {
        self.world_point(feature)
    }

    #[must_use]
    pub fn world_frame(&self, feature: SpatialFrameFeatureId) -> Option<Frame3> {
        self.frames
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.world))
    }

    #[must_use]
    pub fn frame(&self, feature: SpatialFrameFeatureId) -> Option<Frame3> {
        self.world_frame(feature)
    }

    #[must_use]
    pub fn axis_feature(
        &self,
        feature: SpatialAxisFeatureId,
    ) -> Option<&SpatialTransformedAxisFeature> {
        self.axes.iter().find(|item| item.feature_id == feature)
    }

    #[must_use]
    pub fn plane_feature(
        &self,
        feature: SpatialPlaneFeatureId,
    ) -> Option<&SpatialTransformedPlaneFeature> {
        self.planes.iter().find(|item| item.feature_id == feature)
    }

    #[must_use]
    pub fn world_axis_frame(&self, feature: SpatialAxisFeatureId) -> Option<Frame3> {
        self.axis_feature(feature).map(|item| item.world)
    }

    #[must_use]
    pub fn world_plane_frame(&self, feature: SpatialPlaneFeatureId) -> Option<Frame3> {
        self.plane_feature(feature).map(|item| item.world)
    }
}

/// Certification of the common-left world action for one domain component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialWorldActionCertification {
    FloatingSe3,
    PhysicallyGrounded,
}

/// Private numerical reference selected for one floating component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialGaugeReference {
    pub body: SpatialBodyId,
    pub target_pose: Pose3,
}

/// Gauge and equality mobility for one deterministic spatial domain component.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialComponentGaugeReport {
    pub component_index: usize,
    pub bodies: Vec<SpatialBodyId>,
    pub sources: Vec<SpatialSourceId>,
    pub mode_monitors: Vec<SpatialModeMonitorId>,
    pub core_component_indices: Vec<usize>,
    pub numerical_equality_right_nullity: usize,
    pub gauge_dof: usize,
    pub internal_mobility: usize,
    pub world_action: SpatialWorldActionCertification,
    pub physical_ground_sources: Vec<SpatialSourceId>,
    pub numerical_reference: Option<SpatialGaugeReference>,
}

/// Domain-certified split of physical equality mobility into gauge and internal motion.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialGaugeReport {
    pub numerical_equality_right_nullity: usize,
    pub gauge_dof: usize,
    pub internal_mobility: usize,
    pub components: Vec<SpatialComponentGaugeReport>,
}

/// Independently accepted spatial solve state and its physical core evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialSolveResult {
    pub geometry: SpatialGeometry,
    pub coordinate_values: Vec<SpatialCoordinateValue>,
    pub mode_evaluations: Vec<SpatialModeEvaluation>,
    pub branch_boundary_evaluations: Vec<SpatialBranchBoundaryEvaluation>,
    pub display_audit: AuditSnapshot,
    pub source_mappings: Vec<SpatialSourceMapping>,
    pub core_report: SolveReport,
    pub acceptance_hard_residual_max: f64,
}

/// One revision-checked spatial assembly edit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialPatch {
    BodyPoseGuess {
        body: SpatialBodyId,
        pose: Pose3,
    },
    PointLocal {
        feature: SpatialPointFeatureId,
        local_point: Point3<f64>,
    },
    FrameLocal {
        feature: SpatialFrameFeatureId,
        local_frame: Frame3,
    },
    AxisLocal {
        feature: SpatialAxisFeatureId,
        local_frame: Frame3,
    },
    PlaneLocal {
        feature: SpatialPlaneFeatureId,
        local_frame: Frame3,
    },
}

/// One edit in a revision-checked spatial assembly transaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialAssemblyEdit {
    BodyPoseGuess {
        body: SpatialBodyId,
        pose: Pose3,
    },
    PointLocal {
        feature: SpatialPointFeatureId,
        local_point: Point3<f64>,
    },
    FrameLocal {
        feature: SpatialFrameFeatureId,
        local_frame: Frame3,
    },
    AxisLocal {
        feature: SpatialAxisFeatureId,
        local_frame: Frame3,
    },
    PlaneLocal {
        feature: SpatialPlaneFeatureId,
        local_frame: Frame3,
    },
    HingeWinding {
        coordinate: SpatialCoordinateId,
        winding: i64,
    },
    HingeDriverTarget {
        source: SpatialSourceId,
        target: SpatialHingeTarget,
    },
    TranslationDriverTarget {
        source: SpatialSourceId,
        target: f64,
    },
    SourceAxisParity {
        source: SpatialSourceId,
        parity: SpatialAxisParity,
    },
    MonitorAxisParity {
        monitor: SpatialModeMonitorId,
        parity: SpatialAxisParity,
    },
    MonitorHingeWinding {
        monitor: SpatialModeMonitorId,
        winding: i64,
    },
    MonitorPlaneSide {
        monitor: SpatialModeMonitorId,
        side: SpatialModeSign,
    },
    MonitorSignedVolumeOrientation {
        monitor: SpatialModeMonitorId,
        orientation: SpatialModeSign,
    },
}

impl From<SpatialPatch> for SpatialAssemblyEdit {
    fn from(patch: SpatialPatch) -> Self {
        match patch {
            SpatialPatch::BodyPoseGuess { body, pose } => Self::BodyPoseGuess { body, pose },
            SpatialPatch::PointLocal {
                feature,
                local_point,
            } => Self::PointLocal {
                feature,
                local_point,
            },
            SpatialPatch::FrameLocal {
                feature,
                local_frame,
            } => Self::FrameLocal {
                feature,
                local_frame,
            },
            SpatialPatch::AxisLocal {
                feature,
                local_frame,
            } => Self::AxisLocal {
                feature,
                local_frame,
            },
            SpatialPatch::PlaneLocal {
                feature,
                local_frame,
            } => Self::PlaneLocal {
                feature,
                local_frame,
            },
        }
    }
}

/// One atomic spatial transaction. All edits are staged and solved together.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialAssemblyTransaction {
    pub expected_revision: u64,
    pub edits: Vec<SpatialAssemblyEdit>,
}

/// Explicit direction of a periodic hinge principal-phase transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialPrincipalCutDirection {
    PositiveToNegative,
    NegativeToPositive,
}

/// One high-level explicit spatial assembly-mode change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialAssemblyModeChange {
    SourceAxisParity {
        source: SpatialSourceId,
        parity: SpatialAxisParity,
    },
    MonitorAxisParity {
        monitor: SpatialModeMonitorId,
        parity: SpatialAxisParity,
    },
    MonitorPlaneSide {
        monitor: SpatialModeMonitorId,
        side: SpatialModeSign,
    },
    MonitorSignedVolume {
        monitor: SpatialModeMonitorId,
        orientation: SpatialModeSign,
    },
    HingePrincipalCut {
        coordinate: SpatialCoordinateId,
        direction: SpatialPrincipalCutDirection,
        new_principal_phase: f64,
    },
}

/// One revision-checked atomic mode change plus optional pose/driver seeds.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialModeChangeTransaction {
    pub expected_revision: u64,
    pub changes: Vec<SpatialAssemblyModeChange>,
    pub companion_edits: Vec<SpatialAssemblyEdit>,
}

impl SpatialAssemblyTransaction {
    #[must_use]
    pub const fn new(expected_revision: u64, edits: Vec<SpatialAssemblyEdit>) -> Self {
        Self {
            expected_revision,
            edits,
        }
    }

    #[must_use]
    pub fn one(expected_revision: u64, edit: SpatialAssemblyEdit) -> Self {
        Self::new(expected_revision, vec![edit])
    }

    pub fn push(&mut self, edit: SpatialAssemblyEdit) -> &mut Self {
        self.edits.push(edit);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SpatialAssemblyEditKey {
    BodyPose(SpatialBodyId),
    PointLocal(SpatialPointFeatureId),
    FrameLocal(SpatialFrameFeatureId),
    AxisLocal(SpatialAxisFeatureId),
    PlaneLocal(SpatialPlaneFeatureId),
    HingeWinding(SpatialCoordinateId),
    HingeDriverTarget(SpatialSourceId),
    TranslationDriverTarget(SpatialSourceId),
    SourceAxisParity(SpatialSourceId),
    ModeMonitor(SpatialModeMonitorId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SpatialScratchSolveSummary {
    iterations: usize,
    backend: Option<LinearSolveBackend>,
    sparse_fallback_reason: Option<SparseFallbackReason>,
}

/// Accepted spatial assembly plus its authoritative ungauged physical core session.
#[derive(Clone, Debug)]
pub struct SpatialAssemblySession {
    assembly: SpatialAssembly,
    core_session: SolveSession,
    body_variables: Vec<SpatialBodyVariableMapping>,
    source_mappings: Vec<SpatialSourceMapping>,
    accepted_result: SpatialSolveResult,
    gauge_report: SpatialGaugeReport,
    scratch_solve: SpatialScratchSolveSummary,
    config: SolverConfig,
}

impl SpatialAssemblySession {
    /// Solves through a private-gauge scratch problem, then publishes a separately
    /// solved and independently validated ungauged physical problem.
    ///
    /// # Errors
    ///
    /// Rejects invalid assembly/gauge data, unsuccessful core solves, invalid
    /// rank, non-finite geometry/audit, excessive residuals, or branch failures.
    #[allow(clippy::too_many_lines)]
    pub fn new(
        mut assembly: SpatialAssembly,
        config: SolverConfig,
    ) -> Result<Self, SpatialAssemblyError> {
        assembly.validate_structure()?;
        let components = certified_components(&assembly)?;
        let references = resolve_gauge_references(&assembly.gauge_policy, &components)?;

        let mut scratch = assembly.compile_validated()?;
        for body in references.iter().flatten() {
            let target = assembly.require_body(*body)?.pose_guess;
            scratch.add_numerical_pose_gauge(*body, target, assembly.model_scale)?;
        }
        let CompiledSpatialAssembly {
            problem,
            body_variables,
            source_mappings,
            point_features,
            frame_features,
            axis_features,
            plane_features,
        } = scratch;
        let scratch_session = accepted_session(problem, config, "private-gauge scratch solve")?;
        let scratch_solve = SpatialScratchSolveSummary {
            iterations: scratch_session.report().iterations,
            backend: scratch_session.report().actual_backend,
            sparse_fallback_reason: scratch_session.report().sparse_fallback_reason,
        };
        let scratch_geometry = solved_geometry_from_problem(
            scratch_session.problem(),
            &body_variables,
            &point_features,
            &frame_features,
            &axis_features,
            &plane_features,
        )?;
        let scratch_coordinate_values = accepted_coordinate_values(&assembly, &scratch_geometry)?;
        validate_physical_candidate(
            &assembly,
            &scratch_geometry,
            &scratch_coordinate_values,
            &scratch_session,
            &source_mappings,
            config,
        )?;
        project_geometry(&mut assembly, &scratch_geometry)?;

        let physical = assembly.compile_validated()?;
        let CompiledSpatialAssembly {
            problem,
            body_variables,
            source_mappings,
            point_features,
            frame_features,
            axis_features,
            plane_features,
        } = physical;
        let core_session = accepted_session(problem, config, "ungauged physical solve")?;
        let geometry = solved_geometry_from_problem(
            core_session.problem(),
            &body_variables,
            &point_features,
            &frame_features,
            &axis_features,
            &plane_features,
        )?;
        let coordinate_values = accepted_coordinate_values(&assembly, &geometry)?;
        let (acceptance_hard_residual_max, mode_evaluations) = validate_physical_candidate(
            &assembly,
            &geometry,
            &coordinate_values,
            &core_session,
            &source_mappings,
            config,
        )?;
        let branch_boundary_evaluations =
            initial_spatial_boundary_evaluations(&assembly, &geometry)?;
        project_geometry(&mut assembly, &geometry)?;
        let gauge_report = build_gauge_report(
            &assembly,
            &components,
            &references,
            &body_variables,
            &source_mappings,
            &core_session,
        )?;
        let core_report = core_session.report().clone();
        let accepted_result = SpatialSolveResult {
            geometry,
            coordinate_values,
            mode_evaluations,
            branch_boundary_evaluations,
            display_audit: core_report.audit.clone(),
            source_mappings: source_mappings.clone(),
            core_report,
            acceptance_hard_residual_max,
        };
        Ok(Self {
            assembly,
            core_session,
            body_variables,
            source_mappings,
            accepted_result,
            gauge_report,
            scratch_solve,
            config,
        })
    }

    #[must_use]
    pub const fn assembly(&self) -> &SpatialAssembly {
        &self.assembly
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.assembly.revision
    }

    #[must_use]
    pub const fn core_session(&self) -> &SolveSession {
        &self.core_session
    }

    #[must_use]
    pub const fn accepted_result(&self) -> &SpatialSolveResult {
        &self.accepted_result
    }

    #[must_use]
    pub const fn gauge_report(&self) -> &SpatialGaugeReport {
        &self.gauge_report
    }

    #[must_use]
    pub fn coordinate_values(&self) -> &[SpatialCoordinateValue] {
        &self.accepted_result.coordinate_values
    }

    #[must_use]
    pub fn coordinate_value(
        &self,
        coordinate: SpatialCoordinateId,
    ) -> Option<&SpatialCoordinateValue> {
        self.accepted_result
            .coordinate_values
            .iter()
            .find(|value| value.coordinate == coordinate)
    }

    #[must_use]
    pub fn mode_evaluations(&self) -> &[SpatialModeEvaluation] {
        &self.accepted_result.mode_evaluations
    }

    #[must_use]
    pub fn mode_evaluation(&self, monitor: SpatialModeMonitorId) -> Option<&SpatialModeEvaluation> {
        self.accepted_result
            .mode_evaluations
            .iter()
            .find(|evaluation| evaluation.monitor_id == monitor)
    }

    #[must_use]
    pub fn branch_boundary_evaluations(&self) -> &[SpatialBranchBoundaryEvaluation] {
        &self.accepted_result.branch_boundary_evaluations
    }

    #[must_use]
    pub fn body_variables(&self) -> &[SpatialBodyVariableMapping] {
        &self.body_variables
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[SpatialSourceMapping] {
        &self.source_mappings
    }

    /// Applies a batch on one staged clone and swaps only after complete acceptance.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions, duplicate semantic edits, stale or wrong-kind IDs,
    /// invalid final state, revision exhaustion, or any failed solve/validation while
    /// retaining every accepted view.
    pub fn apply_transaction(
        &mut self,
        transaction: SpatialAssemblyTransaction,
    ) -> Result<&SpatialSolveResult, SpatialAssemblyError> {
        self.require_revision(transaction.expected_revision)?;
        validate_transaction_edit_uniqueness(&transaction.edits)?;
        let mut candidate = self.assembly.clone();
        for edit in transaction.edits {
            apply_spatial_assembly_edit(&mut candidate, edit)?;
        }
        candidate.validate_structure()?;
        candidate.revision = transaction
            .expected_revision
            .checked_add(1)
            .ok_or(SpatialAssemblyError::RevisionExhausted)?;
        let mut replacement = Self::new(candidate, self.config)?;
        update_spatial_boundary_hysteresis(
            &self.accepted_result.branch_boundary_evaluations,
            &mut replacement.accepted_result.branch_boundary_evaluations,
            SpatialBoundaryObservation::CorrectedPhysicalEndpoint,
        );
        *self = replacement;
        Ok(&self.accepted_result)
    }

    /// Applies explicit parity, side, orientation, or hinge-cut mode changes atomically.
    ///
    /// Hinge-cut changes expand to the coordinate, every associated driver, and
    /// every associated winding monitor. The solved replacement must leave each
    /// changed boundary beyond the hysteresis leave threshold before one revision
    /// is committed.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions, wrong-kind IDs, duplicate lowered edits, invalid
    /// cut direction/phase, insufficient post-change clearance, or any failed
    /// solve while retaining every accepted view.
    #[allow(clippy::too_many_lines)]
    pub fn change_modes(
        &mut self,
        transaction: SpatialModeChangeTransaction,
    ) -> Result<&SpatialSolveResult, SpatialAssemblyError> {
        self.require_revision(transaction.expected_revision)?;
        if transaction.changes.is_empty() {
            return invalid_field(
                "spatial_mode_change.changes",
                "at least one explicit mode change is required",
            );
        }
        let mut edits = transaction.companion_edits;
        let mut expected_boundaries = Vec::new();
        for change in transaction.changes {
            match change {
                SpatialAssemblyModeChange::SourceAxisParity { source, parity } => {
                    self.assembly.require_source(source)?;
                    edits.push(SpatialAssemblyEdit::SourceAxisParity { source, parity });
                    expected_boundaries
                        .push(SpatialBranchBoundary::SourceAxisParity { source, parity });
                }
                SpatialAssemblyModeChange::MonitorAxisParity { monitor, parity } => {
                    self.assembly.require_mode_monitor(monitor)?;
                    edits.push(SpatialAssemblyEdit::MonitorAxisParity { monitor, parity });
                    expected_boundaries
                        .push(SpatialBranchBoundary::MonitorAxisParity { monitor, parity });
                }
                SpatialAssemblyModeChange::MonitorPlaneSide { monitor, side } => {
                    self.assembly.require_mode_monitor(monitor)?;
                    edits.push(SpatialAssemblyEdit::MonitorPlaneSide { monitor, side });
                    expected_boundaries
                        .push(SpatialBranchBoundary::MonitorPlaneSide { monitor, side });
                }
                SpatialAssemblyModeChange::MonitorSignedVolume {
                    monitor,
                    orientation,
                } => {
                    self.assembly.require_mode_monitor(monitor)?;
                    edits.push(SpatialAssemblyEdit::MonitorSignedVolumeOrientation {
                        monitor,
                        orientation,
                    });
                    expected_boundaries.push(SpatialBranchBoundary::MonitorSignedVolume {
                        monitor,
                        orientation,
                    });
                }
                SpatialAssemblyModeChange::HingePrincipalCut {
                    coordinate,
                    direction,
                    new_principal_phase,
                } => {
                    validate_hinge_target(SpatialHingeTarget {
                        principal_phase: new_principal_phase,
                        winding: 0,
                    })?;
                    let accepted = self
                        .coordinate_value(coordinate)
                        .ok_or(SpatialAssemblyError::UnknownCoordinate(coordinate))?;
                    let SpatialCoordinateValueKind::Hinge(accepted) = accepted.value else {
                        return Err(SpatialAssemblyError::WrongCoordinateKind {
                            coordinate,
                            expected: "hinge",
                        });
                    };
                    let old_clearance = std::f64::consts::PI - accepted.principal_phase.abs();
                    if old_clearance > SPATIAL_BOUNDARY_ENTER_CLEARANCE {
                        return invalid_field(
                            "spatial_mode_change.hinge_principal_cut",
                            "accepted hinge phase is not in the boundary event band",
                        );
                    }
                    let new_clearance = std::f64::consts::PI - new_principal_phase.abs();
                    if new_clearance < SPATIAL_BOUNDARY_LEAVE_CLEARANCE {
                        return invalid_field(
                            "spatial_mode_change.hinge_principal_cut",
                            "new hinge phase must leave the hysteresis band",
                        );
                    }
                    let new_winding = match direction {
                        SpatialPrincipalCutDirection::PositiveToNegative
                            if accepted.principal_phase >= 0.0 && new_principal_phase < 0.0 =>
                        {
                            accepted.winding.checked_add(1).ok_or_else(|| {
                                SpatialAssemblyError::InvalidField {
                                    field: "spatial_mode_change.hinge_winding",
                                    message: "hinge winding overflowed".to_owned(),
                                }
                            })?
                        }
                        SpatialPrincipalCutDirection::NegativeToPositive
                            if accepted.principal_phase < 0.0 && new_principal_phase >= 0.0 =>
                        {
                            accepted.winding.checked_sub(1).ok_or_else(|| {
                                SpatialAssemblyError::InvalidField {
                                    field: "spatial_mode_change.hinge_winding",
                                    message: "hinge winding underflowed".to_owned(),
                                }
                            })?
                        }
                        _ => {
                            return invalid_field(
                                "spatial_mode_change.hinge_principal_cut",
                                "cut direction does not match old and new principal phases",
                            );
                        }
                    };
                    edits.push(SpatialAssemblyEdit::HingeWinding {
                        coordinate,
                        winding: new_winding,
                    });
                    for source in &self.assembly.sources {
                        if matches!(
                            source.kind,
                            SpatialSourceKind::HingePositionDriver {
                                coordinate: candidate,
                                ..
                            } if candidate == coordinate
                        ) {
                            edits.push(SpatialAssemblyEdit::HingeDriverTarget {
                                source: source.id,
                                target: SpatialHingeTarget {
                                    principal_phase: new_principal_phase,
                                    winding: new_winding,
                                },
                            });
                        }
                    }
                    for monitor in &self.assembly.mode_monitors {
                        if matches!(
                            monitor.kind,
                            SpatialModeMonitorKind::HingeWinding {
                                coordinate: candidate,
                                ..
                            } if candidate == coordinate
                        ) {
                            edits.push(SpatialAssemblyEdit::MonitorHingeWinding {
                                monitor: monitor.id,
                                winding: new_winding,
                            });
                        }
                    }
                    expected_boundaries.push(SpatialBranchBoundary::HingePrincipalCut {
                        coordinate,
                        winding: new_winding,
                    });
                }
            }
        }
        let mut candidate = self.clone();
        candidate.apply_transaction(SpatialAssemblyTransaction::new(
            transaction.expected_revision,
            edits,
        ))?;
        for boundary in expected_boundaries {
            let evaluation = candidate
                .branch_boundary_evaluations()
                .iter()
                .find(|evaluation| evaluation.boundary == boundary)
                .ok_or_else(|| {
                    SpatialAssemblyError::IndependentValidation(format!(
                        "changed spatial boundary {boundary:?} is absent"
                    ))
                })?;
            if evaluation.clearance < SPATIAL_BOUNDARY_LEAVE_CLEARANCE {
                return independent(format!(
                    "changed spatial boundary {boundary:?} clearance {} is below leave threshold {}",
                    evaluation.clearance, SPATIAL_BOUNDARY_LEAVE_CLEARANCE
                ));
            }
        }
        *self = candidate;
        Ok(&self.accepted_result)
    }

    /// Applies one legacy patch through the atomic transaction path.
    ///
    /// # Errors
    ///
    /// Returns the corresponding transaction validation or solve error.
    pub fn apply_patch(
        &mut self,
        expected_revision: u64,
        patch: SpatialPatch,
    ) -> Result<&SpatialSolveResult, SpatialAssemblyError> {
        self.apply_transaction(SpatialAssemblyTransaction::one(
            expected_revision,
            patch.into(),
        ))
    }

    /// Replaces only numerical gauge metadata through the same atomic rebuild path.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions; duplicate, missing, stale, or grounded references;
    /// revision exhaustion; or any failed solve/validation without changing this session.
    pub fn set_gauge_policy(
        &mut self,
        expected_revision: u64,
        policy: SpatialGaugePolicy,
    ) -> Result<&SpatialSolveResult, SpatialAssemblyError> {
        self.require_revision(expected_revision)?;
        let mut candidate = self.assembly.clone();
        candidate.gauge_policy = policy;
        candidate.revision = expected_revision
            .checked_add(1)
            .ok_or(SpatialAssemblyError::RevisionExhausted)?;
        let mut replacement = Self::new(candidate, self.config)?;
        update_spatial_boundary_hysteresis(
            &self.accepted_result.branch_boundary_evaluations,
            &mut replacement.accepted_result.branch_boundary_evaluations,
            SpatialBoundaryObservation::CorrectedPhysicalEndpoint,
        );
        *self = replacement;
        Ok(&self.accepted_result)
    }

    fn require_revision(&self, expected: u64) -> Result<(), SpatialAssemblyError> {
        let actual = self.assembly.revision;
        if expected == actual {
            Ok(())
        } else {
            Err(SpatialAssemblyError::StaleRevision { expected, actual })
        }
    }
}

fn validate_transaction_edit_uniqueness(
    edits: &[SpatialAssemblyEdit],
) -> Result<(), SpatialAssemblyError> {
    let mut keys = BTreeSet::new();
    for edit in edits {
        let (key, role, id) = match *edit {
            SpatialAssemblyEdit::BodyPoseGuess { body, .. } => (
                SpatialAssemblyEditKey::BodyPose(body),
                "body pose",
                body.to_string(),
            ),
            SpatialAssemblyEdit::PointLocal { feature, .. } => (
                SpatialAssemblyEditKey::PointLocal(feature),
                "point local geometry",
                feature.to_string(),
            ),
            SpatialAssemblyEdit::FrameLocal { feature, .. } => (
                SpatialAssemblyEditKey::FrameLocal(feature),
                "frame local geometry",
                feature.to_string(),
            ),
            SpatialAssemblyEdit::AxisLocal { feature, .. } => (
                SpatialAssemblyEditKey::AxisLocal(feature),
                "axis local geometry",
                feature.to_string(),
            ),
            SpatialAssemblyEdit::PlaneLocal { feature, .. } => (
                SpatialAssemblyEditKey::PlaneLocal(feature),
                "plane local geometry",
                feature.to_string(),
            ),
            SpatialAssemblyEdit::HingeWinding { coordinate, .. } => (
                SpatialAssemblyEditKey::HingeWinding(coordinate),
                "hinge winding",
                coordinate.to_string(),
            ),
            SpatialAssemblyEdit::HingeDriverTarget { source, .. } => (
                SpatialAssemblyEditKey::HingeDriverTarget(source),
                "hinge driver target",
                source.to_string(),
            ),
            SpatialAssemblyEdit::TranslationDriverTarget { source, .. } => (
                SpatialAssemblyEditKey::TranslationDriverTarget(source),
                "translation driver target",
                source.to_string(),
            ),
            SpatialAssemblyEdit::SourceAxisParity { source, .. } => (
                SpatialAssemblyEditKey::SourceAxisParity(source),
                "source axis parity",
                source.to_string(),
            ),
            SpatialAssemblyEdit::MonitorAxisParity { monitor, .. } => (
                SpatialAssemblyEditKey::ModeMonitor(monitor),
                "axis-parity monitor state",
                monitor.to_string(),
            ),
            SpatialAssemblyEdit::MonitorHingeWinding { monitor, .. } => (
                SpatialAssemblyEditKey::ModeMonitor(monitor),
                "hinge-winding monitor state",
                monitor.to_string(),
            ),
            SpatialAssemblyEdit::MonitorPlaneSide { monitor, .. } => (
                SpatialAssemblyEditKey::ModeMonitor(monitor),
                "plane-side monitor state",
                monitor.to_string(),
            ),
            SpatialAssemblyEdit::MonitorSignedVolumeOrientation { monitor, .. } => (
                SpatialAssemblyEditKey::ModeMonitor(monitor),
                "signed-volume monitor state",
                monitor.to_string(),
            ),
        };
        if !keys.insert(key) {
            return Err(SpatialAssemblyError::DuplicateEdit { role, id });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn apply_spatial_assembly_edit(
    assembly: &mut SpatialAssembly,
    edit: SpatialAssemblyEdit,
) -> Result<(), SpatialAssemblyError> {
    match edit {
        SpatialAssemblyEdit::BodyPoseGuess { body, pose } => {
            validate_pose(pose)?;
            assembly
                .bodies
                .iter_mut()
                .find(|candidate| candidate.id == body)
                .ok_or(SpatialAssemblyError::UnknownBody(body))?
                .pose_guess = pose;
        }
        SpatialAssemblyEdit::PointLocal {
            feature,
            local_point,
        } => {
            validate_point(local_point, "transaction.point_local")?;
            assembly
                .point_features
                .iter_mut()
                .find(|candidate| candidate.id == feature)
                .ok_or(SpatialAssemblyError::UnknownPointFeature(feature))?
                .local_point = local_point;
        }
        SpatialAssemblyEdit::FrameLocal {
            feature,
            local_frame,
        } => {
            let local_frame = revalidate_frame(local_frame)?;
            assembly
                .frame_features
                .iter_mut()
                .find(|candidate| candidate.id == feature)
                .ok_or(SpatialAssemblyError::UnknownFrameFeature(feature))?
                .local_frame = local_frame;
        }
        SpatialAssemblyEdit::AxisLocal {
            feature,
            local_frame,
        } => {
            let local_frame = revalidate_frame(local_frame)?;
            assembly
                .axis_features
                .iter_mut()
                .find(|candidate| candidate.id == feature)
                .ok_or(SpatialAssemblyError::UnknownAxisFeature(feature))?
                .local_frame = local_frame;
        }
        SpatialAssemblyEdit::PlaneLocal {
            feature,
            local_frame,
        } => {
            let local_frame = revalidate_frame(local_frame)?;
            assembly
                .plane_features
                .iter_mut()
                .find(|candidate| candidate.id == feature)
                .ok_or(SpatialAssemblyError::UnknownPlaneFeature(feature))?
                .local_frame = local_frame;
        }
        SpatialAssemblyEdit::HingeWinding {
            coordinate,
            winding: new_winding,
        } => {
            let coordinate_definition = assembly
                .coordinates
                .iter_mut()
                .find(|candidate| candidate.id == coordinate)
                .ok_or(SpatialAssemblyError::UnknownCoordinate(coordinate))?;
            let SpatialCoordinateKind::Hinge { winding, .. } = &mut coordinate_definition.kind
            else {
                return Err(SpatialAssemblyError::WrongCoordinateKind {
                    coordinate,
                    expected: "hinge",
                });
            };
            *winding = new_winding;
        }
        SpatialAssemblyEdit::HingeDriverTarget { source, target } => {
            validate_hinge_target(target)?;
            let source_definition = assembly
                .sources
                .iter_mut()
                .find(|candidate| candidate.id == source)
                .ok_or(SpatialAssemblyError::UnknownSource(source))?;
            let SpatialSourceKind::HingePositionDriver {
                target: current, ..
            } = &mut source_definition.kind
            else {
                return Err(SpatialAssemblyError::WrongSourceKind {
                    source_id: source,
                    expected: "a hinge driver target edit",
                });
            };
            *current = target;
        }
        SpatialAssemblyEdit::TranslationDriverTarget { source, target } => {
            validate_translation_target(target)?;
            let source_definition = assembly
                .sources
                .iter_mut()
                .find(|candidate| candidate.id == source)
                .ok_or(SpatialAssemblyError::UnknownSource(source))?;
            let SpatialSourceKind::TranslationPositionDriver {
                target: current, ..
            } = &mut source_definition.kind
            else {
                return Err(SpatialAssemblyError::WrongSourceKind {
                    source_id: source,
                    expected: "a translation driver target edit",
                });
            };
            *current = target;
        }
        SpatialAssemblyEdit::SourceAxisParity { source, parity } => {
            let source_definition = assembly
                .sources
                .iter_mut()
                .find(|candidate| candidate.id == source)
                .ok_or(SpatialAssemblyError::UnknownSource(source))?;
            match &mut source_definition.kind {
                SpatialSourceKind::RevoluteJoint {
                    parity: current, ..
                }
                | SpatialSourceKind::PrismaticJoint {
                    parity: current, ..
                }
                | SpatialSourceKind::CylindricalJoint {
                    parity: current, ..
                }
                | SpatialSourceKind::PlanarJoint {
                    parity: current, ..
                }
                | SpatialSourceKind::AxisAlignmentMate {
                    parity: current, ..
                } => *current = parity,
                _ => {
                    return Err(SpatialAssemblyError::WrongSourceKind {
                        source_id: source,
                        expected: "an axis-parity edit",
                    });
                }
            }
        }
        SpatialAssemblyEdit::MonitorAxisParity { monitor, parity } => {
            let definition = assembly
                .mode_monitors
                .iter_mut()
                .find(|candidate| candidate.id == monitor)
                .ok_or(SpatialAssemblyError::UnknownModeMonitor(monitor))?;
            let SpatialModeMonitorKind::AxisParity {
                parity: current, ..
            } = &mut definition.kind
            else {
                return Err(SpatialAssemblyError::WrongModeMonitorKind {
                    monitor_id: monitor,
                    expected: "an axis-parity monitor edit",
                });
            };
            *current = parity;
        }
        SpatialAssemblyEdit::MonitorHingeWinding { monitor, winding } => {
            let definition = assembly
                .mode_monitors
                .iter_mut()
                .find(|candidate| candidate.id == monitor)
                .ok_or(SpatialAssemblyError::UnknownModeMonitor(monitor))?;
            let SpatialModeMonitorKind::HingeWinding {
                winding: current, ..
            } = &mut definition.kind
            else {
                return Err(SpatialAssemblyError::WrongModeMonitorKind {
                    monitor_id: monitor,
                    expected: "a hinge-winding monitor edit",
                });
            };
            *current = winding;
        }
        SpatialAssemblyEdit::MonitorPlaneSide { monitor, side } => {
            let definition = assembly
                .mode_monitors
                .iter_mut()
                .find(|candidate| candidate.id == monitor)
                .ok_or(SpatialAssemblyError::UnknownModeMonitor(monitor))?;
            let SpatialModeMonitorKind::PlaneSide { side: current, .. } = &mut definition.kind
            else {
                return Err(SpatialAssemblyError::WrongModeMonitorKind {
                    monitor_id: monitor,
                    expected: "a plane-side monitor edit",
                });
            };
            *current = side;
        }
        SpatialAssemblyEdit::MonitorSignedVolumeOrientation {
            monitor,
            orientation,
        } => {
            let definition = assembly
                .mode_monitors
                .iter_mut()
                .find(|candidate| candidate.id == monitor)
                .ok_or(SpatialAssemblyError::UnknownModeMonitor(monitor))?;
            let SpatialModeMonitorKind::SignedVolume {
                orientation: current,
                ..
            } = &mut definition.kind
            else {
                return Err(SpatialAssemblyError::WrongModeMonitorKind {
                    monitor_id: monitor,
                    expected: "a signed-volume monitor edit",
                });
            };
            *current = orientation;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct CertifiedSpatialComponent {
    bodies: Vec<SpatialBodyId>,
    sources: Vec<SpatialSourceId>,
    mode_monitors: Vec<SpatialModeMonitorId>,
    physical_ground_sources: Vec<SpatialSourceId>,
}

fn certified_components(
    assembly: &SpatialAssembly,
) -> Result<Vec<CertifiedSpatialComponent>, SpatialAssemblyError> {
    let bodies = assembly
        .bodies
        .iter()
        .map(|body| body.id)
        .collect::<Vec<_>>();
    let body_indices = bodies
        .iter()
        .enumerate()
        .map(|(index, body)| (*body, index))
        .collect::<HashMap<_, _>>();
    let mut parents = (0..bodies.len()).collect::<Vec<_>>();
    for source in &assembly.sources {
        let incident = source_bodies(assembly, source)?;
        if let Some((&first, rest)) = incident.split_first() {
            let first = *body_indices
                .get(&first)
                .ok_or(SpatialAssemblyError::UnknownBody(first))?;
            for body in rest {
                let next = *body_indices
                    .get(body)
                    .ok_or(SpatialAssemblyError::UnknownBody(*body))?;
                union_roots(&mut parents, first, next);
            }
        }
    }
    for monitor in &assembly.mode_monitors {
        let incident = mode_monitor_bodies(assembly, monitor)?;
        if let Some((&first, rest)) = incident.split_first() {
            let first = *body_indices
                .get(&first)
                .ok_or(SpatialAssemblyError::UnknownBody(first))?;
            for body in rest {
                let next = *body_indices
                    .get(body)
                    .ok_or(SpatialAssemblyError::UnknownBody(*body))?;
                union_roots(&mut parents, first, next);
            }
        }
    }

    let mut groups = BTreeMap::<SpatialBodyId, Vec<SpatialBodyId>>::new();
    for (index, body) in bodies.iter().copied().enumerate() {
        let root = find_root(&mut parents, index);
        groups.entry(bodies[root]).or_default().push(body);
    }
    let mut components = groups
        .into_values()
        .map(|mut component_bodies| {
            component_bodies.sort_unstable();
            CertifiedSpatialComponent {
                bodies: component_bodies,
                sources: Vec::new(),
                mode_monitors: Vec::new(),
                physical_ground_sources: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    components.sort_by_key(|component| component.bodies[0]);
    let component_for_body = components
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.bodies.iter().map(move |body| (*body, index)))
        .collect::<HashMap<_, _>>();
    for source in &assembly.sources {
        let incident = source_bodies(assembly, source)?;
        let body = incident.first().copied().ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "source {} has no incident body",
                source.id
            ))
        })?;
        let component_index = *component_for_body
            .get(&body)
            .ok_or(SpatialAssemblyError::UnknownBody(body))?;
        components[component_index].sources.push(source.id);
        if matches!(source.kind, SpatialSourceKind::PhysicalGround { .. }) {
            components[component_index]
                .physical_ground_sources
                .push(source.id);
        }
    }
    for monitor in &assembly.mode_monitors {
        let incident = mode_monitor_bodies(assembly, monitor)?;
        let body = incident.first().copied().ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "mode monitor {} has no incident body",
                monitor.id
            ))
        })?;
        let component_index = *component_for_body
            .get(&body)
            .ok_or(SpatialAssemblyError::UnknownBody(body))?;
        components[component_index].mode_monitors.push(monitor.id);
    }
    Ok(components)
}

fn source_bodies(
    assembly: &SpatialAssembly,
    source: &SpatialSource,
) -> Result<Vec<SpatialBodyId>, SpatialAssemblyError> {
    let mut bodies = match source.kind {
        SpatialSourceKind::PhysicalGround { body, .. } => vec![body],
        SpatialSourceKind::BallJoint { first, second }
        | SpatialSourceKind::PointDistanceMate { first, second, .. } => vec![
            assembly.require_point_feature(first)?.body,
            assembly.require_point_feature(second)?.body,
        ],
        SpatialSourceKind::FixedFrame { first, second }
        | SpatialSourceKind::RevoluteJoint { first, second, .. }
        | SpatialSourceKind::FrameOffsetMate { first, second, .. } => vec![
            assembly.require_frame_feature(first)?.body,
            assembly.require_frame_feature(second)?.body,
        ],
        SpatialSourceKind::PrismaticJoint { first, second, .. }
        | SpatialSourceKind::CylindricalJoint { first, second, .. }
        | SpatialSourceKind::UniversalJoint { first, second }
        | SpatialSourceKind::AxisAngleMate { first, second, .. }
        | SpatialSourceKind::AxisAlignmentMate { first, second, .. } => vec![
            assembly.require_axis_feature(first)?.body,
            assembly.require_axis_feature(second)?.body,
        ],
        SpatialSourceKind::PlanarJoint { first, second, .. } => vec![
            assembly.require_plane_feature(first)?.body,
            assembly.require_plane_feature(second)?.body,
        ],
        SpatialSourceKind::HingePositionDriver { coordinate, .. } => {
            let coordinate = assembly.require_coordinate(coordinate)?;
            let resolved = resolve_hinge_coordinate_definition(assembly, coordinate)?;
            vec![resolved.first_body, resolved.second_body]
        }
        SpatialSourceKind::TranslationPositionDriver { coordinate, .. } => {
            let coordinate = assembly.require_coordinate(coordinate)?;
            let resolved = resolve_translation_coordinate_definition(assembly, coordinate)?;
            vec![resolved.first_body, resolved.second_body]
        }
    };
    bodies.sort_unstable();
    bodies.dedup();
    Ok(bodies)
}

fn mode_monitor_bodies(
    assembly: &SpatialAssembly,
    monitor: &SpatialModeMonitor,
) -> Result<Vec<SpatialBodyId>, SpatialAssemblyError> {
    let mut bodies = match monitor.kind {
        SpatialModeMonitorKind::AxisParity { first, second, .. } => vec![
            assembly.require_axis_feature(first)?.body,
            assembly.require_axis_feature(second)?.body,
        ],
        SpatialModeMonitorKind::HingeWinding { coordinate, .. } => {
            let coordinate = assembly.require_coordinate(coordinate)?;
            let resolved = resolve_hinge_coordinate_definition(assembly, coordinate)?;
            vec![resolved.first_body, resolved.second_body]
        }
        SpatialModeMonitorKind::PlaneSide { plane, point, .. } => vec![
            assembly.require_plane_feature(plane)?.body,
            assembly.require_point_feature(point)?.body,
        ],
        SpatialModeMonitorKind::SignedVolume { points, .. } => points
            .into_iter()
            .map(|point| Ok(assembly.require_point_feature(point)?.body))
            .collect::<Result<Vec<_>, SpatialAssemblyError>>()?,
    };
    bodies.sort_unstable();
    bodies.dedup();
    Ok(bodies)
}

fn resolve_gauge_references(
    policy: &SpatialGaugePolicy,
    components: &[CertifiedSpatialComponent],
) -> Result<Vec<Option<SpatialBodyId>>, SpatialAssemblyError> {
    match policy {
        SpatialGaugePolicy::LowestPersistentBody => Ok(components
            .iter()
            .map(|component| {
                component
                    .physical_ground_sources
                    .is_empty()
                    .then_some(component.bodies[0])
            })
            .collect()),
        SpatialGaugePolicy::ExplicitReferences { bodies } => {
            if bodies.iter().copied().collect::<BTreeSet<_>>().len() != bodies.len() {
                return Err(SpatialAssemblyError::InvalidGaugePolicy(
                    "explicit references must be unique".to_owned(),
                ));
            }
            let all_bodies = components
                .iter()
                .flat_map(|component| component.bodies.iter().copied())
                .collect::<BTreeSet<_>>();
            if let Some(body) = bodies.iter().find(|body| !all_bodies.contains(body)) {
                return Err(SpatialAssemblyError::UnknownBody(*body));
            }
            components
                .iter()
                .map(|component| {
                    let selected = bodies
                        .iter()
                        .copied()
                        .filter(|body| component.bodies.contains(body))
                        .collect::<Vec<_>>();
                    if component.physical_ground_sources.is_empty() {
                        if selected.len() == 1 {
                            Ok(Some(selected[0]))
                        } else {
                            Err(SpatialAssemblyError::InvalidGaugePolicy(format!(
                                "floating component beginning at {} requires exactly one reference",
                                component.bodies[0]
                            )))
                        }
                    } else if selected.is_empty() {
                        Ok(None)
                    } else {
                        Err(SpatialAssemblyError::InvalidGaugePolicy(format!(
                            "physically grounded component beginning at {} cannot have a numerical reference",
                            component.bodies[0]
                        )))
                    }
                })
                .collect()
        }
    }
}

#[allow(clippy::too_many_lines)]
fn build_gauge_report(
    assembly: &SpatialAssembly,
    certified: &[CertifiedSpatialComponent],
    references: &[Option<SpatialBodyId>],
    body_variables: &[SpatialBodyVariableMapping],
    source_mappings: &[SpatialSourceMapping],
    session: &SolveSession,
) -> Result<SpatialGaugeReport, SpatialAssemblyError> {
    let component_for_body = certified
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.bodies.iter().map(move |body| (*body, index)))
        .collect::<HashMap<_, _>>();
    let mut variable_components = HashMap::new();
    for mapping in body_variables {
        let component = *component_for_body.get(&mapping.body_id).ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "body {} has no certified component",
                mapping.body_id
            ))
        })?;
        variable_components.insert(mapping.variable_id, component);
    }
    let source_components = certified
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.sources.iter().map(move |source| (*source, index)))
        .collect::<HashMap<_, _>>();
    let mut residual_components = HashMap::new();
    for mapping in source_mappings {
        let component = *source_components.get(&mapping.source).ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "physical source {} has no certified component",
                mapping.source
            ))
        })?;
        for residual in &mapping.residual_ids {
            residual_components.insert(*residual, component);
        }
    }
    let mut core_components = vec![Vec::new(); certified.len()];
    let mut right_nullities = vec![0_usize; certified.len()];
    for summary in &session.report().structural.component_summaries {
        let mut domain_components = summary
            .variable_ids
            .iter()
            .map(|variable| {
                variable_components.get(variable).copied().ok_or_else(|| {
                    SpatialAssemblyError::GaugeCertification(format!(
                        "core variable {variable:?} is not a spatial body"
                    ))
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        for residual in &summary.residual_ids {
            domain_components.insert(*residual_components.get(residual).ok_or_else(|| {
                SpatialAssemblyError::GaugeCertification(format!(
                    "core residual {residual:?} is not mapped to a spatial source"
                ))
            })?);
        }
        if domain_components.len() != 1 {
            return Err(SpatialAssemblyError::GaugeCertification(format!(
                "core component {} does not map to exactly one spatial component",
                summary.component_index
            )));
        }
        let domain_component = *domain_components.iter().next().expect("length checked");
        let solve = session
            .report()
            .component_solves
            .iter()
            .find(|solve| solve.component_index == summary.component_index)
            .ok_or_else(|| {
                SpatialAssemblyError::GaugeCertification(format!(
                    "core component {} has no numerical report",
                    summary.component_index
                ))
            })?;
        core_components[domain_component].push(summary.component_index);
        right_nullities[domain_component] = right_nullities[domain_component]
            .checked_add(solve.right_nullity)
            .ok_or_else(|| {
                SpatialAssemblyError::GaugeCertification(
                    "component right nullity overflowed".to_owned(),
                )
            })?;
    }

    let mut components = Vec::with_capacity(certified.len());
    for (index, component) in certified.iter().enumerate() {
        let (world_action, gauge_dof, numerical_reference) = if let Some(body) = references[index] {
            let target_pose = assembly.require_body(body)?.pose_guess;
            (
                SpatialWorldActionCertification::FloatingSe3,
                6,
                Some(SpatialGaugeReference { body, target_pose }),
            )
        } else {
            (SpatialWorldActionCertification::PhysicallyGrounded, 0, None)
        };
        if right_nullities[index] < gauge_dof {
            return Err(SpatialAssemblyError::GaugeCertification(format!(
                "component {index} has right nullity {} below certified gauge DOF {gauge_dof}",
                right_nullities[index]
            )));
        }
        components.push(SpatialComponentGaugeReport {
            component_index: index,
            bodies: component.bodies.clone(),
            sources: component.sources.clone(),
            mode_monitors: component.mode_monitors.clone(),
            core_component_indices: core_components[index].clone(),
            numerical_equality_right_nullity: right_nullities[index],
            gauge_dof,
            internal_mobility: right_nullities[index] - gauge_dof,
            world_action,
            physical_ground_sources: component.physical_ground_sources.clone(),
            numerical_reference,
        });
    }
    let numerical_equality_right_nullity = checked_sum(
        right_nullities.iter().copied(),
        "total equality right nullity overflowed",
    )?;
    if numerical_equality_right_nullity != session.report().right_nullity {
        return Err(SpatialAssemblyError::GaugeCertification(format!(
            "mapped right nullity {numerical_equality_right_nullity} does not match core {}",
            session.report().right_nullity
        )));
    }
    let gauge_dof = checked_sum(
        components.iter().map(|component| component.gauge_dof),
        "total gauge DOF overflowed",
    )?;
    let internal_mobility = checked_sum(
        components
            .iter()
            .map(|component| component.internal_mobility),
        "total internal mobility overflowed",
    )?;
    Ok(SpatialGaugeReport {
        numerical_equality_right_nullity,
        gauge_dof,
        internal_mobility,
        components,
    })
}

fn accepted_session(
    problem: Problem,
    config: SolverConfig,
    stage: &'static str,
) -> Result<SolveSession, SpatialAssemblyError> {
    SolveSession::new(problem, config)
        .map_err(|error| SpatialAssemblyError::InitialRejected(format!("{stage}: {error}")))
}

fn solved_geometry_from_problem(
    problem: &Problem,
    body_variables: &[SpatialBodyVariableMapping],
    point_features: &[SpatialPointFeature],
    frame_features: &[SpatialFrameFeature],
    axis_features: &[SpatialAxisFeature],
    plane_features: &[SpatialPlaneFeature],
) -> Result<SpatialGeometry, SpatialAssemblyError> {
    let mut bodies = Vec::with_capacity(body_variables.len());
    let mut poses = HashMap::with_capacity(body_variables.len());
    for mapping in body_variables {
        let variable = problem
            .variable(mapping.variable_id)
            .ok_or(CoreError::UnknownVariable(mapping.variable_id))?;
        let VariableValue::Pose3(ambient) = variable.value() else {
            return Err(CoreError::VariableKindMismatch {
                expected: VariableKind::Pose3,
                actual: variable.kind(),
            }
            .into());
        };
        let pose = Pose3::from_ambient(ambient)?;
        bodies.push(SpatialSolvedBody {
            body_id: mapping.body_id,
            pose,
        });
        poses.insert(mapping.body_id, pose);
    }
    let points = point_features
        .iter()
        .map(|feature| {
            let pose = poses
                .get(&feature.body)
                .copied()
                .ok_or(SpatialAssemblyError::UnknownBody(feature.body))?;
            Ok(SpatialTransformedPointFeature {
                feature_id: feature.id,
                body_id: feature.body,
                world: pose.try_transform_point(feature.local_point)?,
            })
        })
        .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
    let frames = frame_features
        .iter()
        .map(|feature| {
            let pose = poses
                .get(&feature.body)
                .copied()
                .ok_or(SpatialAssemblyError::UnknownBody(feature.body))?;
            Ok(SpatialTransformedFrameFeature {
                feature_id: feature.id,
                body_id: feature.body,
                world: transform_frame(pose, feature.local_frame)?,
            })
        })
        .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
    let axes = axis_features
        .iter()
        .map(|feature| {
            let pose = poses
                .get(&feature.body)
                .copied()
                .ok_or(SpatialAssemblyError::UnknownBody(feature.body))?;
            Ok(SpatialTransformedAxisFeature {
                feature_id: feature.id,
                body_id: feature.body,
                world: transform_frame(pose, feature.local_frame)?,
            })
        })
        .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
    let planes = plane_features
        .iter()
        .map(|feature| {
            let pose = poses
                .get(&feature.body)
                .copied()
                .ok_or(SpatialAssemblyError::UnknownBody(feature.body))?;
            Ok(SpatialTransformedPlaneFeature {
                feature_id: feature.id,
                body_id: feature.body,
                world: transform_frame(pose, feature.local_frame)?,
            })
        })
        .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
    Ok(SpatialGeometry {
        bodies,
        points,
        frames,
        axes,
        planes,
    })
}

fn accepted_coordinate_values(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
) -> Result<Vec<SpatialCoordinateValue>, SpatialAssemblyError> {
    assembly
        .coordinates
        .iter()
        .map(|coordinate| {
            let value = match coordinate.kind {
                SpatialCoordinateKind::Hinge { winding, .. } => {
                    let resolved = resolve_hinge_coordinate_definition(assembly, coordinate)?;
                    let (first, second) = resolved_coordinate_world_frames(geometry, resolved)?;
                    require_parity_branch(
                        resolved.parent_source,
                        "hinge coordinate parent",
                        resolved.parity,
                        first.z_axis(),
                        second.z_axis(),
                    )?;
                    let sine = first.y_axis().dot(&second.x_axis());
                    let cosine = first.x_axis().dot(&second.x_axis());
                    let projection_norm = sine.hypot(cosine);
                    if !sine.is_finite()
                        || !cosine.is_finite()
                        || !projection_norm.is_finite()
                        || projection_norm <= ORIENTATION_BRANCH_MARGIN
                    {
                        return independent(format!(
                            "hinge coordinate {} has a non-finite or ambiguous clock projection",
                            coordinate.id
                        ));
                    }
                    let principal_phase = canonical_phase(sine.atan2(cosine))?;
                    SpatialCoordinateValueKind::Hinge(SpatialHingeCoordinateValue {
                        principal_phase,
                        winding,
                    })
                }
                SpatialCoordinateKind::AxialTranslation { .. }
                | SpatialCoordinateKind::PlanarTranslation { .. } => {
                    let resolved = resolve_translation_coordinate_definition(assembly, coordinate)?;
                    let (first, second) = resolved_coordinate_world_frames(geometry, resolved)?;
                    require_parity_branch(
                        resolved.parent_source,
                        "translation coordinate parent",
                        resolved.parity,
                        first.z_axis(),
                        second.z_axis(),
                    )?;
                    let local_axis = translation_local_axis(coordinate, resolved)?;
                    let first_pose = geometry
                        .body_pose(resolved.first_body)
                        .ok_or(SpatialAssemblyError::UnknownBody(resolved.first_body))?;
                    let world_axis = first_pose.try_transform_vector(local_axis)?;
                    let value = world_axis.dot(&(second.origin() - first.origin()));
                    if !value.is_finite() {
                        return independent(format!(
                            "translation coordinate {} is non-finite",
                            coordinate.id
                        ));
                    }
                    match coordinate.kind {
                        SpatialCoordinateKind::AxialTranslation { .. } => {
                            SpatialCoordinateValueKind::AxialTranslation(value)
                        }
                        SpatialCoordinateKind::PlanarTranslation { axis, .. } => {
                            SpatialCoordinateValueKind::PlanarTranslation { axis, value }
                        }
                        SpatialCoordinateKind::Hinge { .. } => unreachable!("matched above"),
                    }
                }
            };
            Ok(SpatialCoordinateValue {
                coordinate: coordinate.id,
                coordinate_label: coordinate.label.clone(),
                value,
            })
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn evaluate_mode_monitors(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
    coordinate_values: &[SpatialCoordinateValue],
) -> Result<Vec<SpatialModeEvaluation>, SpatialAssemblyError> {
    assembly
        .mode_monitors
        .iter()
        .map(|monitor| {
            let involved_bodies = mode_monitor_bodies(assembly, monitor)?;
            let (fresh_raw_metric, retained_normalized_metric, involved_features, coordinate, winding) =
                match monitor.kind {
                    SpatialModeMonitorKind::AxisParity {
                        first,
                        second,
                        parity,
                    } => {
                        let first_frame = geometry
                            .world_axis_frame(first)
                            .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
                        let second_frame = geometry
                            .world_axis_frame(second)
                            .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
                        let raw = first_frame.z_axis().dot(&second_frame.z_axis());
                        let retained = parity.multiplier() * raw;
                        require_retained_mode_metric(
                            monitor.id,
                            retained,
                            "axis parity",
                        )?;
                        (
                            Some(raw),
                            retained,
                            vec![SpatialModeFeature::Axis(first), SpatialModeFeature::Axis(second)],
                            None,
                            None,
                        )
                    }
                    SpatialModeMonitorKind::HingeWinding {
                        coordinate,
                        winding,
                    } => {
                        let coordinate_definition = assembly.require_coordinate(coordinate)?;
                        let SpatialCoordinateKind::Hinge {
                            winding: coordinate_winding,
                            ..
                        } = coordinate_definition.kind
                        else {
                            return Err(SpatialAssemblyError::WrongCoordinateKind {
                                coordinate,
                                expected: "hinge",
                            });
                        };
                        require_matching_winding(coordinate, coordinate_winding, winding)?;
                        let accepted = coordinate_values
                            .iter()
                            .find(|value| value.coordinate == coordinate)
                            .ok_or(SpatialAssemblyError::UnknownCoordinate(coordinate))?;
                        let SpatialCoordinateValueKind::Hinge(accepted) = accepted.value else {
                            return Err(SpatialAssemblyError::WrongCoordinateKind {
                                coordinate,
                                expected: "hinge",
                            });
                        };
                        require_matching_winding(coordinate, accepted.winding, winding)?;
                        let resolved =
                            resolve_hinge_coordinate_definition(assembly, coordinate_definition)?;
                        let (first, second) =
                            resolved_coordinate_world_frames(geometry, resolved)?;
                        let parity_metric = first
                            .z_axis()
                            .dot(&(second.z_axis() * resolved.parity.multiplier()));
                        require_retained_mode_metric(
                            monitor.id,
                            parity_metric,
                            "hinge parent axis parity",
                        )?;
                        let sine = first.y_axis().dot(&second.x_axis());
                        let cosine = first.x_axis().dot(&second.x_axis());
                        let projection = sine.hypot(cosine);
                        require_retained_mode_metric(
                            monitor.id,
                            projection,
                            "hinge clock projection",
                        )?;
                        let principal_phase = canonical_phase(sine.atan2(cosine))?;
                        let phase_difference =
                            canonical_phase(principal_phase - accepted.principal_phase)?;
                        if phase_difference.abs() > 8.0 * f64::EPSILON {
                            return independent(format!(
                                "hinge-winding monitor {} fresh phase does not match its accepted coordinate value",
                                monitor.id
                            ));
                        }
                        (
                            Some(principal_phase),
                            projection,
                            hinge_monitor_features(assembly, coordinate_definition)?,
                            Some(coordinate),
                            Some(winding),
                        )
                    }
                    SpatialModeMonitorKind::PlaneSide { plane, point, side } => {
                        let plane_frame = geometry
                            .world_plane_frame(plane)
                            .ok_or(SpatialAssemblyError::UnknownPlaneFeature(plane))?;
                        let point_world = geometry
                            .world_point(point)
                            .ok_or(SpatialAssemblyError::UnknownPointFeature(point))?;
                        let raw = plane_frame
                            .z_axis()
                            .dot(&(point_world - plane_frame.origin()));
                        let retained = side.multiplier() * raw / assembly.model_scale;
                        require_retained_mode_metric(monitor.id, retained, "plane side")?;
                        (
                            Some(raw),
                            retained,
                            vec![SpatialModeFeature::Plane(plane), SpatialModeFeature::Point(point)],
                            None,
                            None,
                        )
                    }
                    SpatialModeMonitorKind::SignedVolume {
                        points,
                        orientation,
                    } => {
                        require_distinct_volume_points(points)?;
                        let [a, b, c, d] = points.map(|point| {
                            geometry
                                .world_point(point)
                                .ok_or(SpatialAssemblyError::UnknownPointFeature(point))
                        });
                        let [a, b, c, d] = [a?, b?, c?, d?];
                        let ab = finite_unit_edge(b - a, monitor.id, "B-A")?;
                        let ac = finite_unit_edge(c - a, monitor.id, "C-A")?;
                        let ad = finite_unit_edge(d - a, monitor.id, "D-A")?;
                        let cross = ab.cross(&ac);
                        let cross_norm = robust_norm(cross);
                        if !cross.iter().all(|value| value.is_finite())
                            || !cross_norm.is_finite()
                            || cross_norm == 0.0
                        {
                            return independent(format!(
                                "signed-volume monitor {} has collinear A/B/C geometry",
                                monitor.id
                            ));
                        }
                        let raw = cross.dot(&ad);
                        let retained = orientation.multiplier() * raw;
                        require_retained_mode_metric(
                            monitor.id,
                            retained,
                            "signed volume",
                        )?;
                        (
                            Some(raw),
                            retained,
                            points
                                .into_iter()
                                .map(SpatialModeFeature::Point)
                                .collect(),
                            None,
                            None,
                        )
                    }
                };
            if fresh_raw_metric.is_some_and(|metric| !metric.is_finite())
                || !retained_normalized_metric.is_finite()
            {
                return independent(format!(
                    "spatial mode monitor {} produced non-finite evaluation data",
                    monitor.id
                ));
            }
            Ok(SpatialModeEvaluation {
                monitor_id: monitor.id,
                monitor_label: monitor.label.clone(),
                kind: monitor.kind,
                fresh_raw_metric,
                retained_normalized_metric,
                retained: true,
                involved_bodies,
                involved_features,
                coordinate,
                winding,
            })
        })
        .collect()
}

fn initial_spatial_boundary_evaluations(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
) -> Result<Vec<SpatialBranchBoundaryEvaluation>, SpatialAssemblyError> {
    spatial_branch_boundaries(assembly)
        .into_iter()
        .map(|boundary| {
            let (raw_metric, clearance) =
                evaluate_spatial_branch_boundary(assembly, geometry, boundary)?;
            Ok(SpatialBranchBoundaryEvaluation {
                boundary,
                raw_metric,
                clearance,
                hysteresis_state: initial_spatial_hysteresis(clearance)?,
            })
        })
        .collect()
}

fn spatial_branch_boundaries(assembly: &SpatialAssembly) -> Vec<SpatialBranchBoundary> {
    let mut boundaries = Vec::new();
    for source in &assembly.sources {
        match source.kind {
            SpatialSourceKind::FixedFrame { .. } => {
                for axis in [
                    SpatialFrameAxis::X,
                    SpatialFrameAxis::Y,
                    SpatialFrameAxis::Z,
                ] {
                    boundaries.push(SpatialBranchBoundary::FixedFrameDiagonal {
                        source: source.id,
                        axis,
                    });
                }
            }
            SpatialSourceKind::FrameOffsetMate { .. } => {
                for axis in [
                    SpatialFrameAxis::X,
                    SpatialFrameAxis::Y,
                    SpatialFrameAxis::Z,
                ] {
                    boundaries.push(SpatialBranchBoundary::FrameOffsetDiagonal {
                        source: source.id,
                        axis,
                    });
                }
            }
            SpatialSourceKind::RevoluteJoint { parity, .. }
            | SpatialSourceKind::PrismaticJoint { parity, .. }
            | SpatialSourceKind::CylindricalJoint { parity, .. }
            | SpatialSourceKind::PlanarJoint { parity, .. }
            | SpatialSourceKind::AxisAlignmentMate { parity, .. } => {
                boundaries.push(SpatialBranchBoundary::SourceAxisParity {
                    source: source.id,
                    parity,
                });
                if matches!(source.kind, SpatialSourceKind::PrismaticJoint { .. }) {
                    boundaries
                        .push(SpatialBranchBoundary::PrismaticClockRoot { source: source.id });
                }
            }
            SpatialSourceKind::HingePositionDriver { coordinate, .. } => {
                boundaries.push(SpatialBranchBoundary::HingeDriverPositiveRoot {
                    source: source.id,
                    coordinate,
                });
            }
            _ => {}
        }
    }
    for coordinate in &assembly.coordinates {
        if let SpatialCoordinateKind::Hinge { winding, .. } = coordinate.kind {
            boundaries.push(SpatialBranchBoundary::HingePrincipalCut {
                coordinate: coordinate.id,
                winding,
            });
        }
    }
    for monitor in &assembly.mode_monitors {
        let boundary = match monitor.kind {
            SpatialModeMonitorKind::AxisParity { parity, .. } => {
                Some(SpatialBranchBoundary::MonitorAxisParity {
                    monitor: monitor.id,
                    parity,
                })
            }
            SpatialModeMonitorKind::PlaneSide { side, .. } => {
                Some(SpatialBranchBoundary::MonitorPlaneSide {
                    monitor: monitor.id,
                    side,
                })
            }
            SpatialModeMonitorKind::SignedVolume { orientation, .. } => {
                Some(SpatialBranchBoundary::MonitorSignedVolume {
                    monitor: monitor.id,
                    orientation,
                })
            }
            SpatialModeMonitorKind::HingeWinding { .. } => None,
        };
        boundaries.extend(boundary);
    }
    boundaries
}

#[allow(clippy::too_many_lines)]
fn evaluate_spatial_branch_boundary(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
    boundary: SpatialBranchBoundary,
) -> Result<(f64, f64), SpatialAssemblyError> {
    let (raw_metric, multiplier) = match boundary {
        SpatialBranchBoundary::FixedFrameDiagonal { source, axis } => {
            let source_definition = assembly.require_source(source)?;
            let SpatialSourceKind::FixedFrame { first, second } = source_definition.kind else {
                return independent("fixed-frame boundary source changed kind");
            };
            let first = geometry
                .world_frame(first)
                .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
            let second = geometry
                .world_frame(second)
                .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
            (frame_axis(first, axis).dot(&frame_axis(second, axis)), 1.0)
        }
        SpatialBranchBoundary::FrameOffsetDiagonal { source, axis } => {
            let source_definition = assembly.require_source(source)?;
            let SpatialSourceKind::FrameOffsetMate {
                first,
                second,
                offset,
            } = source_definition.kind
            else {
                return independent("frame-offset boundary source changed kind");
            };
            let first = geometry
                .world_frame(first)
                .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
            let expected = compose_frames(first, offset)?;
            let second = geometry
                .world_frame(second)
                .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
            (
                frame_axis(expected, axis).dot(&frame_axis(second, axis)),
                1.0,
            )
        }
        SpatialBranchBoundary::SourceAxisParity { source, parity } => {
            let source_definition = assembly.require_source(source)?;
            let raw = match source_definition.kind {
                SpatialSourceKind::RevoluteJoint { first, second, .. } => {
                    let first = geometry
                        .world_frame(first)
                        .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
                    let second = geometry
                        .world_frame(second)
                        .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
                    first.z_axis().dot(&second.z_axis())
                }
                SpatialSourceKind::PrismaticJoint { first, second, .. }
                | SpatialSourceKind::CylindricalJoint { first, second, .. }
                | SpatialSourceKind::AxisAlignmentMate { first, second, .. } => {
                    let first = geometry
                        .world_axis_frame(first)
                        .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
                    let second = geometry
                        .world_axis_frame(second)
                        .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
                    first.z_axis().dot(&second.z_axis())
                }
                SpatialSourceKind::PlanarJoint { first, second, .. } => {
                    let first = geometry
                        .world_plane_frame(first)
                        .ok_or(SpatialAssemblyError::UnknownPlaneFeature(first))?;
                    let second = geometry
                        .world_plane_frame(second)
                        .ok_or(SpatialAssemblyError::UnknownPlaneFeature(second))?;
                    first.z_axis().dot(&second.z_axis())
                }
                _ => return independent("axis-parity boundary source changed kind"),
            };
            (raw, parity.multiplier())
        }
        SpatialBranchBoundary::PrismaticClockRoot { source } => {
            let source_definition = assembly.require_source(source)?;
            let SpatialSourceKind::PrismaticJoint { first, second, .. } = source_definition.kind
            else {
                return independent("prismatic-clock boundary source changed kind");
            };
            let first = geometry
                .world_axis_frame(first)
                .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
            let second = geometry
                .world_axis_frame(second)
                .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
            (first.x_axis().dot(&second.x_axis()), 1.0)
        }
        SpatialBranchBoundary::HingeDriverPositiveRoot { source, coordinate } => {
            let source_definition = assembly.require_source(source)?;
            let SpatialSourceKind::HingePositionDriver {
                coordinate: source_coordinate,
                target,
            } = source_definition.kind
            else {
                return independent("hinge-driver boundary source changed kind");
            };
            if source_coordinate != coordinate {
                return independent("hinge-driver boundary coordinate changed");
            }
            let principal_phase = measured_hinge_principal_phase(assembly, geometry, coordinate)?;
            (
                canonical_phase(principal_phase - target.principal_phase)?.cos(),
                1.0,
            )
        }
        SpatialBranchBoundary::HingePrincipalCut {
            coordinate,
            winding,
        } => {
            let coordinate_definition = assembly.require_coordinate(coordinate)?;
            let SpatialCoordinateKind::Hinge {
                winding: coordinate_winding,
                ..
            } = coordinate_definition.kind
            else {
                return Err(SpatialAssemblyError::WrongCoordinateKind {
                    coordinate,
                    expected: "hinge",
                });
            };
            require_matching_winding(coordinate, coordinate_winding, winding)?;
            let principal_phase = measured_hinge_principal_phase(assembly, geometry, coordinate)?;
            let clearance = std::f64::consts::PI - principal_phase.abs();
            if !clearance.is_finite() {
                return independent("hinge principal-cut clearance is non-finite");
            }
            return Ok((principal_phase, clearance));
        }
        SpatialBranchBoundary::MonitorAxisParity { monitor, parity } => {
            let monitor_definition = assembly.require_mode_monitor(monitor)?;
            let SpatialModeMonitorKind::AxisParity { first, second, .. } = monitor_definition.kind
            else {
                return independent("axis-parity boundary monitor changed kind");
            };
            let first = geometry
                .world_axis_frame(first)
                .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
            let second = geometry
                .world_axis_frame(second)
                .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
            (first.z_axis().dot(&second.z_axis()), parity.multiplier())
        }
        SpatialBranchBoundary::MonitorPlaneSide { monitor, side } => {
            let monitor_definition = assembly.require_mode_monitor(monitor)?;
            let SpatialModeMonitorKind::PlaneSide { plane, point, .. } = monitor_definition.kind
            else {
                return independent("plane-side boundary monitor changed kind");
            };
            let plane = geometry
                .world_plane_frame(plane)
                .ok_or(SpatialAssemblyError::UnknownPlaneFeature(plane))?;
            let point = geometry
                .world_point(point)
                .ok_or(SpatialAssemblyError::UnknownPointFeature(point))?;
            (
                plane.z_axis().dot(&(point - plane.origin())),
                side.multiplier() / assembly.model_scale,
            )
        }
        SpatialBranchBoundary::MonitorSignedVolume {
            monitor,
            orientation,
        } => {
            let monitor_definition = assembly.require_mode_monitor(monitor)?;
            let SpatialModeMonitorKind::SignedVolume { points, .. } = monitor_definition.kind
            else {
                return independent("signed-volume boundary monitor changed kind");
            };
            require_distinct_volume_points(points)?;
            let [a, b, c, d] = points.map(|point| {
                geometry
                    .world_point(point)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(point))
            });
            let [a, b, c, d] = [a?, b?, c?, d?];
            let ab = finite_unit_edge(b - a, monitor, "B-A")?;
            let ac = finite_unit_edge(c - a, monitor, "C-A")?;
            let ad = finite_unit_edge(d - a, monitor, "D-A")?;
            (ab.cross(&ac).dot(&ad), orientation.multiplier())
        }
    };
    let clearance = raw_metric * multiplier;
    if !raw_metric.is_finite() || !clearance.is_finite() {
        return independent("spatial branch-boundary metric is non-finite");
    }
    Ok((raw_metric, clearance))
}

fn frame_axis(frame: Frame3, axis: SpatialFrameAxis) -> Vector3<f64> {
    match axis {
        SpatialFrameAxis::X => frame.x_axis(),
        SpatialFrameAxis::Y => frame.y_axis(),
        SpatialFrameAxis::Z => frame.z_axis(),
    }
}

fn measured_hinge_principal_phase(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
    coordinate: SpatialCoordinateId,
) -> Result<f64, SpatialAssemblyError> {
    let coordinate_definition = assembly.require_coordinate(coordinate)?;
    let resolved = resolve_hinge_coordinate_definition(assembly, coordinate_definition)?;
    let (first, second) = resolved_coordinate_world_frames(geometry, resolved)?;
    let sine = first.y_axis().dot(&second.x_axis());
    let cosine = first.x_axis().dot(&second.x_axis());
    let projection = sine.hypot(cosine);
    if !sine.is_finite() || !cosine.is_finite() || !projection.is_finite() || projection == 0.0 {
        return independent(format!(
            "hinge coordinate {coordinate} has an invalid boundary clock projection"
        ));
    }
    canonical_phase(sine.atan2(cosine))
}

fn initial_spatial_hysteresis(
    clearance: f64,
) -> Result<SpatialBoundaryHysteresisState, SpatialAssemblyError> {
    if !clearance.is_finite() {
        independent("spatial branch-boundary clearance is non-finite")
    } else if clearance >= SPATIAL_BOUNDARY_LEAVE_CLEARANCE {
        Ok(SpatialBoundaryHysteresisState::Clear)
    } else {
        Ok(SpatialBoundaryHysteresisState::Near)
    }
}

pub(crate) fn update_spatial_boundary_hysteresis(
    previous: &[SpatialBranchBoundaryEvaluation],
    current: &mut [SpatialBranchBoundaryEvaluation],
    observation: SpatialBoundaryObservation,
) -> Vec<SpatialBranchBoundaryEvent> {
    let mut events = Vec::new();
    for evaluation in current {
        let Some(retained) = previous
            .iter()
            .find(|candidate| candidate.boundary == evaluation.boundary)
        else {
            continue;
        };
        let transition = match retained.hysteresis_state {
            SpatialBoundaryHysteresisState::Clear
                if evaluation.clearance <= SPATIAL_BOUNDARY_ENTER_CLEARANCE =>
            {
                Some((
                    SpatialBoundaryTransition::Entered,
                    SpatialBoundaryHysteresisState::Near,
                ))
            }
            SpatialBoundaryHysteresisState::Near
                if evaluation.clearance >= SPATIAL_BOUNDARY_LEAVE_CLEARANCE =>
            {
                Some((
                    SpatialBoundaryTransition::Left,
                    SpatialBoundaryHysteresisState::Clear,
                ))
            }
            _ => None,
        };
        if let Some((transition, state)) = transition {
            evaluation.hysteresis_state = state;
            events.push(SpatialBranchBoundaryEvent {
                boundary: evaluation.boundary,
                transition,
                observation,
                previous_clearance: retained.clearance,
                clearance: evaluation.clearance,
                raw_metric: evaluation.raw_metric,
            });
        } else {
            evaluation.hysteresis_state = retained.hysteresis_state;
        }
    }
    events
}

fn hinge_monitor_features(
    assembly: &SpatialAssembly,
    coordinate: &SpatialCoordinate,
) -> Result<Vec<SpatialModeFeature>, SpatialAssemblyError> {
    let SpatialCoordinateKind::Hinge { parent, .. } = coordinate.kind else {
        return Err(SpatialAssemblyError::WrongCoordinateKind {
            coordinate: coordinate.id,
            expected: "hinge",
        });
    };
    match assembly.require_source(parent)?.kind {
        SpatialSourceKind::RevoluteJoint { first, second, .. } => Ok(vec![
            SpatialModeFeature::Frame(first),
            SpatialModeFeature::Frame(second),
        ]),
        SpatialSourceKind::CylindricalJoint { first, second, .. } => Ok(vec![
            SpatialModeFeature::Axis(first),
            SpatialModeFeature::Axis(second),
        ]),
        SpatialSourceKind::PlanarJoint { first, second, .. } => Ok(vec![
            SpatialModeFeature::Plane(first),
            SpatialModeFeature::Plane(second),
        ]),
        _ => Err(SpatialAssemblyError::WrongCoordinateParent {
            source_id: parent,
            expected: "hinge",
        }),
    }
}

fn finite_unit_edge(
    edge: Vector3<f64>,
    monitor: SpatialModeMonitorId,
    label: &str,
) -> Result<Vector3<f64>, SpatialAssemblyError> {
    let norm = robust_norm(edge);
    if !edge.iter().all(|value| value.is_finite()) || !norm.is_finite() {
        return independent(format!(
            "signed-volume monitor {monitor} has non-finite {label} edge geometry"
        ));
    }
    if norm == 0.0 {
        return independent(format!(
            "signed-volume monitor {monitor} has collapsed {label} edge geometry"
        ));
    }
    let unit = edge / norm;
    if unit.iter().all(|value| value.is_finite()) {
        Ok(unit)
    } else {
        independent(format!(
            "signed-volume monitor {monitor} could not normalize {label} edge geometry"
        ))
    }
}

fn require_retained_mode_metric(
    monitor: SpatialModeMonitorId,
    metric: f64,
    context: &str,
) -> Result<(), SpatialAssemblyError> {
    if metric.is_finite() && metric > ORIENTATION_BRANCH_MARGIN {
        Ok(())
    } else {
        independent(format!(
            "{context} monitor {monitor} retained metric {metric} is non-finite or does not exceed branch margin {ORIENTATION_BRANCH_MARGIN:e}"
        ))
    }
}

fn resolved_coordinate_world_frames(
    geometry: &SpatialGeometry,
    resolved: ResolvedSpatialCoordinateDefinition,
) -> Result<(Frame3, Frame3), SpatialAssemblyError> {
    let first_pose = geometry
        .body_pose(resolved.first_body)
        .ok_or(SpatialAssemblyError::UnknownBody(resolved.first_body))?;
    let second_pose = geometry
        .body_pose(resolved.second_body)
        .ok_or(SpatialAssemblyError::UnknownBody(resolved.second_body))?;
    Ok((
        transform_frame(first_pose, resolved.first_local)?,
        transform_frame(second_pose, resolved.second_local)?,
    ))
}

fn validate_physical_candidate(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
    coordinate_values: &[SpatialCoordinateValue],
    session: &SolveSession,
    mappings: &[SpatialSourceMapping],
    config: SolverConfig,
) -> Result<(f64, Vec<SpatialModeEvaluation>), SpatialAssemblyError> {
    let tolerance = spatial_acceptance_tolerance(config);
    validate_core_acceptance(session.report(), tolerance)?;
    let core_max = physical_audit_max(session, mappings, tolerance)?;
    validate_transformed_features(assembly, geometry)?;
    let domain_max = physical_domain_residual_max(assembly, geometry, coordinate_values)?;
    let maximum = core_max.max(domain_max);
    if !maximum.is_finite() {
        return independent("combined physical residual maximum is non-finite");
    }
    if maximum > tolerance {
        return independent(format!(
            "physical residual {maximum:e} exceeds {tolerance:e}"
        ));
    }
    let mode_evaluations = evaluate_mode_monitors(assembly, geometry, coordinate_values)?;
    Ok((maximum, mode_evaluations))
}

fn validate_transformed_features(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
) -> Result<(), SpatialAssemblyError> {
    if geometry.bodies.len() != assembly.bodies.len()
        || geometry.points.len() != assembly.point_features.len()
        || geometry.frames.len() != assembly.frame_features.len()
        || geometry.axes.len() != assembly.axis_features.len()
        || geometry.planes.len() != assembly.plane_features.len()
    {
        return independent("transformed spatial geometry does not cover every stored feature");
    }
    for (definition, solved) in assembly.bodies.iter().zip(&geometry.bodies) {
        if solved.body_id != definition.id {
            return independent("transformed spatial body order or identity changed");
        }
        validate_pose(solved.pose)?;
    }
    for (definition, transformed) in assembly.point_features.iter().zip(&geometry.points) {
        if transformed.feature_id != definition.id || transformed.body_id != definition.body {
            return independent("transformed spatial point order, identity, or body changed");
        }
        validate_point(transformed.world, "geometry.point.world")?;
        let pose = geometry
            .body_pose(definition.body)
            .ok_or(SpatialAssemblyError::UnknownBody(definition.body))?;
        let expected = pose.try_transform_point(definition.local_point)?;
        if transformed.world != expected {
            return independent(format!(
                "transformed spatial point feature {} does not match its body-local definition",
                definition.id
            ));
        }
    }
    for (definition, transformed) in assembly.frame_features.iter().zip(&geometry.frames) {
        if transformed.feature_id != definition.id || transformed.body_id != definition.body {
            return independent("transformed spatial frame order, identity, or body changed");
        }
        revalidate_frame(transformed.world)?;
        let pose = geometry
            .body_pose(definition.body)
            .ok_or(SpatialAssemblyError::UnknownBody(definition.body))?;
        if transformed.world != transform_frame(pose, definition.local_frame)? {
            return independent(format!(
                "transformed spatial frame feature {} does not match its body-local definition",
                definition.id
            ));
        }
    }
    for (definition, transformed) in assembly.axis_features.iter().zip(&geometry.axes) {
        if transformed.feature_id != definition.id || transformed.body_id != definition.body {
            return independent("transformed spatial axis order, identity, or body changed");
        }
        revalidate_frame(transformed.world)?;
        let pose = geometry
            .body_pose(definition.body)
            .ok_or(SpatialAssemblyError::UnknownBody(definition.body))?;
        if transformed.world != transform_frame(pose, definition.local_frame)? {
            return independent(format!(
                "transformed spatial axis feature {} does not match its body-local definition",
                definition.id
            ));
        }
    }
    for (definition, transformed) in assembly.plane_features.iter().zip(&geometry.planes) {
        if transformed.feature_id != definition.id || transformed.body_id != definition.body {
            return independent("transformed spatial plane order, identity, or body changed");
        }
        revalidate_frame(transformed.world)?;
        let pose = geometry
            .body_pose(definition.body)
            .ok_or(SpatialAssemblyError::UnknownBody(definition.body))?;
        if transformed.world != transform_frame(pose, definition.local_frame)? {
            return independent(format!(
                "transformed spatial plane feature {} does not match its body-local definition",
                definition.id
            ));
        }
    }
    Ok(())
}

fn validate_core_acceptance(
    report: &SolveReport,
    tolerance: f64,
) -> Result<(), SpatialAssemblyError> {
    if report.hard_validity != HardValidity::Valid {
        return independent(format!("core hard validity is {:?}", report.hard_validity));
    }
    if !report.hard_residuals_validated {
        return independent("core hard rows were not independently validated");
    }
    if !report.rank_is_valid {
        return independent("core numerical rank is invalid");
    }
    if !report.hard_residual_max.is_finite() || report.hard_residual_max > tolerance {
        return independent(format!(
            "core hard maximum {} is non-finite or exceeds {:e}",
            report.hard_residual_max, tolerance
        ));
    }
    for component in &report.component_solves {
        if component.hard_validity != HardValidity::Valid
            || !component.hard_residuals_validated
            || !component.rank_is_valid
            || !component.hard_residual_max.is_finite()
        {
            return independent(format!(
                "core component {} lacks finite hard/rank validity",
                component.component_index
            ));
        }
    }
    Ok(())
}

fn physical_audit_max(
    session: &SolveSession,
    mappings: &[SpatialSourceMapping],
    tolerance: f64,
) -> Result<f64, SpatialAssemblyError> {
    let mut maximum = 0.0_f64;
    for mapping in mappings {
        let source = session
            .report()
            .audit
            .sources
            .iter()
            .find(|source| source.source_id == mapping.core_source_id)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(format!(
                    "physical source {} is absent from accepted audit",
                    mapping.source
                ))
            })?;
        if source.source_label != mapping.source_label {
            return independent(format!(
                "physical source {} audit label does not match its mapping",
                mapping.source
            ));
        }
        let expected_rows = mapping.residual_ids.iter().try_fold(0_usize, |sum, id| {
            let rows = session
                .problem()
                .residual(*id)
                .ok_or(CoreError::UnknownResidual(*id))?
                .output_dimension();
            sum.checked_add(rows).ok_or(CoreError::DimensionOverflow {
                context: "spatial physical audit rows",
            })
        })?;
        if source.rows.len() != expected_rows {
            return independent(format!(
                "physical source {} has {} audit rows, expected {expected_rows}",
                mapping.source,
                source.rows.len()
            ));
        }
        for row in &source.rows {
            if !mapping.residual_ids.contains(&row.residual_id) {
                return independent(format!(
                    "physical source {} contains an unmapped residual",
                    mapping.source
                ));
            }
            if row.category != ResidualCategory::Hard
                || row.evaluation_status != AuditEvaluationStatus::Evaluated
            {
                return independent(format!(
                    "physical source {} contains a non-evaluated hard row",
                    mapping.source
                ));
            }
            if !row.raw_residual.is_finite()
                || !row.normalized_residual.is_finite()
                || !row.scale.is_finite()
                || row.scale <= 0.0
            {
                return independent(format!(
                    "physical source {} contains non-finite audit data",
                    mapping.source
                ));
            }
            maximum = maximum.max(row.normalized_residual.abs());
        }
    }
    if maximum > tolerance {
        return independent(format!(
            "physical core audit maximum {maximum:e} exceeds {tolerance:e}"
        ));
    }
    Ok(maximum)
}

#[allow(clippy::too_many_lines)]
fn physical_domain_residual_max(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
    coordinate_values: &[SpatialCoordinateValue],
) -> Result<f64, SpatialAssemblyError> {
    let mut maximum = 0.0_f64;
    for source in &assembly.sources {
        match source.kind {
            SpatialSourceKind::PhysicalGround { body, target_pose } => {
                let pose = geometry
                    .body_pose(body)
                    .ok_or(SpatialAssemblyError::UnknownBody(body))?;
                let difference = target_pose.local_difference(&pose)?;
                include_normalized(
                    &mut maximum,
                    &[
                        difference[0] / assembly.model_scale,
                        difference[1] / assembly.model_scale,
                        difference[2] / assembly.model_scale,
                        difference[3],
                        difference[4],
                        difference[5],
                    ],
                    "physical ground",
                )?;
            }
            SpatialSourceKind::BallJoint { first, second } => {
                let first = geometry
                    .world_point(first)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(first))?;
                let second = geometry
                    .world_point(second)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(second))?;
                let difference = second - first;
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                    ],
                    "ball joint",
                )?;
            }
            SpatialSourceKind::PointDistanceMate {
                first,
                second,
                distance,
            } => {
                let first = geometry
                    .world_point(first)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(first))?;
                let second = geometry
                    .world_point(second)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(second))?;
                let measured = regular_distance(second - first, "point distance mate")?;
                include_normalized(
                    &mut maximum,
                    &[(measured - distance) / assembly.model_scale],
                    "point distance mate",
                )?;
            }
            SpatialSourceKind::FixedFrame { first, second } => {
                let first = geometry
                    .world_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
                let second = geometry
                    .world_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
                let difference = second.origin() - first.origin();
                let diagonal = [
                    first.x_axis().dot(&second.x_axis()),
                    first.y_axis().dot(&second.y_axis()),
                    first.z_axis().dot(&second.z_axis()),
                ];
                if !diagonal.iter().all(|value| value.is_finite())
                    || diagonal
                        .iter()
                        .any(|value| *value <= ORIENTATION_BRANCH_MARGIN)
                {
                    return independent(format!(
                        "fixed-frame source {} reached a false half-turn orientation root",
                        source.id
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                        first.y_axis().dot(&second.x_axis()),
                        first.z_axis().dot(&second.x_axis()),
                        first.z_axis().dot(&second.y_axis()),
                    ],
                    "fixed frame",
                )?;
            }
            SpatialSourceKind::FrameOffsetMate {
                first,
                second,
                offset,
            } => {
                let first = geometry
                    .world_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
                let second = geometry
                    .world_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
                let expected = compose_frames(first, offset)?;
                let difference = second.origin() - expected.origin();
                let diagonal = [
                    expected.x_axis().dot(&second.x_axis()),
                    expected.y_axis().dot(&second.y_axis()),
                    expected.z_axis().dot(&second.z_axis()),
                ];
                if !diagonal.iter().all(|value| value.is_finite())
                    || diagonal
                        .iter()
                        .any(|value| *value <= ORIENTATION_BRANCH_MARGIN)
                {
                    return independent(format!(
                        "frame-offset source {} reached a false half-turn relative to its target",
                        source.id
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                        expected.y_axis().dot(&second.x_axis()),
                        expected.z_axis().dot(&second.x_axis()),
                        expected.z_axis().dot(&second.y_axis()),
                    ],
                    "frame offset mate",
                )?;
            }
            SpatialSourceKind::RevoluteJoint {
                first,
                second,
                parity,
            } => {
                let first = geometry
                    .world_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
                let second = geometry
                    .world_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
                let difference = second.origin() - first.origin();
                let second_axis = second.z_axis() * parity.multiplier();
                let parity_dot = first.z_axis().dot(&second_axis);
                if !parity_dot.is_finite() || parity_dot <= ORIENTATION_BRANCH_MARGIN {
                    return independent(format!(
                        "revolute source {} violated {:?} axis parity",
                        source.id, parity
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                        first.x_axis().dot(&second_axis),
                        first.y_axis().dot(&second_axis),
                    ],
                    "revolute joint",
                )?;
            }
            SpatialSourceKind::PrismaticJoint {
                first,
                second,
                parity,
            } => {
                let first = geometry
                    .world_axis_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
                let second = geometry
                    .world_axis_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
                let difference = second.origin() - first.origin();
                let second_axis = require_parity_branch(
                    source.id,
                    "prismatic axis",
                    parity,
                    first.z_axis(),
                    second.z_axis(),
                )?;
                let clock_dot = first.x_axis().dot(&second.x_axis());
                if !clock_dot.is_finite() || clock_dot <= ORIENTATION_BRANCH_MARGIN {
                    return independent(format!(
                        "prismatic source {} violated its positive clock branch",
                        source.id
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[
                        first.x_axis().dot(&difference) / assembly.model_scale,
                        first.y_axis().dot(&difference) / assembly.model_scale,
                        first.x_axis().dot(&second_axis),
                        first.y_axis().dot(&second_axis),
                        first.y_axis().dot(&second.x_axis()),
                    ],
                    "prismatic joint",
                )?;
            }
            SpatialSourceKind::CylindricalJoint {
                first,
                second,
                parity,
            } => {
                let first = geometry
                    .world_axis_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
                let second = geometry
                    .world_axis_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
                let difference = second.origin() - first.origin();
                let second_axis = require_parity_branch(
                    source.id,
                    "cylindrical axis",
                    parity,
                    first.z_axis(),
                    second.z_axis(),
                )?;
                include_normalized(
                    &mut maximum,
                    &[
                        first.x_axis().dot(&difference) / assembly.model_scale,
                        first.y_axis().dot(&difference) / assembly.model_scale,
                        first.x_axis().dot(&second_axis),
                        first.y_axis().dot(&second_axis),
                    ],
                    "cylindrical joint",
                )?;
            }
            SpatialSourceKind::PlanarJoint {
                first,
                second,
                parity,
            } => {
                let first = geometry
                    .world_plane_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownPlaneFeature(first))?;
                let second = geometry
                    .world_plane_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownPlaneFeature(second))?;
                let difference = second.origin() - first.origin();
                let second_normal = require_parity_branch(
                    source.id,
                    "planar normal",
                    parity,
                    first.z_axis(),
                    second.z_axis(),
                )?;
                include_normalized(
                    &mut maximum,
                    &[
                        first.z_axis().dot(&difference) / assembly.model_scale,
                        first.x_axis().dot(&second_normal),
                        first.y_axis().dot(&second_normal),
                    ],
                    "planar joint",
                )?;
            }
            SpatialSourceKind::UniversalJoint { first, second } => {
                let first = geometry
                    .world_axis_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
                let second = geometry
                    .world_axis_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
                let difference = second.origin() - first.origin();
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                        first.z_axis().dot(&second.z_axis()),
                    ],
                    "universal joint",
                )?;
            }
            SpatialSourceKind::AxisAngleMate {
                first,
                second,
                angle,
            } => {
                let first = geometry
                    .world_axis_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
                let second = geometry
                    .world_axis_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
                let first_axis = first.z_axis();
                let second_axis = second.z_axis();
                let cosine = first_axis.dot(&second_axis);
                let sine = regular_distance(
                    first_axis.cross(&second_axis),
                    "axis angle mate principal-angle sine",
                )?;
                if !cosine.is_finite() {
                    return independent("axis angle mate principal-angle cosine is non-finite");
                }
                let principal = sine.atan2(cosine);
                if !principal.is_finite() || principal <= 0.0 || principal >= std::f64::consts::PI {
                    return independent(format!(
                        "axis-angle source {} reached a singular principal-angle endpoint",
                        source.id
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[cosine - angle.cos(), principal - angle],
                    "axis angle mate",
                )?;
            }
            SpatialSourceKind::AxisAlignmentMate {
                first,
                second,
                parity,
            } => {
                let first = geometry
                    .world_axis_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(first))?;
                let second = geometry
                    .world_axis_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownAxisFeature(second))?;
                let adjusted = require_parity_branch(
                    source.id,
                    "axis-alignment mate",
                    parity,
                    first.z_axis(),
                    second.z_axis(),
                )?;
                include_normalized(
                    &mut maximum,
                    &[first.x_axis().dot(&adjusted), first.y_axis().dot(&adjusted)],
                    "axis alignment mate",
                )?;
            }
            SpatialSourceKind::HingePositionDriver { coordinate, target } => {
                validate_hinge_target(target)?;
                let definition = assembly.require_coordinate(coordinate)?;
                let SpatialCoordinateKind::Hinge { winding, .. } = definition.kind else {
                    return Err(SpatialAssemblyError::WrongCoordinateKind {
                        coordinate,
                        expected: "hinge",
                    });
                };
                require_matching_winding(coordinate, winding, target.winding)?;
                let value = coordinate_values
                    .iter()
                    .find(|value| value.coordinate == coordinate)
                    .ok_or(SpatialAssemblyError::UnknownCoordinate(coordinate))?;
                let SpatialCoordinateValueKind::Hinge(value) = value.value else {
                    return independent(format!(
                        "accepted value for hinge coordinate {coordinate} has the wrong kind"
                    ));
                };
                require_matching_winding(coordinate, value.winding, target.winding)?;
                let resolved = resolve_hinge_coordinate_definition(assembly, definition)?;
                let (first, second) = resolved_coordinate_world_frames(geometry, resolved)?;
                let sine = first.y_axis().dot(&second.x_axis());
                let cosine = first.x_axis().dot(&second.x_axis());
                let (target_sine, target_cosine) = target.principal_phase.sin_cos();
                let smooth_error = sine * target_cosine - cosine * target_sine;
                let wrapped_error =
                    canonical_phase(value.principal_phase - target.principal_phase)?;
                let target_cosine_root = wrapped_error.cos();
                if !target_cosine_root.is_finite()
                    || target_cosine_root <= ORIENTATION_BRANCH_MARGIN
                {
                    return independent(format!(
                        "hinge driver source {} did not retain the positive target cosine root",
                        source.id
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[smooth_error, wrapped_error],
                    "hinge position driver",
                )?;
            }
            SpatialSourceKind::TranslationPositionDriver { coordinate, target } => {
                validate_translation_target(target)?;
                let definition = assembly.require_coordinate(coordinate)?;
                if !matches!(
                    definition.kind,
                    SpatialCoordinateKind::AxialTranslation { .. }
                        | SpatialCoordinateKind::PlanarTranslation { .. }
                ) {
                    return Err(SpatialAssemblyError::WrongCoordinateKind {
                        coordinate,
                        expected: "translation",
                    });
                }
                let value = coordinate_values
                    .iter()
                    .find(|value| value.coordinate == coordinate)
                    .ok_or(SpatialAssemblyError::UnknownCoordinate(coordinate))?;
                let measured = match (definition.kind, value.value) {
                    (
                        SpatialCoordinateKind::AxialTranslation { .. },
                        SpatialCoordinateValueKind::AxialTranslation(value),
                    ) => value,
                    (
                        SpatialCoordinateKind::PlanarTranslation {
                            axis: expected_axis,
                            ..
                        },
                        SpatialCoordinateValueKind::PlanarTranslation { axis, value },
                    ) if axis == expected_axis => value,
                    _ => {
                        return independent(format!(
                            "accepted value for translation coordinate {coordinate} has the wrong kind"
                        ));
                    }
                };
                include_normalized(
                    &mut maximum,
                    &[(measured - target) / assembly.model_scale],
                    "translation position driver",
                )?;
            }
        }
    }
    Ok(maximum)
}

fn project_geometry(
    assembly: &mut SpatialAssembly,
    geometry: &SpatialGeometry,
) -> Result<(), SpatialAssemblyError> {
    for solved in &geometry.bodies {
        validate_pose(solved.pose)?;
        assembly
            .bodies
            .iter_mut()
            .find(|body| body.id == solved.body_id)
            .ok_or(SpatialAssemblyError::UnknownBody(solved.body_id))?
            .pose_guess = solved.pose;
    }
    Ok(())
}

fn ground_audit_rows(body: SpatialBodyId, target: Pose3) -> Vec<ResidualRowAudit> {
    let coordinates = ["vx", "vy", "vz", "wx", "wy", "wz"];
    coordinates
        .iter()
        .enumerate()
        .map(|(index, coordinate)| {
            ResidualRowAudit::new(
                format!("physical ground target local difference {coordinate}"),
                vec![
                    AuditBinding::new("body", body.to_string()),
                    AuditBinding::new("target_pose", format!("{:?}", target.ambient())),
                ],
                if index < 3 { "model-unit" } else { "rad" },
            )
        })
        .collect()
}

fn private_gauge_audit_rows(body: SpatialBodyId) -> Vec<ResidualRowAudit> {
    let coordinates = ["vx", "vy", "vz", "wx", "wy", "wz"];
    coordinates
        .iter()
        .enumerate()
        .map(|(index, coordinate)| {
            ResidualRowAudit::new(
                format!("private spatial numerical gauge local {coordinate}"),
                vec![AuditBinding::new("body", body.to_string())],
                if index < 3 { "model-unit" } else { "rad" },
            )
        })
        .collect()
}

fn point_joint_audit_rows(
    joint: &str,
    first: &SpatialPointFeature,
    second: &SpatialPointFeature,
) -> Vec<ResidualRowAudit> {
    ["x", "y", "z"]
        .into_iter()
        .map(|coordinate| {
            ResidualRowAudit::new(
                format!("{joint} second world point {coordinate} - first world point {coordinate}"),
                point_bindings(first, second),
                "model-unit",
            )
        })
        .collect()
}

fn point_bindings(first: &SpatialPointFeature, second: &SpatialPointFeature) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("first_body", first.body.to_string()),
        AuditBinding::new("first_point_feature", first.id.to_string()),
        AuditBinding::new("second_body", second.body.to_string()),
        AuditBinding::new("second_point_feature", second.id.to_string()),
    ]
}

fn point_distance_audit_row(
    first: &SpatialPointFeature,
    second: &SpatialPointFeature,
    distance: f64,
) -> ResidualRowAudit {
    let mut bindings = point_bindings(first, second);
    bindings.push(AuditBinding::new("target_distance", distance.to_string()));
    ResidualRowAudit::new(
        "point distance mate norm(second world point - first world point) - target distance",
        bindings,
        "model-unit",
    )
}

fn frame_joint_audit_rows(
    joint: &str,
    first: &SpatialFrameFeature,
    second: &SpatialFrameFeature,
    parity: Option<SpatialAxisParity>,
    templates: &[&str],
) -> Vec<ResidualRowAudit> {
    templates
        .iter()
        .enumerate()
        .map(|(index, template)| {
            ResidualRowAudit::new(
                format!("{joint} {template}"),
                frame_bindings(first, second, parity),
                if index < 3 {
                    "model-unit"
                } else {
                    "dimensionless"
                },
            )
        })
        .collect()
}

fn frame_bindings(
    first: &SpatialFrameFeature,
    second: &SpatialFrameFeature,
    parity: Option<SpatialAxisParity>,
) -> Vec<AuditBinding> {
    let mut bindings = vec![
        AuditBinding::new("first_body", first.body.to_string()),
        AuditBinding::new("first_frame_feature", first.id.to_string()),
        AuditBinding::new("second_body", second.body.to_string()),
        AuditBinding::new("second_frame_feature", second.id.to_string()),
    ];
    if let Some(parity) = parity {
        bindings.push(AuditBinding::new("axis_parity", format!("{parity:?}")));
    }
    bindings
}

fn frame_offset_audit_rows(
    first: &SpatialFrameFeature,
    second: &SpatialFrameFeature,
    offset: Frame3,
) -> Vec<ResidualRowAudit> {
    let templates = [
        "frame offset mate second origin x - expected origin x",
        "frame offset mate second origin y - expected origin y",
        "frame offset mate second origin z - expected origin z",
        "frame offset mate expected y dot second x",
        "frame offset mate expected z dot second x",
        "frame offset mate expected z dot second y",
    ];
    templates
        .iter()
        .enumerate()
        .map(|(index, template)| {
            let mut bindings = frame_bindings(first, second, None);
            bindings.push(AuditBinding::new(
                "offset_in_first_frame",
                format!("{offset:?}"),
            ));
            ResidualRowAudit::new(
                *template,
                bindings,
                if index < 3 {
                    "model-unit"
                } else {
                    "dimensionless"
                },
            )
        })
        .collect()
}

fn axis_joint_audit_rows(
    joint: &str,
    first: &SpatialAxisFeature,
    second: &SpatialAxisFeature,
    parity: Option<SpatialAxisParity>,
    rows: &[(&str, &str)],
) -> Vec<ResidualRowAudit> {
    rows.iter()
        .map(|(template, unit)| {
            ResidualRowAudit::new(
                format!("{joint} {template}"),
                axis_bindings(first, second, parity),
                *unit,
            )
        })
        .collect()
}

fn axis_bindings(
    first: &SpatialAxisFeature,
    second: &SpatialAxisFeature,
    parity: Option<SpatialAxisParity>,
) -> Vec<AuditBinding> {
    let mut bindings = vec![
        AuditBinding::new("first_body", first.body.to_string()),
        AuditBinding::new("first_axis_feature", first.id.to_string()),
        AuditBinding::new("second_body", second.body.to_string()),
        AuditBinding::new("second_axis_feature", second.id.to_string()),
    ];
    if let Some(parity) = parity {
        bindings.push(AuditBinding::new("axis_parity", format!("{parity:?}")));
    }
    bindings
}

fn axis_angle_audit_row(
    first: &SpatialAxisFeature,
    second: &SpatialAxisFeature,
    angle: f64,
) -> ResidualRowAudit {
    let mut bindings = axis_bindings(first, second, None);
    bindings.push(AuditBinding::new("target_angle", angle.to_string()));
    ResidualRowAudit::new(
        "axis angle mate first z dot second z - cos(target angle)",
        bindings,
        "dimensionless",
    )
}

fn coordinate_driver_bindings(
    coordinate: &SpatialCoordinate,
    resolved: ResolvedSpatialCoordinateDefinition,
) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("coordinate", coordinate.id.to_string()),
        AuditBinding::new("coordinate_label", coordinate.label.clone()),
        AuditBinding::new("parent_source", resolved.parent_source.to_string()),
        AuditBinding::new("first_body", resolved.first_body.to_string()),
        AuditBinding::new("second_body", resolved.second_body.to_string()),
        AuditBinding::new("axis_parity", format!("{:?}", resolved.parity)),
    ]
}

fn hinge_driver_audit_row(
    coordinate: &SpatialCoordinate,
    resolved: ResolvedSpatialCoordinateDefinition,
    target: SpatialHingeTarget,
) -> ResidualRowAudit {
    let mut bindings = coordinate_driver_bindings(coordinate, resolved);
    bindings.push(AuditBinding::new(
        "target_principal_phase_rad",
        target.principal_phase.to_string(),
    ));
    bindings.push(AuditBinding::new(
        "target_winding",
        target.winding.to_string(),
    ));
    ResidualRowAudit::new(
        "hinge position driver cos(target) * (y1 dot x2) - sin(target) * (x1 dot x2)",
        bindings,
        "dimensionless",
    )
}

fn parameterized_hinge_driver_audit_row(
    coordinate: &SpatialCoordinate,
    resolved: ResolvedSpatialCoordinateDefinition,
    winding: i64,
) -> ResidualRowAudit {
    let mut bindings = coordinate_driver_bindings(coordinate, resolved);
    bindings.push(AuditBinding::new(
        "active_parameter",
        "ephemeral spatial continuation scalar",
    ));
    bindings.push(AuditBinding::new("retained_winding", winding.to_string()));
    ResidualRowAudit::new(
        "hinge position driver cos(active continuation parameter) * (y1 dot x2) - sin(active continuation parameter) * (x1 dot x2)",
        bindings,
        "dimensionless",
    )
}

fn translation_driver_audit_row(
    coordinate: &SpatialCoordinate,
    resolved: ResolvedSpatialCoordinateDefinition,
    target: f64,
) -> ResidualRowAudit {
    let mut bindings = coordinate_driver_bindings(coordinate, resolved);
    bindings.push(AuditBinding::new("target_translation", target.to_string()));
    let template = match coordinate.kind {
        SpatialCoordinateKind::AxialTranslation { .. } => {
            "translation position driver first z dot (second origin - first origin) - target"
        }
        SpatialCoordinateKind::PlanarTranslation {
            axis: SpatialPlanarTranslationAxis::X,
            ..
        } => "translation position driver first x dot (second origin - first origin) - target",
        SpatialCoordinateKind::PlanarTranslation {
            axis: SpatialPlanarTranslationAxis::Y,
            ..
        } => "translation position driver first y dot (second origin - first origin) - target",
        SpatialCoordinateKind::Hinge { .. } => unreachable!("validated translation coordinate"),
    };
    ResidualRowAudit::new(template, bindings, "model-unit")
}

fn parameterized_translation_driver_audit_row(
    coordinate: &SpatialCoordinate,
    resolved: ResolvedSpatialCoordinateDefinition,
) -> ResidualRowAudit {
    let mut bindings = coordinate_driver_bindings(coordinate, resolved);
    bindings.push(AuditBinding::new(
        "active_parameter",
        "ephemeral spatial continuation scalar",
    ));
    let template = match coordinate.kind {
        SpatialCoordinateKind::AxialTranslation { .. } => {
            "translation position driver first z dot (second origin - first origin) - active continuation parameter"
        }
        SpatialCoordinateKind::PlanarTranslation {
            axis: SpatialPlanarTranslationAxis::X,
            ..
        } => {
            "translation position driver first x dot (second origin - first origin) - active continuation parameter"
        }
        SpatialCoordinateKind::PlanarTranslation {
            axis: SpatialPlanarTranslationAxis::Y,
            ..
        } => {
            "translation position driver first y dot (second origin - first origin) - active continuation parameter"
        }
        SpatialCoordinateKind::Hinge { .. } => unreachable!("validated translation coordinate"),
    };
    ResidualRowAudit::new(template, bindings, "model-unit")
}

fn plane_joint_audit_rows(
    joint: &str,
    first: &SpatialPlaneFeature,
    second: &SpatialPlaneFeature,
    parity: SpatialAxisParity,
    rows: &[(&str, &str)],
) -> Vec<ResidualRowAudit> {
    rows.iter()
        .map(|(template, unit)| {
            ResidualRowAudit::new(
                format!("{joint} {template}"),
                vec![
                    AuditBinding::new("first_body", first.body.to_string()),
                    AuditBinding::new("first_plane_feature", first.id.to_string()),
                    AuditBinding::new("second_body", second.body.to_string()),
                    AuditBinding::new("second_plane_feature", second.id.to_string()),
                    AuditBinding::new("normal_parity", format!("{parity:?}")),
                ],
                *unit,
            )
        })
        .collect()
}

fn transform_frame(pose: Pose3, local: Frame3) -> Result<Frame3, SpatialAssemblyError> {
    let local = revalidate_frame(local)?;
    Ok(Frame3::try_new(
        pose.try_transform_point(local.origin())?,
        pose.try_transform_vector(local.x_axis())?,
        pose.try_transform_vector(local.y_axis())?,
        pose.try_transform_vector(local.z_axis())?,
    )?)
}

fn compose_frames(parent: Frame3, child: Frame3) -> Result<Frame3, SpatialAssemblyError> {
    let parent = revalidate_frame(parent)?;
    let child = revalidate_frame(child)?;
    Ok(Frame3::try_new(
        parent.transform_point(child.origin())?,
        parent.transform_vector(child.x_axis())?,
        parent.transform_vector(child.y_axis())?,
        parent.transform_vector(child.z_axis())?,
    )?)
}

fn revalidate_frame(frame: Frame3) -> Result<Frame3, SpatialAssemblyError> {
    Ok(Frame3::try_new(
        frame.origin(),
        frame.x_axis(),
        frame.y_axis(),
        frame.z_axis(),
    )?)
}

fn validate_pose(pose: Pose3) -> Result<(), SpatialAssemblyError> {
    Pose3::from_ambient(pose.ambient())?;
    Ok(())
}

fn validate_point(point: Point3<f64>, field: &'static str) -> Result<(), SpatialAssemblyError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        invalid_field(field, "point coordinates must be finite")
    }
}

fn validate_model_scale(model_scale: f64) -> Result<(), SpatialAssemblyError> {
    if model_scale.is_finite() && model_scale > 0.0 {
        Ok(())
    } else {
        Err(SpatialAssemblyError::InvalidModelScale { value: model_scale })
    }
}

fn validate_positive_distance(distance: f64) -> Result<(), SpatialAssemblyError> {
    if distance.is_finite() && distance > 0.0 {
        Ok(())
    } else {
        invalid_field(
            "point_distance_mate.distance",
            "distance must be strictly positive and finite; use a ball joint for coincidence",
        )
    }
}

fn validate_interior_angle(angle: f64) -> Result<(), SpatialAssemblyError> {
    if angle.is_finite() && angle > 0.0 && angle < std::f64::consts::PI {
        Ok(())
    } else {
        invalid_field(
            "axis_angle_mate.angle",
            "angle must be finite and strictly inside (0, PI); use explicit-parity alignment at an endpoint",
        )
    }
}

fn validate_hinge_target(target: SpatialHingeTarget) -> Result<(), SpatialAssemblyError> {
    if target.principal_phase.is_finite()
        && (-std::f64::consts::PI..std::f64::consts::PI).contains(&target.principal_phase)
    {
        Ok(())
    } else {
        invalid_field(
            "hinge_position_driver.target",
            "principal phase must be finite and canonical in [-PI, PI)",
        )
    }
}

fn validate_translation_target(target: f64) -> Result<(), SpatialAssemblyError> {
    if target.is_finite() {
        Ok(())
    } else {
        invalid_field(
            "translation_position_driver.target",
            "target must be finite",
        )
    }
}

fn canonical_phase(phase: f64) -> Result<f64, SpatialAssemblyError> {
    if !phase.is_finite() {
        return independent("spatial principal phase is non-finite");
    }
    let principal =
        (phase + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    if principal.is_finite() && (-std::f64::consts::PI..std::f64::consts::PI).contains(&principal) {
        Ok(principal)
    } else {
        independent("spatial principal phase could not be canonicalized")
    }
}

fn validate_point_distance_candidate(
    assembly: &SpatialAssembly,
    first: &SpatialPointFeature,
    second: &SpatialPointFeature,
) -> Result<(), SpatialAssemblyError> {
    let first_pose = assembly.require_body(first.body)?.pose_guess;
    let second_pose = assembly.require_body(second.body)?.pose_guess;
    let first_world = first_pose.try_transform_point(first.local_point)?;
    let second_world = second_pose.try_transform_point(second.local_point)?;
    let displacement = second_world - first_world;
    let distance = robust_norm(displacement);
    if !displacement.iter().all(|value| value.is_finite()) || !distance.is_finite() {
        invalid_field(
            "point_distance_mate.candidate",
            "candidate point separation must be finite",
        )
    } else if distance == 0.0 {
        invalid_field(
            "point_distance_mate.candidate",
            "candidate points must be noncoincident for a regular distance derivative",
        )
    } else {
        Ok(())
    }
}

fn spatial_acceptance_tolerance(config: SolverConfig) -> f64 {
    config
        .normalized_residual_tolerance
        .min(SPATIAL_ACCEPTANCE_TOLERANCE)
}

fn validate_label(label: &str, field: &'static str) -> Result<(), SpatialAssemblyError> {
    if label.trim().is_empty() {
        Err(SpatialAssemblyError::InvalidLabel { field })
    } else {
        Ok(())
    }
}

fn require_distinct_bodies(
    first: SpatialBodyId,
    second: SpatialBodyId,
) -> Result<(), SpatialAssemblyError> {
    if first == second {
        Err(SpatialAssemblyError::SameBodyJointEndpoints(first))
    } else {
        Ok(())
    }
}

fn require_distinct_volume_points(
    points: [SpatialPointFeatureId; 4],
) -> Result<(), SpatialAssemblyError> {
    if points.into_iter().collect::<BTreeSet<_>>().len() == points.len() {
        Ok(())
    } else {
        invalid_field(
            "signed_volume_monitor.points",
            "signed-volume monitor requires four distinct point-feature IDs",
        )
    }
}

fn require_owned_unique_id<T: SpatialIdValue>(
    ids: &mut BTreeSet<u64>,
    namespace: u64,
    id: T,
) -> Result<(), SpatialAssemblyError> {
    let ordinal = id.ordinal();
    if !id.belongs_to_namespace(namespace) {
        invalid_field(
            "id",
            format!("ID {ordinal} belongs to another spatial assembly"),
        )
    } else if ordinal == 0 || !ids.insert(ordinal) {
        invalid_field("id", format!("ID {ordinal} is zero or duplicated"))
    } else {
        Ok(())
    }
}

fn allocate_spatial_assembly_namespace() -> Result<u64, SpatialAssemblyError> {
    NEXT_SPATIAL_ASSEMBLY_NAMESPACE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |namespace| {
            namespace.checked_add(1)
        })
        .map_err(|_| SpatialAssemblyError::IdExhausted)
}

fn variable_for_body(
    variables: &HashMap<SpatialBodyId, VariableId>,
    body: SpatialBodyId,
) -> Result<VariableId, SpatialAssemblyError> {
    variables
        .get(&body)
        .copied()
        .ok_or(SpatialAssemblyError::UnknownBody(body))
}

fn point_array(point: Point3<f64>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

fn include_normalized(
    maximum: &mut f64,
    values: &[f64],
    context: &str,
) -> Result<(), SpatialAssemblyError> {
    if !values.iter().all(|value| value.is_finite()) {
        return independent(format!("{context} independent residual is non-finite"));
    }
    for value in values {
        *maximum = maximum.max(value.abs());
    }
    Ok(())
}

fn regular_distance(
    displacement: Vector3<f64>,
    context: &str,
) -> Result<f64, SpatialAssemblyError> {
    if !displacement.iter().all(|value| value.is_finite()) {
        return independent(format!("{context} displacement is non-finite"));
    }
    let distance = robust_norm(displacement);
    if !distance.is_finite() {
        return independent(format!("{context} norm is non-finite"));
    }
    if distance == 0.0 {
        return independent(format!("{context} collapsed to a singular zero norm"));
    }
    Ok(distance)
}

fn robust_norm(vector: Vector3<f64>) -> f64 {
    vector.x.hypot(vector.y).hypot(vector.z)
}

fn require_parity_branch(
    source: SpatialSourceId,
    relation: &str,
    parity: SpatialAxisParity,
    first: Vector3<f64>,
    second: Vector3<f64>,
) -> Result<Vector3<f64>, SpatialAssemblyError> {
    let adjusted = second * parity.multiplier();
    let metric = first.dot(&adjusted);
    if !metric.is_finite() || metric <= ORIENTATION_BRANCH_MARGIN {
        independent(format!(
            "{relation} source {source} violated {parity:?} parity"
        ))
    } else {
        Ok(adjusted)
    }
}

fn checked_sum(
    values: impl IntoIterator<Item = usize>,
    message: &'static str,
) -> Result<usize, SpatialAssemblyError> {
    values.into_iter().try_fold(0_usize, |sum, value| {
        sum.checked_add(value)
            .ok_or_else(|| SpatialAssemblyError::GaugeCertification(message.to_owned()))
    })
}

fn find_root(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index {
        parents[index] = find_root(parents, parents[index]);
    }
    parents[index]
}

fn union_roots(parents: &mut [usize], first: usize, second: usize) {
    let first_root = find_root(parents, first);
    let second_root = find_root(parents, second);
    if first_root != second_root {
        let (lower, higher) = if first_root < second_root {
            (first_root, second_root)
        } else {
            (second_root, first_root)
        };
        parents[higher] = lower;
    }
}

fn invalid_field<T>(
    field: &'static str,
    message: impl Into<String>,
) -> Result<T, SpatialAssemblyError> {
    Err(SpatialAssemblyError::InvalidField {
        field,
        message: message.into(),
    })
}

fn independent<T>(message: impl Into<String>) -> Result<T, SpatialAssemblyError> {
    Err(SpatialAssemblyError::IndependentValidation(message.into()))
}

#[cfg(test)]
mod continuation_audit_tests {
    use crate::spatial_scenarios::{SpatialExampleIds, SpatialExampleKind, spatial_example};

    #[test]
    fn parameterized_spatial_drivers_audit_the_active_scalar_incidence() {
        let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
        let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
            unreachable!();
        };
        for (driver, parameter) in [(ids.drivers[0], 0.48), (ids.drivers[1], 1.9)] {
            let (compiled, parameter_variable) = fixture
                .assembly
                .compile_with_parameterized_driver(driver, parameter)
                .unwrap();
            let mapping = compiled.source_mapping(driver).unwrap();
            let [residual_id] = mapping.residual_ids.as_slice() else {
                panic!("parameterized driver must have one residual block");
            };
            let residual = compiled.problem.residual(*residual_id).unwrap();
            assert_eq!(residual.incident_variables().len(), 3);
            assert_eq!(
                residual.incident_variables().last(),
                Some(&parameter_variable)
            );
            let [audit] = residual.audit_rows() else {
                panic!("parameterized driver must have one audit row");
            };
            assert!(audit.template.contains("active continuation parameter"));
            assert!(audit.bindings.iter().any(|binding| {
                binding.name == "active_parameter"
                    && binding.value == "ephemeral spatial continuation scalar"
            }));
            assert!(audit.bindings.iter().all(|binding| {
                binding.name != "target_principal_phase_rad" && binding.name != "target_translation"
            }));
        }
    }
}
