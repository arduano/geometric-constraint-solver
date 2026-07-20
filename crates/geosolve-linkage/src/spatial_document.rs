// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::str::FromStr;

use geosolve_core::SolverConfig;
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use super::{
    SpatialAssembly, SpatialAssemblyError, SpatialAssemblySession, SpatialAxisFeatureId,
    SpatialAxisParity, SpatialBodyId, SpatialBoundaryHysteresisState, SpatialBranchBoundary,
    SpatialCoordinateId, SpatialCoordinateKind, SpatialFrameAxis, SpatialFrameFeatureId,
    SpatialGaugePolicy, SpatialHingeTarget, SpatialModeMonitorId, SpatialModeMonitorKind,
    SpatialModeSign, SpatialPlanarTranslationAxis, SpatialPlaneFeatureId, SpatialPointFeatureId,
    SpatialSolveResult, SpatialSourceId, SpatialSourceKind,
};

pub const SPATIAL_ASSEMBLY_DOCUMENT_VERSION: u32 = 1;
const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_OBJECTS: usize = 100_000;
const MAX_LABEL_BYTES: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct HexId(u128);

impl fmt::Display for HexId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

impl FromStr for HexId {
    type Err = SpatialDocumentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(SpatialDocumentError::InvalidId(value.to_owned()));
        }
        u128::from_str_radix(value, 16)
            .map(Self)
            .map_err(|_| SpatialDocumentError::InvalidId(value.to_owned()))
    }
}

impl Serialize for HexId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for HexId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

