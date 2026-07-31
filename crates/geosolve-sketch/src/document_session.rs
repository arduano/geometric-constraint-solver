use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use geosolve_core::{
    HardValidity, OperationCheckpoint, OperationControl, OperationController, OperationOutcome,
    OperationReport, OperationWorkCounter, SolveTermination, SolverConfig,
};
use geosolve_geometry::Point2;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::compiler::PreviousStateReference;
use crate::document::{
    ActivationDigest, ContactBranchEdit, ContactDefinition, ContactId, ContactStateEdit,
    CurveCurveFilletIds, CurveCurveFilletRequest, CurveDefinition, CurveId, CurveSpan,
    DesignPointId, DesignScalarId, DocumentAngleOrientation, DocumentArcSweep,
    DocumentBSplineInsertion, DocumentBSplineSpanDirection, DocumentCircleTangencyMode,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCurveNormalSide,
    DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode, DocumentElementId,
    DocumentError, DocumentExternalBindingId, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentHyperbolaBranch, DocumentMirroredBSplineInsertion,
    DocumentNurbsInsertion, DocumentObjectId, DocumentParameterId, DocumentParameterKind,
    DocumentParameterTarget, DocumentSourceId, ExternalFeatureKindV1, ExternalTopologyDigest,
    GeometryRole, HostConfigurationActivation, LineLineFilletIds, LineLineFilletRequest,
    MirroredCurveIds, PersistentId, RectangleIds, ScalarDomain, ScalarUnit, SketchDocument,
};
use crate::document_lowering::{
    DocumentRuntimeMap, ResolvedDocumentParameters, ResolvedParameterBinding, RuntimeSource,
};
use crate::{
    DocumentMeasurementProvenance, DocumentScalarUnit, SketchSession, SketchSessionError,
    SketchSolveRequest, SketchSolveResult, SketchSource, SolveRejection,
};

/// Unstable pre-M62 external snapshot wire version.
pub const EXTERNAL_SNAPSHOT_SET_VERSION_V1: u32 = 1;
/// Defensive bound for one immutable external snapshot set.
pub const MAX_EXTERNAL_SNAPSHOT_ENTRIES: usize = crate::MAX_EXTERNAL_BINDINGS;
/// Defensive M43 resource-evidence bounds.
pub const MAX_EXTERNAL_SNAPSHOT_POINTS: u32 = 2;
pub const MAX_EXTERNAL_SNAPSHOT_CONTROLS: u32 = 2;
pub const MAX_EXTERNAL_SNAPSHOT_SPANS: u32 = 1;

/// Canonical digest of host-provided bytes for one external feature.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ExternalSnapshotDigest([u8; 32]);

impl ExternalSnapshotDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity of a complete immutable external snapshot set.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ExternalSnapshotSetDigest([u8; 32]);

impl ExternalSnapshotSetDigest {
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Bounded resource evidence carried by every snapshot feature.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalSnapshotResourcesV1 {
    pub point_count: u32,
    pub control_count: u32,
    pub span_count: u32,
}

/// Closed directed orientation contract for an external line span.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalLineOrientationV1 {
    StartToEnd,
}

/// Closed finite external geometry language for M43.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExternalSnapshotFeatureV1 {
    Point {
        position: [f64; 2],
        scale: f64,
        resources: ExternalSnapshotResourcesV1,
    },
    LineSegment {
        start: [f64; 2],
        end: [f64; 2],
        domain: [f64; 2],
        orientation: ExternalLineOrientationV1,
        scale: f64,
        topology_digest: ExternalTopologyDigest,
        resources: ExternalSnapshotResourcesV1,
    },
}

impl ExternalSnapshotFeatureV1 {
    #[must_use]
    pub const fn kind(&self) -> ExternalFeatureKindV1 {
        match self {
            Self::Point { .. } => ExternalFeatureKindV1::Point,
            Self::LineSegment { .. } => ExternalFeatureKindV1::LineSegment,
        }
    }
}

/// One binding-keyed entry in an immutable external snapshot set.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalSnapshotEntry {
    pub binding: DocumentExternalBindingId,
    pub source_revision: u64,
    pub source_digest: ExternalSnapshotDigest,
    pub feature: ExternalSnapshotFeatureV1,
}

/// Typed rejection of malformed or stale external input evidence.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExternalSnapshotInputError {
    #[error("unsupported external snapshot set version {actual}; expected 1")]
    UnsupportedVersion { actual: u32 },
    #[error("nonempty external snapshot sets require a positive revision")]
    InvalidSetRevision,
    #[error("external snapshot entry {binding} requires a positive source revision")]
    InvalidSourceRevision { binding: DocumentExternalBindingId },
    #[error("duplicate external snapshot binding {binding}")]
    DuplicateBinding { binding: DocumentExternalBindingId },
    #[error("external snapshot set has {actual} entries; limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
    #[error("invalid external snapshot feature for binding {binding}: {reason}")]
    InvalidFeature {
        binding: DocumentExternalBindingId,
        reason: &'static str,
    },
    #[error("claimed external snapshot set digest does not match canonical bytes")]
    DigestMismatch,
    #[error("external snapshot entry names unknown binding {binding}")]
    UnknownBinding { binding: DocumentExternalBindingId },
    #[error("required external snapshot binding {binding} is missing")]
    MissingBinding { binding: DocumentExternalBindingId },
    #[error("external snapshot binding {binding} has kind {actual:?}; expected {expected:?}")]
    WrongKind {
        binding: DocumentExternalBindingId,
        expected: ExternalFeatureKindV1,
        actual: ExternalFeatureKindV1,
    },
    #[error("external line topology for binding {binding} does not match its retained declaration")]
    TopologyMismatch { binding: DocumentExternalBindingId },
    #[error("invalid external snapshot JSON: {0}")]
    Json(String),
}

/// Separately versioned immutable external snapshot envelope (unstable until M62).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalSnapshotSetV1 {
    version: u32,
    revision: u64,
    digest: ExternalSnapshotSetDigest,
    entries: Vec<ExternalSnapshotEntry>,
}

/// M43 convenience name for the current closed external snapshot envelope.
pub type ExternalSnapshotSet = ExternalSnapshotSetV1;

impl Default for ExternalSnapshotSetV1 {
    fn default() -> Self {
        let entries = Vec::new();
        Self {
            version: EXTERNAL_SNAPSHOT_SET_VERSION_V1,
            revision: 0,
            digest: external_snapshot_set_digest(0, &entries),
            entries,
        }
    }
}

impl ExternalSnapshotSetV1 {
    /// Constructs and canonically stamps one immutable external snapshot set.
    ///
    /// # Errors
    ///
    /// Rejects invalid revisions, duplicate bindings, malformed features, or resources
    /// above the fixed M43 limits.
    pub fn new(
        revision: u64,
        mut entries: Vec<ExternalSnapshotEntry>,
    ) -> Result<Self, ExternalSnapshotInputError> {
        validate_external_snapshot_entries(revision, &mut entries)?;
        Ok(Self {
            version: EXTERNAL_SNAPSHOT_SET_VERSION_V1,
            revision,
            digest: external_snapshot_set_digest(revision, &entries),
            entries,
        })
    }

    /// Reconstructs a set only when its version and canonical digest are exact.
    ///
    /// # Errors
    ///
    /// Rejects unsupported versions, invalid entries, or a digest mismatch.
    pub fn from_digest(
        version: u32,
        revision: u64,
        digest: ExternalSnapshotSetDigest,
        mut entries: Vec<ExternalSnapshotEntry>,
    ) -> Result<Self, ExternalSnapshotInputError> {
        if version != EXTERNAL_SNAPSHOT_SET_VERSION_V1 {
            return Err(ExternalSnapshotInputError::UnsupportedVersion { actual: version });
        }
        validate_external_snapshot_entries(revision, &mut entries)?;
        if external_snapshot_set_digest(revision, &entries) != digest {
            return Err(ExternalSnapshotInputError::DigestMismatch);
        }
        Ok(Self {
            version,
            revision,
            digest,
            entries,
        })
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn digest(&self) -> ExternalSnapshotSetDigest {
        self.digest
    }

    #[must_use]
    pub fn entries(&self) -> &[ExternalSnapshotEntry] {
        &self.entries
    }

    /// Encodes the independently revalidated set as canonical JSON.
    ///
    /// # Errors
    ///
    /// Rejects internally inconsistent evidence or JSON serialization failure.
    pub fn to_canonical_json(&self) -> Result<String, ExternalSnapshotInputError> {
        Self::from_digest(
            self.version,
            self.revision,
            self.digest,
            self.entries.clone(),
        )?;
        serde_json::to_string(self)
            .map_err(|error| ExternalSnapshotInputError::Json(error.to_string()))
    }

    /// Decodes strict JSON and independently validates its claimed digest.
    ///
    /// # Errors
    ///
    /// Rejects malformed JSON, unknown fields or variants, invalid features, and
    /// inconsistent version/digest evidence.
    pub fn from_json(json: &str) -> Result<Self, ExternalSnapshotInputError> {
        let decoded: Self = serde_json::from_str(json)
            .map_err(|error| ExternalSnapshotInputError::Json(error.to_string()))?;
        Self::from_digest(
            decoded.version,
            decoded.revision,
            decoded.digest,
            decoded.entries,
        )
    }
}

fn validate_external_snapshot_entries(
    revision: u64,
    entries: &mut Vec<ExternalSnapshotEntry>,
) -> Result<(), ExternalSnapshotInputError> {
    if entries.is_empty() {
        if revision != 0 {
            return Err(ExternalSnapshotInputError::InvalidSetRevision);
        }
    } else if revision == 0 {
        return Err(ExternalSnapshotInputError::InvalidSetRevision);
    }
    if entries.len() > MAX_EXTERNAL_SNAPSHOT_ENTRIES {
        return Err(ExternalSnapshotInputError::ResourceLimit {
            actual: entries.len(),
            limit: MAX_EXTERNAL_SNAPSHOT_ENTRIES,
        });
    }
    entries.sort_by_key(|entry| entry.binding);
    for pair in entries.windows(2) {
        if pair[0].binding == pair[1].binding {
            return Err(ExternalSnapshotInputError::DuplicateBinding {
                binding: pair[0].binding,
            });
        }
    }
    for entry in entries {
        if entry.source_revision == 0 {
            return Err(ExternalSnapshotInputError::InvalidSourceRevision {
                binding: entry.binding,
            });
        }
        validate_external_snapshot_feature(entry.binding, &entry.feature)?;
    }
    Ok(())
}

fn validate_external_snapshot_feature(
    binding: DocumentExternalBindingId,
    feature: &ExternalSnapshotFeatureV1,
) -> Result<(), ExternalSnapshotInputError> {
    let invalid = |reason| ExternalSnapshotInputError::InvalidFeature { binding, reason };
    let (scale, resources, expected) = match feature {
        ExternalSnapshotFeatureV1::Point {
            position,
            scale,
            resources,
        } => {
            if !position.iter().all(|value| value.is_finite()) {
                return Err(invalid("point position must be finite"));
            }
            (*scale, *resources, (1, 0, 0))
        }
        ExternalSnapshotFeatureV1::LineSegment {
            start,
            end,
            domain,
            scale,
            resources,
            ..
        } => {
            if !start.iter().chain(end).all(|value| value.is_finite()) {
                return Err(invalid("line endpoints must be finite"));
            }
            if domain[0].to_bits() != 0.0_f64.to_bits() || domain[1].to_bits() != 1.0_f64.to_bits()
            {
                return Err(invalid("line domain must be exactly [0, 1]"));
            }
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            if !dx.is_finite() || !dy.is_finite() || dx.hypot(dy) <= f64::EPSILON {
                return Err(invalid("line direction must be finite and nondegenerate"));
            }
            (*scale, *resources, (2, 0, 1))
        }
    };
    if !scale.is_finite() || scale <= 0.0 {
        return Err(invalid("scale must be positive and finite"));
    }
    if resources.point_count > MAX_EXTERNAL_SNAPSHOT_POINTS
        || resources.control_count > MAX_EXTERNAL_SNAPSHOT_CONTROLS
        || resources.span_count > MAX_EXTERNAL_SNAPSHOT_SPANS
    {
        return Err(invalid("resource evidence exceeds M43 limits"));
    }
    if (
        resources.point_count,
        resources.control_count,
        resources.span_count,
    ) != expected
    {
        return Err(invalid("resource evidence does not match feature kind"));
    }
    Ok(())
}

fn external_snapshot_set_digest(
    revision: u64,
    entries: &[ExternalSnapshotEntry],
) -> ExternalSnapshotSetDigest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"geosolve-external-snapshot-set-v1");
    bytes.extend_from_slice(&revision.to_be_bytes());
    for entry in entries {
        bytes.extend_from_slice(&entry.binding.0.as_u128().to_be_bytes());
        bytes.extend_from_slice(&entry.source_revision.to_be_bytes());
        bytes.extend_from_slice(&entry.source_digest.bytes());
        match &entry.feature {
            ExternalSnapshotFeatureV1::Point {
                position,
                scale,
                resources,
            } => {
                bytes.push(0);
                append_f64s(&mut bytes, position);
                bytes.extend_from_slice(&scale.to_bits().to_be_bytes());
                append_external_resources(&mut bytes, *resources);
            }
            ExternalSnapshotFeatureV1::LineSegment {
                start,
                end,
                domain,
                scale,
                topology_digest,
                resources,
                ..
            } => {
                bytes.push(1);
                append_f64s(&mut bytes, start);
                append_f64s(&mut bytes, end);
                append_f64s(&mut bytes, domain);
                bytes.push(0);
                bytes.extend_from_slice(&scale.to_bits().to_be_bytes());
                bytes.extend_from_slice(&topology_digest.bytes());
                append_external_resources(&mut bytes, *resources);
            }
        }
    }
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4_u64,
        0x9e37_79b1_85eb_ca87_u64,
        0xd6e8_feb8_6659_fd93_u64,
    ];
    for byte in bytes {
        for (index, lane) in lanes.iter_mut().enumerate() {
            *lane ^= u64::from(byte) + u64::try_from(index).unwrap_or(0);
            *lane = lane.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let mut digest = [0; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        digest[index * 8..(index + 1) * 8].copy_from_slice(&lane.to_be_bytes());
    }
    ExternalSnapshotSetDigest(digest)
}

fn append_f64s(bytes: &mut Vec<u8>, values: &[f64]) {
    for value in values {
        bytes.extend_from_slice(&value.to_bits().to_be_bytes());
    }
}

fn append_external_resources(bytes: &mut Vec<u8>, resources: ExternalSnapshotResourcesV1) {
    bytes.extend_from_slice(&resources.point_count.to_be_bytes());
    bytes.extend_from_slice(&resources.control_count.to_be_bytes());
    bytes.extend_from_slice(&resources.span_count.to_be_bytes());
}

fn parameter_digest(revision: u64, entries: &[ParameterBatchEntry]) -> ParameterDigest {
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4_u64,
        0x9e37_79b1_85eb_ca87_u64,
        0xd6e8_feb8_6659_fd93_u64,
    ];
    let mut bytes = Vec::with_capacity(16 + entries.len() * 25);
    bytes.extend_from_slice(b"geosolve-parameter-batch-v1");
    bytes.extend_from_slice(&revision.to_be_bytes());
    for entry in entries {
        bytes.extend_from_slice(&entry.parameter.0.as_u128().to_be_bytes());
        match entry.value {
            ParameterValue::Length(value) => {
                bytes.push(0);
                bytes.extend_from_slice(&value.to_bits().to_be_bytes());
            }
            ParameterValue::Angle(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.to_bits().to_be_bytes());
            }
            ParameterValue::Dimensionless(value) => {
                bytes.push(2);
                bytes.extend_from_slice(&value.to_bits().to_be_bytes());
            }
            ParameterValue::Activation(value) => {
                bytes.push(3);
                bytes.push(u8::from(value));
            }
        }
    }
    for byte in bytes {
        for (lane_index, lane) in lanes.iter_mut().enumerate() {
            *lane ^= u64::from(byte) + u64::try_from(lane_index).unwrap_or(0);
            *lane = lane.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let mut digest = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        digest[index * 8..(index + 1) * 8].copy_from_slice(&lane.to_be_bytes());
    }
    ParameterDigest(digest)
}

/// Defensive bound for one immutable host parameter batch.
pub const MAX_PARAMETER_BATCH_ENTRIES: usize = crate::MAX_DOCUMENT_PARAMETERS;

/// One closed typed canonical host parameter value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParameterValue {
    Length(f64),
    Angle(f64),
    Dimensionless(f64),
    Activation(bool),
}

impl ParameterValue {
    #[must_use]
    pub const fn kind(self) -> DocumentParameterKind {
        match self {
            Self::Length(_) => DocumentParameterKind::Length,
            Self::Angle(_) => DocumentParameterKind::Angle,
            Self::Dimensionless(_) => DocumentParameterKind::Dimensionless,
            Self::Activation(_) => DocumentParameterKind::Activation,
        }
    }

    #[must_use]
    pub const fn numeric(self) -> Option<f64> {
        match self {
            Self::Length(value) | Self::Angle(value) | Self::Dimensionless(value) => Some(value),
            Self::Activation(_) => None,
        }
    }
}

/// One canonical immutable host parameter entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParameterBatchEntry {
    pub parameter: DocumentParameterId,
    pub value: ParameterValue,
}

/// Canonical deterministic identity of one immutable parameter batch.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ParameterDigest([u8; 32]);

impl ParameterDigest {
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Immutable ordered, revisioned host parameter input captured by one attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterBatch {
    revision: u64,
    digest: ParameterDigest,
    entries: Vec<ParameterBatchEntry>,
}

impl Default for ParameterBatch {
    fn default() -> Self {
        Self {
            revision: 0,
            digest: parameter_digest(0, &[]),
            entries: Vec::new(),
        }
    }
}

impl ParameterBatch {
    /// Canonicalizes and captures one complete host input batch.
    ///
    /// # Errors
    ///
    /// Rejects a zero revision, excess, duplicate, or non-finite entries.
    pub fn new(
        revision: u64,
        mut entries: Vec<ParameterBatchEntry>,
    ) -> Result<Self, DocumentError> {
        if revision == 0 {
            return Err(DocumentError::InvalidField {
                field: "parameter batch revision",
                message: "must be positive".into(),
            });
        }
        if entries.len() > MAX_PARAMETER_BATCH_ENTRIES {
            return Err(DocumentError::ResourceLimit {
                resource: "parameter batch entries",
                actual: entries.len(),
                limit: MAX_PARAMETER_BATCH_ENTRIES,
            });
        }
        entries.sort_by_key(|entry| entry.parameter);
        for pair in entries.windows(2) {
            if pair[0].parameter == pair[1].parameter {
                return Err(DocumentError::InvalidField {
                    field: "parameter batch",
                    message: format!("duplicate parameter {}", pair[0].parameter),
                });
            }
        }
        for entry in &entries {
            if entry
                .value
                .numeric()
                .is_some_and(|value| !value.is_finite())
            {
                return Err(DocumentError::InvalidField {
                    field: "parameter batch",
                    message: format!("parameter {} is not finite", entry.parameter),
                });
            }
        }
        Ok(Self {
            revision,
            digest: parameter_digest(revision, &entries),
            entries,
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn digest(&self) -> ParameterDigest {
        self.digest
    }

    #[must_use]
    pub fn entries(&self) -> &[ParameterBatchEntry] {
        &self.entries
    }
}

#[allow(clippy::too_many_lines)]
fn resolve_parameter_batch(
    document: &SketchDocument,
    batch: &ParameterBatch,
    unavailable_external: &BTreeSet<DocumentElementId>,
) -> Result<ResolvedDocumentParameters, ParameterInputFailure> {
    document.validate().map_err(|error| ParameterInputFailure {
        issue: crate::SketchParameterInputIssue::InvalidDocument,
        error,
    })?;
    let mut activation_required = BTreeSet::new();
    for binding in document.parameter_bindings() {
        if matches!(binding.target, DocumentParameterTarget::Activation(_)) {
            activation_required.insert(binding.parameter);
        }
    }
    let mut supplied = BTreeMap::new();
    for entry in batch.entries() {
        let declaration = document
            .parameter(entry.parameter)
            .ok_or(ParameterInputFailure {
                issue: crate::SketchParameterInputIssue::Unknown(entry.parameter),
                error: DocumentError::UnknownId {
                    kind: "parameter batch entry",
                    id: entry.parameter.0,
                },
            })?;
        if declaration.kind != entry.value.kind() {
            return Err(ParameterInputFailure {
                issue: crate::SketchParameterInputIssue::WrongKind {
                    parameter: entry.parameter,
                    expected: declaration.kind,
                    actual: entry.value.kind(),
                },
                error: DocumentError::InvalidField {
                    field: "parameter batch",
                    message: format!("parameter {} has the wrong kind", entry.parameter),
                },
            });
        }
        supplied.insert(entry.parameter, entry.value);
    }
    if let Some(missing) = activation_required
        .iter()
        .find(|id| !supplied.contains_key(id))
    {
        return Err(ParameterInputFailure {
            issue: crate::SketchParameterInputIssue::Missing(*missing),
            error: DocumentError::InvalidField {
                field: "parameter batch",
                message: format!("required parameter {missing} is missing"),
            },
        });
    }

    let mut inactive = BTreeSet::<DocumentElementId>::new();
    for binding in document.parameter_bindings() {
        if let DocumentParameterTarget::Activation(element) = binding.target {
            let Some(ParameterValue::Activation(active)) = supplied.get(&binding.parameter) else {
                continue;
            };
            if !active {
                inactive.insert(element);
            }
        }
    }
    let activity = document.effective_activity_with_input_overlays(&inactive, unavailable_external);
    let mut required = activation_required;
    for binding in document.parameter_bindings() {
        let active = match binding.target {
            DocumentParameterTarget::DrivingDimension(id) => activity.is_active(id),
            DocumentParameterTarget::DimensionlessFixedScalar(property) => {
                document.dimensionless_parameter_target_is_active(property, &activity)
            }
            DocumentParameterTarget::Activation(_) => false,
        };
        if active {
            required.insert(binding.parameter);
        }
    }
    if let Some(unexpected) = supplied.keys().find(|id| !required.contains(id)) {
        return Err(ParameterInputFailure {
            issue: crate::SketchParameterInputIssue::Unexpected(*unexpected),
            error: DocumentError::InvalidField {
                field: "parameter batch",
                message: format!("parameter {unexpected} is not a required input"),
            },
        });
    }
    if let Some(missing) = required.iter().find(|id| !supplied.contains_key(id)) {
        return Err(ParameterInputFailure {
            issue: crate::SketchParameterInputIssue::Missing(*missing),
            error: DocumentError::InvalidField {
                field: "parameter batch",
                message: format!("required parameter {missing} is missing"),
            },
        });
    }
    let mut dimensions = BTreeMap::new();
    let mut dimensionless = BTreeMap::new();
    for binding in document.parameter_bindings() {
        let Some(value) = supplied
            .get(&binding.parameter)
            .and_then(|value| value.numeric())
        else {
            continue;
        };
        let resolved = ResolvedParameterBinding {
            parameter: binding.parameter,
            target: binding.target,
            value,
            parameter_revision: batch.revision(),
            parameter_digest: batch.digest(),
        };
        match binding.target {
            DocumentParameterTarget::DrivingDimension(dimension)
                if activity.is_active(dimension) =>
            {
                document
                    .validate_parameter_dimension_value(dimension, value)
                    .map_err(|error| ParameterInputFailure {
                        issue: crate::SketchParameterInputIssue::InvalidValue(binding.parameter),
                        error,
                    })?;
                dimensions.insert(dimension, resolved);
            }
            DocumentParameterTarget::DimensionlessFixedScalar(property)
                if document.dimensionless_parameter_target_is_active(property, &activity) =>
            {
                document
                    .validate_parameter_scalar_value(property.scalar, value)
                    .map_err(|error| ParameterInputFailure {
                        issue: crate::SketchParameterInputIssue::InvalidValue(binding.parameter),
                        error,
                    })?;
                dimensionless.insert(property.scalar, resolved);
            }
            DocumentParameterTarget::DrivingDimension(_)
            | DocumentParameterTarget::DimensionlessFixedScalar(_)
            | DocumentParameterTarget::Activation(_) => {}
        }
    }
    Ok(ResolvedDocumentParameters {
        activity,
        dimensions,
        dimensionless,
        external_revision: 0,
        external_digest: ExternalSnapshotSet::default().digest(),
        external: BTreeMap::new(),
    })
}

fn resolve_attempt_inputs(
    document: &SketchDocument,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<ResolvedDocumentParameters, AttemptInputError> {
    let entries = snapshots
        .entries()
        .iter()
        .map(|entry| (entry.binding, entry.clone()))
        .collect::<BTreeMap<_, _>>();
    for binding in entries.keys() {
        if document.external_binding(*binding).is_none() {
            return Err(AttemptInputError::External {
                error: ExternalSnapshotInputError::UnknownBinding { binding: *binding },
                activity: document.effective_activity(),
            });
        }
    }
    let mut unavailable = BTreeSet::new();
    let mut incompatibilities = BTreeMap::new();
    for declaration in document.external_bindings() {
        let error = match entries.get(&declaration.id) {
            None => Some(ExternalSnapshotInputError::MissingBinding {
                binding: declaration.id,
            }),
            Some(entry) if entry.feature.kind() != declaration.expected_kind => {
                Some(ExternalSnapshotInputError::WrongKind {
                    binding: declaration.id,
                    expected: declaration.expected_kind,
                    actual: entry.feature.kind(),
                })
            }
            Some(ExternalSnapshotEntry {
                feature:
                    ExternalSnapshotFeatureV1::LineSegment {
                        topology_digest, ..
                    },
                ..
            }) if declaration.expected_topology != Some(*topology_digest) => {
                Some(ExternalSnapshotInputError::TopologyMismatch {
                    binding: declaration.id,
                })
            }
            Some(_) => None,
        };
        if let Some(error) = error {
            unavailable.insert(DocumentElementId::ExternalBinding(declaration.id));
            incompatibilities.insert(declaration.id, error);
        }
    }
    let mut resolved = resolve_parameter_batch(document, parameters, &unavailable)
        .map_err(AttemptInputError::Parameter)?;
    for constraint in document.constraints() {
        let binding = match constraint.definition {
            DocumentConstraintDefinition::ExternalPointCoincident { external, .. } => {
                external.binding
            }
            DocumentConstraintDefinition::ExternalLineCollinear { external, .. } => {
                external.binding
            }
            _ => continue,
        };
        if resolved.activity.reason(binding)
            == Some(crate::InactivityReason::UnavailableExternalReference)
            && matches!(
                resolved.activity.reason(constraint.id),
                Some(crate::InactivityReason::UnavailableDependency {
                    dependency: DocumentElementId::ExternalBinding(dependency)
                }) if dependency == binding
            )
        {
            return Err(AttemptInputError::External {
                error: incompatibilities
                    .get(&binding)
                    .expect("unavailable binding has structured input evidence")
                    .clone(),
                activity: resolved.activity,
            });
        }
    }
    resolved.external_revision = snapshots.revision();
    resolved.external_digest = snapshots.digest();
    resolved.external = entries;
    Ok(resolved)
}

enum AttemptInputError {
    Parameter(ParameterInputFailure),
    External {
        error: ExternalSnapshotInputError,
        activity: crate::EffectiveActivity,
    },
}

struct ParameterInputFailure {
    issue: crate::SketchParameterInputIssue,
    error: DocumentError,
}

/// Persistent drag request lowered only after runtime IDs have been allocated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentDragTarget {
    pub point: DesignPointId,
    pub target: [f64; 2],
}

/// One persistent point selected to preserve a passive mechanism freedom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentDragLocalityAnchor {
    point: DesignPointId,
    /// Immutable accepted position captured when the gesture began.
    target: [f64; 2],
    /// Rank of this point's two-coordinate response in the accepted hard nullspace.
    mobility_rank: usize,
}

impl DocumentDragLocalityAnchor {
    #[must_use]
    pub const fn point(&self) -> DesignPointId {
        self.point
    }

    #[must_use]
    pub const fn target(&self) -> [f64; 2] {
        self.target
    }

    #[must_use]
    pub const fn mobility_rank(&self) -> usize {
        self.mobility_rank
    }
}

/// Persistent, exact-state-stamped locality ownership for one projected drag.
///
/// Runtime rank evidence chooses a deterministic complete greedy passive-anchor cover.
/// Persistent identities and targets are then captured from the current accepted
/// document. The complete value must be retained unchanged through continuation,
/// rejection and recovery; it becomes stale after either stamped identity changes.
///
/// Its DOF and rank fields describe the accepted hard-*equality* nullspace. Active
/// bounds and their one-sided feasible mobility remain separate core-owned evidence
/// and are not encoded by this plan.
#[derive(Clone, Debug)]
pub struct DocumentDragLocalityPlan {
    design: SketchDesignIdentity,
    /// Exact process-local provenance for `design`.
    ///
    /// Design revisions can collide when lifecycle clones publish independently.
    /// The shared token survives ordinary gesture clones and changes on every
    /// retained-design publication.
    design_provenance: Arc<()>,
    accepted: SketchAcceptedStateIdentity,
    /// Exact process-local provenance for `accepted`.
    ///
    /// Revisions can collide when lifecycle clones advance independently. The
    /// shared token survives the ordinary clones used by one gesture and changes
    /// on every accepted publication.
    accepted_provenance: Arc<()>,
    point: DesignPointId,
    hard_degrees_of_freedom: usize,
    active_rank: usize,
    passive_degrees_of_freedom: usize,
    anchors: Vec<DocumentDragLocalityAnchor>,
}

impl PartialEq for DocumentDragLocalityPlan {
    fn eq(&self, other: &Self) -> bool {
        self.design == other.design
            && Arc::ptr_eq(&self.design_provenance, &other.design_provenance)
            && self.accepted == other.accepted
            && Arc::ptr_eq(&self.accepted_provenance, &other.accepted_provenance)
            && self.point == other.point
            && self.hard_degrees_of_freedom == other.hard_degrees_of_freedom
            && self.active_rank == other.active_rank
            && self.passive_degrees_of_freedom == other.passive_degrees_of_freedom
            && self.anchors == other.anchors
    }
}

impl DocumentDragLocalityPlan {
    #[must_use]
    pub const fn design_identity(&self) -> SketchDesignIdentity {
        self.design
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }

    #[must_use]
    pub const fn point(&self) -> DesignPointId {
        self.point
    }

    #[must_use]
    /// Returns the dimension of the accepted hard-equality nullspace.
    ///
    /// This is not bounded or one-sided feasible DOF; active-bound mobility remains
    /// separate core evidence.
    pub const fn hard_degrees_of_freedom(&self) -> usize {
        self.hard_degrees_of_freedom
    }

    #[must_use]
    /// Returns the active point's rank in the accepted hard-equality nullspace.
    pub const fn active_rank(&self) -> usize {
        self.active_rank
    }

    #[must_use]
    /// Returns the hard-equality nullspace rank not controlled by the active point.
    pub const fn passive_degrees_of_freedom(&self) -> usize {
        self.passive_degrees_of_freedom
    }

    #[must_use]
    pub fn anchors(&self) -> &[DocumentDragLocalityAnchor] {
        &self.anchors
    }
}

/// Per-solve interaction preferences expressed only in persistent IDs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentSolveRequest {
    pub drag: Option<DocumentDragTarget>,
    pub previous_state_preferences: bool,
}

impl DocumentSolveRequest {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            drag: None,
            previous_state_preferences: true,
        }
    }

