use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::str::FromStr;

use geosolve_core::{SessionError, SolveSession, SolverConfig, VariableId};
use geosolve_geometry::{PlaneFrame, Point2, Point3, Pose2, Vector2, Vector3};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::compiler::AcceptedCompiledLinkage;
use crate::velocity::{VelocityGaugeComponent, velocity_from_accepted_session};
use crate::{
    AxisDirectionBranch, AxisFeatureId, BodyId, BranchMonitor, BranchMonitorId, BranchSign,
    DriverId, DriverKind, JointId, JointKind, Linkage, LinkageError, LinkageGeometry,
    LinkageSolveResult, LinkageSource, PointFeatureId, VelocityResult,
};

/// Current persistent planar-linkage document schema.
pub const PLANAR_LINKAGE_DOCUMENT_VERSION: u32 = 1;
const MAX_DOCUMENT_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_OBJECTS: usize = 100_000;
const MAX_LABEL_BYTES: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct PersistentId(u128);

impl fmt::Display for PersistentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

impl FromStr for PersistentId {
    type Err = PlanarLinkageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(PlanarLinkageError::InvalidId(value.to_owned()));
        }
        u128::from_str_radix(value, 16)
            .map(Self)
            .map_err(|_| PlanarLinkageError::InvalidId(value.to_owned()))
    }
}

impl Serialize for PersistentId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PersistentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

macro_rules! persistent_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(PersistentId);

        impl $name {
            #[must_use]
            pub const fn from_u128(value: u128) -> Self {
                Self(PersistentId(value))
            }

            #[must_use]
            pub const fn as_u128(self) -> u128 {
                self.0.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

persistent_id!(
    PlanarDocumentId,
    "Persistent planar-linkage document identity."
);
persistent_id!(PlanarBodyId, "Persistent planar rigid-body identity.");
persistent_id!(PlanarFeatureId, "Persistent body-local feature identity.");
persistent_id!(
    PlanarSourceId,
    "Persistent physical source and branch identity."
);

/// Persistence, validation, lowering, or session error for the planar architecture.
#[derive(Debug, Error)]
pub enum PlanarLinkageError {
    #[error("unsupported planar-linkage document version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid persistent ID {0:?}")]
    InvalidId(String),
    #[error("persistent ID space is exhausted")]
    IdExhausted,
    #[error("duplicate persistent ID {0}")]
    DuplicateId(String),
    #[error("invalid planar-linkage field {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("unknown persistent {kind} reference {id}")]
    UnknownReference { kind: &'static str, id: String },
    #[error("persistent feature {0} has the wrong point/axis kind")]
    FeatureKindMismatch(PlanarFeatureId),
    #[error("source order must contain every persistent source exactly once")]
    InvalidSourceOrder,
    #[error("accepted state must contain every body and driver exactly once")]
    IncompleteAcceptedState,
    #[error("stale planar-linkage revision: expected {expected}, actual {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("invalid numerical gauge policy: {0}")]
    InvalidGaugePolicy(String),
    #[error("planar gauge certification failed: {0}")]
    GaugeCertification(String),
    #[error("initial planar linkage was rejected: {0}")]
    InitialRejected(String),
    #[error("planar-linkage JSON exceeds the {MAX_DOCUMENT_JSON_BYTES}-byte limit")]
    JsonTooLarge,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Linkage(#[from] LinkageError),
    #[error(transparent)]
    Session(#[from] SessionError),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PlaneFrameRecord {
    origin: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
}

impl PlaneFrameRecord {
    fn from_frame(frame: PlaneFrame) -> Self {
        Self {
            origin: frame.origin().coords.into(),
            u: frame.u().into(),
            v: frame.v().into(),
        }
    }

    fn frame(self) -> Result<PlaneFrame, PlanarLinkageError> {
        PlaneFrame::try_new(
            Point3::new(self.origin[0], self.origin[1], self.origin[2]),
            Vector3::new(self.u[0], self.u[1], self.u[2]),
            Vector3::new(self.v[0], self.v[1], self.v[2]),
        )
        .map_err(|error| PlanarLinkageError::InvalidField {
            field: "topology.plane_frame",
            message: error.to_string(),
        })
    }
}

/// Persistent rigid-body topology without an accepted pose.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarBody {
    id: PlanarBodyId,
    label: String,
}

impl PlanarBody {
    #[must_use]
    pub const fn id(&self) -> PlanarBodyId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Persistent body-local point-feature topology.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarPointFeature {
    id: PlanarFeatureId,
    label: String,
    body: PlanarBodyId,
    local_point: [f64; 2],
}

impl PlanarPointFeature {
    #[must_use]
    pub const fn id(&self) -> PlanarFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> PlanarBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_point(&self) -> [f64; 2] {
        self.local_point
    }
}

/// Persistent body-local directed-axis topology.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarAxisFeature {
    id: PlanarFeatureId,
    label: String,
    body: PlanarBodyId,
    local_axis: [f64; 2],
}

impl PlanarAxisFeature {
    #[must_use]
    pub const fn id(&self) -> PlanarFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> PlanarBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_axis(&self) -> [f64; 2] {
        self.local_axis
    }
}

/// Persistent physical equation, driver, or explicit branch definition.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanarSourceKind {
    PhysicalGround {
        body: PlanarBodyId,
    },
    Revolute {
        first: PlanarFeatureId,
        second: PlanarFeatureId,
    },
    Prismatic {
        first_anchor: PlanarFeatureId,
        first_axis: PlanarFeatureId,
        second_anchor: PlanarFeatureId,
        second_axis: PlanarFeatureId,
        axis_branch: AxisDirectionBranch,
    },
    Weld {
        first: PlanarFeatureId,
        second: PlanarFeatureId,
        relative_angle: f64,
    },
    AngularDriver {
        reference: PlanarBodyId,
        driven: PlanarBodyId,
        max_continuation_step: f64,
    },
    LinearDriver {
        origin: PlanarFeatureId,
        measured: PlanarFeatureId,
        guide_axis: PlanarFeatureId,
        max_continuation_step: f64,
    },
    OrientationBranch {
        line_start: PlanarFeatureId,
        line_end: PlanarFeatureId,
        observed: PlanarFeatureId,
        sign: BranchSign,
    },
    DirectedDisplacementBranch {
        origin: PlanarFeatureId,
        measured: PlanarFeatureId,
        axis: PlanarFeatureId,
        sign: BranchSign,
    },
}

impl PlanarSourceKind {
    const fn is_driver(self) -> bool {
        matches!(self, Self::AngularDriver { .. } | Self::LinearDriver { .. })
    }
}

/// One persistent source record in explicit source order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarSource {
    id: PlanarSourceId,
    label: String,
    definition: PlanarSourceKind,
}

/// Persistent numerical coordinate policy, separate from physical source topology.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanarGaugePolicy {
    /// Anchor the lowest persistent body ID in every certified floating component.
    #[default]
    LowestPersistentBody,
    /// Select exactly one persistent reference body for every floating component.
    ExplicitReferences { bodies: Vec<PlanarBodyId> },
}

/// Domain certification of one component's common world-frame action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanarWorldActionCertification {
    FloatingSe2,
    PhysicallyGrounded,
}

/// Numerical reference selected for one floating component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanarGaugeReference {
    pub body: PlanarBodyId,
    pub target_pose: [f64; 3],
}

/// Gauge and physical equality mobility for one persistent planar component.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanarComponentGaugeReport {
    pub component_index: usize,
    pub bodies: Vec<PlanarBodyId>,
    pub sources: Vec<PlanarSourceId>,
    pub core_component_indices: Vec<usize>,
    pub numerical_equality_right_nullity: usize,
    pub gauge_dof: usize,
    pub internal_mobility: usize,
    pub world_action: PlanarWorldActionCertification,
    pub physical_ground_sources: Vec<PlanarSourceId>,
    pub numerical_reference: Option<PlanarGaugeReference>,
}

