// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_sketch::{
    ContactNeighborhood, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentId, DocumentTrimParameter,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Current independent computed-feature intent schema.
pub const COMPUTED_FEATURE_DOCUMENT_VERSION: u32 = 1;
/// Defensive byte limit applied before feature-document JSON deserialization.
pub const MAX_COMPUTED_FEATURE_JSON_BYTES: usize = 16 * 1024 * 1024;
/// Defensive bound for persistent computed features.
pub const MAX_COMPUTED_FEATURES: usize = 100_000;
/// Defensive bound for corners across all Fillet sets.
pub const MAX_COMPUTED_FEATURE_CORNERS: usize = 200_000;
/// Defensive bound for one persistent feature label.
pub const MAX_COMPUTED_FEATURE_LABEL_BYTES: usize = 1_024;

static FEATURE_DOCUMENT_NONCE: AtomicU64 = AtomicU64::new(1);

macro_rules! hex_id {
    ($name:ident, $inner:ty, $width:expr, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name($inner);

        impl $name {
            #[must_use]
            pub const fn from_raw(value: $inner) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn raw(self) -> $inner {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, concat!("{:0", $width, "x}"), self.0)
            }
        }

        impl FromStr for $name {
            type Err = ComputedFeatureDocumentError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.len() != $width
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(ComputedFeatureDocumentError::InvalidId(value.to_owned()));
                }
                <$inner>::from_str_radix(value, 16)
                    .map(Self)
                    .map_err(|_| ComputedFeatureDocumentError::InvalidId(value.to_owned()))
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

hex_id!(
    ComputedFeatureDocumentId,
    u128,
    32,
    "Stable identity of one computed-feature sidecar document."
);
hex_id!(
    ComputedFeatureId,
    u64,
    16,
    "Stable monotonic identity of one computed feature."
);
hex_id!(
    ComputedFeatureCornerId,
    u64,
    16,
    "Stable monotonic identity of one corner within a computed feature."
);

/// Monotonic revision of persistent computed-feature intent.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ComputedFeatureRevision(u64);

impl ComputedFeatureRevision {
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Canonical 256-bit digest of the complete persistent feature payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComputedFeatureDocumentDigest([u8; 32]);

impl ComputedFeatureDocumentDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for ComputedFeatureDocumentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for ComputedFeatureDocumentDigest {
    type Err = ComputedFeatureDocumentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ComputedFeatureDocumentError::InvalidDigestEncoding);
        }
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                .map_err(|_| ComputedFeatureDocumentError::InvalidDigestEncoding)?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for ComputedFeatureDocumentDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ComputedFeatureDocumentDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Exact persistent identity consumed by a computed-feature evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputedFeatureDocumentIdentity {
    pub document: ComputedFeatureDocumentId,
    pub sketch_document: DocumentId,
    pub revision: ComputedFeatureRevision,
    pub digest: ComputedFeatureDocumentDigest,
}

/// Allocator cursors retained across Undo/Redo so identities are never reused.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedFeatureAllocatorHighWater {
    pub next_feature_id: ComputedFeatureId,
    pub next_corner_id: ComputedFeatureCornerId,
}

/// Host-retained feature lifecycle high-water used when restoring history.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedFeatureLifecycleHighWater {
    pub revision: ComputedFeatureRevision,
    pub allocator: ComputedFeatureAllocatorHighWater,
}

/// Persistent reference to a native constrained-sketch span.
///
/// V1 has deliberately no computed-edge or external-geometry variant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCurveSpanSource {
    pub span: CurveSpan,
}

/// One explicit parent branch retained by a computed Fillet corner.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedFilletParent {
    pub source: NativeCurveSpanSource,
    pub picked_parameter: f64,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
    pub normal_side: DocumentCurveNormalSide,
    pub retained_endpoint: DocumentFilletTrimEndpoint,
    pub periodic_anchor: Option<DocumentTrimParameter>,
}

/// Authoring input for one persistent Fillet corner.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewComputedFilletCorner {
    pub first: ComputedFilletParent,
    pub second: ComputedFilletParent,
    pub endpoint_order: DocumentFilletEndpointOrder,
    pub sweep: DocumentArcSweep,
}