    #[must_use]
    pub const fn without_previous_state_preferences(mut self) -> Self {
        self.previous_state_preferences = false;
        self
    }

    /// Retains all non-targeted accepted points as the lowest-priority interaction objective.
    #[must_use]
    pub const fn with_previous_state_preferences(mut self) -> Self {
        self.previous_state_preferences = true;
        self
    }

    /// Removes the interaction-scoped drag target before a retained restore.
    #[must_use]
    pub const fn without_temporary_targets(mut self) -> Self {
        self.drag = None;
        self
    }

    #[must_use]
    pub const fn with_drag(mut self, point: DesignPointId, target: [f64; 2]) -> Self {
        self.drag = Some(DocumentDragTarget { point, target });
        self
    }
}

impl Default for DocumentSolveRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Separate attempted diagnostics and accepted state views for one document solve.
#[derive(Clone, Debug)]
pub struct DocumentSolveResult {
    attempted_solve: SketchSolveResult,
    accepted_view: SketchSolveResult,
    /// Runtime mappings for the accepted document state.
    mappings: DocumentRuntimeMap,
    /// Candidate mappings retained only for attempted diagnostics.
    attempted_mappings: DocumentRuntimeMap,
    attempted_sources: Vec<crate::SketchSourceMapping>,
    attempted_bound_mappings: Vec<crate::SketchBoundMapping>,
}

impl DocumentSolveResult {
    fn new(solve: SketchSolveResult, mappings: DocumentRuntimeMap) -> Self {
        Self {
            attempted_sources: solve.source_mappings.clone(),
            attempted_bound_mappings: solve.bound_mappings.clone(),
            attempted_mappings: mappings.clone(),
            accepted_view: solve.clone(),
            attempted_solve: solve,
            mappings,
        }
    }

    /// Returns the complete candidate attempt and its diagnostic mappings.
    #[must_use]
    pub const fn solve(&self) -> &SketchSolveResult {
        &self.attempted_solve
    }

    /// Returns the state/audit view retained by the document session.
    #[must_use]
    pub const fn accepted_view(&self) -> &SketchSolveResult {
        &self.accepted_view
    }

    #[must_use]
    pub const fn mappings(&self) -> &DocumentRuntimeMap {
        &self.mappings
    }

    /// Candidate remap used only to interpret an attempted solve's diagnostics.
    #[must_use]
    pub const fn attempted_mappings(&self) -> &DocumentRuntimeMap {
        &self.attempted_mappings
    }

    /// Candidate bound identities corresponding to `solve().unstable_core_report().bounds`.
    #[must_use]
    pub fn attempted_bound_mappings(&self) -> &[crate::SketchBoundMapping] {
        &self.attempted_bound_mappings
    }

    /// Returns one accepted reference-dimension measurement by persistent identity.
    #[must_use]
    pub fn accepted_reference_value(
        &self,
        document: &SketchDocument,
        dimension: DocumentDimensionId,
    ) -> Option<f64> {
        let source = document.dimension(dimension)?.source_id;
        let RuntimeSource::Dimension(runtime) = self.mappings.runtime_source(source)? else {
            return None;
        };
        self.accepted_view
            .reference_values
            .iter()
            .find_map(|value| (value.dimension_id == runtime).then_some(value.value))
    }

    #[must_use]
    pub fn accepted(&self) -> bool {
        self.attempted_solve.rejection.is_none()
    }

    /// Maps a runtime domain source from solver diagnostics back to persistent source identity.
    #[must_use]
    pub fn persistent_source(&self, source: SketchSource) -> Option<DocumentSourceId> {
        let runtime = match source {
            SketchSource::Constraint(id) => RuntimeSource::Constraint(id),
            SketchSource::Dimension(id) => RuntimeSource::Dimension(id),
            SketchSource::DragTarget(_) | SketchSource::PreviousState(_) => return None,
        };
        self.attempted_mappings
            .source_mappings()
            .iter()
            .find_map(|mapping| (mapping.runtime == Some(runtime)).then_some(mapping.source_id))
    }

    /// Maps a core source from a solve report back to persistent source identity.
    #[must_use]
    pub fn persistent_core_source(
        &self,
        source: geosolve_core::SourceConstraintId,
    ) -> Option<DocumentSourceId> {
        let runtime = self.attempted_sources.iter().find_map(|mapping| {
            (mapping.core_source_id == Some(source)).then_some(mapping.source)
        })?;
        self.persistent_source(runtime)
    }
}

macro_rules! lifecycle_revision {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

lifecycle_revision!(
    SketchDesignRevision,
    "Monotonic revision in the retained-design identity domain."
);
lifecycle_revision!(
    SketchAttemptRevision,
    "Never-reused revision in the solve-attempt identity domain."
);
lifecycle_revision!(
    SketchAcceptedRevision,
    "Monotonic revision in the independently accepted-state identity domain."
);

/// Identity of one finite, structurally valid retained design graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SketchDesignIdentity {
    document: crate::DocumentId,
    revision: SketchDesignRevision,
}

impl SketchDesignIdentity {
    #[must_use]
    pub const fn document(self) -> crate::DocumentId {
        self.document
    }

    #[must_use]
    pub const fn revision(self) -> SketchDesignRevision {
        self.revision
    }
}

/// Identity of one evaluation of one exact retained design revision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SketchAttemptIdentity {
    document: crate::DocumentId,
    revision: SketchAttemptRevision,
}

impl SketchAttemptIdentity {
    #[must_use]
    pub const fn document(self) -> crate::DocumentId {
        self.document
    }

    #[must_use]
    pub const fn revision(self) -> SketchAttemptRevision {
        self.revision
    }
}

/// Identity of one independently validated solved state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SketchAcceptedStateIdentity {
    document: crate::DocumentId,
    revision: SketchAcceptedRevision,
}

impl SketchAcceptedStateIdentity {
    #[must_use]
    pub const fn document(self) -> crate::DocumentId {
        self.document
    }

    #[must_use]
    pub const fn revision(self) -> SketchAcceptedRevision {
        self.revision
    }
}

/// Exact M34 inputs evaluated by one attempt before later host-input stamps exist.
///
/// M41-M43 extend the lifecycle with activation, parameter and external-snapshot
/// identities; M56 later adds prepared-work identity. This type
/// intentionally records only inputs implemented by M34 and does not claim to be the
/// final v5 input stamp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchAttemptInput {
    design: SketchDesignIdentity,
    candidate_request: DocumentSolveRequest,
    publication_request: DocumentSolveRequest,
    solver_config: SolverConfig,
    effective_activation_revision: u64,
    activation_digest: ActivationDigest,
    parameter_revision: u64,
    parameter_digest: ParameterDigest,
    external_snapshot_set_revision: u64,
    external_snapshot_set_digest: ExternalSnapshotSetDigest,
}

impl SketchAttemptInput {
    fn for_document_with_parameters(
        document: &SketchDocument,
        design: SketchDesignIdentity,
        candidate_request: DocumentSolveRequest,
        publication_request: DocumentSolveRequest,
        solver_config: SolverConfig,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Self {
        // A valid M42 batch contributes its activation overlay to the exact
        // immutable attempt stamp. Invalid batches remain attempts (rather than
        // accepted states), so retain the document-only activity for their
        // diagnostic capsule.
        let activity = resolve_attempt_inputs(document, parameters, snapshots).map_or_else(
            |_| document.effective_activity(),
            |resolved| resolved.activity,
        );
        Self {
            design,
            candidate_request,
            publication_request,
            solver_config,
            effective_activation_revision: activity.activation_revision(),
            activation_digest: activity.activation_digest(),
            parameter_revision: parameters.revision(),
            parameter_digest: parameters.digest(),
            external_snapshot_set_revision: snapshots.revision(),
            external_snapshot_set_digest: snapshots.digest(),
        }
    }

    #[must_use]
    pub const fn design_identity(self) -> SketchDesignIdentity {
        self.design
    }

    #[must_use]
    pub const fn candidate_request(self) -> DocumentSolveRequest {
        self.candidate_request
    }

    /// Returns the request used to rebuild and independently publish accepted state.
    #[must_use]
    pub const fn publication_request(self) -> DocumentSolveRequest {
        self.publication_request
    }

    #[must_use]
    pub const fn solver_config(self) -> SolverConfig {
        self.solver_config
    }

    /// Returns the effective activation revision captured before lowering.
    #[must_use]
    pub const fn effective_activation_revision(self) -> u64 {
        self.effective_activation_revision
    }

    /// Returns the exact effective activation payload identity captured before lowering.
    #[must_use]
    pub const fn activation_digest(self) -> ActivationDigest {
        self.activation_digest
    }

    #[must_use]
    pub const fn parameter_revision(self) -> u64 {
        self.parameter_revision
    }

    #[must_use]
    pub const fn parameter_digest(self) -> ParameterDigest {
        self.parameter_digest
    }

    #[must_use]
    pub const fn external_snapshot_set_revision(self) -> u64 {
        self.external_snapshot_set_revision
    }

    #[must_use]
    pub const fn external_snapshot_set_digest(self) -> ExternalSnapshotSetDigest {
        self.external_snapshot_set_digest
    }
}

/// Stage at which a retained-design attempt failed before producing a solve report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchAttemptFailureKind {
    ParameterInput,
    ExternalSnapshotInput,
    Lowering,
    Request,
    Solve,
    AcceptedSession,
    Publication,
}

/// Structured non-solve failure for an identifiable retained-design attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchAttemptFailure {
    kind: SketchAttemptFailureKind,
    message: String,
    parameter_issue: Option<crate::SketchParameterInputIssue>,
    external_error: Option<ExternalSnapshotInputError>,
    effective_activity: Option<crate::EffectiveActivity>,
}

impl SketchAttemptFailure {
    #[must_use]
    pub const fn kind(&self) -> SketchAttemptFailureKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn parameter_input_issue(&self) -> Option<crate::SketchParameterInputIssue> {
        self.parameter_issue
    }

    #[must_use]
    pub const fn external_snapshot_error(&self) -> Option<&ExternalSnapshotInputError> {
        self.external_error.as_ref()
    }

    /// Returns the exact M41 closure used to classify an external-input failure.
    #[must_use]
    pub const fn effective_activity(&self) -> Option<&crate::EffectiveActivity> {
        self.effective_activity.as_ref()
    }
}

/// Non-authoritative evidence from one exact retained-design solve attempt.
#[derive(Clone, Debug)]
pub struct SketchDocumentAttempt {
    identity: SketchAttemptIdentity,
    input: SketchAttemptInput,
    /// Exact process-local provenance for the retained design evaluated here.
    design_provenance: Arc<()>,
    /// Provenance of the immediately preceding retained design publication.
    ///
    /// This remains stable across reattempts of the same design so a branch
    /// preview can prove that its next-revision design descended from the
    /// authoritative base rather than a divergent same-revision clone.
    parent_design_provenance: Option<Arc<()>>,
    parent_accepted: Option<SketchAcceptedStateIdentity>,
    /// Exact process-local provenance for `parent_accepted`.
    ///
    /// Accepted revision numbers can collide when two clones advance independently.
    /// The shared token survives ordinary cloning and changes on every accepted
    /// publication, so a preview cannot substitute a divergent same-revision parent.
    parent_accepted_provenance: Option<Arc<()>>,
    accepted_state: Option<SketchAcceptedStateIdentity>,
    solve: Option<SketchSolveResult>,
    attempted_geometry: Option<crate::SketchGeometry>,
    mappings: Option<DocumentRuntimeMap>,
    effective_activity: Option<crate::EffectiveActivity>,
    failure: Option<SketchAttemptFailure>,
}

impl SketchDocumentAttempt {
    #[must_use]
    pub const fn identity(&self) -> SketchAttemptIdentity {
        self.identity
    }

    #[must_use]
    pub const fn design_identity(&self) -> SketchDesignIdentity {
        self.input.design
    }

    #[must_use]
    pub const fn input(&self) -> SketchAttemptInput {
        self.input
    }

    #[must_use]
    pub const fn parent_accepted_identity(&self) -> Option<SketchAcceptedStateIdentity> {
        self.parent_accepted
    }

    /// Returns the state published by this attempt, never an older retained state.
    #[must_use]
    pub const fn accepted_state_identity(&self) -> Option<SketchAcceptedStateIdentity> {
        self.accepted_state
    }

    /// Returns a solve report only when solving reached a reportable outcome.
    #[must_use]
    pub const fn solve_result(&self) -> Option<&SketchSolveResult> {
        self.solve.as_ref()
    }

    /// Returns optional finite candidate geometry as non-authoritative evidence.
    #[must_use]
    pub const fn attempted_geometry(&self) -> Option<&crate::SketchGeometry> {
        self.attempted_geometry.as_ref()
    }

    /// Runtime mappings belong only to this attempt and must not interpret accepted state.
    #[must_use]
    pub const fn mappings(&self) -> Option<&DocumentRuntimeMap> {
        self.mappings.as_ref()
    }

    /// Returns the exact effective activation closure used by this attempt, when
    /// input resolution reached that stage.
    #[must_use]
    pub const fn effective_activity(&self) -> Option<&crate::EffectiveActivity> {
        self.effective_activity.as_ref()
    }

    #[must_use]
    pub const fn failure(&self) -> Option<&SketchAttemptFailure> {
        self.failure.as_ref()
    }

    /// Maps one attempted runtime source back to its persistent design source.
    #[must_use]
    pub fn persistent_source(&self, source: SketchSource) -> Option<DocumentSourceId> {
        let runtime = match source {
            SketchSource::Constraint(id) => RuntimeSource::Constraint(id),
            SketchSource::Dimension(id) => RuntimeSource::Dimension(id),
            SketchSource::DragTarget(_) | SketchSource::PreviousState(_) => return None,
        };
        self.mappings()?
            .source_mappings()
            .iter()
            .find_map(|mapping| (mapping.runtime == Some(runtime)).then_some(mapping.source_id))
    }

    /// Maps one attempted core source back to its persistent design source.
    #[must_use]
    pub fn persistent_core_source(
        &self,
        source: geosolve_core::SourceConstraintId,
    ) -> Option<DocumentSourceId> {
        let runtime = self
            .solve_result()?
            .source_mappings
            .iter()
            .find_map(|mapping| {
                (mapping.core_source_id == Some(source)).then_some(mapping.source)
            })?;
        self.persistent_source(runtime)
    }
}

/// One coherent accepted document, runtime, audit, and provenance view.
#[derive(Clone, Debug)]
pub struct SketchAcceptedDocumentState {
    identity: SketchAcceptedStateIdentity,
    /// Process-local content provenance shared by lifecycle clones.
    provenance: Arc<()>,
    /// Exact process-local provenance for `solved_design`.
    design_provenance: Arc<()>,
    input: SketchAttemptInput,
    originating_attempt: SketchAttemptIdentity,
    solved_design: SketchDocument,
    document: SketchDocument,
    runtime: SketchSession,
    mappings: DocumentRuntimeMap,
    effective_activity: crate::EffectiveActivity,
    redundancy: SketchAcceptedDocumentRedundancy,
    parameter_outputs: Vec<DocumentParameterOutputProposal>,
    profile_cache: RefCell<Vec<(crate::VisualProfileOptions, crate::VisualProfileAnalysis)>>,
}

/// One accepted, independently evaluated reference-dimension output proposal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentParameterOutputProposal {
    pub parameter: DocumentParameterId,
    pub dimension: DocumentDimensionId,
    pub source: DocumentSourceId,
    pub unit: DocumentScalarUnit,
    pub value: f64,
    pub design: SketchDesignIdentity,
    pub attempt: SketchAttemptIdentity,
    pub accepted: SketchAcceptedStateIdentity,
    pub parameter_revision: u64,
    pub parameter_digest: ParameterDigest,
    pub provenance: DocumentMeasurementProvenance,
}

/// Persistent accepted redundancy with exact accepted-state and solved-design provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchAcceptedDocumentRedundancy {
    accepted_state: SketchAcceptedStateIdentity,
    design: SketchDesignIdentity,
    fully_redundant_sources: Vec<DocumentSourceId>,
    sources_containing_redundant_rows: Vec<DocumentSourceId>,
}

impl SketchAcceptedDocumentRedundancy {
    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted_state
    }

    #[must_use]
    pub const fn design_identity(&self) -> SketchDesignIdentity {
        self.design
    }

    #[must_use]
    pub fn fully_redundant_sources(&self) -> &[DocumentSourceId] {
        &self.fully_redundant_sources
    }

    #[must_use]
    pub fn sources_containing_redundant_rows(&self) -> &[DocumentSourceId] {
        &self.sources_containing_redundant_rows
    }
}

impl SketchAcceptedDocumentState {
    #[must_use]
    pub const fn identity(&self) -> SketchAcceptedStateIdentity {
        self.identity
    }

    #[must_use]
    pub const fn design_identity(&self) -> SketchDesignIdentity {
        self.input.design
    }

    #[must_use]
    pub const fn input(&self) -> SketchAttemptInput {
        self.input
    }

    #[must_use]
    pub const fn originating_attempt(&self) -> SketchAttemptIdentity {
        self.originating_attempt
    }

    /// Returns the accepted solved document, which may predate the current design.
    #[must_use]
    pub const fn document(&self) -> &SketchDocument {
        &self.document
    }

    #[must_use]
    pub const fn runtime(&self) -> &SketchSession {
        &self.runtime
    }

    #[must_use]
    pub const fn mappings(&self) -> &DocumentRuntimeMap {
        &self.mappings
    }

    /// Returns the exact effective activation closure accepted with this state.
    #[must_use]
    pub const fn effective_activity(&self) -> &crate::EffectiveActivity {
        &self.effective_activity
    }

    /// Returns accepted geometry, audit, measurements, rank, and diagnostics from one state.
    #[must_use]
    pub const fn solve_result(&self) -> &SketchSolveResult {
        self.runtime.accepted_result()
    }

    /// Returns persistent redundancy derived only from this accepted state.
    #[must_use]
    pub const fn accepted_redundancy(&self) -> &SketchAcceptedDocumentRedundancy {
        &self.redundancy
    }

    /// Returns the immutable output proposals published with this accepted state.
    #[must_use]
    pub fn parameter_output_proposals(&self) -> &[DocumentParameterOutputProposal] {
        &self.parameter_outputs
    }

    /// Returns stable persistent-ID diagnostics for this exact accepted state.
    #[must_use]
    pub fn diagnostics(&self) -> crate::SketchDiagnosticSnapshot {
        let variable_elements =
            crate::diagnostics::diagnostic_variable_elements(self.solve_result(), &self.mappings);
        crate::diagnostics::build_diagnostic_snapshot(
            &crate::diagnostics::SketchDiagnosticBuildInput {
                provenance: crate::SketchDiagnosticProvenance::Accepted {
                    accepted: self.identity,
                    originating_attempt: self.originating_attempt,
                    design: self.design_identity(),
                },
                input: self.input,
                document: &self.solved_design,
                solve: Some(self.solve_result()),
                mappings: Some(&self.mappings),
                activity: &self.effective_activity,
                parameter_issue: None,
                external_issue: None,
                variable_elements: &variable_elements,
            },
        )
    }

    /// Returns bounded visual-profile analysis cached by this exact accepted revision.
    ///
    /// The cache is presentation-independent and cannot affect equations or acceptance.
    /// A new accepted state owns a fresh cache, so results never cross revision identity.
    #[must_use]
    pub fn analyze_visual_profiles_cached(
        &self,
        options: crate::VisualProfileOptions,
    ) -> crate::VisualProfileAnalysis {
        if let Some(analysis) = self
            .profile_cache
            .borrow()
            .iter()
            .find_map(|(cached, analysis)| (*cached == options).then(|| analysis.clone()))
        {
            return analysis;
        }
        let analysis = self.document.analyze_visual_profiles(options);
        self.profile_cache
            .borrow_mut()
            .push((options, analysis.clone()));
        analysis
    }

    /// Number of option-specific profile results retained by this accepted revision.
    #[must_use]
    pub fn visual_profile_cache_entries(&self) -> usize {
        self.profile_cache.borrow().len()
    }

    /// Returns one accepted reference measurement by persistent dimension identity.
    #[must_use]
    pub fn reference_value(&self, dimension: DocumentDimensionId) -> Option<f64> {
        let source = self.document.dimension(dimension)?.source_id;
        let RuntimeSource::Dimension(runtime) = self.mappings.runtime_source(source)? else {
            return None;
        };
        self.solve_result()
            .reference_values
            .iter()
            .find_map(|value| (value.dimension_id == runtime).then_some(value.value))
    }
}

/// Result of retaining a valid design transaction and attempting that exact revision.
#[derive(Clone, Debug)]
pub struct RetainedDocumentTransactionOutcome<T> {
    value: T,
    design: SketchDesignIdentity,
    attempt: SketchAttemptIdentity,
    published_accepted: Option<SketchAcceptedStateIdentity>,
}

/// One explicit persistent line-span branch edit used by an atomic assembly-mode transaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentCurveBranchEdit {
    pub curve: CurveSpan,
    pub direction: [f64; 2],
}

impl<T> RetainedDocumentTransactionOutcome<T> {
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    #[must_use]
    pub const fn design_identity(&self) -> SketchDesignIdentity {
        self.design
    }

    #[must_use]
    pub const fn attempt_identity(&self) -> SketchAttemptIdentity {
        self.attempt
    }

    /// Returns only the accepted state created by this transaction.
    #[must_use]
    pub const fn published_accepted_identity(&self) -> Option<SketchAcceptedStateIdentity> {
        self.published_accepted
    }
}

/// Host-persistable revision high-water metadata kept outside frozen sketch v1-v4.
///
/// Hosts may encode these three integers in application-owned workspace state. They
/// are deliberately not a `GeoSolve` wire envelope or draft-v5 schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SketchLifecycleRevisionHighWater {
    design: SketchDesignRevision,
    attempt: SketchAttemptRevision,
    accepted: Option<SketchAcceptedRevision>,
}

impl SketchLifecycleRevisionHighWater {
    /// Reconstructs host-owned high-water metadata from persisted integer fields.
    #[must_use]
    pub const fn from_raw(design: u64, attempt: u64, accepted: Option<u64>) -> Self {
        Self {
            design: SketchDesignRevision(design),
            attempt: SketchAttemptRevision(attempt),
            accepted: match accepted {
                Some(revision) => Some(SketchAcceptedRevision(revision)),
                None => None,
            },
        }
    }

    #[must_use]
    pub const fn design(self) -> SketchDesignRevision {
        self.design
    }

    #[must_use]
    pub const fn attempt(self) -> SketchAttemptRevision {
        self.attempt
    }

    #[must_use]
    pub const fn accepted(self) -> Option<SketchAcceptedRevision> {
        self.accepted
    }
}

/// One typed document edit. IDs for created objects are returned in [`DocumentCommandEffect`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DocumentEdit {
    CreatePoint {
        label: String,
        position: [f64; 2],
    },
    CreateScalar {
        label: String,
        value: f64,
        unit: ScalarUnit,
        domain: ScalarDomain,
    },
    CreateCurve {
        label: String,
        definition: CurveDefinition,
    },
    CreateContact {
        label: String,
        definition: ContactDefinition,
    },
    CreateConstraint {
        label: String,
        definition: DocumentConstraintDefinition,
    },
    CreateDimension {
        label: String,
        definition: DocumentDimensionDefinition,
        mode: DocumentDimensionMode,
    },
    CreateParameter {
        label: String,
        kind: DocumentParameterKind,
    },
    AddParameterBinding {
        parameter: DocumentParameterId,
        target: DocumentParameterTarget,
    },
    RemoveParameterBinding {
        parameter: DocumentParameterId,
        target: DocumentParameterTarget,
    },
    AddParameterOutput {
        parameter: DocumentParameterId,
        dimension: DocumentDimensionId,
    },
    RemoveParameterOutput {
        parameter: DocumentParameterId,
        dimension: DocumentDimensionId,
    },
    CreateRectangle {
        label: String,
        origin: [f64; 2],
        width: f64,
        height: f64,
    },
    CreateMirroredCurve {
        label: String,
        source_curve: CurveId,
        axis: CurveSpan,
    },
    CreateLineLineFillet {
        label: String,
        request: LineLineFilletRequest,
    },
    CreateCurveCurveFillet {
        label: String,
        request: CurveCurveFilletRequest,
    },
    SetPointPosition {
        point: DesignPointId,
        position: [f64; 2],
    },
    SetScalarValue {
        scalar: DesignScalarId,
        value: f64,
    },
    SetCurveBranch {
        curve: CurveSpan,
        direction: [f64; 2],
    },
    SetArcSweep {
        curve: CurveId,
        sweep: DocumentArcSweep,
    },
    SetLineLineFilletBranch {
        constraint: DocumentConstraintId,
        first_side: DocumentCurveNormalSide,
        second_side: DocumentCurveNormalSide,
        endpoint_order: DocumentFilletEndpointOrder,
        sweep: DocumentArcSweep,
    },
    SetCurveCurveFilletBranch {
        constraint: DocumentConstraintId,
        first_side: DocumentCurveNormalSide,
        first_trim_endpoint: DocumentFilletTrimEndpoint,
        second_side: DocumentCurveNormalSide,
        second_trim_endpoint: DocumentFilletTrimEndpoint,
        endpoint_order: DocumentFilletEndpointOrder,
        sweep: DocumentArcSweep,
    },
    SetConicWeightedMiddle {
        curve: CurveId,
        weighted_middle: [f64; 2],
    },
    SetHyperbolaBranch {
        curve: CurveId,
        branch: DocumentHyperbolaBranch,
    },
    InsertBSplineKnot {
        curve: CurveId,
        parameter: f64,
    },
    InsertMirroredBSplineKnot {
        label: String,
        source_curve: CurveId,
        mirrored_curve: CurveId,
        axis: CurveSpan,
        parameter: f64,
    },
    TransitionBSplineContact {
        contact: ContactId,
        direction: DocumentBSplineSpanDirection,
    },
    InsertNurbsKnot {
        curve: CurveId,
        parameter: f64,
    },
    TransitionNurbsContact {
        contact: ContactId,
        direction: DocumentBSplineSpanDirection,
    },
    SetNurbsWeightGauge {
        curve: CurveId,
        gauge_weight: DesignScalarId,
    },
    SetContactStates {
        edits: Vec<ContactStateEdit>,
    },
    SetContactBranches {
        edits: Vec<ContactBranchEdit>,
    },
    SetCircleTangencyBranch {
        constraint: DocumentConstraintId,
        mode: DocumentCircleTangencyMode,
        center_direction: [f64; 2],
    },
    SetDimensionMode {
        dimension: DocumentDimensionId,
        mode: DocumentDimensionMode,
    },
    SetOrientedAngleOrientation {
        dimension: DocumentDimensionId,
        orientation: DocumentAngleOrientation,
    },
    SetSourceSuppressed {
        source: DocumentSourceId,
        suppressed: bool,
    },
    /// Changes only the profile/construction role of one curve.
    SetGeometryRole {
        curve: CurveId,
        role: GeometryRole,
    },
    /// Generalized user activation for every persistent document element.
    ///
    /// `SetSourceSuppressed` remains the source-specific compatibility command.
    SetElementUserSuppressed {
        element: crate::DocumentElementId,
        suppressed: bool,
    },
    /// Installs a newer immutable host configuration payload.
    ///
    /// A newer payload with no overrides clears all host-requested inactivity.
    SetHostConfigurationActivation {
        activation: HostConfigurationActivation,
    },
    Delete {
        object: DocumentObjectId,
    },
}

/// Revision-checked command input.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentCommand {
    pub expected_revision: u64,
    pub edit: DocumentEdit,
}

impl DocumentCommand {
    #[must_use]
    pub const fn new(expected_revision: u64, edit: DocumentEdit) -> Self {
        Self {
            expected_revision,
            edit,
        }
    }
}

/// Persistent identities affected by an accepted command.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DocumentCommandEffect {
    CreatedPoint(DesignPointId),
    CreatedScalar(DesignScalarId),
    CreatedCurve(crate::CurveId),
    CreatedContact(ContactId),
    CreatedConstraint(DocumentConstraintId),
    CreatedDimension(DocumentDimensionId),
    CreatedParameter(DocumentParameterId),
    AddedParameterBinding {
        parameter: DocumentParameterId,
        target: DocumentParameterTarget,
    },
    RemovedParameterBinding {
        parameter: DocumentParameterId,
        target: DocumentParameterTarget,
    },
    AddedParameterOutput {
        parameter: DocumentParameterId,
        dimension: DocumentDimensionId,
    },
    RemovedParameterOutput {
        parameter: DocumentParameterId,
        dimension: DocumentDimensionId,
    },
    CreatedRectangle(Box<RectangleIds>),
    CreatedMirroredCurve(Box<MirroredCurveIds>),
    CreatedLineLineFillet(Box<LineLineFilletIds>),
    CreatedCurveCurveFillet(Box<CurveCurveFilletIds>),
    UpdatedPoint(DesignPointId),
    UpdatedScalar(DesignScalarId),
    UpdatedCurve(CurveId),
    UpdatedConicWeightedMiddle(CurveId),
    UpdatedHyperbolaBranch(CurveId),
    InsertedBSplineKnot(DocumentBSplineInsertion),
    InsertedMirroredBSplineKnot(Box<DocumentMirroredBSplineInsertion>),
    InsertedNurbsKnot(DocumentNurbsInsertion),
    UpdatedNurbsWeightGauge(CurveId),
    UpdatedContacts(Vec<ContactId>),
    UpdatedConstraint(DocumentConstraintId),
    UpdatedDimension(DocumentDimensionId),
    UpdatedSource(DocumentSourceId),
    UpdatedGeometryRole(CurveId),
    UpdatedElementUserSuppression(crate::DocumentElementId),
    UpdatedHostConfigurationActivation,
    Deleted(DocumentObjectId),
    Transaction(String),
    Imported,
    Undo,
    Redo,
}

/// Accepted IDs/value and command outcome from one atomic document transaction.
#[derive(Clone, Debug)]
pub struct DocumentTransactionOutcome<T> {
    pub value: Option<T>,
    pub outcome: DocumentCommandOutcome,
}

impl<T> DocumentTransactionOutcome<T> {
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.value.is_some() && self.outcome.accepted()
    }
}

/// Accepted or rejected command attempt. Rejected attempts never mutate history.
#[derive(Clone, Debug)]
pub struct DocumentCommandOutcome {
    pub revision: u64,
    pub effect: Option<DocumentCommandEffect>,
    pub result: DocumentSolveResult,
}

impl DocumentCommandOutcome {
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.effect.is_some() && self.result.accepted()
    }
}

/// Captured and current lifecycle identities for a rejected drag-locality plan.
///
/// The payload remains structured for hosts while its boxed storage keeps
/// [`DocumentSessionError`] small on unrelated failure paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaleDragLocalityPlanEvidence {
    /// Retained-design identity stamped into the frozen plan.
    pub expected_design: SketchDesignIdentity,
    /// Accepted-state identity stamped into the frozen plan.
    pub expected_accepted: SketchAcceptedStateIdentity,
    /// Retained-design identity current at validation time.
    pub actual_design: SketchDesignIdentity,
    /// Accepted-state identity current at validation time, when one exists.
    pub actual_accepted: Option<SketchAcceptedStateIdentity>,
}