macro_rules! persistent_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(HexId);

        impl $name {
            #[must_use]
            pub const fn from_u128(value: u128) -> Self {
                Self(HexId(value))
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
    SpatialDocumentId,
    "Persistent spatial-assembly document identity."
);
persistent_id!(
    SpatialPersistentId,
    "Persistent identity in one spatial document."
);

#[derive(Debug, Error)]
pub enum SpatialDocumentError {
    #[error("unsupported spatial document version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid spatial persistent ID {0:?}")]
    InvalidId(String),
    #[error("duplicate spatial persistent ID {0}")]
    DuplicateId(String),
    #[error("unknown persistent spatial {kind} reference {id}")]
    UnknownReference { kind: &'static str, id: String },
    #[error("persistent spatial reference {id} is not a {expected}")]
    WrongReferenceKind { id: String, expected: &'static str },
    #[error("spatial source order must contain every source exactly once")]
    InvalidSourceOrder,
    #[error("invalid spatial document field {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("spatial document JSON exceeds the size limit")]
    JsonTooLarge,
    #[error("spatial persistent ID space is exhausted")]
    IdExhausted,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Spatial(#[from] SpatialAssemblyError),
}

fn invalid(field: &'static str, message: impl Into<String>) -> SpatialDocumentError {
    SpatialDocumentError::InvalidField {
        field,
        message: message.into(),
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PoseRecord {
    ambient: [f64; 7],
}

impl PoseRecord {
    fn from_pose(pose: Pose3) -> Self {
        Self {
            ambient: pose.ambient(),
        }
    }

    fn pose(self) -> Result<Pose3, SpatialDocumentError> {
        Pose3::from_ambient(self.ambient)
            .map_err(|error| invalid("pose.ambient", error.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FrameRecord {
    origin: [f64; 3],
    x_axis: [f64; 3],
    y_axis: [f64; 3],
    z_axis: [f64; 3],
}

impl FrameRecord {
    fn from_frame(frame: Frame3) -> Self {
        Self {
            origin: frame.origin().coords.into(),
            x_axis: frame.x_axis().into(),
            y_axis: frame.y_axis().into(),
            z_axis: frame.z_axis().into(),
        }
    }

    fn frame(self) -> Result<Frame3, SpatialDocumentError> {
        Frame3::try_new(
            Point3::from(self.origin),
            Vector3::from(self.x_axis),
            Vector3::from(self.y_axis),
            Vector3::from(self.z_axis),
        )
        .map_err(|error| invalid("feature.local_frame", error.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum ParityRecord {
    Aligned,
    Opposed,
}

impl From<SpatialAxisParity> for ParityRecord {
    fn from(value: SpatialAxisParity) -> Self {
        match value {
            SpatialAxisParity::Aligned => Self::Aligned,
            SpatialAxisParity::Opposed => Self::Opposed,
        }
    }
}

impl From<ParityRecord> for SpatialAxisParity {
    fn from(value: ParityRecord) -> Self {
        match value {
            ParityRecord::Aligned => Self::Aligned,
            ParityRecord::Opposed => Self::Opposed,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum SignRecord {
    Positive,
    Negative,
}

impl From<SpatialModeSign> for SignRecord {
    fn from(value: SpatialModeSign) -> Self {
        match value {
            SpatialModeSign::Positive => Self::Positive,
            SpatialModeSign::Negative => Self::Negative,
        }
    }
}

impl From<SignRecord> for SpatialModeSign {
    fn from(value: SignRecord) -> Self {
        match value {
            SignRecord::Positive => Self::Positive,
            SignRecord::Negative => Self::Negative,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BodyRecord {
    id: SpatialPersistentId,
    label: String,
}

macro_rules! frame_feature_record {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        #[serde(deny_unknown_fields)]
        struct $name {
            id: SpatialPersistentId,
            label: String,
            body: SpatialPersistentId,
            local_frame: FrameRecord,
        }
    };
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PointRecord {
    id: SpatialPersistentId,
    label: String,
    body: SpatialPersistentId,
    local_point: [f64; 3],
}

frame_feature_record!(FrameFeatureRecord);
frame_feature_record!(AxisFeatureRecord);
frame_feature_record!(PlaneFeatureRecord);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SourceDefinition {
    PhysicalGround {
        body: SpatialPersistentId,
        target: PoseRecord,
    },
    BallJoint {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
    },
    FixedFrame {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
    },
    RevoluteJoint {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        parity: ParityRecord,
    },
    PrismaticJoint {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        parity: ParityRecord,
    },
    CylindricalJoint {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        parity: ParityRecord,
    },
    PlanarJoint {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        parity: ParityRecord,
    },
    UniversalJoint {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
    },
    PointDistanceMate {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        distance: f64,
    },
    AxisAngleMate {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        angle: f64,
    },
    AxisAlignmentMate {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        parity: ParityRecord,
    },
    FrameOffsetMate {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        offset: FrameRecord,
    },
    HingePositionDriver {
        coordinate: SpatialPersistentId,
    },
    TranslationPositionDriver {
        coordinate: SpatialPersistentId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceRecord {
    id: SpatialPersistentId,
    label: String,
    definition: SourceDefinition,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CoordinateDefinition {
    Hinge {
        parent: SpatialPersistentId,
        winding: i64,
    },
    AxialTranslation {
        parent: SpatialPersistentId,
    },
    PlanarTranslation {
        parent: SpatialPersistentId,
        axis: AxisRecord,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum AxisRecord {
    X,
    Y,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CoordinateRecord {
    id: SpatialPersistentId,
    label: String,
    definition: CoordinateDefinition,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MonitorDefinition {
    AxisParity {
        first: SpatialPersistentId,
        second: SpatialPersistentId,
        parity: ParityRecord,
    },
    HingeWinding {
        coordinate: SpatialPersistentId,
        winding: i64,
    },
    PlaneSide {
        plane: SpatialPersistentId,
        point: SpatialPersistentId,
        side: SignRecord,
    },
    SignedVolume {
        points: [SpatialPersistentId; 4],
        orientation: SignRecord,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MonitorRecord {
    id: SpatialPersistentId,
    label: String,
    definition: MonitorDefinition,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum GaugeRecord {
    LowestPersistentBody,
    ExplicitReferences { bodies: Vec<SpatialPersistentId> },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BodyStateRecord {
    body: SpatialPersistentId,
    pose: PoseRecord,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DriverTargetRecord {
    Hinge { principal_phase: f64, winding: i64 },
    Translation { target: f64 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DriverStateRecord {
    source: SpatialPersistentId,
    target: DriverTargetRecord,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum FrameAxisRecord {
    X,
    Y,
    Z,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum BoundaryRecord {
    FixedFrameDiagonal {
        source: SpatialPersistentId,
        axis: FrameAxisRecord,
    },
    FrameOffsetDiagonal {
        source: SpatialPersistentId,
        axis: FrameAxisRecord,
    },
    SourceAxisParity {
        source: SpatialPersistentId,
        parity: ParityRecord,
    },
    PrismaticClockRoot {
        source: SpatialPersistentId,
    },
    HingeDriverPositiveRoot {
        source: SpatialPersistentId,
        coordinate: SpatialPersistentId,
    },
    HingePrincipalCut {
        coordinate: SpatialPersistentId,
        winding: i64,
    },
    MonitorAxisParity {
        monitor: SpatialPersistentId,
        parity: ParityRecord,
    },
    MonitorPlaneSide {
        monitor: SpatialPersistentId,
        side: SignRecord,
    },
    MonitorSignedVolume {
        monitor: SpatialPersistentId,
        orientation: SignRecord,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HysteresisRecord {
    Clear,
    Near,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BoundaryStateRecord {
    boundary: BoundaryRecord,
    state: HysteresisRecord,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TopologyRecord {
    model_scale: f64,
    bodies: Vec<BodyRecord>,
    point_features: Vec<PointRecord>,
    frame_features: Vec<FrameFeatureRecord>,
    axis_features: Vec<AxisFeatureRecord>,
    plane_features: Vec<PlaneFeatureRecord>,
    sources: Vec<SourceRecord>,
    source_order: Vec<SpatialPersistentId>,
    coordinates: Vec<CoordinateRecord>,
    monitors: Vec<MonitorRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AcceptedStateRecord {
    revision: u64,
    bodies: Vec<BodyStateRecord>,
    drivers: Vec<DriverStateRecord>,
    boundaries: Vec<BoundaryStateRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialAssemblyDocument {
    version: u32,
    id: SpatialDocumentId,
    next_id: SpatialPersistentId,
    gauge_policy: GaugeRecord,
    topology: TopologyRecord,
    accepted_state: AcceptedStateRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialRuntimeFeature {
    Point(SpatialPointFeatureId),
    Frame(SpatialFrameFeatureId),
    Axis(SpatialAxisFeatureId),
    Plane(SpatialPlaneFeatureId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpatialAssemblyRuntimeMap {
    document: SpatialDocumentId,
    bodies: Vec<(SpatialPersistentId, SpatialBodyId)>,
    features: Vec<(SpatialPersistentId, SpatialRuntimeFeature)>,
    sources: Vec<(SpatialPersistentId, SpatialSourceId)>,
    coordinates: Vec<(SpatialPersistentId, SpatialCoordinateId)>,
    monitors: Vec<(SpatialPersistentId, SpatialModeMonitorId)>,
}

macro_rules! map_methods {
    ($forward:ident, $reverse:ident, $field:ident, $runtime:ty) => {
        #[must_use]
        pub fn $forward(&self, id: SpatialPersistentId) -> Option<$runtime> {
            self.$field
                .iter()
                .find_map(|(persistent, runtime)| (*persistent == id).then_some(*runtime))
        }
        #[must_use]
        pub fn $reverse(&self, id: $runtime) -> Option<SpatialPersistentId> {
            self.$field
                .iter()
                .find_map(|(persistent, runtime)| (*runtime == id).then_some(*persistent))
        }
    };
}

impl SpatialAssemblyRuntimeMap {
    #[must_use]
    pub const fn document_id(&self) -> SpatialDocumentId {
        self.document
    }
    map_methods!(runtime_body, persistent_body, bodies, SpatialBodyId);
    map_methods!(runtime_source, persistent_source, sources, SpatialSourceId);
    map_methods!(
        runtime_coordinate,
        persistent_coordinate,
        coordinates,
        SpatialCoordinateId
    );
    map_methods!(
        runtime_monitor,
        persistent_monitor,
        monitors,
        SpatialModeMonitorId
    );

    #[must_use]
    pub fn runtime_feature(&self, id: SpatialPersistentId) -> Option<SpatialRuntimeFeature> {
        self.features
            .iter()
            .find_map(|(persistent, runtime)| (*persistent == id).then_some(*runtime))
    }

    #[must_use]
    pub fn persistent_feature(&self, id: SpatialRuntimeFeature) -> Option<SpatialPersistentId> {
        self.features
            .iter()
            .find_map(|(persistent, runtime)| (*runtime == id).then_some(*persistent))
    }
}

#[derive(Debug)]
struct Allocator {
    next: u128,
}

impl Allocator {
    fn new(id: SpatialDocumentId) -> Result<Self, SpatialDocumentError> {
        if id.as_u128() == 0 {
            return Err(SpatialDocumentError::InvalidId(id.to_string()));
        }
        Ok(Self {
            next: id
                .as_u128()
                .checked_add(1)
                .ok_or(SpatialDocumentError::IdExhausted)?,
        })
    }
    fn allocate(&mut self) -> Result<SpatialPersistentId, SpatialDocumentError> {
        let value = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(SpatialDocumentError::IdExhausted)?;
        Ok(SpatialPersistentId::from_u128(value))
    }
}

impl SpatialAssemblyDocument {
    /// Captures only independently accepted spatial domain state.
    ///
    /// # Errors
    ///
    /// Rejects an invalid document identity, inconsistent accepted runtime IDs,
    /// invalid accepted geometry, or persistent ID exhaustion.
    #[allow(clippy::too_many_lines)]
    pub fn capture_accepted(
        id: SpatialDocumentId,
        session: &SpatialAssemblySession,
    ) -> Result<(Self, SpatialAssemblyRuntimeMap), SpatialDocumentError> {
        let mut allocator = Allocator::new(id)?;
        let mut map = SpatialAssemblyRuntimeMap {
            document: id,
            bodies: Vec::new(),
            features: Vec::new(),
            sources: Vec::new(),
            coordinates: Vec::new(),
            monitors: Vec::new(),
        };
        for body in &session.assembly.bodies {
            map.bodies.push((allocator.allocate()?, body.id));
        }
        for feature in &session.assembly.point_features {
            map.features.push((
                allocator.allocate()?,
                SpatialRuntimeFeature::Point(feature.id),
            ));
        }
        for feature in &session.assembly.frame_features {
            map.features.push((
                allocator.allocate()?,
                SpatialRuntimeFeature::Frame(feature.id),
            ));
        }
        for feature in &session.assembly.axis_features {
            map.features.push((
                allocator.allocate()?,
                SpatialRuntimeFeature::Axis(feature.id),
            ));
        }
        for feature in &session.assembly.plane_features {
            map.features.push((
                allocator.allocate()?,
                SpatialRuntimeFeature::Plane(feature.id),
            ));
        }
        for source in &session.assembly.sources {
            map.sources.push((allocator.allocate()?, source.id));
        }
        for coordinate in &session.assembly.coordinates {
            map.coordinates.push((allocator.allocate()?, coordinate.id));
        }
        for monitor in &session.assembly.mode_monitors {
            map.monitors.push((allocator.allocate()?, monitor.id));
        }

        let topology = TopologyRecord {
            model_scale: session.assembly.model_scale,
            bodies: session
                .assembly
                .bodies
                .iter()
                .map(|body| {
                    Ok(BodyRecord {
                        id: persistent_body(&map, body.id)?,
                        label: body.label.clone(),
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            point_features: session
                .assembly
                .point_features
                .iter()
                .map(|feature| {
                    Ok(PointRecord {
                        id: persistent_feature(&map, SpatialRuntimeFeature::Point(feature.id))?,
                        label: feature.label.clone(),
                        body: persistent_body(&map, feature.body)?,
                        local_point: feature.local_point.coords.into(),
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            frame_features: session
                .assembly
                .frame_features
                .iter()
                .map(|feature| {
                    Ok(FrameFeatureRecord {
                        id: persistent_feature(&map, SpatialRuntimeFeature::Frame(feature.id))?,
                        label: feature.label.clone(),
                        body: persistent_body(&map, feature.body)?,
                        local_frame: FrameRecord::from_frame(feature.local_frame),
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            axis_features: session
                .assembly
                .axis_features
                .iter()
                .map(|feature| {
                    Ok(AxisFeatureRecord {
                        id: persistent_feature(&map, SpatialRuntimeFeature::Axis(feature.id))?,
                        label: feature.label.clone(),
                        body: persistent_body(&map, feature.body)?,
                        local_frame: FrameRecord::from_frame(feature.local_frame),
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            plane_features: session
                .assembly
                .plane_features
                .iter()
                .map(|feature| {
                    Ok(PlaneFeatureRecord {
                        id: persistent_feature(&map, SpatialRuntimeFeature::Plane(feature.id))?,
                        label: feature.label.clone(),
                        body: persistent_body(&map, feature.body)?,
                        local_frame: FrameRecord::from_frame(feature.local_frame),
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            sources: session
                .assembly
                .sources
                .iter()
                .map(|source| {
                    Ok(SourceRecord {
                        id: persistent_source(&map, source.id)?,
                        label: source.label.clone(),
                        definition: capture_source(source.kind, &map)?,
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            source_order: session
                .assembly
                .sources
                .iter()
                .map(|source| persistent_source(&map, source.id))
                .collect::<Result<_, _>>()?,
            coordinates: session
                .assembly
                .coordinates
                .iter()
                .map(|coordinate| {
                    Ok(CoordinateRecord {
                        id: persistent_coordinate(&map, coordinate.id)?,
                        label: coordinate.label.clone(),
                        definition: capture_coordinate(coordinate.kind, &map)?,
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            monitors: session
                .assembly
                .mode_monitors
                .iter()
                .map(|monitor| {
                    Ok(MonitorRecord {
                        id: persistent_monitor(&map, monitor.id)?,
                        label: monitor.label.clone(),
                        definition: capture_monitor(monitor.kind, &map)?,
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
        };
        let accepted_state = AcceptedStateRecord {
            revision: session.revision(),
            bodies: session
                .assembly
                .bodies
                .iter()
                .map(|body| {
                    let pose = session
                        .accepted_result
                        .geometry
                        .body_pose(body.id)
                        .ok_or(SpatialAssemblyError::UnknownBody(body.id))?;
                    Ok(BodyStateRecord {
                        body: persistent_body(&map, body.id)?,
                        pose: PoseRecord::from_pose(pose),
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            drivers: session
                .assembly
                .sources
                .iter()
                .filter(|source| {
                    matches!(
                        source.kind,
                        SpatialSourceKind::HingePositionDriver { .. }
                            | SpatialSourceKind::TranslationPositionDriver { .. }
                    )
                })
                .map(|source| {
                    let target = match source.kind {
                        SpatialSourceKind::HingePositionDriver { target, .. } => {
                            DriverTargetRecord::Hinge {
                                principal_phase: target.principal_phase,
                                winding: target.winding,
                            }
                        }
                        SpatialSourceKind::TranslationPositionDriver { target, .. } => {
                            DriverTargetRecord::Translation { target }
                        }
                        _ => unreachable!("filtered to spatial driver sources"),
                    };
                    Ok(DriverStateRecord {
                        source: persistent_source(&map, source.id)?,
                        target,
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
            boundaries: session
                .accepted_result
                .branch_boundary_evaluations
                .iter()
                .map(|evaluation| {
                    Ok(BoundaryStateRecord {
                        boundary: capture_boundary(evaluation.boundary, &map)?,
                        state: match evaluation.hysteresis_state {
                            SpatialBoundaryHysteresisState::Clear => HysteresisRecord::Clear,
                            SpatialBoundaryHysteresisState::Near => HysteresisRecord::Near,
                        },
                    })
                })
                .collect::<Result<_, SpatialDocumentError>>()?,
        };
        let gauge_policy = match &session.assembly.gauge_policy {
            SpatialGaugePolicy::LowestPersistentBody => GaugeRecord::LowestPersistentBody,
            SpatialGaugePolicy::ExplicitReferences { bodies } => GaugeRecord::ExplicitReferences {
                bodies: bodies
                    .iter()
                    .map(|body| persistent_body(&map, *body))
                    .collect::<Result<_, _>>()?,
            },
        };
        let mut document = Self {
            version: SPATIAL_ASSEMBLY_DOCUMENT_VERSION,
            id,
            next_id: SpatialPersistentId::from_u128(allocator.next),
            gauge_policy,
            topology,
            accepted_state,
        };
        document.canonicalize();
        document.validate_basic()?;
        Ok((document, map))
    }

    #[must_use]
    pub const fn id(&self) -> SpatialDocumentId {
        self.id
    }
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.accepted_state.revision
    }

    /// Serializes this document in canonical persistent-ID order.
    ///
    /// # Errors
    ///
    /// Rejects invalid document structure, references, geometry, or JSON values.
    pub fn to_json(&self) -> Result<String, SpatialDocumentError> {
        let mut candidate = self.clone();
        candidate.canonicalize();
        candidate.validate_basic()?;
        candidate.lower()?;
        Ok(serde_json::to_string(&candidate)?)
    }

    /// Parses, canonicalizes, and structurally validates one spatial document.
    ///
    /// # Errors
    ///
    /// Rejects oversized, malformed, unsupported, non-finite, duplicate, stale,
    /// incomplete, or geometrically invalid documents.
    pub fn from_json(json: &str) -> Result<Self, SpatialDocumentError> {
        if json.len() > MAX_JSON_BYTES {
            return Err(SpatialDocumentError::JsonTooLarge);
        }
        let mut document: Self = serde_json::from_str(json)?;
        document.canonicalize();
        document.validate_basic()?;
        document.lower()?;
        Ok(document)
    }

    /// Deterministically lowers persistent IDs into fresh runtime keys.
    ///
    /// # Errors
    ///
    /// Rejects invalid references, geometry, targets, gauges, or assembly modes.
    #[allow(clippy::too_many_lines)]
    pub fn lower(
        &self,
    ) -> Result<(SpatialAssembly, SpatialAssemblyRuntimeMap), SpatialDocumentError> {
        self.validate_basic()?;
        let body_states = self
            .accepted_state
            .bodies
            .iter()
            .map(|state| (state.body, state.pose))
            .collect::<HashMap<_, _>>();
        let driver_states = self
            .accepted_state
            .drivers
            .iter()
            .map(|state| (state.source, state.target.clone()))
            .collect::<HashMap<_, _>>();
        let source_records = self
            .topology
            .sources
            .iter()
            .map(|source| (source.id, source))
            .collect::<HashMap<_, _>>();
        let coordinate_records = self
            .topology
            .coordinates
            .iter()
            .map(|coordinate| (coordinate.id, coordinate))
            .collect::<HashMap<_, _>>();
        let mut assembly = SpatialAssembly::new(self.topology.model_scale)?;
        let mut map = SpatialAssemblyRuntimeMap {
            document: self.id,
            bodies: Vec::new(),
            features: Vec::new(),
            sources: Vec::new(),
            coordinates: Vec::new(),
            monitors: Vec::new(),
        };

        for body in &self.topology.bodies {
            let pose = body_states
                .get(&body.id)
                .ok_or_else(|| unknown("body state", body.id))?
                .pose()?;
            let runtime = assembly.add_body(&body.label, pose)?;
            map.bodies.push((body.id, runtime));
        }
        for feature in &self.topology.point_features {
            let runtime = assembly.add_point_feature(
                &feature.label,
                require_runtime_body(&map, feature.body)?,
                Point3::from(feature.local_point),
            )?;
            map.features
                .push((feature.id, SpatialRuntimeFeature::Point(runtime)));
        }
        for feature in &self.topology.frame_features {
            let runtime = assembly.add_frame_feature(
                &feature.label,
                require_runtime_body(&map, feature.body)?,
                feature.local_frame.frame()?,
            )?;
            map.features
                .push((feature.id, SpatialRuntimeFeature::Frame(runtime)));
        }
        for feature in &self.topology.axis_features {
            let runtime = assembly.add_axis_feature(
                &feature.label,
                require_runtime_body(&map, feature.body)?,
                feature.local_frame.frame()?,
            )?;
            map.features
                .push((feature.id, SpatialRuntimeFeature::Axis(runtime)));
        }
        for feature in &self.topology.plane_features {
            let runtime = assembly.add_plane_feature(
                &feature.label,
                require_runtime_body(&map, feature.body)?,
                feature.local_frame.frame()?,
            )?;
            map.features
                .push((feature.id, SpatialRuntimeFeature::Plane(runtime)));
        }

        for source_id in &self.topology.source_order {
            let source = source_records
                .get(source_id)
                .ok_or_else(|| unknown("source", *source_id))?;
            if let Some(coordinate) = source_driver_coordinate(&source.definition) {
                ensure_coordinate(&mut assembly, &mut map, &coordinate_records, coordinate)?;
            }
            let ground_state = match &source.definition {
                SourceDefinition::PhysicalGround { body, .. } => body_states.get(body).copied(),
                _ => None,
            };
            let runtime = lower_source(
                &mut assembly,
                &map,
                source,
                driver_states.get(source_id).cloned(),
                ground_state,
            )?;
            map.sources.push((*source_id, runtime));
        }
        for coordinate in &self.topology.coordinates {
            ensure_coordinate(&mut assembly, &mut map, &coordinate_records, coordinate.id)?;
        }
        for monitor in &self.topology.monitors {
            let runtime = lower_monitor(&mut assembly, &map, monitor)?;
            map.monitors.push((monitor.id, runtime));
        }
        let mut boundaries = Vec::with_capacity(self.accepted_state.boundaries.len());
        for state in &self.accepted_state.boundaries {
            let boundary = lower_boundary(&state.boundary, &map)?;
            if boundaries.contains(&boundary) {
                return Err(invalid(
                    "accepted_state.boundaries",
                    "contains a duplicate boundary",
                ));
            }
            boundaries.push(boundary);
        }

        assembly.gauge_policy = match &self.gauge_policy {
            GaugeRecord::LowestPersistentBody => SpatialGaugePolicy::LowestPersistentBody,
            GaugeRecord::ExplicitReferences { bodies } => SpatialGaugePolicy::ExplicitReferences {
                bodies: bodies
                    .iter()
                    .map(|body| require_runtime_body(&map, *body))
                    .collect::<Result<_, _>>()?,
            },
        };
        assembly.revision = self.accepted_state.revision;
        assembly.validate_structure()?;
        Ok((assembly, map))
    }

    fn canonicalize(&mut self) {
        self.topology.bodies.sort_by_key(|record| record.id);
        self.topology.point_features.sort_by_key(|record| record.id);
        self.topology.frame_features.sort_by_key(|record| record.id);
        self.topology.axis_features.sort_by_key(|record| record.id);
        self.topology.plane_features.sort_by_key(|record| record.id);
        self.topology.sources.sort_by_key(|record| record.id);
        self.topology.coordinates.sort_by_key(|record| record.id);
        self.topology.monitors.sort_by_key(|record| record.id);
        self.accepted_state.bodies.sort_by_key(|record| record.body);
        self.accepted_state
            .drivers
            .sort_by_key(|record| record.source);
        self.accepted_state
            .boundaries
            .sort_by(|left, right| left.boundary.cmp(&right.boundary));
        if let GaugeRecord::ExplicitReferences { bodies } = &mut self.gauge_policy {
            bodies.sort();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn validate_basic(&self) -> Result<(), SpatialDocumentError> {
        if self.version != SPATIAL_ASSEMBLY_DOCUMENT_VERSION {
            return Err(SpatialDocumentError::UnsupportedVersion(self.version));
        }
        if self.id.as_u128() == 0 {
            return Err(SpatialDocumentError::InvalidId(self.id.to_string()));
        }
        if !self.topology.model_scale.is_finite() || self.topology.model_scale <= 0.0 {
            return Err(invalid(
                "topology.model_scale",
                "must be positive and finite",
            ));
        }
        let object_count = self.topology.bodies.len()
            + self.topology.point_features.len()
            + self.topology.frame_features.len()
            + self.topology.axis_features.len()
            + self.topology.plane_features.len()
            + self.topology.sources.len()
            + self.topology.coordinates.len()
            + self.topology.monitors.len();
        if object_count > MAX_OBJECTS {
            return Err(invalid("topology", "object limit exceeded"));
        }
        let mut ids = BTreeSet::new();
        let mut max_id = self.id.as_u128();
        ids.insert(self.id.as_u128());
        for id in self.all_object_ids() {
            if id.as_u128() == 0 {
                return Err(SpatialDocumentError::InvalidId(id.to_string()));
            }
            if !ids.insert(id.as_u128()) {
                return Err(SpatialDocumentError::DuplicateId(id.to_string()));
            }
            max_id = max_id.max(id.as_u128());
        }
        if self.next_id.as_u128() <= max_id {
            return Err(invalid("next_id", "must exceed every allocated ID"));
        }
        for label in self.all_labels() {
            if label.is_empty() || label.len() > MAX_LABEL_BYTES {
                return Err(invalid(
                    "label",
                    "must be nonempty and within the byte limit",
                ));
            }
        }
        let source_ids = self
            .topology
            .sources
            .iter()
            .map(|source| source.id)
            .collect::<BTreeSet<_>>();
        if self.topology.source_order.len() != source_ids.len()
            || self
                .topology
                .source_order
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                != source_ids
        {
            return Err(SpatialDocumentError::InvalidSourceOrder);
        }
        let body_ids = self
            .topology
            .bodies
            .iter()
            .map(|body| body.id)
            .collect::<BTreeSet<_>>();
        if self
            .accepted_state
            .bodies
            .iter()
            .map(|body| body.body)
            .collect::<BTreeSet<_>>()
            != body_ids
            || self.accepted_state.bodies.len() != body_ids.len()
        {
            return Err(invalid(
                "accepted_state.bodies",
                "must contain exactly one state per body",
            ));
        }
        let driver_ids = self
            .topology
            .sources
            .iter()
            .filter_map(|source| {
                matches!(
                    source.definition,
                    SourceDefinition::HingePositionDriver { .. }
                        | SourceDefinition::TranslationPositionDriver { .. }
                )
                .then_some(source.id)
            })
            .collect::<BTreeSet<_>>();
        if self
            .accepted_state
            .drivers
            .iter()
            .map(|driver| driver.source)
            .collect::<BTreeSet<_>>()
            != driver_ids
            || self.accepted_state.drivers.len() != driver_ids.len()
        {
            return Err(invalid(
                "accepted_state.drivers",
                "must contain exactly one state per driver",
            ));
        }
        Ok(())
    }

    fn all_object_ids(&self) -> impl Iterator<Item = SpatialPersistentId> + '_ {
        self.topology
            .bodies
            .iter()
            .map(|x| x.id)
            .chain(self.topology.point_features.iter().map(|x| x.id))
            .chain(self.topology.frame_features.iter().map(|x| x.id))
            .chain(self.topology.axis_features.iter().map(|x| x.id))
            .chain(self.topology.plane_features.iter().map(|x| x.id))
            .chain(self.topology.sources.iter().map(|x| x.id))
            .chain(self.topology.coordinates.iter().map(|x| x.id))
            .chain(self.topology.monitors.iter().map(|x| x.id))
    }

    fn all_labels(&self) -> impl Iterator<Item = &str> {
        self.topology
            .bodies
            .iter()
            .map(|x| x.label.as_str())
            .chain(
                self.topology
                    .point_features
                    .iter()
                    .map(|x| x.label.as_str()),
            )
            .chain(
                self.topology
                    .frame_features
                    .iter()
                    .map(|x| x.label.as_str()),
            )
            .chain(self.topology.axis_features.iter().map(|x| x.label.as_str()))
            .chain(
                self.topology
                    .plane_features
                    .iter()
                    .map(|x| x.label.as_str()),
            )
            .chain(self.topology.sources.iter().map(|x| x.label.as_str()))
            .chain(self.topology.coordinates.iter().map(|x| x.label.as_str()))
            .chain(self.topology.monitors.iter().map(|x| x.label.as_str()))
    }
}

fn unknown(kind: &'static str, id: SpatialPersistentId) -> SpatialDocumentError {
    SpatialDocumentError::UnknownReference {
        kind,
        id: id.to_string(),
    }
}

fn require_runtime_body(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialBodyId, SpatialDocumentError> {
    map.runtime_body(id).ok_or_else(|| unknown("body", id))
}

fn require_runtime_feature(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
    expected: &'static str,
) -> Result<SpatialRuntimeFeature, SpatialDocumentError> {
    let feature = map
        .runtime_feature(id)
        .ok_or_else(|| unknown("feature", id))?;
    let matches = matches!(
        (feature, expected),
        (SpatialRuntimeFeature::Point(_), "point")
            | (SpatialRuntimeFeature::Frame(_), "frame")
            | (SpatialRuntimeFeature::Axis(_), "axis")
            | (SpatialRuntimeFeature::Plane(_), "plane")
    );
    if matches {
        Ok(feature)
    } else {
        Err(SpatialDocumentError::WrongReferenceKind {
            id: id.to_string(),
            expected,
        })
    }
}

fn runtime_point(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialPointFeatureId, SpatialDocumentError> {
    let SpatialRuntimeFeature::Point(runtime) = require_runtime_feature(map, id, "point")? else {
        unreachable!()
    };
    Ok(runtime)
}
fn runtime_frame(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialFrameFeatureId, SpatialDocumentError> {
    let SpatialRuntimeFeature::Frame(runtime) = require_runtime_feature(map, id, "frame")? else {
        unreachable!()
    };
    Ok(runtime)
}
fn runtime_axis(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialAxisFeatureId, SpatialDocumentError> {
    let SpatialRuntimeFeature::Axis(runtime) = require_runtime_feature(map, id, "axis")? else {
        unreachable!()
    };
    Ok(runtime)
}
fn runtime_plane(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialPlaneFeatureId, SpatialDocumentError> {
    let SpatialRuntimeFeature::Plane(runtime) = require_runtime_feature(map, id, "plane")? else {
        unreachable!()
    };
    Ok(runtime)
}
fn runtime_source(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialSourceId, SpatialDocumentError> {
    map.runtime_source(id).ok_or_else(|| unknown("source", id))
}
fn runtime_coordinate(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialCoordinateId, SpatialDocumentError> {
    map.runtime_coordinate(id)
        .ok_or_else(|| unknown("coordinate", id))
}
fn runtime_monitor(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialPersistentId,
) -> Result<SpatialModeMonitorId, SpatialDocumentError> {
    map.runtime_monitor(id)
        .ok_or_else(|| unknown("monitor", id))
}

fn source_driver_coordinate(definition: &SourceDefinition) -> Option<SpatialPersistentId> {
    match definition {
        SourceDefinition::HingePositionDriver { coordinate }
        | SourceDefinition::TranslationPositionDriver { coordinate } => Some(*coordinate),
        _ => None,
    }
}

fn ensure_coordinate(
    assembly: &mut SpatialAssembly,
    map: &mut SpatialAssemblyRuntimeMap,
    records: &HashMap<SpatialPersistentId, &CoordinateRecord>,
    id: SpatialPersistentId,
) -> Result<SpatialCoordinateId, SpatialDocumentError> {
    if let Some(runtime) = map.runtime_coordinate(id) {
        return Ok(runtime);
    }
    let record = records.get(&id).ok_or_else(|| unknown("coordinate", id))?;
    let runtime = match record.definition {
        CoordinateDefinition::Hinge { parent, winding } => {
            assembly.add_hinge_coordinate(&record.label, runtime_source(map, parent)?, winding)?
        }
        CoordinateDefinition::AxialTranslation { parent } => assembly
            .add_axial_translation_coordinate(&record.label, runtime_source(map, parent)?)?,
        CoordinateDefinition::PlanarTranslation { parent, axis } => assembly
            .add_planar_translation_coordinate(
                &record.label,
                runtime_source(map, parent)?,
                match axis {
                    AxisRecord::X => SpatialPlanarTranslationAxis::X,
                    AxisRecord::Y => SpatialPlanarTranslationAxis::Y,
                },
            )?,
    };
    map.coordinates.push((id, runtime));
    Ok(runtime)
}

#[allow(clippy::too_many_lines)]
fn lower_source(
    assembly: &mut SpatialAssembly,
    map: &SpatialAssemblyRuntimeMap,
    source: &SourceRecord,
    driver: Option<DriverTargetRecord>,
    ground_state: Option<PoseRecord>,
) -> Result<SpatialSourceId, SpatialDocumentError> {
    let runtime = match source.definition {
        SourceDefinition::PhysicalGround { body, target } => {
            let state = ground_state.ok_or_else(|| unknown("body state", body))?;
            let target_pose = target.pose()?;
            let difference = target_pose
                .local_difference(&state.pose()?)
                .map_err(|error| invalid("topology.source.target", error.to_string()))?;
            if difference[..3]
                .iter()
                .any(|value| value.abs() / assembly.model_scale > 1.0e-9)
                || difference[3..].iter().any(|value| value.abs() > 1.0e-9)
            {
                return Err(invalid(
                    "topology.source.target",
                    "ground target must match the independently accepted body pose",
                ));
            }
            let runtime_body = require_runtime_body(map, body)?;
            let runtime = assembly.add_physical_ground(&source.label, runtime_body)?;
            assembly
                .sources
                .last_mut()
                .expect("physical ground was just appended")
                .kind = SpatialSourceKind::PhysicalGround {
                body: runtime_body,
                target_pose,
            };
            runtime
        }
        SourceDefinition::BallJoint { first, second } => assembly.add_ball_joint(
            &source.label,
            runtime_point(map, first)?,
            runtime_point(map, second)?,
        )?,
        SourceDefinition::FixedFrame { first, second } => assembly.add_fixed_frame(
            &source.label,
            runtime_frame(map, first)?,
            runtime_frame(map, second)?,
        )?,
        SourceDefinition::RevoluteJoint {
            first,
            second,
            parity,
        } => assembly.add_revolute_joint(
            &source.label,
            runtime_frame(map, first)?,
            runtime_frame(map, second)?,
            parity.into(),
        )?,
        SourceDefinition::PrismaticJoint {
            first,
            second,
            parity,
        } => assembly.add_prismatic_joint(
            &source.label,
            runtime_axis(map, first)?,
            runtime_axis(map, second)?,
            parity.into(),
        )?,
        SourceDefinition::CylindricalJoint {
            first,
            second,
            parity,
        } => assembly.add_cylindrical_joint(
            &source.label,
            runtime_axis(map, first)?,
            runtime_axis(map, second)?,
            parity.into(),
        )?,
        SourceDefinition::PlanarJoint {
            first,
            second,
            parity,
        } => assembly.add_planar_joint(
            &source.label,
            runtime_plane(map, first)?,
            runtime_plane(map, second)?,
            parity.into(),
        )?,
        SourceDefinition::UniversalJoint { first, second } => assembly.add_universal_joint(
            &source.label,
            runtime_axis(map, first)?,
            runtime_axis(map, second)?,
        )?,
        SourceDefinition::PointDistanceMate {
            first,
            second,
            distance,
        } => assembly.add_point_distance_mate(
            &source.label,
            runtime_point(map, first)?,
            runtime_point(map, second)?,
            distance,
        )?,
        SourceDefinition::AxisAngleMate {
            first,
            second,
            angle,
        } => assembly.add_axis_angle_mate(
            &source.label,
            runtime_axis(map, first)?,
            runtime_axis(map, second)?,
            angle,
        )?,
        SourceDefinition::AxisAlignmentMate {
            first,
            second,
            parity,
        } => assembly.add_axis_alignment_mate(
            &source.label,
            runtime_axis(map, first)?,
            runtime_axis(map, second)?,
            parity.into(),
        )?,
        SourceDefinition::FrameOffsetMate {
            first,
            second,
            offset,
        } => assembly.add_frame_offset_mate(
            &source.label,
            runtime_frame(map, first)?,
            runtime_frame(map, second)?,
            offset.frame()?,
        )?,
        SourceDefinition::HingePositionDriver { coordinate } => {
            let DriverTargetRecord::Hinge {
                principal_phase,
                winding,
            } = driver.ok_or_else(|| unknown("driver state", source.id))?
            else {
                return Err(invalid(
                    "accepted_state.driver.target",
                    "hinge driver requires a hinge target",
                ));
            };
            assembly.add_hinge_position_driver(
                &source.label,
                runtime_coordinate(map, coordinate)?,
                SpatialHingeTarget {
                    principal_phase,
                    winding,
                },
            )?
        }
        SourceDefinition::TranslationPositionDriver { coordinate } => {
            let DriverTargetRecord::Translation { target } =
                driver.ok_or_else(|| unknown("driver state", source.id))?
            else {
                return Err(invalid(
                    "accepted_state.driver.target",
                    "translation driver requires a translation target",
                ));
            };
            assembly.add_translation_position_driver(
                &source.label,
                runtime_coordinate(map, coordinate)?,
                target,
            )?
        }
    };
    Ok(runtime)
}

fn lower_monitor(
    assembly: &mut SpatialAssembly,
    map: &SpatialAssemblyRuntimeMap,
    monitor: &MonitorRecord,
) -> Result<SpatialModeMonitorId, SpatialDocumentError> {
    Ok(match monitor.definition {
        MonitorDefinition::AxisParity {
            first,
            second,
            parity,
        } => assembly.add_axis_parity_monitor(
            &monitor.label,
            runtime_axis(map, first)?,
            runtime_axis(map, second)?,
            parity.into(),
        )?,
        MonitorDefinition::HingeWinding {
            coordinate,
            winding,
        } => assembly.add_hinge_winding_monitor(
            &monitor.label,
            runtime_coordinate(map, coordinate)?,
            winding,
        )?,
        MonitorDefinition::PlaneSide { plane, point, side } => assembly.add_plane_side_monitor(
            &monitor.label,
            runtime_plane(map, plane)?,
            runtime_point(map, point)?,
            side.into(),
        )?,
        MonitorDefinition::SignedVolume {
            points,
            orientation,
        } => assembly.add_signed_volume_monitor(
            &monitor.label,
            [
                runtime_point(map, points[0])?,
                runtime_point(map, points[1])?,
                runtime_point(map, points[2])?,
                runtime_point(map, points[3])?,
            ],
            orientation.into(),
        )?,
    })
}

fn persistent_body(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialBodyId,
) -> Result<SpatialPersistentId, SpatialDocumentError> {
    map.persistent_body(id)
        .ok_or_else(|| SpatialDocumentError::UnknownReference {
            kind: "runtime body",
            id: id.to_string(),
        })
}
fn persistent_feature(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialRuntimeFeature,
) -> Result<SpatialPersistentId, SpatialDocumentError> {
    map.persistent_feature(id)
        .ok_or_else(|| SpatialDocumentError::UnknownReference {
            kind: "runtime feature",
            id: format!("{id:?}"),
        })
}
fn persistent_source(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialSourceId,
) -> Result<SpatialPersistentId, SpatialDocumentError> {
    map.persistent_source(id)
        .ok_or_else(|| SpatialDocumentError::UnknownReference {
            kind: "runtime source",
            id: id.to_string(),
        })
}
fn persistent_coordinate(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialCoordinateId,
) -> Result<SpatialPersistentId, SpatialDocumentError> {
    map.persistent_coordinate(id)
        .ok_or_else(|| SpatialDocumentError::UnknownReference {
            kind: "runtime coordinate",
            id: id.to_string(),
        })
}
fn persistent_monitor(
    map: &SpatialAssemblyRuntimeMap,
    id: SpatialModeMonitorId,
) -> Result<SpatialPersistentId, SpatialDocumentError> {
    map.persistent_monitor(id)
        .ok_or_else(|| SpatialDocumentError::UnknownReference {
            kind: "runtime monitor",
            id: id.to_string(),
        })
}

#[allow(clippy::too_many_lines)]
fn capture_source(
    kind: SpatialSourceKind,
    map: &SpatialAssemblyRuntimeMap,
) -> Result<SourceDefinition, SpatialDocumentError> {
    Ok(match kind {
        SpatialSourceKind::PhysicalGround { body, target_pose } => {
            SourceDefinition::PhysicalGround {
                body: persistent_body(map, body)?,
                target: PoseRecord::from_pose(target_pose),
            }
        }
        SpatialSourceKind::BallJoint { first, second } => SourceDefinition::BallJoint {
            first: persistent_feature(map, SpatialRuntimeFeature::Point(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Point(second))?,
        },
        SpatialSourceKind::FixedFrame { first, second } => SourceDefinition::FixedFrame {
            first: persistent_feature(map, SpatialRuntimeFeature::Frame(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Frame(second))?,
        },
        SpatialSourceKind::RevoluteJoint {
            first,
            second,
            parity,
        } => SourceDefinition::RevoluteJoint {
            first: persistent_feature(map, SpatialRuntimeFeature::Frame(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Frame(second))?,
            parity: parity.into(),
        },
        SpatialSourceKind::PrismaticJoint {
            first,
            second,
            parity,
        } => SourceDefinition::PrismaticJoint {
            first: persistent_feature(map, SpatialRuntimeFeature::Axis(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Axis(second))?,
            parity: parity.into(),
        },
        SpatialSourceKind::CylindricalJoint {
            first,
            second,
            parity,
        } => SourceDefinition::CylindricalJoint {
            first: persistent_feature(map, SpatialRuntimeFeature::Axis(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Axis(second))?,
            parity: parity.into(),
        },
        SpatialSourceKind::PlanarJoint {
            first,
            second,
            parity,
        } => SourceDefinition::PlanarJoint {
            first: persistent_feature(map, SpatialRuntimeFeature::Plane(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Plane(second))?,
            parity: parity.into(),
        },
        SpatialSourceKind::UniversalJoint { first, second } => SourceDefinition::UniversalJoint {
            first: persistent_feature(map, SpatialRuntimeFeature::Axis(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Axis(second))?,
        },
        SpatialSourceKind::PointDistanceMate {
            first,
            second,
            distance,
        } => SourceDefinition::PointDistanceMate {
            first: persistent_feature(map, SpatialRuntimeFeature::Point(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Point(second))?,
            distance,
        },
        SpatialSourceKind::AxisAngleMate {
            first,
            second,
            angle,
        } => SourceDefinition::AxisAngleMate {
            first: persistent_feature(map, SpatialRuntimeFeature::Axis(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Axis(second))?,
            angle,
        },
        SpatialSourceKind::AxisAlignmentMate {
            first,
            second,
            parity,
        } => SourceDefinition::AxisAlignmentMate {
            first: persistent_feature(map, SpatialRuntimeFeature::Axis(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Axis(second))?,
            parity: parity.into(),
        },
        SpatialSourceKind::FrameOffsetMate {
            first,
            second,
            offset,
        } => SourceDefinition::FrameOffsetMate {
            first: persistent_feature(map, SpatialRuntimeFeature::Frame(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Frame(second))?,
            offset: FrameRecord::from_frame(offset),
        },
        SpatialSourceKind::HingePositionDriver { coordinate, .. } => {
            SourceDefinition::HingePositionDriver {
                coordinate: persistent_coordinate(map, coordinate)?,
            }
        }
        SpatialSourceKind::TranslationPositionDriver { coordinate, .. } => {
            SourceDefinition::TranslationPositionDriver {
                coordinate: persistent_coordinate(map, coordinate)?,
            }
        }
    })
}

fn capture_coordinate(
    kind: SpatialCoordinateKind,
    map: &SpatialAssemblyRuntimeMap,
) -> Result<CoordinateDefinition, SpatialDocumentError> {
    Ok(match kind {
        SpatialCoordinateKind::Hinge { parent, winding } => CoordinateDefinition::Hinge {
            parent: persistent_source(map, parent)?,
            winding,
        },
        SpatialCoordinateKind::AxialTranslation { parent } => {
            CoordinateDefinition::AxialTranslation {
                parent: persistent_source(map, parent)?,
            }
        }
        SpatialCoordinateKind::PlanarTranslation { parent, axis } => {
            CoordinateDefinition::PlanarTranslation {
                parent: persistent_source(map, parent)?,
                axis: match axis {
                    SpatialPlanarTranslationAxis::X => AxisRecord::X,
                    SpatialPlanarTranslationAxis::Y => AxisRecord::Y,
                },
            }
        }
    })
}

fn capture_monitor(
    kind: SpatialModeMonitorKind,
    map: &SpatialAssemblyRuntimeMap,
) -> Result<MonitorDefinition, SpatialDocumentError> {
    Ok(match kind {
        SpatialModeMonitorKind::AxisParity {
            first,
            second,
            parity,
        } => MonitorDefinition::AxisParity {
            first: persistent_feature(map, SpatialRuntimeFeature::Axis(first))?,
            second: persistent_feature(map, SpatialRuntimeFeature::Axis(second))?,
            parity: parity.into(),
        },
        SpatialModeMonitorKind::PlaneSide { point, plane, side } => MonitorDefinition::PlaneSide {
            point: persistent_feature(map, SpatialRuntimeFeature::Point(point))?,
            plane: persistent_feature(map, SpatialRuntimeFeature::Plane(plane))?,
            side: side.into(),
        },
        SpatialModeMonitorKind::SignedVolume {
            points,
            orientation,
        } => MonitorDefinition::SignedVolume {
            points: points
                .map(|point| persistent_feature(map, SpatialRuntimeFeature::Point(point)))
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .expect("four persistent point IDs"),
            orientation: orientation.into(),
        },
        SpatialModeMonitorKind::HingeWinding {
            coordinate,
            winding,
        } => MonitorDefinition::HingeWinding {
            coordinate: persistent_coordinate(map, coordinate)?,
            winding,
        },
    })
}

fn capture_boundary(
    boundary: SpatialBranchBoundary,
    map: &SpatialAssemblyRuntimeMap,
) -> Result<BoundaryRecord, SpatialDocumentError> {
    Ok(match boundary {
        SpatialBranchBoundary::FixedFrameDiagonal { source, axis } => {
            BoundaryRecord::FixedFrameDiagonal {
                source: persistent_source(map, source)?,
                axis: frame_axis_record(axis),
            }
        }
        SpatialBranchBoundary::FrameOffsetDiagonal { source, axis } => {
            BoundaryRecord::FrameOffsetDiagonal {
                source: persistent_source(map, source)?,
                axis: frame_axis_record(axis),
            }
        }
        SpatialBranchBoundary::SourceAxisParity { source, parity } => {
            BoundaryRecord::SourceAxisParity {
                source: persistent_source(map, source)?,
                parity: parity.into(),
            }
        }
        SpatialBranchBoundary::PrismaticClockRoot { source } => {
            BoundaryRecord::PrismaticClockRoot {
                source: persistent_source(map, source)?,
            }
        }
        SpatialBranchBoundary::HingeDriverPositiveRoot { source, coordinate } => {
            BoundaryRecord::HingeDriverPositiveRoot {
                source: persistent_source(map, source)?,
                coordinate: persistent_coordinate(map, coordinate)?,
            }
        }
        SpatialBranchBoundary::HingePrincipalCut {
            coordinate,
            winding,
        } => BoundaryRecord::HingePrincipalCut {
            coordinate: persistent_coordinate(map, coordinate)?,
            winding,
        },
        SpatialBranchBoundary::MonitorAxisParity { monitor, parity } => {
            BoundaryRecord::MonitorAxisParity {
                monitor: persistent_monitor(map, monitor)?,
                parity: parity.into(),
            }
        }
        SpatialBranchBoundary::MonitorPlaneSide { monitor, side } => {
            BoundaryRecord::MonitorPlaneSide {
                monitor: persistent_monitor(map, monitor)?,
                side: side.into(),
            }
        }
        SpatialBranchBoundary::MonitorSignedVolume {
            monitor,
            orientation,
        } => BoundaryRecord::MonitorSignedVolume {
            monitor: persistent_monitor(map, monitor)?,
            orientation: orientation.into(),
        },
    })
}

const fn frame_axis_record(axis: SpatialFrameAxis) -> FrameAxisRecord {
    match axis {
        SpatialFrameAxis::X => FrameAxisRecord::X,
        SpatialFrameAxis::Y => FrameAxisRecord::Y,
        SpatialFrameAxis::Z => FrameAxisRecord::Z,
    }
}

fn runtime_frame_axis(axis: FrameAxisRecord) -> SpatialFrameAxis {
    match axis {
        FrameAxisRecord::X => SpatialFrameAxis::X,
        FrameAxisRecord::Y => SpatialFrameAxis::Y,
        FrameAxisRecord::Z => SpatialFrameAxis::Z,
    }
}

fn lower_boundary(
    boundary: &BoundaryRecord,
    map: &SpatialAssemblyRuntimeMap,
) -> Result<SpatialBranchBoundary, SpatialDocumentError> {
    Ok(match *boundary {
        BoundaryRecord::FixedFrameDiagonal { source, axis } => {
            SpatialBranchBoundary::FixedFrameDiagonal {
                source: runtime_source(map, source)?,
                axis: runtime_frame_axis(axis),
            }
        }
        BoundaryRecord::FrameOffsetDiagonal { source, axis } => {
            SpatialBranchBoundary::FrameOffsetDiagonal {
                source: runtime_source(map, source)?,
                axis: runtime_frame_axis(axis),
            }
        }
        BoundaryRecord::SourceAxisParity { source, parity } => {
            SpatialBranchBoundary::SourceAxisParity {
                source: runtime_source(map, source)?,
                parity: parity.into(),
            }
        }
        BoundaryRecord::PrismaticClockRoot { source } => {
            SpatialBranchBoundary::PrismaticClockRoot {
                source: runtime_source(map, source)?,
            }
        }
        BoundaryRecord::HingeDriverPositiveRoot { source, coordinate } => {
            SpatialBranchBoundary::HingeDriverPositiveRoot {
                source: runtime_source(map, source)?,
                coordinate: runtime_coordinate(map, coordinate)?,
            }
        }
        BoundaryRecord::HingePrincipalCut {
            coordinate,
            winding,
        } => SpatialBranchBoundary::HingePrincipalCut {
            coordinate: runtime_coordinate(map, coordinate)?,
            winding,
        },
        BoundaryRecord::MonitorAxisParity { monitor, parity } => {
            SpatialBranchBoundary::MonitorAxisParity {
                monitor: runtime_monitor(map, monitor)?,
                parity: parity.into(),
            }
        }
        BoundaryRecord::MonitorPlaneSide { monitor, side } => {
            SpatialBranchBoundary::MonitorPlaneSide {
                monitor: runtime_monitor(map, monitor)?,
                side: side.into(),
            }
        }
        BoundaryRecord::MonitorSignedVolume {
            monitor,
            orientation,
        } => SpatialBranchBoundary::MonitorSignedVolume {
            monitor: runtime_monitor(map, monitor)?,
            orientation: orientation.into(),
        },
    })
}

fn restore_boundary_state(
    document: &SpatialAssemblyDocument,
    map: &SpatialAssemblyRuntimeMap,
    session: &mut SpatialAssemblySession,
) -> Result<(), SpatialDocumentError> {
    let mut persisted = Vec::with_capacity(document.accepted_state.boundaries.len());
    for state in &document.accepted_state.boundaries {
        let boundary = lower_boundary(&state.boundary, map)?;
        if persisted.iter().any(|(existing, _)| *existing == boundary) {
            return Err(invalid(
                "accepted_state.boundaries",
                "contains a duplicate boundary",
            ));
        }
        persisted.push((boundary, state.state));
    }
    if persisted.len() != session.accepted_result.branch_boundary_evaluations.len()
        || session
            .accepted_result
            .branch_boundary_evaluations
            .iter()
            .any(|evaluation| {
                !persisted
                    .iter()
                    .any(|(boundary, _)| *boundary == evaluation.boundary)
            })
    {
        return Err(invalid(
            "accepted_state.boundaries",
            "must contain exactly one latch for every boundary implied by topology",
        ));
    }
    for evaluation in &mut session.accepted_result.branch_boundary_evaluations {
        let (_, state) = persisted
            .iter()
            .find(|(boundary, _)| *boundary == evaluation.boundary)
            .expect("complete boundary set checked above");
        evaluation.hysteresis_state = match state {
            HysteresisRecord::Clear => SpatialBoundaryHysteresisState::Clear,
            HysteresisRecord::Near => SpatialBoundaryHysteresisState::Near,
        };
    }
    Ok(())
}

/// Accepted persistent spatial document plus its validated runtime session.
#[derive(Clone, Debug)]
pub struct SpatialAssemblyDocumentSession {
    document: SpatialAssemblyDocument,
    runtime_map: SpatialAssemblyRuntimeMap,
    session: SpatialAssemblySession,
    config: SolverConfig,
}

impl SpatialAssemblyDocumentSession {
    /// Lowers, solves, independently validates, and retains a persistent document.
    ///
    /// # Errors
    ///
    /// Rejects invalid persistence, geometry, topology, modes, gauges, or solve results.
    pub fn new(
        mut document: SpatialAssemblyDocument,
        config: SolverConfig,
    ) -> Result<Self, SpatialDocumentError> {
        document.canonicalize();
        document.validate_basic()?;
        let (assembly, runtime_map) = document.lower()?;
        let mut session = SpatialAssemblySession::new(assembly, config)?;
        restore_boundary_state(&document, &runtime_map, &mut session)?;
        Ok(Self {
            document,
            runtime_map,
            session,
            config,
        })
    }

    /// Captures an already accepted spatial session without re-solving it.
    ///
    /// # Errors
    ///
    /// Rejects an invalid document identity or persistent ID exhaustion.
    pub fn from_accepted_session(
        id: SpatialDocumentId,
        session: &SpatialAssemblySession,
    ) -> Result<Self, SpatialDocumentError> {
        let (document, runtime_map) = SpatialAssemblyDocument::capture_accepted(id, session)?;
        Ok(Self {
            document,
            runtime_map,
            session: session.clone(),
            config: session.config,
        })
    }

    /// Parses and validates one complete document session.
    ///
    /// # Errors
    ///
    /// Returns the corresponding parse, persistence, or spatial solve error.
    pub fn from_json(json: &str, config: SolverConfig) -> Result<Self, SpatialDocumentError> {
        Self::new(SpatialAssemblyDocument::from_json(json)?, config)
    }

    /// Atomically replaces this session from JSON after complete validation.
    ///
    /// # Errors
    ///
    /// Retains every accepted view if parsing, lowering, solving, or validation fails.
    pub fn replace_json(&mut self, json: &str) -> Result<(), SpatialDocumentError> {
        let replacement = Self::from_json(json, self.config)?;
        *self = replacement;
        Ok(())
    }

    #[must_use]
    pub const fn document(&self) -> &SpatialAssemblyDocument {
        &self.document
    }
    #[must_use]
    pub const fn runtime_map(&self) -> &SpatialAssemblyRuntimeMap {
        &self.runtime_map
    }
    #[must_use]
    pub const fn session(&self) -> &SpatialAssemblySession {
        &self.session
    }
    #[must_use]
    pub fn accepted_result(&self) -> &SpatialSolveResult {
        self.session.accepted_result()
    }

    /// Serializes the retained accepted document.
    ///
    /// # Errors
    ///
    /// Returns a structural validation or JSON serialization error.
    pub fn to_json(&self) -> Result<String, SpatialDocumentError> {
        self.document.to_json()
    }
}