/// Domain-certified split of physical equality mobility into gauge and internal motion.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanarGaugeReport {
    pub numerical_equality_right_nullity: usize,
    pub gauge_dof: usize,
    pub internal_mobility: usize,
    pub components: Vec<PlanarComponentGaugeReport>,
}

impl PlanarSource {
    #[must_use]
    pub const fn id(&self) -> PlanarSourceId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn definition(&self) -> PlanarSourceKind {
        self.definition
    }
}

/// Persistent planar topology, independent of accepted continuous values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarLinkageTopology {
    model_scale: f64,
    plane_frame: PlaneFrameRecord,
    bodies: Vec<PlanarBody>,
    point_features: Vec<PlanarPointFeature>,
    axis_features: Vec<PlanarAxisFeature>,
    sources: Vec<PlanarSource>,
    source_order: Vec<PlanarSourceId>,
}

impl PlanarLinkageTopology {
    #[must_use]
    pub const fn model_scale(&self) -> f64 {
        self.model_scale
    }

    /// Returns the validated embedded plane.
    ///
    /// # Errors
    ///
    /// Returns an error if imported frame data is invalid.
    pub fn plane_frame(&self) -> Result<PlaneFrame, PlanarLinkageError> {
        self.plane_frame.frame()
    }

    #[must_use]
    pub fn bodies(&self) -> &[PlanarBody] {
        &self.bodies
    }

    #[must_use]
    pub fn point_features(&self) -> &[PlanarPointFeature] {
        &self.point_features
    }

    #[must_use]
    pub fn axis_features(&self) -> &[PlanarAxisFeature] {
        &self.axis_features
    }

    #[must_use]
    pub fn sources(&self) -> &[PlanarSource] {
        &self.sources
    }

    #[must_use]
    pub fn source_order(&self) -> &[PlanarSourceId] {
        &self.source_order
    }

    #[must_use]
    pub fn source(&self, id: PlanarSourceId) -> Option<&PlanarSource> {
        self.sources.iter().find(|source| source.id == id)
    }
}

/// One accepted body pose, separate from body topology.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarBodyState {
    body: PlanarBodyId,
    pose: [f64; 3],
}

impl PlanarBodyState {
    #[must_use]
    pub const fn body(&self) -> PlanarBodyId {
        self.body
    }

    #[must_use]
    pub const fn ambient_pose(&self) -> [f64; 3] {
        self.pose
    }

    /// Returns the validated manifold pose.
    ///
    /// # Errors
    ///
    /// Returns an error for non-finite imported state.
    pub fn pose(&self) -> Result<Pose2, PlanarLinkageError> {
        pose_from_ambient(self.pose, "accepted_state.body.pose")
    }
}

/// One accepted driver target, separate from driver topology.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarDriverState {
    source: PlanarSourceId,
    target: f64,
}

impl PlanarDriverState {
    #[must_use]
    pub const fn source(&self) -> PlanarSourceId {
        self.source
    }

    #[must_use]
    pub const fn target(&self) -> f64 {
        self.target
    }
}

/// Complete accepted continuous state keyed only by persistent IDs.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarLinkageAcceptedState {
    revision: u64,
    bodies: Vec<PlanarBodyState>,
    drivers: Vec<PlanarDriverState>,
}

impl PlanarLinkageAcceptedState {
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn bodies(&self) -> &[PlanarBodyState] {
        &self.bodies
    }

    #[must_use]
    pub fn drivers(&self) -> &[PlanarDriverState] {
        &self.drivers
    }

    #[must_use]
    pub fn body(&self, id: PlanarBodyId) -> Option<&PlanarBodyState> {
        self.bodies.iter().find(|state| state.body == id)
    }

    #[must_use]
    pub fn driver(&self, id: PlanarSourceId) -> Option<&PlanarDriverState> {
        self.drivers.iter().find(|state| state.source == id)
    }
}

/// Versioned persistent planar linkage with topology and accepted state separated.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanarLinkageDocument {
    version: u32,
    id: PlanarDocumentId,
    next_id: PersistentId,
    gauge_policy: PlanarGaugePolicy,
    topology: PlanarLinkageTopology,
    accepted_state: PlanarLinkageAcceptedState,
}

/// Runtime point/axis feature selected by one persistent feature ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanarRuntimeFeature {
    Point(PointFeatureId),
    Axis(AxisFeatureId),
}

/// Runtime object selected by one persistent source ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanarRuntimeSource {
    Ground(BodyId),
    Joint(JointId),
    Driver(DriverId),
    BranchMonitor(BranchMonitorId),
}

/// Deterministic persistent-to-runtime mapping; runtime keys are never serialized.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanarLinkageRuntimeMap {
    bodies: Vec<(PlanarBodyId, BodyId)>,
    features: Vec<(PlanarFeatureId, PlanarRuntimeFeature)>,
    sources: Vec<(PlanarSourceId, PlanarRuntimeSource)>,
}

impl PlanarLinkageRuntimeMap {
    #[must_use]
    pub fn runtime_body(&self, id: PlanarBodyId) -> Option<BodyId> {
        self.bodies
            .iter()
            .find_map(|(persistent, runtime)| (*persistent == id).then_some(*runtime))
    }

    #[must_use]
    pub fn persistent_body(&self, id: BodyId) -> Option<PlanarBodyId> {
        self.bodies
            .iter()
            .find_map(|(persistent, runtime)| (*runtime == id).then_some(*persistent))
    }

    #[must_use]
    pub fn runtime_feature(&self, id: PlanarFeatureId) -> Option<PlanarRuntimeFeature> {
        self.features
            .iter()
            .find_map(|(persistent, runtime)| (*persistent == id).then_some(*runtime))
    }

    #[must_use]
    pub fn persistent_point_feature(&self, id: PointFeatureId) -> Option<PlanarFeatureId> {
        self.features.iter().find_map(|(persistent, runtime)| {
            (*runtime == PlanarRuntimeFeature::Point(id)).then_some(*persistent)
        })
    }

    #[must_use]
    pub fn persistent_axis_feature(&self, id: AxisFeatureId) -> Option<PlanarFeatureId> {
        self.features.iter().find_map(|(persistent, runtime)| {
            (*runtime == PlanarRuntimeFeature::Axis(id)).then_some(*persistent)
        })
    }

    #[must_use]
    pub fn runtime_source(&self, id: PlanarSourceId) -> Option<PlanarRuntimeSource> {
        self.sources
            .iter()
            .find_map(|(persistent, runtime)| (*persistent == id).then_some(*runtime))
    }

    #[must_use]
    pub fn persistent_source(&self, source: LinkageSource) -> Option<PlanarSourceId> {
        let runtime = match source {
            LinkageSource::Ground(body) => PlanarRuntimeSource::Ground(body),
            LinkageSource::Joint(joint) => PlanarRuntimeSource::Joint(joint),
            LinkageSource::Driver(driver) => PlanarRuntimeSource::Driver(driver),
        };
        self.sources
            .iter()
            .find_map(|(persistent, candidate)| (*candidate == runtime).then_some(*persistent))
    }

    #[must_use]
    pub fn persistent_branch_monitor(&self, id: BranchMonitorId) -> Option<PlanarSourceId> {
        self.sources.iter().find_map(|(persistent, runtime)| {
            (*runtime == PlanarRuntimeSource::BranchMonitor(id)).then_some(*persistent)
        })
    }
}

#[derive(Debug)]
struct IdAllocator {
    next: u128,
}