impl fmt::Display for StaleDragLocalityPlanEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "captured design {:?} and accepted state {:?}, current design is {:?} and accepted \
             state is {:?}",
            self.expected_design, self.expected_accepted, self.actual_design, self.actual_accepted
        )
    }
}

/// Construction, command, history, or persistence failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentSessionError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    SketchSession(#[from] SketchSessionError),
    #[error(transparent)]
    Sketch(#[from] crate::SketchError),
    #[error("stale document command: expected revision {expected}, accepted revision {actual}")]
    StaleCommand { expected: u64, actual: u64 },
    #[error("there is no accepted command to undo")]
    NothingToUndo,
    #[error("there is no accepted command to redo")]
    NothingToRedo,
    #[error("an accepted history snapshot unexpectedly failed to rebuild")]
    InvalidHistorySnapshot,
    #[error("stale retained-design identity: expected {expected:?}, current {actual:?}")]
    StaleDesign {
        expected: SketchDesignIdentity,
        actual: SketchDesignIdentity,
    },
    #[error("parameter batch revision {actual} is not newer than retained revision {retained}")]
    StaleParameterRevision { actual: u64, retained: u64 },
    #[error(
        "external snapshot set revision {actual} is not newer than retained revision {retained}"
    )]
    StaleExternalSnapshotRevision { actual: u64, retained: u64 },
    #[error("retained design belongs to document {actual}, expected {expected}")]
    ForeignDesign {
        expected: crate::DocumentId,
        actual: crate::DocumentId,
    },
    #[error("{domain} revision space is exhausted")]
    RevisionExhausted { domain: &'static str },
    #[error("the restored accepted document did not produce an independently accepted state")]
    InvalidAcceptedSnapshot,
    #[error("point-move preview belongs to a different document")]
    PreviewForeignDocument,
    #[error("point-move preview does not belong to the current retained design")]
    PreviewStaleDesign,
    #[error("point-move preview does not descend from the current accepted state")]
    PreviewAcceptedProvenance,
    #[error("point-move preview's latest attempt did not publish its accepted state")]
    PreviewNotAccepted,
    #[error("point and position do not bitwise match the preview drag and accepted point")]
    PreviewPointMismatch,
    #[error("branch preview does not exactly match the proposed point and persistent branches")]
    PreviewBranchMismatch,
    #[error("drag-locality planning requires a current independently accepted state")]
    DragLocalityUnavailable,
    #[error("stale drag-locality plan: {evidence}")]
    StaleDragLocalityPlan {
        evidence: Box<StaleDragLocalityPlanEvidence>,
    },
    #[error(
        "drag-locality plan for point {plan_point} does not match request point {request_point:?}"
    )]
    DragLocalityRequestMismatch {
        plan_point: DesignPointId,
        request_point: Option<DesignPointId>,
    },
    #[error("drag-locality plan is invalid: {context}")]
    InvalidDragLocalityPlan { context: &'static str },
    #[error("point-move publication did not produce an independently accepted state")]
    DragPublicationNotAccepted,
    #[error("point-move publication violated frozen drag continuity: {context}")]
    DragPublicationContinuity { context: &'static str },
    #[error("controlled preview publication stopped before commit: {report:?}")]
    PreviewPublicationStopped { report: Box<OperationReport> },
    #[error("stale prepared sketch patch: captured {expected:?}, current {actual:?}")]
    StalePreparedPatch {
        expected: Box<PreparedSketchInput>,
        actual: Box<PreparedSketchInput>,
    },
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    before: SketchDocument,
    after: SketchDocument,
    effect: DocumentCommandEffect,
}

/// Accepted persistent document plus solver session and accepted-only command history.
#[derive(Clone, Debug)]
pub struct SketchDocumentSession {
    document: SketchDocument,
    runtime: SketchSession,
    mappings: DocumentRuntimeMap,
    request: DocumentSolveRequest,
    config: SolverConfig,
    revision: u64,
    history: Vec<HistoryEntry>,
    history_cursor: usize,
    allocator_cursors: BTreeMap<crate::DocumentId, PersistentId>,
    span_allocator_cursors: BTreeMap<crate::DocumentId, BTreeMap<CurveId, u32>>,
}

/// Retained design intent plus separate latest-attempt and accepted solved views.
///
/// Design-tree consumers read [`Self::design_document`]. Solved rendering, accepted
/// audit, measurements, and profiles must read [`Self::accepted_state`]. Optional
/// candidate geometry from [`Self::last_attempt`] is preview evidence only.
#[derive(Clone, Debug)]
pub struct RetainedSketchDocumentSession {
    design: SketchDocument,
    design_identity: SketchDesignIdentity,
    /// Exact process-local identity of the retained design publication.
    ///
    /// Ordinary clones share the token. Every retained design mutation receives a
    /// fresh token even when its numeric revision collides with a divergent clone.
    design_provenance: Arc<()>,
    /// Exact provenance of the design publication immediately preceding `design`.
    ///
    /// Reattempts preserve this link so next-revision branch previews remain
    /// attributable to their authoritative base design.
    parent_design_provenance: Option<Arc<()>>,
    last_attempt: SketchDocumentAttempt,
    accepted: Option<SketchAcceptedDocumentState>,
    accepted_revision_high_water: Option<SketchAcceptedRevision>,
    request: DocumentSolveRequest,
    config: SolverConfig,
    parameter_batch: ParameterBatch,
    external_snapshots: ExternalSnapshotSet,
}

/// Exact immutable session input captured before host-scheduled solve work.
///
/// The attempt input records the current retained design, solve requests, solver
/// policy, activation, parameter, and external-snapshot identities. The latest
/// attempt and accepted/high-water identities close the lifecycle stamp so a
/// candidate computed from an older session can never compare equal after any
/// intervening attempt or accepted publication.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedSketchInput {
    input: SketchAttemptInput,
    latest_attempt: SketchAttemptIdentity,
    accepted: Option<SketchAcceptedStateIdentity>,
    accepted_revision_high_water: Option<SketchAcceptedRevision>,
}

impl PreparedSketchInput {
    #[must_use]
    pub const fn attempt_input(self) -> SketchAttemptInput {
        self.input
    }

    #[must_use]
    pub const fn design_identity(self) -> SketchDesignIdentity {
        self.input.design_identity()
    }

    #[must_use]
    pub const fn latest_attempt_identity(self) -> SketchAttemptIdentity {
        self.latest_attempt
    }

    #[must_use]
    pub const fn accepted_state_identity(self) -> Option<SketchAcceptedStateIdentity> {
        self.accepted
    }

    #[must_use]
    pub const fn accepted_revision_high_water(self) -> Option<SketchAcceptedRevision> {
        self.accepted_revision_high_water
    }
}

/// One operation that may be solved away from the session owner.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PreparedSketchOperation {
    Apply(DocumentEdit),
    Reattempt {
        request: DocumentSolveRequest,
    },
    UpdateParameterBatch {
        batch: ParameterBatch,
        request: DocumentSolveRequest,
    },
    UpdateExternalSnapshotSet {
        snapshots: ExternalSnapshotSet,
        request: DocumentSolveRequest,
    },
}

impl PreparedSketchOperation {
    #[must_use]
    pub const fn kind(&self) -> PreparedSketchOperationKind {
        match self {
            Self::Apply(_) => PreparedSketchOperationKind::Apply,
            Self::Reattempt { .. } => PreparedSketchOperationKind::Reattempt,
            Self::UpdateParameterBatch { .. } => PreparedSketchOperationKind::UpdateParameterBatch,
            Self::UpdateExternalSnapshotSet { .. } => {
                PreparedSketchOperationKind::UpdateExternalSnapshotSet
            }
        }
    }
}

/// Stable characterization of a prepared sketch operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum PreparedSketchOperationKind {
    Apply,
    Reattempt,
    UpdateParameterBatch,
    UpdateExternalSnapshotSet,
}

/// Immutable accepted/design snapshot from which host-scheduled work is prepared.
///
/// Native hosts may clone then move this single-owner value to a worker.
/// Session-bearing snapshots/jobs/patches are `Send` but deliberately not promised
/// `Sync`: core solver caches use safe single-owner interior mutability. Immutable
/// stamps and operation DTOs are `Send + Sync`. Single-threaded WASM hosts use the
/// same synchronous [`Self::prepare`] and [`PreparedSketchJob::execute`] boundary
/// without spawning a thread.
#[derive(Clone, Debug)]
pub struct PreparedSketchSnapshot {
    input: PreparedSketchInput,
    session: RetainedSketchDocumentSession,
}

impl PreparedSketchSnapshot {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.input
    }

    #[must_use]
    pub const fn design_document(&self) -> &SketchDocument {
        self.session.design_document()
    }

    #[must_use]
    pub const fn accepted_state(&self) -> Option<&SketchAcceptedDocumentState> {
        self.session.accepted_state()
    }

    #[must_use]
    pub fn prepare(self, operation: PreparedSketchOperation) -> PreparedSketchJob {
        PreparedSketchJob {
            snapshot: self,
            operation,
        }
    }
}

/// One immutable-input job ready for host-managed execution.
#[derive(Debug)]
pub struct PreparedSketchJob {
    snapshot: PreparedSketchSnapshot,
    operation: PreparedSketchOperation,
}

impl PreparedSketchJob {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.snapshot.input
    }

    #[must_use]
    pub const fn operation(&self) -> &PreparedSketchOperation {
        &self.operation
    }

    /// Executes entirely on the captured session clone.
    ///
    /// Completion returns a non-mutating candidate patch. Cancellation or work
    /// exhaustion returns no patch and therefore cannot publish any lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns the ordinary validation, revision, lowering, or solve-setup error for
    /// the prepared operation. The live owning session is never touched.
    pub fn execute(
        self,
        control: OperationControl,
    ) -> Result<OperationOutcome<PreparedSketchPatch>, DocumentSessionError> {
        let Self {
            snapshot,
            operation,
        } = self;
        let base = snapshot.input;
        let mut candidate = snapshot.session;
        let kind = operation.kind();
        let outcome = match operation {
            PreparedSketchOperation::Apply(edit) => candidate
                .apply_controlled(base.design_identity(), edit, control)?
                .map(|outcome| {
                    PreparedSketchCommit::new(
                        kind,
                        outcome.design_identity(),
                        outcome.attempt_identity(),
                        outcome.published_accepted_identity(),
                    )
                }),
            PreparedSketchOperation::Reattempt { request } => candidate
                .reattempt_controlled(base.design_identity(), request, control)?
                .map(|attempt| {
                    PreparedSketchCommit::new(
                        kind,
                        attempt.design_identity(),
                        attempt.identity(),
                        attempt.accepted_state_identity(),
                    )
                }),
            PreparedSketchOperation::UpdateParameterBatch { batch, request } => candidate
                .update_parameter_batch_controlled(base.design_identity(), batch, request, control)?
                .map(|attempt| {
                    PreparedSketchCommit::new(
                        kind,
                        attempt.design_identity(),
                        attempt.identity(),
                        attempt.accepted_state_identity(),
                    )
                }),
            PreparedSketchOperation::UpdateExternalSnapshotSet { snapshots, request } => candidate
                .update_external_snapshot_set_controlled(
                    base.design_identity(),
                    snapshots,
                    request,
                    control,
                )?
                .map(|attempt| {
                    PreparedSketchCommit::new(
                        kind,
                        attempt.design_identity(),
                        attempt.identity(),
                        attempt.accepted_state_identity(),
                    )
                }),
        };
        Ok(outcome.map(|commit| PreparedSketchPatch {
            base,
            commit,
            candidate,
        }))
    }
}

/// Non-mutating candidate session produced by one completed prepared job.
///
/// This value has no public candidate-session accessor. It can only be inspected
/// through its stamps and consumed by
/// [`RetainedSketchDocumentSession::commit_prepared_patch`].
#[derive(Debug)]
pub struct PreparedSketchPatch {
    base: PreparedSketchInput,
    commit: PreparedSketchCommit,
    candidate: RetainedSketchDocumentSession,
}

impl PreparedSketchPatch {
    #[must_use]
    pub const fn base_input(&self) -> PreparedSketchInput {
        self.base
    }

    #[must_use]
    pub const fn proposed_commit(&self) -> PreparedSketchCommit {
        self.commit
    }
}

/// Identities published when one prepared patch wins compare-and-swap commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedSketchCommit {
    operation: PreparedSketchOperationKind,
    design: SketchDesignIdentity,
    attempt: SketchAttemptIdentity,
    accepted: Option<SketchAcceptedStateIdentity>,
}

impl PreparedSketchCommit {
    const fn new(
        operation: PreparedSketchOperationKind,
        design: SketchDesignIdentity,
        attempt: SketchAttemptIdentity,
        accepted: Option<SketchAcceptedStateIdentity>,
    ) -> Self {
        Self {
            operation,
            design,
            attempt,
            accepted,
        }
    }

    #[must_use]
    pub const fn operation(self) -> PreparedSketchOperationKind {
        self.operation
    }

    #[must_use]
    pub const fn design_identity(self) -> SketchDesignIdentity {
        self.design
    }

    #[must_use]
    pub const fn attempt_identity(self) -> SketchAttemptIdentity {
        self.attempt
    }

    #[must_use]
    pub const fn accepted_state_identity(self) -> Option<SketchAcceptedStateIdentity> {
        self.accepted
    }
}

impl SketchDocumentSession {
    /// Builds the first independently validated accepted document revision.
    ///
    /// # Errors
    ///
    /// Returns document/lowering/session errors or an initial solve rejection.
    pub fn new(
        document: SketchDocument,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentSessionError> {
        let lowered = document.lower()?;
        let (sketch, mappings) = lowered.into_parts();
        let runtime_request = lower_request(request, &mappings)?;
        let runtime = SketchSession::new(sketch, runtime_request, config)?;
        let mut document = document;
        document.project_accepted_state(runtime.sketch(), &mappings)?;
        let allocator_cursors = BTreeMap::from([(document.id(), document.allocator_cursor())]);
        let span_allocator_cursors =
            BTreeMap::from([(document.id(), document.spline_span_allocator_cursors())]);
        Ok(Self {
            document,
            runtime,
            mappings,
            request,
            config,
            revision: 0,
            history: Vec::new(),
            history_cursor: 0,
            allocator_cursors,
            span_allocator_cursors,
        })
    }

    /// Builds the first accepted document/session revision under operation control.
    ///
    /// Construction and projection use scratch state. An interrupted outcome
    /// contains no partially constructed session.
    ///
    /// # Errors
    ///
    /// Returns document/lowering/session errors or an initial solve rejection.
    pub fn new_controlled(
        document: SketchDocument,
        request: DocumentSolveRequest,
        config: SolverConfig,
        control: geosolve_core::OperationControl,
    ) -> Result<geosolve_core::OperationOutcome<Self>, DocumentSessionError> {
        let mut controller = OperationController::new(control);
        let Some(lowered) = document.lower_with_controller(&mut controller)? else {
            return Ok(controller.outcome_unchecked());
        };
        let (sketch, mappings) = lowered.into_parts();
        let runtime_request = lower_request(request, &mappings)?;
        let Some(runtime) =
            SketchSession::new_with_controller(sketch, runtime_request, config, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let mut document = document;
        let projected = document.project_accepted_state_with_controller(
            runtime.sketch(),
            &mappings,
            &mut controller,
        );
        if matches!(projected, Ok(false)) {
            return Ok(controller.outcome_unchecked());
        }
        if controller
            .checkpoint(OperationCheckpoint::AfterFinalValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        projected?;
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let allocator_cursors = BTreeMap::from([(document.id(), document.allocator_cursor())]);
        let span_allocator_cursors =
            BTreeMap::from([(document.id(), document.spline_span_allocator_cursors())]);
        Ok(controller.outcome(Self {
            document,
            runtime,
            mappings,
            request,
            config,
            revision: 0,
            history: Vec::new(),
            history_cursor: 0,
            allocator_cursors,
            span_allocator_cursors,
        }))
    }

    #[must_use]
    pub const fn document(&self) -> &SketchDocument {
        &self.document
    }

    #[must_use]
    pub const fn runtime(&self) -> &SketchSession {
        &self.runtime
    }

    #[must_use]
    pub const fn mappings(&self) -> &DocumentRuntimeMap {
        &self.mappings
    }

    #[must_use]
    pub const fn request(&self) -> DocumentSolveRequest {
        self.request
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    #[must_use]
    pub const fn history_cursor(&self) -> usize {
        self.history_cursor
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history_cursor > 0
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history_cursor < self.history.len()
    }

    #[must_use]
    pub fn accepted_result(&self) -> DocumentSolveResult {
        DocumentSolveResult::new(
            self.runtime.accepted_result().clone(),
            self.mappings.clone(),
        )
    }

    /// Rebuilds a transient drag/request without adding a command-history entry.
    ///
    /// Rejected requests retain the prior accepted document, request, revision, and history.
    ///
    /// # Errors
    ///
    /// Returns a stale revision, persistent-ID mapping, lowering, or solver-start error.
    pub fn rebuild_request(
        &mut self,
        expected_revision: u64,
        request: DocumentSolveRequest,
    ) -> Result<DocumentSolveResult, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let attempt = attempt_document(&self.document, request, None, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(result);
        };
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(result)
    }

    /// Controlled counterpart to [`Self::rebuild_request`].
    ///
    /// # Errors
    ///
    /// Returns the same setup errors as [`Self::rebuild_request`].
    pub fn rebuild_request_controlled(
        &mut self,
        expected_revision: u64,
        request: DocumentSolveRequest,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentSolveResult>, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let mut controller = OperationController::new(control);
        let Some(attempt) = attempt_document_controlled(
            &self.document,
            request,
            None,
            self.config,
            &mut controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(controller.outcome(result));
        };
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(controller.outcome(result))
    }

    /// Applies one command by clone, solve, independent validation, and atomic swap.
    ///
    /// Numerical rejection is returned in the outcome and leaves accepted state/history unchanged.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, edit-validation, lowering, or solver-start error.
    pub fn apply(
        &mut self,
        command: DocumentCommand,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(command.expected_revision)?;
        let before = self.document.clone();
        let mut candidate = before.clone();
        self.advance_candidate_allocator(&mut candidate);
        let (effect, command_drag) = match command.edit {
            DocumentEdit::SetPointPosition { point, position } => {
                let mut target_candidate = candidate.clone();
                target_candidate.set_point_position(point, position)?;
                (
                    DocumentCommandEffect::UpdatedPoint(point),
                    Some(DocumentDragTarget {
                        point,
                        target: position,
                    }),
                )
            }
            edit => (apply_edit(&mut candidate, edit)?, None),
        };
        let attempt = attempt_document(&candidate, self.request, command_drag, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((accepted_document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            });
        };
        if let Some(drag) = command_drag
            && before
                .point(drag.point)
                .map(|point| point.position.map(f64::to_bits))
                == accepted_document
                    .point(drag.point)
                    .map(|point| point.position.map(f64::to_bits))
        {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            });
        }
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: accepted_document.clone(),
            effect: effect.clone(),
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&accepted_document);
        self.commit(accepted_document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(effect),
            result,
        })
    }

    /// Applies one command under cooperative cancellation and deterministic work limits.
    ///
    /// Interrupted work runs only on a complete session clone, so document,
    /// runtime, revisions, history, geometry, and audit remain unchanged.
    ///
    /// # Errors
    ///
    /// Returns the same stale-revision, edit, lowering, and solver-start errors
    /// as [`Self::apply`].
    pub fn apply_controlled(
        &mut self,
        command: DocumentCommand,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentCommandOutcome>, DocumentSessionError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_revision(command.expected_revision)?;
        let before = self.document.clone();
        let mut candidate = before.clone();
        self.advance_candidate_allocator(&mut candidate);
        candidate.defer_mutation_validation();
        let (effect, command_drag) = match command.edit {
            DocumentEdit::SetPointPosition { point, position } => {
                let mut target_candidate = candidate.clone();
                target_candidate.set_point_position(point, position)?;
                (
                    DocumentCommandEffect::UpdatedPoint(point),
                    Some(DocumentDragTarget {
                        point,
                        target: position,
                    }),
                )
            }
            edit => (apply_edit(&mut candidate, edit)?, None),
        };
        candidate.resume_mutation_validation();
        let Some(attempt) = attempt_document_controlled(
            &candidate,
            self.request,
            command_drag,
            self.config,
            &mut controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((accepted_document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(controller.outcome(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            }));
        };
        if let Some(drag) = command_drag
            && before
                .point(drag.point)
                .map(|point| point.position.map(f64::to_bits))
                == accepted_document
                    .point(drag.point)
                    .map(|point| point.position.map(f64::to_bits))
        {
            self.retain_accepted_view(&mut result);
            return Ok(controller.outcome(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            }));
        }
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: accepted_document.clone(),
            effect: effect.clone(),
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&accepted_document);
        self.commit(accepted_document, runtime, mappings);
        Ok(controller.outcome(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(effect),
            result,
        }))
    }

    /// Applies a compound document edit to one clone, solve, and history entry.
    ///
    /// The callback may use the public [`SketchDocument`] construction/editing API. Its
    /// returned value is published only when the complete candidate is independently accepted.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, invalid label/edit, lowering, or solver-start error.
    pub fn transact<T, F>(
        &mut self,
        expected_revision: u64,
        label: impl Into<String>,
        edit: F,
    ) -> Result<DocumentTransactionOutcome<T>, DocumentSessionError>
    where
        F: FnOnce(&mut SketchDocument) -> Result<T, DocumentError>,
    {
        self.check_revision(expected_revision)?;
        let label = label.into();
        if label.trim().is_empty() || label.len() > crate::MAX_LABEL_BYTES {
            return Err(DocumentError::InvalidField {
                field: "transaction label",
                message: format!("must contain 1..={} bytes", crate::MAX_LABEL_BYTES),
            }
            .into());
        }
        let before = self.document.clone();
        let mut candidate = before.clone();
        self.advance_candidate_allocator(&mut candidate);
        let value = edit(&mut candidate)?;
        let attempt = attempt_document(&candidate, self.request, None, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let effect = DocumentCommandEffect::Transaction(label);
        let Some((accepted_document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentTransactionOutcome {
                value: None,
                outcome: DocumentCommandOutcome {
                    revision: self.revision,
                    effect: None,
                    result,
                },
            });
        };
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: accepted_document.clone(),
            effect: effect.clone(),
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&accepted_document);
        self.commit(accepted_document, runtime, mappings);
        Ok(DocumentTransactionOutcome {
            value: Some(value),
            outcome: DocumentCommandOutcome {
                revision: self.revision,
                effect: Some(effect),
                result,
            },
        })
    }

    /// Controlled counterpart to [`Self::transact`].
    ///
    /// # Errors
    ///
    /// Returns the same edit and solve setup errors as [`Self::transact`].
    pub fn transact_controlled<T, F>(
        &mut self,
        expected_revision: u64,
        label: impl Into<String>,
        edit: F,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentTransactionOutcome<T>>, DocumentSessionError>
    where
        F: FnOnce(&mut SketchDocument) -> Result<T, DocumentError>,
    {
        self.check_revision(expected_revision)?;
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let label = label.into();
        if label.trim().is_empty() || label.len() > crate::MAX_LABEL_BYTES {
            return Err(DocumentError::InvalidField {
                field: "transaction label",
                message: format!("must contain 1..={} bytes", crate::MAX_LABEL_BYTES),
            }
            .into());
        }
        let before = self.document.clone();
        let mut candidate = before.clone();
        self.advance_candidate_allocator(&mut candidate);
        candidate.defer_mutation_validation();
        let value = edit(&mut candidate)?;
        candidate.resume_mutation_validation();
        let Some(attempt) = attempt_document_controlled(
            &candidate,
            self.request,
            None,
            self.config,
            &mut controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let effect = DocumentCommandEffect::Transaction(label);
        let Some((accepted_document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(controller.outcome(DocumentTransactionOutcome {
                value: None,
                outcome: DocumentCommandOutcome {
                    revision: self.revision,
                    effect: None,
                    result,
                },
            }));
        };
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: accepted_document.clone(),
            effect: effect.clone(),
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&accepted_document);
        self.commit(accepted_document, runtime, mappings);
        Ok(controller.outcome(DocumentTransactionOutcome {
            value: Some(value),
            outcome: DocumentCommandOutcome {
                revision: self.revision,
                effect: Some(effect),
                result,
            },
        }))
    }

    /// Restores the snapshot before the most recent accepted command.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, empty-history, or unexpected rebuild error.
    pub fn undo(
        &mut self,
        expected_revision: u64,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let index = self
            .history_cursor
            .checked_sub(1)
            .ok_or(DocumentSessionError::NothingToUndo)?;
        let mut candidate = self.history[index].before.clone();
        self.advance_candidate_allocator(&mut candidate);
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let attempt = attempt_document(&candidate, request, None, self.config)?;
        let (document, runtime, mappings) = attempt
            .accepted
            .ok_or(DocumentSessionError::InvalidHistorySnapshot)?;
        self.history_cursor = index;
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Undo),
            result: attempt.result,
        })
    }

    /// Controlled counterpart to [`Self::undo`].
    ///
    /// # Errors
    ///
    /// Returns the same history and solve setup errors as [`Self::undo`].
    pub fn undo_controlled(
        &mut self,
        expected_revision: u64,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentCommandOutcome>, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let index = self
            .history_cursor
            .checked_sub(1)
            .ok_or(DocumentSessionError::NothingToUndo)?;
        let mut candidate = self.history[index].before.clone();
        self.advance_candidate_allocator(&mut candidate);
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let mut controller = OperationController::new(control);
        let Some(attempt) =
            attempt_document_controlled(&candidate, request, None, self.config, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let (document, runtime, mappings) = attempt
            .accepted
            .ok_or(DocumentSessionError::InvalidHistorySnapshot)?;
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.history_cursor = index;
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(controller.outcome(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Undo),
            result: attempt.result,
        }))
    }

    /// Reapplies the next accepted command snapshot.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, exhausted-redo, or unexpected rebuild error.
    pub fn redo(
        &mut self,
        expected_revision: u64,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let entry = self
            .history
            .get(self.history_cursor)
            .ok_or(DocumentSessionError::NothingToRedo)?;
        let mut candidate = entry.after.clone();
        self.advance_candidate_allocator(&mut candidate);
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let attempt = attempt_document(&candidate, request, None, self.config)?;
        let (document, runtime, mappings) = attempt
            .accepted
            .ok_or(DocumentSessionError::InvalidHistorySnapshot)?;
        self.history_cursor += 1;
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Redo),
            result: attempt.result,
        })
    }

    /// Controlled counterpart to [`Self::redo`].
    ///
    /// # Errors
    ///
    /// Returns the same history and solve setup errors as [`Self::redo`].
    pub fn redo_controlled(
        &mut self,
        expected_revision: u64,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentCommandOutcome>, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let entry = self
            .history
            .get(self.history_cursor)
            .ok_or(DocumentSessionError::NothingToRedo)?;
        let mut candidate = entry.after.clone();
        self.advance_candidate_allocator(&mut candidate);
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let mut controller = OperationController::new(control);
        let Some(attempt) =
            attempt_document_controlled(&candidate, request, None, self.config, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let (document, runtime, mappings) = attempt
            .accepted
            .ok_or(DocumentSessionError::InvalidHistorySnapshot)?;
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.history_cursor += 1;
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(controller.outcome(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Redo),
            result: attempt.result,
        }))
    }

    /// Imports a complete candidate atomically and records only an accepted import.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, JSON, validation, lowering, or solver-start error.
    pub fn import_json(
        &mut self,
        expected_revision: u64,
        json: &str,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let mut candidate = SketchDocument::from_json(json)?;
        self.advance_candidate_allocator(&mut candidate);
        let before = self.document.clone();
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let attempt = attempt_document(&candidate, request, None, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            });
        };
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: document.clone(),
            effect: DocumentCommandEffect::Imported,
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&document);
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Imported),
            result,
        })
    }

    /// Controlled counterpart to [`Self::import_json`].
    ///
    /// # Errors
    ///
    /// Returns the same import and solve setup errors as [`Self::import_json`].
    pub fn import_json_controlled(
        &mut self,
        expected_revision: u64,
        json: &str,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentCommandOutcome>, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let mut controller = OperationController::new(control);
        let Some(mut candidate) = SketchDocument::from_json_with_controller(json, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        self.advance_candidate_allocator(&mut candidate);
        let before = self.document.clone();
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let Some(attempt) =
            attempt_document_controlled(&candidate, request, None, self.config, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(controller.outcome(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            }));
        };
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: document.clone(),
            effect: DocumentCommandEffect::Imported,
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&document);
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(controller.outcome(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Imported),
            result,
        }))
    }

    /// Exports the accepted document in canonical deterministic form.
    ///
    /// # Errors
    ///
    /// Returns a document validation or JSON serialization error.
    pub fn export_json(&self) -> Result<String, DocumentError> {
        self.document.to_canonical_json()
    }

    /// Returns the original effect at one accepted history position.
    #[must_use]
    pub fn history_effect(&self, index: usize) -> Option<&DocumentCommandEffect> {
        self.history.get(index).map(|entry| &entry.effect)
    }

    fn check_revision(&self, expected: u64) -> Result<(), DocumentSessionError> {
        if expected == self.revision {
            Ok(())
        } else {
            Err(DocumentSessionError::StaleCommand {
                expected,
                actual: self.revision,
            })
        }
    }

    fn commit(
        &mut self,
        document: SketchDocument,
        runtime: SketchSession,
        mappings: DocumentRuntimeMap,
    ) {
        self.document = document;
        self.runtime = runtime;
        self.mappings = mappings;
        self.revision = self.revision.saturating_add(1);
    }

    fn retain_accepted_view(&self, result: &mut DocumentSolveResult) {
        result
            .accepted_view
            .clone_from(self.runtime.accepted_result());
        result.mappings.clone_from(&self.mappings);
    }

    fn advance_candidate_allocator(&self, candidate: &mut SketchDocument) {
        if let Some(cursor) = self.allocator_cursors.get(&candidate.id()) {
            candidate.advance_allocator(*cursor);
        }
        if let Some(cursors) = self.span_allocator_cursors.get(&candidate.id()) {
            candidate.advance_spline_span_allocators(cursors);
        }
    }

    fn record_allocator(&mut self, document: &SketchDocument) {
        let cursor = document.allocator_cursor();
        self.allocator_cursors
            .entry(document.id())
            .and_modify(|retained| *retained = (*retained).max(cursor))
            .or_insert(cursor);
        let retained = self
            .span_allocator_cursors
            .entry(document.id())
            .or_default();
        for (curve, cursor) in document.spline_span_allocator_cursors() {
            retained
                .entry(curve)
                .and_modify(|value| *value = (*value).max(cursor))
                .or_insert(cursor);
        }
    }
}

