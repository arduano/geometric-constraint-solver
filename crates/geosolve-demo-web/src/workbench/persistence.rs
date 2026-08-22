// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

use geosolve_constraint_editor::{
    AnnotationLayoutEntry, AnnotationLayoutKey, AnnotationLayoutState, AnnotationPlacement,
    EditorScene, RestoreCheckpoint, RetainedEditorCoordinator, SceneAnnotationGeometry,
    SceneAnnotationKind, SceneConstraintGlyph, SelectionItem, Viewport,
};
use geosolve_core::SolverConfig;
use geosolve_sketch::{
    DocumentConstraintId, DocumentDimensionId, DocumentId, DocumentSolveRequest, DocumentSourceId,
    ExternalSnapshotSet, ParameterBatch, PersistentId, RetainedSketchDocumentSession,
    SketchDocument, SketchLifecycleRevisionHighWater, SketchPersistentIdentityHighWater,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater, ComputedFeatureDocument,
    ComputedFeatureLifecycleHighWater,
};

#[cfg(target_arch = "wasm32")]
pub(crate) const STORAGE_KEY: &str = "geosolve.workbench.session.v7";
#[cfg(target_arch = "wasm32")]
pub(crate) const PREVIOUS_STORAGE_KEY: &str = "geosolve.workbench.session.v6";
#[cfg(target_arch = "wasm32")]
pub(crate) const OLDER_STORAGE_KEY: &str = "geosolve.workbench.session.v5";
#[cfg(target_arch = "wasm32")]
pub(crate) const OLDER_V4_STORAGE_KEY: &str = "geosolve.workbench.session.v4";
#[cfg(target_arch = "wasm32")]
pub(crate) const OLDER_V3_STORAGE_KEY: &str = "geosolve.workbench.session.v3";
#[cfg(target_arch = "wasm32")]
pub(crate) const OLDER_V2_STORAGE_KEY: &str = "geosolve.workbench.session.v2";
#[cfg(target_arch = "wasm32")]
pub(crate) const LEGACY_STORAGE_KEY: &str = "geosolve.workbench.session.v1";

/// Maximum accepted byte length of one decoded workbench workspace.
///
/// Nested sketch, feature, lineage and host-input codecs retain their own
/// narrower limits. This outer guard must run before the initial untyped JSON
/// parse so an oversized envelope cannot allocate an unbounded value tree.
const MAX_WORKSPACE_JSON_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceSnapshot {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lineage_session_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lineage_host_input_ledger_json: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_disposable_json_cache",
        skip_serializing_if = "Option::is_none"
    )]
    lineage_materialization_map_json: Option<String>,
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
    #[serde(default = "default_parameter_batch_json")]
    parameter_batch_json: String,
    #[serde(default = "default_external_snapshot_set_json")]
    external_snapshot_set_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_parameter_batch_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_external_snapshot_set_json: Option<String>,
    pub(crate) revisions: WorkspaceRevisions,
}

fn default_parameter_batch_json() -> String {
    ParameterBatch::default()
        .to_canonical_json()
        .expect("the built-in empty parameter batch must remain encodable")
}

fn default_external_snapshot_set_json() -> String {
    ExternalSnapshotSet::default()
        .to_canonical_json()
        .expect("the built-in empty external snapshot set must remain encodable")
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
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| value.as_str().map(str::to_owned)))
}

fn deserialize_disposable_json_cache<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| value.as_str().map(str::to_owned)))
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
}

