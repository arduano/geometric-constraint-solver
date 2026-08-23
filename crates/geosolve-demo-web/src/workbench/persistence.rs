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
    PersistentId, RetainedSketchDocumentSession, SketchDocument, SketchLifecycleRevisionHighWater,
    SketchPersistentIdentityHighWater,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater, ComputedFeatureDocument,
    ComputedFeatureLifecycleHighWater,
};

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
        Ok(Self::from_checkpoint(
            &checkpoint,
            coordinator.editor().annotation_layout(),
        ))
    }

    fn from_checkpoint(
        checkpoint: &RestoreCheckpoint,
        annotation_layout: &AnnotationLayoutState,
    ) -> Self {
        let revisions = checkpoint.revisions();
        Self {
            version: 6,
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
        serde_json::to_string(self).map_err(|error| error.to_string())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed five-version migration matrix is clearer when audited in one dispatch"
    )]
    pub(crate) fn decode(input: &str) -> Result<Self, String> {
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
                let sketch_identity_high_water =
                    derive_sketch_identity_high_water(&design, accepted.as_ref())?;
                Self {
                    version: 6,
                    design,
                    accepted,
                    accepted_belongs_to_current_design: false,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
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
                Self {
                    version: 6,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: false,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
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
                Self {
                    version: 6,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: legacy.accepted_belongs_to_current_design,
                    sketch_identity_high_water,
                    features_json,
                    feature_lifecycle_high_water,
                    computed_evaluation_high_water: default_evaluation_high_water(),
                    annotation_layout_json: None,
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
                Self {
                    version: 6,
                    design: legacy.design,
                    accepted: legacy.accepted,
                    accepted_belongs_to_current_design: legacy.accepted_belongs_to_current_design,
                    sketch_identity_high_water,
                    features_json: legacy.features_json,
                    feature_lifecycle_high_water: legacy.feature_lifecycle_high_water,
                    computed_evaluation_high_water: legacy.computed_evaluation_high_water,
                    annotation_layout_json: None,
                    revisions: legacy.revisions,
                }
                .validated()
            }
            5 => {
                let mut snapshot: Self =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                snapshot.version = 6;
                snapshot.annotation_layout_json = None;
                snapshot.validated()
            }
            6 => {
                let snapshot: Self =
                    serde_json::from_str(input).map_err(|error| error.to_string())?;
                snapshot.validated()
            }
            _ => Err("unsupported workbench snapshot version".into()),
        }
    }

    fn validated(self) -> Result<Self, String> {
        if self.version != 6 {
            return Err("unsupported workbench snapshot version".into());
        }
        if self.accepted_belongs_to_current_design && self.accepted.is_none() {
            return Err("current-design accepted provenance requires an accepted payload".into());
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
        Ok(self)
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
        DocumentId, DocumentLineSupportRef, DocumentObjectId, DocumentSolveRequest, GeometryRole,
        PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
        alpha_scenario,
    };

    use super::{
        WorkspaceSnapshot, annotation_kind_key, coordinator_from_reproduction_payload,
        coordinator_from_snapshot, default_evaluation_high_water,
        derive_sketch_identity_high_water, parse_annotation_kind,
        reproduction_payload_from_coordinator,
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
}