impl RetainedSketchDocumentSession {
    /// Starts a lifecycle from one structurally valid design, which may remain unsolved.
    ///
    /// # Errors
    ///
    /// Rejects malformed design data or invalid solver policy before allocating any
    /// lifecycle identity. Numerical and geometric solve failures become the first
    /// identifiable attempt rather than construction errors.
    pub fn new(
        document: SketchDocument,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentSessionError> {
        Self::new_with_parameter_batch(document, ParameterBatch::default(), request, config)
    }

    /// Starts a lifecycle with one exact immutable M42 host-input batch.
    ///
    /// # Errors
    ///
    /// Rejects malformed design data, host input, or invalid solver policy.
    pub fn new_with_parameter_batch(
        document: SketchDocument,
        parameter_batch: ParameterBatch,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentSessionError> {
        Self::new_at(
            document,
            parameter_batch,
            ExternalSnapshotSet::default(),
            request,
            config,
            0,
            0,
            None,
            0,
        )
    }

    /// Starts a lifecycle with exact immutable parameter and external inputs.
    ///
    /// # Errors
    ///
    /// Rejects malformed document/input state or lifecycle setup failures.
    pub fn new_with_inputs(
        document: SketchDocument,
        parameter_batch: ParameterBatch,
        external_snapshots: ExternalSnapshotSet,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentSessionError> {
        Self::new_at(
            document,
            parameter_batch,
            external_snapshots,
            request,
            config,
            0,
            0,
            None,
            0,
        )
    }

    /// Starts a retained-design lifecycle under operation control.
    ///
    /// All identities, attempts, and accepted state are constructed on scratch
    /// state and are returned only as one completed value.
    ///
    /// # Errors
    ///
    /// Rejects malformed design data or invalid solver policy.
    pub fn new_controlled(
        document: SketchDocument,
        request: DocumentSolveRequest,
        config: SolverConfig,
        control: geosolve_core::OperationControl,
    ) -> Result<geosolve_core::OperationOutcome<Self>, DocumentSessionError> {
        let mut controller = OperationController::new(control);
        if !document.validate_with_controller(Some(&mut controller))? {
            return Ok(controller.outcome_unchecked());
        }
        let config = crate::compiler::acceptance_solver_config(config);
        config.validate().map_err(crate::SketchError::from)?;
        let design_identity = SketchDesignIdentity {
            document: document.id(),
            revision: SketchDesignRevision(0),
        };
        let attempt_identity = SketchAttemptIdentity {
            document: document.id(),
            revision: SketchAttemptRevision(0),
        };
        let design_provenance = Arc::new(());
        let parameter_batch = ParameterBatch::default();
        let external_snapshots = ExternalSnapshotSet::default();
        let input = SketchAttemptInput::for_document_with_parameters(
            &document,
            design_identity,
            request,
            request,
            config,
            &parameter_batch,
            &external_snapshots,
        );
        let Some(execution) = run_retained_attempt_controlled(
            &document,
            &parameter_batch,
            &external_snapshots,
            request,
            None,
            config,
            None,
            &mut controller,
        ) else {
            return Ok(controller.outcome_unchecked());
        };
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let (last_attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &document,
                design_provenance: &design_provenance,
                parent_design_provenance: None,
                input: &input,
                attempt_identity,
                parent_accepted: None,
                next_accepted_revision: Some(0),
            },
            execution,
        );
        let accepted_revision_high_water =
            accepted.as_ref().map(|accepted| accepted.identity.revision);
        Ok(controller.outcome(Self {
            design: document,
            design_identity,
            design_provenance,
            parent_design_provenance: None,
            last_attempt,
            accepted,
            accepted_revision_high_water,
            request,
            config,
            parameter_batch,
            external_snapshots,
        }))
    }

    /// Restores design intent when no prior accepted graph is available.
    ///
    /// The host-owned high-water metadata is advanced before evaluation, so no
    /// design, attempt, or accepted revision from the prior lifecycle is reused.
    ///
    /// # Errors
    ///
    /// Rejects invalid design data, policy, or exhausted revision space.
    pub fn restore_design(
        design: SketchDocument,
        revisions: SketchLifecycleRevisionHighWater,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentSessionError> {
        let design_revision = next_revision(revisions.design.0, "design")?;
        let attempt_revision = next_revision(revisions.attempt.0, "attempt")?;
        let accepted_revision = revisions
            .accepted
            .map_or(Ok(0), |revision| next_revision(revision.0, "accepted"))?;
        Self::new_at(
            design,
            ParameterBatch::default(),
            ExternalSnapshotSet::default(),
            request,
            config,
            design_revision,
            attempt_revision,
            revisions.accepted,
            accepted_revision,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_at(
        document: SketchDocument,
        parameter_batch: ParameterBatch,
        external_snapshots: ExternalSnapshotSet,
        request: DocumentSolveRequest,
        config: SolverConfig,
        design_revision: u64,
        attempt_revision: u64,
        prior_accepted_high_water: Option<SketchAcceptedRevision>,
        accepted_revision: u64,
    ) -> Result<Self, DocumentSessionError> {
        document.validate()?;
        let config = crate::compiler::acceptance_solver_config(config);
        config.validate().map_err(crate::SketchError::from)?;
        let design_identity = SketchDesignIdentity {
            document: document.id(),
            revision: SketchDesignRevision(design_revision),
        };
        let attempt_identity = SketchAttemptIdentity {
            document: document.id(),
            revision: SketchAttemptRevision(attempt_revision),
        };
        let design_provenance = Arc::new(());
        let input = SketchAttemptInput::for_document_with_parameters(
            &document,
            design_identity,
            request,
            request,
            config,
            &parameter_batch,
            &external_snapshots,
        );
        let execution = run_retained_attempt(
            &document,
            &parameter_batch,
            &external_snapshots,
            request,
            None,
            config,
            None,
        );
        let (last_attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &document,
                design_provenance: &design_provenance,
                parent_design_provenance: None,
                input: &input,
                attempt_identity,
                parent_accepted: None,
                next_accepted_revision: Some(accepted_revision),
            },
            execution,
        );
        let accepted_revision_high_water = accepted
            .as_ref()
            .map(|accepted| accepted.identity.revision)
            .or(prior_accepted_high_water);
        Ok(Self {
            design: document,
            design_identity,
            design_provenance,
            parent_design_provenance: None,
            last_attempt,
            accepted,
            accepted_revision_high_water,
            request,
            config,
            parameter_batch,
            external_snapshots,
        })
    }

    /// Restores separate v1-v4 design and accepted graphs into a fresh in-memory lifecycle.
    ///
    /// Lifecycle revisions are intentionally not persisted by frozen sketch v1-v4.
    /// The accepted graph is independently solved first; a distinct retained design is
    /// then attempted as the next design revision. This is not a draft-v5 wire format.
    ///
    /// # Errors
    ///
    /// Rejects either invalid graph, mismatched document identities, invalid policy, or
    /// an accepted snapshot that cannot be independently accepted again.
    pub fn restore_design_with_accepted(
        design: SketchDocument,
        accepted: SketchDocument,
        revisions: SketchLifecycleRevisionHighWater,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentSessionError> {
        design.validate()?;
        accepted.validate()?;
        if design.id() != accepted.id() {
            return Err(DocumentSessionError::ForeignDesign {
                expected: accepted.id(),
                actual: design.id(),
            });
        }
        let accepted_seed = accepted.clone();
        let accepted_bytes = exact_document_bytes(&accepted)?;
        let same_design = exact_document_bytes(&design)? == accepted_bytes;
        let accepted_design_revision = next_revision(revisions.design.0, "design")?;
        let accepted_attempt_revision = next_revision(revisions.attempt.0, "attempt")?;
        let accepted_revision = revisions
            .accepted
            .map_or(Ok(0), |revision| next_revision(revision.0, "accepted"))?;
        let mut session = Self::new_at(
            accepted,
            ParameterBatch::default(),
            ExternalSnapshotSet::default(),
            DocumentSolveRequest::default(),
            config,
            accepted_design_revision,
            accepted_attempt_revision,
            revisions.accepted,
            accepted_revision,
        )?;
        if session.accepted.is_none() {
            return Err(DocumentSessionError::InvalidAcceptedSnapshot);
        }
        session.request = request;
        if same_design {
            let identity = session.design_identity;
            session.reattempt_with_explicit_seed(identity, request, &accepted_seed)?;
        } else {
            session.retain_candidate_with_seed(design, (), None, &accepted_seed)?;
        }
        // Checkpoint replay deliberately strips both interaction-scoped inputs.
        // In that case a newly selected underconstrained solution is not a
        // caller-requested result: restoration must reproduce the validated
        // accepted graph exactly or fail without returning a partial lifecycle.
        let restored_accepted_matches = session
            .accepted
            .as_ref()
            .map(|state| exact_document_bytes(&state.document))
            .transpose()?
            .is_some_and(|bytes| bytes == accepted_bytes);
        if request.drag.is_none()
            && !request.previous_state_preferences
            && !restored_accepted_matches
        {
            return Err(DocumentSessionError::InvalidAcceptedSnapshot);
        }
        Ok(session)
    }

    #[must_use]
    pub const fn design_identity(&self) -> SketchDesignIdentity {
        self.design_identity
    }

    /// Returns authoritative retained intent, whether or not it currently solves.
    #[must_use]
    pub const fn design_document(&self) -> &SketchDocument {
        &self.design
    }

    /// Returns the latest immutable host batch captured for attempts.
    #[must_use]
    pub const fn parameter_batch(&self) -> &ParameterBatch {
        &self.parameter_batch
    }

    /// Returns the exact immutable external set captured for attempts.
    #[must_use]
    pub const fn external_snapshot_set(&self) -> &ExternalSnapshotSet {
        &self.external_snapshots
    }

    /// Returns non-authoritative evidence for the most recent exact attempt.
    #[must_use]
    pub const fn last_attempt(&self) -> &SketchDocumentAttempt {
        &self.last_attempt
    }

    /// Returns the last independently accepted solved state, if one exists.
    #[must_use]
    pub const fn accepted_state(&self) -> Option<&SketchAcceptedDocumentState> {
        self.accepted.as_ref()
    }

    /// Captures deterministic passive-freedom ownership for one projected drag.
    ///
    /// Rank and anchor identities come from the current accepted runtime. Anchor
    /// positions deliberately come from the current accepted document rather than
    /// the runtime's older solve-attempt reference. The returned targets therefore
    /// describe exactly what the user sees when the gesture begins.
    ///
    /// # Errors
    ///
    /// Rejects when the retained design has no matching accepted state, the point
    /// has no accepted runtime identity, or the accepted rank evidence cannot
    /// produce a complete deterministic locality plan.
    pub fn drag_locality_plan(
        &self,
        point: DesignPointId,
    ) -> Result<DocumentDragLocalityPlan, DocumentSessionError> {
        let mut controller = OperationController::new(OperationControl::unlimited());
        self.drag_locality_plan_with_controller(point, &mut controller)?
            .ok_or(DocumentSessionError::DragLocalityUnavailable)
    }

    /// Performs bounded retained-design validation before an interactive host
    /// allocates a deep lifecycle clone.
    ///
    /// `Ok(false)` means operation control stopped the preflight. No lifecycle
    /// identity or state is changed.
    ///
    /// # Errors
    ///
    /// Returns ordinary retained-document validation errors.
    #[doc(hidden)]
    pub fn preflight_design_with_controller(
        &self,
        controller: &mut OperationController,
    ) -> Result<bool, DocumentSessionError> {
        Ok(self.design.validate_with_controller(Some(controller))?)
    }

    #[doc(hidden)]
    pub fn drag_locality_plan_with_controller(
        &self,
        point: DesignPointId,
        controller: &mut OperationController,
    ) -> Result<Option<DocumentDragLocalityPlan>, DocumentSessionError> {
        let accepted = self
            .accepted
            .as_ref()
            .filter(|accepted| accepted.design_identity() == self.design_identity)
            .ok_or(DocumentSessionError::DragLocalityUnavailable)?;
        if accepted.document.point(point).is_none() {
            return Err(DocumentSessionError::InvalidDragLocalityPlan {
                context: "the active point is absent from the accepted document",
            });
        }
        let runtime_point = accepted.mappings.runtime_point(point).ok_or(
            DocumentSessionError::InvalidDragLocalityPlan {
                context: "the active point has no accepted runtime mapping",
            },
        )?;
        let Some(runtime_plan) = accepted
            .runtime
            .drag_locality_plan_with_controller(runtime_point, controller)?
        else {
            return Ok(None);
        };
        let mut anchors = Vec::with_capacity(runtime_plan.anchors.len());
        for runtime_anchor in &runtime_plan.anchors {
            if controller
                .charge(
                    geosolve_core::OperationWorkCounter::DocumentDependencyItems,
                    1,
                    OperationCheckpoint::DocumentDependency,
                )
                .is_err()
            {
                return Ok(None);
            }
            let persistent = accepted
                .mappings
                .persistent_point(runtime_anchor.point)
                .ok_or(DocumentSessionError::InvalidDragLocalityPlan {
                    context: "a runtime anchor has no persistent point mapping",
                })?;
            let target = accepted
                .document
                .point(persistent)
                .ok_or(DocumentSessionError::InvalidDragLocalityPlan {
                    context: "a persistent anchor is absent from the accepted document",
                })?
                .position;
            if !target.iter().all(|value| value.is_finite()) {
                return Err(DocumentSessionError::InvalidDragLocalityPlan {
                    context: "an accepted anchor target is non-finite",
                });
            }
            anchors.push(DocumentDragLocalityAnchor {
                point: persistent,
                target,
                mobility_rank: runtime_anchor.mobility_rank,
            });
        }
        let plan = DocumentDragLocalityPlan {
            design: self.design_identity,
            design_provenance: Arc::clone(&self.design_provenance),
            accepted: accepted.identity,
            accepted_provenance: Arc::clone(&accepted.provenance),
            point,
            hard_degrees_of_freedom: runtime_plan.hard_degrees_of_freedom,
            active_rank: runtime_plan.active_rank,
            passive_degrees_of_freedom: runtime_plan.passive_degrees_of_freedom,
            anchors,
        };
        self.validate_drag_locality_plan(&plan, point)?;
        Ok(Some(plan))
    }

    /// Returns stable persistent-ID diagnostics for the latest attempt.
    #[must_use]
    pub fn latest_attempt_diagnostics(&self) -> crate::SketchDiagnosticSnapshot {
        let attempt = &self.last_attempt;
        let fallback_activity;
        let activity = if let Some(activity) = attempt.effective_activity() {
            activity
        } else {
            fallback_activity = self.design.effective_activity();
            &fallback_activity
        };
        let variable_elements = attempt
            .solve_result()
            .zip(attempt.mappings())
            .map_or_else(BTreeMap::new, |(solve, mappings)| {
                crate::diagnostics::diagnostic_variable_elements(solve, mappings)
            });
        let parameter_issue = attempt
            .failure()
            .and_then(SketchAttemptFailure::parameter_input_issue);
        let external_issue = attempt
            .failure()
            .and_then(SketchAttemptFailure::external_snapshot_error);
        crate::diagnostics::build_diagnostic_snapshot(
            &crate::diagnostics::SketchDiagnosticBuildInput {
                provenance: crate::SketchDiagnosticProvenance::Attempt {
                    attempt: attempt.identity(),
                    design: attempt.design_identity(),
                    parent_accepted: attempt.parent_accepted_identity(),
                },
                input: attempt.input(),
                document: &self.design,
                solve: attempt.solve_result(),
                mappings: attempt.mappings(),
                activity,
                parameter_issue,
                external_issue,
                variable_elements: &variable_elements,
            },
        )
    }

    /// Returns stable persistent-ID diagnostics for the last accepted state.
    #[must_use]
    pub fn accepted_diagnostics(&self) -> Option<crate::SketchDiagnosticSnapshot> {
        self.accepted
            .as_ref()
            .map(SketchAcceptedDocumentState::diagnostics)
    }

    #[must_use]
    pub const fn request(&self) -> DocumentSolveRequest {
        self.request
    }

    /// Returns monotonic counters for an application-owned persistence sidecar.
    #[must_use]
    pub const fn revision_high_water(&self) -> SketchLifecycleRevisionHighWater {
        SketchLifecycleRevisionHighWater {
            design: self.design_identity.revision,
            attempt: self.last_attempt.identity.revision,
            accepted: self.accepted_revision_high_water,
        }
    }

    /// Captures one immutable, exact-input snapshot for host-scheduled work.
    ///
    /// Creating a snapshot performs no solve work and changes no lifecycle identity.
    #[must_use]
    pub fn prepared_snapshot(&self) -> PreparedSketchSnapshot {
        PreparedSketchSnapshot {
            input: self.current_prepared_input(),
            session: self.clone(),
        }
    }

    /// Returns the current complete prepared-work stamp without cloning session state.
    #[must_use]
    pub fn prepared_input(&self) -> PreparedSketchInput {
        self.current_prepared_input()
    }

    /// Commits a completed prepared patch only when its complete captured input
    /// still matches this owning session.
    ///
    /// This is the only prepared-work publication point. Hosts retain exclusive
    /// ownership of the live session, execute [`PreparedSketchJob`] on native
    /// workers or synchronously in single-threaded WASM, then bring the patch back
    /// to this compare-and-swap boundary.
    ///
    /// # Errors
    ///
    /// Returns [`DocumentSessionError::StalePreparedPatch`] without mutation after
    /// any intervening design, attempt, accepted, policy, parameter, activation, or
    /// external-input change.
    pub fn commit_prepared_patch(
        &mut self,
        patch: PreparedSketchPatch,
    ) -> Result<PreparedSketchCommit, DocumentSessionError> {
        let actual = self.current_prepared_input();
        if actual != patch.base {
            return Err(DocumentSessionError::StalePreparedPatch {
                expected: Box::new(patch.base),
                actual: Box::new(actual),
            });
        }
        let commit = patch.commit;
        *self = patch.candidate;
        Ok(commit)
    }

    /// Retains one valid typed edit even when its solve attempt rejects.
    ///
    /// `Ok` means the design transaction was retained. Check
    /// [`RetainedDocumentTransactionOutcome::published_accepted_identity`] to determine
    /// whether this attempt also published a new accepted solved state.
    ///
    /// # Errors
    ///
    /// Rejects a stale identity or malformed/non-finite/referentially invalid edit
    /// before advancing either design or attempt identity.
    pub fn apply(
        &mut self,
        expected: SketchDesignIdentity,
        edit: DocumentEdit,
    ) -> Result<RetainedDocumentTransactionOutcome<DocumentCommandEffect>, DocumentSessionError>
    {
        self.check_design_identity(expected)?;
        let mut candidate = self.design.clone();
        let (effect, command_drag) = match edit {
            DocumentEdit::SetPointPosition { point, position } => {
                candidate.set_point_position(point, position)?;
                (
                    DocumentCommandEffect::UpdatedPoint(point),
                    Some(DocumentDragTarget {
                        point,
                        target: position,
                    }),
                )
            }
            edit => (apply_edit(&mut candidate, edit)?, None),
        };
        candidate.validate()?;
        self.retain_candidate(candidate, effect, command_drag)
    }

    /// Atomically retains a point seed and the complete set of incident line-branch
    /// edits, then attempts that assembly mode under a temporary projected point target.
    ///
    /// This is intended for bounded, non-authoritative branch searches performed on
    /// a cloned session. Publish a chosen result through
    /// [`Self::apply_point_and_curve_branches_from_preview`].
    ///
    /// # Errors
    ///
    /// Rejects stale input, malformed geometry, a missing/duplicate/nonincident
    /// branch edit, or any branch direction incompatible with the seeded coordinates.
    pub fn attempt_point_and_curve_branches(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        branches: &[DocumentCurveBranchEdit],
    ) -> Result<RetainedDocumentTransactionOutcome<()>, DocumentSessionError> {
        self.check_design_identity(expected)?;
        if !branch_edits_cover_incident_lines(&self.design, point, branches) {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        }
        let mut candidate = self.design.clone();
        candidate.set_point_position(point, position)?;
        for branch in branches {
            candidate.set_curve_branch(branch.curve, branch.direction)?;
        }
        candidate.validate()?;
        self.retain_candidate(
            candidate,
            (),
            Some(DocumentDragTarget {
                point,
                target: position,
            }),
        )
    }

    /// Controlled form of [`Self::attempt_point_and_curve_branches`].
    ///
    /// Cancellation or work exhaustion publishes no design, attempt, or accepted
    /// identity. This lets a bounded multi-seed branch search allocate one
    /// aggregate budget across isolated candidate lifecycles.
    ///
    /// # Errors
    ///
    /// Returns the same stale-input, geometry, branch, and session errors as the
    /// uncontrolled form.
    pub fn attempt_point_and_curve_branches_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        branches: &[DocumentCurveBranchEdit],
        control: OperationControl,
    ) -> Result<OperationOutcome<RetainedDocumentTransactionOutcome<()>>, DocumentSessionError>
    {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_design_identity(expected)?;
        if !branch_edits_cover_incident_lines(&self.design, point, branches) {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        }
        let mut candidate = self.design.clone();
        candidate.defer_mutation_validation();
        candidate.set_point_position(point, position)?;
        for branch in branches {
            candidate.set_curve_branch(branch.curve, branch.direction)?;
        }
        candidate.resume_mutation_validation();
        let Some(value) = self.retain_candidate_controlled(
            candidate,
            (),
            Some(DocumentDragTarget {
                point,
                target: position,
            }),
            &mut controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(value))
    }

    /// Attempts an exact branch candidate while using another accepted branch preview only as
    /// its candidate-shaped numerical seed.
    ///
    /// This is the bounded canonicalization seam for a headless alternate-branch search. The
    /// seed preview must be the independently accepted next design revision descended from this
    /// exact in-memory design and accepted parent. Its design must contain only one point seed
    /// and the complete incident line-branch payload. The supplied branch bytes must match that
    /// payload exactly. The new candidate topology, point target, identities, and provenance are
    /// always owned by this session.
    ///
    /// Cancellation or work exhaustion publishes no design, attempt, or accepted identity.
    ///
    /// # Errors
    ///
    /// Rejects stale/foreign/nonaccepted or structurally incoherent seed evidence, malformed
    /// geometry, incomplete incident branch edits, and ordinary controlled-session failures.
    #[doc(hidden)]
    pub fn attempt_point_and_curve_branches_with_preview_seed_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        branches: &[DocumentCurveBranchEdit],
        seed_preview: &Self,
        control: OperationControl,
    ) -> Result<OperationOutcome<RetainedDocumentTransactionOutcome<()>>, DocumentSessionError>
    {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_design_identity(expected)?;
        let seed_accepted = self.validated_branch_seed_preview(point, seed_preview)?;
        if !branch_edits_cover_incident_lines(&self.design, point, branches)
            || branches.iter().any(|branch| {
                seed_preview
                    .design
                    .curve_branch_direction(branch.curve)
                    .is_none_or(|direction| pair_bits(direction) != pair_bits(branch.direction))
            })
        {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        }
        let numerical_seed = seed_accepted.document.clone();
        let mut candidate = self.design.clone();
        candidate.defer_mutation_validation();
        candidate.set_point_position(point, position)?;
        for branch in branches {
            candidate.set_curve_branch(branch.curve, branch.direction)?;
        }
        candidate.resume_mutation_validation();
        let Some(value) = self.retain_candidate_with_seed_controlled(
            candidate,
            (),
            Some(DocumentDragTarget {
                point,
                target: position,
            }),
            &numerical_seed,
            &mut controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(value))
    }

    /// Retains one point-position edit while seeding its final solve from an exact
    /// independently accepted transient preview.
    ///
    /// Only the point edit is retained as design intent. Every solved continuous
    /// value in `preview` seeds the new independent solve; preview lifecycle
    /// identities and temporary request targets are not retained.
    ///
    /// # Errors
    ///
    /// Rejects a stale edit, malformed point position, or preview whose document,
    /// design, parent accepted state, latest attempt, and published accepted state
    /// do not coherently belong to this lifecycle.
    pub fn apply_point_position_from_preview(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        preview: &Self,
    ) -> Result<RetainedDocumentTransactionOutcome<DocumentCommandEffect>, DocumentSessionError>
    {
        completed_preview_publication(self.apply_point_position_from_preview_controlled(
            expected,
            point,
            position,
            preview,
            OperationControl::unlimited(),
        )?)
    }

    /// Controlled form of [`Self::apply_point_position_from_preview`].
    ///
    /// Cancellation or work exhaustion leaves the retained lifecycle unchanged.
    ///
    /// # Errors
    ///
    /// Returns the ordinary preview, document, and solve-setup errors.
    #[doc(hidden)]
    pub fn apply_point_position_from_preview_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        preview: &Self,
        control: OperationControl,
    ) -> Result<
        OperationOutcome<RetainedDocumentTransactionOutcome<DocumentCommandEffect>>,
        DocumentSessionError,
    > {
        self.check_design_identity(expected)?;
        self.validate_preview_for_current_design(preview)?;
        let preview_accepted = preview
            .accepted
            .as_ref()
            .ok_or(DocumentSessionError::PreviewNotAccepted)?;
        if preview
            .last_attempt
            .input()
            .candidate_request()
            .drag
            .map(|drag| drag.point)
            != Some(point)
            || preview_accepted
                .document
                .point(point)
                .map(|accepted| pair_bits(accepted.position))
                != Some(pair_bits(position))
        {
            return Err(DocumentSessionError::PreviewPointMismatch);
        }

        let mut candidate = self.design.clone();
        candidate.defer_mutation_validation();
        candidate.set_point_position(point, position)?;
        candidate.resume_mutation_validation();
        let mut controller = OperationController::new(control);
        let Some(outcome) = self.publish_preview_candidate_controlled(
            PreviewPublicationCandidate {
                candidate,
                value: DocumentCommandEffect::UpdatedPoint(point),
                command_drag: Some(DocumentDragTarget {
                    point,
                    target: position,
                }),
                numerical_seed: &preview_accepted.document,
                previous_state_intent: Some(RetainedPreviousStateIntent::PreviewPublication {
                    preview: &preview_accepted.document,
                    locality: None,
                }),
            },
            &mut controller,
            |publication, outcome, controller| {
                let published_identity = outcome
                    .published_accepted_identity()
                    .ok_or(DocumentSessionError::DragPublicationNotAccepted)?;
                let published = publication
                    .accepted
                    .as_ref()
                    .filter(|accepted| accepted.identity() == published_identity)
                    .ok_or(DocumentSessionError::DragPublicationNotAccepted)?;
                let Some(exact) = documents_have_exact_bytes_controlled(
                    &published.document,
                    &preview_accepted.document,
                    controller,
                )?
                else {
                    return Ok(());
                };
                if exact {
                    Ok(())
                } else {
                    Err(DocumentSessionError::DragPublicationContinuity {
                        context: "published accepted state differs from the visible preview",
                    })
                }
            },
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(outcome))
    }

    /// Atomically publishes a projected point move under its gesture-start locality contract.
    ///
    /// The ordinary preview checks remain mandatory. Publication additionally requires the
    /// preview and independently republished accepted state to preserve every frozen passive
    /// anchor, the active preview position, and all persistent contact branch/parameter state.
    /// All work occurs on a lifecycle clone; any rejection leaves this session unchanged.
    ///
    /// This is a headless coordinator seam. Non-drag preview consumers should use
    /// [`Self::apply_point_position_from_preview`].
    ///
    /// # Errors
    ///
    /// Returns the ordinary preview/locality errors, rejects a publication that does not create
    /// a fresh independently accepted state, or reports the continuity class that changed.
    #[doc(hidden)]
    pub fn apply_point_position_from_preview_with_drag_locality(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        preview: &Self,
        locality: &DocumentDragLocalityPlan,
    ) -> Result<RetainedDocumentTransactionOutcome<DocumentCommandEffect>, DocumentSessionError>
    {
        completed_preview_publication(
            self.apply_point_position_from_preview_with_drag_locality_controlled(
                expected,
                point,
                position,
                preview,
                locality,
                OperationControl::unlimited(),
            )?,
        )
    }

    /// Controlled atomic publication of one projected point preview.
    ///
    /// The command-scoped cursor remains the sole Temporary target and the frozen
    /// locality anchors are the complete Preference set even when the retained
    /// session's ordinary request disables previous-state Preferences.
    ///
    /// Cancellation or work exhaustion leaves design, attempts, accepted state,
    /// and provenance unchanged.
    ///
    /// # Errors
    ///
    /// Returns the ordinary preview/locality and publication-continuity errors.
    #[doc(hidden)]
    pub fn apply_point_position_from_preview_with_drag_locality_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        preview: &Self,
        locality: &DocumentDragLocalityPlan,
        control: OperationControl,
    ) -> Result<
        OperationOutcome<RetainedDocumentTransactionOutcome<DocumentCommandEffect>>,
        DocumentSessionError,
    > {
        self.check_design_identity(expected)?;
        self.validate_drag_locality_plan(locality, point)?;
        self.validate_preview_for_current_design(preview)?;
        let preview_accepted = preview
            .accepted
            .as_ref()
            .ok_or(DocumentSessionError::PreviewNotAccepted)?;
        validate_drag_publication_continuity(
            &preview_accepted.document,
            &preview_accepted.document,
            point,
            position,
            locality,
        )?;

        let mut candidate = self.design.clone();
        candidate.defer_mutation_validation();
        candidate.set_point_position(point, position)?;
        candidate.resume_mutation_validation();
        let mut controller = OperationController::new(control);
        let Some(outcome) = self.publish_preview_candidate_controlled(
            PreviewPublicationCandidate {
                candidate,
                value: DocumentCommandEffect::UpdatedPoint(point),
                command_drag: Some(DocumentDragTarget {
                    point,
                    target: position,
                }),
                numerical_seed: &preview_accepted.document,
                previous_state_intent: Some(RetainedPreviousStateIntent::PreviewPublication {
                    preview: &preview_accepted.document,
                    locality: Some(locality),
                }),
            },
            &mut controller,
            |publication, outcome, controller| {
                let published_identity = outcome
                    .published_accepted_identity()
                    .ok_or(DocumentSessionError::DragPublicationNotAccepted)?;
                let published = publication
                    .accepted
                    .as_ref()
                    .filter(|accepted| accepted.identity() == published_identity)
                    .ok_or(DocumentSessionError::DragPublicationNotAccepted)?;
                validate_drag_publication_continuity(
                    &preview_accepted.document,
                    &published.document,
                    point,
                    position,
                    locality,
                )?;
                let Some(exact) = documents_have_exact_bytes_controlled(
                    &published.document,
                    &preview_accepted.document,
                    controller,
                )?
                else {
                    return Ok(());
                };
                if exact {
                    Ok(())
                } else {
                    Err(DocumentSessionError::DragPublicationContinuity {
                        context: "published accepted state differs from the visible preview",
                    })
                }
            },
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(outcome))
    }

    /// Validates one accepted transient preview against this exact in-memory
    /// retained-design and accepted-parent lineage.
    ///
    /// This is a presentation/coordinator seam, not a persistence identity API.
    /// In particular, equal document/revision numbers from divergent clones do not
    /// establish provenance: design/input content must match and the preview must
    /// carry the shared token of this exact accepted parent.
    ///
    /// # Errors
    ///
    /// Returns the ordinary foreign, stale-design, accepted-provenance, or
    /// nonaccepted preview error without mutation.
    #[doc(hidden)]
    pub fn validate_preview_for_current_design(
        &self,
        preview: &Self,
    ) -> Result<(), DocumentSessionError> {
        if preview.design_identity.document != self.design_identity.document {
            return Err(DocumentSessionError::PreviewForeignDocument);
        }
        if preview.design_identity != self.design_identity
            || preview.last_attempt.design_identity() != self.design_identity
            || !Arc::ptr_eq(&preview.design_provenance, &self.design_provenance)
            || !Arc::ptr_eq(
                &preview.last_attempt.design_provenance,
                &self.design_provenance,
            )
            || !optional_provenance_matches(
                preview.parent_design_provenance.as_ref(),
                self.parent_design_provenance.as_ref(),
            )
            || !optional_provenance_matches(
                preview.last_attempt.parent_design_provenance.as_ref(),
                self.parent_design_provenance.as_ref(),
            )
            || preview.config != self.config
            || preview.parameter_batch != self.parameter_batch
            || preview.external_snapshots != self.external_snapshots
        {
            return Err(DocumentSessionError::PreviewStaleDesign);
        }
        self.validate_preview_parent_provenance(preview)?;
        let Some(preview_accepted) = preview.accepted.as_ref() else {
            return Err(DocumentSessionError::PreviewNotAccepted);
        };
        if preview.last_attempt.failure().is_some()
            || preview
                .last_attempt
                .solve_result()
                .is_none_or(|solve| solve.rejection.is_some())
            || preview.last_attempt.accepted_state_identity() != Some(preview_accepted.identity())
            || preview_accepted.design_identity() != self.design_identity
            || preview_accepted.originating_attempt() != preview.last_attempt.identity()
            || preview_accepted.input() != preview.last_attempt.input()
            || !Arc::ptr_eq(
                &preview_accepted.design_provenance,
                &preview.design_provenance,
            )
        {
            return Err(DocumentSessionError::PreviewNotAccepted);
        }
        Ok(())
    }

    fn validate_preview_parent_provenance(
        &self,
        preview: &Self,
    ) -> Result<(), DocumentSessionError> {
        let identity_matches = preview.last_attempt.parent_accepted_identity()
            == self
                .accepted
                .as_ref()
                .map(SketchAcceptedDocumentState::identity);
        let provenance_matches = match (
            self.accepted.as_ref(),
            preview.last_attempt.parent_accepted_provenance.as_ref(),
        ) {
            (Some(authoritative), Some(preview_parent)) => {
                Arc::ptr_eq(&authoritative.provenance, preview_parent)
            }
            (None, None) => true,
            _ => false,
        };
        if !identity_matches || !provenance_matches {
            return Err(DocumentSessionError::PreviewAcceptedProvenance);
        }
        Ok(())
    }

    /// Atomically accepts one exact independently validated branch-search preview.
    ///
    /// Only the selected point position and the complete incident persistent line
    /// branches become design intent. All continuous values from the preview seed
    /// the independent publication solve, exactly as for an ordinary projected drag
    /// release.
    ///
    /// # Errors
    ///
    /// Rejects stale/foreign/nonaccepted preview evidence, including a candidate that
    /// is not the next design revision descended from the current lifecycle,
    /// mismatched point geometry, or branch edits that do not exactly describe the
    /// preview design.
    pub fn apply_point_and_curve_branches_from_preview(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        branches: &[DocumentCurveBranchEdit],
        preview: &Self,
    ) -> Result<RetainedDocumentTransactionOutcome<()>, DocumentSessionError> {
        completed_preview_publication(self.apply_point_and_curve_branches_from_preview_controlled(
            expected,
            point,
            position,
            branches,
            preview,
            OperationControl::unlimited(),
        )?)
    }

    /// Controlled atomic publication of one exact alternate-branch preview.
    ///
    /// The candidate owns the selected point and persistent branch topology, while
    /// the independently accepted preview contributes only its numerical seed.
    /// Cancellation or work exhaustion leaves the complete retained lifecycle
    /// unchanged.
    ///
    /// # Errors
    ///
    /// Returns the ordinary exact-preview and branch-payload errors.
    #[doc(hidden)]
    pub fn apply_point_and_curve_branches_from_preview_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        point: DesignPointId,
        position: [f64; 2],
        branches: &[DocumentCurveBranchEdit],
        preview: &Self,
        control: OperationControl,
    ) -> Result<OperationOutcome<RetainedDocumentTransactionOutcome<()>>, DocumentSessionError>
    {
        self.check_design_identity(expected)?;
        let preview_accepted = self.validated_branch_preview(point, position, preview)?;
        if !branch_edits_cover_incident_lines(&self.design, point, branches) {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        }

        let mut candidate = self.design.clone();
        candidate.defer_mutation_validation();
        candidate.set_point_position(point, position)?;
        for branch in branches {
            if candidate
                .set_curve_branch(branch.curve, branch.direction)
                .is_err()
            {
                return Err(DocumentSessionError::PreviewBranchMismatch);
            }
        }
        candidate.resume_mutation_validation();
        if !documents_have_exact_bytes(&candidate, &preview.design)? {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        }
        let mut controller = OperationController::new(control);
        let Some(outcome) = self.publish_preview_candidate_controlled(
            PreviewPublicationCandidate {
                candidate,
                value: (),
                command_drag: Some(DocumentDragTarget {
                    point,
                    target: position,
                }),
                numerical_seed: &preview_accepted.document,
                previous_state_intent: Some(RetainedPreviousStateIntent::PreviewPublication {
                    preview: &preview_accepted.document,
                    locality: None,
                }),
            },
            &mut controller,
            |publication, outcome, controller| {
                let Some(published_identity) = outcome.published_accepted_identity() else {
                    return Err(DocumentSessionError::PreviewNotAccepted);
                };
                let Some(published) = publication
                    .accepted
                    .as_ref()
                    .filter(|accepted| accepted.identity() == published_identity)
                else {
                    return Err(DocumentSessionError::PreviewNotAccepted);
                };
                let Some(exact) = documents_have_exact_bytes_controlled(
                    &published.document,
                    &preview_accepted.document,
                    controller,
                )?
                else {
                    return Ok(());
                };
                if !exact {
                    return Err(DocumentSessionError::PreviewBranchMismatch);
                }
                Ok(())
            },
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(outcome))
    }