fn decode_annotation_layout(input: &str) -> Option<AnnotationLayoutState> {
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
        let lineage_session_json = coordinator
            .lineage_session_json()
            .map_err(|error| error.to_string())?;
        let lineage_host_input_ledger_json = coordinator
            .lineage_host_input_ledger_json()
            .map_err(|error| error.to_string())?;
        let lineage_materialization_map_json = coordinator
            .lineage_materialization_map_json()
            .map_err(|error| error.to_string())?;
        Ok(Self::from_checkpoint(
            &checkpoint,
            coordinator.editor().annotation_layout(),
            coordinator.session().parameter_batch(),
            coordinator.session().latest_attempt_external_snapshot_set(),
            coordinator.session().accepted_parameter_batch(),
            coordinator.session().accepted_external_snapshot_set(),
            lineage_session_json,
            Some(lineage_host_input_ledger_json),
            lineage_materialization_map_json,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn from_checkpoint(
        checkpoint: &RestoreCheckpoint,
        annotation_layout: &AnnotationLayoutState,
        parameter_batch: &ParameterBatch,
        external_snapshot_set: &ExternalSnapshotSet,
        accepted_parameter_batch: Option<&ParameterBatch>,
        accepted_external_snapshot_set: Option<&ExternalSnapshotSet>,
        lineage_session_json: String,
        lineage_host_input_ledger_json: Option<String>,
        lineage_materialization_map_json: String,
    ) -> Self {
        let revisions = checkpoint.revisions();
        Self {
            version: 7,
            lineage_session_json: Some(lineage_session_json),
            lineage_host_input_ledger_json,
            lineage_materialization_map_json: Some(lineage_materialization_map_json),
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
            parameter_batch_json: parameter_batch
                .to_canonical_json()
                .expect("validated coordinator parameter input must remain encodable"),
            external_snapshot_set_json: external_snapshot_set
                .to_canonical_json()
                .expect("validated coordinator external input must remain encodable"),
            accepted_parameter_batch_json: accepted_parameter_batch.map(|batch| {
                batch
                    .to_canonical_json()
                    .expect("accepted parameter input must remain encodable")
            }),
            accepted_external_snapshot_set_json: accepted_external_snapshot_set.map(|snapshots| {
                snapshots
                    .to_canonical_json()
                    .expect("accepted external input must remain encodable")
            }),
            revisions: WorkspaceRevisions {
                design: revisions.design().get(),
                attempt: revisions.attempt().get(),
                accepted: revisions
                    .accepted()
                    .map(geosolve_sketch::SketchAcceptedRevision::get),
            },
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
        let mut snapshot = self.clone();
        if snapshot.lineage_session_json.is_none() {
            let coordinator = coordinator_from_snapshot(self)?;
            let lineage_session_json = coordinator
                .lineage_session_json()
                .map_err(|error| error.to_string())?;
            snapshot.lineage_host_input_ledger_json = Some(
                coordinator
                    .lineage_host_input_ledger_json()
                    .map_err(|error| error.to_string())?,
            );
            snapshot.lineage_materialization_map_json = Some(
                RetainedEditorCoordinator::lineage_materialization_map_json_for_session(
                    &lineage_session_json,
                )
                .map_err(|error| error.to_string())?,
            );
            snapshot.lineage_session_json = Some(lineage_session_json);
        }
        serde_json::to_string(&snapshot).map_err(|error| error.to_string())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed five-version migration matrix is clearer when audited in one dispatch"
    )]
    pub(crate) fn decode(input: &str) -> Result<Self, String> {
        if input.len() > MAX_WORKSPACE_JSON_BYTES {
            return Err(format!(
                "workbench snapshot exceeds the {MAX_WORKSPACE_JSON_BYTES}-byte limit"
            ));
        }
        let version = serde_json::from_str::<serde_json::Value>(input)
            .map_err(|error| error.to_string())?
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "workbench snapshot version is missing".to_owned())?;
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
                let has_accepted = accepted.is_some();
                let sketch_identity_high_water =
                    derive_sketch_identity_high_water(&design, accepted.as_ref())?;
                Self {
                    version: 7,
                    lineage_session_json: None,
                    lineage_host_input_ledger_json: None,
                    lineage_materialization_map_json: None,
                    design,
                    accepted,
                    accepted_belongs_to_current_design: false,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
                    parameter_batch_json: default_parameter_batch_json(),
                    external_snapshot_set_json: default_external_snapshot_set_json(),
                    accepted_parameter_batch_json: has_accepted.then(default_parameter_batch_json),
                    accepted_external_snapshot_set_json: has_accepted
                        .then(default_external_snapshot_set_json),
                    revisions: legacy.revisions,
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
                let has_accepted = legacy.accepted.is_some();
                Self {
                    version: 7,
                    lineage_session_json: None,
                    lineage_host_input_ledger_json: None,
                    lineage_materialization_map_json: None,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: false,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
                    parameter_batch_json: default_parameter_batch_json(),
                    external_snapshot_set_json: default_external_snapshot_set_json(),
                    accepted_parameter_batch_json: has_accepted.then(default_parameter_batch_json),
                    accepted_external_snapshot_set_json: has_accepted
                        .then(default_external_snapshot_set_json),
                    revisions: legacy.revisions,
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
                let has_accepted = legacy.accepted.is_some();
                Self {
                    version: 7,
                    lineage_session_json: None,
                    lineage_host_input_ledger_json: None,
                    lineage_materialization_map_json: None,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: legacy.accepted_belongs_to_current_design,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
                    parameter_batch_json: default_parameter_batch_json(),
                    external_snapshot_set_json: default_external_snapshot_set_json(),
                    accepted_parameter_batch_json: has_accepted.then(default_parameter_batch_json),
                    accepted_external_snapshot_set_json: has_accepted
                        .then(default_external_snapshot_set_json),
                    revisions: legacy.revisions,
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
                let has_accepted = legacy.accepted.is_some();
                Self {
                    version: 7,
                    lineage_session_json: None,
                    lineage_host_input_ledger_json: None,
                    lineage_materialization_map_json: None,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: legacy.accepted_belongs_to_current_design,
                    sketch_identity_high_water,
                    features_json: legacy.features_json,
                    feature_lifecycle_high_water: legacy.feature_lifecycle_high_water,
                    computed_evaluation_high_water: legacy.computed_evaluation_high_water,
                    annotation_layout_json: None,
                    parameter_batch_json: default_parameter_batch_json(),
                    external_snapshot_set_json: default_external_snapshot_set_json(),
                    accepted_parameter_batch_json: has_accepted.then(default_parameter_batch_json),
                    accepted_external_snapshot_set_json: has_accepted
                        .then(default_external_snapshot_set_json),
                    revisions: legacy.revisions,
                }
                .validated()
            }
            5 => {
                let mut snapshot: Self =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                snapshot.version = 7;
                snapshot.lineage_session_json = None;
                snapshot.lineage_host_input_ledger_json = None;
                snapshot.lineage_materialization_map_json = None;
                snapshot.annotation_layout_json = None;
                if snapshot.accepted.is_some() {
                    snapshot.accepted_parameter_batch_json =
                        Some(snapshot.parameter_batch_json.clone());
                    snapshot.accepted_external_snapshot_set_json =
                        Some(snapshot.external_snapshot_set_json.clone());
                }
                snapshot.validated()
            }
            6 => {
                let mut snapshot: Self =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                snapshot.version = 7;
                snapshot.lineage_session_json = None;
                snapshot.lineage_host_input_ledger_json = None;
                snapshot.lineage_materialization_map_json = None;
                if snapshot.accepted.is_some() {
                    snapshot.accepted_parameter_batch_json =
                        Some(snapshot.parameter_batch_json.clone());
                    snapshot.accepted_external_snapshot_set_json =
                        Some(snapshot.external_snapshot_set_json.clone());
                }
                snapshot.validated()
            }
            7 => Self::decode_v7(input),
            _ => Err("unsupported workbench snapshot version".into()),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn decode_v7(input: &str) -> Result<Self, String> {
        const V7_FIELDS: &[&str] = &[
            "version",
            "lineage_session_json",
            "lineage_host_input_ledger_json",
            "lineage_materialization_map_json",
            "design",
            "accepted",
            "accepted_belongs_to_current_design",
            "sketch_identity_high_water",
            "features_json",
            "feature_lifecycle_high_water",
            "computed_evaluation_high_water",
            "annotation_layout_json",
            "parameter_batch_json",
            "external_snapshot_set_json",
            "accepted_parameter_batch_json",
            "accepted_external_snapshot_set_json",
            "revisions",
        ];
        let value =
            serde_json::from_str::<serde_json::Value>(input).map_err(|error| error.to_string())?;
        let object = value
            .as_object()
            .ok_or_else(|| "workspace v7 must be a JSON object".to_owned())?;
        if let Some(field) = object
            .keys()
            .find(|field| !V7_FIELDS.contains(&field.as_str()))
        {
            return Err(format!("workspace v7 contains unknown field `{field}`"));
        }
        let lineage_session_json = object
            .get("lineage_session_json")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "workspace v7 is missing authoritative lineage".to_owned())?
            .to_owned();
        let lineage_host_input_ledger_json = object
            .get("lineage_host_input_ledger_json")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let materialized_intent =
            RetainedEditorCoordinator::lineage_materialization_checkpoint(&lineage_session_json)
                .map_err(|error| error.to_string())?;
        let accepted_materialized_intent =
            RetainedEditorCoordinator::lineage_last_accepted_materialization_checkpoint(
                &lineage_session_json,
            )
            .map_err(|error| error.to_string())?;
        let lineage_materialization_map_json =
            RetainedEditorCoordinator::lineage_materialization_map_json_for_session(
                &lineage_session_json,
            )
            .map_err(|error| error.to_string())?;
        let annotation_layout = object
            .get("annotation_layout_json")
            .and_then(serde_json::Value::as_str)
            .and_then(decode_annotation_layout)
            .unwrap_or_default();
        let parameter_batch = object
            .get("parameter_batch_json")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "workspace v7 is missing exact parameter input".to_owned())
            .and_then(|json| ParameterBatch::from_json(json).map_err(|error| error.to_string()))?;
        let external_snapshot_set = object
            .get("external_snapshot_set_json")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "workspace v7 is missing exact external snapshot input".to_owned())
            .and_then(|json| {
                ExternalSnapshotSet::from_json(json).map_err(|error| error.to_string())
            })?;
        let accepted_parameter_batch = if accepted_materialized_intent.is_some() {
            Some(
                object
                    .get("accepted_parameter_batch_json")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        "workspace v7 is missing exact accepted parameter input".to_owned()
                    })
                    .and_then(|json| {
                        ParameterBatch::from_json(json).map_err(|error| error.to_string())
                    })?,
            )
        } else {
            None
        };
        let accepted_external_snapshot_set = if accepted_materialized_intent.is_some() {
            Some(
                object
                    .get("accepted_external_snapshot_set_json")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        "workspace v7 is missing exact accepted external snapshot input".to_owned()
                    })
                    .and_then(|json| {
                        ExternalSnapshotSet::from_json(json).map_err(|error| error.to_string())
                    })?,
            )
        } else {
            None
        };
        RetainedEditorCoordinator::validate_lineage_accepted_external_inputs(
            &lineage_session_json,
            accepted_parameter_batch.as_ref(),
            accepted_external_snapshot_set.as_ref(),
        )
        .map_err(|error| error.to_string())?;
        let current_is_accepted = lineage_current_is_accepted(&lineage_session_json)?;
        // Both branches consume canonical owning-domain accepted bytes from
        // cold-authenticated lineage evaluation. Current authority is tied to
        // the exact retained lineage identity and current inputs; a rejected
        // current program instead carries the exact older accepted lineage
        // evidence used for its visible fallback scene.
        let accepted_evidence = if current_is_accepted {
            Some(
                RetainedEditorCoordinator::lineage_cold_current_accepted_evidence_checkpoint(
                    &lineage_session_json,
                    lineage_host_input_ledger_json.as_deref(),
                    &parameter_batch,
                    &external_snapshot_set,
                )
                .map_err(|error| error.to_string())?,
            )
        } else {
            RetainedEditorCoordinator::lineage_cold_historical_accepted_evidence_checkpoint(
                &lineage_session_json,
                lineage_host_input_ledger_json.as_deref(),
                accepted_parameter_batch.as_ref(),
                accepted_external_snapshot_set.as_ref(),
            )
            .map_err(|error| error.to_string())?
        };
        let materialized = cold_materialize_with_inputs(
            &materialized_intent,
            accepted_evidence.as_ref(),
            parameter_batch.clone(),
            external_snapshot_set.clone(),
            accepted_parameter_batch.clone(),
            accepted_external_snapshot_set.clone(),
        )?;
        let authority_expected = Self::from_checkpoint(
            &materialized_intent,
            &annotation_layout,
            &parameter_batch,
            &external_snapshot_set,
            accepted_parameter_batch.as_ref(),
            accepted_external_snapshot_set.as_ref(),
            lineage_session_json.clone(),
            lineage_host_input_ledger_json.clone(),
            lineage_materialization_map_json.clone(),
        );
        let expected_accepted_parameter_batch = if current_is_accepted {
            Some(&parameter_batch)
        } else {
            accepted_parameter_batch.as_ref()
        };
        let expected_accepted_external_snapshot_set = if current_is_accepted {
            Some(&external_snapshot_set)
        } else {
            accepted_external_snapshot_set.as_ref()
        };
        let expected = Self::from_checkpoint(
            &materialized,
            &annotation_layout,
            &parameter_batch,
            &external_snapshot_set,
            expected_accepted_parameter_batch,
            expected_accepted_external_snapshot_set,
            lineage_session_json.clone(),
            lineage_host_input_ledger_json.clone(),
            lineage_materialization_map_json,
        );

        // Every flat v7 field is a disposable materialization cache. Keep it
        // only when it independently validates and is semantically compatible
        // with authoritative lineage. In particular, feature/sketch revision
        // counters may legitimately be rebased by cold materialization and are
        // therefore compared as monotonic high-waters rather than exact bytes.
        if let Ok(mut cached) = serde_json::from_value::<Self>(value) {
            cached.annotation_layout_json = encode_annotation_layout(&annotation_layout);
            if cached.cache_is_compatible_with(
                &authority_expected,
                &expected,
                &lineage_session_json,
            ) {
                return Ok(cached);
            }
        }
        expected.validated()
    }

    fn cache_is_compatible_with(
        &self,
        intent_expected: &Self,
        cold_expected: &Self,
        lineage_session_json: &str,
    ) -> bool {
        self.validate_flat_cache().is_ok()
            && self
                .lineage_materialization_map_json
                .as_deref()
                .is_some_and(|map| {
                    RetainedEditorCoordinator::validate_lineage_materialization_map_json(
                        lineage_session_json,
                        map,
                    )
                    .is_ok()
                })
            && self.lineage_materialization_map_json
                == intent_expected.lineage_materialization_map_json
            && self.lineage_host_input_ledger_json == intent_expected.lineage_host_input_ledger_json
            && self.parameter_batch_json == intent_expected.parameter_batch_json
            && self.external_snapshot_set_json == intent_expected.external_snapshot_set_json
            && self.accepted_parameter_batch_json == intent_expected.accepted_parameter_batch_json
            && self.accepted_external_snapshot_set_json
                == intent_expected.accepted_external_snapshot_set_json
            && design_intent_value(&self.design).is_ok_and(|cached| {
                design_intent_value(&intent_expected.design)
                    .is_ok_and(|materialized| cached == materialized)
            })
            && accepted_materialization_matches(
                self.accepted.as_ref(),
                cold_expected.accepted.as_ref(),
            )
            && feature_intent_value(&self.features_json).is_ok_and(|cached| {
                feature_intent_value(&intent_expected.features_json)
                    .is_ok_and(|materialized| cached == materialized)
            })
            && sketch_high_water_covers(
                &self.sketch_identity_high_water,
                &intent_expected.sketch_identity_high_water,
            )
            && feature_high_water_covers(
                self.feature_lifecycle_high_water,
                intent_expected.feature_lifecycle_high_water,
            )
            && self.computed_evaluation_high_water.next_revision.raw()
                >= intent_expected
                    .computed_evaluation_high_water
                    .next_revision
                    .raw()
            && revision_high_water_covers(self.revisions(), intent_expected.revisions())
            && lineage_current_is_accepted(lineage_session_json)
                .is_ok_and(|accepted| accepted == self.accepted_belongs_to_current_design)
            && self.cache_reconstructs_independently()
    }

    fn cache_reconstructs_independently(&self) -> bool {
        let Ok(session) =
            self.restore_session(DocumentSolveRequest::default(), SolverConfig::default())
        else {
            return false;
        };
        let Ok(features) = self.feature_document() else {
            return false;
        };
        RetainedEditorCoordinator::with_features_and_high_water(
            session,
            features,
            self.feature_lifecycle_high_water,
            self.computed_evaluation_high_water,
        )
        .is_ok()
    }

    fn validated(self) -> Result<Self, String> {
        if self.version != 7 {
            return Err("unsupported workbench snapshot version".into());
        }
        self.validate_flat_cache()?;
        Ok(self)
    }

    fn validate_flat_cache(&self) -> Result<(), String> {
        if self.accepted_belongs_to_current_design && self.accepted.is_none() {
            return Err("current-design accepted provenance requires an accepted payload".into());
        }
        let parameter_batch = self.parameter_batch()?;
        if parameter_batch
            .to_canonical_json()
            .map_err(|error| error.to_string())?
            != self.parameter_batch_json
        {
            return Err("workspace parameter input is not canonical".into());
        }
        let external_snapshot_set = self.external_snapshot_set()?;
        if external_snapshot_set
            .to_canonical_json()
            .map_err(|error| error.to_string())?
            != self.external_snapshot_set_json
        {
            return Err("workspace external snapshot input is not canonical".into());
        }
        match (
            self.accepted.as_ref(),
            self.accepted_parameter_batch_json.as_deref(),
            self.accepted_external_snapshot_set_json.as_deref(),
        ) {
            (Some(_), Some(parameters), Some(snapshots)) => {
                let parameters =
                    ParameterBatch::from_json(parameters).map_err(|error| error.to_string())?;
                if parameters
                    .to_canonical_json()
                    .map_err(|error| error.to_string())?
                    != self
                        .accepted_parameter_batch_json
                        .as_deref()
                        .unwrap_or_default()
                {
                    return Err("workspace accepted parameter input is not canonical".into());
                }
                let snapshots =
                    ExternalSnapshotSet::from_json(snapshots).map_err(|error| error.to_string())?;
                if snapshots
                    .to_canonical_json()
                    .map_err(|error| error.to_string())?
                    != self
                        .accepted_external_snapshot_set_json
                        .as_deref()
                        .unwrap_or_default()
                {
                    return Err("workspace accepted external input is not canonical".into());
                }
            }
            (None, None, None) => {}
            (Some(_), _, _) => {
                return Err(
                    "accepted workspace authority requires exact accepted host inputs".into(),
                );
            }
            (None, _, _) => {
                return Err(
                    "workspace stores accepted host inputs without accepted authority".into(),
                );
            }
        }
        let design = self.design_document()?;
        let accepted = self.accepted_document()?;
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

    fn parameter_batch(&self) -> Result<ParameterBatch, String> {
        ParameterBatch::from_json(&self.parameter_batch_json).map_err(|error| error.to_string())
    }

    fn external_snapshot_set(&self) -> Result<ExternalSnapshotSet, String> {
        ExternalSnapshotSet::from_json(&self.external_snapshot_set_json)
            .map_err(|error| error.to_string())
    }

    fn accepted_parameter_batch(&self) -> Result<Option<ParameterBatch>, String> {
        self.accepted_parameter_batch_json
            .as_deref()
            .map(ParameterBatch::from_json)
            .transpose()
            .map_err(|error| error.to_string())
    }

    fn accepted_external_snapshot_set(&self) -> Result<Option<ExternalSnapshotSet>, String> {
        self.accepted_external_snapshot_set_json
            .as_deref()
            .map(ExternalSnapshotSet::from_json)
            .transpose()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn restore_session(
        &self,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<RetainedSketchDocumentSession, String> {
        let design = self.design_document()?;
        let parameter_batch = self.parameter_batch()?;
        let external_snapshot_set = self.external_snapshot_set()?;
        let accepted_parameter_batch = self.accepted_parameter_batch()?;
        let accepted_external_snapshot_set = self.accepted_external_snapshot_set()?;
        let mut restored = if let Some(accepted) = self.accepted_document()? {
            let accepted_parameter_batch = accepted_parameter_batch.ok_or_else(|| {
                "accepted workspace authority is missing exact parameter input".to_owned()
            })?;
            let accepted_external_snapshot_set = accepted_external_snapshot_set.ok_or_else(|| {
                "accepted workspace authority is missing exact external snapshot input".to_owned()
            })?;
            if self.accepted_belongs_to_current_design {
                RetainedSketchDocumentSession::restore_current_design_with_accepted_and_distinct_inputs(
                    design,
                    accepted,
                    self.revisions(),
                    parameter_batch,
                    external_snapshot_set,
                    accepted_parameter_batch,
                    accepted_external_snapshot_set,
                    request,
                    config,
                )
            } else {
                RetainedSketchDocumentSession::restore_design_with_accepted_and_distinct_inputs(
                    design,
                    accepted,
                    self.revisions(),
                    parameter_batch,
                    external_snapshot_set,
                    accepted_parameter_batch,
                    accepted_external_snapshot_set,
                    request,
                    config,
                )
            }
        } else {
            RetainedSketchDocumentSession::restore_design_with_inputs(
                design,
                self.revisions(),
                parameter_batch,
                external_snapshot_set,
                request,
                config,
            )
        }
        .map_err(|error| error.to_string())?;
        restored
            .retain_persistent_identity_high_water(&self.sketch_identity_high_water)
            .map_err(|error| error.to_string())?;
        Ok(restored)
    }
}

fn feature_intent_value(input: &str) -> Result<serde_json::Value, String> {
    let mut value =
        serde_json::from_str::<serde_json::Value>(input).map_err(|error| error.to_string())?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "computed-feature cache must be an object".to_owned())?;
    // These fields authenticate one materialization attempt, not durable
    // feature intent. `ComputedFeatureDocument::from_json` has already checked
    // the original digest before this semantic comparison is reached.
    for field in ["revision", "next_feature_id", "next_corner_id", "digest"] {
        object.remove(field);
    }
    Ok(value)
}

fn design_intent_value(payload: &WorkspaceDocumentPayload) -> Result<serde_json::Value, String> {
    let mut value = serde_json::from_str::<serde_json::Value>(&payload.json)
        .map_err(|error| error.to_string())?;
    let document = match payload.encoding {
        WorkspaceDocumentEncoding::CanonicalV4 => value.as_object_mut(),
        WorkspaceDocumentEncoding::DraftV5 => value
            .as_object_mut()
            .and_then(|root| root.get_mut("document"))
            .and_then(serde_json::Value::as_object_mut),
    }
    .ok_or_else(|| "workspace design cache must be an object".to_owned())?;
    document.remove("next_id");
    Ok(serde_json::json!({
        "encoding": payload.encoding,
        "document": value,
    }))
}

fn accepted_materialization_matches(
    cached: Option<&WorkspaceDocumentPayload>,
    cold: Option<&WorkspaceDocumentPayload>,
) -> bool {
    match (cached, cold) {
        (Some(cached), Some(cold)) => design_intent_value(cached)
            .is_ok_and(|cached| design_intent_value(cold).is_ok_and(|cold| cached == cold)),
        (None, None) => true,
        _ => false,
    }
}

fn sketch_high_water_covers(
    retained: &SketchPersistentIdentityHighWater,
    required: &SketchPersistentIdentityHighWater,
) -> bool {
    retained
        .merged(required)
        .is_ok_and(|merged| merged == *retained)
}

const fn feature_high_water_covers(
    retained: ComputedFeatureLifecycleHighWater,
    required: ComputedFeatureLifecycleHighWater,
) -> bool {
    retained.revision.raw() >= required.revision.raw()
        && retained.allocator.next_feature_id.raw() >= required.allocator.next_feature_id.raw()
        && retained.allocator.next_corner_id.raw() >= required.allocator.next_corner_id.raw()
}

const fn revision_high_water_covers(
    retained: SketchLifecycleRevisionHighWater,
    required: SketchLifecycleRevisionHighWater,
) -> bool {
    if retained.design().get() < required.design().get()
        || retained.attempt().get() < required.attempt().get()
    {
        return false;
    }
    match (retained.accepted(), required.accepted()) {
        (_, None) => true,
        (Some(retained), Some(required)) => retained.get() >= required.get(),
        (None, Some(_)) => false,
    }
}

fn lineage_current_is_accepted(input: &str) -> Result<bool, String> {
    let value =
        serde_json::from_str::<serde_json::Value>(input).map_err(|error| error.to_string())?;
    let disposition = value
        .get("latest_attempt")
        .and_then(serde_json::Value::as_object)
        .and_then(|attempt| attempt.get("disposition"))
        .and_then(serde_json::Value::as_str);
    Ok(matches!(disposition, Some("accepted")))
}

#[allow(clippy::too_many_lines)]
fn cold_materialize_with_inputs(
    intent: &RestoreCheckpoint,
    accepted_evidence: Option<&RestoreCheckpoint>,
    parameter_batch: ParameterBatch,
    external_snapshot_set: ExternalSnapshotSet,
    accepted_parameter_batch: Option<ParameterBatch>,
    accepted_external_snapshot_set: Option<ExternalSnapshotSet>,
) -> Result<RestoreCheckpoint, String> {
    let mut design = if intent.design_uses_draft_v5() {
        SketchDocument::from_draft_v5_json(intent.design_json())
    } else {
        SketchDocument::from_json(intent.design_json())
    }
    .map_err(|error| error.to_string())?;
    design
        .retain_persistent_identity_high_water(intent.sketch_identity_high_water())
        .map_err(|error| error.to_string())?;
    let mut session = if let Some(accepted_evidence) = accepted_evidence {
        let accepted_parameter_batch = accepted_parameter_batch.ok_or_else(|| {
            "accepted lineage authority is missing exact parameter input".to_owned()
        })?;
        let accepted_external_snapshot_set = accepted_external_snapshot_set.ok_or_else(|| {
            "accepted lineage authority is missing exact external snapshot input".to_owned()
        })?;
        let accepted_json = accepted_evidence.accepted_json().ok_or_else(|| {
            "cold-authenticated accepted lineage evidence is missing sketch bytes".to_owned()
        })?;
        let mut accepted_design = if accepted_evidence.accepted_uses_draft_v5() {
            SketchDocument::from_draft_v5_json(accepted_json)
        } else {
            SketchDocument::from_json(accepted_json)
        }
        .map_err(|error| error.to_string())?;
        accepted_design
            .retain_persistent_identity_high_water(accepted_evidence.sketch_identity_high_water())
            .map_err(|error| error.to_string())?;
        let same_retained_program = intent.design_uses_draft_v5()
            == accepted_evidence.design_uses_draft_v5()
            && intent.design_json() == accepted_evidence.design_json();
        if same_retained_program {
            RetainedSketchDocumentSession::restore_current_design_with_accepted_and_distinct_inputs(
                design,
                accepted_design,
                intent.revisions(),
                parameter_batch,
                external_snapshot_set,
                accepted_parameter_batch,
                accepted_external_snapshot_set,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
        } else {
            RetainedSketchDocumentSession::restore_design_with_accepted_and_distinct_inputs(
                design,
                accepted_design,
                intent.revisions(),
                parameter_batch,
                external_snapshot_set,
                accepted_parameter_batch,
                accepted_external_snapshot_set,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
        }
    } else {
        RetainedSketchDocumentSession::restore_design_with_inputs(
            design,
            intent.revisions(),
            parameter_batch,
            external_snapshot_set,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
    }
    .map_err(|error| error.to_string())?;
    session
        .retain_persistent_identity_high_water(intent.sketch_identity_high_water())
        .map_err(|error| error.to_string())?;
    let features = ComputedFeatureDocument::from_json(intent.feature_json())
        .map_err(|error| error.to_string())?;
    let coordinator = RetainedEditorCoordinator::with_features_and_high_water(
        session,
        features,
        intent.feature_lifecycle_high_water(),
        intent.computed_evaluation_high_water(),
    )
    .map_err(|error| error.to_string())?;
    coordinator
        .persistence_checkpoint()
        .map_err(|error| error.to_string())
}

pub(crate) fn coordinator_from_snapshot(
    snapshot: &WorkspaceSnapshot,
) -> Result<RetainedEditorCoordinator, String> {
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
    if let Some(lineage_session_json) = &snapshot.lineage_session_json {
        if let Some(host_input_ledger_json) = &snapshot.lineage_host_input_ledger_json {
            coordinator
                .restore_lineage_session_and_host_input_ledger_json(
                    lineage_session_json,
                    host_input_ledger_json,
                )
                .map_err(|error| error.to_string())?;
        } else {
            coordinator
                .restore_lineage_session_json(lineage_session_json)
                .map_err(|error| error.to_string())?;
        }
    }
    let layout = compatible_annotation_layout(&coordinator, &cached_layout);
    coordinator.editor_mut().restore_annotation_layout(layout);
    Ok(coordinator)
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
    let mut snapshot = WorkspaceSnapshot::from_coordinator(coordinator)?;
    // A reproduction capsule carries authoritative/reconstructable workspace state, not the
    // disposable per-viewport annotation cache retained by ordinary local workspace saves.
    snapshot.annotation_layout_json = None;
    let workspace = snapshot.encode()?;
    crate::reproduction::encode_workspace(&workspace).map_err(|error| error.to_string())
}

pub(crate) fn coordinator_from_reproduction_payload(
    payload: &str,
) -> Result<RetainedEditorCoordinator, String> {
    let workspace =
        crate::reproduction::decode_workspace(payload).map_err(|error| error.to_string())?;
    let mut snapshot = WorkspaceSnapshot::decode(&workspace)?;
    // Older capsules may have embedded this optional presentation cache. Ignore it so restoration
    // always recomputes placement from the accepted scene under the receiving viewport.
    snapshot.annotation_layout_json = None;
    coordinator_from_snapshot(&snapshot)
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
        ComputedEdgeGeometry, ComputedFeatureEvaluationState, ComputedSceneState, ConstraintIntent,
        EditorScene, FeatureAuthoringCandidate, FeatureAuthoringOutcome, FeatureAuthoringState,
        FeatureAuthoringTool, Modifiers, PointerInput, RetainedEditorCoordinator,
        SceneAnnotationGeometry, SceneAnnotationKind, SceneConstraintGlyph, ScreenPoint,
        SelectionItem, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        AlphaScenarioIds, AlphaScenarioKind, ContactStateEdit, CurveDefinition, CurveId, CurveSpan,
        DesignPointId, DocumentBSplineForm, DocumentCenterRef, DocumentCommandEffect,
        DocumentConstraintDefinition, DocumentDirectionSense, DocumentEdit, DocumentError,
        DocumentExternalPointRef, DocumentId, DocumentLineSupportRef, DocumentNativeLineFilletIds,
        DocumentObjectId, DocumentParameterKind, DocumentParameterTarget, DocumentSolveRequest,
        ExternalFeatureKindV1, ExternalSnapshotDigest, ExternalSnapshotEntry,
        ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1, ExternalSnapshotSet, GeometryRole,
        ParameterBatch, ParameterBatchEntry, ParameterValue, PersistentId,
        RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, alpha_scenario,
    };

    use super::{
        MAX_WORKSPACE_JSON_BYTES, WorkspaceSnapshot, annotation_kind_key,
        coordinator_from_reproduction_payload, coordinator_from_snapshot,
        default_evaluation_high_water, derive_sketch_identity_high_water, parse_annotation_kind,
        reproduction_payload_from_coordinator,
    };

    fn publish_native_fillet(
        coordinator: &mut RetainedEditorCoordinator,
        corner: DesignPointId,
    ) -> DocumentNativeLineFilletIds {
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("native-Fillet authoring snapshot");
        let accepted = snapshot.sketch_document().clone();
        let mut authoring = FeatureAuthoringState::default();
        assert!(matches!(
            authoring.activate(&snapshot, &accepted, FeatureAuthoringTool::Fillet, &[]),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));
        assert!(matches!(
            authoring.set_options(
                &snapshot,
                geosolve_constraint_editor::FeatureAuthoringOptions {
                    fillet_radius: Some(0.5),
                    ..geosolve_constraint_editor::FeatureAuthoringOptions::default()
                },
            ),
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        let transaction = coordinator
            .transact_feature_authoring_pick_items(
                &mut authoring,
                &[(SelectionItem::Point(corner), None)],
                "workspace native Fillet",
            )
            .expect("native-Fillet candidate transaction");
        let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } = transaction.outcome
        else {
            panic!("line-line corner must produce a native-Fillet preview");
        };
        let preview = transaction.preview.expect("held native-Fillet preview");
        coordinator
            .native_feature_authoring_availability(preview.token, &candidate)
            .expect("native-Fillet publication availability");
        coordinator
            .apply_feature_authoring_native_profile(preview.token, &candidate)
            .expect("native-Fillet publication")
            .value
    }

    fn current_native_fillet_corner() -> (RetainedEditorCoordinator, DocumentNativeLineFilletIds) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let start = document
            .add_point("horizontal start", [0.0, 0.0])
            .expect("start");
        let corner = document
            .add_point("sharp corner", [4.0, 0.0])
            .expect("corner");
        let end = document.add_point("vertical end", [4.0, 4.0]).expect("end");
        document
            .add_curve(
                "horizontal parent",
                CurveDefinition::Line {
                    start,
                    end: corner,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("horizontal line");
        document
            .add_curve(
                "vertical parent",
                CurveDefinition::Line {
                    start: corner,
                    end,
                    branch_direction: [0.0, 1.0],
                },
            )
            .expect("vertical line");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .expect("accepted line corner");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let ids = publish_native_fillet(&mut coordinator, corner);
        (coordinator, ids)
    }

    #[test]
    fn workspace_decode_rejects_an_oversized_envelope_before_json_parsing() {
        let oversized = " ".repeat(MAX_WORKSPACE_JSON_BYTES + 1);
        assert_eq!(
            WorkspaceSnapshot::decode(&oversized),
            Err(format!(
                "workbench snapshot exceeds the {MAX_WORKSPACE_JSON_BYTES}-byte limit"
            ))
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one frozen matrix keeps all six historical schemas, migration authority, and cold-v7 evidence directly comparable"
    )]
    fn m83_w9_frozen_v1_v6_fixtures_migrate_through_one_imported_baseline_and_cold_v7() {
        const FIXTURES: &str =
            include_str!("../../tests/fixtures/m83_workspace_migrations_v1_v6.jsonl");
        // Each row was captured with the exact outer field language introduced
        // by the named historical source. Nested canonical-v4 sketch and
        // computed-feature-v1 bytes are themselves strict frozen codecs.
        const HISTORICAL_SOURCES: [&str; 6] = [
            "ba711c3", "d9cef77", "c1b0336", "941177c", "4b16db3", "8b5cfa5",
        ];
        const HISTORICAL_FIELDS: [&[&str]; 6] = [
            &["accepted_json", "design_json", "revisions", "version"],
            &["accepted", "design", "revisions", "version"],
            &[
                "accepted",
                "accepted_belongs_to_current_design",
                "design",
                "revisions",
                "version",
            ],
            &[
                "accepted",
                "accepted_belongs_to_current_design",
                "computed_evaluation_high_water",
                "design",
                "feature_lifecycle_high_water",
                "features_json",
                "revisions",
                "version",
            ],
            &[
                "accepted",
                "accepted_belongs_to_current_design",
                "computed_evaluation_high_water",
                "design",
                "feature_lifecycle_high_water",
                "features_json",
                "revisions",
                "sketch_identity_high_water",
                "version",
            ],
            &[
                "accepted",
                "accepted_belongs_to_current_design",
                "annotation_layout_json",
                "computed_evaluation_high_water",
                "design",
                "feature_lifecycle_high_water",
                "features_json",
                "revisions",
                "sketch_identity_high_water",
                "version",
            ],
        ];

        let fixtures = FIXTURES.lines().collect::<Vec<_>>();
        assert_eq!(fixtures.len(), 6, "the frozen matrix must remain v1-v6");
        for (offset, input) in fixtures.into_iter().enumerate() {
            let legacy_version = u32::try_from(offset + 1).expect("version fits u32");
            let source = HISTORICAL_SOURCES[offset];
            let raw: serde_json::Value = serde_json::from_str(input)
                .unwrap_or_else(|error| panic!("workspace v{legacy_version} ({source}): {error}"));
            assert_eq!(raw["version"], serde_json::json!(legacy_version));
            let mut actual_fields = raw
                .as_object()
                .expect("frozen workspace object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>();
            actual_fields.sort_unstable();
            assert_eq!(
                actual_fields, HISTORICAL_FIELDS[offset],
                "workspace v{legacy_version} must retain the exact field set emitted at {source}"
            );
            let mut unknown_field = raw.clone();
            unknown_field
                .as_object_mut()
                .expect("frozen workspace object")
                .insert("not_in_historical_schema".into(), serde_json::json!(true));
            assert!(
                WorkspaceSnapshot::decode(
                    &serde_json::to_string(&unknown_field).expect("unknown-field probe")
                )
                .is_err(),
                "workspace v{legacy_version} decoder admitted a foreign outer field"
            );

            let retained_json = if legacy_version == 1 {
                raw["design_json"].as_str().expect("v1 retained sketch")
            } else {
                raw["design"]["json"]
                    .as_str()
                    .expect("v2-v6 retained sketch")
            };
            let accepted_json = if legacy_version == 1 {
                raw["accepted_json"].as_str().expect("v1 accepted sketch")
            } else {
                raw["accepted"]["json"]
                    .as_str()
                    .expect("v2-v6 accepted sketch")
            };
            assert_ne!(
                retained_json, accepted_json,
                "fixture must exercise retained failure over older accepted authority"
            );

            let migrated = WorkspaceSnapshot::decode(input).unwrap_or_else(|error| {
                panic!("workspace v{legacy_version} ({source}) did not migrate: {error}")
            });
            assert_eq!(migrated.version, 7);
            assert!(migrated.lineage_session_json.is_none());
            assert!(!migrated.accepted_belongs_to_current_design);
            assert_eq!(migrated.design.json, retained_json);
            assert_eq!(
                migrated
                    .accepted
                    .as_ref()
                    .map(|payload| payload.json.as_str()),
                Some(accepted_json)
            );
            assert_eq!(migrated.revisions.design, 2);
            assert_eq!(migrated.revisions.attempt, 2);
            assert_eq!(migrated.revisions.accepted, Some(1));
            let retained = migrated.design_document().expect("retained document");
            let accepted = migrated
                .accepted_document()
                .expect("accepted document decode")
                .expect("older accepted document");
            assert_eq!(retained.constraints().len(), 2);
            assert_eq!(accepted.constraints().len(), 1);
            assert_eq!(retained.id(), accepted.id());
            let restored_flat = migrated
                .restore_session(DocumentSolveRequest::default(), SolverConfig::default())
                .expect("restore retained failure and older accepted scene");
            assert!(restored_flat.accepted_state().is_some());
            assert!(restored_flat.accepted_state_for_current_input().is_none());

            if legacy_version < 4 {
                assert!(
                    migrated
                        .feature_document()
                        .expect("derived empty feature document")
                        .features()
                        .is_empty()
                );
                assert_eq!(
                    migrated.computed_evaluation_high_water,
                    default_evaluation_high_water()
                );
            } else {
                assert_eq!(
                    migrated.features_json,
                    raw["features_json"].as_str().expect("legacy feature bytes")
                );
                let features = migrated
                    .feature_document()
                    .expect("preserved computed feature document");
                assert_eq!(features.features().len(), 1);
                assert_eq!(features.features()[0].label, "fixture computed Fillet");
                assert_eq!(
                    serde_json::to_value(migrated.feature_lifecycle_high_water)
                        .expect("feature high-water value"),
                    raw["feature_lifecycle_high_water"]
                );
                assert_eq!(
                    serde_json::to_value(migrated.computed_evaluation_high_water)
                        .expect("evaluation high-water value"),
                    raw["computed_evaluation_high_water"]
                );
            }
            if legacy_version >= 5 {
                assert_eq!(
                    serde_json::to_value(&migrated.sketch_identity_high_water)
                        .expect("sketch high-water value"),
                    raw["sketch_identity_high_water"]
                );
            } else {
                assert_eq!(
                    migrated.sketch_identity_high_water,
                    derive_sketch_identity_high_water(&migrated.design, migrated.accepted.as_ref())
                        .expect("derived historical sketch high-water")
                );
            }
            assert_eq!(
                migrated.annotation_layout().entries().len(),
                usize::from(legacy_version == 6),
                "only workspace v6 can carry annotation placement"
            );

            let imported = coordinator_from_snapshot(&migrated)
                .expect("construct one honest imported-baseline coordinator");
            let lineage: serde_json::Value =
                serde_json::from_str(&imported.lineage_json().expect("canonical imported lineage"))
                    .expect("lineage value");
            let steps = lineage["steps"].as_array().expect("lineage steps");
            assert_eq!(steps.len(), 1, "migration cannot invent recipe history");
            assert_eq!(steps[0]["key"], serde_json::json!("imported-baseline"));
            assert_eq!(
                steps[0]["action"]["kind"],
                serde_json::json!("imported_baseline")
            );
            assert_eq!(
                imported.history_len(),
                1,
                "the baseline position is retained without a fictional pre-import action"
            );
            assert_eq!(imported.history_cursor(), 0);
            assert!(!imported.can_undo());
            assert!(!imported.can_redo());

            let encoded_v7 = migrated.encode().expect("canonical v7 migration output");
            let encoded_value: serde_json::Value =
                serde_json::from_str(&encoded_v7).expect("v7 value");
            assert_eq!(encoded_value["version"], serde_json::json!(7));
            assert!(encoded_value["lineage_session_json"].is_string());
            let canonical_v7 = WorkspaceSnapshot::decode(&encoded_v7).unwrap_or_else(|error| {
                panic!(
                    "workspace v{legacy_version} re-encoded v7 did not cold-authenticate: {error}"
                )
            });
            let canonical_accepted = canonical_v7
                .accepted_document()
                .expect("canonical v7 accepted document")
                .expect("canonical v7 older accepted authority");
            assert_eq!(
                canonical_accepted.constraints(),
                accepted.constraints(),
                "v7 re-encode changed older accepted geometry from workspace v{legacy_version}"
            );
            assert_eq!(canonical_accepted.points(), accepted.points());
            assert_eq!(canonical_accepted.curves(), accepted.curves());

            let mut cache_free = encoded_value;
            let object = cache_free.as_object_mut().expect("workspace v7 object");
            for field in [
                "lineage_materialization_map_json",
                "design",
                "accepted",
                "accepted_belongs_to_current_design",
                "sketch_identity_high_water",
                "features_json",
                "feature_lifecycle_high_water",
                "computed_evaluation_high_water",
                "revisions",
            ] {
                object.remove(field);
            }
            let cold = WorkspaceSnapshot::decode(
                &serde_json::to_string(&cache_free).expect("cache-free v7 workspace"),
            )
            .expect("cache-free cold lineage reconstruction");
            assert_eq!(cold.design.json, retained_json);
            let cold_accepted = cold
                .accepted_document()
                .expect("cache-free accepted document")
                .expect("cache-free older accepted authority");
            assert_eq!(cold_accepted.constraints(), accepted.constraints());
            assert_eq!(cold_accepted.points(), accepted.points());
            assert_eq!(cold_accepted.curves(), accepted.curves());
            assert_eq!(
                cold.features_json, canonical_v7.features_json,
                "feature intent changed during cache-free reload"
            );
            assert_eq!(
                cold.sketch_identity_high_water,
                canonical_v7.sketch_identity_high_water
            );
            assert_eq!(
                cold.feature_lifecycle_high_water,
                canonical_v7.feature_lifecycle_high_water
            );
            assert_eq!(
                cold.computed_evaluation_high_water,
                canonical_v7.computed_evaluation_high_water
            );
            assert_eq!(
                cold.annotation_layout().entries(),
                canonical_v7.annotation_layout().entries()
            );
            let restored_cold =
                coordinator_from_snapshot(&cold).expect("cache-free v7 coordinator reconstruction");
            assert!(restored_cold.session().accepted_state().is_some());
            assert!(
                restored_cold
                    .session()
                    .accepted_state_for_current_input()
                    .is_none()
            );
            let cold_lineage: serde_json::Value =
                serde_json::from_str(&restored_cold.lineage_json().expect("cold imported lineage"))
                    .expect("cold lineage value");
            assert_eq!(cold_lineage["steps"].as_array().map(Vec::len), Some(1));
        }
    }

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

    fn external_point_entry(
        binding: geosolve_sketch::DocumentExternalBindingId,
        source_revision: u64,
        position: [f64; 2],
    ) -> ExternalSnapshotEntry {
        ExternalSnapshotEntry {
            binding,
            source_revision,
            source_digest: ExternalSnapshotDigest::from_bytes(
                [u8::try_from(source_revision).expect("test source revision fits in one byte"); 32],
            ),
            feature: ExternalSnapshotFeatureV1::Point {
                position,
                scale: 1.0,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn workspace_v7_cold_recovery_keeps_exact_current_host_inputs_and_undo_uses_them() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("parameter binding");
        let external_point = document
            .add_point("external point", [1.0, 2.0])
            .expect("point");
        let binding = document
            .add_external_binding("external datum", ExternalFeatureKindV1::Point, None)
            .expect("external binding");
        document
            .add_constraint(
                "external coincidence",
                DocumentConstraintDefinition::ExternalPointCoincident {
                    point: external_point,
                    external: DocumentExternalPointRef { binding },
                },
            )
            .expect("external constraint");
        let initial_parameters = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(4.0),
            }],
        )
        .expect("initial parameters");
        let initial_snapshots =
            ExternalSnapshotSet::new(1, vec![external_point_entry(binding, 1, [1.0, 2.0])])
                .expect("initial snapshots");
        let session = RetainedSketchDocumentSession::new_with_inputs(
            document,
            initial_parameters,
            initial_snapshots,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("initial session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "history point".into(),
                    position: [9.0, 7.0],
                },
            )
            .expect("history edit");
        let history_len_before_host_inputs = coordinator.history_len();
        let history_cursor_before_host_inputs = coordinator.history_cursor();
        let current_parameters = ParameterBatch::new(
            2,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(6.0),
            }],
        )
        .expect("current parameters");
        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                current_parameters.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("parameter attempt");
        let current_snapshots =
            ExternalSnapshotSet::new(2, vec![external_point_entry(binding, 2, [3.0, 4.0])])
                .expect("current snapshots");
        coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                current_snapshots.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("snapshot attempt");
        assert_eq!(
            coordinator.history_len(),
            history_len_before_host_inputs,
            "host-only evaluation must not create user-visible history"
        );
        assert_eq!(
            coordinator.history_cursor(),
            history_cursor_before_host_inputs
        );
        // Capture the current host-input pair in a later action while the
        // preceding accepted program remains only in Undo. Workspace restore
        // must resolve that historical stamp from retained action provenance,
        // rather than reverting to the baseline input pair or trusting flat
        // accepted bytes.
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "current-input history point".into(),
                    position: [11.0, 8.0],
                },
            )
            .expect("current-input history edit");
        let history_len = coordinator.history_len();
        let history_cursor = coordinator.history_cursor();

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace")
            .encode()
            .expect("workspace JSON");
        let mut cold: serde_json::Value = serde_json::from_str(&encoded).expect("workspace value");
        cold["design"]["json"] = serde_json::Value::String("{}".into());
        cold["lineage_materialization_map_json"] =
            serde_json::Value::String("discard this cache".into());
        let recovered = WorkspaceSnapshot::decode(
            &serde_json::to_string(&cold).expect("corrupt disposable cache"),
        )
        .expect("cold lineage recovery");
        let mut restored = coordinator_from_snapshot(&recovered).expect("restored coordinator");
        assert_eq!(restored.session().parameter_batch(), &current_parameters);
        assert_eq!(
            restored.session().external_snapshot_set(),
            &current_snapshots
        );
        assert_eq!(restored.history_len(), history_len);
        assert_eq!(restored.history_cursor(), history_cursor);

        let assert_current_geometry = |coordinator: &RetainedEditorCoordinator| {
            let accepted = coordinator
                .session()
                .accepted_state_for_current_input()
                .expect("accepted current host input");
            let left = accepted
                .document()
                .point(rectangle.points[0])
                .expect("rectangle left")
                .position;
            let right = accepted
                .document()
                .point(rectangle.points[1])
                .expect("rectangle right")
                .position;
            assert!(((right[0] - left[0]) - 6.0).abs() < 1.0e-9);
            let external_position = accepted
                .document()
                .point(external_point)
                .expect("external point")
                .position;
            assert!((external_position[0] - 3.0).abs() < 1.0e-9);
            assert!((external_position[1] - 4.0).abs() < 1.0e-9);
        };
        assert_current_geometry(&restored);
        restored.undo().expect("Undo under restored current inputs");
        assert_eq!(restored.session().parameter_batch(), &current_parameters);
        assert_eq!(
            restored.session().external_snapshot_set(),
            &current_snapshots
        );
        assert_current_geometry(&restored);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn workspace_v7_retains_host_only_inputs_for_accepted_authority_held_only_in_redo() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("parameter binding");
        let external_point = document
            .add_point("external point", [1.0, 2.0])
            .expect("point");
        let binding = document
            .add_external_binding("external datum", ExternalFeatureKindV1::Point, None)
            .expect("external binding");
        document
            .add_constraint(
                "external coincidence",
                DocumentConstraintDefinition::ExternalPointCoincident {
                    point: external_point,
                    external: DocumentExternalPointRef { binding },
                },
            )
            .expect("external constraint");
        let initial_parameters = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(4.0),
            }],
        )
        .expect("initial parameters");
        let initial_snapshots =
            ExternalSnapshotSet::new(1, vec![external_point_entry(binding, 1, [1.0, 2.0])])
                .expect("initial snapshots");
        let session = RetainedSketchDocumentSession::new_with_inputs(
            document,
            initial_parameters,
            initial_snapshots,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("initial session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "redo-only point".into(),
                    position: [9.0, 7.0],
                },
            )
            .expect("history edit before host-only inputs");

        let redo_parameters = ParameterBatch::new(
            2,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(6.0),
            }],
        )
        .expect("redo-only parameters");
        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                redo_parameters,
                DocumentSolveRequest::default(),
            )
            .expect("redo-only parameter acceptance");
        let redo_snapshots =
            ExternalSnapshotSet::new(2, vec![external_point_entry(binding, 2, [3.0, 4.0])])
                .expect("redo-only snapshots");
        coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                redo_snapshots,
                DocumentSolveRequest::default(),
            )
            .expect("redo-only snapshot acceptance");
        coordinator
            .undo()
            .expect("move host-only accepted authority into Redo");

        let current_parameters = ParameterBatch::new(
            3,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(7.0),
            }],
        )
        .expect("current parameters");
        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                current_parameters.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("current parameter acceptance");
        let current_snapshots =
            ExternalSnapshotSet::new(3, vec![external_point_entry(binding, 3, [5.0, 6.0])])
                .expect("current snapshots");
        coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                current_snapshots.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("current snapshot acceptance");
        assert_eq!(coordinator.history_cursor(), 0);
        assert_eq!(coordinator.history_len(), 2);

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace")
            .encode()
            .expect("workspace JSON");
        let mut missing_ledger: serde_json::Value =
            serde_json::from_str(&encoded).expect("workspace value");
        missing_ledger
            .as_object_mut()
            .expect("workspace object")
            .remove("lineage_host_input_ledger_json");
        let missing_ledger = WorkspaceSnapshot::decode(
            &serde_json::to_string(&missing_ledger).expect("workspace without ledger"),
        )
        .expect("legacy v7 shape remains strictly decodable");
        assert!(
            coordinator_from_snapshot(&missing_ledger)
                .expect_err("missing historical payload must fail eager authority restoration")
                .contains("accepted lineage external-input provenance"),
            "a workspace cannot defer an unreproducible Redo authority until traversal"
        );

        let recovered = WorkspaceSnapshot::decode(&encoded).expect("decode workspace");
        let mut restored = coordinator_from_snapshot(&recovered)
            .expect("cold-authenticate accepted authority held only in Redo");
        assert_eq!(restored.session().parameter_batch(), &current_parameters);
        assert_eq!(
            restored.session().external_snapshot_set(),
            &current_snapshots
        );

        restored.redo().expect("Redo with current live host inputs");
        assert_eq!(restored.session().parameter_batch(), &current_parameters);
        assert_eq!(
            restored.session().external_snapshot_set(),
            &current_snapshots
        );
        let accepted = restored
            .session()
            .accepted_state_for_current_input()
            .expect("Redo re-evaluates under current host inputs");
        let left = accepted
            .document()
            .point(rectangle.points[0])
            .expect("rectangle left")
            .position;
        let right = accepted
            .document()
            .point(rectangle.points[1])
            .expect("rectangle right")
            .position;
        assert!(((right[0] - left[0]) - 7.0).abs() < 1.0e-9);
        let external_position = accepted
            .document()
            .point(external_point)
            .expect("external point")
            .position;
        assert!((external_position[0] - 5.0).abs() < 1.0e-9);
        assert!((external_position[1] - 6.0).abs() < 1.0e-9);
    }

    #[test]
    fn workspace_v7_restores_host_only_accepted_inputs_beneath_a_failed_current_attempt() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("parameter binding");
        let initial = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(4.0),
            }],
        )
        .expect("initial parameters");
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            initial,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("initial session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let accepted_parameters = ParameterBatch::new(
            2,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(6.0),
            }],
        )
        .expect("host-only accepted parameters");
        let accepted = coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                accepted_parameters.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("host-only accepted evaluation");
        assert!(accepted.published_accepted.is_some());

        let missing = ParameterBatch::new(3, Vec::new()).expect("missing parameter batch");
        let rejected = coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                missing.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("typed rejected evaluation");
        assert!(rejected.published_accepted.is_none());
        assert_eq!(coordinator.history_len(), 1);

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace")
            .encode()
            .expect("workspace JSON");
        let snapshot = WorkspaceSnapshot::decode(&encoded).expect("decode workspace");
        let restored = coordinator_from_snapshot(&snapshot)
            .expect("restore accepted authority from its stamped ledger entry");
        assert_eq!(restored.session().parameter_batch(), &missing);
        assert_eq!(
            restored.session().accepted_parameter_batch(),
            Some(&accepted_parameters)
        );
        assert!(restored.session().last_attempt().failure().is_some());
        let accepted = restored
            .session()
            .accepted_state()
            .expect("historical accepted state");
        let left = accepted
            .document()
            .point(rectangle.points[0])
            .expect("rectangle left")
            .position;
        let right = accepted
            .document()
            .point(rectangle.points[1])
            .expect("rectangle right")
            .position;
        assert!(((right[0] - left[0]) - 6.0).abs() < 1.0e-9);
    }

    #[test]
    fn workspace_v7_discards_an_independently_valid_alternate_accepted_cache() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let free_point = document
            .add_point("underconstrained point", [1.0, 2.0])
            .expect("free point");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted underconstrained session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let baseline = WorkspaceSnapshot::from_coordinator(&coordinator).expect("workspace");
        let cold_position = baseline
            .accepted_document()
            .expect("accepted payload")
            .expect("accepted document")
            .point(free_point)
            .expect("accepted point")
            .position;

        let mut alternate = baseline
            .accepted_document()
            .expect("accepted payload")
            .expect("accepted document");
        let alternate_position = [17.0, -9.0];
        alternate
            .set_point_position(free_point, alternate_position)
            .expect("move unconstrained accepted point");
        let mut forged = serde_json::to_value(&baseline).expect("workspace value");
        forged["accepted"]["json"] = serde_json::Value::String(
            alternate
                .to_canonical_json()
                .expect("alternate accepted document"),
        );

        let alternate_cache: WorkspaceSnapshot =
            serde_json::from_value(forged.clone()).expect("alternate flat cache");
        alternate_cache
            .validate_flat_cache()
            .expect("alternate solution is a valid flat cache");
        assert!(
            alternate_cache.cache_reconstructs_independently(),
            "the alternate underconstrained solution must be independently valid so only cold lineage identity can reject it"
        );

        let recovered = WorkspaceSnapshot::decode(
            &serde_json::to_string(&forged).expect("forged workspace JSON"),
        )
        .expect("valid lineage must recover its cold accepted materialization");
        let recovered_position = recovered
            .accepted_document()
            .expect("recovered accepted payload")
            .expect("recovered accepted document")
            .point(free_point)
            .expect("recovered accepted point")
            .position;
        assert_eq!(
            recovered_position.map(f64::to_bits),
            cold_position.map(f64::to_bits)
        );
        assert_ne!(
            recovered_position.map(f64::to_bits),
            alternate_position.map(f64::to_bits)
        );
        assert_eq!(recovered.accepted, baseline.accepted);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one cache-free workspace regression keeps current native-Fillet evidence, disposable-cache removal, exact bytes, and independent validity adjacent"
    )]
    fn workspace_v7_cache_free_reload_consumes_exact_current_accepted_native_fillet() {
        let (mut coordinator, ids) = current_native_fillet_corner();
        let center = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted native Fillet")
            .document()
            .point(ids.center)
            .expect("accepted Fillet center")
            .position;
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateConstraint {
                    label: "current center anchor".into(),
                    definition: DocumentConstraintDefinition::FixedPoint {
                        point: ids.center,
                        target: [center[0] + 1.0, center[1] + 0.75],
                    },
                },
            )
            .expect("accepted current center anchor");
        let accepted_before = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted export")
            .expect("accepted native-Fillet bytes");
        let lineage_json = coordinator
            .lineage_session_json()
            .expect("current accepted lineage session");
        let ledger_json = coordinator
            .lineage_host_input_ledger_json()
            .expect("current accepted host-input ledger");
        let intent = RetainedEditorCoordinator::lineage_materialization_checkpoint(&lineage_json)
            .expect("current lineage intent");
        assert_ne!(
            accepted_before,
            intent.design_json(),
            "the fixture must carry topology-sensitive accepted native-Fillet geometry distinct from flattened authored intent"
        );
        let evidence =
            RetainedEditorCoordinator::lineage_cold_current_accepted_evidence_checkpoint(
                &lineage_json,
                Some(&ledger_json),
                coordinator.session().parameter_batch(),
                coordinator.session().external_snapshot_set(),
            )
            .expect("cold current accepted evidence");
        assert!(evidence.accepted_belongs_to_current_design());
        assert_eq!(
            evidence.accepted_json(),
            Some(accepted_before.as_str()),
            "the public current-authority adapter must expose the exact owning-domain bytes"
        );

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace")
            .encode()
            .expect("workspace JSON");
        let mut cache_free: serde_json::Value =
            serde_json::from_str(&encoded).expect("workspace value");
        let object = cache_free.as_object_mut().expect("workspace object");
        for field in [
            "lineage_materialization_map_json",
            "design",
            "accepted",
            "accepted_belongs_to_current_design",
            "sketch_identity_high_water",
            "features_json",
            "feature_lifecycle_high_water",
            "computed_evaluation_high_water",
            "annotation_layout_json",
            "revisions",
        ] {
            object.remove(field);
        }

        let decoded = WorkspaceSnapshot::decode(
            &serde_json::to_string(&cache_free).expect("cache-free workspace JSON"),
        )
        .expect("cache-free current native-Fillet reconstruction");
        assert!(decoded.accepted_belongs_to_current_design);
        assert_eq!(
            decoded
                .accepted_document()
                .expect("decoded accepted payload")
                .expect("decoded current accepted document")
                .to_canonical_json()
                .expect("decoded accepted canonical bytes"),
            accepted_before,
            "workspace decode must consume authenticated current accepted evidence rather than re-solve flattened intent"
        );
        let restored = coordinator_from_snapshot(&decoded).expect("restored coordinator");
        assert_eq!(
            restored
                .session()
                .export_accepted_json()
                .expect("restored accepted export")
                .expect("restored current accepted bytes"),
            accepted_before
        );
        let accepted = restored
            .session()
            .accepted_state_for_current_input()
            .expect("restored current accepted state");
        assert!(accepted.document().point(ids.center).is_some());
        assert!(accepted.document().curve(ids.arc).is_some());
        assert!(
            accepted
                .document()
                .points()
                .iter()
                .all(|point| point.position.into_iter().all(f64::is_finite))
        );
        let report = accepted.solve_result().unstable_core_report();
        assert!(report.hard_residuals_validated, "{report:#?}");
        assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one cache-free workspace regression keeps native-Fillet authoring, retained rejection, exact accepted bytes, identity high-water, and independent validity adjacent"
    )]
    fn workspace_v7_cache_free_reload_publishes_exact_older_accepted_native_fillet() {
        let (mut coordinator, ids) = current_native_fillet_corner();
        let center = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted native Fillet")
            .document()
            .point(ids.center)
            .expect("accepted Fillet center")
            .position;
        let anchored_center = [center[0] + 1.0, center[1] + 0.75];
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateConstraint {
                    label: "accepted center anchor".into(),
                    definition: DocumentConstraintDefinition::FixedPoint {
                        point: ids.center,
                        target: anchored_center,
                    },
                },
            )
            .expect("accepted center anchor");
        let accepted_before = coordinator
            .session()
            .export_accepted_json()
            .expect("accepted export")
            .expect("accepted native-Fillet bytes");
        let rejected = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateConstraint {
                    label: "conflicting center anchor".into(),
                    definition: DocumentConstraintDefinition::FixedPoint {
                        point: ids.center,
                        target: [anchored_center[0] + 1.0, anchored_center[1] + 1.0],
                    },
                },
            )
            .expect("structurally valid conflicting anchor");
        assert!(rejected.published_accepted.is_none());
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        assert_eq!(
            coordinator
                .session()
                .export_accepted_json()
                .expect("retained accepted export")
                .expect("older accepted bytes"),
            accepted_before
        );
        let mut expected =
            SketchDocument::from_json(&accepted_before).expect("accepted native-Fillet document");
        expected
            .retain_persistent_identity_high_water(
                coordinator.session().persistent_identity_high_water(),
            )
            .expect("retain rejected-action identity high-water");
        let expected = expected
            .to_canonical_json()
            .expect("canonical expected accepted bytes");

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace")
            .encode()
            .expect("workspace JSON");
        let accepted_intent =
            RetainedEditorCoordinator::lineage_last_accepted_materialization_checkpoint(
                &coordinator.lineage_session_json().expect("lineage session"),
            )
            .expect("accepted lineage materialization")
            .expect("accepted lineage checkpoint");
        assert_ne!(
            accepted_before,
            accepted_intent.design_json(),
            "the fixture must preserve a topology-sensitive accepted native-Fillet result distinct from its flattened authored seed"
        );
        let mut cache_free: serde_json::Value =
            serde_json::from_str(&encoded).expect("workspace value");
        let object = cache_free.as_object_mut().expect("workspace object");
        for field in [
            "lineage_materialization_map_json",
            "design",
            "accepted",
            "accepted_belongs_to_current_design",
            "sketch_identity_high_water",
            "features_json",
            "feature_lifecycle_high_water",
            "computed_evaluation_high_water",
            "annotation_layout_json",
            "revisions",
        ] {
            object.remove(field);
        }

        let decoded = WorkspaceSnapshot::decode(
            &serde_json::to_string(&cache_free).expect("cache-free workspace JSON"),
        )
        .expect("cache-free cold lineage reconstruction");
        assert_eq!(
            decoded
                .accepted_document()
                .expect("decoded accepted payload")
                .expect("decoded historical accepted document")
                .to_canonical_json()
                .expect("decoded accepted canonical bytes"),
            expected,
            "the workspace decoder itself must publish the exact authenticated accepted evidence"
        );
        let restored = coordinator_from_snapshot(&decoded).expect("restored coordinator");
        assert!(restored.session().last_attempt().failure().is_some());
        assert!(
            restored
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        assert_eq!(
            restored
                .session()
                .export_accepted_json()
                .expect("restored accepted export")
                .expect("restored older accepted bytes"),
            expected,
            "cache-free restore may advance only allocator high-water over the exact cold-authenticated native-Fillet bytes"
        );
        let accepted = restored
            .session()
            .accepted_state()
            .expect("restored historical accepted state");
        assert!(accepted.document().point(ids.center).is_some());
        assert!(accepted.document().curve(ids.arc).is_some());
        assert!(
            accepted
                .document()
                .points()
                .iter()
                .all(|point| point.position.into_iter().all(f64::is_finite))
        );
        let report = accepted.solve_result().unstable_core_report();
        assert!(report.hard_residuals_validated, "{report:#?}");
        assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    }

    #[test]
    fn workspace_v7_retains_an_exact_accepted_cache_with_advanced_identity_high_water() {
        let mut document = SketchDocument::new(8.0).expect("document");
        document
            .add_point("retained point", [1.0, 2.0])
            .expect("retained point");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let baseline = WorkspaceSnapshot::from_coordinator(&coordinator).expect("workspace");

        let advance_allocator = |mut document: SketchDocument| {
            let transient = document
                .add_point("deleted allocator witness", [5.0, 6.0])
                .expect("transient point");
            document
                .remove(DocumentObjectId::Point(transient))
                .expect("remove transient point");
            document
        };
        let advanced_design =
            advance_allocator(baseline.design_document().expect("design document"));
        let advanced_accepted = advance_allocator(
            baseline
                .accepted_document()
                .expect("accepted payload")
                .expect("accepted document"),
        );
        let advanced_identity_high_water = advanced_design.persistent_identity_high_water();
        assert_eq!(
            advanced_accepted.persistent_identity_high_water(),
            advanced_identity_high_water
        );
        assert_ne!(
            advanced_identity_high_water,
            baseline.sketch_identity_high_water
        );

        let mut advanced = serde_json::to_value(&baseline).expect("workspace value");
        advanced["design"]["json"] = serde_json::Value::String(
            advanced_design
                .to_canonical_json()
                .expect("advanced design document"),
        );
        advanced["accepted"]["json"] = serde_json::Value::String(
            advanced_accepted
                .to_canonical_json()
                .expect("advanced accepted document"),
        );
        advanced["sketch_identity_high_water"] =
            serde_json::to_value(&advanced_identity_high_water).expect("advanced high-water value");

        let recovered = WorkspaceSnapshot::decode(
            &serde_json::to_string(&advanced).expect("advanced workspace JSON"),
        )
        .expect("semantically exact cache with advanced allocators remains reusable");
        assert_eq!(
            recovered.sketch_identity_high_water,
            advanced_identity_high_water
        );
        assert_eq!(
            recovered
                .accepted_document()
                .expect("recovered accepted payload")
                .expect("recovered accepted document")
                .persistent_identity_high_water(),
            advanced_identity_high_water
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn workspace_v7_retains_failed_parameter_input_and_lineage_authority_without_history() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("parameter binding");
        let external_binding = document
            .add_external_binding("unused external datum", ExternalFeatureKindV1::Point, None)
            .expect("external binding");
        let initial = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(4.0),
            }],
        )
        .expect("initial parameters");
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            initial.clone(),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("initial session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "history point".into(),
                    position: [9.0, 7.0],
                },
            )
            .expect("history edit");
        let history_len = coordinator.history_len();
        let history_cursor = coordinator.history_cursor();
        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted before failed host input")
            .identity();
        let missing = ParameterBatch::new(2, Vec::new()).expect("missing parameter batch");
        let outcome = coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                missing.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("typed failed parameter attempt");
        assert!(outcome.published_accepted.is_none());
        assert_eq!(coordinator.session().parameter_batch(), &missing);
        let rejected_snapshots = ExternalSnapshotSet::new(
            2,
            vec![external_point_entry(external_binding, 2, [5.0, 7.0])],
        )
        .expect("newer external snapshot candidate");
        let external_outcome = coordinator
            .replace_external_snapshot_set(
                coordinator.session().design_identity(),
                rejected_snapshots.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("typed failed external attempt beneath missing parameter input");
        assert!(external_outcome.published_accepted.is_none());
        assert_eq!(
            coordinator.session().external_snapshot_set(),
            &ExternalSnapshotSet::default()
        );
        assert_eq!(
            coordinator.session().latest_attempt_external_snapshot_set(),
            &rejected_snapshots
        );
        assert_eq!(
            coordinator.session().accepted_parameter_batch(),
            Some(&initial)
        );
        assert_eq!(
            coordinator.session().accepted_external_snapshot_set(),
            Some(&ExternalSnapshotSet::default())
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("prior accepted state remains retained")
                .identity(),
            accepted_before
        );
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        assert_eq!(coordinator.history_len(), history_len);
        assert_eq!(coordinator.history_cursor(), history_cursor);

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace")
            .encode()
            .expect("workspace JSON");
        let encoded_value: serde_json::Value =
            serde_json::from_str(&encoded).expect("workspace value");
        assert_eq!(
            encoded_value["parameter_batch_json"],
            missing.to_canonical_json().expect("current parameter JSON")
        );
        assert_eq!(
            encoded_value["accepted_parameter_batch_json"],
            initial
                .to_canonical_json()
                .expect("accepted parameter JSON")
        );
        assert_eq!(
            encoded_value["external_snapshot_set_json"],
            rejected_snapshots
                .to_canonical_json()
                .expect("current external snapshot JSON")
        );
        assert_eq!(
            encoded_value["accepted_external_snapshot_set_json"],
            ExternalSnapshotSet::default()
                .to_canonical_json()
                .expect("accepted external snapshot JSON")
        );
        let mut forged_accepted_inputs = encoded_value.clone();
        forged_accepted_inputs["accepted_parameter_batch_json"] =
            encoded_value["parameter_batch_json"].clone();
        assert!(
            WorkspaceSnapshot::decode(
                &serde_json::to_string(&forged_accepted_inputs)
                    .expect("forged accepted-input workspace"),
            )
            .is_err_and(|error| error.contains("accepted lineage external-input provenance"))
        );
        let mut missing_accepted_cache = encoded_value;
        missing_accepted_cache["accepted"] = serde_json::Value::Null;
        let recovered_without_accepted_cache = WorkspaceSnapshot::decode(
            &serde_json::to_string(&missing_accepted_cache)
                .expect("workspace without disposable accepted cache"),
        )
        .expect("accepted authority rebuilds from lineage and its exact historical inputs");
        assert!(recovered_without_accepted_cache.accepted.is_some());
        let decoded = WorkspaceSnapshot::decode(&encoded).expect("decode failed-input workspace");
        let mut restored = coordinator_from_snapshot(&decoded).expect("restore failed input");
        assert_eq!(restored.session().parameter_batch(), &missing);
        assert_eq!(
            restored.session().external_snapshot_set(),
            &ExternalSnapshotSet::default()
        );
        assert_eq!(
            restored.session().latest_attempt_external_snapshot_set(),
            &rejected_snapshots
        );
        assert_eq!(
            restored.session().accepted_parameter_batch(),
            Some(&initial)
        );
        assert_eq!(
            restored.session().accepted_external_snapshot_set(),
            Some(&ExternalSnapshotSet::default())
        );
        assert!(restored.session().last_attempt().failure().is_some());
        assert!(
            restored
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        let lineage: serde_json::Value = serde_json::from_str(
            &restored
                .lineage_session_json()
                .expect("restored lineage session"),
        )
        .expect("lineage session value");
        assert_eq!(lineage["latest_attempt"]["disposition"], "failed");
        assert!(lineage["latest_attempt"]["external_inputs"].is_string());
        assert!(lineage["last_accepted"].is_object());
        assert_eq!(restored.history_len(), history_len);
        assert_eq!(restored.history_cursor(), history_cursor);

        restored.undo().expect("Undo under retained failing input");
        assert_eq!(restored.session().parameter_batch(), &missing);
        assert_eq!(
            restored.session().latest_attempt_external_snapshot_set(),
            &rejected_snapshots
        );
        assert!(restored.session().last_attempt().failure().is_some());
        assert!(restored.session().accepted_state().is_some());
        assert_eq!(
            restored.session().accepted_parameter_batch(),
            Some(&initial)
        );
        assert!(
            restored
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
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

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the cache-free reload regression keeps sketch, feature, scene, lineage, and history authority in one end-to-end assertion"
    )]
    fn m83_w11_lineage_only_reload_reconstructs_scene_feature_and_history_authority() {
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
        let feature = apply_computed_fillet(&mut coordinator, points[1], "lineage-only Fillet");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "lineage-only native point".into(),
                    position: [9.0, -2.0],
                },
            )
            .expect("native geometry mutation");

        let expected_lineage = coordinator.lineage_json().expect("lineage document");
        let expected_design = coordinator
            .session()
            .design_document()
            .to_canonical_json()
            .expect("canonical retained design");
        let expected_accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted current scene")
            .document()
            .to_canonical_json()
            .expect("canonical accepted scene");
        let expected_features = coordinator.feature_document().features().to_vec();
        let expected_feature_high_water = coordinator.feature_document().lifecycle_high_water();
        let expected_computed = coordinator
            .computed_snapshot()
            .expect("computed Fillet scene")
            .edges()
            .iter()
            .map(|edge| (edge.geometry.clone(), edge.provenance.clone()))
            .collect::<Vec<_>>();
        let expected_history = (
            coordinator.history_len(),
            coordinator.history_cursor(),
            coordinator.can_undo(),
            coordinator.can_redo(),
        );

        let encoded = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace")
            .encode()
            .expect("workspace JSON");
        let mut lineage_only: serde_json::Value =
            serde_json::from_str(&encoded).expect("workspace value");
        let object = lineage_only.as_object_mut().expect("workspace object");
        for field in [
            "lineage_materialization_map_json",
            "design",
            "accepted",
            "accepted_belongs_to_current_design",
            "sketch_identity_high_water",
            "features_json",
            "feature_lifecycle_high_water",
            "computed_evaluation_high_water",
            "annotation_layout_json",
            "revisions",
        ] {
            object.remove(field);
        }

        let recovered = WorkspaceSnapshot::decode(
            &serde_json::to_string(&lineage_only).expect("lineage-only workspace JSON"),
        )
        .expect("cold lineage-only workspace recovery");
        let restored = coordinator_from_snapshot(&recovered).expect("restored coordinator");
        assert_eq!(
            restored
                .session()
                .design_document()
                .to_canonical_json()
                .expect("restored retained design"),
            expected_design
        );
        assert_eq!(
            restored
                .session()
                .accepted_state_for_current_input()
                .expect("restored accepted scene")
                .document()
                .to_canonical_json()
                .expect("restored accepted scene JSON"),
            expected_accepted
        );
        assert_eq!(restored.feature_document().features(), expected_features);
        assert!(super::feature_high_water_covers(
            restored.feature_document().lifecycle_high_water(),
            expected_feature_high_water,
        ));
        assert!(restored.feature_document().feature(feature).is_some());
        assert_eq!(
            restored
                .computed_snapshot()
                .expect("restored computed Fillet scene")
                .edges()
                .iter()
                .map(|edge| (edge.geometry.clone(), edge.provenance.clone()))
                .collect::<Vec<_>>(),
            expected_computed
        );
        assert_eq!(
            restored.lineage_json().expect("restored lineage document"),
            expected_lineage
        );
        assert_eq!(
            (
                restored.history_len(),
                restored.history_cursor(),
                restored.can_undo(),
                restored.can_redo(),
            ),
            expected_history
        );
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
        assert_eq!(snapshot.version, 7);
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
        assert_eq!(migrated_v5.version, 7);
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
        assert_eq!(snapshot.version, 7);
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
    #[allow(clippy::too_many_lines)]
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
        .expect("transport workspace with corrupt flat cache");
        let recovered = coordinator_from_reproduction_payload(&invalid_payload)
            .expect("valid lineage must recover from a corrupt v7 flat cache");
        assert_eq!(
            recovered.session().design_document(),
            coordinator.session().design_document()
        );
        assert_eq!(
            WorkspaceSnapshot::from_coordinator(&coordinator)
                .expect("workspace after cache recovery")
                .encode()
                .expect("workspace JSON after cache recovery"),
            retained
        );

        let mut invalid_lineage: serde_json::Value =
            serde_json::from_str(&retained).expect("workspace value");
        invalid_lineage["lineage_session_json"] = serde_json::Value::String("{}".into());
        let invalid_lineage_payload = crate::reproduction::encode_workspace(
            &serde_json::to_string(&invalid_lineage).expect("invalid lineage workspace JSON"),
        )
        .expect("transport structurally invalid lineage");
        assert!(coordinator_from_reproduction_payload(&invalid_lineage_payload).is_err());
        assert_eq!(
            WorkspaceSnapshot::from_coordinator(&coordinator)
                .expect("workspace after invalid lineage restore")
                .encode()
                .expect("workspace JSON after invalid lineage restore"),
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
        // Historical v6 snapshots have no lineage authority, so their flat
        // lifecycle state remains strict input to coordinator reconstruction.
        reconstruction_failure["version"] = serde_json::Value::from(6);
        reconstruction_failure
            .as_object_mut()
            .expect("workspace object")
            .remove("lineage_session_json");
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
        assert_eq!(snapshot.version, 7);
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
    #[allow(
        clippy::too_many_lines,
        reason = "one cache-free v7 lifecycle regression retains abandoned native, spline, feature and corner identities across Redo loss"
    )]
    fn workspace_v7_lineage_only_reload_after_undo_preserves_abandoned_native_feature_and_spline_high_water()
     {
        let (mut document, spline) = clamped_bspline_document();
        let fillet_points = [
            document
                .add_point("fillet p0", [0.0, 6.0])
                .expect("fillet p0"),
            document
                .add_point("fillet p1", [4.0, 6.0])
                .expect("fillet p1"),
            document
                .add_point("fillet p2", [4.0, 10.0])
                .expect("fillet p2"),
        ];
        document
            .add_curve(
                "fillet polyline",
                CurveDefinition::Polyline {
                    points: fillet_points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
                },
            )
            .expect("fillet polyline");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted mixed session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        let insertion = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::InsertBSplineKnot {
                    curve: spline,
                    parameter: 0.25,
                },
            )
            .expect("abandoned knot insertion");
        let DocumentCommandEffect::InsertedBSplineKnot(insertion) = insertion.value else {
            panic!("expected B-spline insertion");
        };
        let abandoned_native = insertion.new_control;
        let abandoned_span = insertion.new_span_id.expect("new spline span identity");
        let abandoned_feature = apply_computed_fillet(
            &mut coordinator,
            fillet_points[1],
            "abandoned computed Fillet",
        );
        let abandoned_corner = {
            let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
                &coordinator
                    .feature_document()
                    .feature(abandoned_feature)
                    .expect("abandoned feature before Undo")
                    .definition;
            fillet.corners[0].id
        };
        let abandoned_sketch_high_water = coordinator
            .session()
            .persistent_identity_high_water()
            .clone();
        let abandoned_feature_high_water = coordinator.feature_document().lifecycle_high_water();

        coordinator.undo().expect("Undo computed feature");
        coordinator.undo().expect("Undo knot insertion");
        assert!(coordinator.can_redo());
        assert!(
            coordinator
                .session()
                .design_document()
                .point(abandoned_native)
                .is_none()
        );
        assert!(
            coordinator
                .feature_document()
                .feature(abandoned_feature)
                .is_none()
        );

        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator)
            .expect("workspace with abandoned Redo identities");
        let mut lineage_only = serde_json::to_value(snapshot).expect("workspace value");
        for cache_field in [
            "lineage_materialization_map_json",
            "design",
            "accepted",
            "accepted_belongs_to_current_design",
            "sketch_identity_high_water",
            "features_json",
            "feature_lifecycle_high_water",
            "computed_evaluation_high_water",
            "revisions",
        ] {
            lineage_only[cache_field] = serde_json::Value::Null;
        }
        let decoded = WorkspaceSnapshot::decode(
            &serde_json::to_string(&lineage_only).expect("lineage-only workspace JSON"),
        )
        .expect("cold cache-free lineage recovery");
        let mut restored = coordinator_from_snapshot(&decoded).expect("restored coordinator");

        let sketch_covers_abandoned = restored
            .session()
            .persistent_identity_high_water()
            .merged(&abandoned_sketch_high_water)
            .is_ok_and(|merged| merged == *restored.session().persistent_identity_high_water());
        let restored_feature_high_water = restored.feature_document().lifecycle_high_water();
        let feature_covers_abandoned = restored_feature_high_water.revision.raw()
            >= abandoned_feature_high_water.revision.raw()
            && restored_feature_high_water.allocator.next_feature_id.raw()
                >= abandoned_feature_high_water.allocator.next_feature_id.raw()
            && restored_feature_high_water.allocator.next_corner_id.raw()
                >= abandoned_feature_high_water.allocator.next_corner_id.raw();

        let replacement_point = restored
            .apply_edit(
                restored.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "post-reload replacement".into(),
                    position: [12.0, 12.0],
                },
            )
            .expect("new edit clears Redo without releasing IDs");
        let DocumentCommandEffect::CreatedPoint(replacement_point) = replacement_point.value else {
            panic!("expected replacement point");
        };
        assert!(!restored.can_redo());
        let replacement_insertion = restored
            .apply_edit(
                restored.session().design_identity(),
                DocumentEdit::InsertBSplineKnot {
                    curve: spline,
                    parameter: 0.75,
                },
            )
            .expect("replacement knot insertion");
        let DocumentCommandEffect::InsertedBSplineKnot(replacement_insertion) =
            replacement_insertion.value
        else {
            panic!("expected replacement B-spline insertion");
        };
        let replacement_feature = apply_computed_fillet(
            &mut restored,
            fillet_points[1],
            "replacement computed Fillet",
        );
        let replacement_corner = {
            let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) = &restored
                .feature_document()
                .feature(replacement_feature)
                .expect("replacement feature")
                .definition;
            fillet.corners[0].id
        };

        assert!(
            sketch_covers_abandoned,
            "native and spline high-water regressed"
        );
        assert!(
            feature_covers_abandoned,
            "feature and corner high-water regressed"
        );
        assert!(replacement_point.0.as_u128() > abandoned_native.0.as_u128());
        assert!(
            replacement_insertion
                .new_span_id
                .expect("replacement span identity")
                > abandoned_span
        );
        assert!(replacement_feature.raw() > abandoned_feature.raw());
        assert!(replacement_corner.raw() > abandoned_corner.raw());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one cache-free current-authority regression keeps flexible-Fillet setup, historical-route rejection, and exact restored bytes adjacent"
    )]
    fn workspace_v7_cache_free_current_design_solve_restores_flexible_fillet_exactly() {
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
        let current_lineage = coordinator
            .lineage_session_json()
            .expect("current accepted lineage session");
        let current_ledger = coordinator
            .lineage_host_input_ledger_json()
            .expect("current accepted host-input ledger");
        let historical_error =
            RetainedEditorCoordinator::lineage_cold_historical_accepted_evidence_checkpoint(
                &current_lineage,
                Some(&current_ledger),
                coordinator.session().accepted_parameter_batch(),
                coordinator.session().accepted_external_snapshot_set(),
            )
            .expect_err("current accepted authority is not historical fallback evidence");
        assert!(
            historical_error
                .to_string()
                .contains("requires a rejected current lineage attempt")
        );

        let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).expect("capture v7");
        assert!(snapshot.accepted_belongs_to_current_design);
        let mut cache_free: serde_json::Value =
            serde_json::from_str(&snapshot.encode().expect("encode v7")).expect("workspace value");
        let object = cache_free.as_object_mut().expect("workspace object");
        for field in [
            "lineage_materialization_map_json",
            "design",
            "accepted",
            "accepted_belongs_to_current_design",
            "sketch_identity_high_water",
            "features_json",
            "feature_lifecycle_high_water",
            "computed_evaluation_high_water",
            "annotation_layout_json",
            "revisions",
        ] {
            object.remove(field);
        }
        let decoded = WorkspaceSnapshot::decode(
            &serde_json::to_string(&cache_free).expect("cache-free workspace JSON"),
        )
        .expect("cache-free v7 decode");
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
    #[allow(clippy::too_many_lines)]
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
        let current_v7: serde_json::Value = serde_json::from_str(&encoded).expect("snapshot value");
        let mut baseline = current_v7.clone();
        baseline["version"] = serde_json::Value::from(5);
        baseline
            .as_object_mut()
            .expect("workspace object")
            .remove("lineage_session_json");
        baseline
            .as_object_mut()
            .expect("workspace object")
            .remove("annotation_layout_json");
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
        spline_cursor_behind["version"] = serde_json::Value::from(5);
        spline_cursor_behind
            .as_object_mut()
            .expect("spline workspace object")
            .remove("lineage_session_json");
        spline_cursor_behind
            .as_object_mut()
            .expect("spline workspace object")
            .remove("annotation_layout_json");
        spline_cursor_behind["sketch_identity_high_water"]["spline_span_cursors"]
            .as_object_mut()
            .expect("spline cursor map")
            .insert(curve.to_string(), serde_json::Value::from(99));
        assert_rejected(spline_cursor_behind);

        let mut accepted_cursor_ahead = snapshot.clone();
        accepted_cursor_ahead.version = 5;
        accepted_cursor_ahead.lineage_session_json = None;
        accepted_cursor_ahead.annotation_layout_json = None;
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

        let mut recoverable_v7 = current_v7;
        recoverable_v7["sketch_identity_high_water"]["next_id"] =
            serde_json::Value::String("00000000000000000000000000000000".into());
        let recovered = WorkspaceSnapshot::decode(
            &serde_json::to_string(&recoverable_v7).expect("recoverable v7 cache"),
        )
        .expect("v7 lineage must recover a corrupt identity cache");
        assert_eq!(
            recovered.sketch_identity_high_water,
            snapshot.sketch_identity_high_water
        );

        assert!(WorkspaceSnapshot::decode(&format!("{encoded} trailing")).is_err());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one compatibility probe follows a current draft payload through every legacy payload adapter"
    )]
    fn current_draft_payload_remains_compatible_with_legacy_workspace_adapters() {
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
        assert_eq!(migrated_v4.version, 7);
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
        assert_eq!(migrated_v3.version, 7);
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
        assert_eq!(migrated_v2.version, 7);
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
        assert_eq!(migrated.version, 7);
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
}
