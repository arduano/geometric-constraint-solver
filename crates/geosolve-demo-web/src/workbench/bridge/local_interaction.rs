// SPDX-License-Identifier: GPL-3.0-or-later
//! Detached canvas navigation and independently retained accepted-model browsing.
//! Navigation owns no solver. Browsing can describe source edits but cannot publish them.

use super::*;
use geosolve_constraint_editor::{
    AnnotationLayoutState, ConstraintEditor, CurvePickContext, DimensionDisplayMode,
    DimensionPresentationContext, DimensionPresentationState, GeometryInteractionPolicy,
    SelectionPresentationState, Viewport,
};
use std::collections::BTreeMap;

mod presence;

const FORMAT: &str = "geosolve-local-interaction-v1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LocalDimensionSeed {
    pub state: DimensionPresentationState,
    pub context: DimensionPresentationContext,
    pub layout: AnnotationLayoutState,
    pub ids: BTreeMap<String, geosolve_constraint_editor::AnnotationLayoutKey>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InteractionSeed {
    format: String,
    scene_key: String,
    scene: String,
    title: String,
    host_size_received: bool,
    semantic_preview: bool,
    selection: Vec<SelectionItem>,
    curve_picks: Vec<CurvePickContext>,
    grid_visible: bool,
    policy: GeometryInteractionPolicy,
    dimensions: LocalDimensionSeed,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bindings: Option<geosolve_constraint_editor::ProjectionalPresentationBindings>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    point_targets: BTreeMap<geosolve_sketch::DesignPointId, serde_json::Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    presence_bindings: BTreeMap<String, Vec<geosolve_constraint_editor::IntentNativeBinding>>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InteractionState {
    format: String,
    scene_key: String,
    viewport: Viewport,
    selection: Vec<SelectionItem>,
    curve_picks: Vec<CurvePickContext>,
    grid_visible: bool,
    dimension_mode: String,
    dimension_pins: Vec<String>,
    dimension_focus: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InteractionUpdate {
    frame: FrameSnapshot,
    state: InteractionState,
    selection_changed: bool,
    server_frame_compatible: bool,
}

fn mode_name(mode: DimensionDisplayMode) -> &'static str {
    match mode {
        DimensionDisplayMode::Focused => "focused",
        DimensionDisplayMode::All => "all",
        DimensionDisplayMode::Hidden => "hidden",
    }
}
fn mode(value: &str) -> Result<DimensionDisplayMode, String> {
    match value {
        "focused" => Ok(DimensionDisplayMode::Focused),
        "all" => Ok(DimensionDisplayMode::All),
        "hidden" => Ok(DimensionDisplayMode::Hidden),
        _ => Err("Unknown dimension mode".into()),
    }
}
fn camera(viewport: Viewport) -> Result<super::super::scene::CanvasCamera, String> {
    if viewport
        .screen_size
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0 || *v > MAX_HOST_EXTENT)
    {
        return Err("Invalid local viewport extent".into());
    }
    let mut camera = super::super::scene::CanvasCamera::new(
        viewport.model_center,
        viewport.pixels_per_model_unit,
    )
    .ok_or("Invalid local camera")?;
    camera.resize(viewport.screen_size);
    Ok(camera)
}

impl WorkbenchBridge {
    fn local_scene_key(&self) -> String {
        serde_json::json!({"geometry":self.navigation_geometry_key(),
            "policy":self.editor().editor().geometry_interaction_policy(),
            "design":self.retained_scene.as_ref().map(|scene|format!("{:?}",scene.design_identity))
        })
        .to_string()
    }
    pub(crate) fn interaction_snapshot_json(&mut self) -> Result<String, String> {
        let snapshot = self.snapshot()?;
        let scene = self
            .retained_scene
            .as_ref()
            .ok_or("No accepted interaction scene")?;
        let seed = InteractionSeed {
            format: FORMAT.into(),
            scene_key: self.local_scene_key(),
            scene: scene.to_detached_json().map_err(|e| e.to_string())?,
            title: self.title.clone(),
            host_size_received: self.host_size_received,
            semantic_preview: self.active_tool != "select"
                || self.construction_preview.is_some()
                || self
                    .editor()
                    .editor()
                    .draft_inference_resolution()
                    .is_some()
                || self.editor().feature_authoring_preview_item().is_some(),
            selection: self.editor().editor().selection().to_vec(),
            curve_picks: self
                .editor()
                .editor()
                .selection_presentation_state()
                .curve_picks,
            grid_visible: self.grid_visible,
            policy: self.editor().editor().geometry_interaction_policy(),
            dimensions: self.local_dimension_seed(),
            bindings: self.editor().presentation_bindings(),
            point_targets: self
                .code_project
                .as_ref()
                .map(|code| code.local_point_targets(self.editor()))
                .unwrap_or_default(),
            presence_bindings: self
                .code_project
                .as_ref()
                .map(|code| code.local_presence_bindings(self.editor()))
                .unwrap_or_default(),
        };
        serde_json::to_string(&serde_json::json!({"snapshot":snapshot,"seed":seed}))
            .map_err(|e| e.to_string())
    }

    pub(crate) fn interaction_apply_json(&mut self, request: &str) -> Result<String, String> {
        let state: InteractionState = decode_request(request)?;
        self.apply_interaction_state(state)?;
        self.snapshot_json()
    }

    fn apply_interaction_state(&mut self, state: InteractionState) -> Result<(), String> {
        if state.format != FORMAT || state.scene_key != self.local_scene_key() {
            return Err("Local interaction belongs to a stale accepted scene".into());
        }
        if self.pending_managed_mutation.is_some()
            || self.captured_pointer.is_some()
            || self.editor().editor().active_pointer_gesture().is_some()
        {
            return Err("Finish the active edit before applying local presentation".into());
        }
        let camera = camera(state.viewport)?;
        let mode = mode(&state.dimension_mode)?;
        self.refresh_current_scene();
        let scene = self
            .retained_scene
            .as_ref()
            .ok_or("No accepted interaction scene")?;
        if state.selection.len() > MAX_VISIBILITY_ROWS
            || state.dimension_pins.len() > DimensionPresentationState::MAX_PINS
        {
            return Err("Local selection or pins exceed their bound".into());
        }
        self.validate_local_dimension_pins(&state.dimension_pins)?;
        if let Some(focus) = &state.dimension_focus {
            self.validate_local_dimension_pins(std::slice::from_ref(focus))?;
        }
        let selection = SelectionPresentationState {
            items: state.selection,
            curve_picks: state.curve_picks,
        };
        selection.validate(scene).map_err(|e| e.to_string())?;
        // Camera/dimension refreshes retain explicit Explorer row ownership,
        // including declarations with no visible native selection items.
        if selection != self.editor().editor().selection_presentation_state() {
            let scene = scene.clone();
            self.editor_mut()
                .restore_selection_presentation(&scene, selection)
                .map_err(|e| e.to_string())?;
        }
        // All fallible admission precedes state publication. This changes no authored state.
        self.camera = camera;
        self.host_size = state.viewport.screen_size;
        self.host_size_received = true;
        self.grid_visible = state.grid_visible;
        self.apply_local_dimension_preferences(
            mode,
            &state.dimension_pins,
            state.dimension_focus.as_deref(),
        );
        Ok(())
    }
}