    fn validated_branch_preview<'a>(
        &self,
        point: DesignPointId,
        position: [f64; 2],
        preview: &'a Self,
    ) -> Result<&'a SketchAcceptedDocumentState, DocumentSessionError> {
        let preview_accepted = self.validated_next_design_branch_preview(preview)?;
        let requested_drag = preview
            .last_attempt
            .input()
            .candidate_request()
            .drag
            .map(|drag| (drag.point, pair_bits(drag.target)));
        if requested_drag != Some((point, pair_bits(position)))
            || preview_accepted
                .document
                .point(point)
                .map(|accepted| pair_bits(accepted.position))
                != Some(pair_bits(position))
        {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        }
        Ok(preview_accepted)
    }

    fn validated_branch_seed_preview<'a>(
        &self,
        point: DesignPointId,
        preview: &'a Self,
    ) -> Result<&'a SketchAcceptedDocumentState, DocumentSessionError> {
        let preview_accepted = self.validated_next_design_branch_preview(preview)?;
        let Some(requested_drag) = preview
            .last_attempt
            .input()
            .candidate_request()
            .drag
            .filter(|drag| {
                drag.point == point && drag.target.iter().all(|value| value.is_finite())
            })
        else {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        };
        let mut expected_preview_design = self.design.clone();
        expected_preview_design.defer_mutation_validation();
        expected_preview_design
            .set_point_position(point, requested_drag.target)
            .map_err(|_| DocumentSessionError::PreviewBranchMismatch)?;
        for span in incident_line_branch_spans(&self.design, point) {
            let direction = preview
                .design
                .curve_branch_direction(span)
                .ok_or(DocumentSessionError::PreviewBranchMismatch)?;
            expected_preview_design
                .set_curve_branch(span, direction)
                .map_err(|_| DocumentSessionError::PreviewBranchMismatch)?;
        }
        expected_preview_design.resume_mutation_validation();
        if !documents_have_exact_bytes(&expected_preview_design, &preview.design)?
            || preview_accepted
                .document
                .point(point)
                .is_none_or(|accepted| !accepted.position.iter().all(|value| value.is_finite()))
        {
            return Err(DocumentSessionError::PreviewBranchMismatch);
        }
        Ok(preview_accepted)
    }

    fn validated_next_design_branch_preview<'a>(
        &self,
        preview: &'a Self,
    ) -> Result<&'a SketchAcceptedDocumentState, DocumentSessionError> {
        if preview.design_identity.document != self.design_identity.document {
            return Err(DocumentSessionError::PreviewForeignDocument);
        }
        let expected_preview_revision = self
            .design_identity
            .revision
            .0
            .checked_add(1)
            .ok_or(DocumentSessionError::RevisionExhausted { domain: "design" })?;
        if preview.design_identity.revision.0 != expected_preview_revision
            || preview.last_attempt.design_identity() != preview.design_identity
            || Arc::ptr_eq(&preview.design_provenance, &self.design_provenance)
            || !Arc::ptr_eq(
                &preview.last_attempt.design_provenance,
                &preview.design_provenance,
            )
            || !preview
                .parent_design_provenance
                .as_ref()
                .is_some_and(|parent| Arc::ptr_eq(parent, &self.design_provenance))
            || !preview
                .last_attempt
                .parent_design_provenance
                .as_ref()
                .is_some_and(|parent| Arc::ptr_eq(parent, &self.design_provenance))
            || preview.config != self.config
            || preview.parameter_batch != self.parameter_batch
            || preview.external_snapshots != self.external_snapshots
        {
            return Err(DocumentSessionError::PreviewStaleDesign);
        }
        self.validate_preview_parent_provenance(preview)?;
        let Some(preview_accepted) = preview.accepted.as_ref() else {
            return Err(DocumentSessionError::PreviewNotAccepted);
        };
        if preview.last_attempt.failure().is_some()
            || preview
                .last_attempt
                .solve_result()
                .is_none_or(|solve| solve.rejection.is_some())
            || preview.last_attempt.accepted_state_identity() != Some(preview_accepted.identity())
            || preview_accepted.design_identity() != preview.design_identity
            || preview_accepted.originating_attempt() != preview.last_attempt.identity()
            || preview_accepted.input() != preview.last_attempt.input()
            || !Arc::ptr_eq(
                &preview_accepted.design_provenance,
                &preview.design_provenance,
            )
        {
            return Err(DocumentSessionError::PreviewNotAccepted);
        }
        Ok(preview_accepted)
    }

    /// Retains and attempts one edit under cooperative operation control.
    ///
    /// Cancellation or work exhaustion advances no design, attempt, or accepted
    /// identity because all work is performed on a lifecycle clone.
    ///
    /// # Errors
    ///
    /// Returns the same stale-design, edit, validation, and solve setup errors
    /// as [`Self::apply`].
    pub fn apply_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        edit: DocumentEdit,
        control: OperationControl,
    ) -> Result<
        OperationOutcome<RetainedDocumentTransactionOutcome<DocumentCommandEffect>>,
        DocumentSessionError,
    > {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_design_identity(expected)?;
        let mut candidate = self.design.clone();
        candidate.defer_mutation_validation();
        let (effect, command_drag) = match edit {
            DocumentEdit::SetPointPosition { point, position } => {
                candidate.set_point_position(point, position)?;
                (
                    DocumentCommandEffect::UpdatedPoint(point),
                    Some(DocumentDragTarget {
                        point,
                        target: position,
                    }),
                )
            }
            edit => (apply_edit(&mut candidate, edit)?, None),
        };
        candidate.resume_mutation_validation();
        let Some(value) =
            self.retain_candidate_controlled(candidate, effect, command_drag, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(value))
    }

    /// Reattempts retained design under cooperative operation control.
    ///
    /// # Errors
    ///
    /// Returns the same stale-design and revision-exhaustion errors as
    /// [`Self::reattempt`].
    pub fn reattempt_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        self.reattempt_with_optional_drag_locality_controlled(expected, request, None, control)
    }

    /// Reattempts one projected pointer sample with a frozen drag-locality plan.
    ///
    /// Exactly one retained solve attempt is published for the sample. Only the
    /// plan's persistent anchors become `PreviousState` Preference rows, and their
    /// gesture-start targets remain unchanged.
    ///
    /// # Errors
    ///
    /// Rejects a stale plan, a request for another point, or any malformed plan
    /// before advancing attempt identity.
    pub fn reattempt_with_drag_locality_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
        locality: &DocumentDragLocalityPlan,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        self.check_design_identity(expected)?;
        self.validate_drag_locality_request(locality, request)?;
        self.reattempt_with_optional_drag_locality_controlled(
            expected,
            request,
            Some(locality),
            control,
        )
    }

    fn reattempt_with_optional_drag_locality_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
        locality: Option<&DocumentDragLocalityPlan>,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_design_identity(expected)?;
        let attempt_identity = self.next_attempt_identity()?;
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &self.parameter_batch,
            &self.external_snapshots,
        );
        let seed = match seed_from_accepted_parent_controlled(
            &self.design,
            self.accepted.as_ref(),
            &mut controller,
        ) {
            Ok(Some(seed)) => seed,
            Ok(None) => return Ok(controller.outcome_unchecked()),
            Err(error) => {
                let execution = RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::AcceptedSession,
                    error.to_string(),
                );
                let (attempt, accepted) = publish_retained_attempt(
                    current_attempt_publication(self, &input, attempt_identity, parent),
                    execution,
                );
                if controller
                    .checkpoint(OperationCheckpoint::BeforeCommit)
                    .is_err()
                {
                    return Ok(controller.outcome_unchecked());
                }
                self.request = request;
                self.last_attempt = attempt.clone();
                debug_assert!(accepted.is_none());
                return Ok(controller.outcome(attempt));
            }
        };
        let Some(execution) = run_retained_attempt_with_previous_state_reference_controlled(
            &seed,
            &self.parameter_batch,
            &self.external_snapshots,
            request,
            None,
            self.config,
            self.accepted.as_ref(),
            locality
                .map(RetainedPreviousStateIntent::DragLocality)
                .or_else(|| same_input_previous_state_intent(parent, &input)),
            &mut controller,
        ) else {
            return Ok(controller.outcome_unchecked());
        };
        let (attempt, accepted) = publish_retained_attempt(
            current_attempt_publication(self, &input, attempt_identity, parent),
            execution,
        );
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.request = request;
        self.last_attempt = attempt.clone();
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(controller.outcome(attempt))
    }

    /// Reattempts retained design while numerically continuing from an accepted preview.
    ///
    /// The preview supplies only the finite accepted geometry and compatible retained runtime
    /// state used as the numerical seed. Publication still descends from this session's
    /// authoritative accepted parent, so the resulting preview remains eligible for an exact
    /// point-position commit through [`Self::apply_point_position_from_preview`].
    ///
    /// # Errors
    ///
    /// Rejects a stale design or a preview whose document, design, parent accepted state, latest
    /// attempt, and published accepted state do not coherently belong to this lifecycle.
    pub fn reattempt_from_accepted_preview_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
        preview: &Self,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        self.reattempt_from_accepted_preview_with_optional_drag_locality_controlled(
            expected, request, preview, None, control,
        )
    }

    /// Continues one projected pointer sample from an accepted preview while
    /// preserving the gesture-start drag-locality plan.
    ///
    /// The preview contributes only a numerical seed. The plan's persistent
    /// anchors and frozen targets remain the sole `PreviousState` Preferences.
    ///
    /// # Errors
    ///
    /// Returns the ordinary preview-provenance errors, plus stale or mismatched
    /// drag-locality errors, before advancing attempt identity.
    pub fn reattempt_from_accepted_preview_with_drag_locality_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
        preview: &Self,
        locality: &DocumentDragLocalityPlan,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        self.check_design_identity(expected)?;
        self.validate_drag_locality_request(locality, request)?;
        self.reattempt_from_accepted_preview_with_optional_drag_locality_controlled(
            expected,
            request,
            preview,
            Some(locality),
            control,
        )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "preview provenance validation and atomic controlled publication remain one lifecycle boundary"
    )]
    fn reattempt_from_accepted_preview_with_optional_drag_locality_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
        preview: &Self,
        locality: Option<&DocumentDragLocalityPlan>,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_design_identity(expected)?;
        self.validate_preview_for_current_design(preview)?;
        let preview_accepted = preview
            .accepted
            .as_ref()
            .ok_or(DocumentSessionError::PreviewNotAccepted)?;
        let parent = self.accepted.as_ref();

        let attempt_identity = self.next_attempt_identity()?;
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &self.parameter_batch,
            &self.external_snapshots,
        );
        let seed = match seed_from_accepted_parent_controlled(
            &self.design,
            Some(preview_accepted),
            &mut controller,
        ) {
            Ok(Some(seed)) => seed,
            Ok(None) => return Ok(controller.outcome_unchecked()),
            Err(error) => {
                let execution = RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::AcceptedSession,
                    error.to_string(),
                );
                let (attempt, accepted) = publish_retained_attempt(
                    current_attempt_publication(self, &input, attempt_identity, parent),
                    execution,
                );
                if controller
                    .checkpoint(OperationCheckpoint::BeforeCommit)
                    .is_err()
                {
                    return Ok(controller.outcome_unchecked());
                }
                self.request = request;
                self.last_attempt = attempt.clone();
                debug_assert!(accepted.is_none());
                return Ok(controller.outcome(attempt));
            }
        };
        let Some(execution) = run_retained_attempt_with_previous_state_reference_controlled(
            &seed,
            &self.parameter_batch,
            &self.external_snapshots,
            request,
            None,
            self.config,
            Some(preview_accepted),
            locality.map_or_else(
                || {
                    self.accepted
                        .as_ref()
                        .map(RetainedPreviousStateIntent::AcceptedRuntimeReference)
                },
                |plan| Some(RetainedPreviousStateIntent::DragLocality(plan)),
            ),
            &mut controller,
        ) else {
            return Ok(controller.outcome_unchecked());
        };
        let (attempt, accepted) = publish_retained_attempt(
            current_attempt_publication(self, &input, attempt_identity, parent),
            execution,
        );
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.request = request;
        self.last_attempt = attempt.clone();
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(controller.outcome(attempt))
    }

    /// Retains a compound design transaction and attempts its complete resulting graph.
    ///
    /// # Errors
    ///
    /// The callback or final document validation may reject before lifecycle identities
    /// advance. A solve rejection is returned as a successful retained transaction.
    pub fn transact<T, F>(
        &mut self,
        expected: SketchDesignIdentity,
        edit: F,
    ) -> Result<RetainedDocumentTransactionOutcome<T>, DocumentSessionError>
    where
        F: FnOnce(&mut SketchDocument) -> Result<T, DocumentError>,
    {
        self.check_design_identity(expected)?;
        let mut candidate = self.design.clone();
        let value = edit(&mut candidate)?;
        candidate.validate()?;
        self.retain_candidate(candidate, value, None)
    }

    /// Controlled counterpart to [`Self::transact`].
    ///
    /// # Errors
    ///
    /// Returns the same edit and solve setup errors as [`Self::transact`].
    pub fn transact_controlled<T, F>(
        &mut self,
        expected: SketchDesignIdentity,
        edit: F,
        control: OperationControl,
    ) -> Result<OperationOutcome<RetainedDocumentTransactionOutcome<T>>, DocumentSessionError>
    where
        F: FnOnce(&mut SketchDocument) -> Result<T, DocumentError>,
    {
        self.check_design_identity(expected)?;
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let mut candidate = self.design.clone();
        candidate.defer_mutation_validation();
        let value = edit(&mut candidate)?;
        candidate.resume_mutation_validation();
        let Some(outcome) =
            self.retain_candidate_controlled(candidate, value, None, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(outcome))
    }

    /// Attempts the current design again without allocating a design revision.
    ///
    /// # Errors
    ///
    /// Rejects a stale design identity or exhausted attempt revision space.
    pub fn reattempt(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
    ) -> Result<&SketchDocumentAttempt, DocumentSessionError> {
        self.check_design_identity(expected)?;
        let attempt_identity = self.next_attempt_identity()?;
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &self.parameter_batch,
            &self.external_snapshots,
        );
        let previous_state_intent = same_input_previous_state_intent(parent, &input);
        let execution = match seed_from_accepted_parent(&self.design, self.accepted.as_ref()) {
            Ok(seed) => run_retained_attempt_with_previous_state_reference(
                &seed,
                &self.parameter_batch,
                &self.external_snapshots,
                request,
                None,
                self.config,
                self.accepted.as_ref(),
                previous_state_intent,
            ),
            Err(error) => RetainedAttemptExecution::failure(
                SketchAttemptFailureKind::AcceptedSession,
                error.to_string(),
            ),
        };
        let (attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &self.design,
                design_provenance: &self.design_provenance,
                parent_design_provenance: self.parent_design_provenance.as_ref(),
                input: &input,
                attempt_identity,
                parent_accepted: parent,
                next_accepted_revision: next_accepted_revision(self.accepted_revision_high_water),
            },
            execution,
        );
        self.request = request;
        self.last_attempt = attempt;
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(&self.last_attempt)
    }

    fn reattempt_with_explicit_seed(
        &mut self,
        expected: SketchDesignIdentity,
        request: DocumentSolveRequest,
        seed: &SketchDocument,
    ) -> Result<&SketchDocumentAttempt, DocumentSessionError> {
        self.check_design_identity(expected)?;
        if seed.id() != self.design_identity.document {
            return Err(DocumentSessionError::ForeignDesign {
                expected: self.design_identity.document,
                actual: seed.id(),
            });
        }
        seed.validate()?;
        let attempt_identity = self.next_attempt_identity()?;
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &self.parameter_batch,
            &self.external_snapshots,
        );
        let execution = run_retained_attempt(
            seed,
            &self.parameter_batch,
            &self.external_snapshots,
            request,
            None,
            self.config,
            self.accepted.as_ref(),
        );
        let (attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &self.design,
                design_provenance: &self.design_provenance,
                parent_design_provenance: self.parent_design_provenance.as_ref(),
                input: &input,
                attempt_identity,
                parent_accepted: parent,
                next_accepted_revision: next_accepted_revision(self.accepted_revision_high_water),
            },
            execution,
        );
        self.request = request;
        self.last_attempt = attempt;
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(&self.last_attempt)
    }

    /// Attempts retained design with a new exact host parameter batch.
    ///
    /// Changed payloads require a strictly newer host revision. An exact immutable
    /// reattempt may reuse the current batch and revision.
    ///
    /// # Errors
    ///
    /// Rejects a stale design identity, stale batch revision, or exhausted
    /// lifecycle revision space.
    pub fn update_parameter_batch(
        &mut self,
        expected: SketchDesignIdentity,
        batch: ParameterBatch,
        request: DocumentSolveRequest,
    ) -> Result<&SketchDocumentAttempt, DocumentSessionError> {
        self.check_design_identity(expected)?;
        if batch != self.parameter_batch && batch.revision() <= self.parameter_batch.revision() {
            return Err(DocumentSessionError::StaleParameterRevision {
                actual: batch.revision(),
                retained: self.parameter_batch.revision(),
            });
        }
        let attempt_identity = self.next_attempt_identity()?;
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &batch,
            &self.external_snapshots,
        );
        let execution = match seed_from_accepted_parent(&self.design, self.accepted.as_ref()) {
            Ok(seed) => run_retained_attempt(
                &seed,
                &batch,
                &self.external_snapshots,
                request,
                None,
                self.config,
                self.accepted.as_ref(),
            ),
            Err(error) => RetainedAttemptExecution::failure(
                SketchAttemptFailureKind::AcceptedSession,
                error.to_string(),
            ),
        };
        let (attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &self.design,
                design_provenance: &self.design_provenance,
                parent_design_provenance: self.parent_design_provenance.as_ref(),
                input: &input,
                attempt_identity,
                parent_accepted: parent,
                next_accepted_revision: next_accepted_revision(self.accepted_revision_high_water),
            },
            execution,
        );
        self.parameter_batch = batch;
        self.request = request;
        self.last_attempt = attempt;
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(&self.last_attempt)
    }

    /// Controlled counterpart to [`Self::update_parameter_batch`].
    ///
    /// Cancellation and work exhaustion leave the parameter batch, request, and
    /// every lifecycle identity unchanged.
    ///
    /// # Errors
    ///
    /// Rejects stale design/batch revisions or lifecycle revision exhaustion.
    pub fn update_parameter_batch_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        batch: ParameterBatch,
        request: DocumentSolveRequest,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_design_identity(expected)?;
        if batch != self.parameter_batch && batch.revision() <= self.parameter_batch.revision() {
            return Err(DocumentSessionError::StaleParameterRevision {
                actual: batch.revision(),
                retained: self.parameter_batch.revision(),
            });
        }
        let attempt_identity = self.next_attempt_identity()?;
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &batch,
            &self.external_snapshots,
        );
        let seed = match seed_from_accepted_parent_controlled(
            &self.design,
            self.accepted.as_ref(),
            &mut controller,
        ) {
            Ok(Some(seed)) => seed,
            Ok(None) => return Ok(controller.outcome_unchecked()),
            Err(error) => {
                let execution = RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::AcceptedSession,
                    error.to_string(),
                );
                let (attempt, accepted) = publish_retained_attempt(
                    current_attempt_publication(self, &input, attempt_identity, parent),
                    execution,
                );
                if controller
                    .checkpoint(OperationCheckpoint::BeforeCommit)
                    .is_err()
                {
                    return Ok(controller.outcome_unchecked());
                }
                self.parameter_batch = batch;
                self.request = request;
                self.last_attempt = attempt.clone();
                debug_assert!(accepted.is_none());
                return Ok(controller.outcome(attempt));
            }
        };
        let Some(execution) = run_retained_attempt_controlled(
            &seed,
            &batch,
            &self.external_snapshots,
            request,
            None,
            self.config,
            self.accepted.as_ref(),
            &mut controller,
        ) else {
            return Ok(controller.outcome_unchecked());
        };
        let (attempt, accepted) = publish_retained_attempt(
            current_attempt_publication(self, &input, attempt_identity, parent),
            execution,
        );
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.parameter_batch = batch;
        self.request = request;
        self.last_attempt = attempt.clone();
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(controller.outcome(attempt))
    }

    /// Attempts retained design with a newer immutable external snapshot set.
    ///
    /// The retained set changes only when the supplied set produces a newly
    /// independently accepted state. Input, lowering, solve, validation, and
    /// publication failures remain inspectable attempts while retaining both the
    /// current set and the prior accepted state's snapshot stamp.
    ///
    /// # Errors
    ///
    /// Rejects a stale design identity, a lower revision, or a same-revision set
    /// with a different digest before allocating an attempt identity.
    pub fn update_external_snapshot_set(
        &mut self,
        expected: SketchDesignIdentity,
        snapshots: ExternalSnapshotSet,
        request: DocumentSolveRequest,
    ) -> Result<&SketchDocumentAttempt, DocumentSessionError> {
        self.check_design_identity(expected)?;
        if snapshots != self.external_snapshots
            && snapshots.revision() <= self.external_snapshots.revision()
        {
            return Err(DocumentSessionError::StaleExternalSnapshotRevision {
                actual: snapshots.revision(),
                retained: self.external_snapshots.revision(),
            });
        }
        let attempt_identity = self.next_attempt_identity()?;
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &self.parameter_batch,
            &snapshots,
        );
        let execution = match seed_from_accepted_parent(&self.design, self.accepted.as_ref()) {
            Ok(seed) => run_retained_attempt(
                &seed,
                &self.parameter_batch,
                &snapshots,
                request,
                None,
                self.config,
                self.accepted.as_ref(),
            ),
            Err(error) => RetainedAttemptExecution::failure(
                SketchAttemptFailureKind::AcceptedSession,
                error.to_string(),
            ),
        };
        let (attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &self.design,
                design_provenance: &self.design_provenance,
                parent_design_provenance: self.parent_design_provenance.as_ref(),
                input: &input,
                attempt_identity,
                parent_accepted: parent,
                next_accepted_revision: next_accepted_revision(self.accepted_revision_high_water),
            },
            execution,
        );
        self.request = request;
        self.last_attempt = attempt;
        if let Some(accepted) = accepted {
            self.external_snapshots = snapshots;
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(&self.last_attempt)
    }

    /// Controlled counterpart to [`Self::update_external_snapshot_set`].
    ///
    /// Cancellation and work exhaustion leave the retained and accepted external
    /// snapshot sets unchanged.
    ///
    /// # Errors
    ///
    /// Rejects stale design identity, stale snapshot revisions, malformed inputs, or
    /// lifecycle revision exhaustion.
    pub fn update_external_snapshot_set_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        snapshots: ExternalSnapshotSet,
        request: DocumentSolveRequest,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchDocumentAttempt>, DocumentSessionError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.check_design_identity(expected)?;
        if snapshots != self.external_snapshots
            && snapshots.revision() <= self.external_snapshots.revision()
        {
            return Err(DocumentSessionError::StaleExternalSnapshotRevision {
                actual: snapshots.revision(),
                retained: self.external_snapshots.revision(),
            });
        }
        let attempt_identity = self.next_attempt_identity()?;
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &self.design,
            self.design_identity,
            request,
            request,
            self.config,
            &self.parameter_batch,
            &snapshots,
        );
        let seed = match seed_from_accepted_parent_controlled(
            &self.design,
            self.accepted.as_ref(),
            &mut controller,
        ) {
            Ok(Some(seed)) => seed,
            Ok(None) => return Ok(controller.outcome_unchecked()),
            Err(error) => {
                let execution = RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::AcceptedSession,
                    error.to_string(),
                );
                let (attempt, accepted) = publish_retained_attempt(
                    current_attempt_publication(self, &input, attempt_identity, parent),
                    execution,
                );
                if controller
                    .checkpoint(OperationCheckpoint::BeforeCommit)
                    .is_err()
                {
                    return Ok(controller.outcome_unchecked());
                }
                self.request = request;
                self.last_attempt = attempt.clone();
                debug_assert!(accepted.is_none());
                return Ok(controller.outcome(attempt));
            }
        };
        let Some(execution) = run_retained_attempt_controlled(
            &seed,
            &self.parameter_batch,
            &snapshots,
            request,
            None,
            self.config,
            self.accepted.as_ref(),
            &mut controller,
        ) else {
            return Ok(controller.outcome_unchecked());
        };
        let (attempt, accepted) = publish_retained_attempt(
            current_attempt_publication(self, &input, attempt_identity, parent),
            execution,
        );
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.request = request;
        self.last_attempt = attempt.clone();
        if let Some(accepted) = accepted {
            self.external_snapshots = snapshots;
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(controller.outcome(attempt))
    }

    /// Imports a supported v1-v4 graph as retained design intent.
    ///
    /// The payload carries no lifecycle revisions and makes no accepted-state claim.
    /// Use [`Self::export_accepted_json`] separately for the solved-state graph.
    ///
    /// # Errors
    ///
    /// Invalid JSON or a foreign document identity rejects before revisions advance.
    pub fn import_design_json(
        &mut self,
        expected: SketchDesignIdentity,
        json: &str,
    ) -> Result<RetainedDocumentTransactionOutcome<DocumentCommandEffect>, DocumentSessionError>
    {
        self.check_design_identity(expected)?;
        let candidate = SketchDocument::from_json(json)?;
        self.retain_candidate(candidate, DocumentCommandEffect::Imported, None)
    }

    /// Controlled counterpart to [`Self::import_design_json`].
    ///
    /// # Errors
    ///
    /// Returns the same import and solve setup errors as [`Self::import_design_json`].
    pub fn import_design_json_controlled(
        &mut self,
        expected: SketchDesignIdentity,
        json: &str,
        control: OperationControl,
    ) -> Result<
        OperationOutcome<RetainedDocumentTransactionOutcome<DocumentCommandEffect>>,
        DocumentSessionError,
    > {
        self.check_design_identity(expected)?;
        let mut controller = OperationController::new(control);
        let Some(candidate) = SketchDocument::from_json_with_controller(json, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        let Some(outcome) = self.retain_candidate_controlled(
            candidate,
            DocumentCommandEffect::Imported,
            None,
            &mut controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(outcome))
    }

    /// Exports only the retained design graph in frozen canonical v4 syntax.
    ///
    /// The payload does not encode or imply solve acceptance or lifecycle revisions.
    ///
    /// # Errors
    ///
    /// Returns a document validation or JSON serialization error.
    pub fn export_design_json(&self) -> Result<String, DocumentError> {
        self.design.to_canonical_json()
    }

    /// Exports only the last independently accepted solved graph, when available.
    ///
    /// # Errors
    ///
    /// Returns a document validation or JSON serialization error.
    pub fn export_accepted_json(&self) -> Result<Option<String>, DocumentError> {
        self.accepted
            .as_ref()
            .map(|accepted| accepted.document.to_canonical_json())
            .transpose()
    }

    fn check_design_identity(
        &self,
        expected: SketchDesignIdentity,
    ) -> Result<(), DocumentSessionError> {
        if expected == self.design_identity {
            Ok(())
        } else {
            Err(DocumentSessionError::StaleDesign {
                expected,
                actual: self.design_identity,
            })
        }
    }

    fn validate_drag_locality_request(
        &self,
        plan: &DocumentDragLocalityPlan,
        request: DocumentSolveRequest,
    ) -> Result<(), DocumentSessionError> {
        self.validate_drag_locality_plan(plan, plan.point)?;
        if request.drag.map(|drag| drag.point) != Some(plan.point) {
            return Err(DocumentSessionError::DragLocalityRequestMismatch {
                plan_point: plan.point,
                request_point: request.drag.map(|drag| drag.point),
            });
        }
        if !request.previous_state_preferences {
            return Err(DocumentSessionError::InvalidDragLocalityPlan {
                context: "the request disables the plan's PreviousState Preferences",
            });
        }
        Ok(())
    }

    fn validate_drag_locality_plan(
        &self,
        plan: &DocumentDragLocalityPlan,
        point: DesignPointId,
    ) -> Result<(), DocumentSessionError> {
        let actual_accepted = self
            .accepted
            .as_ref()
            .map(SketchAcceptedDocumentState::identity);
        let provenance_matches = self
            .accepted
            .as_ref()
            .is_some_and(|accepted| Arc::ptr_eq(&accepted.provenance, &plan.accepted_provenance));
        if plan.design != self.design_identity
            || !Arc::ptr_eq(&plan.design_provenance, &self.design_provenance)
            || actual_accepted != Some(plan.accepted)
            || !provenance_matches
        {
            return Err(DocumentSessionError::StaleDragLocalityPlan {
                evidence: Box::new(StaleDragLocalityPlanEvidence {
                    expected_design: plan.design,
                    expected_accepted: plan.accepted,
                    actual_design: self.design_identity,
                    actual_accepted,
                }),
            });
        }
        if plan.point != point {
            return Err(DocumentSessionError::DragLocalityRequestMismatch {
                plan_point: plan.point,
                request_point: Some(point),
            });
        }
        let accepted = self
            .accepted
            .as_ref()
            .ok_or(DocumentSessionError::DragLocalityUnavailable)?;
        if accepted.design_identity() != self.design_identity
            || accepted.document.point(plan.point).is_none()
        {
            return Err(DocumentSessionError::DragLocalityUnavailable);
        }
        if plan.active_rank > plan.hard_degrees_of_freedom
            || plan.passive_degrees_of_freedom
                != plan
                    .hard_degrees_of_freedom
                    .saturating_sub(plan.active_rank)
            || (plan.hard_degrees_of_freedom == 0 && !plan.anchors.is_empty())
            || (plan.passive_degrees_of_freedom > 0 && plan.anchors.is_empty())
        {
            return Err(DocumentSessionError::InvalidDragLocalityPlan {
                context: "the plan's rank and anchor evidence is inconsistent",
            });
        }
        let mut seen = BTreeSet::new();
        for anchor in &plan.anchors {
            if anchor.point == plan.point
                || !seen.insert(anchor.point)
                || anchor.mobility_rank == 0
                || anchor.mobility_rank > 2
                || !anchor.target.iter().all(|value| value.is_finite())
                || accepted
                    .document
                    .point(anchor.point)
                    .is_none_or(|point| pair_bits(point.position) != pair_bits(anchor.target))
                || accepted.mappings.runtime_point(anchor.point).is_none()
            {
                return Err(DocumentSessionError::InvalidDragLocalityPlan {
                    context: "the plan contains an invalid or non-authoritative anchor",
                });
            }
        }
        Ok(())
    }

    fn next_attempt_identity(&self) -> Result<SketchAttemptIdentity, DocumentSessionError> {
        let revision = self
            .last_attempt
            .identity
            .revision
            .0
            .checked_add(1)
            .ok_or(DocumentSessionError::RevisionExhausted { domain: "attempt" })?;
        Ok(SketchAttemptIdentity {
            document: self.design_identity.document,
            revision: SketchAttemptRevision(revision),
        })
    }

    fn retain_candidate<T>(
        &mut self,
        candidate: SketchDocument,
        value: T,
        command_drag: Option<DocumentDragTarget>,
    ) -> Result<RetainedDocumentTransactionOutcome<T>, DocumentSessionError> {
        if candidate.id() != self.design_identity.document {
            return Err(DocumentSessionError::ForeignDesign {
                expected: self.design_identity.document,
                actual: candidate.id(),
            });
        }
        candidate.validate()?;
        let design_revision = self
            .design_identity
            .revision
            .0
            .checked_add(1)
            .ok_or(DocumentSessionError::RevisionExhausted { domain: "design" })?;
        let attempt_identity = self.next_attempt_identity()?;
        let design_identity = SketchDesignIdentity {
            document: self.design_identity.document,
            revision: SketchDesignRevision(design_revision),
        };
        let parent_design_provenance = Arc::clone(&self.design_provenance);
        let design_provenance = Arc::new(());
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &candidate,
            design_identity,
            effective_attempt_request(self.request, command_drag),
            self.request,
            self.config,
            &self.parameter_batch,
            &self.external_snapshots,
        );
        let execution = match seed_from_accepted_parent(&candidate, self.accepted.as_ref()) {
            Ok(seed) => run_retained_attempt(
                &seed,
                &self.parameter_batch,
                &self.external_snapshots,
                self.request,
                command_drag,
                self.config,
                self.accepted.as_ref(),
            ),
            Err(error) => RetainedAttemptExecution::failure(
                SketchAttemptFailureKind::AcceptedSession,
                error.to_string(),
            ),
        };
        let (attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &candidate,
                design_provenance: &design_provenance,
                parent_design_provenance: Some(&parent_design_provenance),
                input: &input,
                attempt_identity,
                parent_accepted: parent,
                next_accepted_revision: next_accepted_revision(self.accepted_revision_high_water),
            },
            execution,
        );
        let published_accepted = accepted.as_ref().map(SketchAcceptedDocumentState::identity);
        self.design = candidate;
        self.design_identity = design_identity;
        self.design_provenance = design_provenance;
        self.parent_design_provenance = Some(parent_design_provenance);
        self.last_attempt = attempt;
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(RetainedDocumentTransactionOutcome {
            value,
            design: design_identity,
            attempt: attempt_identity,
            published_accepted,
        })
    }

    fn retain_candidate_with_seed<T>(
        &mut self,
        candidate: SketchDocument,
        value: T,
        command_drag: Option<DocumentDragTarget>,
        numerical_seed: &SketchDocument,
    ) -> Result<RetainedDocumentTransactionOutcome<T>, DocumentSessionError> {
        if candidate.id() != self.design_identity.document || numerical_seed.id() != candidate.id()
        {
            return Err(DocumentSessionError::ForeignDesign {
                expected: self.design_identity.document,
                actual: candidate.id(),
            });
        }
        candidate.validate()?;
        numerical_seed.validate()?;
        let seed = candidate_shaped_numerical_seed(&candidate, numerical_seed);
        let design_revision = next_revision(self.design_identity.revision.0, "design")?;
        let attempt_identity = self.next_attempt_identity()?;
        let design_identity = SketchDesignIdentity {
            document: self.design_identity.document,
            revision: SketchDesignRevision(design_revision),
        };
        let parent_design_provenance = Arc::clone(&self.design_provenance);
        let design_provenance = Arc::new(());
        let parent = self.accepted.as_ref();
        let input = SketchAttemptInput::for_document_with_parameters(
            &candidate,
            design_identity,
            effective_attempt_request(self.request, command_drag),
            self.request,
            self.config,
            &self.parameter_batch,
            &self.external_snapshots,
        );
        let execution = run_retained_attempt(
            &seed,
            &self.parameter_batch,
            &self.external_snapshots,
            self.request,
            command_drag,
            self.config,
            self.accepted.as_ref(),
        );
        let (attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &candidate,
                design_provenance: &design_provenance,
                parent_design_provenance: Some(&parent_design_provenance),
                input: &input,
                attempt_identity,
                parent_accepted: parent,
                next_accepted_revision: next_accepted_revision(self.accepted_revision_high_water),
            },
            execution,
        );
        let published_accepted = accepted.as_ref().map(SketchAcceptedDocumentState::identity);
        self.design = candidate;
        self.design_identity = design_identity;
        self.design_provenance = design_provenance;
        self.parent_design_provenance = Some(parent_design_provenance);
        self.last_attempt = attempt;
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(RetainedDocumentTransactionOutcome {
            value,
            design: design_identity,
            attempt: attempt_identity,
            published_accepted,
        })
    }

    fn retain_candidate_controlled<T>(
        &mut self,
        candidate: SketchDocument,
        value: T,
        command_drag: Option<DocumentDragTarget>,
        controller: &mut OperationController,
    ) -> Result<Option<RetainedDocumentTransactionOutcome<T>>, DocumentSessionError> {
        self.retain_candidate_with_optional_seed_controlled(
            candidate,
            value,
            command_drag,
            None,
            None,
            controller,
        )
    }

    fn retain_candidate_with_seed_controlled<T>(
        &mut self,
        candidate: SketchDocument,
        value: T,
        command_drag: Option<DocumentDragTarget>,
        numerical_seed: &SketchDocument,
        controller: &mut OperationController,
    ) -> Result<Option<RetainedDocumentTransactionOutcome<T>>, DocumentSessionError> {
        self.retain_candidate_with_optional_seed_controlled(
            candidate,
            value,
            command_drag,
            Some(numerical_seed),
            None,
            controller,
        )
    }

    fn publish_preview_candidate_controlled<T, F>(
        &mut self,
        publication_candidate: PreviewPublicationCandidate<'_, T>,
        controller: &mut OperationController,
        validate_publication: F,
    ) -> Result<Option<RetainedDocumentTransactionOutcome<T>>, DocumentSessionError>
    where
        F: FnOnce(
            &RetainedSketchDocumentSession,
            &RetainedDocumentTransactionOutcome<T>,
            &mut OperationController,
        ) -> Result<(), DocumentSessionError>,
    {
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(None);
        }
        let PreviewPublicationCandidate {
            candidate,
            value,
            command_drag,
            numerical_seed,
            previous_state_intent,
        } = publication_candidate;
        let mut publication = self.clone();
        let Some(outcome) = publication.retain_candidate_with_optional_seed_controlled(
            candidate,
            value,
            command_drag,
            Some(numerical_seed),
            previous_state_intent,
            controller,
        )?
        else {
            return Ok(None);
        };
        validate_publication(&publication, &outcome, controller)?;
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(None);
        }
        *self = publication;
        Ok(Some(outcome))
    }

    fn retain_candidate_with_optional_seed_controlled<T>(
        &mut self,
        candidate: SketchDocument,
        value: T,
        command_drag: Option<DocumentDragTarget>,
        numerical_seed: Option<&SketchDocument>,
        previous_state_intent: Option<RetainedPreviousStateIntent<'_>>,
        controller: &mut OperationController,
    ) -> Result<Option<RetainedDocumentTransactionOutcome<T>>, DocumentSessionError> {
        if candidate.id() != self.design_identity.document
            || numerical_seed.is_some_and(|seed| seed.id() != candidate.id())
        {
            return Err(DocumentSessionError::ForeignDesign {
                expected: self.design_identity.document,
                actual: candidate.id(),
            });
        }
        if !candidate.validate_with_controller(Some(controller))? {
            return Ok(None);
        }
        if let Some(numerical_seed) = numerical_seed
            && !numerical_seed.validate_with_controller(Some(controller))?
        {
            return Ok(None);
        }
        let design_revision = self
            .design_identity
            .revision
            .0
            .checked_add(1)
            .ok_or(DocumentSessionError::RevisionExhausted { domain: "design" })?;
        let attempt_identity = self.next_attempt_identity()?;
        let design_identity = SketchDesignIdentity {
            document: self.design_identity.document,
            revision: SketchDesignRevision(design_revision),
        };
        let parent_design_provenance = Arc::clone(&self.design_provenance);
        let design_provenance = Arc::new(());
        let parent = self.accepted.as_ref();
        let publication_request =
            effective_publication_request(self.request, previous_state_intent);
        let input = SketchAttemptInput::for_document_with_parameters(
            &candidate,
            design_identity,
            effective_attempt_request_with_previous_state_intent(
                publication_request,
                command_drag,
                previous_state_intent,
            ),
            publication_request,
            self.config,
            &self.parameter_batch,
            &self.external_snapshots,
        );
        let Some(execution) = self.run_candidate_with_optional_seed_controlled(
            &candidate,
            publication_request,
            command_drag,
            numerical_seed,
            previous_state_intent,
            controller,
        ) else {
            return Ok(None);
        };
        let (attempt, accepted) = publish_retained_attempt(
            RetainedAttemptPublication {
                solved_design: &candidate,
                design_provenance: &design_provenance,
                parent_design_provenance: Some(&parent_design_provenance),
                input: &input,
                attempt_identity,
                parent_accepted: parent,
                next_accepted_revision: next_accepted_revision(self.accepted_revision_high_water),
            },
            execution,
        );
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(None);
        }
        let published_accepted = accepted.as_ref().map(SketchAcceptedDocumentState::identity);
        self.design = candidate;
        self.design_identity = design_identity;
        self.design_provenance = design_provenance;
        self.parent_design_provenance = Some(parent_design_provenance);
        self.last_attempt = attempt;
        if let Some(accepted) = accepted {
            self.accepted_revision_high_water = Some(accepted.identity.revision);
            self.accepted = Some(accepted);
        }
        Ok(Some(RetainedDocumentTransactionOutcome {
            value,
            design: design_identity,
            attempt: attempt_identity,
            published_accepted,
        }))
    }

    fn run_candidate_with_optional_seed_controlled(
        &self,
        candidate: &SketchDocument,
        publication_request: DocumentSolveRequest,
        command_drag: Option<DocumentDragTarget>,
        numerical_seed: Option<&SketchDocument>,
        previous_state_intent: Option<RetainedPreviousStateIntent<'_>>,
        controller: &mut OperationController,
    ) -> Option<RetainedAttemptExecution> {
        let seed = if let Some(numerical_seed) = numerical_seed {
            candidate_shaped_numerical_seed_controlled(candidate, numerical_seed, controller)?
        } else {
            match seed_from_accepted_parent_controlled(
                candidate,
                self.accepted.as_ref(),
                controller,
            ) {
                Ok(Some(seed)) => seed,
                Ok(None) => return None,
                Err(error) => {
                    return Some(RetainedAttemptExecution::failure(
                        SketchAttemptFailureKind::AcceptedSession,
                        error.to_string(),
                    ));
                }
            }
        };
        run_retained_attempt_with_previous_state_reference_controlled(
            &seed,
            &self.parameter_batch,
            &self.external_snapshots,
            publication_request,
            command_drag,
            self.config,
            self.accepted.as_ref(),
            previous_state_intent,
            controller,
        )
    }

    fn current_prepared_input(&self) -> PreparedSketchInput {
        PreparedSketchInput {
            input: SketchAttemptInput::for_document_with_parameters(
                &self.design,
                self.design_identity,
                self.request,
                self.request,
                self.config,
                &self.parameter_batch,
                &self.external_snapshots,
            ),
            latest_attempt: self.last_attempt.identity,
            accepted: self
                .accepted
                .as_ref()
                .map(SketchAcceptedDocumentState::identity),
            accepted_revision_high_water: self.accepted_revision_high_water,
        }
    }
}

