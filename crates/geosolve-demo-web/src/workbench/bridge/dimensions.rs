// SPDX-License-Identifier: GPL-3.0-or-later
//! Accepted dimension metadata and presentation preferences for the workbench.

use super::super::design_projection::InspectorParameterAuthority;
use super::*;
use geosolve_constraint_editor::{
    AnnotationLayoutKey, DimensionDisplayMode, DimensionPresentationContext,
    DimensionPresentationState, IntentInspectorEditTarget, IntentInspectorEditValue,
    IntentInspectorField, IntentInspectorProjection, IntentNativeBinding, SceneAnnotationKind,
    SceneDimensionEntry,
};
use geosolve_sketch_intent::{IntentLiteral, IntentUnit};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIMENSION_INSTANCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum DisplayMode {
    #[default]
    Focused,
    All,
    Hidden,
}

impl From<DisplayMode> for DimensionDisplayMode {
    fn from(mode: DisplayMode) -> Self {
        match mode {
            DisplayMode::Focused => Self::Focused,
            DisplayMode::All => Self::All,
            DisplayMode::Hidden => Self::Hidden,
        }
    }
}

impl From<DimensionDisplayMode> for DisplayMode {
    fn from(mode: DimensionDisplayMode) -> Self {
        match mode {
            DimensionDisplayMode::Focused => Self::Focused,
            DimensionDisplayMode::All => Self::All,
            DimensionDisplayMode::Hidden => Self::Hidden,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DimensionPersistence {
    #[serde(default)]
    mode: DisplayMode,
    /// Exact native document/source/item identity; never resolved by a label or ordinal.
    #[serde(default)]
    pins: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DimensionsSnapshot {
    mode: DisplayMode,
    entries: Vec<DimensionEntry>,
    parameters: Vec<DimensionalParameter>,
    pin_count: usize,
    all_measurements: Vec<DimensionEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "accepted presentation DTO has independent dimension flags"
)]
struct DimensionEntry {
    id: String,
    /// Stable presentation identity; commands still require the revision-bound ID.
    row_key: String,
    label: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    kind: &'static str,
    reference: bool,
    generated: bool,
    default_priority: bool,
    pinned: bool,
    visible: bool,
    focused: bool,
    editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    contextual: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<AuthoringMetadataSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DimensionalParameter {
    id: String,
    label: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    editable: bool,
    default_priority: bool,
    row_key: String,
    metadata: Option<AuthoringMetadataSnapshot>,
    consumers: Vec<String>,
}

#[derive(Clone)]
enum DimensionEdit {
    Source {
        control: String,
        from_display: f64,
    },
    Native {
        inspector: Box<IntentInspectorProjection>,
        target: IntentInspectorEditTarget,
        unit: IntentUnit,
    },
    ReadOnly(String),
}

struct DimensionMetadata {
    label: String,
    edit: DimensionEdit,
    generated: bool,
    default_priority: bool,
    presentation: Option<AuthoringMetadataSnapshot>,
}

#[derive(Default)]
struct ManagedDimensionProvenance {
    generated: BTreeSet<NodeId>,
    labels: BTreeMap<NodeId, String>,
    patch_controls: BTreeSet<String>,
    priority: BTreeSet<NodeId>,
    presentations: BTreeMap<NodeId, AuthoringMetadataSnapshot>,
}

#[derive(Default)]
struct ProvenanceCache {
    authority: String,
    owners: BTreeMap<SelectionItem, NodeId>,
    dimensions: BTreeMap<SelectionItem, DimensionMetadata>,
    parameters: Vec<(DimensionalParameter, BTreeSet<NodeId>)>,
}

pub(super) struct DimensionBridgeState {
    instance: u64,
    native: DimensionPresentationState,
    cache: ProvenanceCache,
    entries: Vec<(SceneDimensionEntry, DimensionEntry)>,
    parameters: Vec<DimensionalParameter>,
    pending_pins: Vec<String>,
    hover: Option<(SelectionItem, ScreenPoint)>,
    navigation_active: bool,
}

impl Default for DimensionBridgeState {
    fn default() -> Self {
        let mut native = DimensionPresentationState::default();
        native.mode = DimensionDisplayMode::Focused;
        Self {
            instance: NEXT_DIMENSION_INSTANCE.fetch_add(1, Ordering::Relaxed),
            native,
            cache: ProvenanceCache::default(),
            entries: Vec::new(),
            parameters: Vec::new(),
            pending_pins: Vec::new(),
            hover: None,
            navigation_active: false,
        }
    }
}

impl WorkbenchBridge {
    pub(super) fn dimension_instance(&self) -> u64 {
        self.dimensions.instance
    }

    fn dimension_authority(&self) -> String {
        geosolve_sketch_intent::intent_content_digest(
            serde_json::json!({
                "instance": self.dimensions.instance,
                "intent": self.editor().coordinator().intent().identity(),
                "code": self.code_project.as_ref().map(CodeProjectWorkbench::code_session_identity),
                "dirty": self.code_project.as_ref().is_some_and(CodeProjectWorkbench::is_dirty),
                "pending": self.pending_managed_mutation.is_some(),
            })
            .to_string()
            .as_bytes(),
        )
        .to_string()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one shared scene publication resolves native visibility and its matching Inspector rows"
    )]
    pub(super) fn apply_dimensions(&mut self, scene: &mut EditorScene) {
        if self.active_tool != "select" || self.canvas_pan.is_some() {
            self.clear_dimension_hover();
        }
        let authority = self.dimension_authority();
        if self.dimensions.cache.authority != authority {
            self.dimensions.cache = self.build_dimension_provenance(authority);
        }
        let interaction = self.editor().editor();
        let layout = interaction.annotation_layout_for_scene();
        let mut selection = interaction.selection().to_vec();
        if let Some(owner) = self.editor().selected_declaration() {
            selection.extend(self.editor().navigation_selection_items([owner]));
        }
        let active = self
            .editor()
            .offset_authoring_provisional_items()
            .iter()
            .find(|item| matches!(item, SelectionItem::Dimension(_)))
            .copied()
            .or_else(|| {
                interaction.active_pointer_gesture().and_then(|_| {
                    interaction
                        .selection()
                        .iter()
                        .find(|item| matches!(item, SelectionItem::Dimension(_)))
                        .copied()
                })
            });
        let context = DimensionPresentationContext {
            selection,
            hovered: self.dimensions.hover.map(|(item, _)| item),
            generated: self
                .dimensions
                .cache
                .dimensions
                .iter()
                .filter_map(|(item, metadata)| metadata.generated.then_some(*item))
                .collect(),
            default_priority: self
                .dimensions
                .cache
                .dimensions
                .iter()
                .filter_map(|(item, metadata)| metadata.default_priority.then_some(*item))
                .collect(),
            navigation_active: self.dimensions.navigation_active,
            active,
        };
        let mut entries = self.dimensions.native.apply(scene, &layout, &context);
        if !self.dimensions.pending_pins.is_empty() {
            self.dimensions.native.pins = entries
                .iter()
                .filter(|entry| {
                    self.dimensions
                        .pending_pins
                        .contains(&persistent_identity(entry.key))
                })
                .map(|entry| entry.key)
                .collect();
            self.dimensions.pending_pins.clear();
            entries = self.dimensions.native.apply(scene, &layout, &context);
        }
        let selected_owners: BTreeSet<_> = context
            .selection
            .iter()
            .filter_map(|item| self.dimensions.cache.owners.get(item).copied())
            .chain(self.editor().selected_declaration())
            .collect();
        self.dimensions.parameters = self
            .dimensions
            .cache
            .parameters
            .iter()
            .filter(|(row, owners)| row.default_priority || !owners.is_disjoint(&selected_owners))
            .map(|(row, _)| row.clone())
            .collect();
        self.dimensions.entries = entries
            .into_iter()
            .map(|entry| {
                let metadata = self.dimensions.cache.dimensions.get(&entry.key.item);
                let reason = if entry.reference {
                    Some("Reference measurements follow the accepted geometry".to_owned())
                } else if entry.suppressed {
                    Some("This dimension is suppressed".to_owned())
                } else {
                    metadata
                        .and_then(|metadata| match &metadata.edit {
                            DimensionEdit::ReadOnly(reason) => Some(reason.clone()),
                            DimensionEdit::Source { .. } | DimensionEdit::Native { .. } => None,
                        })
                        .or_else(|| {
                            metadata
                                .is_none()
                                .then(|| "No accepted edit target is available".into())
                        })
                };
                let row = DimensionEntry {
                    id: format!(
                        "{}:{}",
                        self.dimensions.cache.authority,
                        persistent_identity(entry.key)
                    ),
                    row_key: format!(
                        "{}:{}",
                        self.dimensions.instance,
                        persistent_identity(entry.key)
                    ),
                    label: metadata
                        .map_or_else(|| entry.label.clone(), |metadata| metadata.label.clone()),
                    value: self.dimension_measurement_value(&entry, scene),
                    unit: Some(
                        if entry.kind == SceneAnnotationKind::OrientedAngle {
                            "°"
                        } else {
                            "mm"
                        }
                        .into(),
                    ),
                    kind: dimension_kind(entry.kind),
                    reference: entry.reference,
                    generated: entry.generated,
                    default_priority: entry.default_priority,
                    pinned: entry.pinned,
                    visible: entry.visible,
                    focused: entry.focused,
                    editable: reason.is_none(),
                    reason,
                    contextual: entry.related || entry.focused || entry.pinned,
                    metadata: metadata.and_then(|metadata| metadata.presentation.clone()),
                };
                (entry, row)
            })
            .collect();
    }

    // Keep the scene-composition call site explicit: the shared owner must run
    // before either numeric drawing, SVG export or native picking consumes it.
    pub(super) fn apply_dimension_scene(&mut self, scene: &mut EditorScene) {
        self.apply_dimensions(scene);
    }

    pub(super) fn dimensions_snapshot(&self) -> DimensionsSnapshot {
        DimensionsSnapshot {
            mode: self.dimensions.native.mode.into(),
            entries: self
                .dimensions
                .entries
                .iter()
                .filter(|(entry, row)| {
                    let summarized_by_parameter = self
                        .dimensions
                        .cache
                        .dimensions
                        .get(&entry.key.item)
                        .and_then(|metadata| match &metadata.edit {
                            DimensionEdit::Source { control, .. } => Some(control),
                            _ => None,
                        })
                        .is_some_and(|control| {
                            self.dimensions.parameters.iter().any(|parameter| {
                                &parameter.id == control && parameter.default_priority
                            })
                        });
                    row.contextual
                        || (!summarized_by_parameter
                            && (entry.default_priority
                                || entry.related
                                || entry.pinned
                                || entry.focused
                                || entry.visible))
                })
                .map(|(_, row)| row.clone())
                .collect(),
            all_measurements: self
                .dimensions
                .entries
                .iter()
                .map(|(_, row)| row.clone())
                .collect(),
            parameters: self.dimensions.parameters.clone(),
            pin_count: self.dimensions.native.pins.len(),
        }
    }

    pub(super) fn dimension_persistence(&self) -> DimensionPersistence {
        DimensionPersistence {
            mode: self.dimensions.native.mode.into(),
            pins: if self.dimensions.pending_pins.is_empty() {
                self.dimensions
                    .native
                    .pins
                    .iter()
                    .copied()
                    .map(persistent_identity)
                    .collect()
            } else {
                self.dimensions.pending_pins.clone()
            },
        }
    }

    pub(super) fn restore_dimensions(
        &mut self,
        persistence: DimensionPersistence,
    ) -> Result<(), String> {
        if persistence.pins.len() > DimensionPresentationState::MAX_PINS
            || persistence.pins.iter().any(|pin| pin.len() > 1024)
            || persistence.pins.iter().collect::<BTreeSet<_>>().len() != persistence.pins.len()
        {
            return Err("dimension preferences contain invalid or excessive pins".into());
        }
        self.dimensions = DimensionBridgeState::default();
        self.dimensions.native.mode = persistence.mode.into();
        self.dimensions.pending_pins = persistence.pins;
        Ok(())
    }

    pub(super) fn restore_dimension_presentation(
        &mut self,
        persistence: DimensionPersistence,
    ) -> Result<(), String> {
        self.restore_dimensions(persistence)
    }

    pub(super) fn reset_dimensions(&mut self) {
        self.dimensions = DimensionBridgeState::default();
    }

    pub(super) fn clear_dimension_hover(&mut self) {
        self.dimensions.hover = None;
    }

    pub(super) fn clear_dimension_focus(&mut self) {
        self.dimensions.native.focus = None;
    }

    pub(super) fn reconsider_hidden_dimensions(&mut self) {
        self.set_dimension_navigation(false);
        self.dimensions.native.reconsider_hidden_dimensions();
    }

    pub(super) fn set_dimension_navigation(&mut self, active: bool) {
        self.dimensions.navigation_active = active;
        if active {
            self.clear_dimension_hover();
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one bounded command vocabulary authenticates all dimension presentation and edit actions"
    )]
    pub(super) fn dispatch_dimensions(
        &mut self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        if command == "dimensions.hover.clear" {
            self.clear_dimension_hover();
            return Ok(());
        }
        if command == "dimensions.hover" {
            let payload: HoverPayload =
                serde_json::from_value(payload).map_err(|error| error.to_string())?;
            return self.preview_dimension_hover(ScreenPoint {
                x: payload.x,
                y: payload.y,
            });
        }
        self.require_dimension_inspection_available()?;
        self.set_dimension_navigation(false);
        self.refresh_current_scene();
        match command {
            "dimensions.mode" => {
                let payload: ModePayload =
                    serde_json::from_value(payload).map_err(|error| error.to_string())?;
                self.dimensions.native.mode = payload.mode.into();
                self.dimensions.native.focus = None;
                self.clear_dimension_hover();
            }
            "dimensions.clearPins" => self.dimensions.native.pins.clear(),
            "dimensions.focus" => {
                let payload: SelectionPayload =
                    serde_json::from_value(payload).map_err(|error| error.to_string())?;
                let key = self.dimension_command_entry(&payload.id)?.0.key;
                self.dimensions.native.focus = Some(key);
                // Inspect is an explicit request to reveal a callout.
                if self.dimensions.native.mode == DimensionDisplayMode::Hidden {
                    self.dimensions.native.mode = DimensionDisplayMode::Focused;
                }
            }
            "dimensions.pin" => {
                let payload: PinPayload =
                    serde_json::from_value(payload).map_err(|error| error.to_string())?;
                let key = self.dimension_command_entry(&payload.id)?.0.key;
                if payload.pinned && !self.dimensions.native.pins.contains(&key) {
                    if self.dimensions.native.pins.len() >= DimensionPresentationState::MAX_PINS {
                        return Err(
                            "Unpin a dimension before adding another; four pins are available"
                                .into(),
                        );
                    }
                    self.dimensions.native.pins.push(key);
                } else if !payload.pinned {
                    self.dimensions.native.pins.retain(|pin| *pin != key);
                }
            }
            "dimensions.edit" => {
                let payload: EditPayload =
                    serde_json::from_value(payload).map_err(|error| error.to_string())?;
                let (entry, row) = self.dimension_command_entry(&payload.id)?;
                if !row.editable {
                    return Err(row
                        .reason
                        .clone()
                        .unwrap_or_else(|| "This dimension is read only".into()));
                }
                let key = entry.key;
                let edit = self
                    .dimensions
                    .cache
                    .dimensions
                    .get(&key.item)
                    .map(|metadata| metadata.edit.clone())
                    .ok_or_else(|| "dimension edit authority is unavailable".to_owned())?;
                let value = finite_edit_value(&payload.value)?;
                let metadata = self
                    .retained_scene
                    .as_ref()
                    .and_then(|scene| entry.target_metadata(scene))
                    .ok_or_else(|| "The accepted dimension target is unavailable".to_owned())?;
                let storage_value = metadata
                    .storage_value_for_display(value)
                    .map_err(|error| error.to_string())?;
                self.dimensions.native.focus = Some(key);
                match edit {
                    DimensionEdit::Source {
                        control,
                        from_display,
                    } => {
                        let display_storage = if metadata.unit == geosolve_sketch::ScalarUnit::Angle
                        {
                            storage_value.to_degrees()
                        } else {
                            storage_value
                        };
                        self.edit_parameter(
                            &control,
                            serde_json::json!(display_storage * from_display),
                        )?;
                    }
                    DimensionEdit::Native {
                        inspector,
                        target,
                        unit,
                    } => {
                        self.editor_mut()
                            .edit_inspector(
                                &inspector,
                                &target,
                                IntentInspectorEditValue::Literal {
                                    literal: IntentLiteral::Quantity {
                                        value: storage_value,
                                        unit,
                                    },
                                },
                            )
                            .map_err(|error| error.to_string())?;
                        if self.publish_generic_code_checkpoint("Edit dimension")? {
                            self.bump_revision();
                            self.notice = "Dimension updated".into();
                        }
                    }
                    DimensionEdit::ReadOnly(reason) => return Err(reason),
                }
            }
            _ => return Err(format!("unknown dimension command `{command}`")),
        }
        Ok(())
    }

    fn dimension_command_entry(
        &self,
        id: &str,
    ) -> Result<&(SceneDimensionEntry, DimensionEntry), String> {
        self.dimensions
            .entries
            .iter()
            .find(|(_, row)| row.id == id)
            .ok_or_else(|| "The dimension belongs to an unavailable or stale accepted scene".into())
    }

    fn require_dimension_inspection_available(&self) -> Result<(), String> {
        if self.active_tool != "select"
            || self.captured_pointer.is_some()
            || self.canvas_pan.is_some()
            || self.editor().editor().active_pointer_gesture().is_some()
            || self.pending_managed_mutation.is_some()
        {
            return Err(
                "Finish or cancel the active canvas interaction before inspecting dimensions"
                    .into(),
            );
        }
        Ok(())
    }

    fn preview_dimension_hover(&mut self, position: ScreenPoint) -> Result<(), String> {
        if !position.x.is_finite()
            || !position.y.is_finite()
            || position.x < 0.0
            || position.y < 0.0
            || position.x > self.host_size[0]
            || position.y > self.host_size[1]
        {
            return Err("dimension hover must lie inside the finite canvas bounds".into());
        }
        if self.require_dimension_inspection_available().is_err()
            || self.dimensions.navigation_active
        {
            self.clear_dimension_hover();
            return Ok(());
        }
        self.refresh_current_scene();
        let Some(scene) = self.retained_scene.as_ref() else {
            return Ok(());
        };
        let interaction = self.editor().editor();
        if let Some(hit) = scene.annotation_hit_test(
            position,
            PickTolerance::default(),
            interaction.selection(),
            interaction.hovered(),
            &[],
        ) && matches!(hit.item, SelectionItem::Dimension(_))
        {
            self.dimensions.hover = Some((hit.item, position));
            return Ok(());
        }
        if let Some((_, origin)) = self.dimensions.hover
            && scene
                .annotations
                .iter()
                .filter(|annotation| {
                    self.dimensions
                        .entries
                        .iter()
                        .any(|(entry, _)| entry.visible && entry.key.item == annotation.item)
                })
                .any(|annotation| annotation.context_hit_test(position, origin, 12.0))
        {
            return Ok(());
        }
        self.dimensions.hover = scene
            .hit_test_with_policy(
                position,
                PickTolerance::default(),
                interaction.geometry_interaction_policy(),
            )
            .map(|hit| (hit.item, position));
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one accepted provenance cache resolves source and native scalar authority without changing live selection"
    )]
    fn build_dimension_provenance(&self, authority: String) -> ProvenanceCache {
        let mut cache = ProvenanceCache {
            authority,
            ..ProvenanceCache::default()
        };
        let editor = self.editor();
        let Some(accepted) = editor.coordinator().accepted_materialization() else {
            return cache;
        };
        for owned in &accepted.ownership.nodes {
            for item in editor.navigation_selection_items_for_bindings(owned.owned.iter().copied())
            {
                cache.owners.insert(item, owned.node);
            }
        }
        let managed = self.dimension_parameter_provenance(&mut cache);
        let mut inspector_editor = self
            .code_project
            .as_ref()
            .and_then(|_| editor.fork_accepted_authority().ok());
        let projection = inspector_editor
            .as_ref()
            .map(geosolve_constraint_editor::ProjectionalEditorSession::workbench_projection);
        let manifest = self
            .code_project
            .as_ref()
            .map(CodeProjectWorkbench::managed_controls_cached);
        for dimension in accepted.session.design_document().dimensions() {
            let item = SelectionItem::Dimension(dimension.id);
            let owner = accepted
                .ownership
                .exact_owner(IntentNativeBinding::Dimension(dimension.id));
            let edit = owner
                .and_then(|owner| {
                    let inspector = IntentInspectorProjection::from_session(
                        editor.coordinator().intent(),
                        owner,
                    )?;
                    let (target, unit) = dimensional_inspector_target(&inspector)?;
                    let native = DimensionEdit::Native {
                        inspector: Box::new(inspector.clone()),
                        target: target.clone(),
                        unit,
                    };
                    let Some(code) = self.code_project.as_ref() else {
                        return Some(native);
                    };
                    let presentation = inspector_editor.as_mut().and_then(|fork| {
                        fork.set_selected_declaration(Some(owner));
                        code.inspector_parameter_presentations_with_manifest(
                            fork,
                            projection.as_ref()?,
                            &inspector,
                            &super::super::design_projection::InspectorDescriptorIndex::new(
                                &inspector,
                            ),
                            manifest
                                .as_ref()?
                                .as_ref()
                                .map(AsRef::as_ref)
                                .map_err(String::as_str),
                        )
                        .ok()
                    });
                    let Some(presentations) = presentation else {
                        return Some(DimensionEdit::ReadOnly(
                            "Accepted source edit authority is unavailable".into(),
                        ));
                    };
                    Some(
                        presentations
                            .into_iter()
                            .find(|presentation| presentation.target == target)
                            .map_or(native.clone(), |presentation| {
                                match presentation.authority {
                                    InspectorParameterAuthority::ModifiableSource {
                                        control_id,
                                        ..
                                    } => {
                                        let source_unit = manifest
                                            .as_ref()
                                            .and_then(|manifest| manifest.as_ref().ok())
                                            .and_then(|manifest| {
                                                manifest
                                                    .controls
                                                    .iter()
                                                    .find(|control| control.id.0 == control_id)
                                            })
                                            .and_then(|control| match &control.value {
                                                ManagedValue::Unit(value) => {
                                                    Some(value.unit.as_str())
                                                }
                                                _ => None,
                                            });
                                        let from_display = match (unit, source_unit) {
                                            (IntentUnit::Angle, Some("deg")) => 1.0,
                                            (IntentUnit::Angle, _) => std::f64::consts::PI / 180.0,
                                            (_, Some("cm")) => 0.1,
                                            (_, Some("m")) => 0.001,
                                            (_, Some("inch")) => 1.0 / 25.4,
                                            _ => 1.0,
                                        };
                                        DimensionEdit::Source {
                                            control: control_id,
                                            from_display,
                                        }
                                    }
                                    InspectorParameterAuthority::ModifiableInstance => native,
                                    InspectorParameterAuthority::Encoded { reason }
                                    | InspectorParameterAuthority::Blocked { reason } => {
                                        DimensionEdit::ReadOnly(reason)
                                    }
                                }
                            }),
                    )
                })
                .unwrap_or_else(|| {
                    DimensionEdit::ReadOnly("This measurement has no scalar Inspector edit".into())
                });
            cache.dimensions.insert(
                item,
                DimensionMetadata {
                    label: owner
                        .and_then(|owner| managed.labels.get(&owner))
                        .cloned()
                        .unwrap_or_else(|| dimension.label.clone()),
                    edit,
                    generated: owner.is_some_and(|owner| managed.generated.contains(&owner)),
                    default_priority: owner.is_some_and(|owner| managed.priority.contains(&owner)),
                    presentation: owner
                        .and_then(|owner| managed.presentations.get(&owner).cloned()),
                },
            );
        }
        distinguish_dimension_names(&mut cache.dimensions);
        let authored_controls: BTreeSet<_> = cache
            .dimensions
            .values()
            .filter_map(|metadata| match &metadata.edit {
                DimensionEdit::Source { control, .. } if !metadata.generated => Some(control),
                _ => None,
            })
            .collect();
        cache.parameters.retain(|(parameter, _)| {
            !authored_controls.contains(&parameter.id)
                || managed.patch_controls.contains(&parameter.id)
        });
        cache
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one cached accepted projection supplies source names, generated ownership and dimensional parameter consumers"
    )]
    fn dimension_parameter_provenance(
        &self,
        cache: &mut ProvenanceCache,
    ) -> ManagedDimensionProvenance {
        let Some(code) = self.code_project.as_ref() else {
            return ManagedDimensionProvenance::default();
        };
        let navigation = code.navigation_index(self.editor());
        let nodes: BTreeMap<_, _> = navigation
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.id.as_str(),
                    entry.nodes.iter().copied().collect::<BTreeSet<_>>(),
                )
            })
            .collect();
        let panel = code.declaration_panel_projection(self.editor());
        let declarations = flattened_declarations(&panel.declarations);
        let authored = code.authored_metadata_cached().ok();
        let default = authored
            .as_ref()
            .and_then(|metadata| metadata.document.dimensions.as_ref())
            .and_then(|defaults| defaults.are_key_constraints_by_default)
            .unwrap_or(false);
        let mut managed = ManagedDimensionProvenance::default();
        for declaration in &declarations {
            if let Some(owned) = nodes.get(declaration.id.as_str()) {
                for node in owned {
                    managed.labels.insert(*node, declaration.label.clone());
                }
                if declaration.kind.starts_with("dimension.") {
                    let presentation = authored
                        .as_ref()
                        .and_then(|metadata| metadata.declarations.get(&declaration.symbol));
                    if presentation
                        .and_then(|presentation| presentation.is_key_constraint)
                        .unwrap_or(default)
                    {
                        managed.priority.extend(owned);
                    }
                    if let Some(presentation) =
                        self.declaration_metadata_snapshot(&declaration.symbol, true)
                    {
                        for node in owned {
                            managed.presentations.insert(*node, presentation.clone());
                        }
                    }
                }
            }
            for member in &declaration.generated {
                if let Some(owned) = nodes.get(member.id.as_str()) {
                    managed.generated.extend(owned);
                    for node in owned {
                        managed
                            .labels
                            .insert(*node, format!("{} · {}", declaration.label, member.label));
                    }
                }
            }
            if declaration.kind == "Patch invocation"
                && let Some(owned) = nodes.get(declaration.id.as_str())
            {
                managed.generated.extend(owned);
            }
        }
        let Ok(manifest) = code.managed_controls_cached() else {
            return managed;
        };
        let mut controls: Vec<_> = manifest.controls.iter().collect();
        controls.sort_by_key(|control| control.source.span.start);
        for control in controls {
            let Some((value, unit)) = managed_value_text(&control.value) else {
                continue;
            };
            if !control.is_public_parameter
                && !unit
                    .as_deref()
                    .is_some_and(|unit| matches!(unit, "mm" | "cm" | "m" | "inch" | "deg" | "rad"))
            {
                continue;
            }
            let mut owners = BTreeSet::new();
            for declaration in &declarations {
                // Authored dimensions already have one complete Inspector row.
                // This section supplies public patch and geometry source values.
                if declaration.kind.starts_with("dimension.") && !control.is_public_parameter {
                    continue;
                }
                let is_owner = declaration.symbol == control.source.declaration
                    || control
                        .consumers
                        .iter()
                        .any(|consumer| match &consumer.target {
                            geosolve_sketch_code::ManagedControlConsumerTarget::Declaration {
                                declaration: owner,
                                ..
                            } => owner == &declaration.symbol,
                            geosolve_sketch_code::ManagedControlConsumerTarget::Generated {
                                address,
                                ..
                            } => declaration
                                .generated
                                .iter()
                                .any(|member| member.address == *address),
                        });
                if is_owner && let Some(owned) = nodes.get(declaration.id.as_str()) {
                    owners.extend(owned);
                    if declaration.kind == "Patch invocation" {
                        managed.patch_controls.insert(control.id.0.clone());
                    }
                }
            }
            if owners.is_empty() && !control.is_public_parameter {
                continue;
            }
            let default_priority = control.presentation.is_key_parameter.unwrap_or(false);
            if control.is_public_parameter {
                managed.patch_controls.insert(control.id.0.clone());
            }
            cache.parameters.push((
                DimensionalParameter {
                    id: control.id.0.clone(),
                    label: managed_control_label(control),
                    value,
                    unit,
                    editable: panel.blocked_reason.is_none()
                        && matches!(control.access, ManagedControlAccess::Editable { .. }),
                    default_priority,
                    row_key: format!("{}:{}", self.dimension_instance(), control.id.0),
                    metadata: self.parameter_metadata_snapshot(control),
                    consumers: self.parameter_consumer_labels(control),
                },
                owners,
            ));
        }
        managed
    }
}

