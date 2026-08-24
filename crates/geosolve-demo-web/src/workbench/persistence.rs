// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, Write};

use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

use geosolve_constraint_editor::{
    AnnotationLayoutEntry, AnnotationLayoutKey, AnnotationLayoutState, AnnotationPlacement,
    EditorScene, ProjectionalEditorSession, RestoreCheckpoint, RetainedEditorCoordinator,
    SceneAnnotationGeometry, SceneAnnotationKind, SceneConstraintGlyph, SelectionItem, Viewport,
    decode_flat_intent_bootstrap, normalize_flat_sketch_intent_with_accepted_materialization,
};
use geosolve_core::SolverConfig;
use geosolve_sketch::{
    DocumentConstraintId, DocumentDimensionId, DocumentId, DocumentSolveRequest, DocumentSourceId,
    PersistentId, RetainedSketchDocumentSession, SketchDocument, SketchLifecycleRevisionHighWater,
    SketchPersistentIdentityHighWater,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater, ComputedFeatureDocument,
    ComputedFeatureLifecycleHighWater,
};
use geosolve_sketch_intent::{
    ContentDigest, IntentSession, IntentSessionId, intent_content_digest,
    intent_legacy_content_digest,
};

const PROJECTIONAL_WORKSPACE_VERSION: u32 = 8;
const FLAT_WORKSPACE_VERSION: u32 = 6;
const ABANDONED_WORKSPACE_VERSION: u32 = 7;
const MAX_WORKSPACE_JSON_BYTES: usize = crate::reproduction::MAX_REPRODUCTION_WORKSPACE_BYTES;
const MAX_ANNOTATION_LAYOUT_JSON_BYTES: usize = 4 * 1024 * 1024;

#[cfg(target_arch = "wasm32")]
pub(crate) const STORAGE_KEY: &str = "geosolve.workbench.session.v6";
#[cfg(target_arch = "wasm32")]
pub(crate) const PREVIOUS_STORAGE_KEY: &str = "geosolve.workbench.session.v5";
#[cfg(target_arch = "wasm32")]
pub(crate) const OLDER_STORAGE_KEY: &str = "geosolve.workbench.session.v4";
#[cfg(target_arch = "wasm32")]
pub(crate) const OLDER_V3_STORAGE_KEY: &str = "geosolve.workbench.session.v3";
#[cfg(target_arch = "wasm32")]
pub(crate) const OLDER_V2_STORAGE_KEY: &str = "geosolve.workbench.session.v2";
#[cfg(target_arch = "wasm32")]
pub(crate) const LEGACY_STORAGE_KEY: &str = "geosolve.workbench.session.v1";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceSnapshot {
    version: u32,
    design: WorkspaceDocumentPayload,
    accepted: Option<WorkspaceDocumentPayload>,
    accepted_belongs_to_current_design: bool,
    sketch_identity_high_water: SketchPersistentIdentityHighWater,
    features_json: String,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    computed_evaluation_high_water: ComputedEvaluationAllocatorHighWater,
    #[serde(
        default,
        deserialize_with = "deserialize_annotation_layout_json",
        skip_serializing_if = "Option::is_none"
    )]
    annotation_layout_json: Option<String>,
    pub(crate) revisions: WorkspaceRevisions,
    #[serde(skip)]
    intent_session_json: Option<String>,
    #[serde(skip)]
    origin: WorkspaceSnapshotOrigin,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum WorkspaceSnapshotOrigin {
    #[default]
    FlatV6,
    ProjectionalV8,
    LegacyBootstrap {
        source_version: u32,
    },
}

/// Strictly decoded historical flat workspace awaiting one typed declaration
/// per native object. The decoded documents remain data for normalization, not
/// a second aggregate semantic authority beside the intent graph.
pub(crate) struct WorkspaceLegacyBootstrap<'a> {
    source_version: u32,
    snapshot: &'a WorkspaceSnapshot,
}

#[allow(
    dead_code,
    reason = "consumed by the coordinator-owned per-object normalizer in the next M83 slice"
)]
impl WorkspaceLegacyBootstrap<'_> {
    pub(crate) const fn source_version(&self) -> u32 {
        self.source_version
    }

    pub(crate) fn design_document(&self) -> Result<SketchDocument, String> {
        self.snapshot.design_document()
    }

    pub(crate) fn accepted_document(&self) -> Result<Option<SketchDocument>, String> {
        self.snapshot.accepted_document()
    }

    pub(crate) fn feature_document(&self) -> Result<ComputedFeatureDocument, String> {
        self.snapshot.feature_document()
    }

    pub(crate) const fn revisions(&self) -> SketchLifecycleRevisionHighWater {
        self.snapshot.revisions()
    }

    pub(crate) const fn accepted_belongs_to_current_design(&self) -> bool {
        self.snapshot.accepted_belongs_to_current_design
    }

    pub(crate) const fn sketch_identity_high_water(&self) -> &SketchPersistentIdentityHighWater {
        &self.snapshot.sketch_identity_high_water
    }

    pub(crate) const fn feature_lifecycle_high_water(&self) -> ComputedFeatureLifecycleHighWater {
        self.snapshot.feature_lifecycle_high_water
    }

    pub(crate) const fn computed_evaluation_high_water(
        &self,
    ) -> ComputedEvaluationAllocatorHighWater {
        self.snapshot.computed_evaluation_high_water
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceMaterializationV8 {
    design: WorkspaceDocumentPayload,
    accepted: Option<WorkspaceDocumentPayload>,
    accepted_belongs_to_current_design: bool,
    sketch_identity_high_water: SketchPersistentIdentityHighWater,
    features_json: String,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    computed_evaluation_high_water: ComputedEvaluationAllocatorHighWater,
    revisions: WorkspaceRevisions,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceSnapshotV8 {
    version: u32,
    intent_session_json: String,
    materialization: WorkspaceMaterializationV8,
    #[serde(
        default,
        deserialize_with = "deserialize_authenticated_annotation_layout_json",
        skip_serializing_if = "Option::is_none"
    )]
    annotation_layout_json: Option<String>,
    digest: ContentDigest,
}

#[derive(Debug, Serialize)]
struct WorkspaceMaterializationV8Ref<'a> {
    design: &'a WorkspaceDocumentPayload,
    accepted: Option<&'a WorkspaceDocumentPayload>,
    accepted_belongs_to_current_design: bool,
    sketch_identity_high_water: &'a SketchPersistentIdentityHighWater,
    features_json: &'a str,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    computed_evaluation_high_water: ComputedEvaluationAllocatorHighWater,
    revisions: WorkspaceRevisions,
}

#[derive(Debug, Serialize)]
struct WorkspaceSnapshotV8Ref<'a> {
    version: u32,
    intent_session_json: &'a str,
    materialization: WorkspaceMaterializationV8Ref<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotation_layout_json: Option<&'a str>,
    digest: ContentDigest,
}

#[derive(Deserialize)]
struct JsonVersionProbe {
    version: u32,
}

impl<'a> WorkspaceMaterializationV8Ref<'a> {
    fn from_snapshot(snapshot: &'a WorkspaceSnapshot) -> Self {
        Self {
            design: &snapshot.design,
            accepted: snapshot.accepted.as_ref(),
            accepted_belongs_to_current_design: snapshot.accepted_belongs_to_current_design,
            sketch_identity_high_water: &snapshot.sketch_identity_high_water,
            features_json: &snapshot.features_json,
            feature_lifecycle_high_water: snapshot.feature_lifecycle_high_water,
            computed_evaluation_high_water: snapshot.computed_evaluation_high_water,
            revisions: snapshot.revisions,
        }
    }
}

fn workspace_v8_digest_payload_from_parts(
    version: u32,
    intent_session_json: &str,
    materialization: &impl Serialize,
    annotation_layout_json: &impl Serialize,
) -> Vec<u8> {
    serde_json::to_vec(&(
        version,
        intent_session_json,
        materialization,
        annotation_layout_json,
    ))
    .expect("workspace-v8 digest payload is infallibly serializable")
}

fn workspace_v8_digest_payload(wire: &WorkspaceSnapshotV8) -> Vec<u8> {
    workspace_v8_digest_payload_from_parts(
        wire.version,
        &wire.intent_session_json,
        &wire.materialization,
        &wire.annotation_layout_json,
    )
}

#[cfg(test)]
fn workspace_v8_digest(wire: &WorkspaceSnapshotV8) -> ContentDigest {
    intent_content_digest(&workspace_v8_digest_payload(wire))
}

#[cfg(test)]
fn legacy_workspace_v8_digest(wire: &WorkspaceSnapshotV8) -> ContentDigest {
    intent_legacy_content_digest(&workspace_v8_digest_payload(wire))
}

#[derive(Debug)]
struct ExactJsonWriter<'a> {
    expected: &'a [u8],
    position: usize,
    differs: bool,
}

impl<'a> ExactJsonWriter<'a> {
    const fn new(expected: &'a str) -> Self {
        Self {
            expected: expected.as_bytes(),
            position: 0,
            differs: false,
        }
    }

    const fn matches(&self) -> bool {
        !self.differs && self.position == self.expected.len()
    }
}