impl IdAllocator {
    fn new(document: PlanarDocumentId) -> Result<Self, PlanarLinkageError> {
        if document.as_u128() == 0 {
            return Err(PlanarLinkageError::InvalidId(document.to_string()));
        }
        Ok(Self {
            next: document
                .as_u128()
                .checked_add(1)
                .ok_or(PlanarLinkageError::IdExhausted)?,
        })
    }

    fn allocate(&mut self) -> Result<PersistentId, PlanarLinkageError> {
        let id = PersistentId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(PlanarLinkageError::IdExhausted)?;
        Ok(id)
    }
}

impl PlanarLinkageDocument {
    /// Captures a compatibility linkage into separated persistent topology/state.
    ///
    /// # Errors
    ///
    /// Returns an error for an exhausted namespace or invalid staged values.
    #[allow(clippy::too_many_lines)]
    pub fn from_linkage(
        id: PlanarDocumentId,
        linkage: &Linkage,
    ) -> Result<(Self, PlanarLinkageRuntimeMap), PlanarLinkageError> {
        let mut allocator = IdAllocator::new(id)?;
        let mut runtime_map = PlanarLinkageRuntimeMap::default();
        let mut body_ids = HashMap::new();
        let mut point_ids = HashMap::new();
        let mut axis_ids = HashMap::new();
        let mut bodies = Vec::new();
        let mut body_states = Vec::new();
        for (runtime_id, body) in linkage.bodies() {
            let persistent = PlanarBodyId(allocator.allocate()?);
            body_ids.insert(runtime_id, persistent);
            runtime_map.bodies.push((persistent, runtime_id));
            bodies.push(PlanarBody {
                id: persistent,
                label: body.label().to_owned(),
            });
            body_states.push(PlanarBodyState {
                body: persistent,
                pose: body.pose().ambient(),
            });
        }
        let mut point_features = Vec::new();
        for (runtime_id, feature) in linkage.point_features() {
            let persistent = PlanarFeatureId(allocator.allocate()?);
            point_ids.insert(runtime_id, persistent);
            runtime_map
                .features
                .push((persistent, PlanarRuntimeFeature::Point(runtime_id)));
            point_features.push(PlanarPointFeature {
                id: persistent,
                label: feature.label().to_owned(),
                body: body_id(&body_ids, feature.body())?,
                local_point: [feature.local_point().x, feature.local_point().y],
            });
        }
        let mut axis_features = Vec::new();
        for (runtime_id, feature) in linkage.axis_features() {
            let persistent = PlanarFeatureId(allocator.allocate()?);
            axis_ids.insert(runtime_id, persistent);
            runtime_map
                .features
                .push((persistent, PlanarRuntimeFeature::Axis(runtime_id)));
            axis_features.push(PlanarAxisFeature {
                id: persistent,
                label: feature.label().to_owned(),
                body: body_id(&body_ids, feature.body())?,
                local_axis: [feature.local_axis().x, feature.local_axis().y],
            });
        }

        let mut sources = Vec::new();
        let mut source_order = Vec::new();
        for (runtime_id, body) in linkage.bodies().filter(|(_, body)| body.grounded()) {
            let source = PlanarSourceId(allocator.allocate()?);
            sources.push(PlanarSource {
                id: source,
                label: body
                    .ground_source_label()
                    .map_or_else(|| format!("grounded body {}", body.label()), str::to_owned),
                definition: PlanarSourceKind::PhysicalGround {
                    body: body_id(&body_ids, runtime_id)?,
                },
            });
            source_order.push(source);
            runtime_map
                .sources
                .push((source, PlanarRuntimeSource::Ground(runtime_id)));
        }
        for (runtime_id, joint) in linkage.joints() {
            let source = PlanarSourceId(allocator.allocate()?);
            let definition = persistent_joint(joint.kind(), &point_ids, &axis_ids)?;
            sources.push(PlanarSource {
                id: source,
                label: joint.label().to_owned(),
                definition,
            });
            source_order.push(source);
            runtime_map
                .sources
                .push((source, PlanarRuntimeSource::Joint(runtime_id)));
        }
        let mut driver_states = Vec::new();
        for (runtime_id, driver) in linkage.drivers() {
            let source = PlanarSourceId(allocator.allocate()?);
            let definition = persistent_driver(
                driver.kind(),
                driver.max_continuation_step(),
                &body_ids,
                &point_ids,
                &axis_ids,
            )?;
            sources.push(PlanarSource {
                id: source,
                label: driver.label().to_owned(),
                definition,
            });
            source_order.push(source);
            driver_states.push(PlanarDriverState {
                source,
                target: driver.target(),
            });
            runtime_map
                .sources
                .push((source, PlanarRuntimeSource::Driver(runtime_id)));
        }
        for (index, (runtime_id, monitor)) in linkage.branch_monitors().enumerate() {
            let source = PlanarSourceId(allocator.allocate()?);
            let (label, definition) = persistent_monitor(index, *monitor, &point_ids, &axis_ids)?;
            sources.push(PlanarSource {
                id: source,
                label,
                definition,
            });
            source_order.push(source);
            runtime_map
                .sources
                .push((source, PlanarRuntimeSource::BranchMonitor(runtime_id)));
        }
        let document = Self {
            version: PLANAR_LINKAGE_DOCUMENT_VERSION,
            id,
            next_id: PersistentId(allocator.next),
            gauge_policy: PlanarGaugePolicy::LowestPersistentBody,
            topology: PlanarLinkageTopology {
                model_scale: linkage.model_scale(),
                plane_frame: PlaneFrameRecord::from_frame(linkage.plane_frame()),
                bodies,
                point_features,
                axis_features,
                sources,
                source_order,
            },
            accepted_state: PlanarLinkageAcceptedState {
                revision: 0,
                bodies: body_states,
                drivers: driver_states,
            },
        };
        document.validate_structure()?;
        Ok((document, runtime_map))
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn id(&self) -> PlanarDocumentId {
        self.id
    }

    #[must_use]
    pub const fn gauge_policy(&self) -> &PlanarGaugePolicy {
        &self.gauge_policy
    }

    /// Replaces numerical gauge metadata without changing physical topology.
    ///
    /// # Errors
    ///
    /// Rejects stale, duplicate, missing, or physically grounded references.
    pub fn set_gauge_policy(
        &mut self,
        policy: PlanarGaugePolicy,
    ) -> Result<(), PlanarLinkageError> {
        let previous = self.gauge_policy.clone();
        self.gauge_policy = policy;
        self.canonicalize();
        if let Err(error) = self.validate_structure() {
            self.gauge_policy = previous;
            return Err(error);
        }
        Ok(())
    }

    #[must_use]
    pub const fn topology(&self) -> &PlanarLinkageTopology {
        &self.topology
    }

    #[must_use]
    pub const fn accepted_state(&self) -> &PlanarLinkageAcceptedState {
        &self.accepted_state
    }

    /// Serializes the canonical persistent document.
    ///
    /// # Errors
    ///
    /// Returns an error if internal validation or JSON serialization fails.
    pub fn to_json(&self) -> Result<String, PlanarLinkageError> {
        self.validate_structure()?;
        self.lower()?;
        Ok(serde_json::to_string(self)?)
    }

    /// Parses, canonicalizes, and validates a persistent document.
    ///
    /// # Errors
    ///
    /// Rejects oversized, malformed, unsupported, non-finite, duplicate, stale,
    /// or incomplete documents.
    pub fn from_json(json: &str) -> Result<Self, PlanarLinkageError> {
        if json.len() > MAX_DOCUMENT_JSON_BYTES {
            return Err(PlanarLinkageError::JsonTooLarge);
        }
        let mut document: Self = serde_json::from_str(json)?;
        document.canonicalize();
        document.validate_structure()?;
        document.lower()?;
        Ok(document)
    }

    /// Deterministically lowers persistent IDs into fresh runtime keys.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid references, geometry, state, or source data.
    pub fn lower(&self) -> Result<(Linkage, PlanarLinkageRuntimeMap), PlanarLinkageError> {
        self.validate_structure()?;
        self.lower_validated()
    }

    fn canonicalize(&mut self) {
        self.topology.bodies.sort_by_key(|body| body.id);
        self.topology
            .point_features
            .sort_by_key(|feature| feature.id);
        self.topology
            .axis_features
            .sort_by_key(|feature| feature.id);
        self.topology.sources.sort_by_key(|source| source.id);
        self.accepted_state.bodies.sort_by_key(|state| state.body);
        self.accepted_state
            .drivers
            .sort_by_key(|state| state.source);
        if let PlanarGaugePolicy::ExplicitReferences { bodies } = &mut self.gauge_policy {
            bodies.sort_unstable();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn validate_structure(&self) -> Result<(), PlanarLinkageError> {
        if self.version != PLANAR_LINKAGE_DOCUMENT_VERSION {
            return Err(PlanarLinkageError::UnsupportedVersion(self.version));
        }
        if self.id.as_u128() == 0 || self.next_id.0 == 0 {
            return Err(PlanarLinkageError::InvalidId(self.id.to_string()));
        }
        let object_count = self.topology.bodies.len()
            + self.topology.point_features.len()
            + self.topology.axis_features.len()
            + self.topology.sources.len();
        if object_count > MAX_DOCUMENT_OBJECTS {
            return invalid("topology", "too many persistent objects");
        }
        let mut ids = BTreeSet::new();
        ids.insert(self.id.as_u128());
        for (id, label) in self
            .topology
            .bodies
            .iter()
            .map(|body| (body.id.as_u128(), body.label.as_str()))
            .chain(
                self.topology
                    .point_features
                    .iter()
                    .map(|feature| (feature.id.as_u128(), feature.label.as_str())),
            )
            .chain(
                self.topology
                    .axis_features
                    .iter()
                    .map(|feature| (feature.id.as_u128(), feature.label.as_str())),
            )
            .chain(
                self.topology
                    .sources
                    .iter()
                    .map(|source| (source.id.as_u128(), source.label.as_str())),
            )
        {
            if id == 0 || !ids.insert(id) {
                return Err(PlanarLinkageError::DuplicateId(format!("{id:032x}")));
            }
            validate_label(label)?;
        }
        if ids
            .iter()
            .next_back()
            .is_some_and(|maximum| self.next_id.0 <= *maximum)
        {
            return invalid("next_id", "must exceed every allocated persistent ID");
        }
        let bodies = self
            .topology
            .bodies
            .iter()
            .map(|body| body.id)
            .collect::<BTreeSet<_>>();
        let point_features = self
            .topology
            .point_features
            .iter()
            .map(|feature| feature.id)
            .collect::<BTreeSet<_>>();
        let axis_features = self
            .topology
            .axis_features
            .iter()
            .map(|feature| feature.id)
            .collect::<BTreeSet<_>>();
        for feature in &self.topology.point_features {
            require_member(&bodies, feature.body, "body")?;
        }
        for feature in &self.topology.axis_features {
            require_member(&bodies, feature.body, "body")?;
        }
        let source_ids = self
            .topology
            .sources
            .iter()
            .map(|source| source.id)
            .collect::<BTreeSet<_>>();
        let ordered_sources = self
            .topology
            .source_order
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if source_ids != ordered_sources || source_ids.len() != self.topology.source_order.len() {
            return Err(PlanarLinkageError::InvalidSourceOrder);
        }
        let mut grounded = BTreeSet::new();
        for source in &self.topology.sources {
            validate_source(
                source,
                &bodies,
                &point_features,
                &axis_features,
                &mut grounded,
            )?;
        }
        let body_states = self
            .accepted_state
            .bodies
            .iter()
            .map(|state| state.body)
            .collect::<BTreeSet<_>>();
        let driver_sources = self
            .topology
            .sources
            .iter()
            .filter(|source| source.definition.is_driver())
            .map(|source| source.id)
            .collect::<BTreeSet<_>>();
        let driver_states = self
            .accepted_state
            .drivers
            .iter()
            .map(|state| state.source)
            .collect::<BTreeSet<_>>();
        if body_states != bodies
            || body_states.len() != self.accepted_state.bodies.len()
            || driver_states != driver_sources
            || driver_states.len() != self.accepted_state.drivers.len()
        {
            return Err(PlanarLinkageError::IncompleteAcceptedState);
        }
        for state in &self.accepted_state.bodies {
            state.pose()?;
        }
        for state in &self.accepted_state.drivers {
            if !state.target.is_finite() {
                return invalid("accepted_state.driver.target", "must be finite");
            }
        }
        self.topology.plane_frame.frame()?;
        let components = certified_components(&self.topology)?;
        resolve_gauge_references(&self.gauge_policy, &components)?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn lower_validated(&self) -> Result<(Linkage, PlanarLinkageRuntimeMap), PlanarLinkageError> {
        let mut linkage = Linkage::new(
            self.topology.model_scale,
            self.topology.plane_frame.frame()?,
        )?;
        let source_lookup = self
            .topology
            .sources
            .iter()
            .map(|source| (source.id, source))
            .collect::<HashMap<_, _>>();
        let grounded = self
            .topology
            .sources
            .iter()
            .filter_map(|source| match source.definition {
                PlanarSourceKind::PhysicalGround { body } => Some(body),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let body_states = self
            .accepted_state
            .bodies
            .iter()
            .map(|state| (state.body, state))
            .collect::<HashMap<_, _>>();
        let driver_states = self
            .accepted_state
            .drivers
            .iter()
            .map(|state| (state.source, state.target))
            .collect::<HashMap<_, _>>();
        let mut runtime_map = PlanarLinkageRuntimeMap::default();
        for body in &self.topology.bodies {
            let pose = body_states
                .get(&body.id)
                .ok_or(PlanarLinkageError::IncompleteAcceptedState)?
                .pose()?;
            let runtime = linkage.add_body(&body.label, pose, grounded.contains(&body.id))?;
            runtime_map.bodies.push((body.id, runtime));
        }
        for feature in &self.topology.point_features {
            let runtime = linkage.add_point_feature(
                &feature.label,
                runtime_map.runtime_body(feature.body).ok_or_else(|| {
                    PlanarLinkageError::UnknownReference {
                        kind: "body",
                        id: feature.body.to_string(),
                    }
                })?,
                Point2::new(feature.local_point[0], feature.local_point[1]),
            )?;
            runtime_map
                .features
                .push((feature.id, PlanarRuntimeFeature::Point(runtime)));
        }
        for feature in &self.topology.axis_features {
            let runtime = linkage.add_axis_feature(
                &feature.label,
                runtime_map.runtime_body(feature.body).ok_or_else(|| {
                    PlanarLinkageError::UnknownReference {
                        kind: "body",
                        id: feature.body.to_string(),
                    }
                })?,
                Vector2::new(feature.local_axis[0], feature.local_axis[1]),
            )?;
            runtime_map
                .features
                .push((feature.id, PlanarRuntimeFeature::Axis(runtime)));
        }
        for source_id in &self.topology.source_order {
            let source = source_lookup.get(source_id).ok_or_else(|| {
                PlanarLinkageError::UnknownReference {
                    kind: "source",
                    id: source_id.to_string(),
                }
            })?;
            let runtime = lower_source(
                &mut linkage,
                &runtime_map,
                source,
                driver_states.get(source_id).copied(),
            )?;
            runtime_map.sources.push((*source_id, runtime));
        }
        Ok((linkage, runtime_map))
    }

    fn project_runtime_state(
        &mut self,
        linkage: &Linkage,
        mappings: &PlanarLinkageRuntimeMap,
    ) -> Result<(), PlanarLinkageError> {
        for state in &mut self.accepted_state.bodies {
            let runtime = mappings.runtime_body(state.body).ok_or_else(|| {
                PlanarLinkageError::UnknownReference {
                    kind: "body",
                    id: state.body.to_string(),
                }
            })?;
            state.pose = linkage
                .body(runtime)
                .ok_or(LinkageError::UnknownBody(runtime))?
                .pose()
                .ambient();
        }
        for state in &mut self.accepted_state.drivers {
            let PlanarRuntimeSource::Driver(runtime) = mappings
                .runtime_source(state.source)
                .ok_or_else(|| PlanarLinkageError::UnknownReference {
                    kind: "source",
                    id: state.source.to_string(),
                })?
            else {
                return invalid(
                    "accepted_state.driver.source",
                    "persistent driver state mapped to a non-driver runtime source",
                );
            };
            state.target = linkage
                .driver(runtime)
                .ok_or(LinkageError::UnknownDriver(runtime))?
                .target();
        }
        Ok(())
    }
}

/// Accepted persistent document plus its private runtime and compiled core session.
#[derive(Debug)]
pub struct PlanarLinkageSession {
    document: PlanarLinkageDocument,
    runtime: Linkage,
    runtime_map: PlanarLinkageRuntimeMap,
    accepted_compiled: AcceptedCompiledLinkage,
    accepted_result: LinkageSolveResult,
    gauge_report: PlanarGaugeReport,
    config: SolverConfig,
}

impl PlanarLinkageSession {
    /// Lowers, solves, validates, and retains an initial persistent planar document.
    ///
    /// # Errors
    ///
    /// Rejects invalid persistence, linkage, branch, hard-residual, or core-session state.
    pub fn new(
        mut document: PlanarLinkageDocument,
        config: SolverConfig,
    ) -> Result<Self, PlanarLinkageError> {
        document.canonicalize();
        document.validate_structure()?;
        let components = certified_components(&document.topology)?;
        let gauge_references = resolve_gauge_references(&document.gauge_policy, &components)?;
        let (mut runtime, runtime_map) = document.lower_validated()?;

        let mut private_compiled = runtime.compile()?;
        for reference in gauge_references.iter().flatten() {
            let runtime_body = runtime_map
                .runtime_body(*reference)
                .ok_or_else(|| unknown("body", *reference))?;
            let target = runtime
                .body(runtime_body)
                .ok_or(LinkageError::UnknownBody(runtime_body))?
                .pose();
            private_compiled.add_numerical_pose_gauge(
                runtime_body,
                target,
                document.topology.model_scale,
            )?;
        }
        let private_accepted = private_compiled
            .into_accepted_session(config)
            .map_err(|error| PlanarLinkageError::InitialRejected(error.to_string()))?;
        let private_geometry = private_accepted.solved_geometry()?;
        validate_runtime_candidate(&runtime, &private_geometry, config)?;
        project_geometry_to_runtime(&mut runtime, &private_geometry)?;

        let accepted_compiled = runtime
            .compile()?
            .into_accepted_session(config)
            .map_err(|error| PlanarLinkageError::InitialRejected(error.to_string()))?;
        let physical_geometry = accepted_compiled.solved_geometry()?;
        validate_runtime_candidate(&runtime, &physical_geometry, config)?;
        project_geometry_to_runtime(&mut runtime, &physical_geometry)?;
        document.project_runtime_state(&runtime, &runtime_map)?;
        let accepted_result = runtime.accepted_result_from_session(&accepted_compiled)?;
        let gauge_report = build_gauge_report(
            &document,
            &runtime_map,
            &components,
            &gauge_references,
            &accepted_compiled,
        )?;
        Ok(Self {
            document,
            runtime,
            runtime_map,
            accepted_compiled,
            accepted_result,
            gauge_report,
            config,
        })
    }

    #[must_use]
    pub const fn document(&self) -> &PlanarLinkageDocument {
        &self.document
    }

    #[must_use]
    pub const fn runtime(&self) -> &Linkage {
        &self.runtime
    }

    #[must_use]
    pub const fn runtime_map(&self) -> &PlanarLinkageRuntimeMap {
        &self.runtime_map
    }

    #[must_use]
    pub const fn core_session(&self) -> &SolveSession {
        self.accepted_compiled.session()
    }

    #[must_use]
    pub const fn accepted_result(&self) -> &LinkageSolveResult {
        &self.accepted_result
    }

    #[must_use]
    pub const fn gauge_report(&self) -> &PlanarGaugeReport {
        &self.gauge_report
    }

    /// Solves one persistent driver rate from this session's accepted hard linearization.
    ///
    /// # Errors
    ///
    /// Rejects a stale/non-driver source, non-finite rate, inconsistent
    /// differentiated equations, or any failed independent validation.
    pub fn velocity(
        &self,
        driver: PlanarSourceId,
        driver_rate: f64,
    ) -> Result<VelocityResult, PlanarLinkageError> {
        let PlanarRuntimeSource::Driver(runtime_driver) =
            self.runtime_map
                .runtime_source(driver)
                .ok_or_else(|| unknown("source", driver))?
        else {
            return invalid("velocity.driver", "persistent source is not a driver");
        };
        let gauges = self
            .gauge_report
            .components
            .iter()
            .filter_map(|component| {
                component.numerical_reference.map(|reference| {
                    let bodies = component
                        .bodies
                        .iter()
                        .map(|body| {
                            self.runtime_map
                                .runtime_body(*body)
                                .ok_or_else(|| unknown("body", *body))
                        })
                        .collect::<Result<Vec<_>, PlanarLinkageError>>()?;
                    let reference = self
                        .runtime_map
                        .runtime_body(reference.body)
                        .ok_or_else(|| unknown("body", reference.body))?;
                    Ok(VelocityGaugeComponent { bodies, reference })
                })
            })
            .collect::<Result<Vec<_>, PlanarLinkageError>>()?;
        Ok(velocity_from_accepted_session(
            &self.runtime,
            &self.accepted_compiled,
            runtime_driver,
            driver_rate,
            &gauges,
        )?)
    }

    /// Explicitly changes numerical gauge metadata and rebuilds transactionally.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions, invalid references, or a replacement session
    /// that cannot independently validate the retained physical state.
    pub fn set_gauge_policy(
        &mut self,
        expected_revision: u64,
        policy: PlanarGaugePolicy,
    ) -> Result<(), PlanarLinkageError> {
        let actual = self.document.accepted_state.revision;
        if expected_revision != actual {
            return Err(PlanarLinkageError::StaleRevision {
                expected: expected_revision,
                actual,
            });
        }
        let mut candidate = self.document.clone();
        candidate.set_gauge_policy(policy)?;
        candidate.accepted_state.revision =
            actual
                .checked_add(1)
                .ok_or_else(|| PlanarLinkageError::InvalidField {
                    field: "accepted_state.revision",
                    message: "revision is exhausted".to_owned(),
                })?;
        let replacement = Self::new(candidate, self.config)?;
        *self = replacement;
        Ok(())
    }

    #[must_use]
    pub fn into_document(self) -> PlanarLinkageDocument {
        self.document
    }
}

#[derive(Clone, Debug)]
struct CertifiedPlanarComponent {
    bodies: Vec<PlanarBodyId>,
    sources: Vec<PlanarSourceId>,
    physical_ground_sources: Vec<PlanarSourceId>,
}

fn certified_components(
    topology: &PlanarLinkageTopology,
) -> Result<Vec<CertifiedPlanarComponent>, PlanarLinkageError> {
    let bodies = topology
        .bodies
        .iter()
        .map(|body| body.id)
        .collect::<Vec<_>>();
    let body_indices = bodies
        .iter()
        .enumerate()
        .map(|(index, body)| (*body, index))
        .collect::<HashMap<_, _>>();
    let point_owners = topology
        .point_features
        .iter()
        .map(|feature| (feature.id, feature.body))
        .collect::<HashMap<_, _>>();
    let axis_owners = topology
        .axis_features
        .iter()
        .map(|feature| (feature.id, feature.body))
        .collect::<HashMap<_, _>>();
    let source_lookup = topology
        .sources
        .iter()
        .map(|source| (source.id, source))
        .collect::<HashMap<_, _>>();
    let mut parents = (0..bodies.len()).collect::<Vec<_>>();
    for source_id in &topology.source_order {
        let source = source_lookup
            .get(source_id)
            .ok_or_else(|| unknown("source", *source_id))?;
        let source_bodies = persistent_source_bodies(source, &point_owners, &axis_owners)?;
        if let Some((&first, rest)) = source_bodies.split_first() {
            let first = *body_indices
                .get(&first)
                .ok_or_else(|| unknown("body", first))?;
            for body in rest {
                let next = *body_indices
                    .get(body)
                    .ok_or_else(|| unknown("body", *body))?;
                union_roots(&mut parents, first, next);
            }
        }
    }
    let mut groups = BTreeMap::<PlanarBodyId, Vec<PlanarBodyId>>::new();
    for (index, body) in bodies.iter().copied().enumerate() {
        let root = find_root(&mut parents, index);
        let key = bodies[root];
        groups.entry(key).or_default().push(body);
    }
    let mut components = groups
        .into_values()
        .map(|mut component_bodies| {
            component_bodies.sort_unstable();
            CertifiedPlanarComponent {
                bodies: component_bodies,
                sources: Vec::new(),
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
    for source_id in &topology.source_order {
        let source = source_lookup
            .get(source_id)
            .ok_or_else(|| unknown("source", *source_id))?;
        let source_bodies = persistent_source_bodies(source, &point_owners, &axis_owners)?;
        let first = *source_bodies.first().ok_or_else(|| {
            PlanarLinkageError::GaugeCertification(format!(
                "source {source_id} has no incident body"
            ))
        })?;
        let component_index = *component_for_body
            .get(&first)
            .ok_or_else(|| unknown("body", first))?;
        let component = &mut components[component_index];
        component.sources.push(*source_id);
        if matches!(source.definition, PlanarSourceKind::PhysicalGround { .. }) {
            component.physical_ground_sources.push(*source_id);
        }
    }
    Ok(components)
}

fn persistent_source_bodies(
    source: &PlanarSource,
    points: &HashMap<PlanarFeatureId, PlanarBodyId>,
    axes: &HashMap<PlanarFeatureId, PlanarBodyId>,
) -> Result<Vec<PlanarBodyId>, PlanarLinkageError> {
    let point = |feature| {
        points
            .get(&feature)
            .copied()
            .ok_or_else(|| unknown("point feature", feature))
    };
    let axis_owner = |feature| {
        axes.get(&feature)
            .copied()
            .ok_or_else(|| unknown("axis feature", feature))
    };
    let mut bodies = match source.definition {
        PlanarSourceKind::PhysicalGround { body } => vec![body],
        PlanarSourceKind::Revolute { first, second }
        | PlanarSourceKind::Weld { first, second, .. } => vec![point(first)?, point(second)?],
        PlanarSourceKind::Prismatic {
            first_anchor,
            first_axis,
            second_anchor,
            second_axis,
            ..
        } => vec![
            point(first_anchor)?,
            axis_owner(first_axis)?,
            point(second_anchor)?,
            axis_owner(second_axis)?,
        ],
        PlanarSourceKind::AngularDriver {
            reference, driven, ..
        } => vec![reference, driven],
        PlanarSourceKind::LinearDriver {
            origin,
            measured,
            guide_axis,
            ..
        }
        | PlanarSourceKind::DirectedDisplacementBranch {
            origin,
            measured,
            axis: guide_axis,
            ..
        } => vec![point(origin)?, point(measured)?, axis_owner(guide_axis)?],
        PlanarSourceKind::OrientationBranch {
            line_start,
            line_end,
            observed,
            ..
        } => vec![point(line_start)?, point(line_end)?, point(observed)?],
    };
    bodies.sort_unstable();
    bodies.dedup();
    Ok(bodies)
}

fn resolve_gauge_references(
    policy: &PlanarGaugePolicy,
    components: &[CertifiedPlanarComponent],
) -> Result<Vec<Option<PlanarBodyId>>, PlanarLinkageError> {
    match policy {
        PlanarGaugePolicy::LowestPersistentBody => Ok(components
            .iter()
            .map(|component| {
                component
                    .physical_ground_sources
                    .is_empty()
                    .then_some(component.bodies[0])
            })
            .collect()),
        PlanarGaugePolicy::ExplicitReferences { bodies } => {
            if bodies.iter().copied().collect::<BTreeSet<_>>().len() != bodies.len() {
                return Err(PlanarLinkageError::InvalidGaugePolicy(
                    "explicit references must be unique".to_owned(),
                ));
            }
            let all_bodies = components
                .iter()
                .flat_map(|component| component.bodies.iter().copied())
                .collect::<BTreeSet<_>>();
            if let Some(body) = bodies.iter().find(|body| !all_bodies.contains(body)) {
                return Err(PlanarLinkageError::InvalidGaugePolicy(format!(
                    "unknown explicit body reference {body}"
                )));
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
                            Err(PlanarLinkageError::InvalidGaugePolicy(format!(
                                "floating component beginning at {} requires exactly one reference",
                                component.bodies[0]
                            )))
                        }
                    } else if selected.is_empty() {
                        Ok(None)
                    } else {
                        Err(PlanarLinkageError::InvalidGaugePolicy(format!(
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
    document: &PlanarLinkageDocument,
    runtime_map: &PlanarLinkageRuntimeMap,
    certified: &[CertifiedPlanarComponent],
    references: &[Option<PlanarBodyId>],
    accepted: &AcceptedCompiledLinkage,
) -> Result<PlanarGaugeReport, PlanarLinkageError> {
    let component_for_body = certified
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.bodies.iter().map(move |body| (*body, index)))
        .collect::<HashMap<_, _>>();
    let mut variable_components = HashMap::<VariableId, usize>::new();
    for mapping in accepted.body_variables() {
        let persistent = runtime_map
            .persistent_body(mapping.body_id)
            .ok_or_else(|| {
                PlanarLinkageError::GaugeCertification(format!(
                    "runtime body {:?} has no persistent identity",
                    mapping.body_id
                ))
            })?;
        let component = *component_for_body
            .get(&persistent)
            .ok_or_else(|| unknown("body", persistent))?;
        variable_components.insert(mapping.variable_id, component);
    }
    let mut core_components = vec![Vec::new(); certified.len()];
    let mut right_nullities = vec![0_usize; certified.len()];
    for summary in &accepted.session().report().structural.component_summaries {
        let domain_components = summary
            .variable_ids
            .iter()
            .map(|variable| {
                variable_components.get(variable).copied().ok_or_else(|| {
                    PlanarLinkageError::GaugeCertification(format!(
                        "core variable {variable:?} is not a planar body"
                    ))
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if domain_components.len() != 1 {
            return Err(PlanarLinkageError::GaugeCertification(format!(
                "core component {} does not map to exactly one planar component",
                summary.component_index
            )));
        }
        let domain_component = *domain_components.iter().next().expect("length checked");
        let solve = accepted
            .session()
            .report()
            .component_solves
            .iter()
            .find(|solve| solve.component_index == summary.component_index)
            .ok_or_else(|| {
                PlanarLinkageError::GaugeCertification(format!(
                    "core component {} has no numerical report",
                    summary.component_index
                ))
            })?;
        core_components[domain_component].push(summary.component_index);
        right_nullities[domain_component] = right_nullities[domain_component]
            .checked_add(solve.right_nullity)
            .ok_or_else(|| {
                PlanarLinkageError::GaugeCertification(
                    "component right nullity overflowed".to_owned(),
                )
            })?;
    }
    let mut components = Vec::with_capacity(certified.len());
    for (index, component) in certified.iter().enumerate() {
        let (world_action, gauge_dof, numerical_reference) = if let Some(body) = references[index] {
            let target_pose = document
                .accepted_state
                .body(body)
                .ok_or(PlanarLinkageError::IncompleteAcceptedState)?
                .ambient_pose();
            (
                PlanarWorldActionCertification::FloatingSe2,
                3,
                Some(PlanarGaugeReference { body, target_pose }),
            )
        } else {
            (PlanarWorldActionCertification::PhysicallyGrounded, 0, None)
        };
        if right_nullities[index] < gauge_dof {
            return Err(PlanarLinkageError::GaugeCertification(format!(
                "component {index} has right nullity {} below certified gauge DOF {gauge_dof}",
                right_nullities[index]
            )));
        }
        components.push(PlanarComponentGaugeReport {
            component_index: index,
            bodies: component.bodies.clone(),
            sources: component.sources.clone(),
            core_component_indices: core_components[index].clone(),
            numerical_equality_right_nullity: right_nullities[index],
            gauge_dof,
            internal_mobility: right_nullities[index] - gauge_dof,
            world_action,
            physical_ground_sources: component.physical_ground_sources.clone(),
            numerical_reference,
        });
    }
    let numerical_equality_right_nullity = right_nullities.iter().sum::<usize>();
    if numerical_equality_right_nullity != accepted.session().report().right_nullity {
        return Err(PlanarLinkageError::GaugeCertification(format!(
            "mapped right nullity {numerical_equality_right_nullity} does not match core {}",
            accepted.session().report().right_nullity
        )));
    }
    let gauge_dof = components.iter().map(|component| component.gauge_dof).sum();
    let internal_mobility = components
        .iter()
        .map(|component| component.internal_mobility)
        .sum();
    Ok(PlanarGaugeReport {
        numerical_equality_right_nullity,
        gauge_dof,
        internal_mobility,
        components,
    })
}

fn validate_runtime_candidate(
    runtime: &Linkage,
    geometry: &LinkageGeometry,
    config: SolverConfig,
) -> Result<(), PlanarLinkageError> {
    let maximum = runtime.domain_hard_residual_max(geometry, None)?;
    if maximum > config.normalized_residual_tolerance {
        return Err(PlanarLinkageError::InitialRejected(format!(
            "domain hard residual {maximum:e} exceeds {:e}",
            config.normalized_residual_tolerance
        )));
    }
    if let Some(violation) = runtime.first_branch_violation(geometry)? {
        return Err(PlanarLinkageError::InitialRejected(format!(
            "explicit branch check failed: {violation:?}"
        )));
    }
    Ok(())
}

fn project_geometry_to_runtime(
    runtime: &mut Linkage,
    geometry: &LinkageGeometry,
) -> Result<(), PlanarLinkageError> {
    for body in &geometry.bodies {
        runtime.set_body_pose(body.body_id, body.pose)?;
    }
    Ok(())
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

fn validate_source(
    source: &PlanarSource,
    bodies: &BTreeSet<PlanarBodyId>,
    points: &BTreeSet<PlanarFeatureId>,
    axes: &BTreeSet<PlanarFeatureId>,
    grounded: &mut BTreeSet<PlanarBodyId>,
) -> Result<(), PlanarLinkageError> {
    let point = |id| require_member(points, id, "point feature");
    let require_axis = |id| require_member(axes, id, "axis feature");
    match source.definition {
        PlanarSourceKind::PhysicalGround { body } => {
            require_member(bodies, body, "body")?;
            if !grounded.insert(body) {
                return invalid("source.physical_ground", "body is grounded more than once");
            }
        }
        PlanarSourceKind::Revolute { first, second }
        | PlanarSourceKind::Weld { first, second, .. } => {
            point(first)?;
            point(second)?;
        }
        PlanarSourceKind::Prismatic {
            first_anchor,
            first_axis,
            second_anchor,
            second_axis,
            ..
        } => {
            point(first_anchor)?;
            require_axis(first_axis)?;
            point(second_anchor)?;
            require_axis(second_axis)?;
        }
        PlanarSourceKind::AngularDriver {
            reference, driven, ..
        } => {
            require_member(bodies, reference, "body")?;
            require_member(bodies, driven, "body")?;
        }
        PlanarSourceKind::LinearDriver {
            origin,
            measured,
            guide_axis,
            ..
        }
        | PlanarSourceKind::DirectedDisplacementBranch {
            origin,
            measured,
            axis: guide_axis,
            ..
        } => {
            point(origin)?;
            point(measured)?;
            require_axis(guide_axis)?;
        }
        PlanarSourceKind::OrientationBranch {
            line_start,
            line_end,
            observed,
            ..
        } => {
            point(line_start)?;
            point(line_end)?;
            point(observed)?;
        }
    }
    Ok(())
}

fn lower_source(
    linkage: &mut Linkage,
    mappings: &PlanarLinkageRuntimeMap,
    source: &PlanarSource,
    target: Option<f64>,
) -> Result<PlanarRuntimeSource, PlanarLinkageError> {
    Ok(match source.definition {
        PlanarSourceKind::PhysicalGround { body } => {
            let body = mappings
                .runtime_body(body)
                .ok_or_else(|| unknown("body", body))?;
            linkage.set_ground_source_label(body, &source.label)?;
            PlanarRuntimeSource::Ground(body)
        }
        PlanarSourceKind::Revolute { first, second } => {
            PlanarRuntimeSource::Joint(linkage.add_revolute_joint(
                &source.label,
                point_feature(mappings, first)?,
                point_feature(mappings, second)?,
            )?)
        }
        PlanarSourceKind::Prismatic {
            first_anchor,
            first_axis,
            second_anchor,
            second_axis,
            axis_branch,
        } => PlanarRuntimeSource::Joint(linkage.add_prismatic_joint(
            &source.label,
            point_feature(mappings, first_anchor)?,
            axis_feature(mappings, first_axis)?,
            point_feature(mappings, second_anchor)?,
            axis_feature(mappings, second_axis)?,
            axis_branch,
        )?),
        PlanarSourceKind::Weld {
            first,
            second,
            relative_angle,
        } => PlanarRuntimeSource::Joint(linkage.add_weld_joint_with_angle(
            &source.label,
            point_feature(mappings, first)?,
            point_feature(mappings, second)?,
            relative_angle,
        )?),
        PlanarSourceKind::AngularDriver {
            reference,
            driven,
            max_continuation_step,
        } => PlanarRuntimeSource::Driver(
            linkage.add_angular_driver(
                &source.label,
                mappings
                    .runtime_body(reference)
                    .ok_or_else(|| unknown("body", reference))?,
                mappings
                    .runtime_body(driven)
                    .ok_or_else(|| unknown("body", driven))?,
                target.ok_or(PlanarLinkageError::IncompleteAcceptedState)?,
                max_continuation_step,
            )?,
        ),
        PlanarSourceKind::LinearDriver {
            origin,
            measured,
            guide_axis,
            max_continuation_step,
        } => PlanarRuntimeSource::Driver(linkage.add_linear_driver(
            &source.label,
            point_feature(mappings, origin)?,
            point_feature(mappings, measured)?,
            axis_feature(mappings, guide_axis)?,
            target.ok_or(PlanarLinkageError::IncompleteAcceptedState)?,
            max_continuation_step,
        )?),
        PlanarSourceKind::OrientationBranch {
            line_start,
            line_end,
            observed,
            sign,
        } => PlanarRuntimeSource::BranchMonitor(linkage.add_orientation_branch_monitor(
            point_feature(mappings, line_start)?,
            point_feature(mappings, line_end)?,
            point_feature(mappings, observed)?,
            sign,
        )?),
        PlanarSourceKind::DirectedDisplacementBranch {
            origin,
            measured,
            axis,
            sign,
        } => PlanarRuntimeSource::BranchMonitor(linkage.add_directed_displacement_branch_monitor(
            point_feature(mappings, origin)?,
            point_feature(mappings, measured)?,
            axis_feature(mappings, axis)?,
            sign,
        )?),
    })
}

fn persistent_joint(
    kind: JointKind,
    points: &HashMap<PointFeatureId, PlanarFeatureId>,
    axes: &HashMap<AxisFeatureId, PlanarFeatureId>,
) -> Result<PlanarSourceKind, PlanarLinkageError> {
    Ok(match kind {
        JointKind::Revolute { first, second } => PlanarSourceKind::Revolute {
            first: point_id(points, first)?,
            second: point_id(points, second)?,
        },
        JointKind::Prismatic {
            first_anchor,
            first_axis,
            second_anchor,
            second_axis,
            axis_branch,
        } => PlanarSourceKind::Prismatic {
            first_anchor: point_id(points, first_anchor)?,
            first_axis: axis_id(axes, first_axis)?,
            second_anchor: point_id(points, second_anchor)?,
            second_axis: axis_id(axes, second_axis)?,
            axis_branch,
        },
        JointKind::Weld {
            first,
            second,
            relative_angle,
        } => PlanarSourceKind::Weld {
            first: point_id(points, first)?,
            second: point_id(points, second)?,
            relative_angle,
        },
    })
}

fn persistent_driver(
    kind: DriverKind,
    max_continuation_step: f64,
    bodies: &HashMap<BodyId, PlanarBodyId>,
    points: &HashMap<PointFeatureId, PlanarFeatureId>,
    axes: &HashMap<AxisFeatureId, PlanarFeatureId>,
) -> Result<PlanarSourceKind, PlanarLinkageError> {
    Ok(match kind {
        DriverKind::Angular { reference, driven } => PlanarSourceKind::AngularDriver {
            reference: body_id(bodies, reference)?,
            driven: body_id(bodies, driven)?,
            max_continuation_step,
        },
        DriverKind::Linear {
            origin,
            measured,
            guide_axis,
        } => PlanarSourceKind::LinearDriver {
            origin: point_id(points, origin)?,
            measured: point_id(points, measured)?,
            guide_axis: axis_id(axes, guide_axis)?,
            max_continuation_step,
        },
    })
}

fn persistent_monitor(
    index: usize,
    monitor: BranchMonitor,
    points: &HashMap<PointFeatureId, PlanarFeatureId>,
    axes: &HashMap<AxisFeatureId, PlanarFeatureId>,
) -> Result<(String, PlanarSourceKind), PlanarLinkageError> {
    Ok(match monitor {
        BranchMonitor::Orientation {
            line_start,
            line_end,
            observed,
            sign,
        } => (
            format!("orientation branch monitor {}", index + 1),
            PlanarSourceKind::OrientationBranch {
                line_start: point_id(points, line_start)?,
                line_end: point_id(points, line_end)?,
                observed: point_id(points, observed)?,
                sign,
            },
        ),
        BranchMonitor::DirectedDisplacement {
            origin,
            measured,
            axis,
            sign,
        } => (
            format!("directed displacement branch monitor {}", index + 1),
            PlanarSourceKind::DirectedDisplacementBranch {
                origin: point_id(points, origin)?,
                measured: point_id(points, measured)?,
                axis: axis_id(axes, axis)?,
                sign,
            },
        ),
    })
}

fn point_feature(
    mappings: &PlanarLinkageRuntimeMap,
    id: PlanarFeatureId,
) -> Result<PointFeatureId, PlanarLinkageError> {
    match mappings.runtime_feature(id) {
        Some(PlanarRuntimeFeature::Point(runtime)) => Ok(runtime),
        Some(PlanarRuntimeFeature::Axis(_)) => Err(PlanarLinkageError::FeatureKindMismatch(id)),
        None => Err(unknown("feature", id)),
    }
}

fn axis_feature(
    mappings: &PlanarLinkageRuntimeMap,
    id: PlanarFeatureId,
) -> Result<AxisFeatureId, PlanarLinkageError> {
    match mappings.runtime_feature(id) {
        Some(PlanarRuntimeFeature::Axis(runtime)) => Ok(runtime),
        Some(PlanarRuntimeFeature::Point(_)) => Err(PlanarLinkageError::FeatureKindMismatch(id)),
        None => Err(unknown("feature", id)),
    }
}

fn body_id(
    mappings: &HashMap<BodyId, PlanarBodyId>,
    id: BodyId,
) -> Result<PlanarBodyId, PlanarLinkageError> {
    mappings
        .get(&id)
        .copied()
        .ok_or_else(|| PlanarLinkageError::InvalidField {
            field: "runtime body mapping",
            message: format!("missing {id:?}"),
        })
}

fn point_id(
    mappings: &HashMap<PointFeatureId, PlanarFeatureId>,
    id: PointFeatureId,
) -> Result<PlanarFeatureId, PlanarLinkageError> {
    mappings
        .get(&id)
        .copied()
        .ok_or_else(|| PlanarLinkageError::InvalidField {
            field: "runtime point mapping",
            message: format!("missing {id:?}"),
        })
}

fn axis_id(
    mappings: &HashMap<AxisFeatureId, PlanarFeatureId>,
    id: AxisFeatureId,
) -> Result<PlanarFeatureId, PlanarLinkageError> {
    mappings
        .get(&id)
        .copied()
        .ok_or_else(|| PlanarLinkageError::InvalidField {
            field: "runtime axis mapping",
            message: format!("missing {id:?}"),
        })
}

fn pose_from_ambient(ambient: [f64; 3], field: &'static str) -> Result<Pose2, PlanarLinkageError> {
    Pose2::from_ambient(ambient).map_err(|error| PlanarLinkageError::InvalidField {
        field,
        message: error.to_string(),
    })
}

fn validate_label(label: &str) -> Result<(), PlanarLinkageError> {
    if label.is_empty() || label.len() > MAX_LABEL_BYTES {
        invalid("label", "must be nonempty and at most 1024 bytes")
    } else {
        Ok(())
    }
}

fn require_member<T>(set: &BTreeSet<T>, id: T, kind: &'static str) -> Result<(), PlanarLinkageError>
where
    T: Copy + fmt::Display + Ord,
{
    if set.contains(&id) {
        Ok(())
    } else {
        Err(PlanarLinkageError::UnknownReference {
            kind,
            id: id.to_string(),
        })
    }
}

fn unknown<T: fmt::Display>(kind: &'static str, id: T) -> PlanarLinkageError {
    PlanarLinkageError::UnknownReference {
        kind,
        id: id.to_string(),
    }
}

fn invalid<T>(field: &'static str, message: impl Into<String>) -> Result<T, PlanarLinkageError> {
    Err(PlanarLinkageError::InvalidField {
        field,
        message: message.into(),
    })
}