/// Per-tab retained chrome; construction validates one complete accepted model.
/// No editing, compiler or gesture method is exposed by this adapter.
pub(crate) struct BrowsingPresentation {
    bridge: WorkbenchBridge,
    source_key: String,
    native_key: String,
    source_scene: EditorScene,
    mapping: PresentationMapping,
    reverse: PresentationMapping,
    dimension_ids: BTreeMap<String, String>,
    source_dimension_ids: BTreeMap<String, String>,
}
impl BrowsingPresentation {
    pub(crate) fn new(encoded: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            project: String,
            design: super::super::code_projects::WorkspaceDesign,
            seed: InteractionSeed,
        }
        let request: Request = decode_request(encoded)?;
        if request.seed.format != FORMAT {
            return Err("Unsupported browsing scene format".into());
        }
        let project = geosolve_sketch_code::CodeProject::from_json(&request.project)
            .map_err(|e| e.to_string())?;
        let expected_design = serde_json::to_value(&request.design).map_err(|e| e.to_string())?;
        let (code, editor) = CodeProjectWorkbench::open_workspace_design(project, request.design)?;
        let title = code.title().to_owned();
        let mut bridge = WorkbenchBridge::from_parts(
            WorkbenchDocumentAuthority::from_projectional_editor(*editor)?,
            Some(code),
            super::super::samples::SampleCatalogState::default(),
            title,
            String::new(),
        )?;
        let exported: serde_json::Value =
            serde_json::from_str(&bridge.export_project_json()?).map_err(|e| e.to_string())?;
        let actual_project: serde_json::Value = serde_json::from_str(
            exported["contents"]
                .as_str()
                .ok_or("Browsing project export missing contents")?,
        )
        .map_err(|e| e.to_string())?;
        let expected_project: serde_json::Value =
            serde_json::from_str(&request.project).map_err(|e| e.to_string())?;
        let actual_design: serde_json::Value =
            serde_json::from_str(&bridge.workspace_design_json()?).map_err(|e| e.to_string())?;
        if actual_project != expected_project || actual_design != expected_design {
            return Err("Browsing reconstruction disagrees with accepted source/design".into());
        }
        bridge.snapshot()?;
        let source_scene =
            EditorScene::from_detached_json(&request.seed.scene).map_err(|e| e.to_string())?;
        let native_scene = bridge
            .retained_scene
            .as_ref()
            .ok_or("Browsing native scene unavailable")?;
        let mapping = PresentationMapping::new(
            request.seed.bindings.as_ref(),
            bridge.editor().presentation_bindings().as_ref(),
            source_scene.presentation_document().id(),
            native_scene.presentation_document().id(),
        )?;
        let native_key = bridge.local_scene_key();
        let reverse = mapping.reverse()?;
        let native_dimensions = bridge.local_dimension_seed();
        let mut dimension_ids = BTreeMap::new();
        for (id, key) in &request.seed.dimensions.ids {
            let native_key = mapping.layout_key(*key)?;
            let native_id = native_dimensions
                .ids
                .iter()
                .find(|(_, key)| **key == native_key)
                .map(|(id, _)| id)
                .ok_or("Browsing dimension correspondence is unavailable")?;
            dimension_ids.insert(id.clone(), native_id.clone());
        }
        let source_dimension_ids = dimension_ids
            .iter()
            .map(|(source, native)| (native.clone(), source.clone()))
            .collect();
        Ok(Self {
            bridge,
            source_key: request.seed.scene_key,
            native_key,
            source_scene,
            mapping,
            reverse,
            dimension_ids,
            source_dimension_ids,
        })
    }
    pub(crate) fn update_json(&mut self, encoded: &str) -> Result<String, String> {
        self.apply_state(decode_request(encoded)?)?;
        serde_json::to_string(&self.chrome()?).map_err(|e| e.to_string())
    }
    fn apply_state(&mut self, mut state: InteractionState) -> Result<(), String> {
        if state.format != FORMAT
            || state.scene_key != self.source_key
            || self.bridge.local_scene_key() != self.native_key
        {
            return Err("Browsing view belongs to an obsolete accepted model".into());
        }
        SelectionPresentationState {
            items: state.selection.clone(),
            curve_picks: state.curve_picks.clone(),
        }
        .validate(&self.source_scene)
        .map_err(|e| e.to_string())?;
        let native_scene = self
            .bridge
            .retained_scene
            .as_ref()
            .ok_or("Browsing native scene unavailable")?;
        state.selection = state
            .selection
            .into_iter()
            .map(|item| self.mapping.selection(item))
            .collect::<Result<_, _>>()?;
        state.curve_picks = state
            .curve_picks
            .into_iter()
            .map(|pick| self.mapping.curve_pick(pick, native_scene))
            .collect::<Result<_, _>>()?;
        state.scene_key.clone_from(&self.native_key);
        state.dimension_pins = state
            .dimension_pins
            .iter()
            .map(|id| self.native_dimension_id(id))
            .collect::<Result<_, _>>()?;
        state.dimension_focus = state
            .dimension_focus
            .as_deref()
            .map(|id| self.native_dimension_id(id))
            .transpose()?;
        self.bridge.apply_interaction_state(state)
    }
    fn native_dimension_id(&self, source: &str) -> Result<String, String> {
        self.dimension_ids
            .get(source)
            .cloned()
            .ok_or_else(|| "Browsing dimension belongs to another scene".into())
    }
    fn chrome(&mut self) -> Result<serde_json::Value, String> {
        let mut snapshot = self.bridge.snapshot()?;
        snapshot
            .dimensions
            .translate_ids(&self.source_dimension_ids)?;
        // Native source spans describe the accepted canonical source. They must
        // not be applied to a divergent working text without exact basis mapping.
        Ok(serde_json::json!({
            "explorer":snapshot.explorer,"navigation":snapshot.navigation,"dimensions":snapshot.dimensions,
            "authoringDocument":snapshot.authoring_document,"selection":snapshot.selection,
            "parameters":snapshot.parameters,"problems":snapshot.problems,
            "selectedGeometryRole":snapshot.presentation.selected_geometry_role,
        }))
    }
    pub(crate) fn navigate_json(&mut self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            state: InteractionState,
            command: String,
            payload: serde_json::Value,
        }
        let request: Request = decode_request(encoded)?;
        if !matches!(
            request.command.as_str(),
            "navigation.rows.select" | "navigation.source.select"
        ) {
            return Err("Unsupported native browsing navigation".into());
        }
        let mut state = request.state.clone();
        self.apply_state(request.state)?;
        match request.command.as_str() {
            "navigation.rows.select" => self.bridge.select_navigation_rows_json(request.payload)?,
            "navigation.source.select" => {
                self.bridge.select_navigation_source_json(request.payload)?;
            }
            _ => unreachable!("whitelisted navigation"),
        }
        let selection = self.bridge.editor().editor().selection_presentation_state();
        state.selection = selection
            .items
            .into_iter()
            .map(|item| self.reverse.selection(item))
            .collect::<Result<_, _>>()?;
        state.curve_picks = selection
            .curve_picks
            .into_iter()
            .map(|pick| self.reverse.curve_pick(pick, &self.source_scene))
            .collect::<Result<_, _>>()?;
        SelectionPresentationState {
            items: state.selection.clone(),
            curve_picks: state.curve_picks.clone(),
        }
        .validate(&self.source_scene)
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&serde_json::json!({"state":state,"chrome":self.chrome()?}))
            .map_err(|e| e.to_string())
    }
    /// Describe a native source edit without preparing a compiler job or
    /// publishing anything. The server must independently authorize and apply it.
    pub(crate) fn describe_json(&mut self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            state: InteractionState,
            authority: String,
            command: String,
            payload: serde_json::Value,
        }
        let request: Request = decode_request(encoded)?;
        self.apply_state(request.state)?;
        self.bridge
            .validate_metadata_authority(&request.authority)?;
        let mutation = match request.command.as_str() {
            "parameter.edit" => {
                let payload: ParameterPayload = decode_payload(request.payload)?;
                self.bridge
                    .parameter_source_mutation(&payload.id, payload.value)?
            }
            "dimensions.edit" => {
                let mut payload: ParameterPayload = decode_payload(request.payload)?;
                payload.id = self.native_dimension_id(&payload.id)?;
                self.bridge.dimension_source_mutation(
                    serde_json::json!({"id":payload.id,"value":payload.value}),
                )?
            }
            "authoring.metadata.set" => {
                Some(self.bridge.metadata_source_mutation(request.payload)?)
            }
            "declaration.move" => self
                .bridge
                .declaration_move_mutation(&decode_payload(request.payload)?)?,
            "declaration.delete" => {
                let payload: SelectionPayload = decode_payload(request.payload)?;
                if let Some(DeclarationRowTarget::Managed {
                    closure_role: ManagedDeclarationClosureRole::Helper { root },
                    ..
                }) = self.bridge.declaration_row_target(&payload.id)
                {
                    return Err(format!(
                        "Profile Offset helper declarations cannot be deleted independently of `{}`",
                        root.0
                    ));
                }
                Some(ManagedSketchMutation::Delete {
                    target: self.bridge.managed_row_target(&payload.id)?,
                })
            }
            _ => return Err("Unsupported native source edit description".into()),
        };
        serde_json::to_string(&mutation).map_err(|e| e.to_string())
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PredictionGuide {
    Point { position: [f64; 2] },
    Polyline { points: Vec<[f64; 2]>, closed: bool },
    Rectangle { first: [f64; 2], second: [f64; 2] },
    Circle { center: [f64; 2], radius: f64 },
}
impl PredictionGuide {
    fn preview(self) -> geosolve_constraint_editor::ConstructionPreviewGeometry {
        use geosolve_constraint_editor::ConstructionPreviewGeometry as Geometry;
        match self {
            Self::Point { position } => Geometry::Point { position },
            Self::Polyline { mut points, closed } => {
                if closed && let Some(first) = points.first().copied() {
                    points.push(first);
                }
                Geometry::Polyline { points }
            }
            Self::Rectangle { first, second } => Geometry::Rectangle { first, second },
            Self::Circle { center, radius } => Geometry::Circle { center, radius },
        }
    }
    fn inference(self) -> Result<geosolve_constraint_editor::DraftGuideGeometry, String> {
        use geosolve_constraint_editor::DraftGuideGeometry as Geometry;
        match self {
            Self::Point { position } => Ok(Geometry::Point { position }),
            Self::Polyline {
                points,
                closed: false,
            } if points.len() == 2 => Ok(Geometry::Segment {
                start: points[0],
                end: points[1],
            }),
            _ => Err("Unsupported native prediction inference guide".into()),
        }
    }
}