/// Builds a solve seed whose graph is always the candidate graph.
///
/// Persistent point identity and an unchanged curve/contact scalar role are the
/// numerical-state compatibility boundary. Dimension target scalars are equation
/// coefficients rather than solver coordinates and are deliberately never imported.
/// The one curve-local numerical coordinate that is not stored in a point or scalar is
/// copied only when its rational-conic carrier topology matches. Constraints, dimensions,
/// contacts, branches, activation, ownership, and every other equation-bearing field
/// always come from `candidate`.
///
/// Compatible IDs do not prove that their merged values satisfy candidate-only topology.
/// If numerical import makes the otherwise valid candidate structurally invalid, the
/// untouched candidate is the deterministic seed rather than turning history/restore into
/// an error or substituting an older topology.
fn candidate_shaped_numerical_seed(
    candidate: &SketchDocument,
    numerical_state: &SketchDocument,
) -> SketchDocument {
    debug_assert_eq!(candidate.id(), numerical_state.id());
    let seed = merge_candidate_shaped_numerical_seed(candidate, numerical_state);
    if seed.validate().is_ok() {
        seed
    } else {
        candidate.clone()
    }
}

fn candidate_shaped_numerical_seed_controlled(
    candidate: &SketchDocument,
    numerical_state: &SketchDocument,
    controller: &mut OperationController,
) -> Option<SketchDocument> {
    debug_assert_eq!(candidate.id(), numerical_state.id());
    let dependency_items = candidate
        .points()
        .len()
        .saturating_add(candidate.scalars().len())
        .saturating_add(candidate.curves().len());
    if controller
        .charge(
            geosolve_core::OperationWorkCounter::DocumentDependencyItems,
            dependency_items,
            OperationCheckpoint::DocumentDependency,
        )
        .is_err()
    {
        return None;
    }
    let seed = merge_candidate_shaped_numerical_seed(candidate, numerical_state);
    match seed.validate_with_controller(Some(controller)) {
        Ok(true) => Some(seed),
        Ok(false) => None,
        Err(_) => Some(candidate.clone()),
    }
}

fn merge_candidate_shaped_numerical_seed(
    candidate: &SketchDocument,
    numerical_state: &SketchDocument,
) -> SketchDocument {
    let mut seed = candidate.clone();

    for point in candidate.points() {
        let Some(numerical_point) = numerical_state.point(point.id) else {
            continue;
        };
        seed.point_mut(point.id)
            .expect("point came from the candidate document")
            .position = numerical_point.position;
    }

    for scalar in candidate.scalars() {
        let Some(numerical_scalar) = numerical_state.scalar(scalar.id) else {
            continue;
        };
        if scalar.unit == numerical_scalar.unit
            && scalar.domain == numerical_scalar.domain
            && numerical_scalar_state_is_compatible(candidate, numerical_state, scalar.id)
        {
            seed.scalar_mut(scalar.id)
                .expect("scalar came from the candidate document")
                .value = numerical_scalar.value;
        }
    }

    for curve in candidate.curves() {
        let (
            CurveDefinition::RationalQuadraticConic {
                start,
                middle_weight,
                end,
                ..
            },
            Some(crate::document::DesignCurve {
                definition:
                    CurveDefinition::RationalQuadraticConic {
                        start: numerical_start,
                        weighted_middle: numerical_middle,
                        middle_weight: numerical_weight,
                        end: numerical_end,
                    },
                ..
            }),
        ) = (&curve.definition, numerical_state.curve(curve.id))
        else {
            continue;
        };
        if start == numerical_start && middle_weight == numerical_weight && end == numerical_end {
            let CurveDefinition::RationalQuadraticConic {
                weighted_middle, ..
            } = &mut seed
                .curve_mut(curve.id)
                .expect("curve came from the candidate document")
                .definition
            else {
                unreachable!("curve family came from the candidate document");
            };
            *weighted_middle = *numerical_middle;
        }
    }
    seed
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumericalScalarOwner {
    Curve {
        curve: CurveId,
        role: CurveNumericalScalarRole,
    },
    Contact(ContactId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CurveNumericalScalarRole {
    CircleRadius,
    CircularArcRadius,
    CircularArcStart,
    CircularArcEnd,
    EllipseRatio,
    EllipticalArcRatio,
    EllipticalArcStart,
    EllipticalArcEnd,
    RationalMiddleWeight,
    ParabolaTrimStart,
    ParabolaTrimEnd,
    HyperbolaSemiConjugate,
    HyperbolaTrimStart,
    HyperbolaTrimEnd,
    NurbsWeight { index: usize, gauge: bool },
}

fn numerical_scalar_state_is_compatible(
    candidate: &SketchDocument,
    numerical_state: &SketchDocument,
    scalar: DesignScalarId,
) -> bool {
    let owner = numerical_scalar_owner(candidate, scalar);
    if owner.is_none() || owner != numerical_scalar_owner(numerical_state, scalar) {
        return false;
    }
    match owner {
        Some(NumericalScalarOwner::Contact(contact)) => {
            candidate.contact(contact) == numerical_state.contact(contact)
        }
        Some(NumericalScalarOwner::Curve { .. }) => true,
        None => false,
    }
}

fn numerical_scalar_owner(
    document: &SketchDocument,
    scalar: DesignScalarId,
) -> Option<NumericalScalarOwner> {
    for curve in document.curves() {
        if let Some(role) = curve_numerical_scalar_role(&curve.definition, scalar) {
            return Some(NumericalScalarOwner::Curve {
                curve: curve.id,
                role,
            });
        }
    }
    document
        .contacts()
        .iter()
        .find(|contact| contact.parameter == scalar)
        .map(|contact| NumericalScalarOwner::Contact(contact.id))
}

#[allow(clippy::too_many_lines)]
fn curve_numerical_scalar_role(
    definition: &CurveDefinition,
    scalar: DesignScalarId,
) -> Option<CurveNumericalScalarRole> {
    match definition {
        CurveDefinition::Line { .. }
        | CurveDefinition::Polyline { .. }
        | CurveDefinition::QuadraticBezier { .. }
        | CurveDefinition::CubicBezier { .. }
        | CurveDefinition::BSpline { .. } => None,
        CurveDefinition::Circle { radius, .. } => {
            (*radius == scalar).then_some(CurveNumericalScalarRole::CircleRadius)
        }
        CurveDefinition::CircularArc {
            radius,
            start_angle,
            end_angle,
            ..
        } => [
            (*radius, CurveNumericalScalarRole::CircularArcRadius),
            (*start_angle, CurveNumericalScalarRole::CircularArcStart),
            (*end_angle, CurveNumericalScalarRole::CircularArcEnd),
        ]
        .into_iter()
        .find_map(|(candidate, role)| (candidate == scalar).then_some(role)),
        CurveDefinition::Ellipse {
            minor_axis_ratio, ..
        } => (*minor_axis_ratio == scalar).then_some(CurveNumericalScalarRole::EllipseRatio),
        CurveDefinition::EllipticalArc {
            minor_axis_ratio,
            start_angle,
            end_angle,
            ..
        } => [
            (
                *minor_axis_ratio,
                CurveNumericalScalarRole::EllipticalArcRatio,
            ),
            (*start_angle, CurveNumericalScalarRole::EllipticalArcStart),
            (*end_angle, CurveNumericalScalarRole::EllipticalArcEnd),
        ]
        .into_iter()
        .find_map(|(candidate, role)| (candidate == scalar).then_some(role)),
        CurveDefinition::RationalQuadraticConic { middle_weight, .. } => {
            (*middle_weight == scalar).then_some(CurveNumericalScalarRole::RationalMiddleWeight)
        }
        CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        } => [
            (*trim_start, CurveNumericalScalarRole::ParabolaTrimStart),
            (*trim_end, CurveNumericalScalarRole::ParabolaTrimEnd),
        ]
        .into_iter()
        .find_map(|(candidate, role)| (candidate == scalar).then_some(role)),
        CurveDefinition::HyperbolaSegment {
            semi_conjugate,
            trim_start,
            trim_end,
            ..
        } => [
            (
                *semi_conjugate,
                CurveNumericalScalarRole::HyperbolaSemiConjugate,
            ),
            (*trim_start, CurveNumericalScalarRole::HyperbolaTrimStart),
            (*trim_end, CurveNumericalScalarRole::HyperbolaTrimEnd),
        ]
        .into_iter()
        .find_map(|(candidate, role)| (candidate == scalar).then_some(role)),
        CurveDefinition::Nurbs {
            weights,
            gauge_weight,
            ..
        } => weights
            .iter()
            .position(|weight| *weight == scalar)
            .map(|index| CurveNumericalScalarRole::NurbsWeight {
                index,
                gauge: *gauge_weight == scalar,
            }),
    }
}

/// Returns one deterministic, bit-preserving document encoding for exact lifecycle checks.
///
/// Frozen v1-v4-compatible documents use canonical v4 bytes. Documents that require
/// explicitly unstable draft-v5 fields use the same deterministic fallback as editor
/// checkpoints. This is intentionally stricter than `SketchDocument::PartialEq`, whose
/// floating-point equality treats distinct encodings such as `+0.0` and `-0.0` as equal.
fn exact_document_bytes(document: &SketchDocument) -> Result<Vec<u8>, DocumentError> {
    match document.to_canonical_json() {
        Ok(json) => Ok(json.into_bytes()),
        Err(_) => document.to_draft_v5_json().map(String::into_bytes),
    }
}

fn documents_have_exact_bytes(
    first: &SketchDocument,
    second: &SketchDocument,
) -> Result<bool, DocumentError> {
    Ok(first.exact_unserialized_state_matches(second)
        && exact_document_bytes(first)? == exact_document_bytes(second)?)
}

fn documents_have_exact_bytes_controlled(
    first: &SketchDocument,
    second: &SketchDocument,
    controller: &mut OperationController,
) -> Result<Option<bool>, DocumentError> {
    let comparison_items = exact_document_comparison_items(first)
        .saturating_add(exact_document_comparison_items(second));
    if controller
        .charge(
            OperationWorkCounter::DocumentValidationItems,
            comparison_items,
            OperationCheckpoint::DocumentValidation,
        )
        .is_err()
    {
        return Ok(None);
    }
    Ok(Some(documents_have_exact_bytes(first, second)?))
}

fn exact_document_comparison_items(document: &SketchDocument) -> usize {
    [
        1,
        document.points().len(),
        document.scalars().len(),
        document.curves().len(),
        document.contacts().len(),
        document.trim_views().len(),
        document.constraints().len(),
        document.dimensions().len(),
        document.parameters().len(),
        document.parameter_bindings().len(),
        document.parameter_outputs().len(),
        document.external_bindings().len(),
        document.source_order().len(),
        document.exact_unserialized_state_items(),
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add)
}

fn optional_provenance_matches(first: Option<&Arc<()>>, second: Option<&Arc<()>>) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => Arc::ptr_eq(first, second),
        (None, None) => true,
        _ => false,
    }
}

fn next_revision(current: u64, domain: &'static str) -> Result<u64, DocumentSessionError> {
    current
        .checked_add(1)
        .ok_or(DocumentSessionError::RevisionExhausted { domain })
}

fn next_accepted_revision(high_water: Option<SketchAcceptedRevision>) -> Option<u64> {
    match high_water {
        Some(revision) => revision.0.checked_add(1),
        None => Some(0),
    }
}

fn effective_attempt_request(
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
) -> DocumentSolveRequest {
    DocumentSolveRequest {
        drag: command_drag.or(request.drag),
        previous_state_preferences: command_drag.is_none() && request.previous_state_preferences,
    }
}

fn effective_attempt_request_with_previous_state_intent(
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    previous_state_intent: Option<RetainedPreviousStateIntent<'_>>,
) -> DocumentSolveRequest {
    let mut effective = effective_attempt_request(request, command_drag);
    if previous_state_intent.is_some_and(RetainedPreviousStateIntent::is_drag_locality) {
        effective.previous_state_preferences = true;
    }
    effective
}

fn effective_publication_request(
    request: DocumentSolveRequest,
    previous_state_intent: Option<RetainedPreviousStateIntent<'_>>,
) -> DocumentSolveRequest {
    if previous_state_intent.is_some_and(RetainedPreviousStateIntent::is_drag_locality) {
        request.with_previous_state_preferences()
    } else {
        request
    }
}

fn incremental_runtime_sources(
    candidate: &SketchDocument,
    mappings: &DocumentRuntimeMap,
    stamps: &RetainedAttemptInputStamps,
    parent: &SketchAcceptedDocumentState,
) -> Vec<SketchSource> {
    let mut changed = BTreeSet::new();
    for point in candidate.points() {
        if parent.solved_design.point(point.id) != Some(point) {
            changed.insert(DocumentElementId::Point(point.id));
        }
    }
    for scalar in candidate.scalars() {
        if parent.solved_design.scalar(scalar.id) != Some(scalar) {
            changed.insert(DocumentElementId::Scalar(scalar.id));
        }
    }
    for curve in candidate.curves() {
        if parent.solved_design.curve(curve.id) != Some(curve) {
            changed.insert(DocumentElementId::Curve(curve.id));
        }
    }
    for contact in candidate.contacts() {
        if parent.solved_design.contact(contact.id) != Some(contact) {
            changed.insert(DocumentElementId::Contact(contact.id));
        }
    }
    for constraint in candidate.constraints() {
        if parent.solved_design.constraint(constraint.id) != Some(constraint) {
            changed.insert(DocumentElementId::Constraint(constraint.id));
        }
    }
    for dimension in candidate.dimensions() {
        if parent.solved_design.dimension(dimension.id) != Some(dimension) {
            changed.insert(DocumentElementId::Dimension(dimension.id));
        }
    }
    for parameter in candidate.parameters() {
        if parent
            .solved_design
            .parameters()
            .iter()
            .find(|retained| retained.id == parameter.id)
            != Some(parameter)
        {
            changed.insert(DocumentElementId::Parameter(parameter.id));
        }
    }
    for binding in candidate.external_bindings() {
        if parent.solved_design.external_binding(binding.id) != Some(binding)
            || stamps.external_digest != parent.input.external_snapshot_set_digest()
        {
            changed.insert(DocumentElementId::ExternalBinding(binding.id));
        }
    }

    let mut sources = Vec::new();
    for mapping in mappings.source_mappings() {
        let source_element = DocumentElementId::Source(mapping.source_id);
        if !changed.contains(&source_element)
            && !candidate
                .dependency_closure(source_element)
                .iter()
                .any(|dependency| changed.contains(dependency))
        {
            continue;
        }
        if let Some(runtime) = mapping.runtime {
            push_unique_runtime_source(&mut sources, runtime);
        }
    }
    for binding in mappings.parameter_bindings() {
        let retained = parent
            .mappings
            .parameter_bindings()
            .iter()
            .find(|retained| {
                retained.parameter == binding.parameter && retained.target == binding.target
            });
        if retained != Some(binding) {
            push_unique_runtime_source(&mut sources, binding.runtime);
        }
    }
    sources
}

fn push_unique_runtime_source(sources: &mut Vec<SketchSource>, runtime: RuntimeSource) {
    let source = match runtime {
        RuntimeSource::Constraint(source) => SketchSource::Constraint(source),
        RuntimeSource::Dimension(source) => SketchSource::Dimension(source),
    };
    if !sources.contains(&source) {
        sources.push(source);
    }
}

