// SPDX-License-Identifier: GPL-3.0-or-later
//! Local presentation over a detached accepted scene. There is no solver or document editor here.

use super::*;
use geosolve_constraint_editor::{
    AnnotationLayoutState, ConstraintEditor, CurvePickContext, DimensionDisplayMode,
    DimensionPresentationContext, DimensionPresentationState, GeometryInteractionPolicy,
    SelectionPresentationState, Viewport,
};
use std::collections::BTreeMap;

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
}

#[derive(Clone, Serialize, Deserialize)]
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
        };
        serde_json::to_string(&serde_json::json!({"snapshot":snapshot,"seed":seed}))
            .map_err(|e| e.to_string())
    }

    pub(crate) fn interaction_apply_json(&mut self, request: &str) -> Result<String, String> {
        let state: InteractionState = decode_request(request)?;
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
        let scene = scene.clone();
        self.editor_mut()
            .restore_selection_presentation(&scene, selection)
            .map_err(|e| e.to_string())?;
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
        self.snapshot_json()
    }
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
}
impl LocalInteraction {
    pub(crate) fn new(encoded: &str) -> Result<Self, String> {
        let seed: InteractionSeed = decode_request(encoded)?;
        if seed.format != FORMAT {
            return Err("Unsupported local interaction format".into());
        }
        let scene = EditorScene::from_detached_json(&seed.scene).map_err(|e| e.to_string())?;
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
    pub(crate) fn state_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.state()).map_err(|e| e.to_string())
    }
    fn compose(&mut self, selection_changed: bool) -> Result<String, String> {
        self.compose_for_replacement(selection_changed, false)
    }
    fn compose_for_replacement(
        &mut self,
        selection_changed: bool,
        server_frame_compatible: bool,
    ) -> Result<String, String> {
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
        let scene = geosolve_sketch_render::compose_draw_frame(
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
        serde_json::to_string(&InteractionUpdate {
            frame: FrameSnapshot {
                scene,
                aria_label: format!("{} accepted sketch viewport", self.title),
            },
            state: self.state(),
            selection_changed,
            server_frame_compatible,
        })
        .map_err(|e| e.to_string())
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