fn distinguish_dimension_names(dimensions: &mut BTreeMap<SelectionItem, DimensionMetadata>) {
    let mut counts = BTreeMap::<String, usize>::new();
    for metadata in dimensions.values() {
        *counts.entry(metadata.label.clone()).or_default() += 1;
    }
    let mut occurrences = BTreeMap::<String, usize>::new();
    for metadata in dimensions.values_mut() {
        if counts[&metadata.label] > 1 {
            let occurrence = occurrences.entry(metadata.label.clone()).or_default();
            *occurrence += 1;
            metadata.label = format!("{} · Dimension {occurrence}", metadata.label);
        }
    }
}

fn flattened_declarations(rows: &[ManagedDeclarationPanelRow]) -> Vec<&ManagedDeclarationPanelRow> {
    rows.iter()
        .flat_map(|row| std::iter::once(row).chain(flattened_declarations(&row.closure_helpers)))
        .collect()
}

fn dimensional_inspector_target(
    inspector: &IntentInspectorProjection,
) -> Option<(IntentInspectorEditTarget, IntentUnit)> {
    let mut targets = inspector.fields.iter().filter_map(|field| match field {
        IntentInspectorField::Definition {
            definition,
            value:
                Some(IntentLiteral::Quantity {
                    unit: unit @ (IntentUnit::Length | IntentUnit::Angle),
                    ..
                }),
        } => Some((
            IntentInspectorEditTarget::Definition {
                field: definition.clone(),
            },
            *unit,
        )),
        IntentInspectorField::Instance {
            leaf,
            value:
                Some(IntentLiteral::Quantity {
                    unit: unit @ (IntentUnit::Length | IntentUnit::Angle),
                    ..
                }),
        } => Some((IntentInspectorEditTarget::Instance { leaf: *leaf }, *unit)),
        _ => None,
    });
    let target = targets.next()?;
    targets.next().is_none().then_some(target)
}