struct RetainedAttemptExecution {
    solve: Option<SketchSolveResult>,
    attempted_geometry: Option<crate::SketchGeometry>,
    mappings: Option<DocumentRuntimeMap>,
    effective_activity: Option<crate::EffectiveActivity>,
    accepted: Option<(
        SketchDocument,
        SketchSession,
        DocumentRuntimeMap,
        RetainedAttemptInputStamps,
    )>,
    failure: Option<SketchAttemptFailure>,
}

#[derive(Clone)]
struct RetainedAttemptInputStamps {
    activity: crate::EffectiveActivity,
    activation_revision: u64,
    activation_digest: ActivationDigest,
    parameter_revision: u64,
    parameter_digest: ParameterDigest,
    external_revision: u64,
    external_digest: ExternalSnapshotSetDigest,
}

#[derive(Clone, Copy)]
enum RetainedPreviousStateIntent<'a> {
    /// Preserve the accepted runtime's immutable Preference targets while
    /// remapping its runtime point IDs through persistent document identity.
    AcceptedRuntimeReference(&'a SketchAcceptedDocumentState),
    /// Use only the persistent frozen anchors selected when a drag began.
    DragLocality(&'a DocumentDragLocalityPlan),
    /// Republish an already accepted visible preview through fresh validation
    /// without another numerical move.
    ///
    /// A projected drag retains its complete gesture-start locality plan and
    /// restricts Preferences to those anchors, retargeted to the accepted preview.
    /// Other exact-preview publications retain the ordinary request and, when it
    /// enables previous-state Preferences, target the complete preview state.
    PreviewPublication {
        preview: &'a SketchDocument,
        locality: Option<&'a DocumentDragLocalityPlan>,
    },
}

impl RetainedPreviousStateIntent<'_> {
    const fn is_drag_locality(self) -> bool {
        matches!(
            self,
            Self::DragLocality(_)
                | Self::PreviewPublication {
                    locality: Some(_),
                    ..
                }
        )
    }

    const fn is_no_motion_publication(self) -> bool {
        matches!(self, Self::PreviewPublication { .. })
    }
}

fn same_input_previous_state_intent<'a>(
    parent: Option<&'a SketchAcceptedDocumentState>,
    input: &SketchAttemptInput,
) -> Option<RetainedPreviousStateIntent<'a>> {
    parent
        .filter(|accepted| accepted.input == *input)
        .map(RetainedPreviousStateIntent::AcceptedRuntimeReference)
}

struct PreviewPublicationCandidate<'a, T> {
    candidate: SketchDocument,
    value: T,
    command_drag: Option<DocumentDragTarget>,
    numerical_seed: &'a SketchDocument,
    previous_state_intent: Option<RetainedPreviousStateIntent<'a>>,
}

fn retained_previous_state_reference(
    sketch: &crate::Sketch,
    mappings: &DocumentRuntimeMap,
    intent: Option<RetainedPreviousStateIntent<'_>>,
) -> Result<PreviousStateReference, DocumentSessionError> {
    let mut targets = sketch.clone();
    let mut restricted = None;
    match intent {
        None => {}
        Some(RetainedPreviousStateIntent::AcceptedRuntimeReference(accepted)) => {
            let accepted_reference = accepted.runtime.previous_state_reference();
            let mut selected = Vec::new();
            for mapping in mappings.point_mappings() {
                let accepted_runtime = accepted
                    .mappings
                    .runtime_point(mapping.persistent)
                    .ok_or(SketchSessionError::RebuildRequired)?;
                let target = accepted_reference.point_position(accepted_runtime)?;
                targets.set_point_position(mapping.runtime, target)?;
                if accepted_reference.includes_preference(accepted_runtime) {
                    selected.push(mapping.runtime);
                }
            }
            if accepted_reference.preferences_are_restricted() {
                restricted = Some(selected);
            }
        }
        Some(RetainedPreviousStateIntent::DragLocality(plan)) => {
            let mut selected = Vec::with_capacity(plan.anchors.len());
            for anchor in &plan.anchors {
                let runtime = mappings.runtime_point(anchor.point).ok_or(
                    DocumentSessionError::InvalidDragLocalityPlan {
                        context: "a frozen anchor has no runtime mapping in this attempt",
                    },
                )?;
                targets
                    .set_point_position(runtime, Point2::new(anchor.target[0], anchor.target[1]))?;
                selected.push(runtime);
            }
            restricted = Some(selected);
        }
        Some(RetainedPreviousStateIntent::PreviewPublication { preview, locality }) => {
            if let Some(locality) = locality {
                let mut selected = Vec::with_capacity(locality.anchors.len());
                for anchor in &locality.anchors {
                    let runtime = mappings.runtime_point(anchor.point).ok_or(
                        DocumentSessionError::InvalidDragLocalityPlan {
                            context: "a frozen anchor has no runtime mapping in this publication",
                        },
                    )?;
                    let target = preview.point(anchor.point).ok_or(
                        DocumentSessionError::DragPublicationContinuity {
                            context: "a frozen anchor is absent from the publication preview",
                        },
                    )?;
                    targets.set_point_position(
                        runtime,
                        Point2::new(target.position[0], target.position[1]),
                    )?;
                    selected.push(runtime);
                }
                restricted = Some(selected);
            } else {
                for mapping in mappings.point_mappings() {
                    let target = preview.point(mapping.persistent).ok_or(
                        DocumentSessionError::DragPublicationContinuity {
                            context: "a publication point is absent from the accepted preview",
                        },
                    )?;
                    targets.set_point_position(
                        mapping.runtime,
                        Point2::new(target.position[0], target.position[1]),
                    )?;
                }
            }
        }
    }
    let mut reference = PreviousStateReference::capture(&targets);
    if let Some(selected) = restricted {
        reference.restrict_preferences_to(selected)?;
    }
    Ok(reference)
}

impl RetainedAttemptExecution {
    fn failure(kind: SketchAttemptFailureKind, message: String) -> Self {
        Self {
            solve: None,
            attempted_geometry: None,
            mappings: None,
            effective_activity: None,
            accepted: None,
            failure: Some(SketchAttemptFailure {
                kind,
                message,
                parameter_issue: None,
                external_error: None,
                effective_activity: None,
            }),
        }
    }

    fn parameter_input_failure(error: &ParameterInputFailure) -> Self {
        Self {
            solve: None,
            attempted_geometry: None,
            mappings: None,
            accepted: None,
            effective_activity: None,
            failure: Some(SketchAttemptFailure {
                kind: SketchAttemptFailureKind::ParameterInput,
                message: error.error.to_string(),
                parameter_issue: Some(error.issue),
                external_error: None,
                effective_activity: None,
            }),
        }
    }

    fn external_input_failure(
        error: ExternalSnapshotInputError,
        activity: crate::EffectiveActivity,
    ) -> Self {
        Self {
            solve: None,
            attempted_geometry: None,
            mappings: None,
            effective_activity: Some(activity.clone()),
            accepted: None,
            failure: Some(SketchAttemptFailure {
                kind: SketchAttemptFailureKind::ExternalSnapshotInput,
                message: error.to_string(),
                parameter_issue: None,
                external_error: Some(error),
                effective_activity: Some(activity),
            }),
        }
    }
}

fn run_retained_attempt(
    candidate: &SketchDocument,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    config: SolverConfig,
    parent: Option<&SketchAcceptedDocumentState>,
) -> RetainedAttemptExecution {
    run_retained_attempt_with_previous_state_reference(
        candidate,
        parameters,
        snapshots,
        request,
        command_drag,
        config,
        parent,
        None,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_retained_attempt_with_previous_state_reference(
    candidate: &SketchDocument,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    config: SolverConfig,
    parent: Option<&SketchAcceptedDocumentState>,
    previous_state_intent: Option<RetainedPreviousStateIntent<'_>>,
) -> RetainedAttemptExecution {
    let resolved = match resolve_attempt_inputs(candidate, parameters, snapshots) {
        Ok(resolved) => resolved,
        Err(AttemptInputError::Parameter(error)) => {
            return RetainedAttemptExecution::parameter_input_failure(&error);
        }
        Err(AttemptInputError::External { error, activity }) => {
            return RetainedAttemptExecution::external_input_failure(error, activity);
        }
    };
    let input_stamps = RetainedAttemptInputStamps {
        activity: resolved.activity.clone(),
        activation_revision: resolved.activity.activation_revision(),
        activation_digest: resolved.activity.activation_digest(),
        parameter_revision: parameters.revision(),
        parameter_digest: parameters.digest(),
        external_revision: resolved.external_revision,
        external_digest: resolved.external_digest,
    };
    let lowered = match candidate.lower_with_resolved_parameters(&resolved) {
        Ok(lowered) => lowered,
        Err(error) => {
            return RetainedAttemptExecution {
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Lowering,
                    error.to_string(),
                )
            };
        }
    };
    let (mut sketch, mappings) = lowered.into_parts();
    let previous_state =
        match retained_previous_state_reference(&sketch, &mappings, previous_state_intent) {
            Ok(previous_state) => previous_state,
            Err(error) => {
                return RetainedAttemptExecution {
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    ..RetainedAttemptExecution::failure(
                        SketchAttemptFailureKind::Request,
                        error.to_string(),
                    )
                };
            }
        };
    let runtime_request = match lower_request(request, &mappings) {
        Ok(request) => request,
        Err(error) => {
            return RetainedAttemptExecution {
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Request,
                    error.to_string(),
                )
            };
        }
    };
    let attempted_request = match lower_request(
        effective_attempt_request_with_previous_state_intent(
            request,
            command_drag,
            previous_state_intent,
        ),
        &mappings,
    ) {
        Ok(request) => request,
        Err(error) => {
            return RetainedAttemptExecution {
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Request,
                    error.to_string(),
                )
            };
        }
    };
    if attempted_request == runtime_request
        && let Some(parent) = parent.filter(|parent| {
            parent.mappings.has_compatible_runtime_topology(&mappings)
                && parent.runtime.request() == runtime_request
        })
    {
        let sources = incremental_runtime_sources(candidate, &mappings, &input_stamps, parent);
        let mut runtime = parent.runtime.clone();
        match runtime.apply_compatible_candidate(
            sketch.clone(),
            runtime_request,
            &sources,
            &previous_state,
            None,
        ) {
            Ok(Some(result)) if result.accepted() => {
                let mut document = candidate.clone();
                if let Err(error) = document.project_accepted_state(runtime.sketch(), &mappings) {
                    let mut rejected = result;
                    rejected.rejection = Some(SolveRejection::IndependentValidationFailed(
                        error.to_string(),
                    ));
                    rejected.acceptance_hard_residual_max = None;
                    rejected.core_report.hard_validity = HardValidity::Invalid;
                    rejected.core_report.termination = SolveTermination::Stalled;
                    return RetainedAttemptExecution {
                        attempted_geometry: rejected.attempted_geometry.clone(),
                        solve: Some(rejected),
                        mappings: Some(mappings),
                        effective_activity: Some(resolved.activity.clone()),
                        accepted: None,
                        failure: None,
                    };
                }
                let solve = runtime.accepted_result().clone();
                return RetainedAttemptExecution {
                    attempted_geometry: solve.attempted_geometry.clone(),
                    solve: Some(solve),
                    mappings: Some(mappings.clone()),
                    effective_activity: Some(resolved.activity),
                    accepted: Some((document, runtime, mappings, input_stamps)),
                    failure: None,
                };
            }
            Ok(Some(rejected)) => {
                return RetainedAttemptExecution {
                    attempted_geometry: rejected.attempted_geometry.clone(),
                    solve: Some(rejected),
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    accepted: None,
                    failure: None,
                };
            }
            Ok(None) => unreachable!("uncontrolled incremental solve cannot be interrupted"),
            Err(SketchSessionError::RebuildRequired) => {}
            Err(error) => {
                return RetainedAttemptExecution {
                    solve: None,
                    attempted_geometry: None,
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    accepted: None,
                    failure: Some(SketchAttemptFailure {
                        kind: SketchAttemptFailureKind::AcceptedSession,
                        message: error.to_string(),
                        parameter_issue: None,
                        external_error: None,
                        effective_activity: None,
                    }),
                };
            }
        }
    }
    let solve = match sketch.solve_with_previous_state_reference(
        attempted_request,
        config,
        &previous_state,
    ) {
        Ok(solve) => solve,
        Err(error) => {
            return RetainedAttemptExecution {
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Solve,
                    error.to_string(),
                )
            };
        }
    };
    let attempted_geometry = solve.attempted_geometry.clone();
    if !solve.accepted() {
        return RetainedAttemptExecution {
            solve: Some(solve),
            attempted_geometry,
            mappings: Some(mappings),
            effective_activity: Some(resolved.activity.clone()),
            accepted: None,
            failure: None,
        };
    }
    let accepted_attempt_state;
    let runtime_previous_state = if attempted_request == runtime_request
        || previous_state_intent.is_some_and(RetainedPreviousStateIntent::is_drag_locality)
    {
        &previous_state
    } else {
        // A command-scoped drag is the authoring projection used to produce
        // the new accepted geometry. Once it succeeds, that geometry—not the
        // pre-command seed—is the baseline for the publication runtime.
        accepted_attempt_state = PreviousStateReference::capture(&sketch);
        &accepted_attempt_state
    };

    let incremental_sources = parent
        .map(|parent| incremental_runtime_sources(candidate, &mappings, &input_stamps, parent));
    let incremental = parent
        .zip(incremental_sources.as_deref())
        .filter(|(parent, _)| {
            parent.mappings.has_compatible_runtime_topology(&mappings)
                && parent.runtime.request() == runtime_request
        })
        .map(|(parent, sources)| {
            let mut runtime = parent.runtime.clone();
            runtime
                .apply_compatible_candidate(
                    sketch.clone(),
                    runtime_request,
                    sources,
                    runtime_previous_state,
                    None,
                )
                .map(|result| result.map(|result| (runtime, result)))
        });
    let runtime = match incremental {
        Some(Ok(Some((runtime, result)))) if result.accepted() => runtime,
        Some(Ok(Some((_, rejected)))) => {
            return RetainedAttemptExecution {
                attempted_geometry: rejected.attempted_geometry.clone(),
                solve: Some(rejected),
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                accepted: None,
                failure: None,
            };
        }
        Some(Ok(None)) => unreachable!("uncontrolled incremental solve cannot be interrupted"),
        Some(Err(SketchSessionError::RebuildRequired)) | None => {
            match SketchSession::new_with_previous_state_reference(
                sketch,
                runtime_request,
                config,
                runtime_previous_state,
            ) {
                Ok(mut runtime) => {
                    if parent.is_some() {
                        runtime.mark_full_rebuild();
                    }
                    runtime
                }
                Err(error) => {
                    return RetainedAttemptExecution {
                        solve: None,
                        attempted_geometry,
                        mappings: Some(mappings),
                        effective_activity: Some(resolved.activity.clone()),
                        accepted: None,
                        failure: Some(SketchAttemptFailure {
                            kind: SketchAttemptFailureKind::AcceptedSession,
                            message: error.to_string(),
                            parameter_issue: None,
                            external_error: None,
                            effective_activity: None,
                        }),
                    };
                }
            }
        }
        Some(Err(error)) => {
            return RetainedAttemptExecution {
                solve: None,
                attempted_geometry,
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                accepted: None,
                failure: Some(SketchAttemptFailure {
                    kind: SketchAttemptFailureKind::AcceptedSession,
                    message: error.to_string(),
                    parameter_issue: None,
                    external_error: None,
                    effective_activity: None,
                }),
            };
        }
    };
    let mut document = candidate.clone();
    if let Err(error) = document.project_accepted_state(runtime.sketch(), &mappings) {
        let mut rejected = runtime.accepted_result().clone();
        rejected.rejection = Some(SolveRejection::IndependentValidationFailed(
            error.to_string(),
        ));
        rejected.acceptance_hard_residual_max = None;
        rejected.core_report.hard_validity = HardValidity::Invalid;
        rejected.core_report.termination = SolveTermination::Stalled;
        return RetainedAttemptExecution {
            attempted_geometry: rejected.attempted_geometry.clone(),
            solve: Some(rejected),
            mappings: Some(mappings),
            effective_activity: Some(resolved.activity.clone()),
            accepted: None,
            failure: None,
        };
    }
    let solve = runtime.accepted_result().clone();
    RetainedAttemptExecution {
        attempted_geometry: solve.attempted_geometry.clone(),
        solve: Some(solve),
        mappings: Some(mappings.clone()),
        effective_activity: Some(resolved.activity),
        accepted: Some((document, runtime, mappings, input_stamps)),
        failure: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_retained_attempt_controlled(
    candidate: &SketchDocument,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    config: SolverConfig,
    parent: Option<&SketchAcceptedDocumentState>,
    controller: &mut OperationController,
) -> Option<RetainedAttemptExecution> {
    run_retained_attempt_with_previous_state_reference_controlled(
        candidate,
        parameters,
        snapshots,
        request,
        command_drag,
        config,
        parent,
        None,
        controller,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_retained_attempt_with_previous_state_reference_controlled(
    candidate: &SketchDocument,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    config: SolverConfig,
    parent: Option<&SketchAcceptedDocumentState>,
    previous_state_intent: Option<RetainedPreviousStateIntent<'_>>,
    controller: &mut OperationController,
) -> Option<RetainedAttemptExecution> {
    let resolved = match resolve_attempt_inputs(candidate, parameters, snapshots) {
        Ok(resolved) => resolved,
        Err(AttemptInputError::Parameter(error)) => {
            return Some(RetainedAttemptExecution::parameter_input_failure(&error));
        }
        Err(AttemptInputError::External { error, activity }) => {
            return Some(RetainedAttemptExecution::external_input_failure(
                error, activity,
            ));
        }
    };
    let input_stamps = RetainedAttemptInputStamps {
        activity: resolved.activity.clone(),
        activation_revision: resolved.activity.activation_revision(),
        activation_digest: resolved.activity.activation_digest(),
        parameter_revision: parameters.revision(),
        parameter_digest: parameters.digest(),
        external_revision: resolved.external_revision,
        external_digest: resolved.external_digest,
    };
    let lowered =
        match candidate.lower_with_resolved_parameters_with_controller(&resolved, controller) {
            Ok(Some(lowered)) => lowered,
            Ok(None) => return None,
            Err(error) => {
                return Some(RetainedAttemptExecution {
                    effective_activity: Some(resolved.activity.clone()),
                    ..RetainedAttemptExecution::failure(
                        SketchAttemptFailureKind::Lowering,
                        error.to_string(),
                    )
                });
            }
        };
    let (mut sketch, mappings) = lowered.into_parts();
    let validation_sketch = sketch.clone();
    let previous_state = match retained_previous_state_reference(
        &validation_sketch,
        &mappings,
        previous_state_intent,
    ) {
        Ok(previous_state) => previous_state,
        Err(error) => {
            return Some(RetainedAttemptExecution {
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Request,
                    error.to_string(),
                )
            });
        }
    };
    let runtime_request = match lower_request(request, &mappings) {
        Ok(request) => request,
        Err(error) => {
            return Some(RetainedAttemptExecution {
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Request,
                    error.to_string(),
                )
            });
        }
    };
    let attempted_request = match lower_request(
        effective_attempt_request_with_previous_state_intent(
            request,
            command_drag,
            previous_state_intent,
        ),
        &mappings,
    ) {
        Ok(request) => request,
        Err(error) => {
            return Some(RetainedAttemptExecution {
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Request,
                    error.to_string(),
                )
            });
        }
    };
    if previous_state_intent.is_some_and(RetainedPreviousStateIntent::is_no_motion_publication) {
        let runtime =
            match SketchSession::certify_current_state_with_previous_state_reference_and_controller(
                sketch,
                runtime_request,
                config,
                &previous_state,
                controller,
            ) {
                Ok(Some(runtime)) => runtime,
                Ok(None) => return None,
                Err(error) => {
                    return Some(RetainedAttemptExecution {
                        solve: None,
                        attempted_geometry: None,
                        mappings: Some(mappings),
                        effective_activity: Some(resolved.activity.clone()),
                        accepted: None,
                        failure: Some(SketchAttemptFailure {
                            kind: SketchAttemptFailureKind::AcceptedSession,
                            message: error.to_string(),
                            parameter_issue: None,
                            external_error: None,
                            effective_activity: None,
                        }),
                    });
                }
            };
        if controller
            .checkpoint(OperationCheckpoint::BeforeFinalValidation)
            .is_err()
        {
            return None;
        }
        let mut document = candidate.clone();
        let projected = document.project_accepted_state_with_controller(
            runtime.sketch(),
            &mappings,
            controller,
        );
        if matches!(projected, Ok(false)) {
            return None;
        }
        if controller
            .checkpoint(OperationCheckpoint::AfterFinalValidation)
            .is_err()
        {
            return None;
        }
        if let Err(error) = projected {
            let mut rejected = runtime.accepted_result().clone();
            rejected.rejection = Some(SolveRejection::IndependentValidationFailed(
                error.to_string(),
            ));
            rejected.acceptance_hard_residual_max = None;
            rejected.core_report.hard_validity = HardValidity::Invalid;
            rejected.core_report.termination = SolveTermination::Stalled;
            return Some(RetainedAttemptExecution {
                attempted_geometry: rejected.attempted_geometry.clone(),
                solve: Some(rejected),
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                accepted: None,
                failure: None,
            });
        }
        let solve = runtime.accepted_result().clone();
        return Some(RetainedAttemptExecution {
            attempted_geometry: solve.attempted_geometry.clone(),
            solve: Some(solve),
            mappings: Some(mappings.clone()),
            effective_activity: Some(resolved.activity),
            accepted: Some((document, runtime, mappings, input_stamps)),
            failure: None,
        });
    }
    if attempted_request == runtime_request
        && let Some(parent) = parent.filter(|parent| {
            parent.mappings.has_compatible_runtime_topology(&mappings)
                && parent.runtime.request() == runtime_request
        })
    {
        let sources = incremental_runtime_sources(candidate, &mappings, &input_stamps, parent);
        let mut runtime = parent.runtime.clone();
        match runtime.apply_compatible_candidate(
            sketch.clone(),
            runtime_request,
            &sources,
            &previous_state,
            Some(controller),
        ) {
            Ok(Some(result)) if result.accepted() => {
                if controller
                    .checkpoint(OperationCheckpoint::BeforeFinalValidation)
                    .is_err()
                {
                    return None;
                }
                let mut document = candidate.clone();
                let projected = document.project_accepted_state_with_controller(
                    runtime.sketch(),
                    &mappings,
                    controller,
                );
                if matches!(projected, Ok(false)) {
                    return None;
                }
                if controller
                    .checkpoint(OperationCheckpoint::AfterFinalValidation)
                    .is_err()
                {
                    return None;
                }
                if let Err(error) = projected {
                    let mut rejected = result;
                    rejected.rejection = Some(SolveRejection::IndependentValidationFailed(
                        error.to_string(),
                    ));
                    rejected.acceptance_hard_residual_max = None;
                    rejected.core_report.hard_validity = HardValidity::Invalid;
                    rejected.core_report.termination = SolveTermination::Stalled;
                    return Some(RetainedAttemptExecution {
                        attempted_geometry: rejected.attempted_geometry.clone(),
                        solve: Some(rejected),
                        mappings: Some(mappings),
                        effective_activity: Some(resolved.activity.clone()),
                        accepted: None,
                        failure: None,
                    });
                }
                let solve = runtime.accepted_result().clone();
                return Some(RetainedAttemptExecution {
                    attempted_geometry: solve.attempted_geometry.clone(),
                    solve: Some(solve),
                    mappings: Some(mappings.clone()),
                    effective_activity: Some(resolved.activity),
                    accepted: Some((document, runtime, mappings, input_stamps)),
                    failure: None,
                });
            }
            Ok(Some(rejected)) => {
                return Some(RetainedAttemptExecution {
                    attempted_geometry: rejected.attempted_geometry.clone(),
                    solve: Some(rejected),
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    accepted: None,
                    failure: None,
                });
            }
            Ok(None) => return None,
            Err(SketchSessionError::RebuildRequired) => {}
            Err(error) => {
                return Some(RetainedAttemptExecution {
                    solve: None,
                    attempted_geometry: None,
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    accepted: None,
                    failure: Some(SketchAttemptFailure {
                        kind: SketchAttemptFailureKind::AcceptedSession,
                        message: error.to_string(),
                        parameter_issue: None,
                        external_error: None,
                        effective_activity: None,
                    }),
                });
            }
        }
    }
    let solve = match sketch.solve_with_previous_state_reference_and_controller(
        attempted_request,
        config,
        &previous_state,
        controller,
    ) {
        Ok(Some(solve)) => solve,
        Ok(None) => return None,
        Err(error) => {
            return Some(RetainedAttemptExecution {
                mappings: Some(mappings),
                effective_activity: Some(resolved.activity.clone()),
                ..RetainedAttemptExecution::failure(
                    SketchAttemptFailureKind::Solve,
                    error.to_string(),
                )
            });
        }
    };
    let attempted_geometry = solve.attempted_geometry.clone();
    if !solve.accepted() {
        return Some(RetainedAttemptExecution {
            solve: Some(solve),
            attempted_geometry,
            mappings: Some(mappings),
            effective_activity: Some(resolved.activity.clone()),
            accepted: None,
            failure: None,
        });
    }
    let accepted_attempt_state;
    let runtime_previous_state = if attempted_request == runtime_request
        || previous_state_intent.is_some_and(RetainedPreviousStateIntent::is_drag_locality)
    {
        &previous_state
    } else {
        accepted_attempt_state = PreviousStateReference::capture(&sketch);
        &accepted_attempt_state
    };
    if controller
        .checkpoint(OperationCheckpoint::BeforeFinalValidation)
        .is_err()
    {
        return None;
    }
    let mut incremental_runtime = None;
    if let Some(parent) = parent.filter(|parent| {
        parent.mappings.has_compatible_runtime_topology(&mappings)
            && parent.runtime.request() == runtime_request
    }) {
        let sources = incremental_runtime_sources(candidate, &mappings, &input_stamps, parent);
        let mut runtime = parent.runtime.clone();
        match runtime.apply_compatible_candidate(
            sketch.clone(),
            runtime_request,
            &sources,
            runtime_previous_state,
            Some(controller),
        ) {
            Ok(Some(result)) if result.accepted() => incremental_runtime = Some(runtime),
            Ok(Some(rejected)) => {
                return Some(RetainedAttemptExecution {
                    attempted_geometry: rejected.attempted_geometry.clone(),
                    solve: Some(rejected),
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    accepted: None,
                    failure: None,
                });
            }
            Ok(None) => return None,
            Err(SketchSessionError::RebuildRequired) => {}
            Err(error) => {
                return Some(RetainedAttemptExecution {
                    solve: None,
                    attempted_geometry,
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    accepted: None,
                    failure: Some(SketchAttemptFailure {
                        kind: SketchAttemptFailureKind::AcceptedSession,
                        message: error.to_string(),
                        parameter_issue: None,
                        external_error: None,
                        effective_activity: None,
                    }),
                });
            }
        }
    }
    let runtime = if let Some(runtime) = incremental_runtime {
        runtime
    } else {
        let runtime_solve = if attempted_request == runtime_request {
            solve.clone()
        } else {
            match sketch.solve_with_previous_state_reference_and_controller(
                runtime_request,
                config,
                runtime_previous_state,
                controller,
            ) {
                Ok(Some(solve)) => solve,
                Ok(None) => return None,
                Err(error) => {
                    return Some(RetainedAttemptExecution {
                        solve: None,
                        attempted_geometry,
                        mappings: Some(mappings),
                        effective_activity: Some(resolved.activity.clone()),
                        accepted: None,
                        failure: Some(SketchAttemptFailure {
                            kind: SketchAttemptFailureKind::AcceptedSession,
                            message: error.to_string(),
                            parameter_issue: None,
                            external_error: None,
                            effective_activity: None,
                        }),
                    });
                }
            }
        };
        let mut runtime = match SketchSession::from_accepted_solve_with_controller(
            sketch,
            &validation_sketch,
            runtime_request,
            config,
            runtime_solve,
            runtime_previous_state,
            controller,
        ) {
            Ok(Some(runtime)) => runtime,
            Ok(None) => return None,
            Err(error) => {
                return Some(RetainedAttemptExecution {
                    solve: None,
                    attempted_geometry,
                    mappings: Some(mappings),
                    effective_activity: Some(resolved.activity.clone()),
                    accepted: None,
                    failure: Some(SketchAttemptFailure {
                        kind: SketchAttemptFailureKind::AcceptedSession,
                        message: error.to_string(),
                        parameter_issue: None,
                        external_error: None,
                        effective_activity: None,
                    }),
                });
            }
        };
        if parent.is_some() {
            runtime.mark_full_rebuild();
        }
        runtime
    };
    let mut document = candidate.clone();
    let projected =
        document.project_accepted_state_with_controller(runtime.sketch(), &mappings, controller);
    if matches!(projected, Ok(false)) {
        return None;
    }
    if controller
        .checkpoint(OperationCheckpoint::AfterFinalValidation)
        .is_err()
    {
        return None;
    }
    if let Err(error) = projected {
        let mut rejected = runtime.accepted_result().clone();
        rejected.rejection = Some(SolveRejection::IndependentValidationFailed(
            error.to_string(),
        ));
        rejected.acceptance_hard_residual_max = None;
        rejected.core_report.hard_validity = HardValidity::Invalid;
        rejected.core_report.termination = SolveTermination::Stalled;
        return Some(RetainedAttemptExecution {
            attempted_geometry: rejected.attempted_geometry.clone(),
            solve: Some(rejected),
            mappings: Some(mappings),
            effective_activity: Some(resolved.activity.clone()),
            accepted: None,
            failure: None,
        });
    }
    let solve = runtime.accepted_result().clone();
    Some(RetainedAttemptExecution {
        attempted_geometry: solve.attempted_geometry.clone(),
        solve: Some(solve),
        mappings: Some(mappings.clone()),
        effective_activity: Some(resolved.activity),
        accepted: Some((document, runtime, mappings, input_stamps)),
        failure: None,
    })
}

#[derive(Clone, Copy)]
struct RetainedAttemptPublication<'a> {
    solved_design: &'a SketchDocument,
    design_provenance: &'a Arc<()>,
    parent_design_provenance: Option<&'a Arc<()>>,
    input: &'a SketchAttemptInput,
    attempt_identity: SketchAttemptIdentity,
    parent_accepted: Option<&'a SketchAcceptedDocumentState>,
    next_accepted_revision: Option<u64>,
}

fn current_attempt_publication<'a>(
    session: &'a RetainedSketchDocumentSession,
    input: &'a SketchAttemptInput,
    attempt_identity: SketchAttemptIdentity,
    parent_accepted: Option<&'a SketchAcceptedDocumentState>,
) -> RetainedAttemptPublication<'a> {
    RetainedAttemptPublication {
        solved_design: &session.design,
        design_provenance: &session.design_provenance,
        parent_design_provenance: session.parent_design_provenance.as_ref(),
        input,
        attempt_identity,
        parent_accepted,
        next_accepted_revision: next_accepted_revision(session.accepted_revision_high_water),
    }
}