impl NewComputedFilletCorner {
    /// Canonicalizes parent order by persistent native-span identity while
    /// preserving which contact is the output arc's first endpoint.
    #[must_use]
    pub const fn canonicalized(self) -> Self {
        if source_precedes_or_equals(self.first.source, self.second.source) {
            self
        } else {
            Self {
                first: self.second,
                second: self.first,
                endpoint_order: match self.endpoint_order {
                    DocumentFilletEndpointOrder::FirstThenSecond => {
                        DocumentFilletEndpointOrder::SecondThenFirst
                    }
                    DocumentFilletEndpointOrder::SecondThenFirst => {
                        DocumentFilletEndpointOrder::FirstThenSecond
                    }
                },
                sweep: self.sweep,
            }
        }
    }
}

/// One persistent branch-explicit corner in a Fillet set.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedFilletCorner {
    pub id: ComputedFeatureCornerId,
    pub first: ComputedFilletParent,
    pub second: ComputedFilletParent,
    pub endpoint_order: DocumentFilletEndpointOrder,
    pub sweep: DocumentArcSweep,
}

impl ComputedFilletCorner {
    #[must_use]
    pub const fn without_id(self) -> NewComputedFilletCorner {
        NewComputedFilletCorner {
            first: self.first,
            second: self.second,
            endpoint_order: self.endpoint_order,
            sweep: self.sweep,
        }
    }
}

/// One persistent multi-corner Fillet operation with a shared radius.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedFilletSet {
    pub radius: f64,
    pub corners: Vec<ComputedFilletCorner>,
}

/// Closed persistent computed-feature definition surface.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ComputedFeatureDefinition {
    FilletSet(ComputedFilletSet),
}

/// One persistent feature-tree item.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedFeature {
    pub id: ComputedFeatureId,
    pub label: String,
    pub suppressed: bool,
    pub definition: ComputedFeatureDefinition,
}

/// Separately versioned persistent computed-feature intent.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedFeatureDocument {
    id: ComputedFeatureDocumentId,
    sketch_document: DocumentId,
    revision: ComputedFeatureRevision,
    next_feature_id: ComputedFeatureId,
    next_corner_id: ComputedFeatureCornerId,
    features: Vec<ComputedFeature>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ComputedFeaturePayload<'a> {
    document_id: ComputedFeatureDocumentId,
    sketch_document: DocumentId,
    revision: ComputedFeatureRevision,
    next_feature_id: ComputedFeatureId,
    next_corner_id: ComputedFeatureCornerId,
    features: &'a [ComputedFeature],
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ComputedFeatureWireV1 {
    version: u32,
    document_id: ComputedFeatureDocumentId,
    sketch_document: DocumentId,
    revision: ComputedFeatureRevision,
    next_feature_id: ComputedFeatureId,
    next_corner_id: ComputedFeatureCornerId,
    features: Vec<ComputedFeature>,
    digest: ComputedFeatureDocumentDigest,
}

/// Strict persistence or mutation failure for computed-feature intent.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ComputedFeatureDocumentError {
    #[error("invalid computed-feature ID encoding `{0}`")]
    InvalidId(String),
    #[error("invalid computed-feature digest encoding")]
    InvalidDigestEncoding,
    #[error("unsupported computed-feature document version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("computed-feature JSON exceeds {limit} bytes")]
    JsonResourceLimit { limit: usize },
    #[error("computed-feature resource limit exceeded for {resource}: {actual} > {limit}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("invalid computed-feature field `{field}`: {message}")]
    InvalidField {
        field: &'static str,
        message: &'static str,
    },
    #[error("duplicate computed feature {0}")]
    DuplicateFeature(ComputedFeatureId),
    #[error("duplicate computed feature corner {0}")]
    DuplicateCorner(ComputedFeatureCornerId),
    #[error("unknown computed feature {0}")]
    UnknownFeature(ComputedFeatureId),
    #[error("unknown computed feature corner {0}")]
    UnknownCorner(ComputedFeatureCornerId),
    #[error("computed-feature allocator is exhausted")]
    IdExhausted,
    #[error("computed-feature revision is exhausted")]
    RevisionExhausted,
    #[error("computed-feature payload digest does not match its canonical content")]
    DigestMismatch,
    #[error("invalid computed-feature JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[allow(
    clippy::missing_errors_doc,
    reason = "all persistent mutation failures are enumerated by ComputedFeatureDocumentError"
)]
impl ComputedFeatureDocument {
    /// Creates an empty feature sidecar bound to one sketch namespace.
    #[must_use]
    pub fn new(sketch_document: DocumentId) -> Self {
        let nonce = FEATURE_DOCUMENT_NONCE.fetch_add(1, Ordering::Relaxed);
        let mixed = sketch_document.0.as_u128()
            ^ (u128::from(nonce) << 64)
            ^ 0x6765_6f73_6f6c_7665_6665_6174_7572_6573_u128;
        let id = ComputedFeatureDocumentId::from_raw(if mixed == 0 { 1 } else { mixed });
        Self::with_id(sketch_document, id)
    }