fn persistent_identity(key: AnnotationLayoutKey) -> String {
    format!(
        "{}:{}:{:?}:{:?}:{:?}",
        key.document, key.source, key.item, key.kind, key.marker_index
    )
}

impl WorkbenchBridge {
    fn dimension_measurement_value(
        &self,
        entry: &SceneDimensionEntry,
        scene: &EditorScene,
    ) -> String {
        let metadata = entry.target_metadata(scene);
        let value = if entry.reference {
            let SelectionItem::Dimension(id) = entry.key.item else {
                return "Unavailable".into();
            };
            self.editor().presentation_session()
                .and_then(geosolve_sketch::RetainedSketchDocumentSession::accepted_state_for_current_input)
                .filter(|accepted| accepted.document().id() == entry.key.document)
                .and_then(|accepted| accepted.reference_value(id))
                .and_then(|value| geosolve_constraint_editor::display_dimension_target(value, metadata?.unit))
                .map(|display| display.value)
        } else {
            metadata.map(|metadata| metadata.display_value)
        };
        value
            .filter(|value| value.is_finite())
            .map_or_else(|| "Unavailable".into(), |value| value.to_string())
    }
}

fn finite_edit_value(value: &serde_json::Value) -> Result<f64, String> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        .filter(|value| value.is_finite())
        .ok_or_else(|| "Dimension value must be a finite number".into())
}

const fn dimension_kind(kind: SceneAnnotationKind) -> &'static str {
    match kind {
        SceneAnnotationKind::PointDistance => "Point distance",
        SceneAnnotationKind::CurveLength => "Curve length",
        SceneAnnotationKind::Radius => "Radius",
        SceneAnnotationKind::Diameter => "Diameter",
        SceneAnnotationKind::OrientedAngle => "Angle",
        SceneAnnotationKind::SupportingLineOffset => "Line offset",
        SceneAnnotationKind::ExactTranslatedSegmentOffset => "Segment offset",
        SceneAnnotationKind::ProfileOffset => "Profile offset",
        SceneAnnotationKind::Constraint(_) => "Constraint",
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModePayload {
    mode: DisplayMode,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinPayload {
    id: String,
    pinned: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditPayload {
    id: String,
    value: serde_json::Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HoverPayload {
    x: f64,
    y: f64,
}

#[cfg(test)]
mod tests;