impl Write for ExactJsonWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining = self.expected.len().saturating_sub(self.position);
        let comparable = remaining.min(bytes.len());
        let expected_start = self.position.min(self.expected.len());
        if self.expected[expected_start..expected_start + comparable] != bytes[..comparable]
            || comparable != bytes.len()
        {
            self.differs = true;
        }
        self.position = self.position.saturating_add(bytes.len());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn is_exact_canonical_json(value: &impl Serialize, input: &str) -> Result<bool, String> {
    let mut writer = ExactJsonWriter::new(input);
    serde_json::to_writer(&mut writer, value).map_err(|error| error.to_string())?;
    Ok(writer.matches())
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceDocumentEncoding {
    CanonicalV4,
    DraftV5,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceDocumentPayload {
    encoding: WorkspaceDocumentEncoding,
    json: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorkspaceSnapshotV1 {
    version: u32,
    design_json: String,
    accepted_json: Option<String>,
    revisions: WorkspaceRevisions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorkspaceSnapshotV2 {
    version: u32,
    design: WorkspaceDocumentPayload,
    accepted: Option<WorkspaceDocumentPayload>,
    revisions: WorkspaceRevisions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorkspaceSnapshotV3 {
    version: u32,
    design: WorkspaceDocumentPayload,
    accepted: Option<WorkspaceDocumentPayload>,
    accepted_belongs_to_current_design: bool,
    revisions: WorkspaceRevisions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorkspaceSnapshotV4 {
    version: u32,
    design: WorkspaceDocumentPayload,
    accepted: Option<WorkspaceDocumentPayload>,
    accepted_belongs_to_current_design: bool,
    features_json: String,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    computed_evaluation_high_water: ComputedEvaluationAllocatorHighWater,
    revisions: WorkspaceRevisions,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceAnnotationLayoutCache {
    version: u32,
    entries: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceAnnotationLayoutEntry {
    document: String,
    source: String,
    item_kind: String,
    item_id: String,
    annotation_kind: String,
    marker_index: Option<usize>,
    placement: WorkspaceAnnotationPlacement,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "form")]
enum WorkspaceAnnotationPlacement {
    Linear {
        perpendicular_pixels: f64,
    },
    Radial {
        direction_radians: f64,
        clearance_pixels: f64,
    },
    Angular {
        radius_pixels: f64,
    },
    Free {
        offset_pixels: [f64; 2],
    },
}

fn deserialize_annotation_layout_json<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // The cache is disposable presentation data. A syntactically valid
    // workspace must therefore survive an incompatible outer cache value just
    // as it survives an incompatible cache version or row.
    deserializer.deserialize_any(DisposableAnnotationLayoutVisitor {
        retain_oversized_string_for_authentication: false,
    })
}

fn deserialize_authenticated_annotation_layout_json<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Workspace v8 authenticates this disposable field as part of its outer
    // digest. Retain an oversized string only through authentication, then
    // evict it before any nested cache decode.
    deserializer.deserialize_any(DisposableAnnotationLayoutVisitor {
        retain_oversized_string_for_authentication: true,
    })
}

struct DisposableAnnotationLayoutVisitor {
    retain_oversized_string_for_authentication: bool,
}

impl<'de> Visitor<'de> for DisposableAnnotationLayoutVisitor {
    type Value = Option<String>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an optional annotation-layout JSON string")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok((self.retain_oversized_string_for_authentication
            || value.len() <= MAX_ANNOTATION_LAYOUT_JSON_BYTES)
            .then(|| value.to_owned()))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok((self.retain_oversized_string_for_authentication
            || value.len() <= MAX_ANNOTATION_LAYOUT_JSON_BYTES)
            .then(|| value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok((self.retain_oversized_string_for_authentication
            || value.len() <= MAX_ANNOTATION_LAYOUT_JSON_BYTES)
            .then_some(value))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

fn encode_annotation_layout(layout: &AnnotationLayoutState) -> Option<String> {
    let entries = layout
        .entries()
        .into_iter()
        .filter_map(workspace_annotation_entry)
        .filter_map(|entry| serde_json::to_value(entry).ok())
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return None;
    }
    serde_json::to_string(&WorkspaceAnnotationLayoutCache {
        version: AnnotationLayoutState::VERSION,
        entries,
    })
    .ok()
    .filter(|json| json.len() <= MAX_ANNOTATION_LAYOUT_JSON_BYTES)
}

fn decode_annotation_layout(input: &str) -> Option<AnnotationLayoutState> {
    if input.len() > MAX_ANNOTATION_LAYOUT_JSON_BYTES {
        return None;
    }
    let cache: WorkspaceAnnotationLayoutCache = serde_json::from_str(input).ok()?;
    if cache.version != AnnotationLayoutState::VERSION {
        return None;
    }
    Some(AnnotationLayoutState::from_entries(
        cache
            .entries
            .into_iter()
            .filter_map(|entry| serde_json::from_value(entry).ok())
            .filter_map(|entry| annotation_layout_entry(&entry)),
    ))
}

fn workspace_annotation_entry(
    entry: AnnotationLayoutEntry,
) -> Option<WorkspaceAnnotationLayoutEntry> {
    let (item_kind, item_id) = match entry.key.item {
        SelectionItem::Constraint(id) => ("constraint", id.to_string()),
        SelectionItem::Dimension(id) => ("dimension", id.to_string()),
        SelectionItem::Point(_)
        | SelectionItem::Curve(_)
        | SelectionItem::Datum(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => return None,
    };
    Some(WorkspaceAnnotationLayoutEntry {
        document: entry.key.document.to_string(),
        source: entry.key.source.to_string(),
        item_kind: item_kind.into(),
        item_id,
        annotation_kind: annotation_kind_key(entry.key.kind).into(),
        marker_index: entry.key.marker_index,
        placement: match entry.placement {
            AnnotationPlacement::Linear {
                perpendicular_pixels,
            } => WorkspaceAnnotationPlacement::Linear {
                perpendicular_pixels,
            },
            AnnotationPlacement::Radial {
                direction_radians,
                clearance_pixels,
            } => WorkspaceAnnotationPlacement::Radial {
                direction_radians,
                clearance_pixels,
            },
            AnnotationPlacement::Angular { radius_pixels } => {
                WorkspaceAnnotationPlacement::Angular { radius_pixels }
            }
            AnnotationPlacement::Free { offset_pixels } => {
                WorkspaceAnnotationPlacement::Free { offset_pixels }
            }
        },
    })
}

fn annotation_layout_entry(
    entry: &WorkspaceAnnotationLayoutEntry,
) -> Option<AnnotationLayoutEntry> {
    let document = DocumentId(entry.document.parse::<PersistentId>().ok()?);
    let source = DocumentSourceId(entry.source.parse::<PersistentId>().ok()?);
    let persistent = entry.item_id.parse::<PersistentId>().ok()?;
    let item = match entry.item_kind.as_str() {
        "constraint" => SelectionItem::Constraint(DocumentConstraintId(persistent)),
        "dimension" => SelectionItem::Dimension(DocumentDimensionId(persistent)),
        _ => return None,
    };
    let placement = match &entry.placement {
        WorkspaceAnnotationPlacement::Linear {
            perpendicular_pixels,
        } => AnnotationPlacement::Linear {
            perpendicular_pixels: *perpendicular_pixels,
        },
        WorkspaceAnnotationPlacement::Radial {
            direction_radians,
            clearance_pixels,
        } => AnnotationPlacement::Radial {
            direction_radians: *direction_radians,
            clearance_pixels: *clearance_pixels,
        },
        WorkspaceAnnotationPlacement::Angular { radius_pixels } => AnnotationPlacement::Angular {
            radius_pixels: *radius_pixels,
        },
        WorkspaceAnnotationPlacement::Free { offset_pixels } => AnnotationPlacement::Free {
            offset_pixels: *offset_pixels,
        },
    };
    placement.is_valid().then_some(AnnotationLayoutEntry {
        key: AnnotationLayoutKey {
            document,
            source,
            item,
            kind: parse_annotation_kind(&entry.annotation_kind)?,
            marker_index: entry.marker_index,
        },
        placement,
    })
}

const fn annotation_kind_key(kind: SceneAnnotationKind) -> &'static str {
    match kind {
        SceneAnnotationKind::Constraint(glyph) => constraint_glyph_key(glyph),
        SceneAnnotationKind::PointDistance => "dimension:point-distance",
        SceneAnnotationKind::CurveLength => "dimension:curve-length",
        SceneAnnotationKind::Radius => "dimension:radius",
        SceneAnnotationKind::Diameter => "dimension:diameter",
        SceneAnnotationKind::OrientedAngle => "dimension:angle",
        SceneAnnotationKind::SupportingLineOffset => "dimension:supporting-offset",
        SceneAnnotationKind::ExactTranslatedSegmentOffset => "dimension:translated-offset",
        SceneAnnotationKind::ProfileOffset => "dimension:profile-offset",
    }
}

fn parse_annotation_kind(value: &str) -> Option<SceneAnnotationKind> {
    Some(match value {
        "dimension:point-distance" => SceneAnnotationKind::PointDistance,
        "dimension:curve-length" => SceneAnnotationKind::CurveLength,
        "dimension:radius" => SceneAnnotationKind::Radius,
        "dimension:diameter" => SceneAnnotationKind::Diameter,
        "dimension:angle" => SceneAnnotationKind::OrientedAngle,
        "dimension:supporting-offset" => SceneAnnotationKind::SupportingLineOffset,
        "dimension:translated-offset" => SceneAnnotationKind::ExactTranslatedSegmentOffset,
        "dimension:profile-offset" => SceneAnnotationKind::ProfileOffset,
        value => SceneAnnotationKind::Constraint(parse_constraint_glyph(value)?),
    })
}

const fn constraint_glyph_key(glyph: SceneConstraintGlyph) -> &'static str {
    match glyph {
        SceneConstraintGlyph::Fixed => "constraint:fixed",
        SceneConstraintGlyph::Coincident => "constraint:coincident",
        SceneConstraintGlyph::Horizontal => "constraint:horizontal",
        SceneConstraintGlyph::Vertical => "constraint:vertical",
        SceneConstraintGlyph::PointOnCurve => "constraint:point-on-curve",
        SceneConstraintGlyph::Parallel => "constraint:parallel",
        SceneConstraintGlyph::Perpendicular => "constraint:perpendicular",
        SceneConstraintGlyph::Concentric => "constraint:concentric",
        SceneConstraintGlyph::Collinear => "constraint:collinear",
        SceneConstraintGlyph::EqualLength => "constraint:equal-length",
        SceneConstraintGlyph::EqualRadius => "constraint:equal-radius",
        SceneConstraintGlyph::Midpoint => "constraint:midpoint",
        SceneConstraintGlyph::Symmetry => "constraint:symmetry",
        SceneConstraintGlyph::Contact => "constraint:contact",
        SceneConstraintGlyph::Tangency => "constraint:tangency",
        SceneConstraintGlyph::Direction => "constraint:direction",
        SceneConstraintGlyph::Normal => "constraint:normal",
        SceneConstraintGlyph::EqualCurvature => "constraint:equal-curvature",
        SceneConstraintGlyph::Continuity => "constraint:continuity",
        SceneConstraintGlyph::Fillet => "constraint:fillet",
    }
}

fn parse_constraint_glyph(value: &str) -> Option<SceneConstraintGlyph> {
    Some(match value {
        "constraint:fixed" => SceneConstraintGlyph::Fixed,
        "constraint:coincident" => SceneConstraintGlyph::Coincident,
        "constraint:horizontal" => SceneConstraintGlyph::Horizontal,
        "constraint:vertical" => SceneConstraintGlyph::Vertical,
        "constraint:point-on-curve" => SceneConstraintGlyph::PointOnCurve,
        "constraint:parallel" => SceneConstraintGlyph::Parallel,
        "constraint:perpendicular" => SceneConstraintGlyph::Perpendicular,
        "constraint:concentric" => SceneConstraintGlyph::Concentric,
        "constraint:collinear" => SceneConstraintGlyph::Collinear,
        "constraint:equal-length" => SceneConstraintGlyph::EqualLength,
        "constraint:equal-radius" => SceneConstraintGlyph::EqualRadius,
        "constraint:midpoint" => SceneConstraintGlyph::Midpoint,
        "constraint:symmetry" => SceneConstraintGlyph::Symmetry,
        "constraint:contact" => SceneConstraintGlyph::Contact,
        "constraint:tangency" => SceneConstraintGlyph::Tangency,
        "constraint:direction" => SceneConstraintGlyph::Direction,
        "constraint:normal" => SceneConstraintGlyph::Normal,
        "constraint:equal-curvature" => SceneConstraintGlyph::EqualCurvature,
        "constraint:continuity" => SceneConstraintGlyph::Continuity,
        "constraint:fillet" => SceneConstraintGlyph::Fillet,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceRevisions {
    pub(crate) design: u64,
    pub(crate) attempt: u64,
    pub(crate) accepted: Option<u64>,
}

impl WorkspaceSnapshot {
    pub(crate) fn from_coordinator(
        coordinator: &RetainedEditorCoordinator,
    ) -> Result<Self, String> {
        let checkpoint = coordinator
            .persistence_checkpoint()
            .map_err(|error| error.to_string())?;
        Ok(Self::from_checkpoint(
            &checkpoint,
            coordinator.editor().annotation_layout(),
        ))
    }

    /// Captures one projectional workspace from its sole durable authority.
    ///
    /// The flat documents in workspace v8 are authenticated reconstruction
    /// evidence only. They are derived from the projectional coordinator's
    /// last accepted cold materialization and never from a second retained
    /// editor coordinator or history.
    pub(crate) fn from_projectional_editor(
        projectional: &ProjectionalEditorSession,
    ) -> Result<Self, String> {
        Self::from_projectional_editor_with_evaluation_high_water(
            projectional,
            default_evaluation_high_water(),
        )
    }

    pub(crate) fn from_projectional_editor_with_evaluation_high_water(
        projectional: &ProjectionalEditorSession,
        computed_evaluation_high_water: ComputedEvaluationAllocatorHighWater,
    ) -> Result<Self, String> {
        let native = &projectional
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| {
                "projectional workspace has no independently accepted materialization".to_owned()
            })?
            .session;
        let revisions = native.revision_high_water();
        Self::from_projectional_editor_with_persistence_high_water(
            projectional,
            computed_evaluation_high_water,
            WorkspaceRevisions {
                design: revisions.design().get(),
                attempt: revisions.attempt().get(),
                accepted: revisions
                    .accepted()
                    .map(geosolve_sketch::SketchAcceptedRevision::get),
            },
        )
    }

    pub(crate) fn from_projectional_editor_with_persistence_high_water(
        projectional: &ProjectionalEditorSession,
        computed_evaluation_high_water: ComputedEvaluationAllocatorHighWater,
        retained_revisions: WorkspaceRevisions,
    ) -> Result<Self, String> {
        let coordinator = projectional.coordinator();
        let intent = coordinator.intent();
        let materialization = coordinator.accepted_materialization().ok_or_else(|| {
            "projectional workspace has no independently accepted materialization".to_owned()
        })?;
        let native = &materialization.session;
        let accepted = native
            .accepted_state_for_current_input()
            .ok_or_else(|| {
                "projectional workspace has no current accepted native scene".to_owned()
            })?
            .document();
        let design = native.design_document();
        let features = materialization.features.clone();
        let feature_lifecycle_high_water = materialization.feature_lifecycle_high_water;
        let revisions = retained_revisions;
        let accepted_belongs_to_current_design = intent
            .accepted()
            .is_some_and(|authority| authority.target == intent.semantic_identity());

        Self {
            version: PROJECTIONAL_WORKSPACE_VERSION,
            design: document_payload(design)?,
            accepted: Some(document_payload(accepted)?),
            accepted_belongs_to_current_design,
            sketch_identity_high_water: native.persistent_identity_high_water().clone(),
            features_json: features.to_json().map_err(|error| error.to_string())?,
            feature_lifecycle_high_water,
            computed_evaluation_high_water,
            annotation_layout_json: encode_annotation_layout(
                projectional.editor().annotation_layout(),
            ),
            revisions,
            intent_session_json: Some(
                intent
                    .to_canonical_json()
                    .map_err(|error| error.to_string())?,
            ),
            origin: WorkspaceSnapshotOrigin::ProjectionalV8,
        }
        .validated()
    }

    fn from_checkpoint(
        checkpoint: &RestoreCheckpoint,
        annotation_layout: &AnnotationLayoutState,
    ) -> Self {
        let revisions = checkpoint.revisions();
        Self {
            version: FLAT_WORKSPACE_VERSION,
            design: WorkspaceDocumentPayload {
                encoding: if checkpoint.design_uses_draft_v5() {
                    WorkspaceDocumentEncoding::DraftV5
                } else {
                    WorkspaceDocumentEncoding::CanonicalV4
                },
                json: checkpoint.design_json().to_owned(),
            },
            accepted: checkpoint
                .accepted_json()
                .map(|json| WorkspaceDocumentPayload {
                    encoding: if checkpoint.accepted_uses_draft_v5() {
                        WorkspaceDocumentEncoding::DraftV5
                    } else {
                        WorkspaceDocumentEncoding::CanonicalV4
                    },
                    json: json.to_owned(),
                }),
            accepted_belongs_to_current_design: checkpoint.accepted_belongs_to_current_design(),
            sketch_identity_high_water: checkpoint.sketch_identity_high_water().clone(),
            features_json: checkpoint.feature_json().to_owned(),
            feature_lifecycle_high_water: checkpoint.feature_lifecycle_high_water(),
            computed_evaluation_high_water: checkpoint.computed_evaluation_high_water(),
            annotation_layout_json: encode_annotation_layout(annotation_layout),
            revisions: WorkspaceRevisions {
                design: revisions.design().get(),
                attempt: revisions.attempt().get(),
                accepted: revisions
                    .accepted()
                    .map(geosolve_sketch::SketchAcceptedRevision::get),
            },
            intent_session_json: None,
            origin: WorkspaceSnapshotOrigin::FlatV6,
        }
    }

    pub(crate) const fn revisions(&self) -> SketchLifecycleRevisionHighWater {
        SketchLifecycleRevisionHighWater::from_raw(
            self.revisions.design,
            self.revisions.attempt,
            self.revisions.accepted,
        )
    }

    pub(crate) fn encode(&self) -> Result<String, String> {
        match self.origin {
            WorkspaceSnapshotOrigin::FlatV6 if self.version == FLAT_WORKSPACE_VERSION => {
                let json = serde_json::to_string(self).map_err(|error| error.to_string())?;
                if json.len() > MAX_WORKSPACE_JSON_BYTES {
                    return Err(format!(
                        "workspace v6 exceeds the {MAX_WORKSPACE_JSON_BYTES}-byte limit"
                    ));
                }
                Ok(json)
            }
            WorkspaceSnapshotOrigin::ProjectionalV8
                if self.version == PROJECTIONAL_WORKSPACE_VERSION =>
            {
                self.validate()?;
                let intent_session_json = self
                    .intent_session_json
                    .as_deref()
                    .ok_or_else(|| "workspace v8 requires a canonical intent session".to_owned())?;
                let materialization = WorkspaceMaterializationV8Ref::from_snapshot(self);
                let annotation_layout_json = self.annotation_layout_json.as_deref();
                let digest_payload = workspace_v8_digest_payload_from_parts(
                    PROJECTIONAL_WORKSPACE_VERSION,
                    intent_session_json,
                    &materialization,
                    &annotation_layout_json,
                );
                let digest = intent_content_digest(&digest_payload);
                drop(digest_payload);
                let wire = WorkspaceSnapshotV8Ref {
                    version: PROJECTIONAL_WORKSPACE_VERSION,
                    intent_session_json,
                    materialization,
                    annotation_layout_json,
                    digest,
                };
                let json = serde_json::to_string(&wire).map_err(|error| error.to_string())?;
                if json.len() > MAX_WORKSPACE_JSON_BYTES {
                    return Err(format!(
                        "workspace v8 exceeds the {MAX_WORKSPACE_JSON_BYTES}-byte limit"
                    ));
                }
                Ok(json)
            }
            WorkspaceSnapshotOrigin::LegacyBootstrap { source_version } => Err(format!(
                "legacy workspace v{source_version} must be normalized into per-object bootstrap declarations before v8 encoding"
            )),
            _ => Err("workspace snapshot version/origin is inconsistent".into()),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed workspace-version migration matrix is clearer when audited in one dispatch"
    )]
    pub(crate) fn decode(input: &str) -> Result<Self, String> {
        if input.len() > MAX_WORKSPACE_JSON_BYTES {
            return Err(format!(
                "workbench snapshot exceeds the {MAX_WORKSPACE_JSON_BYTES}-byte limit"
            ));
        }
        let version = serde_json::from_str::<JsonVersionProbe>(input)
            .map_err(|error| error.to_string())?
            .version;
        match version {
            1 => {
                let legacy: LegacyWorkspaceSnapshotV1 =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                if legacy.version != 1 {
                    return Err("unsupported workbench snapshot version".into());
                }
                let design_document = SketchDocument::from_json(&legacy.design_json)
                    .map_err(|error| error.to_string())?;
                let (features_json, feature_lifecycle_high_water) =
                    empty_feature_bundle(&design_document)?;
                let design = WorkspaceDocumentPayload {
                    encoding: WorkspaceDocumentEncoding::CanonicalV4,
                    json: legacy.design_json,
                };
                let accepted = legacy.accepted_json.map(|json| WorkspaceDocumentPayload {
                    encoding: WorkspaceDocumentEncoding::CanonicalV4,
                    json,
                });
                let sketch_identity_high_water =
                    derive_sketch_identity_high_water(&design, accepted.as_ref())?;
                Self {
                    version: FLAT_WORKSPACE_VERSION,
                    design,
                    accepted,
                    accepted_belongs_to_current_design: false,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
                    revisions: legacy.revisions,
                    intent_session_json: None,
                    origin: WorkspaceSnapshotOrigin::LegacyBootstrap { source_version: 1 },
                }
                .validated()
            }
            2 => {
                let legacy: LegacyWorkspaceSnapshotV2 =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                if legacy.version != 2 {
                    return Err("unsupported workbench snapshot version".into());
                }
                let design = decode_document(&legacy.design)?;
                let (features_json, feature_lifecycle_high_water) = empty_feature_bundle(&design)?;
                let sketch_identity_high_water =
                    derive_sketch_identity_high_water(&legacy.design, legacy.accepted.as_ref())?;
                Self {
                    version: FLAT_WORKSPACE_VERSION,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: false,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
                    revisions: legacy.revisions,
                    intent_session_json: None,
                    origin: WorkspaceSnapshotOrigin::LegacyBootstrap { source_version: 2 },
                }
                .validated()
            }
            3 => {
                let legacy: LegacyWorkspaceSnapshotV3 =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                if legacy.version != 3 {
                    return Err("unsupported workbench snapshot version".into());
                }
                if legacy.accepted_belongs_to_current_design && legacy.accepted.is_none() {
                    return Err(
                        "current-design accepted provenance requires an accepted payload".into(),
                    );
                }
                let design = decode_document(&legacy.design)?;
                let (features_json, feature_lifecycle_high_water) = empty_feature_bundle(&design)?;
                let sketch_identity_high_water =
                    derive_sketch_identity_high_water(&legacy.design, legacy.accepted.as_ref())?;
                Self {
                    version: FLAT_WORKSPACE_VERSION,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: legacy.accepted_belongs_to_current_design,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
                    revisions: legacy.revisions,
                    intent_session_json: None,
                    origin: WorkspaceSnapshotOrigin::LegacyBootstrap { source_version: 3 },
                }
                .validated()
            }
            4 => {
                let legacy: LegacyWorkspaceSnapshotV4 =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                if legacy.version != 4 {
                    return Err("unsupported workbench snapshot version".into());
                }
                let sketch_identity_high_water =
                    derive_sketch_identity_high_water(&legacy.design, legacy.accepted.as_ref())?;
                Self {
                    version: FLAT_WORKSPACE_VERSION,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: legacy.accepted_belongs_to_current_design,
                    sketch_identity_high_water,
                    features_json: legacy.features_json,
                    feature_lifecycle_high_water: legacy.feature_lifecycle_high_water,
                    computed_evaluation_high_water: legacy.computed_evaluation_high_water,
                    annotation_layout_json: None,
                    revisions: legacy.revisions,
                    intent_session_json: None,
                    origin: WorkspaceSnapshotOrigin::LegacyBootstrap { source_version: 4 },
                }
                .validated()
            }
            5 => {
                let mut snapshot: Self =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                snapshot.version = FLAT_WORKSPACE_VERSION;
                snapshot.annotation_layout_json = None;
                snapshot.intent_session_json = None;
                snapshot.origin = WorkspaceSnapshotOrigin::LegacyBootstrap { source_version: 5 };
                snapshot.validated()
            }
            FLAT_WORKSPACE_VERSION => {
                let mut snapshot: Self =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                snapshot.intent_session_json = None;
                snapshot.origin = WorkspaceSnapshotOrigin::LegacyBootstrap { source_version: 6 };
                snapshot.validated()
            }
            ABANDONED_WORKSPACE_VERSION => Err(
                "workbench snapshot version 7 belongs to the abandoned chronological lineage format and is not supported"
                    .into(),
            ),
            PROJECTIONAL_WORKSPACE_VERSION => Self::decode_v8(input),
            _ => Err("unsupported workbench snapshot version".into()),
        }
    }

    fn decode_v8(input: &str) -> Result<Self, String> {
        if input.len() > MAX_WORKSPACE_JSON_BYTES {
            return Err(format!(
                "workspace v8 exceeds the {MAX_WORKSPACE_JSON_BYTES}-byte limit"
            ));
        }
        let wire: WorkspaceSnapshotV8 =
            serde_json::from_str(input).map_err(|error| error.to_string())?;
        if wire.version != PROJECTIONAL_WORKSPACE_VERSION {
            return Err("unsupported workbench snapshot version".into());
        }
        let digest_payload = workspace_v8_digest_payload(&wire);
        let has_sha_outer_digest = wire.digest == intent_content_digest(&digest_payload);
        let has_legacy_outer_digest =
            !has_sha_outer_digest && wire.digest == intent_legacy_content_digest(&digest_payload);
        if !has_sha_outer_digest && !has_legacy_outer_digest {
            return Err("workspace v8 digest does not match its contents".into());
        }
        drop(digest_payload);
        if !is_exact_canonical_json(&wire, input)? {
            return Err("workspace v8 JSON is not canonical".into());
        }
        let nested_version = serde_json::from_str::<JsonVersionProbe>(&wire.intent_session_json)
            .map_err(|error| error.to_string())?
            .version;
        if has_legacy_outer_digest && nested_version != 1 {
            return Err(
                "legacy workspace-v8 digest requires a canonical legacy intent session".into(),
            );
        }
        let intent = IntentSession::from_json(&wire.intent_session_json)
            .map_err(|error| error.to_string())?;
        let intent_session_json = if nested_version == 1 {
            intent
                .to_canonical_json()
                .map_err(|error| error.to_string())?
        } else {
            wire.intent_session_json
        };
        let materialization = wire.materialization;
        let snapshot = Self {
            version: PROJECTIONAL_WORKSPACE_VERSION,
            design: materialization.design,
            accepted: materialization.accepted,
            accepted_belongs_to_current_design: materialization.accepted_belongs_to_current_design,
            sketch_identity_high_water: materialization.sketch_identity_high_water,
            features_json: materialization.features_json,
            feature_lifecycle_high_water: materialization.feature_lifecycle_high_water,
            computed_evaluation_high_water: materialization.computed_evaluation_high_water,
            annotation_layout_json: wire
                .annotation_layout_json
                .filter(|json| json.len() <= MAX_ANNOTATION_LAYOUT_JSON_BYTES),
            revisions: materialization.revisions,
            intent_session_json: Some(intent_session_json),
            origin: WorkspaceSnapshotOrigin::ProjectionalV8,
        };
        snapshot.validate_with_intent(&intent)?;
        Ok(snapshot)
    }

    fn validated(mut self) -> Result<Self, String> {
        if self
            .annotation_layout_json
            .as_ref()
            .is_some_and(|json| json.len() > MAX_ANNOTATION_LAYOUT_JSON_BYTES)
        {
            self.annotation_layout_json = None;
        }
        self.validate()?;
        Ok(self)
    }

    fn validate(&self) -> Result<(), String> {
        let intent = match self.origin {
            WorkspaceSnapshotOrigin::ProjectionalV8 => Some(
                self.intent_session()?
                    .ok_or_else(|| "workspace v8 requires a canonical intent session".to_owned())?,
            ),
            WorkspaceSnapshotOrigin::FlatV6 | WorkspaceSnapshotOrigin::LegacyBootstrap { .. } => {
                None
            }
        };
        self.validate_with_optional_intent(intent.as_ref())
    }

    fn validate_with_intent(&self, intent: &IntentSession) -> Result<(), String> {
        self.validate_with_optional_intent(Some(intent))
    }

    fn validate_with_optional_intent(&self, intent: Option<&IntentSession>) -> Result<(), String> {
        if !matches!(
            self.version,
            FLAT_WORKSPACE_VERSION | PROJECTIONAL_WORKSPACE_VERSION
        ) {
            return Err("unsupported workbench snapshot version".into());
        }
        if self.accepted_belongs_to_current_design && self.accepted.is_none() {
            return Err("current-design accepted provenance requires an accepted payload".into());
        }
        let design = self.design_document()?;
        let accepted = self.accepted_document()?;
        match self.origin {
            WorkspaceSnapshotOrigin::FlatV6 | WorkspaceSnapshotOrigin::LegacyBootstrap { .. } => {
                if self.version != FLAT_WORKSPACE_VERSION || self.intent_session_json.is_some() {
                    return Err("flat workspace carries inconsistent intent authority".into());
                }
            }
            WorkspaceSnapshotOrigin::ProjectionalV8 => {
                if self.version != PROJECTIONAL_WORKSPACE_VERSION {
                    return Err("projectional workspace carries an inconsistent version".into());
                }
                let intent = intent
                    .ok_or_else(|| "workspace v8 requires a canonical intent session".to_owned())?;
                validate_intent_accepted_materialization(
                    intent,
                    self.accepted.as_ref(),
                    self.accepted_belongs_to_current_design,
                )?;
            }
        }
        validate_sketch_identity_high_water(
            &self.sketch_identity_high_water,
            &design,
            accepted.as_ref(),
        )?;
        let features = self.feature_document()?;
        if features.sketch_document() != design.id() {
            return Err("computed-feature sidecar belongs to a different sketch".into());
        }
        if self.feature_lifecycle_high_water.revision < features.revision()
            || self.feature_lifecycle_high_water.allocator.next_feature_id
                < features.allocator_high_water().next_feature_id
            || self.feature_lifecycle_high_water.allocator.next_corner_id
                < features.allocator_high_water().next_corner_id
        {
            return Err("computed-feature lifecycle high-water trails the sidecar".into());
        }
        if self.computed_evaluation_high_water.next_revision.raw() == 0 {
            return Err("computed-feature evaluation high-water must be nonzero".into());
        }
        Ok(())
    }

    pub(crate) fn intent_session(&self) -> Result<Option<IntentSession>, String> {
        self.intent_session_json
            .as_deref()
            .map(IntentSession::from_json)
            .transpose()
            .map_err(|error| error.to_string())
    }

    #[allow(
        dead_code,
        reason = "consumed by the pending per-object legacy normalization activation slice"
    )]
    pub(crate) fn legacy_bootstrap(&self) -> Option<WorkspaceLegacyBootstrap<'_>> {
        let WorkspaceSnapshotOrigin::LegacyBootstrap { source_version } = self.origin else {
            return None;
        };
        Some(WorkspaceLegacyBootstrap {
            source_version,
            snapshot: self,
        })
    }

    pub(crate) fn annotation_layout(&self) -> AnnotationLayoutState {
        self.annotation_layout_json
            .as_deref()
            .and_then(decode_annotation_layout)
            .unwrap_or_default()
    }

    pub(crate) fn design_document(&self) -> Result<SketchDocument, String> {
        decode_document(&self.design)
    }

    pub(crate) fn accepted_document(&self) -> Result<Option<SketchDocument>, String> {
        self.accepted.as_ref().map(decode_document).transpose()
    }

    pub(crate) fn feature_document(&self) -> Result<ComputedFeatureDocument, String> {
        ComputedFeatureDocument::from_json(&self.features_json).map_err(|error| error.to_string())
    }

    pub(crate) const fn feature_lifecycle_high_water(&self) -> ComputedFeatureLifecycleHighWater {
        self.feature_lifecycle_high_water
    }

    pub(crate) const fn computed_evaluation_high_water(
        &self,
    ) -> ComputedEvaluationAllocatorHighWater {
        self.computed_evaluation_high_water
    }

    pub(crate) fn restore_session(
        &self,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<RetainedSketchDocumentSession, String> {
        let design = self.design_document()?;
        let mut restored = if let Some(accepted) = self.accepted_document()? {
            if self.accepted_belongs_to_current_design {
                RetainedSketchDocumentSession::restore_current_design_with_accepted(
                    design,
                    accepted,
                    self.revisions(),
                    request,
                    config,
                )
            } else {
                RetainedSketchDocumentSession::restore_design_with_accepted(
                    design,
                    accepted,
                    self.revisions(),
                    request,
                    config,
                )
            }
        } else {
            RetainedSketchDocumentSession::restore_design(design, self.revisions(), request, config)
        }
        .map_err(|error| error.to_string())?;
        restored
            .retain_persistent_identity_high_water(&self.sketch_identity_high_water)
            .map_err(|error| error.to_string())?;
        Ok(restored)
    }
}

pub(crate) fn coordinator_from_snapshot(
    snapshot: &WorkspaceSnapshot,
) -> Result<RetainedEditorCoordinator, String> {
    if snapshot.intent_session_json.is_some() {
        return Err(
            "workspace v8 must restore through its sole projectional editor authority".into(),
        );
    }
    let session =
        snapshot.restore_session(DocumentSolveRequest::default(), SolverConfig::default())?;
    let cached_layout = snapshot.annotation_layout();
    let features = snapshot.feature_document()?;
    let mut coordinator = RetainedEditorCoordinator::with_features_and_high_water(
        session,
        features,
        snapshot.feature_lifecycle_high_water(),
        snapshot.computed_evaluation_high_water(),
    )
    .map_err(|error| error.to_string())?;
    let layout = compatible_annotation_layout(&coordinator, &cached_layout);
    coordinator.editor_mut().restore_annotation_layout(layout);
    Ok(coordinator)
}

/// Cold-restores workspace v8 into its sole projectional editor authority.
///
/// Stored flat documents are comparison evidence only. They are never
/// installed into a parallel retained editor coordinator. Computed-feature
/// sidecars fail closed until their declarations have an authenticated
/// projectional materializer path.
#[allow(
    clippy::too_many_lines,
    reason = "one cold-restore boundary validates every persisted authority before installing the projectional editor"
)]
pub(crate) fn projectional_editor_from_snapshot(
    snapshot: &WorkspaceSnapshot,
) -> Result<ProjectionalEditorSession, String> {
    let intent = snapshot
        .intent_session()?
        .ok_or_else(|| "workspace does not contain projectional intent authority".to_owned())?;
    let features = snapshot.feature_document()?;
    let contains_bootstrap = intent
        .graph()
        .nodes()
        .values()
        .any(intent_node_requires_bootstrap_seed)
        || intent.accepted().is_some_and(|accepted| {
            accepted
                .graph
                .nodes()
                .values()
                .any(intent_node_requires_bootstrap_seed)
        });
    if contains_bootstrap {
        let canonical_bootstrap = decode_flat_intent_bootstrap(&intent).ok();
        let mut projectional = if let Some(decoded) = canonical_bootstrap {
            if decoded.document != snapshot.design_document()?
                || decoded.features != features
                || decoded.feature_lifecycle_high_water != snapshot.feature_lifecycle_high_water()
            {
                return Err(
                    "workspace v8 bootstrap declarations disagree with stored native evidence"
                        .into(),
                );
            }
            let native = snapshot
                .restore_session(DocumentSolveRequest::default(), SolverConfig::default())?;
            ProjectionalEditorSession::restore_native_bootstrap(intent, native)
                .map_err(|error| error.to_string())?
        } else {
            let stored_design = snapshot.design_document()?;
            let stored_accepted = snapshot.accepted_document()?;
            let retained_native = snapshot
                .restore_session(DocumentSolveRequest::default(), SolverConfig::default())?;
            let projectional = ProjectionalEditorSession::restore_retained_native_bootstrap(
                intent.clone(),
                retained_native,
            )
            .or_else(|_| ProjectionalEditorSession::restore_with_bootstrap_prefix(intent))
            .map_err(|error| error.to_string())?;
            let rebuilt = projectional
                .coordinator()
                .accepted_materialization()
                .ok_or_else(|| {
                    "mixed bootstrap reconstruction omitted accepted native evidence".to_owned()
                })?;
            let rebuilt_accepted = rebuilt
                .session
                .accepted_state_for_current_input()
                .ok_or_else(|| {
                    "mixed bootstrap reconstruction omitted current accepted evidence".to_owned()
                })?;
            if rebuilt.session.design_document() != &stored_design
                || Some(rebuilt_accepted.document()) != stored_accepted.as_ref()
                || rebuilt.features != features
                || rebuilt.feature_lifecycle_high_water != snapshot.feature_lifecycle_high_water()
            {
                return Err(
                    "mixed bootstrap reconstruction disagrees with stored flat evidence".into(),
                );
            }
            projectional
        };
        let layout = compatible_annotation_layout_for_projectional(
            &projectional,
            &snapshot.annotation_layout(),
        );
        projectional.editor_mut().restore_annotation_layout(layout);
        return Ok(projectional);
    }
    let stored_design = snapshot.design_document()?;
    let stored_accepted = snapshot.accepted_document()?;
    let mut projectional =
        ProjectionalEditorSession::restore(intent, stored_design.id(), stored_design.model_scale())
            .map_err(|error| error.to_string())?;

    let rebuilt = projectional.coordinator().accepted_materialization();
    match (rebuilt, stored_accepted.as_ref()) {
        (Some(rebuilt), Some(stored_accepted)) => {
            let rebuilt_session = &rebuilt.session;
            let rebuilt_accepted = rebuilt_session
                .accepted_state_for_current_input()
                .ok_or_else(|| {
                    "cold projectional restore omitted current accepted native evidence".to_owned()
                })?;
            if rebuilt_session.design_document() != &stored_design
                || rebuilt_accepted.document() != stored_accepted
                || rebuilt.features != features
            {
                return Err(
                    "cold projectional reconstruction disagrees with stored flat evidence".into(),
                );
            }
        }
        (None, None) => {
            return Err(
                "workspace v8 omitted both projectional and flat accepted authority".into(),
            );
        }
        _ => {
            return Err(
                "cold projectional reconstruction and stored accepted authority disagree".into(),
            );
        }
    }

    let layout =
        compatible_annotation_layout_for_projectional(&projectional, &snapshot.annotation_layout());
    projectional.editor_mut().restore_annotation_layout(layout);
    Ok(projectional)
}

fn intent_node_requires_bootstrap_seed(node: &geosolve_sketch_intent::IntentNode) -> bool {
    matches!(
        node.kind,
        geosolve_sketch_intent::IntentNodeKind::Bootstrap { .. }
    ) || node.bootstrap_origin.is_some()
}

/// Strictly restores and normalizes one historical flat workspace into a
/// history-free projectional authority.
///
/// The ordinary workspace decoder and retained-session restore remain the
/// authority for legacy bytes, IDs, lifecycle high-water and independent
/// accepted-state validation. Normalization then records exactly one typed
/// bootstrap declaration per persisted object without inventing recipes or a
/// user-visible migration transaction.
pub(crate) fn projectional_editor_from_legacy_snapshot(
    snapshot: &WorkspaceSnapshot,
) -> Result<ProjectionalEditorSession, String> {
    let bootstrap = snapshot
        .legacy_bootstrap()
        .ok_or_else(|| "workspace is not a strict v1-v6 legacy bootstrap".to_owned())?;
    let native =
        snapshot.restore_session(DocumentSolveRequest::default(), SolverConfig::default())?;
    let features = bootstrap.feature_document()?;
    let annotation_layout = snapshot.annotation_layout();
    projectional_editor_from_flat_parts(
        native,
        &features,
        bootstrap.feature_lifecycle_high_water(),
        &annotation_layout,
        bootstrap.source_version(),
    )
}

/// Normalizes one already accepted in-process flat coordinator, such as a
/// sample-library fixture, into the same history-free per-object projectional
/// bootstrap used by strict v1-v6 workspace migration.
///
/// The coordinator is borrowed only as authenticated native input. Its flat
/// Undo/Redo stack is deliberately not copied into the returned authority.
pub(crate) fn projectional_editor_from_flat_coordinator(
    coordinator: &RetainedEditorCoordinator,
) -> Result<ProjectionalEditorSession, String> {
    let checkpoint = coordinator
        .persistence_checkpoint()
        .map_err(|error| error.to_string())?;
    projectional_editor_from_flat_parts(
        coordinator.session().clone(),
        coordinator.feature_document(),
        checkpoint.feature_lifecycle_high_water(),
        coordinator.editor().annotation_layout(),
        FLAT_WORKSPACE_VERSION,
    )
}

fn projectional_editor_from_flat_parts(
    native: RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    annotation_layout: &AnnotationLayoutState,
    source_version: u32,
) -> Result<ProjectionalEditorSession, String> {
    let accepted = native
        .accepted_state_for_current_input()
        .ok_or_else(|| {
            format!(
                "legacy workspace v{source_version} has no current accepted scene for safe projectional bootstrap activation"
            )
        })?
        .document()
        .clone();
    let design = native.design_document().clone();
    let session_id = IntentSessionId::from_raw(
        (design.id().0.as_u128() ^ 0x6765_6f73_6f6c_7665_2d6d_3833_2d76_3800_u128).max(1),
    );
    let intent = normalize_flat_sketch_intent_with_accepted_materialization(
        session_id,
        &design,
        &accepted,
        features,
        feature_lifecycle_high_water,
    )
    .map_err(|error| error.to_string())?;
    let mut projectional = ProjectionalEditorSession::restore_native_bootstrap(intent, native)
        .map_err(|error| error.to_string())?;
    let layout = compatible_annotation_layout_for_projectional(&projectional, annotation_layout);
    projectional.editor_mut().restore_annotation_layout(layout);
    Ok(projectional)
}

fn compatible_annotation_layout(
    coordinator: &RetainedEditorCoordinator,
    cached: &AnnotationLayoutState,
) -> AnnotationLayoutState {
    let session = coordinator.session();
    let design = session.design_document();
    let Some(accepted) = session.accepted_state() else {
        return AnnotationLayoutState::default();
    };
    let Ok(viewport) = Viewport::new([1024.0, 768.0], [0.0, 0.0], 1.0) else {
        return AnnotationLayoutState::default();
    };
    let Ok(scene) = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        session.design_identity(),
        accepted.document(),
        design,
        viewport,
        0.5,
    ) else {
        return AnnotationLayoutState::default();
    };

    compatible_annotation_layout_entries(cached, design, &scene)
}

fn compatible_annotation_layout_for_projectional(
    projectional: &ProjectionalEditorSession,
    cached: &AnnotationLayoutState,
) -> AnnotationLayoutState {
    let Some(session) = projectional.coordinator().presentation_session() else {
        return AnnotationLayoutState::default();
    };
    let design = session.design_document();
    let Some(_accepted) = session.accepted_state_for_current_input() else {
        return AnnotationLayoutState::default();
    };
    let Ok(viewport) = Viewport::new([1024.0, 768.0], [0.0, 0.0], 1.0) else {
        return AnnotationLayoutState::default();
    };
    let Ok(scene) = projectional.scene(viewport, 0.5) else {
        return AnnotationLayoutState::default();
    };

    compatible_annotation_layout_entries(cached, design, &scene)
}

fn compatible_annotation_layout_entries(
    cached: &AnnotationLayoutState,
    design: &SketchDocument,
    scene: &EditorScene,
) -> AnnotationLayoutState {
    AnnotationLayoutState::from_entries(cached.entries().into_iter().filter(|entry| {
        if entry.key.document != design.id() || !layout_item_source_is_current(*entry, design) {
            return false;
        }
        scene
            .annotations
            .iter()
            .find(|annotation| {
                annotation.item == entry.key.item
                    && annotation.source == entry.key.source
                    && annotation.kind == entry.key.kind
            })
            .is_some_and(|annotation| layout_form_is_compatible(*entry, annotation))
    }))
}

fn document_payload(document: &SketchDocument) -> Result<WorkspaceDocumentPayload, String> {
    match document.to_canonical_json() {
        Ok(json) => Ok(WorkspaceDocumentPayload {
            encoding: WorkspaceDocumentEncoding::CanonicalV4,
            json,
        }),
        Err(_) => Ok(WorkspaceDocumentPayload {
            encoding: WorkspaceDocumentEncoding::DraftV5,
            json: document
                .to_draft_v5_json()
                .map_err(|error| error.to_string())?,
        }),
    }
}

fn layout_item_source_is_current(entry: AnnotationLayoutEntry, design: &SketchDocument) -> bool {
    match entry.key.item {
        SelectionItem::Constraint(id) => design
            .constraint(id)
            .is_some_and(|constraint| constraint.source_id == entry.key.source),
        SelectionItem::Dimension(id) => design
            .dimension(id)
            .is_some_and(|dimension| dimension.source_id == entry.key.source),
        SelectionItem::Point(_)
        | SelectionItem::Curve(_)
        | SelectionItem::Datum(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => false,
    }
}

fn layout_form_is_compatible(
    entry: AnnotationLayoutEntry,
    annotation: &geosolve_constraint_editor::SceneAnnotation,
) -> bool {
    match (
        &annotation.geometry,
        entry.key.marker_index,
        entry.placement,
    ) {
        (
            SceneAnnotationGeometry::Glyph { markers },
            Some(index),
            AnnotationPlacement::Free { .. },
        ) => index < markers.len(),
        // A genuine perpendicular corner is fixed in this viewport, but its
        // two fallback marks may become visible and movable after a camera
        // change. Keep only those two semantically valid dormant occurrences.
        (
            SceneAnnotationGeometry::RightAngle { .. },
            Some(index),
            AnnotationPlacement::Free { .. },
        ) => {
            annotation.kind == SceneAnnotationKind::Constraint(SceneConstraintGlyph::Perpendicular)
                && index < 2
        }
        (
            SceneAnnotationGeometry::LinearDimension { .. },
            None,
            AnnotationPlacement::Linear { .. },
        )
        | (
            SceneAnnotationGeometry::RadialDimension { .. },
            None,
            AnnotationPlacement::Radial { .. },
        )
        | (
            SceneAnnotationGeometry::AngularDimension { .. },
            None,
            AnnotationPlacement::Angular { .. },
        )
        | (SceneAnnotationGeometry::Label { .. }, None, AnnotationPlacement::Free { .. }) => true,
        _ => false,
    }
}

pub(crate) fn reproduction_payload_from_coordinator(
    coordinator: &RetainedEditorCoordinator,
) -> Result<String, String> {
    reproduction_payload_from_snapshot(WorkspaceSnapshot::from_coordinator(coordinator)?)
}

/// Encodes one already-authenticated application workspace as a bounded reproduction capsule.
///
/// This boundary deliberately accepts the complete workspace snapshot rather than a flat editor
/// coordinator so the projectional v8 authority and its unified intent history travel through the
/// same transport. Annotation placement remains a disposable viewport cache for either authority.
pub(crate) fn reproduction_payload_from_snapshot(
    mut snapshot: WorkspaceSnapshot,
) -> Result<String, String> {
    // A reproduction capsule carries authoritative/reconstructable workspace state, not the
    // disposable per-viewport annotation cache retained by ordinary local workspace saves.
    snapshot.annotation_layout_json = None;
    let workspace = snapshot.encode()?;
    crate::reproduction::encode_workspace(&workspace).map_err(|error| error.to_string())
}

pub(crate) fn coordinator_from_reproduction_payload(
    payload: &str,
) -> Result<RetainedEditorCoordinator, String> {
    let snapshot = snapshot_from_reproduction_payload(payload)?;
    coordinator_from_snapshot(&snapshot)
}

/// Decodes and validates a bounded reproduction capsule before any live workbench authority is
/// replaced. The returned snapshot has no presentation cache, including when an older capsule
/// carried one, so the receiving viewport recomputes annotation placement deterministically.
pub(crate) fn snapshot_from_reproduction_payload(
    payload: &str,
) -> Result<WorkspaceSnapshot, String> {
    let workspace =
        crate::reproduction::decode_workspace(payload).map_err(|error| error.to_string())?;
    let mut snapshot = WorkspaceSnapshot::decode(&workspace)?;
    // Older capsules may have embedded this optional presentation cache. Ignore it so restoration
    // always recomputes placement from the accepted scene under the receiving viewport.
    snapshot.annotation_layout_json = None;
    snapshot.validated()
}

fn derive_sketch_identity_high_water(
    design: &WorkspaceDocumentPayload,
    accepted: Option<&WorkspaceDocumentPayload>,
) -> Result<SketchPersistentIdentityHighWater, String> {
    let design = decode_document(design)?;
    let mut high_water = design.persistent_identity_high_water();
    if let Some(accepted) = accepted {
        high_water = high_water
            .merged(&decode_document(accepted)?.persistent_identity_high_water())
            .map_err(|error| error.to_string())?;
    }
    Ok(high_water)
}

fn validate_sketch_identity_high_water(
    retained: &SketchPersistentIdentityHighWater,
    design: &SketchDocument,
    accepted: Option<&SketchDocument>,
) -> Result<(), String> {
    let mut required = design.persistent_identity_high_water();
    if let Some(accepted) = accepted {
        required = required
            .merged(&accepted.persistent_identity_high_water())
            .map_err(|error| error.to_string())?;
    }
    let merged = retained
        .merged(&required)
        .map_err(|error| error.to_string())?;
    if &merged != retained {
        return Err("persistent sketch identity high-water trails a stored document".into());
    }
    Ok(())
}

fn empty_feature_bundle(
    document: &SketchDocument,
) -> Result<(String, ComputedFeatureLifecycleHighWater), String> {
    let features = ComputedFeatureDocument::new(document.id());
    let lifecycle = features.lifecycle_high_water();
    let json = features.to_json().map_err(|error| error.to_string())?;
    Ok((json, lifecycle))
}

fn default_evaluation_high_water() -> ComputedEvaluationAllocatorHighWater {
    ComputedEvaluationAllocator::default().high_water()
}

fn validate_intent_accepted_materialization(
    intent: &IntentSession,
    accepted: Option<&WorkspaceDocumentPayload>,
    accepted_belongs_to_current_design: bool,
) -> Result<(), String> {
    let Some(authority) = intent.accepted() else {
        return Err(if accepted.is_some() {
            "workspace v8 flat restoration has accepted geometry without intent authority".into()
        } else if accepted_belongs_to_current_design {
            "workspace v8 accepted provenance has no intent authority".into()
        } else {
            "workspace v8 requires independently accepted intent and flat authority".into()
        });
    };
    let accepted = accepted.ok_or_else(|| {
        "workspace v8 intent authority has no exact flat accepted materialization".to_owned()
    })?;
    if (authority.target == intent.semantic_identity()) != accepted_belongs_to_current_design {
        return Err("workspace v8 intent and flat accepted-current provenance disagree".into());
    }

    let evidence_json = std::str::from_utf8(&authority.evidence.materialization)
        .map_err(|_| "workspace v8 intent materialization is not sketch JSON".to_owned())?;
    let evidence_document = SketchDocument::from_draft_v5_json(evidence_json)
        .or_else(|_| SketchDocument::from_json(evidence_json))
        .map_err(|error| format!("workspace v8 intent materialization is invalid: {error}"))?;
    if evidence_document != decode_document(accepted)? {
        return Err(
            "workspace v8 intent evidence and flat accepted materialization disagree".into(),
        );
    }
    Ok(())
}

fn decode_document(payload: &WorkspaceDocumentPayload) -> Result<SketchDocument, String> {
    match payload.encoding {
        WorkspaceDocumentEncoding::CanonicalV4 => SketchDocument::from_json(&payload.json),
        WorkspaceDocumentEncoding::DraftV5 => SketchDocument::from_draft_v5_json(&payload.json),
    }
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        AnnotationLayoutEntry, AnnotationLayoutKey, AnnotationLayoutState, AnnotationPlacement,
        AuthoringMutation, AuthoringOperand, AuthoringOutcome, AuthoringState, AuthoringTool,
        ColdIntentMaterializer, ComputedEdgeGeometry, ComputedFeatureEvaluationState,
        ComputedSceneState, ConstraintIntent, EditorScene, FeatureAuthoringCandidate,
        FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool, Modifiers,
        PointerInput, ProjectionalEditorSession, ProjectionalIntentCoordinator,
        RetainedEditorCoordinator, SceneAnnotationGeometry, SceneAnnotationKind,
        SceneConstraintGlyph, ScreenPoint, SelectionItem, Viewport, normalize_flat_sketch_intent,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        AlphaScenarioIds, AlphaScenarioKind, ContactStateEdit, CurveDefinition, CurveId, CurveSpan,
        DesignPointId, DocumentBSplineForm, DocumentCenterRef, DocumentCommandEffect,
        DocumentConstraintDefinition, DocumentDirectionSense, DocumentEdit, DocumentError,
        DocumentId, DocumentLineSupportRef, DocumentObjectId, DocumentSolveRequest, GeometryRole,
        PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
        alpha_scenario,
    };
    use geosolve_sketch_intent::{
        BootstrapNativeKind, ComponentIdentity, ContentDigest, DeletePolicy, GeometryRecipeKind,
        IntentAcceptedAuthority, IntentAllocatorHighWater, IntentEvaluation, IntentExternalInputs,
        IntentExternalInputsIdentity, IntentGraph, IntentGraphIdentity, IntentInstanceIdentity,
        IntentInstanceState, IntentKey, IntentLatestAttempt, IntentLiteral, IntentNodeDraft,
        IntentNodeKind, IntentOrganization, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
        IntentPlanDisposition, IntentPortRole, IntentPortSelector, IntentReservationLedger,
        IntentReservationLedgerIdentity, IntentSemanticIdentity, IntentSession, IntentSessionId,
        IntentTransactionDescriptor, IntentUnit, LeafField, ReservationId, Revision,
        intent_legacy_content_digest,
    };
    use serde::{Deserialize, Serialize};

    use super::{
        WorkspaceSnapshot, WorkspaceSnapshotV8, annotation_kind_key,
        coordinator_from_reproduction_payload, coordinator_from_snapshot,
        default_evaluation_high_water, derive_sketch_identity_high_water,
        intent_node_requires_bootstrap_seed, legacy_workspace_v8_digest, parse_annotation_kind,
        projectional_editor_from_legacy_snapshot, projectional_editor_from_snapshot,
        reproduction_payload_from_coordinator, reproduction_payload_from_snapshot,
        snapshot_from_reproduction_payload, workspace_v8_digest,
    };

    fn restored_annotation_layout(
        snapshot: &WorkspaceSnapshot,
    ) -> (DocumentId, Vec<AnnotationLayoutEntry>) {
        let restored = coordinator_from_snapshot(snapshot).expect("restore workspace");
        (
            restored.session().design_document().id(),
            restored.editor().annotation_layout().entries(),
        )
    }

    fn reproduced_annotation_layout(payload: &str) -> Vec<AnnotationLayoutEntry> {
        coordinator_from_reproduction_payload(payload)
            .expect("restore reproduction")
            .editor()
            .annotation_layout()
            .entries()
    }

    fn m83_projectional_fixture() -> (WorkspaceSnapshot, IntentSession, SketchDocument) {
        let document = DocumentId(PersistentId::from_u128(0x8308_u128 << 64));
        let mut coordinator = ProjectionalIntentCoordinator::empty(
            IntentSessionId::from_raw(0x8308),
            ColdIntentMaterializer::with_default_policy(document, 1.0).expect("cold materializer"),
        )
        .expect("projectional coordinator");
        let primary = IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        };
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            IntentKey::new("ProjectionalPoint").expect("symbol"),
        )
        .with_instance_leaf(
            primary,
            LeafField::X,
            IntentLiteral::Quantity {
                value: 1.25,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            primary,
            LeafField::Y,
            IntentLiteral::Quantity {
                value: -2.5,
                unit: IntentUnit::Length,
            },
        );
        let patch = IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: IntentKey::new("point").expect("alias"),
                draft: Box::new(draft),
                cell: None,
            }],
        );
        coordinator.apply_patch(patch).expect("materialize point");
        let projectional = ProjectionalEditorSession::new(coordinator);
        let accepted = projectional
            .coordinator()
            .accepted_materialization()
            .expect("accepted materialization")
            .session
            .accepted_state_for_current_input()
            .expect("accepted state")
            .document()
            .clone();
        let intent = projectional.coordinator().intent().clone();
        let snapshot =
            WorkspaceSnapshot::from_projectional_editor(&projectional).expect("workspace v8");
        (snapshot, intent, accepted)
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyIntentCheckpoint {
        graph: IntentGraph,
        instance: IntentInstanceState,
        reservation_identity: IntentReservationLedgerIdentity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reservation_high_water: Option<ReservationId>,
        organization: IntentOrganization,
        external_inputs: IntentExternalInputs,
        latest_attempt: Option<IntentLatestAttempt>,
        accepted: Option<IntentAcceptedAuthority>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyIntentHistoryEntry {
        checkpoint: LegacyIntentCheckpoint,
        descriptor: IntentTransactionDescriptor,
        #[serde(rename = "before", default, skip_serializing)]
        _before: Option<ContentDigest>,
        #[serde(rename = "after", default, skip_serializing)]
        _after: Option<ContentDigest>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyIntentWire {
        version: u32,
        id: IntentSessionId,
        revision: Revision,
        current: LegacyIntentCheckpoint,
        undo: Vec<LegacyIntentHistoryEntry>,
        redo: Vec<LegacyIntentHistoryEntry>,
        reservations: IntentReservationLedger,
        allocator: IntentAllocatorHighWater,
        digest: ContentDigest,
    }

    fn legacy_graph_identity(graph: &IntentGraph) -> IntentGraphIdentity {
        let revision = graph.identity().0.revision;
        IntentGraphIdentity(ComponentIdentity {
            revision,
            digest: intent_legacy_content_digest(
                &serde_json::to_vec(&(revision, graph.nodes())).expect("legacy graph identity"),
            ),
        })
    }

    fn legacy_instance_identity(instance: &IntentInstanceState) -> IntentInstanceIdentity {
        let revision = instance.identity().0.revision;
        IntentInstanceIdentity(ComponentIdentity {
            revision,
            digest: intent_legacy_content_digest(
                &serde_json::to_vec(&(revision, instance.values()))
                    .expect("legacy instance identity"),
            ),
        })
    }

    fn legacy_reservation_identity(
        reservations: &IntentReservationLedger,
    ) -> IntentReservationLedgerIdentity {
        IntentReservationLedgerIdentity(ComponentIdentity {
            revision: reservations.revision(),
            digest: intent_legacy_content_digest(
                &serde_json::to_vec(&(reservations.revision(), reservations.entries()))
                    .expect("legacy reservation identity"),
            ),
        })
    }

    fn legacy_external_identity(external: &IntentExternalInputs) -> IntentExternalInputsIdentity {
        IntentExternalInputsIdentity {
            revision: external.revision,
            digest: intent_legacy_content_digest(
                &serde_json::to_vec(&(
                    external.revision,
                    &external.parameter_batch,
                    &external.external_snapshots,
                ))
                .expect("legacy external-input identity"),
            ),
        }
    }

    fn legacy_semantic_identity(
        graph: &IntentGraph,
        instance: &IntentInstanceState,
        reservations: IntentReservationLedgerIdentity,
        external: &IntentExternalInputs,
    ) -> IntentSemanticIdentity {
        IntentSemanticIdentity {
            graph: legacy_graph_identity(graph),
            instance: legacy_instance_identity(instance),
            reservations,
            external_inputs: legacy_external_identity(external),
        }
    }

    fn rewrite_checkpoint_as_legacy(checkpoint: &mut LegacyIntentCheckpoint) {
        let reservation_identity = if let Some(accepted) = &checkpoint.accepted {
            assert_eq!(
                accepted.reservations.identity(),
                checkpoint.reservation_identity,
                "fixture accepted checkpoints use their exact reservation ledger"
            );
            legacy_reservation_identity(&accepted.reservations)
        } else {
            let empty = IntentReservationLedger::empty();
            assert!(checkpoint.graph.nodes().is_empty());
            assert!(checkpoint.instance.values().is_empty());
            assert_eq!(checkpoint.reservation_identity, empty.identity());
            legacy_reservation_identity(&empty)
        };
        checkpoint.reservation_identity = reservation_identity;
        checkpoint.reservation_high_water = None;

        let accepted_digest = checkpoint.accepted.as_mut().map(|accepted| {
            let accepted_external = legacy_external_identity(&accepted.external_inputs);
            accepted.evidence.external_inputs = accepted_external;
            accepted.evidence.digest = intent_legacy_content_digest(
                &serde_json::to_vec(&(
                    accepted.evidence.external_inputs,
                    &accepted.evidence.materialization,
                    &accepted.evidence.ownership,
                    &accepted.evidence.host_validation,
                ))
                .expect("legacy materialization-evidence identity"),
            );
            accepted.target = legacy_semantic_identity(
                &accepted.graph,
                &accepted.instance,
                legacy_reservation_identity(&accepted.reservations),
                &accepted.external_inputs,
            );
            accepted.evidence.digest
        });

        if let Some(attempt) = &mut checkpoint.latest_attempt {
            attempt.target = legacy_semantic_identity(
                &checkpoint.graph,
                &checkpoint.instance,
                reservation_identity,
                &checkpoint.external_inputs,
            );
            if attempt.materialization_digest.is_some() {
                attempt.materialization_digest = accepted_digest;
            }
        }
    }

    fn rewrite_intent_session_as_legacy(json: &str) -> String {
        let mut wire: LegacyIntentWire =
            serde_json::from_str(json).expect("canonical intent-session wire");
        rewrite_checkpoint_as_legacy(&mut wire.current);
        for entry in wire.undo.iter_mut().chain(&mut wire.redo) {
            rewrite_checkpoint_as_legacy(&mut entry.checkpoint);
        }
        wire.version = 1;
        wire.digest = intent_legacy_content_digest(
            &serde_json::to_vec(&(
                wire.version,
                wire.id,
                wire.revision,
                &wire.current,
                &wire.undo,
                &wire.redo,
                &wire.reservations,
                wire.allocator,
            ))
            .expect("legacy intent-session digest"),
        );
        serde_json::to_string(&wire).expect("canonical legacy intent-session wire")
    }

    fn synthetic_intent_acceptance(
        candidate: &geosolve_sketch_intent::IntentCandidate,
    ) -> IntentEvaluation {
        IntentEvaluation::Accepted {
            evidence: geosolve_sketch_intent::MaterializationEvidence::new_host_artifacts(
                candidate.external_inputs().identity(),
                format!("synthetic:{:?}", candidate.semantic_identity()).into_bytes(),
                b"synthetic-ownership".to_vec(),
                b"synthetic-validation".to_vec(),
            )
            .expect("bounded synthetic evidence"),
        }
    }

    fn run_m83_persistence_test(name: &str, test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .name(name.to_owned())
            .stack_size(32 * 1024 * 1024)
            .spawn(test)
            .expect("spawn projectional persistence test")
            .join()
            .expect("projectional persistence test thread");
    }

    #[test]
    fn m80_profile_offset_annotation_cache_identity_round_trips() {
        let key = annotation_kind_key(SceneAnnotationKind::ProfileOffset);
        assert_eq!(key, "dimension:profile-offset");
        assert_eq!(
            parse_annotation_kind(key),
            Some(SceneAnnotationKind::ProfileOffset)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn m80_profile_offset_draft_workspace_and_reproduction_round_trip_exactly() {
        let mut document = SketchDocument::new(4.0).expect("document");
        let source_points = [
            document.add_point("source start", [0.0, 0.0]).unwrap(),
            document.add_point("source end", [4.0, 0.0]).unwrap(),
        ];
        let target_points = [
            document.add_point("target start", [0.0, 1.0]).unwrap(),
            document.add_point("target end", [4.0, 1.0]).unwrap(),
        ];
        let add_line = |document: &mut SketchDocument, label: &str, points: [DesignPointId; 2]| {
            document
                .add_curve(
                    label,
                    CurveDefinition::Line {
                        start: points[0],
                        end: points[1],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap()
        };
        let source = CurveSpan::line(add_line(&mut document, "source", source_points));
        let target = CurveSpan::line(add_line(&mut document, "target", target_points));
        let operand = geosolve_sketch::DocumentProfileOffsetOperand::OpenChain {
            side: geosolve_sketch::DocumentLineSide::Left,
            chain: geosolve_sketch::DocumentProfileOffsetChain {
                edges: vec![geosolve_sketch::DocumentProfileOffsetEdgePair {
                    source: geosolve_sketch::DocumentDirectedProfileOffsetCurve {
                        curve: source,
                        traversal: geosolve_sketch::DocumentOffsetTraversal::Forward,
                    },
                    target: geosolve_sketch::DocumentDirectedProfileOffsetCurve {
                        curve: target,
                        traversal: geosolve_sketch::DocumentOffsetTraversal::Forward,
                    },
                }],
                junctions: Vec::new(),
                start_terminal:
                    geosolve_sketch::DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                end_terminal:
                    geosolve_sketch::DocumentProfileOffsetTerminalPolicy::NormalTranslation,
            },
        };
        let ids = document
            .add_profile_offset("one-line offset", 1.0, operand.clone())
            .expect("profile offset");
        let document_id = document.id();
        let dimension_source = document
            .dimension(ids.dimension)
            .expect("Profile Offset dimension")
            .source_id;
        let draft = document.to_draft_v5_json().expect("M80 draft-v5");
        assert!(matches!(
            document.to_canonical_json(),
            Err(DocumentError::UnsupportedM80State)
        ));
        let mut coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("accepted M80 session"),
        )
        .expect("M80 coordinator");

        let annotation_entry = AnnotationLayoutEntry {
            key: AnnotationLayoutKey {
                document: document_id,
                source: dimension_source,
                item: SelectionItem::Dimension(ids.dimension),
                kind: SceneAnnotationKind::ProfileOffset,
                marker_index: None,
            },
            placement: AnnotationPlacement::Linear {
                perpendicular_pixels: 47.0,
            },
        };
        coordinator
            .editor_mut()
            .restore_annotation_layout(AnnotationLayoutState::from_entries([annotation_entry]));

        let workspace = WorkspaceSnapshot::from_coordinator(&coordinator).expect("workspace");
        assert_eq!(
            workspace.design.encoding,
            super::WorkspaceDocumentEncoding::DraftV5
        );
        assert_eq!(workspace.design.json, draft);
        let decoded = WorkspaceSnapshot::decode(&workspace.encode().expect("encode workspace"))
            .expect("decode workspace");
        let restored = coordinator_from_snapshot(&decoded).expect("restore workspace");
        assert_eq!(
            restored.editor().annotation_layout().entries(),
            vec![annotation_entry],
            "ordinary workspace restore must retain a compatible Profile Offset placement",
        );
        assert_eq!(
            restored
                .session()
                .design_document()
                .to_draft_v5_json()
                .expect("restored draft"),
            draft
        );
        assert_eq!(
            restored
                .session()
                .design_document()
                .dimension(ids.dimension)
                .map(|dimension| dimension.definition.clone()),
            Some(
                geosolve_sketch::DocumentDimensionDefinition::ProfileOffset {
                    target: ids.target,
                    operand: operand.clone(),
                }
            )
        );

        let payload = reproduction_payload_from_coordinator(&coordinator).expect("reproduction");
        let reproduction_workspace =
            crate::reproduction::decode_workspace(&payload).expect("decode reproduction");
        assert!(
            WorkspaceSnapshot::decode(&reproduction_workspace)
                .expect("decode reproduction workspace")
                .annotation_layout_json
                .is_none(),
            "reproduction export must omit disposable Profile Offset placement",
        );
        let reproduced =
            coordinator_from_reproduction_payload(&payload).expect("restore reproduction");
        assert!(
            reproduced.editor().annotation_layout().entries().is_empty(),
            "reproduction restore must recompute Profile Offset placement",
        );
        assert_eq!(
            reproduced
                .session()
                .design_document()
                .to_draft_v5_json()
                .expect("reproduced draft"),
            draft
        );
        assert_eq!(
            reproduced
                .session()
                .design_document()
                .dimension(ids.dimension)
                .map(|dimension| dimension.definition.clone()),
            Some(
                geosolve_sketch::DocumentDimensionDefinition::ProfileOffset {
                    target: ids.target,
                    operand,
                }
            )
        );

        let legacy_payload = crate::reproduction::encode_workspace(
            &workspace
                .encode()
                .expect("workspace carrying Profile Offset placement"),
        )
        .expect("legacy reproduction with presentation cache");
        assert!(
            coordinator_from_reproduction_payload(&legacy_payload)
                .expect("restore legacy reproduction")
                .editor()
                .annotation_layout()
                .entries()
                .is_empty(),
            "reproduction import must ignore a carried Profile Offset placement",
        );
    }

    fn computed_fillet_candidate(
        coordinator: &RetainedEditorCoordinator,
        corner: DesignPointId,
    ) -> FeatureAuthoringCandidate {
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("feature-authoring snapshot");
        let document = snapshot.sketch_document();
        let mut authoring = FeatureAuthoringState::default();
        match authoring.activate(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Point(corner), None)],
        ) {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
            other => panic!("expected computed Fillet candidate, got {other:?}"),
        }
    }

    fn apply_computed_fillet(
        coordinator: &mut RetainedEditorCoordinator,
        corner: DesignPointId,
        label: &str,
    ) -> geosolve_constraint_editor::ComputedFeatureId {
        let candidate = computed_fillet_candidate(coordinator, corner);
        let preview = coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                label,
            )
            .expect("computed Fillet preview");
        coordinator
            .apply_feature_authoring_preview(preview.token, &candidate)
            .expect("computed Fillet publication")
            .value
    }

    fn clamped_bspline_document() -> (SketchDocument, CurveId) {
        let mut document = SketchDocument::new(1.0).expect("document");
        let controls = [[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [3.0, 1.5], [4.0, 0.0]]
            .map(|position| {
                document
                    .add_point("clamped control", position)
                    .expect("control")
            })
            .to_vec();
        let curve = document
            .add_curve(
                "clamped cubic",
                CurveDefinition::BSpline {
                    form: DocumentBSplineForm::Clamped,
                    degree: 3,
                    controls,
                    knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
                    span_ids: vec![41, 73],
                    next_span_id: 100,
                },
            )
            .expect("B-spline");
        (document, curve)
    }

    #[test]
    fn checkpoint_codec_round_trips_design_accepted_and_revisions() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(8.0).unwrap(),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).unwrap();
        let decoded = WorkspaceSnapshot::decode(&snapshot.encode().unwrap()).unwrap();
        assert_eq!(snapshot.version, 6);
        assert!(snapshot.accepted_belongs_to_current_design);
        assert_eq!(
            decoded.accepted_belongs_to_current_design,
            snapshot.accepted_belongs_to_current_design
        );
        assert_eq!(decoded.design, snapshot.design);
        assert_eq!(decoded.accepted, snapshot.accepted);
        assert_eq!(
            decoded.sketch_identity_high_water,
            snapshot.sketch_identity_high_water
        );
        assert_eq!(decoded.features_json, snapshot.features_json);
        let mut v5_value = serde_json::to_value(&snapshot).expect("workspace v5 value");
        v5_value["version"] = serde_json::Value::from(5);
        v5_value
            .as_object_mut()
            .expect("workspace object")
            .remove("annotation_layout_json");
        let migrated_v5 = WorkspaceSnapshot::decode(
            &serde_json::to_string(&v5_value).expect("workspace v5 JSON"),
        )
        .expect("migrate workspace v5");
        assert_eq!(migrated_v5.version, 6);
        assert!(migrated_v5.annotation_layout().entries().is_empty());
        assert_eq!(
            decoded.feature_lifecycle_high_water,
            snapshot.feature_lifecycle_high_water
        );
        assert_eq!(
            decoded.computed_evaluation_high_water,
            snapshot.computed_evaluation_high_water
        );
        assert_eq!(decoded.revisions().design(), snapshot.revisions().design());
        assert_eq!(
            decoded.revisions().attempt(),
            snapshot.revisions().attempt()
        );
        assert_eq!(
            decoded.revisions().accepted(),
            snapshot.revisions().accepted()
        );
    }

    #[test]
    fn workspace_decode_rejects_above_public_workspace_limit_before_json_parsing() {
        let hostile = "x".repeat(crate::reproduction::MAX_REPRODUCTION_WORKSPACE_BYTES + 1);
        assert_eq!(
            WorkspaceSnapshot::decode(&hostile).unwrap_err(),
            format!(
                "workbench snapshot exceeds the {}-byte limit",
                crate::reproduction::MAX_REPRODUCTION_WORKSPACE_BYTES,
            ),
        );
    }

    #[test]
    fn disposable_outer_annotation_cache_ignores_wide_non_string_input() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(8.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).expect("workspace");
        let expected_design = snapshot.design_document().expect("design");
        let mut wire = serde_json::to_value(snapshot).expect("workspace wire");
        wire["annotation_layout_json"] = serde_json::Value::Array(
            (0..16_384)
                .map(|index| {
                    serde_json::json!({
                        "hostile": index,
                        "payload": "annotation cache rows are disposable presentation data",
                    })
                })
                .collect(),
        );
        let encoded = serde_json::to_string(&wire).expect("wide workspace wire");

        let decoded = WorkspaceSnapshot::decode(&encoded)
            .expect("non-string disposable cache must not reject semantic workspace state");
        assert_eq!(
            decoded.design_document().expect("decoded design"),
            expected_design
        );
        assert!(decoded.annotation_layout_json.is_none());
        assert!(decoded.annotation_layout().entries().is_empty());

        let mut oversized =
            WorkspaceSnapshot::from_coordinator(&coordinator).expect("oversized-cache workspace");
        oversized.annotation_layout_json =
            Some("x".repeat(super::MAX_ANNOTATION_LAYOUT_JSON_BYTES + 1));
        let encoded = serde_json::to_string(&oversized).expect("oversized-cache workspace wire");
        let decoded = WorkspaceSnapshot::decode(&encoded)
            .expect("oversized disposable cache must not reject semantic workspace state");
        assert_eq!(
            decoded.design_document().expect("decoded design"),
            expected_design
        );
        assert!(decoded.annotation_layout_json.is_none());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one workspace-v6 matrix keeps round-trip and row-local corruption recovery auditable together"
    )]
    fn m76_workspace_v6_round_trips_layout_and_ignores_corrupt_cache() {
        let mut design = SketchDocument::new(8.0).unwrap();
        let point = design.add_point("editable point", [0.0, 0.0]).unwrap();
        let constraint = design
            .add_constraint(
                "fixed point",
                DocumentConstraintDefinition::FixedPoint {
                    point,
                    target: [0.0, 0.0],
                },
            )
            .unwrap();
        let source = design.constraint(constraint).unwrap().source_id;
        let session = RetainedSketchDocumentSession::new(
            design,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let document = session.design_document().id();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let empty_layout_snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).unwrap();
        assert!(empty_layout_snapshot.annotation_layout_json.is_none());
        assert!(
            !empty_layout_snapshot
                .encode()
                .unwrap()
                .contains("annotation_layout_json")
        );
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted layout fixture");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("layout scene");
        let marker = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(constraint))
            .and_then(|annotation| match &annotation.geometry {
                SceneAnnotationGeometry::Glyph { markers } => markers.first(),
                _ => None,
            })
            .expect("fixed marker")
            .anchor;
        let pointer = |pointer_id, position: ScreenPoint| PointerInput {
            pointer_id,
            position,
            modifiers: Modifiers::default(),
        };
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Point(point)]);
        coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(76, marker));
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(76, marker));
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(
                76,
                ScreenPoint {
                    x: marker.x + 12.0,
                    y: marker.y - 7.0,
                },
            ),
        );
        assert_eq!(
            coordinator
                .editor()
                .annotation_layout_for_scene()
                .entries()
                .len(),
            1,
        );
        assert!(
            coordinator
                .editor()
                .annotation_layout()
                .entries()
                .is_empty()
        );
        assert!(
            WorkspaceSnapshot::from_coordinator(&coordinator)
                .unwrap()
                .annotation_layout_json
                .is_none(),
            "autosave during a drag must exclude its cancellable preview",
        );
        coordinator.editor_mut().cancel();
        let entry = AnnotationLayoutEntry {
            key: AnnotationLayoutKey {
                document,
                source,
                item: SelectionItem::Constraint(constraint),
                kind: SceneAnnotationKind::Constraint(SceneConstraintGlyph::Fixed),
                marker_index: Some(0),
            },
            placement: AnnotationPlacement::Free {
                offset_pixels: [18.0, -9.0],
            },
        };
        let design_before_layout = coordinator.session().design_identity();
        let attempt_before_layout = coordinator.session().last_attempt().identity();
        let accepted_before_layout = coordinator
            .session()
            .accepted_state_for_current_input()
            .unwrap()
            .identity();
        coordinator
            .editor_mut()
            .restore_annotation_layout(AnnotationLayoutState::from_entries([entry]));
        assert_eq!(
            coordinator.session().design_identity(),
            design_before_layout
        );
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            attempt_before_layout
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .unwrap()
                .identity(),
            accepted_before_layout
        );
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point,
                    position: [1.0, 2.0],
                },
            )
            .unwrap();
        assert_eq!(
            coordinator.editor().annotation_layout().entries(),
            vec![entry]
        );
        coordinator.undo().unwrap();
        assert_eq!(
            coordinator.editor().annotation_layout().entries(),
            vec![entry]
        );
        coordinator.redo().unwrap();
        assert_eq!(
            coordinator.editor().annotation_layout().entries(),
            vec![entry]
        );
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Constraint(constraint)]);
        coordinator
            .delete_selected(coordinator.session().design_identity())
            .unwrap();
        assert_eq!(
            coordinator.editor().annotation_layout().entries(),
            vec![entry]
        );
        coordinator.undo().unwrap();
        assert_eq!(
            coordinator.editor().annotation_layout().entries(),
            vec![entry]
        );
        coordinator.redo().unwrap();
        assert_eq!(
            coordinator.editor().annotation_layout().entries(),
            vec![entry]
        );
        coordinator.undo().unwrap();
        assert!(
            coordinator
                .session()
                .design_document()
                .constraint(constraint)
                .is_some(),
            "the round-trip fixture must retain a live semantic owner",
        );

        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).unwrap();
        assert_eq!(snapshot.version, 6);
        let decoded = WorkspaceSnapshot::decode(&snapshot.encode().unwrap()).unwrap();
        let (restored_document, restored_entries) = restored_annotation_layout(&decoded);
        assert_eq!(restored_document, document);
        assert_eq!(restored_entries, vec![entry]);

        let reproduction = reproduction_payload_from_coordinator(&coordinator).unwrap();
        let reproduction_workspace =
            crate::reproduction::decode_workspace(&reproduction).expect("reproduction workspace");
        let reproduction_snapshot =
            WorkspaceSnapshot::decode(&reproduction_workspace).expect("reproduction snapshot");
        assert!(
            reproduction_snapshot.annotation_layout_json.is_none(),
            "a reproduction capsule must omit disposable annotation placement",
        );
        assert_eq!(
            reproduced_annotation_layout(&reproduction),
            Vec::new(),
            "reproduction restore must recompute disposable annotation placement",
        );

        let legacy_reproduction = crate::reproduction::encode_workspace(
            &snapshot
                .encode()
                .expect("workspace with a presentation cache"),
        )
        .expect("legacy reproduction");
        assert!(
            reproduced_annotation_layout(&legacy_reproduction).is_empty(),
            "import must ignore a presentation cache carried by an older reproduction capsule",
        );

        let mut mixed_rows = snapshot.clone();
        let mut mixed_cache: serde_json::Value = serde_json::from_str(
            mixed_rows
                .annotation_layout_json
                .as_deref()
                .expect("encoded layout cache"),
        )
        .unwrap();
        mixed_cache["entries"]
            .as_array_mut()
            .expect("layout rows")
            .push(serde_json::json!({"malformed": true}));
        mixed_rows.annotation_layout_json = Some(serde_json::to_string(&mixed_cache).unwrap());
        let decoded_mixed = WorkspaceSnapshot::decode(&mixed_rows.encode().unwrap()).unwrap();
        let (_, restored_entries) = restored_annotation_layout(&decoded_mixed);
        assert_eq!(
            restored_entries,
            vec![entry],
            "one malformed row must not discard independent valid placement",
        );

        let corrupt_row_restores_empty = |field: &str, value: serde_json::Value| {
            let mut corrupted = snapshot.clone();
            let mut cache: serde_json::Value = serde_json::from_str(
                corrupted
                    .annotation_layout_json
                    .as_deref()
                    .expect("encoded layout cache"),
            )
            .unwrap();
            cache["entries"][0][field] = value;
            corrupted.annotation_layout_json = Some(serde_json::to_string(&cache).unwrap());
            let decoded = WorkspaceSnapshot::decode(&corrupted.encode().unwrap()).unwrap();
            let (_, restored_entries) = restored_annotation_layout(&decoded);
            assert!(
                restored_entries.is_empty(),
                "corrupt annotation field {field} must be discarded independently",
            );
        };
        corrupt_row_restores_empty(
            "item_id",
            serde_json::Value::String(PersistentId::from_u128(0x76_01).to_string()),
        );
        corrupt_row_restores_empty(
            "source",
            serde_json::Value::String(PersistentId::from_u128(0x76_02).to_string()),
        );
        corrupt_row_restores_empty(
            "annotation_kind",
            serde_json::Value::String("constraint:horizontal".into()),
        );
        corrupt_row_restores_empty("marker_index", serde_json::Value::from(99));
        corrupt_row_restores_empty(
            "placement",
            serde_json::json!({"form":"linear","perpendicular_pixels":24.0}),
        );

        let mut wrong_outer: serde_json::Value =
            serde_json::from_str(&snapshot.encode().unwrap()).unwrap();
        wrong_outer["annotation_layout_json"] = serde_json::json!({
            "version": AnnotationLayoutState::VERSION,
            "entries": [],
        });
        let decoded = WorkspaceSnapshot::decode(&serde_json::to_string(&wrong_outer).unwrap())
            .expect("a disposable cache with the wrong outer JSON type cannot reject the sketch");
        let (restored_document, restored_entries) = restored_annotation_layout(&decoded);
        assert!(restored_entries.is_empty());
        assert_eq!(restored_document, document);

        let mut stale_document = snapshot.clone();
        let mut stale_cache: serde_json::Value = serde_json::from_str(
            stale_document
                .annotation_layout_json
                .as_deref()
                .expect("encoded layout cache"),
        )
        .unwrap();
        stale_cache["entries"][0]["document"] =
            serde_json::Value::String(PersistentId::from_u128(0x7600).to_string());
        stale_document.annotation_layout_json = Some(serde_json::to_string(&stale_cache).unwrap());
        let decoded_stale = WorkspaceSnapshot::decode(&stale_document.encode().unwrap()).unwrap();
        let (_, restored_entries) = restored_annotation_layout(&decoded_stale);
        assert!(restored_entries.is_empty());

        let mut corrupt = snapshot.clone();
        corrupt.annotation_layout_json = Some("{not valid layout json".into());
        let decoded = WorkspaceSnapshot::decode(&corrupt.encode().unwrap()).unwrap();
        let (restored_document, restored_entries) = restored_annotation_layout(&decoded);
        assert!(restored_entries.is_empty());
        assert_eq!(restored_document, document);

        let mut incompatible = snapshot;
        incompatible.annotation_layout_json = Some(r#"{"version":999,"entries":[]}"#.into());
        let decoded_incompatible =
            WorkspaceSnapshot::decode(&incompatible.encode().unwrap()).unwrap();
        let (_, restored_entries) = restored_annotation_layout(&decoded_incompatible);
        assert!(restored_entries.is_empty());
    }

    #[test]
    fn reproduction_restore_is_atomic_and_keeps_workspace_validation_authoritative() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(8.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let retained = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("retained workspace")
            .encode()
            .expect("retained workspace JSON");
        let payload = reproduction_payload_from_coordinator(&coordinator)
            .expect("valid reproduction payload");
        assert_eq!(
            reproduction_payload_from_coordinator(&coordinator)
                .expect("repeat valid reproduction payload"),
            payload,
            "copying an unchanged workspace must be byte-stable"
        );

        let mut corrupt_fields = payload.split(':').map(str::to_owned).collect::<Vec<_>>();
        corrupt_fields[3] = "0000000000000000".into();
        let corrupt = corrupt_fields.join(":");
        assert!(
            coordinator_from_reproduction_payload(&corrupt)
                .unwrap_err()
                .contains("checksum mismatch")
        );
        assert_eq!(
            WorkspaceSnapshot::from_coordinator(&coordinator)
                .expect("workspace after corrupt payload")
                .encode()
                .expect("workspace JSON after corrupt payload"),
            retained
        );

        let mut invalid_workspace: serde_json::Value =
            serde_json::from_str(&retained).expect("workspace value");
        invalid_workspace["computed_evaluation_high_water"]["next_revision"] =
            serde_json::Value::from(0);
        let invalid_payload = crate::reproduction::encode_workspace(
            &serde_json::to_string(&invalid_workspace).expect("invalid workspace JSON"),
        )
        .expect("transport structurally invalid workspace");
        assert!(
            coordinator_from_reproduction_payload(&invalid_payload)
                .unwrap_err()
                .contains("must be nonzero")
        );
        assert_eq!(
            WorkspaceSnapshot::from_coordinator(&coordinator)
                .expect("workspace after invalid restore")
                .encode()
                .expect("workspace JSON after invalid restore"),
            retained
        );

        let restored = coordinator_from_reproduction_payload(&payload)
            .expect("restore valid reproduction payload");
        assert_eq!(
            restored.session().design_document(),
            coordinator.session().design_document()
        );
        assert_eq!(
            restored
                .session()
                .accepted_state()
                .map(geosolve_sketch::SketchAcceptedDocumentState::document),
            coordinator
                .session()
                .accepted_state()
                .map(geosolve_sketch::SketchAcceptedDocumentState::document)
        );
        assert_eq!(
            restored.feature_document().id(),
            coordinator.feature_document().id()
        );
        assert_eq!(
            restored.feature_document().sketch_document(),
            coordinator.feature_document().sketch_document()
        );
        assert_eq!(
            restored.feature_document().features(),
            coordinator.feature_document().features()
        );
        assert_eq!(
            restored.feature_document().allocator_high_water(),
            coordinator.feature_document().allocator_high_water()
        );
        assert_eq!(
            restored.session().persistent_identity_high_water(),
            coordinator.session().persistent_identity_high_water()
        );
    }

    #[test]
    fn m70b_f005_supplied_payload_restores_exact_current_fillet_through_ordinary_decoder() {
        const PAYLOAD: &str = include_str!("../../tests/fixtures/m70b_f005_repro.txt");
        let payload = PAYLOAD.trim_end();
        let workspace = crate::reproduction::decode_workspace(payload)
            .expect("exact F005 reproduction transport");
        assert_eq!(workspace.len(), 4_228);
        let snapshot =
            WorkspaceSnapshot::decode(&workspace).expect("exact F005 application workspace");
        assert!(snapshot.accepted_belongs_to_current_design);

        let coordinator = coordinator_from_reproduction_payload(payload)
            .expect("ordinary F005 coordinator restoration");
        assert_eq!(coordinator.session().design_document().points().len(), 3);
        assert_eq!(coordinator.session().design_document().curves().len(), 2);
        assert_eq!(coordinator.feature_document().features().len(), 1);
        let ComputedSceneState::Current { snapshot, .. } = coordinator.computed_scene_state()
        else {
            panic!("F005 payload must restore one authoritative current computed scene");
        };
        assert!(matches!(
            snapshot.feature_evaluations(),
            [evaluation]
                if matches!(evaluation.state, ComputedFeatureEvaluationState::Current { .. })
        ));
        assert_eq!(
            snapshot
                .edges()
                .iter()
                .filter(|edge| matches!(edge.geometry, ComputedEdgeGeometry::CircularArc(_)))
                .count(),
            1
        );
        assert_eq!(
            payload.split(':').nth(3),
            Some("0823d31f269300af"),
            "the checked-in fixture must retain the supplied checksum identity"
        );
    }

    #[test]
    fn reproduction_restore_rejects_coordinator_reconstruction_failure_atomically() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(8.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let retained = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("retained workspace")
            .encode()
            .expect("retained workspace JSON");
        let mut reconstruction_failure: serde_json::Value =
            serde_json::from_str(&retained).expect("workspace value");
        reconstruction_failure["feature_lifecycle_high_water"]["revision"] =
            serde_json::Value::from(u64::MAX);
        let payload = crate::reproduction::encode_workspace(
            &serde_json::to_string(&reconstruction_failure)
                .expect("coordinator-invalid workspace JSON"),
        )
        .expect("transport coordinator-invalid workspace");
        let workspace = crate::reproduction::decode_workspace(&payload)
            .expect("decode coordinator-invalid workspace transport");
        let snapshot = WorkspaceSnapshot::decode(&workspace)
            .expect("workspace validation precedes coordinator reconstruction");
        assert!(
            coordinator_from_snapshot(&snapshot)
                .unwrap_err()
                .contains("exhausted"),
            "coordinator reconstruction must reject an exhausted feature lifecycle revision"
        );
        assert!(
            coordinator_from_reproduction_payload(&payload)
                .unwrap_err()
                .contains("exhausted"),
            "the complete payload path must propagate coordinator reconstruction failure"
        );
        assert_eq!(
            WorkspaceSnapshot::from_coordinator(&coordinator)
                .expect("workspace after coordinator reconstruction failure")
                .encode()
                .expect("workspace JSON after coordinator reconstruction failure"),
            retained
        );
    }

    #[test]
    fn workspace_v5_round_trips_persistent_construction_role() {
        let mut document = SketchDocument::new(4.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("point");
        let end = document.add_point("end", [2.0, 0.0]).expect("point");
        let guide = document
            .add_curve_with_role(
                "guide",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
                GeometryRole::Construction,
            )
            .expect("Construction curve");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).expect("workspace v5");
        assert_eq!(snapshot.version, 6);
        let decoded =
            WorkspaceSnapshot::decode(&snapshot.encode().expect("encode")).expect("decode");
        let restored = decoded
            .restore_session(DocumentSolveRequest::default(), SolverConfig::default())
            .expect("restore");
        assert_eq!(
            restored.design_document().geometry_role(guide),
            Some(GeometryRole::Construction)
        );
        assert_eq!(
            restored
                .accepted_state_for_current_input()
                .expect("accepted")
                .document()
                .geometry_role(guide),
            Some(GeometryRole::Construction)
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one persistence oracle binds stable computed intent to regenerated revision-local output"
    )]
    fn workspace_v5_round_trips_multiple_computed_sets_and_regenerates_output_ids() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [
            document.add_point("p0", [0.0, 0.0]).expect("p0"),
            document.add_point("p1", [4.0, 0.0]).expect("p1"),
            document.add_point("p2", [4.0, 4.0]).expect("p2"),
            document.add_point("p3", [8.0, 4.0]).expect("p3"),
        ];
        document
            .add_curve(
                "three-span polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
                },
            )
            .expect("polyline");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let first = apply_computed_fillet(&mut coordinator, points[1], "Left Fillet");
        let second = apply_computed_fillet(&mut coordinator, points[2], "Right Fillet");
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(second)]);
        coordinator
            .set_selected_suppressed(coordinator.session().design_identity(), true)
            .expect("suppress second set");

        let features_before = coordinator.feature_document().features().to_vec();
        assert_eq!(features_before.len(), 2);
        assert_eq!(features_before[0].id, first);
        assert_eq!(features_before[0].label, "Left Fillet");
        assert!(!features_before[0].suppressed);
        assert_eq!(features_before[1].id, second);
        assert_eq!(features_before[1].label, "Right Fillet");
        assert!(features_before[1].suppressed);
        let allocator_before = coordinator.feature_document().allocator_high_water();
        let old_edges = coordinator
            .computed_snapshot()
            .expect("current computed output")
            .edges()
            .to_vec();
        assert!(!old_edges.is_empty());
        let old_output = old_edges
            .iter()
            .map(|edge| (edge.geometry.clone(), edge.provenance.clone()))
            .collect::<Vec<_>>();

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("capture workspace v5")
            .encode()
            .expect("encode workspace v5");
        let decoded = WorkspaceSnapshot::decode(&encoded).expect("decode workspace v5");
        let decoded_features = decoded.feature_document().expect("feature sidecar");
        assert_eq!(decoded_features.features(), features_before.as_slice());
        assert_eq!(decoded_features.allocator_high_water(), allocator_before);
        let restored_session = decoded
            .restore_session(DocumentSolveRequest::default(), SolverConfig::default())
            .expect("restore sketch session");
        let restored_features = decoded.feature_document().expect("restored sidecar");
        let restored = RetainedEditorCoordinator::with_features_and_high_water(
            restored_session,
            restored_features,
            decoded.feature_lifecycle_high_water(),
            decoded.computed_evaluation_high_water(),
        )
        .expect("restore composite coordinator");

        assert_eq!(
            restored.feature_document().features(),
            features_before.as_slice()
        );
        assert!(
            restored
                .feature_document()
                .allocator_high_water()
                .next_feature_id
                >= allocator_before.next_feature_id
        );
        assert!(
            restored
                .feature_document()
                .allocator_high_water()
                .next_corner_id
                >= allocator_before.next_corner_id
        );
        let regenerated = restored
            .computed_snapshot()
            .expect("regenerated computed output")
            .edges();
        assert_eq!(
            regenerated
                .iter()
                .map(|edge| (edge.geometry.clone(), edge.provenance.clone()))
                .collect::<Vec<_>>(),
            old_output
        );
        assert!(
            old_edges
                .iter()
                .all(|old| { regenerated.iter().all(|current| current.id != old.id) })
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one real save/reload sequence covers history and transient evaluation high-water"
    )]
    fn save_after_undo_and_cancelled_preview_preserves_all_live_high_water() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [
            document.add_point("p0", [0.0, 0.0]).expect("p0"),
            document.add_point("p1", [4.0, 0.0]).expect("p1"),
            document.add_point("p2", [4.0, 4.0]).expect("p2"),
            document.add_point("p3", [8.0, 4.0]).expect("p3"),
        ];
        document
            .add_curve(
                "three-span polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
                },
            )
            .expect("polyline");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let first = apply_computed_fillet(&mut coordinator, points[1], "first");
        let removed_by_undo = apply_computed_fillet(&mut coordinator, points[2], "second");

        coordinator.undo().expect("undo second feature");
        assert!(coordinator.feature_document().feature(first).is_some());
        assert!(
            coordinator
                .feature_document()
                .feature(removed_by_undo)
                .is_none()
        );
        assert!(
            coordinator
                .feature_document()
                .lifecycle_high_water()
                .allocator
                .next_feature_id
                .raw()
                > removed_by_undo.raw()
        );

        let cancelled_candidate = computed_fillet_candidate(&coordinator, points[2]);
        coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &cancelled_candidate,
                "cancelled preview",
            )
            .expect("transient preview");
        let cancelled_evaluation = coordinator
            .feature_authoring_preview()
            .expect("held transient preview")
            .snapshot()
            .evaluation_revision();
        let cancelled_edges = coordinator
            .feature_authoring_preview()
            .expect("held transient preview")
            .snapshot()
            .edges()
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        coordinator.clear_feature_authoring_preview();
        let live_sketch_high_water = coordinator.session().revision_high_water();

        let payload = reproduction_payload_from_coordinator(&coordinator)
            .expect("encode live reproduction payload");
        let encoded = crate::reproduction::decode_workspace(&payload)
            .expect("decode exact workspace JSON from reproduction payload");
        let decoded = WorkspaceSnapshot::decode(&encoded).expect("decode live workspace");
        assert_eq!(decoded.revisions(), live_sketch_high_water);
        assert!(
            decoded
                .feature_lifecycle_high_water()
                .allocator
                .next_feature_id
                .raw()
                > removed_by_undo.raw()
        );
        assert!(
            decoded.computed_evaluation_high_water().next_revision.raw()
                > cancelled_evaluation.raw()
        );

        let mut restored = coordinator_from_reproduction_payload(&payload)
            .expect("restore complete reproduction payload");
        let regenerated = restored
            .computed_snapshot()
            .expect("regenerated computed output");
        assert!(regenerated.evaluation_revision().raw() > cancelled_evaluation.raw());
        assert!(
            cancelled_edges
                .iter()
                .all(|old| regenerated.edge(*old).is_none())
        );

        let replacement = apply_computed_fillet(&mut restored, points[2], "replacement");
        assert!(replacement.raw() > removed_by_undo.raw());
    }

    #[test]
    fn process_reload_retains_an_undone_spline_cursor_after_the_curve_is_deleted() {
        let (document, curve) = clamped_bspline_document();
        let original_curve_graph = document.clone();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let insertion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::InsertBSplineKnot {
                    curve,
                    parameter: 0.25,
                },
            )
            .expect("first insertion");
        let DocumentCommandEffect::InsertedBSplineKnot(insertion) = insertion.value else {
            panic!("expected B-spline insertion");
        };
        assert_eq!(insertion.new_span_id, Some(100));

        coordinator.undo().expect("undo insertion");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Curve(curve),
                },
            )
            .expect("divergently delete curve");
        assert!(
            coordinator
                .session()
                .design_document()
                .curve(curve)
                .is_none()
        );
        assert!(!coordinator.can_redo());
        let retained_before_reload = coordinator
            .session()
            .persistent_identity_high_water()
            .clone();

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("capture workspace v5")
            .encode()
            .expect("encode workspace v5");
        let decoded = WorkspaceSnapshot::decode(&encoded).expect("decode workspace v5");
        let restored = decoded
            .restore_session(DocumentSolveRequest::default(), SolverConfig::default())
            .expect("restore sketch session");
        assert_eq!(
            restored.persistent_identity_high_water(),
            &retained_before_reload
        );
        assert!(restored.design_document().curve(curve).is_none());

        let mut reintroduced_curve_graph = original_curve_graph;
        reintroduced_curve_graph
            .retain_persistent_identity_high_water(restored.persistent_identity_high_water())
            .expect("merge process-restored high-water");
        let divergent = reintroduced_curve_graph
            .insert_bspline_knot(curve, 0.75)
            .expect("divergent insertion after process reload");
        assert_eq!(divergent.new_span_id, Some(101));
    }

    #[test]
    fn v5_current_design_provenance_restores_flexible_fillet_bytes_exactly() {
        let fixture = alpha_scenario(AlphaScenarioKind::FilletLineCircle, 1.0)
            .expect("line-circle fillet fixture");
        let AlphaScenarioIds::FilletLineCircle(ids) = fixture.ids else {
            panic!("line-circle fillet IDs expected")
        };
        let session = RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .expect("fillet session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let deletion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::Delete {
                    object: DocumentObjectId::Dimension(ids.fillet.radius_dimension),
                },
            )
            .expect("delete fillet driving radius");
        assert!(deletion.published_accepted.is_some());
        let initial_center = coordinator
            .session()
            .accepted_state()
            .expect("accepted flexible fillet")
            .document()
            .point(ids.fillet.center)
            .expect("fillet center")
            .position;
        let moved = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: ids.fillet.center,
                    position: [initial_center[0] + 0.2, initial_center[1] + 0.15],
                },
            )
            .expect("move flexible fillet center");
        assert!(moved.published_accepted.is_some());

        let design_before = coordinator.session().design_document().clone();
        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted moved fillet")
            .document()
            .clone();
        assert_ne!(
            design_before, accepted_before,
            "the regression requires distinct retained seeds and solved materialization"
        );
        let accepted_json_before = accepted_before
            .to_canonical_json()
            .expect("accepted canonical bytes");

        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).expect("capture v5");
        assert!(snapshot.accepted_belongs_to_current_design);
        let decoded =
            WorkspaceSnapshot::decode(&snapshot.encode().expect("encode v5")).expect("decode v5");
        assert!(decoded.accepted_belongs_to_current_design);
        let restored = decoded
            .restore_session(DocumentSolveRequest::default(), SolverConfig::default())
            .expect("exactly restore current flexible fillet");

        assert_eq!(restored.design_document(), &design_before);
        assert_eq!(
            restored
                .accepted_state()
                .expect("restored accepted fillet")
                .document(),
            &accepted_before
        );
        assert_eq!(
            restored
                .accepted_state()
                .expect("restored accepted fillet")
                .document()
                .to_canonical_json()
                .expect("restored accepted canonical bytes"),
            accepted_json_before
        );
    }

    #[test]
    fn authored_constraint_round_trips_workspace_and_remains_editable() {
        let mut document = SketchDocument::new(4.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("free", [2.0, 1.0]).expect("point");
        let line = CurveSpan::line(
            document
                .add_curve(
                    "line",
                    CurveDefinition::Line {
                        start: first,
                        end: second,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let mut authoring = AuthoringState::default();
        let application = match authoring.activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            &[AuthoringOperand::selected(SelectionItem::Curve(line))],
        ) {
            AuthoringOutcome::Apply(application) => application,
            outcome => panic!("expected horizontal application, got {outcome:?}"),
        };
        let created = match coordinator
            .apply_authoring(coordinator.session().design_identity(), &application)
            .expect("author horizontal constraint")
        {
            AuthoringMutation::Constraint(outcome) => outcome,
            AuthoringMutation::Dimension(_) => panic!("expected constraint mutation"),
        };
        assert!(created.published_accepted.is_some());
        assert!(matches!(
            coordinator
                .session()
                .design_document()
                .constraint(created.value)
                .expect("authored constraint")
                .definition,
            DocumentConstraintDefinition::Horizontal { line: actual } if actual == line
        ));

        let authored_json = coordinator.checkpoint().design_json().to_owned();
        let snapshot =
            WorkspaceSnapshot::from_coordinator(&coordinator).expect("capture workspace");
        let decoded =
            WorkspaceSnapshot::decode(&snapshot.encode().expect("encode")).expect("decode");
        let restored_session = decoded
            .restore_session(DocumentSolveRequest::default(), SolverConfig::default())
            .expect("restore session");
        let mut restored =
            RetainedEditorCoordinator::new(restored_session).expect("restored coordinator");
        assert_eq!(restored.checkpoint().design_json(), authored_json);

        let source = restored
            .session()
            .design_document()
            .constraint(created.value)
            .expect("restored authored constraint")
            .source_id;
        restored
            .editor_mut()
            .set_selection([SelectionItem::Constraint(created.value)]);
        let edited = restored
            .set_selected_suppressed(restored.session().design_identity(), true)
            .expect("suppress restored constraint");
        assert!(edited.published_accepted.is_some());
        assert!(
            restored
                .session()
                .design_document()
                .source(source)
                .expect("restored authored source")
                .suppressed
        );

        restored.undo().expect("undo suppression");
        assert!(
            !restored
                .session()
                .design_document()
                .source(source)
                .expect("restored authored source after undo")
                .suppressed
        );
    }

    #[test]
    fn m49_checkpoint_codec_round_trips_accepted_a4_contact_state() {
        let fixture = alpha_scenario(AlphaScenarioKind::A4, 1.0).unwrap();
        let AlphaScenarioIds::A4(ids) = fixture.ids else {
            panic!("A4 fixture IDs expected");
        };
        let mut document = fixture.document;
        let original = document.contact(ids.circle_contact).cloned().unwrap();
        let original_principal = document.scalar(original.parameter).unwrap().value;
        let paired_arc = document.contact(ids.arc_contact).cloned().unwrap();
        let paired_arc_principal = document.scalar(paired_arc.parameter).unwrap().value;
        document
            .set_contact_states(&[
                ContactStateEdit {
                    contact: ids.circle_contact,
                    value: original_principal,
                    winding: original.winding + 1,
                    neighborhood: original.neighborhood,
                    tangent_orientation: original.tangent_orientation,
                },
                ContactStateEdit {
                    contact: ids.arc_contact,
                    value: paired_arc_principal,
                    winding: paired_arc.winding,
                    neighborhood: paired_arc.neighborhood,
                    tangent_orientation: paired_arc.tangent_orientation,
                },
            ])
            .unwrap();
        let session =
            RetainedSketchDocumentSession::new(document, fixture.request, SolverConfig::default())
                .unwrap();
        let coordinator = RetainedEditorCoordinator::new(session).unwrap();

        let snapshot =
            WorkspaceSnapshot::from_coordinator(&coordinator).expect("capture workspace");
        let decoded = WorkspaceSnapshot::decode(&snapshot.encode().unwrap()).unwrap();
        assert_eq!(decoded.design, snapshot.design);
        assert_eq!(decoded.accepted, snapshot.accepted);
        assert_eq!(decoded.revisions(), snapshot.revisions());

        for document in [
            decoded.design_document().unwrap(),
            decoded
                .accepted_document()
                .unwrap()
                .expect("accepted document"),
        ] {
            let circle_contact = document.contact(ids.circle_contact).unwrap();
            assert_eq!(circle_contact.id, ids.circle_contact);
            assert_eq!(
                circle_contact.winding,
                original.winding + 1,
                "accepted circle winding did not persist"
            );
            assert_eq!(circle_contact.neighborhood, original.neighborhood);
            assert_eq!(
                circle_contact.tangent_orientation,
                original.tangent_orientation
            );
            assert_eq!(
                document
                    .scalar(circle_contact.parameter)
                    .unwrap()
                    .value
                    .to_bits(),
                original_principal.to_bits()
            );

            let arc_contact = document.contact(ids.arc_contact).unwrap();
            assert_eq!(arc_contact.id, ids.arc_contact);
            assert_eq!(arc_contact.winding, paired_arc.winding);
            assert_eq!(arc_contact.neighborhood, paired_arc.neighborhood);
            assert_eq!(
                arc_contact.tangent_orientation,
                paired_arc.tangent_orientation
            );
        }
        assert!(decoded.revisions().accepted().is_some());
        assert!(
            decoded.revisions().design().get() >= decoded.revisions().accepted().unwrap().get()
        );
    }

    #[test]
    fn codec_rejects_malformed_unknown_version_and_unknown_fields() {
        for input in [
            "not json",
            r#"{"version":4,"design":{"encoding":"canonical_v4","json":"{}"},"accepted":null,"accepted_belongs_to_current_design":false,"revisions":{"design":1,"attempt":1,"accepted":null}}"#,
            r#"{"version":3,"design":{"encoding":"canonical_v4","json":"{}"},"accepted":null,"revisions":{"design":1,"attempt":1,"accepted":null}}"#,
            r#"{"version":3,"design":{"encoding":"canonical_v4","json":"{}"},"accepted":null,"accepted_belongs_to_current_design":true,"revisions":{"design":1,"attempt":1,"accepted":null}}"#,
            r#"{"version":2,"design":{"encoding":"future_v6","json":"{}"},"accepted":null,"revisions":{"design":1,"attempt":1,"accepted":null}}"#,
            r#"{"version":2,"design":{"encoding":"canonical_v4","json":"{}"},"accepted":null,"accepted_belongs_to_current_design":true,"revisions":{"design":1,"attempt":1,"accepted":null}}"#,
            r#"{"version":2,"design":{"encoding":"canonical_v4","json":"{}"},"accepted":null,"revisions":{"design":1,"attempt":1,"accepted":null},"extra":true}"#,
        ] {
            assert!(
                WorkspaceSnapshot::decode(input).is_err(),
                "accepted {input}"
            );
        }
    }

    #[test]
    fn workspace_v5_rejects_invalid_sketch_identity_high_water() {
        let coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                SketchDocument::new(1.0).expect("document"),
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("session"),
        )
        .expect("coordinator");
        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).expect("snapshot");
        let encoded = snapshot.encode().expect("encoded snapshot");
        let baseline: serde_json::Value = serde_json::from_str(&encoded).expect("snapshot value");
        let assert_rejected = |value: serde_json::Value| {
            let input = serde_json::to_string(&value).expect("test input");
            assert!(
                WorkspaceSnapshot::decode(&input).is_err(),
                "accepted invalid high-water payload {input}"
            );
        };

        let mut missing = baseline.clone();
        missing
            .as_object_mut()
            .expect("workspace object")
            .remove("sketch_identity_high_water");
        assert_rejected(missing);

        let mut unknown = baseline.clone();
        unknown["sketch_identity_high_water"]
            .as_object_mut()
            .expect("high-water object")
            .insert("extra".into(), serde_json::Value::Bool(true));
        assert_rejected(unknown);

        let mut foreign = baseline.clone();
        foreign["sketch_identity_high_water"] = serde_json::to_value(
            SketchDocument::new(1.0)
                .expect("foreign document")
                .persistent_identity_high_water(),
        )
        .expect("foreign high-water value");
        assert_rejected(foreign);

        let mut object_cursor_behind = baseline.clone();
        object_cursor_behind["sketch_identity_high_water"]["next_id"] =
            serde_json::Value::String("00000000000000000000000000000000".into());
        assert_rejected(object_cursor_behind);

        let (spline_document, curve) = clamped_bspline_document();
        let spline_coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                spline_document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("spline session"),
        )
        .expect("spline coordinator");
        let spline_snapshot =
            WorkspaceSnapshot::from_coordinator(&spline_coordinator).expect("spline snapshot");
        let mut spline_cursor_behind: serde_json::Value =
            serde_json::from_str(&spline_snapshot.encode().expect("encoded spline snapshot"))
                .expect("spline snapshot value");
        spline_cursor_behind["sketch_identity_high_water"]["spline_span_cursors"]
            .as_object_mut()
            .expect("spline cursor map")
            .insert(curve.to_string(), serde_json::Value::from(99));
        assert_rejected(spline_cursor_behind);

        let mut accepted_cursor_ahead = snapshot;
        let mut accepted = accepted_cursor_ahead
            .accepted_document()
            .expect("accepted payload")
            .expect("accepted document");
        accepted
            .add_point("accepted-only point", [2.0, 3.0])
            .expect("advance accepted cursor");
        let accepted_payload = accepted_cursor_ahead
            .accepted
            .as_mut()
            .expect("accepted workspace payload");
        accepted_payload.json = accepted
            .to_canonical_json()
            .expect("accepted canonical payload");
        assert_rejected(
            serde_json::to_value(accepted_cursor_ahead).expect("accepted-ahead snapshot value"),
        );

        assert!(WorkspaceSnapshot::decode(&format!("{encoded} trailing")).is_err());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one regression follows all legacy versions through the same current sidecar invariant"
    )]
    fn v5_round_trips_draft_v5_and_migrates_v4_v3_v2_and_v1() {
        use geosolve_sketch::{
            CurveDefinition, CurveSpan, DocumentCurveTrimView, DocumentTrimBoundary,
            DocumentTrimParameter,
        };

        let mut document = SketchDocument::new(8.0).unwrap();
        let first = document.add_point("first", [0.0, 0.0]).unwrap();
        let second = document.add_point("second", [4.0, 0.0]).unwrap();
        let curve = document
            .add_curve(
                "split support",
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let support = CurveSpan::line(curve);
        let boundary = |parameter| {
            DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                parameter,
                winding: 0,
            })
        };
        document
            .replace_trim_views(
                support,
                vec![
                    DocumentCurveTrimView {
                        support,
                        start: boundary(0.0),
                        end: boundary(0.5),
                    },
                    DocumentCurveTrimView {
                        support,
                        start: boundary(0.5),
                        end: boundary(1.0),
                    },
                ],
            )
            .unwrap();
        let coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let snapshot =
            WorkspaceSnapshot::from_coordinator(&coordinator).expect("capture workspace");
        assert_eq!(
            snapshot.design.encoding,
            super::WorkspaceDocumentEncoding::DraftV5
        );
        let decoded = WorkspaceSnapshot::decode(&snapshot.encode().unwrap()).unwrap();
        assert_eq!(
            decoded
                .design_document()
                .unwrap()
                .visible_intervals(support)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(decoded.features_json, snapshot.features_json);

        let v4 = serde_json::json!({
            "version": 4,
            "design": snapshot.design.clone(),
            "accepted": snapshot.accepted.clone(),
            "accepted_belongs_to_current_design": snapshot.accepted_belongs_to_current_design,
            "features_json": snapshot.features_json.clone(),
            "feature_lifecycle_high_water": snapshot.feature_lifecycle_high_water,
            "computed_evaluation_high_water": snapshot.computed_evaluation_high_water,
            "revisions": snapshot.revisions,
        })
        .to_string();
        let migrated_v4 = WorkspaceSnapshot::decode(&v4).expect("migrate workspace v4");
        assert_eq!(migrated_v4.version, 6);
        assert_eq!(migrated_v4.design, snapshot.design);
        assert_eq!(migrated_v4.accepted, snapshot.accepted);
        assert_eq!(migrated_v4.features_json, snapshot.features_json);
        assert!(migrated_v4.annotation_layout().entries().is_empty());
        assert_eq!(
            migrated_v4.sketch_identity_high_water,
            derive_sketch_identity_high_water(&snapshot.design, snapshot.accepted.as_ref())
                .expect("derived legacy sketch high-water")
        );

        let v3 = serde_json::json!({
            "version": 3,
            "design": snapshot.design.clone(),
            "accepted": snapshot.accepted.clone(),
            "accepted_belongs_to_current_design": snapshot.accepted_belongs_to_current_design,
            "revisions": snapshot.revisions,
        })
        .to_string();
        let migrated_v3 = WorkspaceSnapshot::decode(&v3).unwrap();
        assert_eq!(migrated_v3.version, 6);
        assert!(migrated_v3.annotation_layout().entries().is_empty());
        assert!(
            migrated_v3
                .feature_document()
                .unwrap()
                .features()
                .is_empty()
        );
        assert_eq!(
            migrated_v3.computed_evaluation_high_water,
            default_evaluation_high_water()
        );

        let v2 = serde_json::json!({
            "version": 2,
            "design": snapshot.design.clone(),
            "accepted": snapshot.accepted.clone(),
            "revisions": snapshot.revisions,
        })
        .to_string();
        let migrated_v2 = WorkspaceSnapshot::decode(&v2).unwrap();
        assert_eq!(migrated_v2.version, 6);
        assert!(migrated_v2.annotation_layout().entries().is_empty());
        assert!(!migrated_v2.accepted_belongs_to_current_design);
        assert_eq!(
            migrated_v2
                .design_document()
                .unwrap()
                .visible_intervals(support)
                .unwrap()
                .len(),
            2
        );
        assert!(
            migrated_v2
                .feature_document()
                .unwrap()
                .features()
                .is_empty()
        );

        let empty = SketchDocument::new(8.0).unwrap();
        let v1 = format!(
            r#"{{"version":1,"design_json":{},"accepted_json":null,"revisions":{{"design":1,"attempt":1,"accepted":null}}}}"#,
            serde_json::to_string(&empty.to_canonical_json().unwrap()).unwrap()
        );
        let migrated = WorkspaceSnapshot::decode(&v1).unwrap();
        assert_eq!(migrated.version, 6);
        assert!(migrated.annotation_layout().entries().is_empty());
        assert!(!migrated.accepted_belongs_to_current_design);
        assert_eq!(
            migrated.design.encoding,
            super::WorkspaceDocumentEncoding::CanonicalV4
        );
        migrated.design_document().unwrap();
        assert!(migrated.feature_document().unwrap().features().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one adapter regression keeps all six retained M71 records and exact workspace authority contiguous"
    )]
    fn v5_round_trips_all_m71_relations_through_the_workspace_adapter() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let points = [
            [0.0, 0.0],
            [2.0, 0.0],
            [4.0, 0.0],
            [4.0, 2.0],
            [7.0, 0.0],
            [7.0, 0.0],
            [10.0, 0.0],
            [12.0, 0.0],
            [13.0, 0.0],
            [15.0, 0.0],
        ]
        .map(|position| document.add_point("M71 point", position).expect("point"));
        let circles = [(points[4], 1.0), (points[5], 2.0)].map(|(center, value)| {
            let radius = document
                .add_scalar(
                    "M71 radius",
                    value,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .expect("radius");
            document
                .add_curve("M71 circle", CurveDefinition::Circle { center, radius })
                .expect("circle")
        });
        let lines = [(points[6], points[7]), (points[8], points[9])].map(|(start, end)| {
            document
                .add_curve(
                    "M71 line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line")
        });
        for (label, definition) in [
            (
                "M71 horizontal points",
                DocumentConstraintDefinition::HorizontalPoints {
                    first: points[0],
                    second: points[1],
                },
            ),
            (
                "M71 vertical points",
                DocumentConstraintDefinition::VerticalPoints {
                    first: points[2],
                    second: points[3],
                },
            ),
            (
                "M71 horizontal point to midpoint",
                DocumentConstraintDefinition::HorizontalPointToMidpoint {
                    point: points[2],
                    line: CurveSpan::line(lines[0]),
                },
            ),
            (
                "M71 vertical point to midpoint",
                DocumentConstraintDefinition::VerticalPointToMidpoint {
                    point: points[3],
                    line: CurveSpan::line(lines[1]),
                },
            ),
            (
                "M71 concentric",
                DocumentConstraintDefinition::Concentric {
                    first: DocumentCenterRef { curve: circles[0] },
                    second: DocumentCenterRef { curve: circles[1] },
                },
            ),
            (
                "M71 collinear",
                DocumentConstraintDefinition::Collinear {
                    first: DocumentLineSupportRef {
                        span: CurveSpan::line(lines[0]),
                        direction: DocumentDirectionSense::Forward,
                    },
                    second: DocumentLineSupportRef {
                        span: CurveSpan::line(lines[1]),
                        direction: DocumentDirectionSense::Reverse,
                    },
                },
            ),
        ] {
            document
                .add_constraint(label, definition)
                .expect("M71 relation");
        }

        let exact_draft = document.to_draft_v5_json().expect("draft-v5 document");
        assert!(matches!(
            document.to_canonical_json(),
            Err(DocumentError::UnsupportedM71State)
        ));
        let expected_definitions = document
            .constraints()
            .iter()
            .map(|constraint| constraint.definition.clone())
            .collect::<Vec<_>>();
        let expected_source_order = document.source_order().to_vec();
        let coordinator = RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new(
                document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("accepted M71 session"),
        )
        .expect("M71 coordinator");

        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).expect("workspace");
        assert_eq!(
            snapshot.design.encoding,
            super::WorkspaceDocumentEncoding::DraftV5
        );
        assert_eq!(snapshot.design.json, exact_draft);
        assert_eq!(
            snapshot.accepted.as_ref().map(|payload| payload.encoding),
            Some(super::WorkspaceDocumentEncoding::DraftV5)
        );
        assert!(snapshot.accepted_belongs_to_current_design);

        let decoded = WorkspaceSnapshot::decode(&snapshot.encode().expect("encode workspace"))
            .expect("decode workspace");
        let restored = coordinator_from_snapshot(&decoded).expect("restore coordinator");
        let restored_document = restored.session().design_document();
        assert_eq!(
            restored_document
                .constraints()
                .iter()
                .map(|constraint| constraint.definition.clone())
                .collect::<Vec<_>>(),
            expected_definitions
        );
        assert_eq!(restored_document.source_order(), expected_source_order);
        assert_eq!(
            restored_document
                .to_draft_v5_json()
                .expect("restored draft-v5 document"),
            exact_draft
        );
        assert!(matches!(
            restored_document.to_canonical_json(),
            Err(DocumentError::UnsupportedM71State)
        ));
        assert!(
            restored
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );
    }

    #[test]
    fn legacy_workspace_migration_preserves_solver_owned_fillet_without_computed_migration() {
        let fixture = alpha_scenario(AlphaScenarioKind::FilletLineCircle, 1.0)
            .expect("legacy M28 Fillet fixture");
        let AlphaScenarioIds::FilletLineCircle(ids) = fixture.ids else {
            panic!("line-circle Fillet IDs expected")
        };
        let design_json = fixture
            .document
            .to_canonical_json()
            .expect("canonical M28 document");
        assert!(
            fixture
                .document
                .curve_curve_fillet_for_arc(ids.fillet.arc)
                .is_some()
        );
        let revisions = serde_json::json!({
            "design": 1,
            "attempt": 1,
            "accepted": null,
        });
        let payload = serde_json::json!({
            "encoding": "canonical_v4",
            "json": design_json,
        });
        let legacy = [
            serde_json::json!({
                "version": 1,
                "design_json": design_json,
                "accepted_json": null,
                "revisions": revisions,
            }),
            serde_json::json!({
                "version": 2,
                "design": payload,
                "accepted": null,
                "revisions": revisions,
            }),
            serde_json::json!({
                "version": 3,
                "design": payload,
                "accepted": null,
                "accepted_belongs_to_current_design": false,
                "revisions": revisions,
            }),
        ];

        for encoded in legacy.map(|value| value.to_string()) {
            let migrated = WorkspaceSnapshot::decode(&encoded).expect("migrate legacy workspace");
            let document = migrated.design_document().expect("migrated M28 document");
            assert!(
                document
                    .curve_curve_fillet_for_arc(ids.fillet.arc)
                    .is_some(),
                "legacy M28 Fillet changed meaning in {encoded}"
            );
            assert!(migrated.feature_document().unwrap().features().is_empty());
        }
    }

    #[test]
    fn m83_workspace_v8_round_trips_canonical_intent_and_flat_accepted_authority_exactly() {
        run_m83_persistence_test("m83-v8-round-trip", || {
            m83_workspace_v8_round_trips_canonical_intent_and_flat_accepted_authority_exactly_body(
            );
        });
    }

    #[test]
    fn m83_projectional_reproduction_round_trips_v8_authority_without_annotation_cache() {
        run_m83_persistence_test("m83-v8-reproduction-round-trip", || {
            let (mut snapshot, intent, accepted) = m83_projectional_fixture();
            snapshot.annotation_layout_json = Some(
                serde_json::to_string(&super::WorkspaceAnnotationLayoutCache {
                    version: AnnotationLayoutState::VERSION,
                    entries: Vec::new(),
                })
                .expect("annotation cache"),
            );

            let payload = reproduction_payload_from_snapshot(snapshot)
                .expect("projectional reproduction payload");
            assert!(payload.starts_with("GEOSOLVE_REPRO_V1:"));
            let workspace = crate::reproduction::decode_workspace(&payload)
                .expect("decode reproduction transport");
            assert!(workspace.contains("\"version\":8"));

            let decoded = snapshot_from_reproduction_payload(&payload)
                .expect("decode projectional reproduction snapshot");
            assert!(
                decoded.annotation_layout_json.is_none(),
                "reproduction transport must discard disposable annotation placement",
            );
            assert_eq!(
                decoded
                    .intent_session()
                    .expect("intent decode")
                    .expect("projectional intent")
                    .to_canonical_json()
                    .expect("canonical intent"),
                intent.to_canonical_json().expect("source intent"),
            );
            assert_eq!(
                decoded.accepted_document().expect("accepted evidence"),
                Some(accepted.clone()),
            );
            let restored = projectional_editor_from_snapshot(&decoded)
                .expect("restore projectional reproduction authority");
            assert_eq!(
                restored.coordinator().intent().identity(),
                intent.identity()
            );
            assert_eq!(
                restored
                    .coordinator()
                    .accepted_materialization()
                    .expect("accepted materialization")
                    .session
                    .accepted_state_for_current_input()
                    .expect("accepted state")
                    .document(),
                &accepted,
            );
        });
    }

    #[test]
    fn m83_legacy_workspace_v8_outer_digest_migrates_to_sha256() {
        let (snapshot, _, _) = m83_projectional_fixture();
        let canonical = snapshot.encode().expect("canonical workspace v8");
        let mut wire: WorkspaceSnapshotV8 =
            serde_json::from_str(&canonical).expect("workspace-v8 wire");
        wire.intent_session_json = rewrite_intent_session_as_legacy(&wire.intent_session_json);
        wire.digest = legacy_workspace_v8_digest(&wire);

        let legacy_json = serde_json::to_string(&wire).expect("legacy-digest workspace v8");
        let migrated = WorkspaceSnapshot::decode(&legacy_json)
            .expect("genuine legacy workspace-v8 authority remains migratable");
        assert_eq!(
            migrated.encode().expect("canonical migrated workspace v8"),
            canonical,
        );
    }

    #[test]
    fn m83_authenticated_workspace_v8_discards_oversized_annotation_cache() {
        let (snapshot, intent, accepted) = m83_projectional_fixture();
        let canonical = snapshot.encode().expect("canonical workspace v8");
        let mut wire: WorkspaceSnapshotV8 =
            serde_json::from_str(&canonical).expect("workspace-v8 wire");
        wire.annotation_layout_json = Some("x".repeat(super::MAX_ANNOTATION_LAYOUT_JSON_BYTES + 1));
        wire.digest = workspace_v8_digest(&wire);

        let oversized = serde_json::to_string(&wire).expect("oversized-cache workspace v8");
        let migrated = WorkspaceSnapshot::decode(&oversized)
            .expect("authenticated semantic workspace must survive disposable cache eviction");
        assert!(migrated.annotation_layout_json.is_none());
        assert!(migrated.annotation_layout().entries().is_empty());
        assert_eq!(migrated.intent_session().unwrap(), Some(intent));
        assert_eq!(migrated.accepted_document().unwrap(), Some(accepted));

        let canonical_without_cache = migrated.encode().expect("migrated workspace v8");
        assert!(!canonical_without_cache.contains("annotation_layout_json"));
        WorkspaceSnapshot::decode(&canonical_without_cache)
            .expect("cache-free migrated workspace remains canonical");
    }

    #[test]
    fn m83_workspace_v8_rejects_unowned_flat_design_without_accepted_authority() {
        let (mut snapshot, _, _) = m83_projectional_fixture();
        snapshot.accepted = None;
        snapshot.accepted_belongs_to_current_design = false;
        let empty_intent = IntentSession::with_id(IntentSessionId::from_raw(0x8308_0000))
            .expect("empty intent session");
        snapshot.intent_session_json = Some(
            empty_intent
                .to_canonical_json()
                .expect("canonical empty intent session"),
        );

        assert!(
            snapshot
                .encode()
                .unwrap_err()
                .contains("requires independently accepted intent and flat authority")
        );
    }

    #[test]
    fn m83_legacy_workspace_v8_outer_digest_rejects_nested_v2_authority() {
        let (snapshot, _, _) = m83_projectional_fixture();
        let canonical = snapshot.encode().expect("canonical workspace v8");
        let mut wire: WorkspaceSnapshotV8 =
            serde_json::from_str(&canonical).expect("workspace-v8 wire");
        wire.digest = legacy_workspace_v8_digest(&wire);
        let forged = serde_json::to_string(&wire).expect("legacy-outer workspace v8");

        assert!(
            WorkspaceSnapshot::decode(&forged)
                .unwrap_err()
                .contains("legacy workspace-v8 digest requires")
        );
    }

    #[test]
    fn m83_workspace_v8_bootstrap_routing_recognizes_a_fully_ejected_graph() {
        let mut document = SketchDocument::new(1.0).expect("historical document");
        let point = document
            .add_point("historical point", [1.25, -2.5])
            .expect("historical point");
        let features = geosolve_sketch_features::ComputedFeatureDocument::new(document.id());
        let mut intent = normalize_flat_sketch_intent(
            IntentSessionId::from_raw(0x8308_0002),
            &document,
            &features,
            features.lifecycle_high_water(),
        )
        .expect("normalized bootstrap");
        let point_node = intent
            .graph()
            .nodes()
            .values()
            .find(|node| node.symbol.as_str() == format!("legacy-point-{point}"))
            .expect("historical point declaration")
            .id;
        let ejection = intent
            .plan_patch(
                IntentPatch::new(
                    intent.identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::EjectBootstrapPoint { node: point_node }],
                ),
                synthetic_intent_acceptance,
            )
            .expect("eject historical point");
        intent.commit_plan(ejection).expect("commit ejection");

        let bootstrap_nodes = intent
            .graph()
            .nodes()
            .values()
            .filter(|node| matches!(node.kind, IntentNodeKind::Bootstrap { .. }))
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(
            bootstrap_nodes.len(),
            1,
            "a point-only flat document should retain only its document header"
        );
        let delete_header = intent
            .plan_patch(
                IntentPatch::new(
                    intent.identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::DeleteNode {
                        node: bootstrap_nodes[0],
                        policy: DeletePolicy::RejectDependents,
                    }],
                ),
                synthetic_intent_acceptance,
            )
            .expect("detached bootstrap header is deletable");
        intent
            .commit_plan(delete_header)
            .expect("commit header deletion");

        assert!(
            intent
                .graph()
                .nodes()
                .values()
                .all(|node| !matches!(node.kind, IntentNodeKind::Bootstrap { .. }))
        );
        let ejected = intent.graph().node(point_node).expect("ejected point");
        assert!(ejected.bootstrap_origin.is_some());
        assert!(intent_node_requires_bootstrap_seed(ejected));
        assert!(
            intent
                .graph()
                .nodes()
                .values()
                .any(intent_node_requires_bootstrap_seed)
        );
    }

    fn m83_workspace_v8_round_trips_canonical_intent_and_flat_accepted_authority_exactly_body() {
        let (snapshot, intent, accepted) = m83_projectional_fixture();
        assert_eq!(snapshot.version, 8);
        assert!(snapshot.legacy_bootstrap().is_none());

        let encoded = snapshot.encode().expect("canonical workspace v8");
        let decoded = WorkspaceSnapshot::decode(&encoded).expect("decode workspace v8");
        assert_eq!(decoded.encode().expect("re-encode workspace v8"), encoded);
        assert_eq!(
            coordinator_from_snapshot(&decoded)
                .expect_err("v8 must not install a parallel flat coordinator"),
            "workspace v8 must restore through its sole projectional editor authority",
        );
        assert_eq!(
            decoded
                .intent_session()
                .expect("decode intent")
                .expect("v8 intent")
                .to_canonical_json()
                .expect("canonical restored intent"),
            intent
                .to_canonical_json()
                .expect("canonical original intent")
        );
        assert_eq!(
            decoded.accepted_document().expect("accepted flat state"),
            Some(accepted.clone())
        );
        let restored =
            projectional_editor_from_snapshot(&decoded).expect("cold-restore projectional editor");
        assert_eq!(
            restored.coordinator().intent().identity(),
            intent.identity()
        );
        assert!(
            restored
                .coordinator()
                .presentation_session()
                .expect("presentation session")
                .accepted_state_for_current_input()
                .is_some(),
            "v8 must cold-rebuild the exact accepted scene for immediate canvas restoration",
        );
        assert_eq!(
            restored
                .coordinator()
                .accepted_materialization()
                .expect("cold accepted materialization")
                .session
                .accepted_state_for_current_input()
                .expect("accepted state")
                .document(),
            &accepted,
        );
    }

    #[test]
    fn m83_migrated_v6_continues_through_edit_undo_redo_and_cold_v8_reload() {
        run_m83_persistence_test("m83-mixed-bootstrap-v8", || {
            m83_migrated_v6_continues_through_edit_undo_redo_and_cold_v8_reload_body();
        });
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one retained-invalid reload regression preserves canonical intent, accepted geometry, history, Undo and corruption rejection together"
    )]
    fn m83_retained_invalid_migrated_bootstrap_v8_reload_preserves_scene_intent_and_undo() {
        run_m83_persistence_test("m83-retained-invalid-bootstrap-v8", || {
            let mut document = SketchDocument::new(1.0).expect("document");
            let historical = document
                .add_point("historical point", [1.25, -2.5])
                .expect("historical point");
            let native = RetainedSketchDocumentSession::new(
                document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("accepted native session");
            let flat = WorkspaceSnapshot::from_coordinator(
                &RetainedEditorCoordinator::new(native).expect("flat coordinator"),
            )
            .expect("flat v6 workspace");
            let migrated = WorkspaceSnapshot::decode(&flat.encode().expect("encode v6"))
                .expect("strict v6 decode");
            let mut projectional =
                projectional_editor_from_legacy_snapshot(&migrated).expect("activate v6 bootstrap");

            let accepted_before = projectional
                .coordinator()
                .accepted_materialization()
                .expect("accepted bootstrap scene")
                .clone();
            let point_node = projectional
                .coordinator()
                .intent()
                .graph()
                .nodes()
                .values()
                .find(|node| {
                    matches!(
                        &node.kind,
                        IntentNodeKind::Bootstrap { object }
                            if object.kind == BootstrapNativeKind::Point
                    )
                })
                .expect("historical point declaration")
                .id;
            let outcome = projectional
                .apply_patch(IntentPatch::new(
                    projectional.coordinator().intent().identity(),
                    IntentPatchPolicy::RetainFailedIntent,
                    vec![IntentPatchOperation::SetSuppressed {
                        node: point_node,
                        suppressed: true,
                    }],
                ))
                .expect("retain explicitly invalid imported declaration");
            assert_eq!(outcome.disposition, IntentPlanDisposition::RetainedFailed);
            assert!(
                projectional
                    .coordinator()
                    .intent()
                    .graph()
                    .node(point_node)
                    .expect("current failed declaration")
                    .suppressed
            );
            assert!(
                !projectional
                    .coordinator()
                    .intent()
                    .accepted()
                    .expect("accepted bootstrap authority")
                    .graph
                    .node(point_node)
                    .expect("accepted declaration")
                    .suppressed
            );
            assert_eq!(
                projectional
                    .coordinator()
                    .accepted_materialization()
                    .expect("retained accepted scene")
                    .evidence,
                accepted_before.evidence
            );
            let retained_invalid_json = projectional
                .coordinator()
                .intent()
                .to_canonical_json()
                .expect("retained-invalid canonical intent");

            let snapshot = WorkspaceSnapshot::from_projectional_editor(&projectional)
                .expect("capture retained-invalid v8");
            let encoded = snapshot.encode().expect("encode retained-invalid v8");
            let decoded = WorkspaceSnapshot::decode(&encoded).expect("decode retained-invalid v8");
            let mut restored = projectional_editor_from_snapshot(&decoded)
                .expect("cold restore retained-invalid bootstrap authority");

            assert_eq!(
                restored
                    .coordinator()
                    .intent()
                    .to_canonical_json()
                    .expect("restored retained-invalid intent"),
                retained_invalid_json,
                "cold reload must keep the failed current graph inspectable",
            );
            let restored_accepted = restored
                .coordinator()
                .accepted_materialization()
                .expect("restored accepted scene");
            assert_eq!(restored_accepted.evidence, accepted_before.evidence);
            assert_eq!(
                restored_accepted
                    .session
                    .accepted_state_for_current_input()
                    .expect("accepted native scene")
                    .document()
                    .point(historical)
                    .expect("historical accepted point")
                    .position
                    .map(f64::to_bits),
                [1.25, -2.5].map(f64::to_bits),
            );
            let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
            assert!(
                restored
                    .scene(viewport, 0.5)
                    .expect("retained accepted canvas")
                    .points
                    .iter()
                    .any(|point| point.id == historical),
                "retained-invalid reload must not blank accepted geometry",
            );

            assert!(restored.undo().expect("undo retained failure").is_some());
            let intent = restored.coordinator().intent();
            assert_eq!(
                intent
                    .accepted()
                    .expect("accepted authority after Undo")
                    .target,
                intent.semantic_identity(),
            );
            assert!(
                !intent
                    .graph()
                    .node(point_node)
                    .expect("restored imported declaration")
                    .suppressed
            );
            let accepted_after_undo = restored
                .coordinator()
                .accepted_materialization()
                .expect("accepted scene after Undo");
            assert_eq!(
                accepted_after_undo.session.design_document(),
                accepted_before.session.design_document(),
            );
            assert_eq!(
                accepted_after_undo
                    .session
                    .accepted_state_for_current_input()
                    .expect("accepted native scene after Undo")
                    .document(),
                accepted_before
                    .session
                    .accepted_state_for_current_input()
                    .expect("original accepted native scene")
                    .document(),
            );

            let primary = IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            };
            let ordinary = IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::SketchPoint,
                },
                IntentKey::new("ordinary point").expect("symbol"),
            )
            .with_instance_leaf(
                primary,
                LeafField::X,
                IntentLiteral::Quantity {
                    value: 4.0,
                    unit: IntentUnit::Length,
                },
            )
            .with_instance_leaf(
                primary,
                LeafField::Y,
                IntentLiteral::Quantity {
                    value: 3.0,
                    unit: IntentUnit::Length,
                },
            );
            restored
                .apply_patch(IntentPatch::new(
                    restored.coordinator().intent().identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::CreateNode {
                        alias: IntentKey::new("ordinary-point").expect("alias"),
                        draft: Box::new(ordinary),
                        cell: None,
                    }],
                ))
                .expect("add ordinary declaration after imported bootstrap");

            let bootstrap_nodes = restored
                .coordinator()
                .intent()
                .graph()
                .nodes()
                .iter()
                .filter_map(|(node, value)| {
                    intent_node_requires_bootstrap_seed(value).then_some(*node)
                })
                .collect::<std::collections::BTreeSet<_>>();
            let exact_nodes = restored
                .coordinator()
                .intent()
                .graph()
                .dependent_closure(bootstrap_nodes.iter().copied())
                .expect("bootstrap dependent closure");
            let root = *bootstrap_nodes.iter().next().expect("bootstrap root");
            let deletion = restored
                .apply_patch(IntentPatch::new(
                    restored.coordinator().intent().identity(),
                    IntentPatchPolicy::RetainFailedIntent,
                    vec![IntentPatchOperation::DeleteNode {
                        node: root,
                        policy: DeletePolicy::CascadeRoots {
                            exact_roots: bootstrap_nodes,
                            exact_nodes,
                        },
                    }],
                ))
                .expect("retain deletion of every current bootstrap declaration");
            assert_eq!(deletion.disposition, IntentPlanDisposition::RetainedFailed);
            assert!(
                restored
                    .coordinator()
                    .intent()
                    .graph()
                    .nodes()
                    .values()
                    .all(|node| !intent_node_requires_bootstrap_seed(node))
            );

            let deleted_snapshot = WorkspaceSnapshot::from_projectional_editor(&restored)
                .expect("capture bootstrap-free retained-invalid v8");
            let deleted_snapshot = WorkspaceSnapshot::decode(
                &deleted_snapshot
                    .encode()
                    .expect("encode bootstrap-free retained-invalid v8"),
            )
            .expect("decode bootstrap-free retained-invalid v8");
            let restored_deleted = projectional_editor_from_snapshot(&deleted_snapshot)
                .expect("accepted bootstrap graph must route bootstrap-free current reload");
            assert!(
                restored_deleted
                    .coordinator()
                    .intent()
                    .graph()
                    .nodes()
                    .values()
                    .all(|node| !intent_node_requires_bootstrap_seed(node)),
                "the retained failed current graph remains inspectable",
            );
            assert!(
                restored_deleted
                    .scene(viewport, 0.5)
                    .expect("accepted canvas beneath bootstrap-free current graph")
                    .points
                    .iter()
                    .any(|point| point.id == historical),
                "bootstrap routing must inspect accepted as well as current intent",
            );
        });
    }

    #[allow(
        clippy::float_cmp,
        clippy::too_many_lines,
        reason = "one migration transcript keeps exact legacy scalar identity and complete continuation evidence contiguous"
    )]
    fn m83_migrated_v6_continues_through_edit_undo_redo_and_cold_v8_reload_body() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let historical = document
            .add_point("historical point", [0.0, 0.0])
            .expect("historical point");
        let historical_high_water = document.persistent_identity_high_water();
        let native = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted native session");
        let flat = WorkspaceSnapshot::from_coordinator(
            &RetainedEditorCoordinator::new(native).expect("flat coordinator"),
        )
        .expect("flat v6 workspace");
        let migrated = WorkspaceSnapshot::decode(&flat.encode().expect("encode v6"))
            .expect("strict v6 decode");
        let mut projectional =
            projectional_editor_from_legacy_snapshot(&migrated).expect("activate v6 bootstrap");

        let bootstrap_x_leaf = projectional
            .coordinator()
            .accepted_materialization()
            .expect("bootstrap accepted materialization")
            .ownership
            .writable_leaf(
                geosolve_constraint_editor::IntentNativeWritableLeaf::PointX { point: historical },
            )
            .expect("historical bootstrap x leaf");
        projectional
            .apply_patch(IntentPatch::new(
                projectional.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::SetInstanceLeaf {
                    leaf: bootstrap_x_leaf,
                    value: IntentLiteral::Quantity {
                        value: 1.0,
                        unit: IntentUnit::Length,
                    },
                }],
            ))
            .expect("edit pure bootstrap point");
        let edited_bootstrap = WorkspaceSnapshot::from_projectional_editor(&projectional)
            .expect("capture edited pure bootstrap v8");
        let edited_bootstrap = WorkspaceSnapshot::decode(
            &edited_bootstrap
                .encode()
                .expect("encode edited pure bootstrap v8"),
        )
        .expect("decode edited pure bootstrap v8");
        projectional = projectional_editor_from_snapshot(&edited_bootstrap)
            .expect("cold reload edited pure bootstrap v8");
        assert_eq!(
            projectional
                .coordinator()
                .accepted_materialization()
                .expect("reloaded bootstrap authority")
                .session
                .design_document()
                .point(historical)
                .expect("historical point")
                .position[0],
            1.0
        );

        let primary = IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        };
        let created = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            IntentKey::new("post migration point").expect("symbol"),
        )
        .with_instance_leaf(
            primary,
            LeafField::X,
            IntentLiteral::Quantity {
                value: 4.0,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            primary,
            LeafField::Y,
            IntentLiteral::Quantity {
                value: 3.0,
                unit: IntentUnit::Length,
            },
        );
        projectional
            .apply_patch(IntentPatch::new(
                projectional.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateNode {
                    alias: IntentKey::new("post-migration-point").expect("alias"),
                    draft: Box::new(created),
                    cell: None,
                }],
            ))
            .expect("create declaration after migration");
        let x_leaf = projectional
            .coordinator()
            .accepted_materialization()
            .expect("mixed accepted materialization")
            .ownership
            .writable_leaf(
                geosolve_constraint_editor::IntentNativeWritableLeaf::PointX { point: historical },
            )
            .expect("historical writable x leaf");
        projectional
            .apply_patch(IntentPatch::new(
                projectional.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::SetInstanceLeaf {
                    leaf: x_leaf,
                    value: IntentLiteral::Quantity {
                        value: 2.0,
                        unit: IntentUnit::Length,
                    },
                }],
            ))
            .expect("edit migrated free point");
        assert!(projectional.undo().expect("undo edit").is_some());
        assert_eq!(
            projectional
                .coordinator()
                .accepted_materialization()
                .expect("undo authority")
                .session
                .design_document()
                .point(historical)
                .expect("historical point")
                .position[0],
            1.0
        );
        assert!(projectional.redo().expect("redo edit").is_some());

        let before = projectional
            .coordinator()
            .accepted_materialization()
            .expect("mixed accepted materialization");
        assert_eq!(before.session.design_document().points().len(), 2);
        assert_ne!(
            before.session.persistent_identity_high_water(),
            &historical_high_water
        );
        let expected_design = before.session.design_document().clone();
        let expected_accepted = before
            .session
            .accepted_state_for_current_input()
            .expect("mixed accepted state")
            .document()
            .clone();
        let expected_intent = projectional
            .coordinator()
            .intent()
            .to_canonical_json()
            .expect("mixed canonical intent");
        let v8 =
            WorkspaceSnapshot::from_projectional_editor(&projectional).expect("capture mixed v8");
        let encoded = v8.encode().expect("encode mixed v8");
        let decoded = WorkspaceSnapshot::decode(&encoded).expect("decode mixed v8");
        assert_eq!(decoded.encode().expect("re-encode mixed v8"), encoded);
        let restored = projectional_editor_from_snapshot(&decoded).expect("cold mixed reload");
        let after = restored
            .coordinator()
            .accepted_materialization()
            .expect("cold mixed authority");
        assert_eq!(after.session.design_document(), &expected_design);
        assert_eq!(
            after
                .session
                .accepted_state_for_current_input()
                .expect("cold mixed accepted state")
                .document(),
            &expected_accepted
        );
        assert_eq!(
            restored
                .coordinator()
                .intent()
                .to_canonical_json()
                .expect("restored canonical intent"),
            expected_intent
        );
    }

    #[test]
    fn m83_workspace_v8_rejects_forged_or_noncanonical_intent_authority() {
        run_m83_persistence_test("m83-v8-rejection", || {
            m83_workspace_v8_rejects_forged_or_noncanonical_intent_authority_body();
        });
    }

    fn m83_workspace_v8_rejects_forged_or_noncanonical_intent_authority_body() {
        let (snapshot, _, _) = m83_projectional_fixture();
        let encoded = snapshot.encode().expect("canonical workspace v8");
        let mut wire: WorkspaceSnapshotV8 =
            serde_json::from_str(&encoded).expect("workspace-v8 wire");
        let mut different = SketchDocument::new(1.0).expect("different document");
        different
            .add_point("different point", [9.0, 4.0])
            .expect("different point");
        wire.materialization
            .accepted
            .as_mut()
            .expect("accepted materialization")
            .json = different.to_canonical_json().expect("different JSON");
        wire.digest = workspace_v8_digest(&wire);
        let forged = serde_json::to_string(&wire).expect("forged canonical wire");
        assert!(
            WorkspaceSnapshot::decode(&forged)
                .unwrap_err()
                .contains("intent evidence and flat accepted materialization disagree")
        );

        let mut wire: WorkspaceSnapshotV8 =
            serde_json::from_str(&encoded).expect("workspace-v8 wire");
        wire.intent_session_json.push(' ');
        wire.digest = workspace_v8_digest(&wire);
        let noncanonical_intent =
            serde_json::to_string(&wire).expect("noncanonical-intent workspace");
        assert!(
            WorkspaceSnapshot::decode(&noncanonical_intent)
                .unwrap_err()
                .contains("canonical")
        );
        assert_eq!(
            WorkspaceSnapshot::decode(&format!("\n{encoded}"))
                .expect_err("workspace-v8 outer JSON must also be canonical"),
            "workspace v8 JSON is not canonical"
        );
    }

    #[test]
    fn m83_workspace_v7_is_explicitly_rejected_as_abandoned() {
        run_m83_persistence_test("m83-v7-rejection", || {
            m83_workspace_v7_is_explicitly_rejected_as_abandoned_body();
        });
    }

    fn m83_workspace_v7_is_explicitly_rejected_as_abandoned_body() {
        let error = WorkspaceSnapshot::decode(r#"{"version":7}"#)
            .expect_err("abandoned workspace v7 must reject");
        assert_eq!(
            error,
            "workbench snapshot version 7 belongs to the abandoned chronological lineage format and is not supported"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one six-version table proves every historical strict decoder feeds the same typed bootstrap boundary"
    )]
    fn m83_legacy_v1_through_v6_restore_as_structured_bootstrap_input() {
        run_m83_persistence_test("m83-legacy-migration", || {
            m83_legacy_v1_through_v6_restore_as_structured_bootstrap_input_body();
        });
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one six-version table proves every historical strict decoder feeds the same typed bootstrap boundary"
    )]
    fn m83_legacy_v1_through_v6_restore_as_structured_bootstrap_input_body() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(1.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let flat = WorkspaceSnapshot::from_coordinator(&coordinator).expect("flat workspace");
        let design_json = flat.design.json.clone();
        let accepted_json = flat.accepted.as_ref().map(|payload| payload.json.clone());
        let revisions = serde_json::to_value(flat.revisions).expect("revisions");
        let wires = vec![
            serde_json::json!({
                "version": 1,
                "design_json": design_json.clone(),
                "accepted_json": accepted_json.clone(),
                "revisions": revisions.clone(),
            })
            .to_string(),
            serde_json::json!({
                "version": 2,
                "design": flat.design.clone(),
                "accepted": flat.accepted.clone(),
                "revisions": revisions.clone(),
            })
            .to_string(),
            serde_json::json!({
                "version": 3,
                "design": flat.design.clone(),
                "accepted": flat.accepted.clone(),
                "accepted_belongs_to_current_design": flat.accepted_belongs_to_current_design,
                "revisions": revisions.clone(),
            })
            .to_string(),
            serde_json::json!({
                "version": 4,
                "design": flat.design.clone(),
                "accepted": flat.accepted.clone(),
                "accepted_belongs_to_current_design": flat.accepted_belongs_to_current_design,
                "features_json": flat.features_json.clone(),
                "feature_lifecycle_high_water": flat.feature_lifecycle_high_water,
                "computed_evaluation_high_water": flat.computed_evaluation_high_water,
                "revisions": revisions.clone(),
            })
            .to_string(),
            {
                let mut value = serde_json::to_value(&flat).expect("flat workspace value");
                value["version"] = serde_json::Value::from(5);
                value
                    .as_object_mut()
                    .expect("workspace object")
                    .remove("annotation_layout_json");
                value.to_string()
            },
            flat.encode().expect("workspace v6"),
        ];

        for (index, wire) in wires.into_iter().enumerate() {
            let source_version = u32::try_from(index + 1).expect("bounded source version");
            let decoded = WorkspaceSnapshot::decode(&wire).expect("strict legacy decode");
            assert!(decoded.intent_session().expect("intent absence").is_none());
            let bootstrap = decoded
                .legacy_bootstrap()
                .expect("typed legacy bootstrap input");
            assert_eq!(bootstrap.source_version(), source_version);
            assert_eq!(
                bootstrap.design_document().expect("bootstrap design"),
                decoded.design_document().expect("decoded design")
            );
            assert_eq!(
                bootstrap.accepted_document().expect("bootstrap accepted"),
                decoded.accepted_document().expect("decoded accepted")
            );
            assert_eq!(
                bootstrap.feature_document().expect("bootstrap features"),
                decoded.feature_document().expect("decoded features")
            );
            assert_eq!(bootstrap.revisions(), decoded.revisions());
            assert_eq!(
                bootstrap.accepted_belongs_to_current_design(),
                decoded.accepted_belongs_to_current_design
            );
            assert_eq!(
                bootstrap.sketch_identity_high_water(),
                &decoded.sketch_identity_high_water
            );
            assert_eq!(
                bootstrap.feature_lifecycle_high_water(),
                decoded.feature_lifecycle_high_water()
            );
            assert_eq!(
                bootstrap.computed_evaluation_high_water(),
                decoded.computed_evaluation_high_water()
            );
            assert_eq!(
                decoded
                    .encode()
                    .expect_err("legacy aggregate is not v8 intent"),
                format!(
                    "legacy workspace v{source_version} must be normalized into per-object bootstrap declarations before v8 encoding"
                )
            );

            let expected_design = decoded.design_document().expect("expected design");
            let expected_accepted = decoded.accepted_document().expect("expected accepted");
            let expected_identity_high_water = decoded.sketch_identity_high_water.clone();
            let expected_feature_lifecycle = decoded.feature_lifecycle_high_water();
            let expected_evaluation_high_water = decoded.computed_evaluation_high_water();
            let expected_revisions = decoded.revisions();
            let projectional = projectional_editor_from_legacy_snapshot(&decoded)
                .expect("activate strict legacy bootstrap");
            assert_eq!(
                projectional
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                0,
                "migration must invent no user history"
            );
            let native = projectional
                .coordinator()
                .presentation_session()
                .expect("activated native authority");
            assert_eq!(native.design_document(), &expected_design);
            assert_eq!(
                native
                    .accepted_state_for_current_input()
                    .map(geosolve_sketch::SketchAcceptedDocumentState::document),
                expected_accepted.as_ref()
            );
            assert_eq!(
                native.persistent_identity_high_water(),
                &expected_identity_high_water
            );

            let v8 = WorkspaceSnapshot::from_projectional_editor_with_persistence_high_water(
                &projectional,
                expected_evaluation_high_water,
                decoded.revisions,
            )
            .expect("capture canonical v8");
            assert_eq!(v8.version, 8);
            assert_eq!(v8.design_document().unwrap(), expected_design);
            assert_eq!(v8.accepted_document().unwrap(), expected_accepted);
            assert_eq!(v8.sketch_identity_high_water, expected_identity_high_water);
            assert_eq!(
                v8.feature_lifecycle_high_water(),
                expected_feature_lifecycle
            );
            assert_eq!(v8.revisions(), expected_revisions);
            assert_eq!(
                v8.computed_evaluation_high_water(),
                expected_evaluation_high_water
            );
            let canonical = v8.encode().expect("canonical v8 JSON");
            assert!(canonical.contains("\"version\":8"));
            let decoded_v8 = WorkspaceSnapshot::decode(&canonical).expect("decode canonical v8");
            assert_eq!(
                decoded_v8.encode().expect("re-encode canonical v8"),
                canonical
            );
            let reactivated = projectional_editor_from_snapshot(&decoded_v8)
                .expect("cold restore canonical bootstrap v8");
            assert_eq!(
                reactivated
                    .coordinator()
                    .presentation_session()
                    .expect("reactivated native authority")
                    .accepted_state_for_current_input()
                    .map(geosolve_sketch::SketchAcceptedDocumentState::document),
                expected_accepted.as_ref()
            );
        }
    }
}