    /// Creates an empty sidecar with a caller-supplied persistent identity.
    #[must_use]
    pub const fn with_id(sketch_document: DocumentId, id: ComputedFeatureDocumentId) -> Self {
        Self {
            id,
            sketch_document,
            revision: ComputedFeatureRevision(0),
            next_feature_id: ComputedFeatureId(1),
            next_corner_id: ComputedFeatureCornerId(1),
            features: Vec::new(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> ComputedFeatureDocumentId {
        self.id
    }

    #[must_use]
    pub const fn sketch_document(&self) -> DocumentId {
        self.sketch_document
    }

    #[must_use]
    pub const fn revision(&self) -> ComputedFeatureRevision {
        self.revision
    }

    #[must_use]
    pub fn features(&self) -> &[ComputedFeature] {
        &self.features
    }

    #[must_use]
    pub fn feature(&self, id: ComputedFeatureId) -> Option<&ComputedFeature> {
        self.features.iter().find(|feature| feature.id == id)
    }

    #[must_use]
    pub fn corner(
        &self,
        feature: ComputedFeatureId,
        corner: ComputedFeatureCornerId,
    ) -> Option<&ComputedFilletCorner> {
        let ComputedFeatureDefinition::FilletSet(fillet) = &self.feature(feature)?.definition;
        fillet.corners.iter().find(|value| value.id == corner)
    }

    #[must_use]
    pub const fn allocator_high_water(&self) -> ComputedFeatureAllocatorHighWater {
        ComputedFeatureAllocatorHighWater {
            next_feature_id: self.next_feature_id,
            next_corner_id: self.next_corner_id,
        }
    }

    #[must_use]
    pub const fn lifecycle_high_water(&self) -> ComputedFeatureLifecycleHighWater {
        ComputedFeatureLifecycleHighWater {
            revision: self.revision,
            allocator: self.allocator_high_water(),
        }
    }

    #[must_use]
    pub fn digest(&self) -> ComputedFeatureDocumentDigest {
        digest_bytes(&self.canonical_payload_bytes())
    }

    #[must_use]
    pub fn identity(&self) -> ComputedFeatureDocumentIdentity {
        ComputedFeatureDocumentIdentity {
            document: self.id,
            sketch_document: self.sketch_document,
            revision: self.revision,
            digest: self.digest(),
        }
    }

    /// Creates one persistent Fillet set and allocates stable IDs atomically.
    ///
    /// # Errors
    ///
    /// Rejects invalid labels, radius, parents, duplicate source pairs or resource exhaustion.
    pub fn create_fillet_set(
        &mut self,
        label: impl Into<String>,
        radius: f64,
        corners: Vec<NewComputedFilletCorner>,
    ) -> Result<ComputedFeatureId, ComputedFeatureDocumentError> {
        let label = label.into();
        validate_label(&label)?;
        validate_radius(radius)?;
        if corners.is_empty() {
            return Err(invalid_field(
                "corners",
                "a Fillet set must contain at least one corner",
            ));
        }
        let future_corner_count = self
            .corner_count()
            .checked_add(corners.len())
            .ok_or(ComputedFeatureDocumentError::IdExhausted)?;
        validate_resource("corners", future_corner_count, MAX_COMPUTED_FEATURE_CORNERS)?;
        validate_resource("features", self.features.len() + 1, MAX_COMPUTED_FEATURES)?;
        let corners = corners
            .into_iter()
            .map(NewComputedFilletCorner::canonicalized)
            .collect::<Vec<_>>();
        for corner in &corners {
            validate_new_corner(*corner)?;
        }
        validate_unique_new_corner_pairs(corners.iter().copied())?;
        let end_corner = self
            .next_corner_id
            .0
            .checked_add(
                u64::try_from(corners.len())
                    .map_err(|_| ComputedFeatureDocumentError::IdExhausted)?,
            )
            .ok_or(ComputedFeatureDocumentError::IdExhausted)?;
        let next_feature = self
            .next_feature_id
            .0
            .checked_add(1)
            .ok_or(ComputedFeatureDocumentError::IdExhausted)?;
        let next_revision = next_revision(self.revision)?;

        let id = self.next_feature_id;
        let mut next_corner = self.next_corner_id.0;
        let corners = corners
            .into_iter()
            .map(|corner| {
                let id = ComputedFeatureCornerId(next_corner);
                next_corner += 1;
                ComputedFilletCorner {
                    id,
                    first: corner.first,
                    second: corner.second,
                    endpoint_order: corner.endpoint_order,
                    sweep: corner.sweep,
                }
            })
            .collect();
        self.next_feature_id = ComputedFeatureId(next_feature);
        self.next_corner_id = ComputedFeatureCornerId(end_corner);
        self.revision = next_revision;
        self.features.push(ComputedFeature {
            id,
            label,
            suppressed: false,
            definition: ComputedFeatureDefinition::FilletSet(ComputedFilletSet { radius, corners }),
        });
        self.normalize();
        Ok(id)
    }

    /// Appends corners to an existing Fillet set while retaining its shared radius.
    ///
    /// # Errors
    ///
    /// Rejects an unknown feature, invalid corner, or allocator/resource exhaustion.
    pub fn add_fillet_corners(
        &mut self,
        feature: ComputedFeatureId,
        corners: Vec<NewComputedFilletCorner>,
    ) -> Result<Vec<ComputedFeatureCornerId>, ComputedFeatureDocumentError> {
        if corners.is_empty() {
            return Ok(Vec::new());
        }
        if self.feature(feature).is_none() {
            return Err(ComputedFeatureDocumentError::UnknownFeature(feature));
        }
        let corners = corners
            .into_iter()
            .map(NewComputedFilletCorner::canonicalized)
            .collect::<Vec<_>>();
        for corner in &corners {
            validate_new_corner(*corner)?;
        }
        let existing_pairs = self
            .feature(feature)
            .into_iter()
            .flat_map(|value| {
                let ComputedFeatureDefinition::FilletSet(fillet) = &value.definition;
                fillet
                    .corners
                    .iter()
                    .map(|corner| corner_source_pair(corner.without_id()))
            })
            .collect::<std::collections::BTreeSet<_>>();
        validate_unique_new_corner_pairs(corners.iter().copied())?;
        if corners
            .iter()
            .any(|corner| existing_pairs.contains(&corner_source_pair(*corner)))
        {
            return Err(invalid_field(
                "corner parents",
                "a Fillet set cannot contain duplicate canonical source pairs",
            ));
        }
        validate_resource(
            "corners",
            self.corner_count()
                .checked_add(corners.len())
                .ok_or(ComputedFeatureDocumentError::IdExhausted)?,
            MAX_COMPUTED_FEATURE_CORNERS,
        )?;
        let count =
            u64::try_from(corners.len()).map_err(|_| ComputedFeatureDocumentError::IdExhausted)?;
        let end = self
            .next_corner_id
            .0
            .checked_add(count)
            .ok_or(ComputedFeatureDocumentError::IdExhausted)?;
        let next_revision = next_revision(self.revision)?;
        let mut ids = Vec::with_capacity(corners.len());
        let mut created = Vec::with_capacity(corners.len());
        for (offset, corner) in corners.into_iter().enumerate() {
            let offset =
                u64::try_from(offset).map_err(|_| ComputedFeatureDocumentError::IdExhausted)?;
            let id = ComputedFeatureCornerId(
                self.next_corner_id
                    .0
                    .checked_add(offset)
                    .ok_or(ComputedFeatureDocumentError::IdExhausted)?,
            );
            ids.push(id);
            created.push(ComputedFilletCorner {
                id,
                first: corner.first,
                second: corner.second,
                endpoint_order: corner.endpoint_order,
                sweep: corner.sweep,
            });
        }
        let value = self
            .features
            .iter_mut()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let ComputedFeatureDefinition::FilletSet(fillet) = &mut value.definition;
        fillet.corners.extend(created);
        fillet.corners.sort_by_key(|corner| corner.id);
        self.next_corner_id = ComputedFeatureCornerId(end);
        self.revision = next_revision;
        Ok(ids)
    }

    /// Changes the shared radius of one Fillet set.
    pub fn set_fillet_radius(
        &mut self,
        feature: ComputedFeatureId,
        radius: f64,
    ) -> Result<(), ComputedFeatureDocumentError> {
        validate_radius(radius)?;
        let current = self
            .features
            .iter()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let ComputedFeatureDefinition::FilletSet(fillet) = &current.definition;
        if fillet.radius.to_bits() == radius.to_bits() {
            return Ok(());
        }
        let next_revision = next_revision(self.revision)?;
        let value = self
            .features
            .iter_mut()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let ComputedFeatureDefinition::FilletSet(fillet) = &mut value.definition;
        fillet.radius = radius;
        self.revision = next_revision;
        Ok(())
    }

    /// Replaces one corner's complete explicit branch state without changing its ID.
    pub fn set_fillet_corner(
        &mut self,
        feature: ComputedFeatureId,
        corner: ComputedFeatureCornerId,
        replacement: NewComputedFilletCorner,
    ) -> Result<(), ComputedFeatureDocumentError> {
        let replacement = replacement.canonicalized();
        validate_new_corner(replacement)?;
        let current_feature = self
            .features
            .iter()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let ComputedFeatureDefinition::FilletSet(current_fillet) = &current_feature.definition;
        if current_fillet.corners.iter().any(|value| {
            value.id != corner
                && corner_source_pair(value.without_id()) == corner_source_pair(replacement)
        }) {
            return Err(invalid_field(
                "corner parents",
                "a Fillet set cannot contain duplicate canonical source pairs",
            ));
        }
        let current = current_fillet
            .corners
            .iter()
            .find(|value| value.id == corner)
            .ok_or(ComputedFeatureDocumentError::UnknownCorner(corner))?;
        if current.without_id() == replacement {
            return Ok(());
        }
        let next_revision = next_revision(self.revision)?;
        let value = self
            .features
            .iter_mut()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let ComputedFeatureDefinition::FilletSet(fillet) = &mut value.definition;
        let value = fillet
            .corners
            .iter_mut()
            .find(|value| value.id == corner)
            .ok_or(ComputedFeatureDocumentError::UnknownCorner(corner))?;
        *value = ComputedFilletCorner {
            id: corner,
            first: replacement.first,
            second: replacement.second,
            endpoint_order: replacement.endpoint_order,
            sweep: replacement.sweep,
        };
        self.revision = next_revision;
        Ok(())
    }

    /// Changes one feature's suppression state.
    pub fn set_suppressed(
        &mut self,
        feature: ComputedFeatureId,
        suppressed: bool,
    ) -> Result<(), ComputedFeatureDocumentError> {
        let current = self
            .features
            .iter()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        if current.suppressed == suppressed {
            return Ok(());
        }
        let next_revision = next_revision(self.revision)?;
        let value = self
            .features
            .iter_mut()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        value.suppressed = suppressed;
        self.revision = next_revision;
        Ok(())
    }

    /// Changes one feature-tree label.
    pub fn set_label(
        &mut self,
        feature: ComputedFeatureId,
        label: impl Into<String>,
    ) -> Result<(), ComputedFeatureDocumentError> {
        let label = label.into();
        validate_label(&label)?;
        let current = self
            .features
            .iter()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        if current.label == label {
            return Ok(());
        }
        let next_revision = next_revision(self.revision)?;
        let value = self
            .features
            .iter_mut()
            .find(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        value.label = label;
        self.revision = next_revision;
        Ok(())
    }

    /// Deletes one corner. Deleting the final corner removes its Fillet set.
    pub fn remove_corner(
        &mut self,
        feature: ComputedFeatureId,
        corner: ComputedFeatureCornerId,
    ) -> Result<bool, ComputedFeatureDocumentError> {
        let index = self
            .features
            .iter()
            .position(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let ComputedFeatureDefinition::FilletSet(fillet) = &self.features[index].definition;
        if !fillet.corners.iter().any(|value| value.id == corner) {
            return Err(ComputedFeatureDocumentError::UnknownCorner(corner));
        }
        let removed_feature = fillet.corners.len() == 1;
        let next_revision = next_revision(self.revision)?;
        if removed_feature {
            self.features.remove(index);
        } else {
            let ComputedFeatureDefinition::FilletSet(fillet) = &mut self.features[index].definition;
            fillet.corners.retain(|value| value.id != corner);
        }
        self.revision = next_revision;
        Ok(removed_feature)
    }

    /// Deletes one complete computed feature.
    pub fn remove_feature(
        &mut self,
        feature: ComputedFeatureId,
    ) -> Result<(), ComputedFeatureDocumentError> {
        let index = self
            .features
            .iter()
            .position(|value| value.id == feature)
            .ok_or(ComputedFeatureDocumentError::UnknownFeature(feature))?;
        let next_revision = next_revision(self.revision)?;
        self.features.remove(index);
        self.revision = next_revision;
        Ok(())
    }

    /// Raises allocator cursors after restoring an older history checkpoint.
    ///
    /// This is the only supported way to merge host-retained allocator high-water
    /// into an older document. Cursors never move backwards.
    pub fn retain_allocator_high_water(
        &mut self,
        retained: ComputedFeatureAllocatorHighWater,
    ) -> Result<(), ComputedFeatureDocumentError> {
        if retained.next_feature_id.0 == 0 || retained.next_corner_id.0 == 0 {
            return Err(invalid_field(
                "allocator",
                "allocator cursors must be nonzero",
            ));
        }
        let next_feature = self.next_feature_id.max(retained.next_feature_id);
        let next_corner = self.next_corner_id.max(retained.next_corner_id);
        if next_feature == self.next_feature_id && next_corner == self.next_corner_id {
            return Ok(());
        }
        let next_revision = next_revision(self.revision)?;
        self.next_feature_id = next_feature;
        self.next_corner_id = next_corner;
        self.revision = next_revision;
        Ok(())
    }

    /// Rebases an older restored checkpoint above every observed revision and
    /// allocator cursor. The resulting exact identity can never alias a prior
    /// forward-history state even when its feature content is otherwise equal.
    pub fn rebase_after_restore(
        &mut self,
        retained: ComputedFeatureLifecycleHighWater,
    ) -> Result<(), ComputedFeatureDocumentError> {
        if retained.allocator.next_feature_id.0 == 0 || retained.allocator.next_corner_id.0 == 0 {
            return Err(invalid_field(
                "allocator",
                "allocator cursors must be nonzero",
            ));
        }
        let next_feature_id = self.next_feature_id.max(retained.allocator.next_feature_id);
        let next_corner_id = self.next_corner_id.max(retained.allocator.next_corner_id);
        let revision = ComputedFeatureRevision(
            self.revision
                .0
                .max(retained.revision.0)
                .checked_add(1)
                .ok_or(ComputedFeatureDocumentError::RevisionExhausted)?,
        );
        self.next_feature_id = next_feature_id;
        self.next_corner_id = next_corner_id;
        self.revision = revision;
        Ok(())
    }

    /// Serializes canonical strict V1 computed-feature JSON.
    pub fn to_json(&self) -> Result<String, ComputedFeatureDocumentError> {
        self.validate()?;
        let wire = ComputedFeatureWireV1 {
            version: COMPUTED_FEATURE_DOCUMENT_VERSION,
            document_id: self.id,
            sketch_document: self.sketch_document,
            revision: self.revision,
            next_feature_id: self.next_feature_id,
            next_corner_id: self.next_corner_id,
            features: self.features.clone(),
            digest: self.digest(),
        };
        Ok(serde_json::to_string(&wire)?)
    }

    /// Imports strict V1 computed-feature JSON and verifies its canonical digest.
    pub fn from_json(json: &str) -> Result<Self, ComputedFeatureDocumentError> {
        if json.len() > MAX_COMPUTED_FEATURE_JSON_BYTES {
            return Err(ComputedFeatureDocumentError::JsonResourceLimit {
                limit: MAX_COMPUTED_FEATURE_JSON_BYTES,
            });
        }
        let wire: ComputedFeatureWireV1 = serde_json::from_str(json)?;
        if wire.version != COMPUTED_FEATURE_DOCUMENT_VERSION {
            return Err(ComputedFeatureDocumentError::UnsupportedVersion {
                actual: wire.version,
                expected: COMPUTED_FEATURE_DOCUMENT_VERSION,
            });
        }
        let mut document = Self {
            id: wire.document_id,
            sketch_document: wire.sketch_document,
            revision: wire.revision,
            next_feature_id: wire.next_feature_id,
            next_corner_id: wire.next_corner_id,
            features: wire.features,
        };
        document.normalize();
        document.validate()?;
        if document.digest() != wire.digest {
            return Err(ComputedFeatureDocumentError::DigestMismatch);
        }
        Ok(document)
    }

    /// Validates persistent structure without requiring the referenced sketch.
    pub fn validate(&self) -> Result<(), ComputedFeatureDocumentError> {
        if self.id.0 == 0 {
            return Err(invalid_field("document_id", "must be nonzero"));
        }
        if self.next_feature_id.0 == 0 || self.next_corner_id.0 == 0 {
            return Err(invalid_field(
                "allocator",
                "allocator cursors must be nonzero",
            ));
        }
        validate_resource("features", self.features.len(), MAX_COMPUTED_FEATURES)?;
        validate_resource("corners", self.corner_count(), MAX_COMPUTED_FEATURE_CORNERS)?;
        let mut feature_ids = std::collections::BTreeSet::new();
        let mut corner_ids = std::collections::BTreeSet::new();
        for feature in &self.features {
            if feature.id.0 == 0 || feature.id >= self.next_feature_id {
                return Err(invalid_field(
                    "feature id",
                    "must be nonzero and below the allocator cursor",
                ));
            }
            if !feature_ids.insert(feature.id) {
                return Err(ComputedFeatureDocumentError::DuplicateFeature(feature.id));
            }
            validate_label(&feature.label)?;
            let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
            validate_radius(fillet.radius)?;
            if fillet.corners.is_empty() {
                return Err(invalid_field(
                    "corners",
                    "a Fillet set must contain at least one corner",
                ));
            }
            let mut pairs = std::collections::BTreeSet::new();
            for corner in &fillet.corners {
                if corner.id.0 == 0 || corner.id >= self.next_corner_id {
                    return Err(invalid_field(
                        "corner id",
                        "must be nonzero and below the allocator cursor",
                    ));
                }
                if !corner_ids.insert(corner.id) {
                    return Err(ComputedFeatureDocumentError::DuplicateCorner(corner.id));
                }
                validate_new_corner(corner.without_id())?;
                if !pairs.insert(corner_source_pair(corner.without_id())) {
                    return Err(invalid_field(
                        "corner parents",
                        "a Fillet set cannot contain duplicate canonical source pairs",
                    ));
                }
            }
        }
        Ok(())
    }

    fn corner_count(&self) -> usize {
        self.features
            .iter()
            .map(|feature| {
                let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
                fillet.corners.len()
            })
            .sum()
    }

    fn normalize(&mut self) {
        self.features.sort_by_key(|feature| feature.id);
        for feature in &mut self.features {
            let ComputedFeatureDefinition::FilletSet(fillet) = &mut feature.definition;
            fillet.corners.sort_by_key(|corner| corner.id);
        }
    }

    fn canonical_payload_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&ComputedFeaturePayload {
            document_id: self.id,
            sketch_document: self.sketch_document,
            revision: self.revision,
            next_feature_id: self.next_feature_id,
            next_corner_id: self.next_corner_id,
            features: &self.features,
        })
        .expect("computed-feature payload contains only infallibly serializable values")
    }

    #[cfg(test)]
    pub(crate) fn set_revision_for_test(&mut self, revision: ComputedFeatureRevision) {
        self.revision = revision;
    }
}

fn validate_new_corner(
    corner: NewComputedFilletCorner,
) -> Result<(), ComputedFeatureDocumentError> {
    validate_parent(corner.first)?;
    validate_parent(corner.second)?;
    if corner.first.source == corner.second.source {
        return Err(invalid_field(
            "corner parents",
            "a Fillet corner requires two distinct native spans",
        ));
    }
    if !source_precedes_or_equals(corner.first.source, corner.second.source) {
        return Err(invalid_field(
            "corner parents",
            "Fillet parents are not in canonical native-span order",
        ));
    }
    Ok(())
}

fn validate_unique_new_corner_pairs(
    corners: impl IntoIterator<Item = NewComputedFilletCorner>,
) -> Result<(), ComputedFeatureDocumentError> {
    let mut pairs = std::collections::BTreeSet::new();
    if corners
        .into_iter()
        .any(|corner| !pairs.insert(corner_source_pair(corner)))
    {
        return Err(invalid_field(
            "corner parents",
            "a Fillet set cannot contain duplicate canonical source pairs",
        ));
    }
    Ok(())
}

const fn corner_source_pair(
    corner: NewComputedFilletCorner,
) -> (NativeCurveSpanSource, NativeCurveSpanSource) {
    (corner.first.source, corner.second.source)
}

const fn source_key(source: NativeCurveSpanSource) -> (u128, u32) {
    (source.span.curve.0.as_u128(), source.span.segment)
}

const fn source_precedes_or_equals(
    first: NativeCurveSpanSource,
    second: NativeCurveSpanSource,
) -> bool {
    let first = source_key(first);
    let second = source_key(second);
    first.0 < second.0 || (first.0 == second.0 && first.1 <= second.1)
}

fn validate_parent(parent: ComputedFilletParent) -> Result<(), ComputedFeatureDocumentError> {
    if !parent.picked_parameter.is_finite() {
        return Err(invalid_field("picked_parameter", "must be finite"));
    }
    match parent.neighborhood {
        ContactNeighborhood::Interior => {}
        ContactNeighborhood::Local { lower, upper }
            if lower.is_finite() && upper.is_finite() && lower < upper => {}
        ContactNeighborhood::Local { .. } => {
            return Err(invalid_field(
                "neighborhood",
                "local bounds must be finite and increasing",
            ));
        }
        ContactNeighborhood::Start | ContactNeighborhood::End => {
            return Err(invalid_field(
                "neighborhood",
                "Fillet parents cannot select support endpoints",
            ));
        }
    }
    if let Some(anchor) = parent.periodic_anchor
        && (!anchor.parameter.is_finite())
    {
        return Err(invalid_field("periodic_anchor", "must be finite"));
    }
    Ok(())
}

fn validate_label(label: &str) -> Result<(), ComputedFeatureDocumentError> {
    validate_resource("label bytes", label.len(), MAX_COMPUTED_FEATURE_LABEL_BYTES)?;
    if label.trim().is_empty() {
        return Err(invalid_field("label", "must not be empty"));
    }
    Ok(())
}

fn validate_radius(radius: f64) -> Result<(), ComputedFeatureDocumentError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(invalid_field("radius", "must be finite and positive"));
    }
    Ok(())
}

fn validate_resource(
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), ComputedFeatureDocumentError> {
    if actual > limit {
        return Err(ComputedFeatureDocumentError::ResourceLimit {
            resource,
            actual,
            limit,
        });
    }
    Ok(())
}

const fn invalid_field(field: &'static str, message: &'static str) -> ComputedFeatureDocumentError {
    ComputedFeatureDocumentError::InvalidField { field, message }
}

fn next_revision(
    revision: ComputedFeatureRevision,
) -> Result<ComputedFeatureRevision, ComputedFeatureDocumentError> {
    revision
        .0
        .checked_add(1)
        .map(ComputedFeatureRevision)
        .ok_or(ComputedFeatureDocumentError::RevisionExhausted)
}

fn digest_bytes(bytes: &[u8]) -> ComputedFeatureDocumentDigest {
    // Four independent, stable FNV-1a lanes provide a deterministic content
    // fingerprint without adding a cryptographic dependency or platform hasher.
    const OFFSETS: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0x6a09_e667_f3bc_c909,
    ];
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut digest = [0_u8; 32];
    for (lane_index, offset) in OFFSETS.into_iter().enumerate() {
        let mut lane = offset ^ u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        for byte in bytes {
            lane ^= u64::from(*byte).wrapping_add((lane_index as u64) << 8);
            lane = lane.wrapping_mul(PRIME);
            lane ^= lane.rotate_right(17);
        }
        digest[lane_index * 8..lane_index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    ComputedFeatureDocumentDigest(digest)
}