struct PresentationMapping {
    source: geosolve_sketch::DocumentId,
    destination: geosolve_sketch::DocumentId,
    bindings: BTreeMap<
        geosolve_constraint_editor::IntentNativeBinding,
        geosolve_constraint_editor::IntentNativeBinding,
    >,
}
impl PresentationMapping {
    fn reverse(&self) -> Result<Self, String> {
        let mut bindings = BTreeMap::new();
        for (source, destination) in &self.bindings {
            if bindings
                .insert(*destination, *source)
                .is_some_and(|previous| previous != *source)
            {
                return Err("Presentation namespace correspondence is not reversible".into());
            }
        }
        Ok(Self {
            source: self.destination,
            destination: self.source,
            bindings,
        })
    }
    fn new(
        source: Option<&geosolve_constraint_editor::ProjectionalPresentationBindings>,
        destination: Option<&geosolve_constraint_editor::ProjectionalPresentationBindings>,
        source_document: geosolve_sketch::DocumentId,
        destination_document: geosolve_sketch::DocumentId,
    ) -> Result<Self, String> {
        let mut bindings = BTreeMap::new();
        if source_document != destination_document {
            let (Some(source), Some(destination)) = (source, destination) else {
                return Err("Prediction namespace correspondence is unavailable".into());
            };
            if source.document != source_document || destination.document != destination_document {
                return Err("Prediction namespace correspondence belongs to another scene".into());
            }
            for (symbol, before) in &source.nodes {
                let after = destination
                    .nodes
                    .get(symbol)
                    .ok_or("Prediction lost a source-owned declaration")?;
                if before.len() != after.len() {
                    return Err("Prediction declaration ownership changed".into());
                }
                for (before, after) in before.iter().zip(after) {
                    if std::mem::discriminant(before) != std::mem::discriminant(after) {
                        return Err("Prediction declaration ownership kind changed".into());
                    }
                    if let Some(previous) = bindings.insert(*before, *after)
                        && previous != *after
                    {
                        return Err("Prediction namespace correspondence is ambiguous".into());
                    }
                }
            }
        }
        Ok(Self {
            source: source_document,
            destination: destination_document,
            bindings,
        })
    }
    fn binding(
        &self,
        source: geosolve_constraint_editor::IntentNativeBinding,
    ) -> Result<geosolve_constraint_editor::IntentNativeBinding, String> {
        if self.source == self.destination {
            return Ok(source);
        }
        self.bindings
            .get(&source)
            .copied()
            .ok_or_else(|| "Prediction has no exact presentation binding".into())
    }
    fn selection(&self, item: SelectionItem) -> Result<SelectionItem, String> {
        use geosolve_constraint_editor::IntentNativeBinding as Binding;
        Ok(match item {
            SelectionItem::Point(point) => match self.binding(Binding::Point(point))? {
                Binding::Point(point) => SelectionItem::Point(point),
                _ => return Err("Prediction point binding changed kind".into()),
            },
            SelectionItem::Curve(span) => match self.binding(Binding::Curve(span.curve))? {
                Binding::Curve(curve) => SelectionItem::Curve(geosolve_sketch::CurveSpan {
                    curve,
                    segment: span.segment,
                }),
                _ => return Err("Prediction curve binding changed kind".into()),
            },
            SelectionItem::Dimension(dimension) => {
                match self.binding(Binding::Dimension(dimension))? {
                    Binding::Dimension(dimension) => SelectionItem::Dimension(dimension),
                    _ => return Err("Prediction dimension binding changed kind".into()),
                }
            }
            SelectionItem::Constraint(constraint) => {
                match self.binding(Binding::Constraint(constraint))? {
                    Binding::Constraint(constraint) => SelectionItem::Constraint(constraint),
                    _ => return Err("Prediction constraint binding changed kind".into()),
                }
            }
            SelectionItem::Feature(feature) => {
                match self.binding(Binding::ComputedFeature(feature))? {
                    Binding::ComputedFeature(feature) => SelectionItem::Feature(feature),
                    _ => return Err("Prediction feature binding changed kind".into()),
                }
            }
            SelectionItem::FeatureCorner(corner) => match (
                self.binding(Binding::ComputedFeature(corner.feature))?,
                self.binding(Binding::ComputedFeatureCorner(corner.corner))?,
            ) {
                (Binding::ComputedFeature(feature), Binding::ComputedFeatureCorner(corner)) => {
                    SelectionItem::FeatureCorner(geosolve_sketch_features::ComputedCornerRef {
                        feature,
                        corner,
                    })
                }
                _ => return Err("Prediction corner binding changed kind".into()),
            },
            SelectionItem::Datum(datum) => SelectionItem::Datum(datum),
        })
    }
    fn curve_pick(
        &self,
        pick: CurvePickContext,
        scene: &EditorScene,
    ) -> Result<CurvePickContext, String> {
        use geosolve_constraint_editor::SceneCurveOrigin;
        let SelectionItem::Curve(span) = self.selection(SelectionItem::Curve(pick.span))? else {
            return Err("Browsing picked curve changed kind".into());
        };
        let origin = match pick.origin {
            SceneCurveOrigin::Native => SceneCurveOrigin::Native,
            SceneCurveOrigin::FilletDiscarded {
                source,
                interval,
                provenance,
                ..
            } => {
                let SelectionItem::Curve(source_span) =
                    self.selection(SelectionItem::Curve(source.span))?
                else {
                    return Err("Browsing implicit curve source changed kind".into());
                };
                let SelectionItem::FeatureCorner(owner) =
                    self.selection(SelectionItem::FeatureCorner(provenance.owner))?
                else {
                    return Err("Browsing implicit curve owner changed kind".into());
                };
                let mut origins = scene.curves.iter().filter_map(|curve| match curve.origin {
                    SceneCurveOrigin::FilletDiscarded {
                        source,
                        interval: native_interval,
                        provenance: native_provenance,
                        ..
                    } if curve.span == span
                        && source.span == source_span
                        && native_interval == interval
                        && native_provenance.owner == owner
                        && native_provenance.endpoint == provenance.endpoint
                        && native_provenance.base_interval == provenance.base_interval =>
                    {
                        Some(curve.origin)
                    }
                    _ => None,
                });
                let origin = origins
                    .next()
                    .ok_or("Browsing picked implicit curve is unavailable")?;
                if origins.next().is_some() {
                    return Err("Browsing picked implicit curve is ambiguous".into());
                }
                origin
            }
        };
        Ok(CurvePickContext {
            span,
            parameter: pick.parameter,
            origin,
        })
    }
    fn layout_key(
        &self,
        key: geosolve_constraint_editor::AnnotationLayoutKey,
    ) -> Result<geosolve_constraint_editor::AnnotationLayoutKey, String> {
        use geosolve_constraint_editor::IntentNativeBinding as Binding;
        if key.document != self.source {
            return Err("Prediction annotation belongs to another document".into());
        }
        let Binding::Source(source) = self.binding(Binding::Source(key.source))? else {
            return Err("Prediction annotation source changed kind".into());
        };
        Ok(geosolve_constraint_editor::AnnotationLayoutKey {
            document: self.destination,
            source,
            item: self.selection(key.item)?,
            ..key
        })
    }
    fn dimensions(&self, dimensions: &mut LocalDimensionSeed) -> Result<(), String> {
        if self.source == self.destination {
            return Ok(());
        }
        dimensions.ids = dimensions
            .ids
            .iter()
            .map(|(id, key)| Ok((id.clone(), self.layout_key(*key)?)))
            .collect::<Result<_, String>>()?;
        dimensions.layout = AnnotationLayoutState::from_entries(
            dimensions
                .layout
                .entries()
                .into_iter()
                .map(|entry| {
                    Ok(geosolve_constraint_editor::AnnotationLayoutEntry {
                        key: self.layout_key(entry.key)?,
                        placement: entry.placement,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        );
        dimensions.context.generated = dimensions
            .context
            .generated
            .iter()
            .map(|item| self.selection(*item))
            .collect::<Result<_, _>>()?;
        dimensions.context.default_priority = dimensions
            .context
            .default_priority
            .iter()
            .map(|item| self.selection(*item))
            .collect::<Result<_, _>>()?;
        dimensions.context.hovered = dimensions
            .context
            .hovered
            .map(|item| self.selection(item))
            .transpose()?;
        dimensions.context.active = dimensions
            .context
            .active
            .map(|item| self.selection(item))
            .transpose()?;
        // Namespace-specific automatic caches are rebuilt once against the exact
        // prediction scene; authored priorities/manual placements are retained.
        dimensions.state = DimensionPresentationState::default();
        Ok(())
    }
}

/// Converts a detached native prediction into paint only. No accepted-scene owner
/// or native editing handle is constructed from this transport.
pub(crate) fn authoring_preview_json(encoded: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct View {
        seed: InteractionSeed,
        state: InteractionState,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Construction {
        preview: Option<PredictionGuide>,
        inference_guides: Vec<PredictionGuide>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        scene: String,
        bindings: Option<geosolve_constraint_editor::ProjectionalPresentationBindings>,
        view: View,
        construction: Option<Construction>,
    }
    let request: Request = decode_request(encoded)?;
    let state = request.view.state;
    if state.format != FORMAT || state.scene_key != request.view.seed.scene_key {
        return Err("Prediction view belongs to a stale accepted scene".into());
    }
    let mut local = LocalInteraction::new(
        &serde_json::to_string(&request.view.seed).map_err(|error| error.to_string())?,
    )?;
    let scene = EditorScene::from_detached_json(&request.scene).map_err(|e| e.to_string())?;
    let mapping = PresentationMapping::new(
        request.view.seed.bindings.as_ref(),
        request.bindings.as_ref(),
        local.scene.presentation_document().id(),
        scene.presentation_document().id(),
    )?;
    mapping.dimensions(&mut local.dimensions)?;
    local.scene = scene;
    local.camera = camera(state.viewport)?;
    local.grid_visible = state.grid_visible;
    local
        .editor
        .restore_selection_presentation(
            &local.scene,
            SelectionPresentationState {
                items: state
                    .selection
                    .into_iter()
                    .map(|item| mapping.selection(item))
                    .collect::<Result<_, _>>()?,
                // Paint has no curve-pick authority. Accepted navigation retains
                // exact native/implicit occurrence contexts independently.
                curve_picks: Vec::new(),
            },
        )
        .map_err(|e| e.to_string())?;
    local.dimensions.state.mode = mode(&state.dimension_mode)?;
    if state.dimension_pins.len() > DimensionPresentationState::MAX_PINS {
        return Err("Too many prediction dimension pins".into());
    }
    let key = |id: &String| {
        local
            .dimensions
            .ids
            .get(id)
            .copied()
            .ok_or_else(|| "Stale prediction dimension".to_string())
    };
    local.dimensions.state.pins = state
        .dimension_pins
        .iter()
        .map(key)
        .collect::<Result<_, _>>()?;
    local.dimensions.state.focus = state.dimension_focus.as_ref().map(key).transpose()?;
    local.dimensions.context.navigation_active = true;
    let mut frame = local.compose_frame()?;
    if let Some(construction) = request.construction {
        let preview = construction.preview.map(PredictionGuide::preview);
        let guides = construction
            .inference_guides
            .into_iter()
            .map(PredictionGuide::inference)
            .collect::<Result<Vec<_>, _>>()?;
        let overlay = geosolve_sketch_render::compose_prediction_guides(
            state.viewport,
            preview.as_ref(),
            &guides,
        )
        .map_err(|e| e.to_string())?;
        frame.scene.items.extend(overlay.items);
    }
    frame
        .scene
        .provenance
        .insert("scene".into(), "provisional".into());
    for item in &mut frame.scene.items {
        item.interactive = false;
    }
    frame.scene.validate().map_err(|e| e.to_string())?;
    frame.aria_label = format!("{} provisional sketch viewport", local.title);
    serde_json::to_string(&frame).map_err(|e| e.to_string())
}

/// Detached presentation session shared by native tests and the browser worker.
pub(crate) struct LocalInteraction {
    scene_key: String,
    scene: EditorScene,
    title: String,
    editor: ConstraintEditor,
    camera: super::super::scene::CanvasCamera,
    grid_visible: bool,
    dimensions: LocalDimensionSeed,
    pan: Option<CanvasPanGesture>,
    dimension_hover: Option<(SelectionItem, ScreenPoint)>,
    host_size_received: bool,
    point_targets: BTreeMap<geosolve_sketch::DesignPointId, serde_json::Value>,
    presence_bindings: BTreeMap<String, Vec<geosolve_constraint_editor::IntentNativeBinding>>,
    presence: presence::PresenceState,
}
impl LocalInteraction {
    pub(crate) fn new(encoded: &str) -> Result<Self, String> {
        let seed: InteractionSeed = decode_request(encoded)?;
        if seed.format != FORMAT {
            return Err("Unsupported local interaction format".into());
        }
        let scene = EditorScene::from_detached_json(&seed.scene).map_err(|e| e.to_string())?;
        if seed
            .bindings
            .as_ref()
            .is_some_and(|bindings| bindings.document != scene.presentation_document().id())
        {
            return Err("Local presentation bindings belong to another scene".into());
        }
        let mut editor = ConstraintEditor::default();
        editor.set_geometry_interaction_policy(seed.policy);
        editor
            .restore_selection_presentation(
                &scene,
                SelectionPresentationState {
                    items: seed.selection,
                    curve_picks: seed.curve_picks,
                },
            )
            .map_err(|e| e.to_string())?;
        let camera = camera(scene.viewport)?;
        Ok(Self {
            scene_key: seed.scene_key,
            scene,
            title: seed.title,
            editor,
            camera,
            grid_visible: seed.grid_visible,
            dimensions: seed.dimensions,
            pan: None,
            dimension_hover: None,
            host_size_received: seed.host_size_received,
            point_targets: seed.point_targets,
            presence_bindings: seed.presence_bindings,
            presence: presence::PresenceState::default(),
        })
    }
    fn state(&self) -> InteractionState {
        InteractionState {
            format: FORMAT.into(),
            scene_key: self.scene_key.clone(),
            viewport: self.camera.viewport(),
            selection: self.editor.selection().to_vec(),
            curve_picks: self.editor.selection_presentation_state().curve_picks,
            grid_visible: self.grid_visible,
            dimension_mode: mode_name(self.dimensions.state.mode).into(),
            dimension_focus: self
                .dimensions
                .ids
                .iter()
                .find(|(_, key)| Some(**key) == self.dimensions.state.focus)
                .map(|(id, _)| id.clone()),
            dimension_pins: self
                .dimensions
                .ids
                .iter()
                .filter(|(_, key)| self.dimensions.state.pins.contains(key))
                .map(|(id, _)| id.clone())
                .collect(),
        }
    }
    pub(crate) fn authoring_pointer_json(&self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            x: f64,
            y: f64,
            #[serde(default)]
            captured: bool,
        }
        let input: Input = decode_request(encoded)?;
        let position = self
            .normalized_position([input.x, input.y], input.captured)
            .ok_or("Invalid authoring pointer position")?;
        let item = self.editor.select_pointer_item(&self.scene, position);
        let target = match item {
            Some(SelectionItem::Point(point)) => self.point_targets.get(&point),
            _ => None,
        };
        serde_json::to_string(&serde_json::json!({"viewport":self.camera.viewport(),
            "position":self.camera.viewport().screen_to_model(position),"target":target}))
        .map_err(|e| e.to_string())
    }
    pub(crate) fn state_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.state()).map_err(|e| e.to_string())
    }
    pub(crate) fn restore_selection_json(&mut self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            expected: InteractionState,
            state: InteractionState,
        }
        let request: Request = decode_request(encoded)?;
        if request.expected != self.state() {
            return Err("Browsing navigation belongs to an obsolete local view".into());
        }
        let mut permitted = request.expected;
        permitted.selection.clone_from(&request.state.selection);
        permitted.curve_picks.clone_from(&request.state.curve_picks);
        if permitted != request.state {
            return Err("Browsing navigation may only replace selection".into());
        }
        let selection = SelectionPresentationState {
            items: permitted.selection,
            curve_picks: permitted.curve_picks,
        };
        self.editor
            .restore_selection_presentation(&self.scene, selection)
            .map_err(|e| e.to_string())?;
        self.compose(true)
    }
    pub(crate) fn presence_json(&mut self, encoded: &str) -> Result<String, String> {
        let next = presence::PresenceState::decode(encoded, &self.scene_key)?;
        let previous = std::mem::replace(&mut self.presence, next);
        let result = self.compose(false);
        if result.is_err() {
            self.presence = previous;
        }
        result
    }
    fn compose(&mut self, selection_changed: bool) -> Result<String, String> {
        self.compose_for_replacement(selection_changed, false)
    }
    fn compose_for_replacement(
        &mut self,
        selection_changed: bool,
        server_frame_compatible: bool,
    ) -> Result<String, String> {
        let frame = self.compose_frame()?;
        serde_json::to_string(&InteractionUpdate {
            frame,
            state: self.state(),
            selection_changed,
            server_frame_compatible,
        })
        .map_err(|e| e.to_string())
    }
    fn compose_frame(&mut self) -> Result<FrameSnapshot, String> {
        if self.scene.viewport != self.camera.viewport() {
            self.scene
                .reproject_viewport(self.camera.viewport())
                .map_err(|e| e.to_string())?;
        }
        self.editor
            .populate_curve_controls(&mut self.scene)
            .map_err(|e| e.to_string())?;
        self.dimensions.context.selection = self.editor.selection().to_vec();
        self.dimensions.state.apply(
            &mut self.scene,
            &self.dimensions.layout,
            &self.dimensions.context,
        );
        let mut scene = geosolve_sketch_render::compose_draw_frame(
            Some(&self.scene),
            None,
            &[],
            self.editor.selection(),
            &[],
            &[],
            self.editor.hover_state(),
            None,
            None,
            None,
            None,
            None,
            self.editor.geometry_interaction_policy(),
            super::super::scene::CanvasDisplayOptions {
                grid_visible: self.grid_visible,
                retain_contextual_annotations: true,
            },
            None,
            self.camera.viewport(),
        )
        .map_err(|e| e.to_string())?;
        self.presence.append(
            &mut scene,
            &self.scene,
            self.editor.geometry_interaction_policy(),
            &self.presence_bindings,
        )?;
        Ok(FrameSnapshot {
            scene,
            aria_label: format!("{} accepted sketch viewport", self.title),
        })
    }
    pub(crate) fn replace_json(&mut self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Replacement {
            seed: InteractionSeed,
            preserve_selection: bool,
        }
        let request: Replacement = decode_request(encoded)?;
        let mut next =
            Self::new(&serde_json::to_string(&request.seed).map_err(|e| e.to_string())?)?;
        next.camera = self.camera;
        next.host_size_received = self.host_size_received;
        next.grid_visible = self.grid_visible;
        next.dimensions.state.mode = self.dimensions.state.mode;
        next.dimensions.state.pins = self
            .dimensions
            .state
            .pins
            .iter()
            .copied()
            .filter(|pin| next.dimensions.ids.values().any(|key| key == pin))
            .collect();
        next.dimensions.state.focus = self
            .dimensions
            .state
            .focus
            .filter(|focus| next.dimensions.ids.values().any(|key| key == focus));
        if self.scene_key == next.scene_key {
            next.dimensions.state = self.dimensions.state.clone();
        }
        next.pan = self.pan;
        next.dimension_hover = if self.scene_key == next.scene_key {
            self.dimension_hover
        } else {
            None
        };
        next.dimensions.context.navigation_active = self.dimensions.context.navigation_active;
        if request.preserve_selection && self.scene_key == next.scene_key {
            next.editor = self.editor.clone();
            next.dimensions.context.hovered = self.dimensions.context.hovered;
        } else if request.preserve_selection {
            let mut selected = self.editor.selection_presentation_state();
            selected.items.retain(|item| {
                SelectionPresentationState {
                    items: vec![*item],
                    curve_picks: vec![],
                }
                .validate(&next.scene)
                .is_ok()
            });
            selected.curve_picks.retain(|pick| {
                let item = SelectionItem::Curve(pick.span);
                let valid = selected.items.contains(&item)
                    && SelectionPresentationState {
                        items: vec![item],
                        curve_picks: vec![*pick],
                    }
                    .validate(&next.scene)
                    .is_ok();
                if !valid {
                    selected.items.retain(|selected| *selected != item);
                }
                valid
            });
            next.editor
                .restore_selection_presentation(&next.scene, selected)
                .map_err(|e| e.to_string())?;
        }
        next.editor
            .set_geometry_interaction_policy(request.seed.policy);
        // Exact server draft/inference paint may survive an unchanged local view.
        // Navigation never paints a preview in an obsolete camera coordinate space.
        let server_frame_compatible = request.seed.semantic_preview
            && next.scene.viewport == next.camera.viewport()
            && next.state().selection == request.seed.selection
            && next.state().curve_picks == request.seed.curve_picks;
        let result = next.compose_for_replacement(true, server_frame_compatible)?;
        *self = next;
        Ok(result)
    }
    pub(crate) fn wheel_json(&mut self, encoded: &str) -> Result<String, String> {
        let request: WheelInputRequest = decode_request(encoded)?;
        let samples = match request {
            WheelInputRequest::Single(v) => vec![v],
            WheelInputRequest::Batch(v) => {
                require_version(v.version)?;
                v.samples
            }
        };
        if samples.is_empty() || samples.len() > 256 {
            return Err("wheel batch requires between 1 and 256 samples".into());
        }
        let mut next = self.camera;
        for sample in samples {
            require_version(sample.version)?;
            if ![sample.x, sample.y, sample.delta_x, sample.delta_y]
                .into_iter()
                .all(f64::is_finite)
            {
                return Err("wheel coordinates and deltas must be finite".into());
            }
            let anchor = self
                .normalized_position([sample.x, sample.y], false)
                .ok_or("wheel anchor is outside the fitted sketch plane")?;
            next.zoom_about(anchor, (-sample.delta_y * 0.0015).exp());
        }
        self.editor.cancel();
        self.pan = None;
        self.camera = next;
        self.dimensions.context.navigation_active = true;
        self.dimensions.context.hovered = None;
        self.dimension_hover = None;
        self.compose(false)
    }
    pub(crate) fn resize_json(&mut self, encoded: &str) -> Result<String, String> {
        let request: ResizeRequest = decode_request(encoded)?;
        require_version(request.version)?;
        if !request.pixel_ratio.is_finite()
            || request.pixel_ratio <= 0.0
            || request.pixel_ratio > MAX_PIXEL_RATIO
        {
            return Err("Invalid pixel ratio".into());
        }
        let next = camera(Viewport {
            screen_size: [request.width, request.height],
            ..self.camera.viewport()
        })?;
        if next == self.camera {
            self.host_size_received = true;
            return Ok("null".into());
        }
        self.reset_navigation();
        self.camera = next;
        if !self.host_size_received {
            self.camera.fit_scene(&self.scene);
            self.dimensions.state.reconsider_hidden_dimensions();
        }
        self.host_size_received = true;
        self.compose(false)
    }
    fn reset_navigation(&mut self) {
        self.editor.cancel();
        self.pan = None;
        self.dimensions.context.navigation_active = false;
        self.dimensions.context.hovered = None;
        self.dimension_hover = None;
    }
    pub(crate) fn cancel_json(&mut self, encoded: &str) -> Result<String, String> {
        let request: CancelRequest = decode_request(encoded)?;
        require_version(request.version)?;
        self.editor.cancel();
        self.pan = None;
        self.dimensions.context.navigation_active = false;
        self.dimensions.context.hovered = None;
        self.dimension_hover = None;
        self.compose(false)
    }
    fn normalized_position(&self, client: [f64; 2], captured: bool) -> Option<ScreenPoint> {
        let size = self.camera.viewport().screen_size;
        let rect = super::super::effect_adapter::ClientRect {
            left: 0.0,
            top: 0.0,
            width: size[0],
            height: size[1],
        };
        if captured {
            super::super::effect_adapter::normalize_captured_client_point(rect, size, client)
        } else {
            super::super::effect_adapter::normalize_client_point(rect, size, client)
        }
    }
    pub(crate) fn pointer_json(&mut self, encoded: &str) -> Result<String, String> {
        let request: PointerRequest = decode_request(encoded)?;
        require_version(request.version)?;
        if ![request.x, request.y].into_iter().all(f64::is_finite)
            || request.pointer_id > MAX_SAFE_INTEGER
        {
            return Err("Invalid local pointer".into());
        }
        let position = self
            .normalized_position([request.x, request.y], self.pan.is_some())
            .ok_or("local pointer is outside the fitted sketch plane")?;
        let before = self.editor.selection_presentation_state();
        let hover_before = self.editor.hover_state();
        if matches!(request.phase, PointerPhase::Down) && request.buttons == MIDDLE_POINTER_BUTTON {
            self.editor.cancel();
            self.pan = Some(CanvasPanGesture {
                pointer_id: request.pointer_id,
                origin: position,
                origin_center: self.camera.model_center(),
            });
            self.dimensions.context.navigation_active = true;
            self.dimensions.context.hovered = None;
            self.dimension_hover = None;
        } else if let Some(pan) = self.pan {
            if pan.pointer_id != request.pointer_id {
                return Err("Local pan pointer changed".into());
            }
            self.camera
                .pan_from(pan.origin_center, pan.origin, position);
            if matches!(request.phase, PointerPhase::Up) {
                self.pan = None;
                self.dimensions.context.navigation_active = false;
            }
        } else {
            let input = PointerInput {
                pointer_id: request.pointer_id,
                position,
                modifiers: Modifiers {
                    shift: request.modifiers.shift,
                    control: request.modifiers.ctrl,
                    command: request.modifiers.meta,
                },
            };
            match request.phase {
                PointerPhase::Down => {
                    self.dimensions.context.navigation_active = false;
                    self.editor.select_at(&self.scene, input);
                    self.dimensions.state.focus = None;
                }
                PointerPhase::Move => {
                    if request.buttons == 0 {
                        self.editor.pointer_move(&self.scene, input);
                    }
                }
                PointerPhase::Up => {}
            }
        }
        if matches!(request.phase, PointerPhase::Move)
            && request.buttons == 0
            && self.pan.is_none()
            && hover_before == self.editor.hover_state()
            && before == self.editor.selection_presentation_state()
        {
            return Ok("null".into());
        }
        self.compose(before != self.editor.selection_presentation_state())
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one bounded presentation command vocabulary with shared scene semantics"
    )]
    pub(crate) fn dispatch_json(&mut self, encoded: &str) -> Result<String, String> {
        let request: CommandRequest = decode_request(encoded)?;
        require_version(request.version)?;
        let before = self.editor.selection_presentation_state();
        let previous_context = self.dimensions.context.clone();
        let transient = request.command.starts_with("dimensions.hover")
            || request.command.starts_with("dimensions.navigation.");
        match request.command.as_str() {
            "view.fit" => {
                self.reset_navigation();
                self.camera.fit_scene(&self.scene);
                self.dimensions.state.reconsider_hidden_dimensions();
            }
            "view.origin" => {
                self.reset_navigation();
                self.camera.center_origin();
            }
            "view.grid.toggle" => {
                self.grid_visible = !self.grid_visible;
            }
            "selection.clear" => {
                self.editor.set_selection([]);
                self.dimensions.state.focus = None;
            }
            "dimensions.mode" => {
                self.dimensions.state.mode = mode(
                    request
                        .payload
                        .get("mode")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("Missing dimension mode")?,
                )?;
                self.dimensions.context.navigation_active = false;
                self.dimensions.state.focus = None;
                self.dimensions.context.hovered = None;
                self.dimension_hover = None;
            }
            "dimensions.focus" => {
                let id = request
                    .payload
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("Missing dimension ID")?;
                self.dimensions.state.focus =
                    Some(*self.dimensions.ids.get(id).ok_or("Stale dimension ID")?);
                self.dimensions.context.navigation_active = false;
                if self.dimensions.state.mode == DimensionDisplayMode::Hidden {
                    self.dimensions.state.mode = DimensionDisplayMode::Focused;
                }
            }
            "dimensions.clearPins" => {
                self.dimensions.state.pins.clear();
                self.dimensions.context.navigation_active = false;
            }
            "dimensions.pin" => {
                let id = request
                    .payload
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("Missing dimension ID")?;
                let key = *self.dimensions.ids.get(id).ok_or("Stale dimension ID")?;
                let pinned = request
                    .payload
                    .get("pinned")
                    .and_then(serde_json::Value::as_bool)
                    .ok_or("Missing pin state")?;
                if !pinned {
                    self.dimensions.state.pins.retain(|p| *p != key);
                } else if !self.dimensions.state.pins.contains(&key) {
                    if self.dimensions.state.pins.len() >= DimensionPresentationState::MAX_PINS {
                        return Err("At most four measurements can be pinned".into());
                    }
                    self.dimensions.state.pins.push(key);
                }
                self.dimensions.context.navigation_active = false;
            }
            "dimensions.hover" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Hover {
                    x: f64,
                    y: f64,
                }
                let payload: Hover =
                    serde_json::from_value(request.payload).map_err(|e| e.to_string())?;
                let position = self
                    .normalized_position([payload.x, payload.y], false)
                    .ok_or("dimension hover must lie inside the finite canvas bounds")?;
                self.dimension_hover =
                    if self.pan.is_some() || self.dimensions.context.navigation_active {
                        None
                    } else {
                        geosolve_constraint_editor::dimension_hover_target(
                            &self.scene,
                            &self.editor,
                            self.dimension_hover,
                            position,
                        )
                    };
                self.dimensions.context.hovered = self.dimension_hover.map(|(item, _)| item);
            }
            "dimensions.hover.clear" => {
                self.dimensions.context.hovered = None;
                self.dimension_hover = None;
            }
            "dimensions.navigation.begin" => {
                self.dimensions.context.navigation_active = true;
                self.dimensions.context.hovered = None;
                self.dimension_hover = None;
            }
            "dimensions.navigation.end" => {
                self.dimensions.context.navigation_active = false;
            }
            _ => return Err("Command requires the authoritative workbench".into()),
        }
        if transient && previous_context == self.dimensions.context {
            return Ok("null".into());
        }
        self.compose(before != self.editor.selection_presentation_state())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> (WorkbenchBridge, LocalInteraction) {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":2}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":2,"command":"sample.open","payload":{"key":"pc-water-manifold"}}"#,
            )
            .unwrap();
        let result: serde_json::Value =
            serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
        let local = LocalInteraction::new(&result["seed"].to_string()).unwrap();
        (bridge, local)
    }
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one cross-namespace browsing trace checks exact chrome and unchanged native authority together"
    )]
    fn browsing_namespace_updates_preserve_accepted_materialization_and_local_chrome() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("native-browsing".into()),
            compiled,
        )
        .unwrap()
        .to_canonical_json()
        .unwrap();
        let mut source = WorkbenchBridge::restore(&project).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&source.interaction_snapshot_json().unwrap()).unwrap();
        let design: serde_json::Value =
            serde_json::from_str(&source.workspace_design_json().unwrap()).unwrap();
        let mut browsing = BrowsingPresentation::new(
            &serde_json::json!({"project":project,"design":design,"seed":pair["seed"]}).to_string(),
        )
        .unwrap();
        let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        assert_ne!(
            local.scene.presentation_document().id(),
            browsing
                .bridge
                .retained_scene
                .as_ref()
                .unwrap()
                .presentation_document()
                .id()
        );
        let accepted = std::ptr::from_ref(
            browsing
                .bridge
                .editor()
                .coordinator()
                .accepted_materialization()
                .unwrap(),
        );
        let identity = browsing.bridge.editor().coordinator().intent().identity();
        let code_identity = browsing
            .bridge
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity()
            .clone();
        let before_project = browsing.bridge.export_project_json().unwrap();
        let before_design = browsing.bridge.workspace_design_json().unwrap();
        let point = local.scene.points[0].screen_position;
        local.pointer_json(&serde_json::json!({"version":2,"phase":"down","pointerId":1,"x":point.x,"y":point.y,"buttons":1,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}).to_string()).unwrap();
        for index in 0..4 {
            if index == 1 {
                let dimension = local.dimensions.ids.keys().next().unwrap().clone();
                local.dispatch_json(&serde_json::json!({"version":2,"command":"dimensions.pin","payload":{"id":dimension,"pinned":true}}).to_string()).unwrap();
                local.dispatch_json(&serde_json::json!({"version":2,"command":"dimensions.focus","payload":{"id":dimension}}).to_string()).unwrap();
            }
            if index == 2 {
                let midpoint = local.camera.viewport().model_to_screen([5.0, 0.0]);
                local.pointer_json(&serde_json::json!({"version":2,"phase":"down","pointerId":1,"x":midpoint.x,"y":midpoint.y,"buttons":1,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}).to_string()).unwrap();
                assert_eq!(
                    local.state().curve_picks.len(),
                    1,
                    "{:?}",
                    local.state().selection
                );
            }
            local
                .wheel_json(r#"{"version":2,"x":300,"y":250,"deltaX":0,"deltaY":-30,"ctrl":false}"#)
                .unwrap();
            let state = local.state_json().unwrap();
            let expected: serde_json::Value =
                serde_json::from_str(&source.interaction_apply_json(&state).unwrap()).unwrap();
            let actual: serde_json::Value =
                serde_json::from_str(&browsing.update_json(&state).unwrap()).unwrap();
            assert_eq!(
                actual["selection"]["source"],
                expected["selection"]["source"]
            );
            assert_eq!(actual["selection"]["label"], expected["selection"]["label"]);
            assert_eq!(
                actual["navigation"]["sources"],
                expected["navigation"]["sources"]
            );
            assert_eq!(actual["navigation"]["rows"], expected["navigation"]["rows"]);
            assert_eq!(actual["explorer"], expected["explorer"]);
            let dimensions = |value: &serde_json::Value| {
                value["dimensions"]["allMeasurements"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        (
                            row["id"].clone(),
                            row["pinned"].clone(),
                            row["focused"].clone(),
                            row["value"].clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(dimensions(&actual), dimensions(&expected));
            if index >= 2 {
                let expected_pick = local.state().curve_picks[0];
                let native_picks = browsing
                    .bridge
                    .editor()
                    .editor()
                    .selection_presentation_state()
                    .curve_picks;
                assert_eq!(native_picks.len(), 1);
                assert_eq!(
                    native_picks[0].parameter.to_bits(),
                    expected_pick.parameter.to_bits()
                );
                assert_ne!(native_picks[0].span, expected_pick.span);
            }
            assert_eq!(
                std::ptr::from_ref(
                    browsing
                        .bridge
                        .editor()
                        .coordinator()
                        .accepted_materialization()
                        .unwrap()
                ),
                accepted
            );
            assert_eq!(
                browsing.bridge.editor().coordinator().intent().identity(),
                identity
            );
            assert_eq!(
                browsing
                    .bridge
                    .code_project
                    .as_ref()
                    .unwrap()
                    .code_session_identity(),
                &code_identity
            );
            assert_eq!(
                browsing.bridge.export_project_json().unwrap(),
                before_project
            );
            assert_eq!(
                browsing.bridge.workspace_design_json().unwrap(),
                before_design
            );
        }
        let before = browsing.bridge.editor().editor().selection().to_vec();
        let mut stale: serde_json::Value =
            serde_json::from_str(&local.state_json().unwrap()).unwrap();
        stale["sceneKey"] = "obsolete".into();
        assert!(browsing.update_json(&stale.to_string()).is_err());
        assert_eq!(browsing.bridge.editor().editor().selection(), before);
    }

    #[test]
    fn local_view_refresh_retains_explicit_empty_explorer_selection() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("empty-navigation".into()),
            compiled,
        )
        .unwrap()
        .to_canonical_json()
        .unwrap();
        let mut source = WorkbenchBridge::restore(&project).unwrap();
        source
            .set_explorer_row_visible("managed:bar", false)
            .unwrap();
        let hidden = serde_json::to_value(source.navigation_snapshot()).unwrap();
        source.select_navigation_rows_json(serde_json::json!({"authority":hidden["authority"],"ids":["managed:bar"],"mode":"replace"})).unwrap();
        assert!(source.editor().editor().selection().is_empty());
        let owner = source.editor().selected_declaration();
        assert!(owner.is_some());
        let pair: serde_json::Value =
            serde_json::from_str(&source.interaction_snapshot_json().unwrap()).unwrap();
        let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let before = source.export_project_json().unwrap();
        local
            .wheel_json(r#"{"version":2,"x":300,"y":250,"deltaX":0,"deltaY":-30,"ctrl":false}"#)
            .unwrap();
        let after: serde_json::Value = serde_json::from_str(
            &source
                .interaction_apply_json(&local.state_json().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(source.editor().selected_declaration(), owner);
        assert_eq!(
            after["navigation"]["rows"],
            pair["snapshot"]["navigation"]["rows"]
        );
        assert_eq!(source.export_project_json().unwrap(), before);
    }

    #[test]
    fn browsing_maps_exact_implicit_fillet_occurrence_and_rejects_forged_pick() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-computed.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("browsing-fillet".into()),
            compiled,
        )
        .unwrap()
        .to_canonical_json()
        .unwrap();
        let mut source = WorkbenchBridge::restore(&project).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&source.interaction_snapshot_json().unwrap()).unwrap();
        let design: serde_json::Value =
            serde_json::from_str(&source.workspace_design_json().unwrap()).unwrap();
        let mut browsing = BrowsingPresentation::new(
            &serde_json::json!({"project":project,"design":design,"seed":pair["seed"]}).to_string(),
        )
        .unwrap();
        let local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let discarded = local
            .scene
            .curves
            .iter()
            .find(|curve| curve.origin.is_implicit_construction())
            .unwrap();
        let mut state = local.state();
        let pick = CurvePickContext {
            span: discarded.span,
            parameter: (discarded.screen_parameters.first().unwrap()
                + discarded.screen_parameters.last().unwrap())
                * 0.5,
            origin: discarded.origin,
        };
        state.selection = vec![SelectionItem::Curve(pick.span)];
        state.curve_picks = vec![pick];
        let original = browsing.bridge.export_project_json().unwrap();
        let actual: serde_json::Value = serde_json::from_str(
            &browsing
                .update_json(&serde_json::to_string(&state).unwrap())
                .unwrap(),
        )
        .unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            &source
                .interaction_apply_json(&serde_json::to_string(&state).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(actual["navigation"]["rows"], expected["navigation"]["rows"]);
        assert_eq!(
            actual["navigation"]["sources"],
            expected["navigation"]["sources"]
        );
        let native = browsing
            .bridge
            .editor()
            .editor()
            .selection_presentation_state();
        assert_eq!(native.curve_picks.len(), 1);
        assert_eq!(
            native.curve_picks[0].parameter.to_bits(),
            pick.parameter.to_bits()
        );
        assert_ne!(native.curve_picks[0].span, pick.span);
        assert_ne!(native.curve_picks[0].origin, pick.origin);
        assert!(native.curve_picks[0].origin.is_implicit_construction());
        native
            .validate(browsing.bridge.retained_scene.as_ref().unwrap())
            .unwrap();
        state.curve_picks[0].parameter = -1.0;
        assert!(
            browsing
                .update_json(&serde_json::to_string(&state).unwrap())
                .is_err()
        );
        assert_eq!(
            browsing
                .bridge
                .editor()
                .editor()
                .selection_presentation_state(),
            native
        );
        assert_eq!(browsing.bridge.export_project_json().unwrap(), original);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one source fixture checks readonly native edits and reverse selection without accepted mutation"
    )]
    fn browsing_describes_source_edits_and_round_trips_navigation_without_publication() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("browsing-edits".into()),
            compiled,
        )
        .unwrap()
        .to_canonical_json()
        .unwrap();
        let mut source = WorkbenchBridge::restore(&project).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&source.interaction_snapshot_json().unwrap()).unwrap();
        let design: serde_json::Value =
            serde_json::from_str(&source.workspace_design_json().unwrap()).unwrap();
        let mut browsing = BrowsingPresentation::new(
            &serde_json::json!({"project":project,"design":design,"seed":pair["seed"]}).to_string(),
        )
        .unwrap();
        let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let before = (
            browsing.bridge.export_project_json().unwrap(),
            browsing.bridge.workspace_design_json().unwrap(),
        );
        let accepted = std::ptr::from_ref(
            browsing
                .bridge
                .editor()
                .coordinator()
                .accepted_materialization()
                .unwrap(),
        );
        let state = local.state();
        let chrome: serde_json::Value = serde_json::from_str(
            &browsing
                .update_json(&serde_json::to_string(&state).unwrap())
                .unwrap(),
        )
        .unwrap();
        let authority = chrome["authoringDocument"]["authority"].as_str().unwrap();
        let describe = |command: &str, payload: serde_json::Value| {
            serde_json::json!({"state":state,"authority":authority,"command":command,"payload":payload}).to_string()
        };
        let control = chrome["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["editable"] == true)
            .unwrap();
        let edit = describe(
            "parameter.edit",
            serde_json::json!({"id":control["id"],"value":"24"}),
        );
        let mutation: ManagedSketchMutation =
            serde_json::from_str(&browsing.describe_json(&edit).unwrap()).unwrap();
        let ManagedSketchMutation::SetValues { values } = &mutation else {
            panic!("exact source value edit")
        };
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].declaration, "length");
        assert_eq!(
            values[0].path,
            vec![geosolve_sketch_code::ManagedPathSegment::Field(
                "value".into()
            )]
        );
        let ManagedValue::Unit(value) = &values[0].value else {
            panic!("retained unit")
        };
        assert_eq!(value.value.to_bits(), 24.0_f64.to_bits());
        let ManagedValue::Unit(value) = &values[0].expected else {
            panic!("expected source unit")
        };
        assert_eq!(value.value.to_bits(), 20.0_f64.to_bits());
        let dimension = chrome["dimensions"]["allMeasurements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["editable"] == true)
            .unwrap();
        let dimension_edit = describe(
            "dimensions.edit",
            serde_json::json!({"id":dimension["id"],"value":"24"}),
        );
        assert_eq!(
            serde_json::from_str::<ManagedSketchMutation>(
                &browsing.describe_json(&dimension_edit).unwrap()
            )
            .unwrap(),
            mutation
        );
        let metadata_edit = describe(
            "authoring.metadata.set",
            serde_json::json!({"authority":authority,"target":{"kind":"dimension","id":"length"},"changes":{"isKeyConstraint":true}}),
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &browsing.describe_json(&metadata_edit).unwrap()
            )
            .unwrap(),
            serde_json::json!({"mutation":"set_metadata","target":{"target":"declaration","declaration":"length"},"property":"isKeyConstraint","value":{"kind":"bool","value":true}})
        );
        let move_edit = describe(
            "declaration.move",
            serde_json::json!({"id":"managed:horizontal","direction":"up"}),
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&browsing.describe_json(&move_edit).unwrap())
                .unwrap(),
            serde_json::json!({"mutation":"reorder_declaration","declaration":"horizontal","before":"bar"})
        );
        let delete_edit = describe(
            "declaration.delete",
            serde_json::json!({"id":"managed:horizontal"}),
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &browsing.describe_json(&delete_edit).unwrap()
            )
            .unwrap(),
            serde_json::json!({"mutation":"delete","target":{"target":"declaration","declaration":"horizontal"}})
        );
        let mut obsolete: serde_json::Value = serde_json::from_str(&edit).unwrap();
        obsolete["authority"] = pair["snapshot"]["authoringDocument"]["authority"].clone();
        assert!(browsing.describe_json(&obsolete.to_string()).is_err());
        assert!(
            browsing
                .describe_json(&describe("workspace.project.apply", serde_json::json!({})))
                .is_err()
        );
        assert_eq!(
            (
                browsing.bridge.export_project_json().unwrap(),
                browsing.bridge.workspace_design_json().unwrap()
            ),
            before
        );
        assert_eq!(
            std::ptr::from_ref(
                browsing
                    .bridge
                    .editor()
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
            ),
            accepted
        );
        let request = serde_json::json!({"state":state,"command":"navigation.rows.select","payload":{"authority":chrome["navigation"]["authority"],"ids":["managed:bar"],"mode":"replace"}});
        let navigated: serde_json::Value =
            serde_json::from_str(&browsing.navigate_json(&request.to_string()).unwrap()).unwrap();
        assert!(navigated["state"]["selection"].as_array().unwrap().len() >= 3);
        let restore = serde_json::json!({"expected":state,"state":navigated["state"]});
        local.restore_selection_json(&restore.to_string()).unwrap();
        source.select_navigation_rows_json(serde_json::json!({"authority":pair["snapshot"]["navigation"]["authority"],"ids":["managed:bar"],"mode":"replace"})).unwrap();
        assert_eq!(
            local.state().selection,
            source.editor().editor().selection()
        );
        let selected = local.state();
        let mut forged = serde_json::json!({"expected":selected,"state":selected});
        forged["state"]["gridVisible"] = (!selected.grid_visible).into();
        assert!(local.restore_selection_json(&forged.to_string()).is_err());
        local
            .wheel_json(r#"{"version":2,"x":300,"y":250,"deltaX":0,"deltaY":-30,"ctrl":false}"#)
            .unwrap();
        let current = local.state_json().unwrap();
        assert!(local.restore_selection_json(&restore.to_string()).is_err());
        assert_eq!(local.state_json().unwrap(), current);
    }

    #[test]
    fn authoring_prediction_paint_uses_current_view_and_never_picking_authority() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":2}"#).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
        let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let source = bridge.export_project_json().unwrap();
        local
            .wheel_json(
                r#"{"version":2,"x":300.0,"y":250.0,"deltaX":0.0,"deltaY":-70.0,"ctrl":false}"#,
            )
            .unwrap();
        let viewport = local.camera.viewport();
        let view = serde_json::json!({"seed":pair["seed"],"state":local.state()});
        let input = serde_json::json!({"scene":pair["seed"]["scene"],"view":view,
            "construction":{"preview":{"kind":"circle","center":[12.0,7.0],"radius":3.0},
            "inference_guides":[{"kind":"point","position":[12.0,7.0]}]}});
        let frame: serde_json::Value =
            serde_json::from_str(&authoring_preview_json(&input.to_string()).unwrap()).unwrap();
        assert_eq!(frame["scene"]["provenance"]["scene"], "provisional");
        let items = frame["scene"]["items"].as_array().unwrap();
        assert!(items.iter().all(|item| item["interactive"] == false));
        let circle = items
            .iter()
            .find(|item| item["className"] == "wb-draft-circle")
            .unwrap();
        let expected = viewport.model_to_screen([12.0, 7.0]);
        assert_eq!(
            circle["center"],
            serde_json::json!([expected.x, expected.y])
        );
        assert_eq!(
            circle["radius"],
            serde_json::json!(3.0 * viewport.pixels_per_model_unit)
        );
        assert_eq!(bridge.export_project_json().unwrap(), source);
        let mut stale = input.clone();
        stale["view"]["state"]["sceneKey"] = "obsolete".into();
        assert!(authoring_preview_json(&stale.to_string()).is_err());
        let mut invalid = input;
        invalid["construction"]["preview"]["radius"] = (-1.0).into();
        assert!(authoring_preview_json(&invalid.to_string()).is_err());
        assert_eq!(bridge.export_project_json().unwrap(), source);
    }

    #[test]
    fn local_canvas_replays_camera_and_selection_without_authoring() {
        let (mut bridge, mut local) = seeded();
        let project = bridge.export_project_json().unwrap();
        let request = r#"{"version":2,"samples":[{"version":2,"x":400.0,"y":320.0,"deltaX":0.0,"deltaY":-80.0,"ctrl":false},{"version":2,"x":350.0,"y":280.0,"deltaX":3.0,"deltaY":40.0,"ctrl":true}]}"#;
        bridge.wheel_json(request).unwrap();
        local.wheel_json(request).unwrap();
        assert_eq!(bridge.camera, local.camera);
        let point = local.scene.points.first().unwrap().screen_position;
        for (phase, buttons) in [("down", 1), ("up", 0)] {
            let request=serde_json::json!({"version":2,"phase":phase,"pointerId":1,"x":point.x,"y":point.y,"buttons":buttons,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}).to_string();
            local.pointer_json(&request).unwrap();
        }
        assert!(!local.editor.selection().is_empty());
        bridge
            .interaction_apply_json(&local.state_json().unwrap())
            .unwrap();
        assert_eq!(
            bridge.editor().editor().selection(),
            local.editor.selection()
        );
        assert_eq!(bridge.export_project_json().unwrap(), project);
        assert!(local.scene.is_detached_presentation());
    }
    #[test]
    fn local_canvas_refuses_stale_or_forged_selection_transactionally() {
        let (mut bridge, local) = seeded();
        let camera = bridge.camera;
        let selection = bridge.editor().editor().selection().to_vec();
        let mut invalid = serde_json::to_value(local.state()).unwrap();
        invalid["selection"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!("Point(forged)"));
        assert!(bridge.interaction_apply_json(&invalid.to_string()).is_err());
        assert_eq!(bridge.camera, camera);
        assert_eq!(bridge.editor().editor().selection(), selection);
        let mut state = local.state();
        state.scene_key.push('x');
        assert!(
            bridge
                .interaction_apply_json(&serde_json::to_string(&state).unwrap())
                .is_err()
        );
        assert_eq!(bridge.camera, camera);
    }
    #[test]
    fn local_canvas_replacement_retains_latest_camera_and_finite_annotations() {
        let (mut bridge, mut local) = seeded();
        local
            .wheel_json(
                r#"{"version":2,"x":300.0,"y":250.0,"deltaX":0.0,"deltaY":-50.0,"ctrl":false}"#,
            )
            .unwrap();
        let camera = local.camera;
        let seed: serde_json::Value =
            serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
        let update = local
            .replace_json(
                &serde_json::json!({"seed":seed["seed"],"preserveSelection":true}).to_string(),
            )
            .unwrap();
        assert_eq!(local.camera, camera);
        let update: serde_json::Value = serde_json::from_str(&update).unwrap();
        assert_eq!(
            update["frame"]["scene"]["provenance"]["scene"],
            "accepted-presentation"
        );
        assert!(
            update["frame"]["scene"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["layer"] == "annotations")
        );
        assert!(
            local
                .dispatch_json(
                    r#"{"version":2,"command":"parameter.edit","payload":{"id":"x","value":"4"}}"#
                )
                .is_err()
        );
    }
    #[test]
    fn local_canvas_exact_pan_resize_hover_and_dimension_preferences() {
        let (mut bridge, mut local) = seeded();
        let resize = r#"{"version":2,"width":920.0,"height":670.0,"pixelRatio":2.0}"#;
        bridge.resize_json(resize).unwrap();
        local.resize_json(resize).unwrap();
        assert_eq!(
            bridge.camera, local.camera,
            "first browser size uses native fit"
        );
        for (phase, buttons, x, y) in [
            ("down", 4, 100.0, 150.0),
            ("move", 4, 142.0, 173.0),
            ("up", 0, 149.0, 181.0),
        ] {
            let request=serde_json::json!({"version":2,"phase":phase,"pointerId":2,"x":x,"y":y,"buttons":buttons,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}).to_string();
            bridge.pointer_json(&request).unwrap();
            local.pointer_json(&request).unwrap();
            assert_eq!(bridge.camera, local.camera, "{phase}");
        }
        let point = local.scene.points[0].screen_position;
        let hover=serde_json::json!({"version":2,"phase":"move","pointerId":2,"x":point.x,"y":point.y,"buttons":0,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}).to_string();
        bridge.pointer_json(&hover).unwrap();
        local.pointer_json(&hover).unwrap();
        assert_eq!(
            bridge.editor().editor().hover_state(),
            local.editor.hover_state()
        );
        assert_eq!(
            local.pointer_json(&hover).unwrap(),
            "null",
            "unchanged hover avoids composition and transport"
        );
        let wheel = r#"{"version":2,"x":300.0,"y":250.0,"deltaX":0.0,"deltaY":-50.0,"ctrl":false}"#;
        local.wheel_json(wheel).unwrap();
        let dimension = local.dimensions.ids.keys().next().unwrap().clone();
        for command in [
            serde_json::json!({"version":2,"command":"dimensions.mode","payload":{"mode":"hidden"}}),
            serde_json::json!({"version":2,"command":"dimensions.focus","payload":{"id":dimension}}),
            serde_json::json!({"version":2,"command":"dimensions.pin","payload":{"id":dimension,"pinned":true}}),
        ] {
            local.dispatch_json(&command.to_string()).unwrap();
        }
        assert!(!local.dimensions.context.navigation_active);
        assert_eq!(local.state().dimension_mode, "focused");
        assert_eq!(local.state().dimension_pins, vec![dimension.clone()]);
        assert_eq!(local.state().dimension_focus, Some(dimension));
        bridge
            .interaction_apply_json(&local.state_json().unwrap())
            .unwrap();
        assert_eq!(
            bridge.local_dimension_seed().state.mode,
            local.dimensions.state.mode
        );
        assert_eq!(
            bridge.local_dimension_seed().state.focus,
            local.dimensions.state.focus
        );
        assert_eq!(
            bridge.local_dimension_seed().state.pins,
            local.dimensions.state.pins
        );
    }
    #[test]
    fn local_canvas_invalid_late_wheel_and_resize_preserve_view() {
        let (_, mut local) = seeded();
        let state = local.state_json().unwrap();
        let malformed = r#"{"version":2,"samples":[{"version":2,"x":20.0,"y":20.0,"deltaX":0.0,"deltaY":-50.0,"ctrl":false},{"version":2,"x":-1.0,"y":20.0,"deltaX":0.0,"deltaY":-50.0,"ctrl":false}]}"#;
        assert!(local.wheel_json(malformed).is_err());
        assert_eq!(local.state_json().unwrap(), state);
        assert!(
            local
                .resize_json(r#"{"version":2,"width":-1.0,"height":670.0,"pixelRatio":2.0}"#)
                .is_err()
        );
        assert_eq!(local.state_json().unwrap(), state);
    }
    #[test]
    fn local_canvas_preserves_explicit_curve_pick_when_server_arms_constraint() {
        let (mut bridge, mut local) = seeded();
        let positions = local
            .scene
            .curves
            .iter()
            .filter_map(|curve| {
                curve
                    .screen_polyline
                    .get(curve.screen_polyline.len() / 2)
                    .copied()
            })
            .collect::<Vec<_>>();
        for point in positions {
            let request=serde_json::json!({"version":2,"phase":"down","pointerId":1,"x":point.x,"y":point.y,"buttons":1,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}).to_string();
            if local.pointer_json(&request).is_ok() && !local.state().curve_picks.is_empty() {
                break;
            }
        }
        let selected = local.editor.selection_presentation_state();
        assert!(
            !selected.curve_picks.is_empty(),
            "fixture exercises exact curve occurrence selection"
        );
        let source = bridge.export_project_json().unwrap();
        bridge
            .interaction_apply_json(&local.state_json().unwrap())
            .unwrap();
        assert_eq!(
            bridge.editor().editor().selection_presentation_state(),
            selected
        );
        assert_eq!(
            bridge.curve_parameter(SelectionItem::Curve(selected.curve_picks[0].span)),
            Some(selected.curve_picks[0].parameter)
        );
        assert_eq!(bridge.export_project_json().unwrap(), source);
        let mut forged = local.state();
        forged.curve_picks[0].parameter = 1.0e200;
        assert!(
            bridge
                .interaction_apply_json(&serde_json::to_string(&forged).unwrap())
                .is_err()
        );
        assert_eq!(
            bridge.editor().editor().selection_presentation_state(),
            selected
        );
    }
    #[test]
    fn local_canvas_server_preview_requires_the_exact_current_view() {
        let (mut bridge, mut local) = seeded();
        bridge.active_tool = "line".into();
        let result: serde_json::Value =
            serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
        let replacement =
            serde_json::json!({"seed":result["seed"],"preserveSelection":true}).to_string();
        let update: serde_json::Value =
            serde_json::from_str(&local.replace_json(&replacement).unwrap()).unwrap();
        assert_eq!(update["serverFrameCompatible"], true);
        local
            .wheel_json(
                r#"{"version":2,"x":300.0,"y":250.0,"deltaX":0.0,"deltaY":-50.0,"ctrl":false}"#,
            )
            .unwrap();
        let camera = local.camera;
        let update: serde_json::Value =
            serde_json::from_str(&local.replace_json(&replacement).unwrap()).unwrap();
        assert_eq!(update["serverFrameCompatible"], false);
        assert_eq!(local.camera, camera);
    }
    #[test]
    fn local_canvas_navigation_handoff_preserves_armed_tool_and_uses_new_camera() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":2}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":2,"command":"project.new"}"#)
            .unwrap();
        let before = bridge.export_project_json().unwrap();
        bridge
            .dispatch_json(r#"{"version":2,"command":"tool.select","payload":{"id":"segment"}}"#)
            .unwrap();
        bridge
            .cancel_json(r#"{"version":2,"reason":"lost-capture"}"#)
            .unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
        let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let wheel = r#"{"version":2,"x":300.0,"y":250.0,"deltaX":0.0,"deltaY":-80.0,"ctrl":false}"#;
        local.wheel_json(wheel).unwrap();
        assert_ne!(bridge.camera, local.camera);
        bridge
            .interaction_apply_json(&local.state_json().unwrap())
            .unwrap();
        assert_eq!(bridge.active_tool, "segment");
        assert_eq!(bridge.camera, local.camera);
        assert_eq!(bridge.export_project_json().unwrap(), before);
        let pointer = |phase, buttons, position: [f64; 2]| {
            serde_json::json!({
            "version":2,"phase":phase,"pointerId":816,"x":position[0],"y":position[1],"buttons":buttons,
            "modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}
        }).to_string()
        };
        bridge
            .pointer_json(&pointer("down", 1, [245.3, 236.2]))
            .unwrap();
        bridge
            .pointer_json(&pointer("up", 0, [245.3, 236.2]))
            .unwrap();
        assert_eq!(
            bridge
                .editor()
                .editor()
                .geometry_draft_status()
                .unwrap()
                .completed_stages,
            1
        );
        let staged = bridge.editor().editor().geometry_draft_status();
        assert!(
            bridge
                .interaction_apply_json(&local.state_json().unwrap())
                .is_err()
        );
        assert_eq!(bridge.editor().editor().geometry_draft_status(), staged);
        bridge
            .cancel_json(r#"{"version":2,"reason":"lost-capture"}"#)
            .unwrap();
        local.wheel_json(wheel).unwrap();
        bridge
            .interaction_apply_json(&local.state_json().unwrap())
            .unwrap();
        assert_eq!(bridge.active_tool, "segment");
        assert_eq!(
            bridge
                .editor()
                .editor()
                .geometry_draft_status()
                .unwrap()
                .completed_stages,
            0
        );
        let positions = [[333.1, 232.6], [642.3, 423.4]];
        let expected = positions.map(|[x, y]| {
            local
                .camera
                .viewport()
                .screen_to_model(ScreenPoint { x, y })
        });
        for position in positions {
            bridge.pointer_json(&pointer("down", 1, position)).unwrap();
            bridge.pointer_json(&pointer("up", 0, position)).unwrap();
        }
        let document = bridge
            .editor()
            .coordinator()
            .presentation_session()
            .unwrap()
            .design_document();
        assert_eq!(document.curves().len(), 1);
        assert_eq!(document.points().len(), 2);
        for (point, expected) in document.points().iter().zip(expected) {
            assert!((point.position[0] - expected[0]).abs() < 1.0e-10);
            assert!((point.position[1] - expected[1]).abs() < 1.0e-10);
        }
        assert_eq!(bridge.camera, local.camera);
        assert_eq!(bridge.active_tool, "segment");
        assert!(bridge.last_error.is_none());
    }
}