fn publish_retained_attempt(
    publication: RetainedAttemptPublication<'_>,
    mut execution: RetainedAttemptExecution,
) -> (SketchDocumentAttempt, Option<SketchAcceptedDocumentState>) {
    let input = *publication.input;
    let published = publish_accepted_state(&publication, &mut execution);
    let accepted_state = published
        .as_ref()
        .map(SketchAcceptedDocumentState::identity);
    let attempt = SketchDocumentAttempt {
        identity: publication.attempt_identity,
        input,
        design_provenance: Arc::clone(publication.design_provenance),
        parent_design_provenance: publication.parent_design_provenance.map(Arc::clone),
        parent_accepted: publication
            .parent_accepted
            .map(SketchAcceptedDocumentState::identity),
        parent_accepted_provenance: publication
            .parent_accepted
            .map(|accepted| Arc::clone(&accepted.provenance)),
        accepted_state,
        solve: execution.solve,
        attempted_geometry: execution.attempted_geometry,
        mappings: execution.mappings,
        effective_activity: execution.effective_activity,
        failure: execution.failure,
    };
    debug_assert!(
        attempt.accepted_state.is_some()
            || attempt.solve.as_ref().is_none_or(|solve| !solve.accepted())
    );
    (attempt, published)
}

fn publish_accepted_state(
    publication: &RetainedAttemptPublication<'_>,
    execution: &mut RetainedAttemptExecution,
) -> Option<SketchAcceptedDocumentState> {
    let (document, runtime, mappings, stamps) = execution.accepted.take()?;
    let input = *publication.input;
    if let Some(message) = stale_publication_input(&input, &stamps) {
        let effective_activity = execution.effective_activity.take();
        *execution =
            RetainedAttemptExecution::failure(SketchAttemptFailureKind::Publication, message);
        execution.effective_activity = effective_activity;
        return None;
    }
    let Some(revision) = publication.next_accepted_revision else {
        execution.solve = None;
        execution.failure = Some(SketchAttemptFailure {
            kind: SketchAttemptFailureKind::Publication,
            message: "accepted revision space is exhausted".into(),
            parameter_issue: None,
            external_error: None,
            effective_activity: None,
        });
        return None;
    };
    let design_identity = input.design;
    let identity = SketchAcceptedStateIdentity {
        document: design_identity.document,
        revision: SketchAcceptedRevision(revision),
    };
    execution.solve = Some(runtime.accepted_result().clone());
    execution.attempted_geometry = execution
        .solve
        .as_ref()
        .and_then(|solve| solve.attempted_geometry.clone());
    execution.mappings = Some(mappings.clone());
    let redundancy = accepted_document_redundancy(
        identity,
        design_identity,
        runtime.accepted_result(),
        &mappings,
    );
    let parameter_outputs = accepted_parameter_outputs(
        publication.solved_design,
        &input,
        publication.attempt_identity,
        identity,
        runtime.accepted_result(),
        &mappings,
    );
    Some(SketchAcceptedDocumentState {
        identity,
        provenance: Arc::new(()),
        design_provenance: Arc::clone(publication.design_provenance),
        input,
        originating_attempt: publication.attempt_identity,
        solved_design: publication.solved_design.clone(),
        document,
        runtime,
        mappings,
        effective_activity: stamps.activity,
        redundancy,
        parameter_outputs,
        profile_cache: RefCell::new(Vec::new()),
    })
}

fn stale_publication_input(
    input: &SketchAttemptInput,
    stamps: &RetainedAttemptInputStamps,
) -> Option<String> {
    if input.effective_activation_revision() != stamps.activation_revision
        || input.activation_digest() != stamps.activation_digest
    {
        Some("effective activation input changed before publication".into())
    } else if input.parameter_revision() != stamps.parameter_revision
        || input.parameter_digest() != stamps.parameter_digest
    {
        Some("parameter input changed before publication".into())
    } else if input.external_snapshot_set_revision() != stamps.external_revision
        || input.external_snapshot_set_digest() != stamps.external_digest
    {
        Some("external snapshot input changed before publication".into())
    } else {
        None
    }
}

fn accepted_parameter_outputs(
    design: &SketchDocument,
    input: &SketchAttemptInput,
    attempt: SketchAttemptIdentity,
    accepted: SketchAcceptedStateIdentity,
    solve: &SketchSolveResult,
    mappings: &DocumentRuntimeMap,
) -> Vec<DocumentParameterOutputProposal> {
    design
        .parameter_outputs()
        .iter()
        .filter_map(|output| {
            let dimension = design.dimension(output.dimension)?;
            let RuntimeSource::Dimension(runtime) = mappings.runtime_source(dimension.source_id)?
            else {
                return None;
            };
            let value = solve.reference_values.iter().find_map(|entry| {
                (entry.dimension_id == runtime && entry.value.is_finite()).then_some(entry.value)
            })?;
            let unit = match dimension.definition {
                DocumentDimensionDefinition::OrientedAngle { .. } => DocumentScalarUnit::Angle,
                _ => DocumentScalarUnit::Length,
            };
            Some(DocumentParameterOutputProposal {
                parameter: output.parameter,
                dimension: output.dimension,
                source: dimension.source_id,
                unit,
                value,
                design: input.design_identity(),
                attempt,
                accepted,
                parameter_revision: input.parameter_revision(),
                parameter_digest: input.parameter_digest(),
                provenance: DocumentMeasurementProvenance::AcceptedDocument {
                    revision: accepted.revision().0,
                },
            })
        })
        .collect()
}

fn accepted_document_redundancy(
    accepted_state: SketchAcceptedStateIdentity,
    design: SketchDesignIdentity,
    solve: &SketchSolveResult,
    mappings: &DocumentRuntimeMap,
) -> SketchAcceptedDocumentRedundancy {
    let runtime = solve
        .accepted_redundancy()
        .expect("published state always has an accepted solve result");
    let persistent_sources = |sources: &[SketchSource]| {
        sources
            .iter()
            .filter_map(|source| {
                let runtime = match source {
                    SketchSource::Constraint(id) => RuntimeSource::Constraint(*id),
                    SketchSource::Dimension(id) => RuntimeSource::Dimension(*id),
                    SketchSource::DragTarget(_) | SketchSource::PreviousState(_) => return None,
                };
                mappings.source_mappings().iter().find_map(|mapping| {
                    (mapping.runtime == Some(runtime)).then_some(mapping.source_id)
                })
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    SketchAcceptedDocumentRedundancy {
        accepted_state,
        design,
        fully_redundant_sources: persistent_sources(runtime.fully_redundant_sources()),
        sources_containing_redundant_rows: persistent_sources(
            runtime.sources_containing_redundant_rows(),
        ),
    }
}

fn seed_from_accepted_parent(
    design: &SketchDocument,
    parent: Option<&SketchAcceptedDocumentState>,
) -> Result<SketchDocument, DocumentError> {
    let Some(parent) = parent else {
        return Ok(design.clone());
    };
    let mut seed = design.clone();
    for point in design.points() {
        let Some(parent_design) = parent.solved_design.point(point.id) else {
            continue;
        };
        let Some(parent_accepted) = parent.document.point(point.id) else {
            continue;
        };
        if pair_bits(point.position) == pair_bits(parent_design.position) {
            seed.set_point_position(point.id, parent_accepted.position)?;
        }
    }
    for scalar in design.scalars() {
        let Some(parent_design) = parent.solved_design.scalar(scalar.id) else {
            continue;
        };
        let Some(parent_accepted) = parent.document.scalar(scalar.id) else {
            continue;
        };
        if scalar.value.to_bits() == parent_design.value.to_bits() {
            seed.scalar_mut(scalar.id)
                .expect("scalar came from this document")
                .value = parent_accepted.value;
        }
    }
    for curve in design.curves() {
        let CurveDefinition::RationalQuadraticConic {
            weighted_middle, ..
        } = curve.definition
        else {
            continue;
        };
        let Some(parent_design) = parent.solved_design.curve(curve.id) else {
            continue;
        };
        let CurveDefinition::RationalQuadraticConic {
            weighted_middle: parent_design_middle,
            ..
        } = parent_design.definition
        else {
            continue;
        };
        let Some(parent_accepted) = parent.document.curve(curve.id) else {
            continue;
        };
        let CurveDefinition::RationalQuadraticConic {
            weighted_middle: parent_accepted_middle,
            ..
        } = parent_accepted.definition
        else {
            continue;
        };
        if pair_bits(weighted_middle) == pair_bits(parent_design_middle) {
            seed.set_conic_weighted_middle(curve.id, parent_accepted_middle)?;
        }
    }
    Ok(seed)
}

fn seed_from_accepted_parent_controlled(
    design: &SketchDocument,
    parent: Option<&SketchAcceptedDocumentState>,
    controller: &mut OperationController,
) -> Result<Option<SketchDocument>, DocumentError> {
    let Some(parent) = parent else {
        return Ok(Some(design.clone()));
    };
    let mut seed = design.clone();
    for point in design.points() {
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(None);
        }
        let (Some(parent_design), Some(parent_accepted)) = (
            parent.solved_design.point(point.id),
            parent.document.point(point.id),
        ) else {
            continue;
        };
        if pair_bits(point.position) == pair_bits(parent_design.position) {
            seed.point_mut(point.id)
                .expect("point came from this document")
                .position = parent_accepted.position;
        }
    }
    for scalar in design.scalars() {
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(None);
        }
        let (Some(parent_design), Some(parent_accepted)) = (
            parent.solved_design.scalar(scalar.id),
            parent.document.scalar(scalar.id),
        ) else {
            continue;
        };
        if scalar.value.to_bits() == parent_design.value.to_bits() {
            seed.scalar_mut(scalar.id)
                .expect("scalar came from this document")
                .value = parent_accepted.value;
        }
    }
    for curve in design.curves() {
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(None);
        }
        let CurveDefinition::RationalQuadraticConic {
            weighted_middle, ..
        } = curve.definition
        else {
            continue;
        };
        let (Some(parent_design), Some(parent_accepted)) = (
            parent.solved_design.curve(curve.id),
            parent.document.curve(curve.id),
        ) else {
            continue;
        };
        let (
            CurveDefinition::RationalQuadraticConic {
                weighted_middle: parent_design_middle,
                ..
            },
            CurveDefinition::RationalQuadraticConic {
                weighted_middle: parent_accepted_middle,
                ..
            },
        ) = (&parent_design.definition, &parent_accepted.definition)
        else {
            continue;
        };
        if pair_bits(weighted_middle) == pair_bits(*parent_design_middle) {
            let CurveDefinition::RationalQuadraticConic {
                weighted_middle: seed_middle,
                ..
            } = &mut seed
                .curve_mut(curve.id)
                .expect("curve came from this document")
                .definition
            else {
                unreachable!("curve family came from this document");
            };
            *seed_middle = *parent_accepted_middle;
        }
    }
    if !seed.validate_with_controller(Some(controller))? {
        return Ok(None);
    }
    Ok(Some(seed))
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    value.map(f64::to_bits)
}

fn completed_preview_publication<T>(
    outcome: OperationOutcome<T>,
) -> Result<T, DocumentSessionError> {
    match outcome {
        OperationOutcome::Completed { value, .. } => Ok(value),
        OperationOutcome::Cancelled { report } | OperationOutcome::WorkExhausted { report } => {
            Err(DocumentSessionError::PreviewPublicationStopped {
                report: Box::new(report),
            })
        }
        outcome => Err(DocumentSessionError::PreviewPublicationStopped {
            report: Box::new(*outcome.report()),
        }),
    }
}

fn validate_drag_publication_continuity(
    preview: &SketchDocument,
    published: &SketchDocument,
    point: DesignPointId,
    position: [f64; 2],
    locality: &DocumentDragLocalityPlan,
) -> Result<(), DocumentSessionError> {
    let geometry_tolerance = validate_drag_publication_active(preview, published, point, position)?;
    validate_drag_publication_anchors(preview, published, locality, geometry_tolerance)?;
    validate_drag_publication_contacts(preview, published)?;
    Ok(())
}

fn validate_drag_publication_active(
    preview: &SketchDocument,
    published: &SketchDocument,
    point: DesignPointId,
    position: [f64; 2],
) -> Result<f64, DocumentSessionError> {
    if preview.model_scale().to_bits() != published.model_scale().to_bits()
        || !preview.model_scale().is_finite()
        || preview.model_scale() <= 0.0
    {
        return Err(DocumentSessionError::DragPublicationContinuity {
            context: "the preview and publication model scales differ",
        });
    }
    let geometry_tolerance = 1.0e-8 * preview.model_scale();
    let preview_active = preview
        .point(point)
        .ok_or(DocumentSessionError::DragPublicationContinuity {
            context: "the active point is absent from the preview",
        })?
        .position;
    let published_active = published
        .point(point)
        .ok_or(DocumentSessionError::DragPublicationContinuity {
            context: "the active point is absent from the publication",
        })?
        .position;
    if !preview_active
        .iter()
        .chain(published_active.iter())
        .all(|value| value.is_finite())
        || pair_bits(preview_active) != pair_bits(position)
        || point_distance(preview_active, published_active) > geometry_tolerance
    {
        return Err(DocumentSessionError::DragPublicationContinuity {
            context: "the active point changed from the accepted preview",
        });
    }
    Ok(geometry_tolerance)
}

fn validate_drag_publication_anchors(
    preview: &SketchDocument,
    published: &SketchDocument,
    locality: &DocumentDragLocalityPlan,
    geometry_tolerance: f64,
) -> Result<(), DocumentSessionError> {
    for anchor in locality.anchors() {
        let anchor_target = anchor.target();
        let preview_position = preview
            .point(anchor.point())
            .ok_or(DocumentSessionError::DragPublicationContinuity {
                context: "a frozen anchor is absent from the preview",
            })?
            .position;
        let published_position = published
            .point(anchor.point())
            .ok_or(DocumentSessionError::DragPublicationContinuity {
                context: "a frozen anchor is absent from the publication",
            })?
            .position;
        if !preview_position
            .iter()
            .chain(published_position.iter())
            .chain(anchor_target.iter())
            .all(|value| value.is_finite())
        {
            return Err(DocumentSessionError::DragPublicationContinuity {
                context: "a frozen anchor position is non-finite",
            });
        }
        if point_distance(preview_position, anchor_target) > geometry_tolerance {
            return Err(DocumentSessionError::DragPublicationContinuity {
                context: "a preview anchor moved from its gesture-start target",
            });
        }
        if point_distance(published_position, anchor_target) > geometry_tolerance {
            return Err(DocumentSessionError::DragPublicationContinuity {
                context: "a published anchor moved from its gesture-start target",
            });
        }
    }
    Ok(())
}

fn validate_drag_publication_contacts(
    preview: &SketchDocument,
    published: &SketchDocument,
) -> Result<(), DocumentSessionError> {
    if preview.contacts().len() != published.contacts().len() {
        return Err(DocumentSessionError::DragPublicationContinuity {
            context: "the persistent contact set changed during publication",
        });
    }
    for preview_contact in preview.contacts() {
        let published_contact = published.contact(preview_contact.id).ok_or(
            DocumentSessionError::DragPublicationContinuity {
                context: "a persistent preview contact is absent from the publication",
            },
        )?;
        if published_contact != preview_contact {
            return Err(DocumentSessionError::DragPublicationContinuity {
                context: "persistent contact branch metadata changed during publication",
            });
        }
        let preview_parameter = preview
            .scalar(preview_contact.parameter)
            .ok_or(DocumentSessionError::DragPublicationContinuity {
                context: "a preview contact parameter is absent",
            })?
            .value;
        let published_parameter = published
            .scalar(published_contact.parameter)
            .ok_or(DocumentSessionError::DragPublicationContinuity {
                context: "a published contact parameter is absent",
            })?
            .value;
        let parameter_tolerance = 1.0e-9
            * preview_parameter
                .abs()
                .max(published_parameter.abs())
                .max(1.0);
        if !preview_parameter.is_finite()
            || !published_parameter.is_finite()
            || (preview_parameter - published_parameter).abs() > parameter_tolerance
        {
            return Err(DocumentSessionError::DragPublicationContinuity {
                context: "a contact parameter changed from the accepted preview",
            });
        }
    }
    Ok(())
}

fn point_distance(first: [f64; 2], second: [f64; 2]) -> f64 {
    (first[0] - second[0]).hypot(first[1] - second[1])
}

fn incident_line_branch_spans(
    document: &SketchDocument,
    point: DesignPointId,
) -> BTreeSet<CurveSpan> {
    let mut spans = BTreeSet::new();
    for curve in document.curves() {
        match &curve.definition {
            CurveDefinition::Line { start, end, .. } if *start == point || *end == point => {
                spans.insert(CurveSpan::line(curve.id));
            }
            CurveDefinition::Polyline { points, closed, .. } => {
                for (segment, pair) in points.windows(2).enumerate() {
                    if pair.contains(&point) {
                        spans.insert(CurveSpan {
                            curve: curve.id,
                            segment: u32::try_from(segment).unwrap_or(u32::MAX),
                        });
                    }
                }
                if *closed
                    && points.len() > 2
                    && let (Some(&start), Some(&end)) = (points.last(), points.first())
                    && (start == point || end == point)
                {
                    spans.insert(CurveSpan {
                        curve: curve.id,
                        segment: u32::try_from(points.len() - 1).unwrap_or(u32::MAX),
                    });
                }
            }
            _ => {}
        }
    }
    spans
}

fn branch_edits_cover_incident_lines(
    document: &SketchDocument,
    point: DesignPointId,
    branches: &[DocumentCurveBranchEdit],
) -> bool {
    let expected = incident_line_branch_spans(document, point);
    let supplied = branches
        .iter()
        .map(|branch| branch.curve)
        .collect::<BTreeSet<_>>();
    !expected.is_empty() && branches.len() == expected.len() && supplied == expected
}

struct AttemptedDocument {
    accepted: Option<(SketchDocument, SketchSession, DocumentRuntimeMap)>,
    result: DocumentSolveResult,
}

fn attempt_document(
    candidate: &SketchDocument,
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    config: SolverConfig,
) -> Result<AttemptedDocument, DocumentSessionError> {
    let lowered = candidate.lower()?;
    let (mut sketch, mappings) = lowered.into_parts();
    let runtime_request = lower_request(request, &mappings)?;
    let attempted_request = lower_request(
        DocumentSolveRequest {
            drag: command_drag.or(request.drag),
            previous_state_preferences: command_drag.is_none()
                && request.previous_state_preferences,
        },
        &mappings,
    )?;
    let solve = sketch.solve(attempted_request, config)?;
    if solve.rejection.is_some() {
        return Ok(AttemptedDocument {
            accepted: None,
            result: DocumentSolveResult::new(solve, mappings),
        });
    }
    let runtime = SketchSession::new(sketch, runtime_request, config)?;
    let mut document = candidate.clone();
    if let Err(error) = document.project_accepted_state(runtime.sketch(), &mappings) {
        let mut solve = runtime.accepted_result().clone();
        solve.rejection = Some(SolveRejection::IndependentValidationFailed(
            error.to_string(),
        ));
        solve.acceptance_hard_residual_max = None;
        solve.core_report.hard_validity = HardValidity::Invalid;
        solve.core_report.termination = SolveTermination::Stalled;
        return Ok(AttemptedDocument {
            accepted: None,
            result: DocumentSolveResult::new(solve, mappings),
        });
    }
    let result = DocumentSolveResult::new(runtime.accepted_result().clone(), mappings.clone());
    Ok(AttemptedDocument {
        accepted: Some((document, runtime, mappings)),
        result,
    })
}

fn attempt_document_controlled(
    candidate: &SketchDocument,
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    config: SolverConfig,
    controller: &mut OperationController,
) -> Result<Option<AttemptedDocument>, DocumentSessionError> {
    let Some(lowered) = candidate.lower_with_controller(controller)? else {
        return Ok(None);
    };
    let (mut sketch, mappings) = lowered.into_parts();
    let validation_sketch = sketch.clone();
    let previous_state = PreviousStateReference::capture(&validation_sketch);
    let runtime_request = lower_request(request, &mappings)?;
    let attempted_request = lower_request(
        DocumentSolveRequest {
            drag: command_drag.or(request.drag),
            previous_state_preferences: command_drag.is_none()
                && request.previous_state_preferences,
        },
        &mappings,
    )?;
    let Some(solve) = sketch.solve_with_previous_state_reference_and_controller(
        attempted_request,
        config,
        &previous_state,
        controller,
    )?
    else {
        return Ok(None);
    };
    if solve.rejection.is_some() {
        return Ok(Some(AttemptedDocument {
            accepted: None,
            result: DocumentSolveResult::new(solve, mappings),
        }));
    }
    if controller
        .checkpoint(OperationCheckpoint::BeforeFinalValidation)
        .is_err()
    {
        return Ok(None);
    }
    let accepted_attempt_state;
    let runtime_previous_state = if attempted_request == runtime_request {
        &previous_state
    } else {
        accepted_attempt_state = PreviousStateReference::capture(&sketch);
        &accepted_attempt_state
    };
    let Some(runtime_solve) = sketch.solve_with_previous_state_reference_and_controller(
        runtime_request,
        config,
        runtime_previous_state,
        controller,
    )?
    else {
        return Ok(None);
    };
    let Some(runtime) = SketchSession::from_accepted_solve_with_controller(
        sketch,
        &validation_sketch,
        runtime_request,
        config,
        runtime_solve,
        runtime_previous_state,
        controller,
    )?
    else {
        return Ok(None);
    };
    let mut document = candidate.clone();
    let projected =
        document.project_accepted_state_with_controller(runtime.sketch(), &mappings, controller);
    if matches!(projected, Ok(false)) {
        return Ok(None);
    }
    if controller
        .checkpoint(OperationCheckpoint::AfterFinalValidation)
        .is_err()
    {
        return Ok(None);
    }
    if let Err(error) = projected {
        let mut solve = runtime.accepted_result().clone();
        solve.rejection = Some(SolveRejection::IndependentValidationFailed(
            error.to_string(),
        ));
        solve.acceptance_hard_residual_max = None;
        solve.core_report.hard_validity = HardValidity::Invalid;
        solve.core_report.termination = SolveTermination::Stalled;
        return Ok(Some(AttemptedDocument {
            accepted: None,
            result: DocumentSolveResult::new(solve, mappings),
        }));
    }
    let result = DocumentSolveResult::new(runtime.accepted_result().clone(), mappings.clone());
    Ok(Some(AttemptedDocument {
        accepted: Some((document, runtime, mappings)),
        result,
    }))
}

fn lower_request(
    request: DocumentSolveRequest,
    mappings: &DocumentRuntimeMap,
) -> Result<SketchSolveRequest, DocumentError> {
    let mut runtime = SketchSolveRequest::new();
    if !request.previous_state_preferences {
        runtime = runtime.without_previous_state_preferences();
    }
    if let Some(drag) = request.drag {
        let point = mappings
            .runtime_point(drag.point)
            .ok_or(DocumentError::UnknownId {
                kind: "drag point",
                id: drag.point.0,
            })?;
        runtime = runtime.with_drag(point, Point2::new(drag.target[0], drag.target[1]));
    }
    Ok(runtime)
}

#[allow(clippy::too_many_lines)]
fn apply_edit(
    document: &mut SketchDocument,
    edit: DocumentEdit,
) -> Result<DocumentCommandEffect, DocumentError> {
    let effect = match edit {
        DocumentEdit::CreatePoint { label, position } => {
            DocumentCommandEffect::CreatedPoint(document.add_point(label, position)?)
        }
        DocumentEdit::CreateScalar {
            label,
            value,
            unit,
            domain,
        } => DocumentCommandEffect::CreatedScalar(document.add_scalar(label, value, unit, domain)?),
        DocumentEdit::CreateCurve { label, definition } => {
            DocumentCommandEffect::CreatedCurve(document.add_curve(label, definition)?)
        }
        DocumentEdit::CreateContact { label, definition } => {
            DocumentCommandEffect::CreatedContact(document.add_contact(label, definition)?)
        }
        DocumentEdit::CreateConstraint { label, definition } => {
            DocumentCommandEffect::CreatedConstraint(document.add_constraint(label, definition)?)
        }
        DocumentEdit::CreateDimension {
            label,
            definition,
            mode,
        } => DocumentCommandEffect::CreatedDimension(
            document.add_dimension(label, definition, mode)?,
        ),
        DocumentEdit::CreateParameter { label, kind } => {
            DocumentCommandEffect::CreatedParameter(document.add_parameter(label, kind)?)
        }
        DocumentEdit::AddParameterBinding { parameter, target } => {
            document.add_parameter_binding(parameter, target)?;
            DocumentCommandEffect::AddedParameterBinding { parameter, target }
        }
        DocumentEdit::RemoveParameterBinding { parameter, target } => {
            document.remove_parameter_binding(parameter, target)?;
            DocumentCommandEffect::RemovedParameterBinding { parameter, target }
        }
        DocumentEdit::AddParameterOutput {
            parameter,
            dimension,
        } => {
            document.add_parameter_output(parameter, dimension)?;
            DocumentCommandEffect::AddedParameterOutput {
                parameter,
                dimension,
            }
        }
        DocumentEdit::RemoveParameterOutput {
            parameter,
            dimension,
        } => {
            document.remove_parameter_output(parameter, dimension)?;
            DocumentCommandEffect::RemovedParameterOutput {
                parameter,
                dimension,
            }
        }
        DocumentEdit::CreateRectangle {
            label,
            origin,
            width,
            height,
        } => DocumentCommandEffect::CreatedRectangle(Box::new(
            document.add_rectangle(&label, origin, width, height)?,
        )),
        DocumentEdit::CreateMirroredCurve {
            label,
            source_curve,
            axis,
        } => DocumentCommandEffect::CreatedMirroredCurve(Box::new(document.add_mirrored_curve(
            &label,
            source_curve,
            axis,
        )?)),
        DocumentEdit::CreateLineLineFillet { label, request } => {
            DocumentCommandEffect::CreatedLineLineFillet(Box::new(
                document.add_line_line_fillet(&label, request)?,
            ))
        }
        DocumentEdit::CreateCurveCurveFillet { label, request } => {
            DocumentCommandEffect::CreatedCurveCurveFillet(Box::new(
                document.add_curve_curve_fillet(&label, request)?,
            ))
        }
        DocumentEdit::SetPointPosition { point, position } => {
            document.set_point_position(point, position)?;
            DocumentCommandEffect::UpdatedPoint(point)
        }
        DocumentEdit::SetScalarValue { scalar, value } => {
            document.set_scalar_value(scalar, value)?;
            DocumentCommandEffect::UpdatedScalar(scalar)
        }
        DocumentEdit::SetCurveBranch { curve, direction } => {
            document.set_curve_branch(curve, direction)?;
            DocumentCommandEffect::UpdatedCurve(curve.curve)
        }
        DocumentEdit::SetArcSweep { curve, sweep } => {
            document.set_arc_sweep(curve, sweep)?;
            DocumentCommandEffect::UpdatedCurve(curve)
        }
        DocumentEdit::SetLineLineFilletBranch {
            constraint,
            first_side,
            second_side,
            endpoint_order,
            sweep,
        } => {
            document.set_line_line_fillet_branch(
                constraint,
                first_side,
                second_side,
                endpoint_order,
                sweep,
            )?;
            DocumentCommandEffect::UpdatedConstraint(constraint)
        }
        DocumentEdit::SetCurveCurveFilletBranch {
            constraint,
            first_side,
            first_trim_endpoint,
            second_side,
            second_trim_endpoint,
            endpoint_order,
            sweep,
        } => {
            document.set_curve_curve_fillet_branch(
                constraint,
                first_side,
                first_trim_endpoint,
                second_side,
                second_trim_endpoint,
                endpoint_order,
                sweep,
            )?;
            DocumentCommandEffect::UpdatedConstraint(constraint)
        }
        DocumentEdit::SetConicWeightedMiddle {
            curve,
            weighted_middle,
        } => {
            document.set_conic_weighted_middle(curve, weighted_middle)?;
            DocumentCommandEffect::UpdatedConicWeightedMiddle(curve)
        }
        DocumentEdit::SetHyperbolaBranch { curve, branch } => {
            document.set_hyperbola_branch(curve, branch)?;
            DocumentCommandEffect::UpdatedHyperbolaBranch(curve)
        }
        DocumentEdit::InsertBSplineKnot { curve, parameter } => {
            DocumentCommandEffect::InsertedBSplineKnot(
                document.insert_bspline_knot(curve, parameter)?,
            )
        }
        DocumentEdit::InsertMirroredBSplineKnot {
            label,
            source_curve,
            mirrored_curve,
            axis,
            parameter,
        } => DocumentCommandEffect::InsertedMirroredBSplineKnot(Box::new(
            document.insert_mirrored_bspline_knot(
                &label,
                source_curve,
                mirrored_curve,
                axis,
                parameter,
            )?,
        )),
        DocumentEdit::TransitionBSplineContact { contact, direction } => {
            document.transition_bspline_contact(contact, direction)?;
            DocumentCommandEffect::UpdatedContacts(vec![contact])
        }
        DocumentEdit::InsertNurbsKnot { curve, parameter } => {
            DocumentCommandEffect::InsertedNurbsKnot(document.insert_nurbs_knot(curve, parameter)?)
        }
        DocumentEdit::TransitionNurbsContact { contact, direction } => {
            document.transition_nurbs_contact(contact, direction)?;
            DocumentCommandEffect::UpdatedContacts(vec![contact])
        }
        DocumentEdit::SetNurbsWeightGauge {
            curve,
            gauge_weight,
        } => {
            document.set_nurbs_weight_gauge(curve, gauge_weight)?;
            DocumentCommandEffect::UpdatedNurbsWeightGauge(curve)
        }
        DocumentEdit::SetContactStates { edits } => {
            let contacts = edits.iter().map(|edit| edit.contact).collect();
            document.set_contact_states(&edits)?;
            DocumentCommandEffect::UpdatedContacts(contacts)
        }
        DocumentEdit::SetContactBranches { edits } => {
            let contacts = edits.iter().map(|edit| edit.contact).collect();
            document.set_contact_branches(&edits)?;
            DocumentCommandEffect::UpdatedContacts(contacts)
        }
        DocumentEdit::SetCircleTangencyBranch {
            constraint,
            mode,
            center_direction,
        } => {
            document.set_circle_tangency_branch(constraint, mode, center_direction)?;
            DocumentCommandEffect::UpdatedConstraint(constraint)
        }
        DocumentEdit::SetDimensionMode { dimension, mode } => {
            document.set_dimension_mode(dimension, mode)?;
            DocumentCommandEffect::UpdatedDimension(dimension)
        }
        DocumentEdit::SetOrientedAngleOrientation {
            dimension,
            orientation,
        } => {
            document.set_oriented_angle_orientation(dimension, orientation)?;
            DocumentCommandEffect::UpdatedDimension(dimension)
        }
        DocumentEdit::SetSourceSuppressed { source, suppressed } => {
            document.set_source_suppressed(source, suppressed)?;
            DocumentCommandEffect::UpdatedSource(source)
        }
        DocumentEdit::SetGeometryRole { curve, role } => {
            document.set_geometry_role(curve, role)?;
            DocumentCommandEffect::UpdatedGeometryRole(curve)
        }
        DocumentEdit::SetElementUserSuppressed {
            element,
            suppressed,
        } => {
            document.set_element_user_suppressed(element, suppressed)?;
            DocumentCommandEffect::UpdatedElementUserSuppression(element)
        }
        DocumentEdit::SetHostConfigurationActivation { activation } => {
            document.set_host_configuration_activation(activation)?;
            DocumentCommandEffect::UpdatedHostConfigurationActivation
        }
        DocumentEdit::Delete { object } => {
            document.remove_with_owned_state(object)?;
            DocumentCommandEffect::Deleted(object)
        }
    };
    Ok(effect)
}
