// SPDX-License-Identifier: GPL-3.0-or-later
//! Detached accepted-scene navigation. No solver, compiler, storage or renderer is owned here.
use crate::{
    ConstraintEditor, CurvePickContext, DetachedDimensionPresentation as LocalDimensionSeed,
    DimensionDisplayMode, DimensionPresentationState, EditorScene, GeometryInteractionPolicy,
    Modifiers, PointerInput, PresentationMapping, ScreenPoint, SelectionItem,
    SelectionPresentationState, Viewport, VisibilitySeed, VisibilityState,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
const FORMAT: &str = "geosolve-local-interaction-v1";
const PROTOCOL_VERSION: u8 = 2;
const MAX_REQUEST_BYTES: usize = 40 * 1024 * 1024;
const MAX_HOST_EXTENT: f64 = 32_768.0;
const MAX_PIXEL_RATIO: f64 = 16.0;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MIDDLE_POINTER_BUTTON: u16 = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetachedInteractionSeed {
    pub format: String,
    pub scene_key: String,
    pub scene: String,
    pub title: String,
    pub host_size_received: bool,
    pub semantic_preview: bool,
    pub selection: Vec<SelectionItem>,
    pub curve_picks: Vec<CurvePickContext>,
    pub grid_visible: bool,
    pub policy: GeometryInteractionPolicy,
    pub dimensions: LocalDimensionSeed,
    #[serde(default)]
    pub visibility_seed: VisibilitySeed,
    #[serde(default)]
    pub visibility: VisibilityState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bindings: Option<crate::ProjectionalPresentationBindings>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub point_targets: BTreeMap<geosolve_sketch::DesignPointId, serde_json::Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub presence_bindings: BTreeMap<String, Vec<crate::IntentNativeBinding>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetachedInteractionState {
    pub format: String,
    pub scene_key: String,
    pub viewport: Viewport,
    pub selection: Vec<SelectionItem>,
    pub curve_picks: Vec<CurvePickContext>,
    pub grid_visible: bool,
    pub dimension_mode: String,
    pub dimension_pins: Vec<String>,
    pub dimension_focus: Option<String>,
    #[serde(default)]
    pub visibility: VisibilityState,
}

pub fn mode_name(mode: DimensionDisplayMode) -> &'static str {
    match mode {
        DimensionDisplayMode::Focused => "focused",
        DimensionDisplayMode::All => "all",
        DimensionDisplayMode::Hidden => "hidden",
    }
}
/// Decodes the stable personal dimension display preference.
///
/// # Errors
/// Rejects unknown display modes.
pub fn mode(value: &str) -> Result<DimensionDisplayMode, String> {
    match value {
        "focused" => Ok(DimensionDisplayMode::Focused),
        "all" => Ok(DimensionDisplayMode::All),
        "hidden" => Ok(DimensionDisplayMode::Hidden),
        _ => Err("Unknown dimension mode".into()),
    }
}
/// Validates a detached viewport and constructs its camera.
///
/// # Errors
/// Rejects nonfinite, nonpositive or out-of-range viewport values.
pub fn camera(viewport: Viewport) -> Result<crate::CanvasCamera, String> {
    if viewport
        .screen_size
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0 || *v > MAX_HOST_EXTENT)
    {
        return Err("Invalid local viewport extent".into());
    }
    let mut camera =
        crate::CanvasCamera::new(viewport.model_center, viewport.pixels_per_model_unit)
            .ok_or("Invalid local camera")?;
    camera.resize(viewport.screen_size);
    Ok(camera)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRequest {
    version: u8,
    command: String,
    #[serde(default)]
    payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct ModifierRequest {
    #[serde(rename = "alt")]
    _alt: bool,
    ctrl: bool,
    meta: bool,
    shift: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PointerPhase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PointerRequest {
    version: u8,
    phase: PointerPhase,
    pointer_id: u64,
    x: f64,
    y: f64,
    buttons: u16,
    modifiers: ModifierRequest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPanGesture {
    pointer_id: u64,
    origin: ScreenPoint,
    origin_center: [f64; 2],
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WheelRequest {
    version: u8,
    x: f64,
    y: f64,
    delta_x: f64,
    delta_y: f64,
    #[serde(rename = "ctrl")]
    _ctrl: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WheelBatchRequest {
    version: u8,
    samples: Vec<WheelRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WheelInputRequest {
    Single(WheelRequest),
    Batch(WheelBatchRequest),
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResizeRequest {
    version: u8,
    width: f64,
    height: f64,
    pixel_ratio: f64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CancelReason {
    Escape,
    LostCapture,
    Blur,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelRequest {
    version: u8,
    #[serde(rename = "reason")]
    _reason: CancelReason,
}

fn decode_request<T: for<'de> Deserialize<'de>>(request: &str) -> Result<T, String> {
    if request.len() > MAX_REQUEST_BYTES {
        return Err(format!(
            "workbench bridge request exceeds {MAX_REQUEST_BYTES} bytes"
        ));
    }
    serde_json::from_str(request).map_err(|error| format!("invalid workbench request: {error}"))
}

fn require_version(version: u8) -> Result<(), String> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err("unsupported workbench bridge protocol version".into())
    }
}

#[derive(Debug)]
pub struct DetachedCanvasState {
    pub scene_key: String,
    pub scene: EditorScene,
    pub full_scene: EditorScene,
    pub visibility_seed: VisibilitySeed,
    pub visibility: VisibilityState,
    pub title: String,
    pub editor: ConstraintEditor,
    pub camera: crate::CanvasCamera,
    pub grid_visible: bool,
    pub dimensions: LocalDimensionSeed,
    pub pan: Option<CanvasPanGesture>,
    pub dimension_hover: Option<(SelectionItem, ScreenPoint)>,
    pub host_size_received: bool,
    pub point_targets: BTreeMap<geosolve_sketch::DesignPointId, serde_json::Value>,
    pub presence_bindings: BTreeMap<String, Vec<crate::IntentNativeBinding>>,
    pub bindings: Option<crate::ProjectionalPresentationBindings>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetachedInteractionUpdate {
    pub state: DetachedInteractionState,
    pub selection_changed: bool,
    pub server_frame_compatible: bool,
}

/// Transient annotation interest for a native frame.
#[derive(Clone, Copy, Debug, Default)]
pub enum DetachedFrameContext {
    /// Preserve the current personal navigation and annotation context.
    #[default]
    Navigation,
    /// Expose the active authored dimension, even when other dimensions are hidden.
    Authoring {
        active_dimension: Option<SelectionItem>,
    },
}

/// Finite detached navigation state. Its read-only view grants no publication authority.
#[derive(Debug)]
pub struct DetachedCanvas {
    state: DetachedCanvasState,
}
impl std::ops::Deref for DetachedCanvas {
    type Target = DetachedCanvasState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DetachedCanvas {
    /// Restores detached presentation with no editing or publication capability.
    ///
    /// # Errors
    /// Rejects malformed seeds, foreign bindings, invalid visibility or selection.
    pub fn new(encoded: &str) -> Result<Self, String> {
        let seed: DetachedInteractionSeed = decode_request(encoded)?;
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
        seed.visibility_seed.validate(&seed.visibility)?;
        let mut editor = ConstraintEditor::default();
        editor.set_geometry_interaction_policy(seed.policy);
        seed.visibility.apply_policy(&mut editor);
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
        let full_scene = scene.clone();
        let mut scene = scene;
        scene
            .hide_items(seed.visibility_seed.hidden_items(&seed.visibility))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            state: DetachedCanvasState {
                scene_key: seed.scene_key,
                scene,
                full_scene,
                visibility_seed: seed.visibility_seed,
                visibility: seed.visibility,
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
                bindings: seed.bindings,
            },
        })
    }
    pub fn state(&self) -> DetachedInteractionState {
        DetachedInteractionState {
            format: FORMAT.into(),
            scene_key: self.state.scene_key.clone(),
            viewport: self.state.camera.viewport(),
            selection: self.state.editor.selection().to_vec(),
            curve_picks: self.state.editor.selection_presentation_state().curve_picks,
            grid_visible: self.state.grid_visible,
            visibility: self.state.visibility.clone(),
            dimension_mode: mode_name(self.state.dimensions.state.mode).into(),
            dimension_focus: self
                .state
                .dimensions
                .ids
                .iter()
                .find(|(_, key)| Some(**key) == self.state.dimensions.state.focus)
                .map(|(id, _)| id.clone()),
            dimension_pins: self
                .state
                .dimensions
                .ids
                .iter()
                .filter(|(_, key)| self.state.dimensions.state.pins.contains(key))
                .map(|(id, _)| id.clone())
                .collect(),
        }
    }
    /// Maps a host pointer to model coordinates and an accepted semantic point target.
    ///
    /// # Errors
    /// Rejects malformed requests and positions outside the permitted capture bounds.
    pub fn authoring_pointer_json(&self, encoded: &str) -> Result<String, String> {
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
        let item = self
            .state
            .editor
            .select_pointer_item(&self.state.scene, position);
        let target = match item {
            Some(SelectionItem::Point(point)) => self.state.point_targets.get(&point),
            _ => None,
        };
        serde_json::to_string(&serde_json::json!({"viewport":self.state.camera.viewport(),
            "position":self.state.camera.viewport().screen_to_model(position),"target":target}))
        .map_err(|e| e.to_string())
    }
    /// Serializes the personal view, independently of accepted source.
    ///
    /// # Errors
    /// Returns a serialization error if the view cannot be encoded.
    pub fn state_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.state()).map_err(|e| e.to_string())
    }
    /// Exports only durable personal visibility/dimension preferences through the
    /// existing native codec. Camera, selection and authoring authority are transient.
    /// # Errors
    /// Rejects invalid personal preferences or serialization failure.
    pub fn export_presentation_json(&self) -> Result<String, String> {
        use crate::presentation_persistence::{
            DimensionPersistence, WorkbenchPresentationPersistence,
        };
        let value = WorkbenchPresentationPersistence {
            hidden_rows: self.state.visibility.hidden_rows.iter().cloned().collect(),
            isolate_restore: self
                .state
                .visibility
                .isolate_restore
                .as_ref()
                .map(|rows| rows.iter().cloned().collect()),
            construction_visible: self.state.visibility.construction_visible,
            dimensions: DimensionPersistence::from_state(&self.state.dimensions.state),
        };
        value.validate()?;
        serde_json::to_string(&value).map_err(|error| error.to_string())
    }
    /// Installs a browsing selection only while its complete expected personal view is current.
    ///
    /// # Errors
    /// Rejects stale views, non-selection changes and invalid native pick occurrences.
    pub fn restore_selection_json(
        &mut self,
        encoded: &str,
    ) -> Result<Option<DetachedInteractionUpdate>, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            expected: DetachedInteractionState,
            state: DetachedInteractionState,
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
        self.state
            .editor
            .restore_selection_presentation(&self.state.scene, selection)
            .map_err(|e| e.to_string())?;
        self.compose(true)
    }
    fn replace_prediction_scene(
        &mut self,
        scene: EditorScene,
        state: &VisibilityState,
        mapping: &PresentationMapping,
    ) -> Result<(), String> {
        mapping.dimensions(&mut self.state.dimensions)?;
        self.state.scene = scene;
        self.state
            .visibility_seed
            .mask_prediction(&mut self.state.scene, state, mapping)?;
        state.apply_policy(&mut self.state.editor);
        Ok(())
    }
    fn apply_visibility(&mut self) -> Result<(), String> {
        self.state
            .visibility_seed
            .validate(&self.state.visibility)?;
        let mut scene = self.state.full_scene.clone();
        scene
            .hide_items(
                self.state
                    .visibility_seed
                    .hidden_items(&self.state.visibility),
            )
            .map_err(|e| e.to_string())?;
        self.state.visibility.apply_policy(&mut self.state.editor);
        self.state.scene = scene;
        self.state.dimension_hover = None;
        Ok(())
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one atomic scene replacement reconciles complete personal presentation before publication"
    )]
    /// Prepares a complete scene replacement while retaining compatible personal preferences.
    ///
    /// # Errors
    /// Rejects malformed scenes or invalid correspondence without changing this canvas.
    pub fn prepare_replacement(
        &self,
        encoded: &str,
    ) -> Result<(Self, DetachedInteractionUpdate), String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Replacement {
            seed: DetachedInteractionSeed,
            preserve_selection: bool,
        }
        let request: Replacement = decode_request(encoded)?;
        let mut next =
            Self::new(&serde_json::to_string(&request.seed).map_err(|e| e.to_string())?)?;
        next.state.camera = self.state.camera;
        next.state.host_size_received = self.state.host_size_received;
        next.state.grid_visible = self.state.grid_visible;
        next.state.visibility = self.state.visibility.clone();
        next.state.visibility.reconcile(&next.state.visibility_seed);
        next.apply_visibility()?;
        let mapping = PresentationMapping::for_replacement(
            &self.state.scene,
            &next.state.scene,
            self.state.bindings.as_ref(),
            next.state.bindings.as_ref(),
        );
        next.state.dimensions.state.mode = self.state.dimensions.state.mode;
        next.state.dimensions.state.pins = self
            .state
            .dimensions
            .state
            .pins
            .iter()
            .filter_map(|pin| mapping.layout_key(*pin).ok())
            .filter(|pin| next.state.dimensions.ids.values().any(|key| key == pin))
            .collect();
        next.state.dimensions.state.focus = self
            .state
            .dimensions
            .state
            .focus
            .and_then(|focus| mapping.layout_key(focus).ok())
            .filter(|focus| next.state.dimensions.ids.values().any(|key| key == focus));
        if self.state.scene_key == next.state.scene_key {
            next.state.dimensions.state = self.state.dimensions.state.clone();
        }
        next.state.pan = self.state.pan;
        next.state.dimension_hover = if self.state.scene_key == next.state.scene_key {
            self.state.dimension_hover
        } else {
            None
        };
        next.state.dimensions.context.navigation_active =
            self.state.dimensions.context.navigation_active;
        if request.preserve_selection && self.state.scene_key == next.state.scene_key {
            next.state.editor = self.state.editor.clone();
            next.state.dimensions.context.hovered = self.state.dimensions.context.hovered;
        } else if request.preserve_selection {
            let previous = self.state.editor.selection_presentation_state();
            let mut selected = SelectionPresentationState {
                items: previous
                    .items
                    .into_iter()
                    .filter_map(|item| mapping.selection(item).ok())
                    .collect(),
                curve_picks: Vec::new(),
            };
            selected.items.retain(|item| {
                SelectionPresentationState {
                    items: vec![*item],
                    curve_picks: vec![],
                }
                .validate(&next.state.scene)
                .is_ok()
            });
            for pick in previous.curve_picks {
                let Ok(item) = mapping.selection(SelectionItem::Curve(pick.span)) else {
                    continue;
                };
                let mapped = mapping
                    .curve_pick(pick, &next.state.scene)
                    .ok()
                    .filter(|pick| {
                        selected.items.contains(&item)
                            && SelectionPresentationState {
                                items: vec![item],
                                curve_picks: vec![*pick],
                            }
                            .validate(&next.state.scene)
                            .is_ok()
                    });
                if let Some(pick) = mapped {
                    selected.curve_picks.push(pick);
                } else {
                    selected.items.retain(|selected| *selected != item);
                }
            }
            next.state
                .editor
                .restore_selection_presentation(&next.state.scene, selected)
                .map_err(|e| e.to_string())?;
        }
        next.state
            .editor
            .set_geometry_interaction_policy(request.seed.policy);
        next.state.visibility.apply_policy(&mut next.state.editor);
        // Exact server draft/inference paint may survive an unchanged local view.
        // Navigation never paints a preview in an obsolete camera coordinate space.
        let server_frame_compatible = request.seed.semantic_preview
            && next.state.visibility == request.seed.visibility
            && next.state.scene.viewport == next.state.camera.viewport()
            && next.state().selection == request.seed.selection
            && next.state().curve_picks == request.seed.curve_picks;
        next.prepare_frame(DetachedFrameContext::Navigation)?;
        let result = DetachedInteractionUpdate {
            state: next.state(),
            selection_changed: true,
            server_frame_compatible,
        };
        Ok((next, result))
    }
    /// Applies a validated ordered wheel batch as one camera update.
    ///
    /// # Errors
    /// Rejects malformed or out-of-bounds samples before installing the camera.
    pub fn wheel_json(
        &mut self,
        encoded: &str,
    ) -> Result<Option<DetachedInteractionUpdate>, String> {
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
        let mut next = self.state.camera;
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
        self.state.editor.cancel();
        self.state.pan = None;
        self.state.camera = next;
        self.state.dimensions.context.navigation_active = true;
        self.state.dimensions.context.hovered = None;
        self.state.dimension_hover = None;
        self.compose(false)
    }
    /// Resizes the finite camera and fits only the first host extent.
    ///
    /// # Errors
    /// Rejects invalid host extents, pixel ratios or scene reprojection.
    pub fn resize_json(
        &mut self,
        encoded: &str,
    ) -> Result<Option<DetachedInteractionUpdate>, String> {
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
            ..self.state.camera.viewport()
        })?;
        if next == self.state.camera {
            self.state.host_size_received = true;
            return Ok(None);
        }
        self.reset_navigation();
        self.state.camera = next;
        if !self.state.host_size_received {
            self.state.camera.fit_scene(&self.state.scene);
            self.state.dimensions.state.reconsider_hidden_dimensions();
        }
        self.state.host_size_received = true;
        self.compose(false)
    }
    fn reset_navigation(&mut self) {
        self.state.editor.cancel();
        self.state.pan = None;
        self.state.dimensions.context.navigation_active = false;
        self.state.dimensions.context.hovered = None;
        self.state.dimension_hover = None;
    }
    /// Releases transient navigation and hover state.
    ///
    /// # Errors
    /// Rejects malformed requests, unsupported protocol versions or invalid scene projection.
    pub fn cancel_json(
        &mut self,
        encoded: &str,
    ) -> Result<Option<DetachedInteractionUpdate>, String> {
        let request: CancelRequest = decode_request(encoded)?;
        require_version(request.version)?;
        self.state.editor.cancel();
        self.state.pan = None;
        self.state.dimensions.context.navigation_active = false;
        self.state.dimensions.context.hovered = None;
        self.state.dimension_hover = None;
        self.compose(false)
    }
    fn normalized_position(&self, client: [f64; 2], captured: bool) -> Option<ScreenPoint> {
        let size = self.state.camera.viewport().screen_size;
        let rect = crate::ClientRect {
            left: 0.0,
            top: 0.0,
            width: size[0],
            height: size[1],
        };
        if captured {
            crate::normalize_captured_client_point(rect, size, client)
        } else {
            crate::normalize_client_point(rect, size, client)
        }
    }
    /// Picks accepted geometry or updates captured panning without authoring geometry.
    ///
    /// # Errors
    /// Rejects invalid coordinates, changed pan pointer identity or invalid scene projection.
    pub fn pointer_json(
        &mut self,
        encoded: &str,
    ) -> Result<Option<DetachedInteractionUpdate>, String> {
        let request: PointerRequest = decode_request(encoded)?;
        require_version(request.version)?;
        if ![request.x, request.y].into_iter().all(f64::is_finite)
            || request.pointer_id > MAX_SAFE_INTEGER
        {
            return Err("Invalid local pointer".into());
        }
        let position = self
            .normalized_position([request.x, request.y], self.state.pan.is_some())
            .ok_or("local pointer is outside the fitted sketch plane")?;
        let before = self.state.editor.selection_presentation_state();
        let hover_before = self.state.editor.hover_state();
        if matches!(request.phase, PointerPhase::Down) && request.buttons == MIDDLE_POINTER_BUTTON {
            self.state.editor.cancel();
            self.state.pan = Some(CanvasPanGesture {
                pointer_id: request.pointer_id,
                origin: position,
                origin_center: self.state.camera.model_center(),
            });
            self.state.dimensions.context.navigation_active = true;
            self.state.dimensions.context.hovered = None;
            self.state.dimension_hover = None;
        } else if let Some(pan) = self.state.pan {
            if pan.pointer_id != request.pointer_id {
                return Err("Local pan pointer changed".into());
            }
            self.state
                .camera
                .pan_from(pan.origin_center, pan.origin, position);
            if matches!(request.phase, PointerPhase::Up) {
                self.state.pan = None;
                self.state.dimensions.context.navigation_active = false;
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
                    self.state.dimensions.context.navigation_active = false;
                    self.state.editor.select_at(&self.state.scene, input);
                    self.state.dimensions.state.focus = None;
                }
                PointerPhase::Move => {
                    if request.buttons == 0 {
                        self.state.editor.pointer_move(&self.state.scene, input);
                    }
                }
                PointerPhase::Up => {}
            }
        }
        if matches!(request.phase, PointerPhase::Move)
            && request.buttons == 0
            && self.state.pan.is_none()
            && hover_before == self.state.editor.hover_state()
            && before == self.state.editor.selection_presentation_state()
        {
            return Ok(None);
        }
        self.compose(before != self.state.editor.selection_presentation_state())
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one bounded presentation command vocabulary with shared scene semantics"
    )]
    /// Applies personal visibility, selection and dimension display commands.
    ///
    /// # Errors
    /// Rejects unknown commands, stale identities, invalid preferences or scene projection.
    pub fn dispatch_json(
        &mut self,
        encoded: &str,
    ) -> Result<Option<DetachedInteractionUpdate>, String> {
        let request: CommandRequest = decode_request(encoded)?;
        require_version(request.version)?;
        let before = self.state.editor.selection_presentation_state();
        let previous_context = self.state.dimensions.context.clone();
        let transient = request.command.starts_with("dimensions.hover")
            || request.command.starts_with("dimensions.navigation.");
        match request.command.as_str() {
            "explorer.visibility.set"
            | "explorer.visibility.isolate"
            | "explorer.visibility.restore"
            | "view.construction.toggle" => {
                self.state.visibility_seed.dispatch(
                    &mut self.state.visibility,
                    &request.command,
                    request.payload.clone(),
                )?;
                self.apply_visibility()?;
            }
            "view.fit" => {
                self.reset_navigation();
                self.state.camera.fit_scene(&self.state.scene);
                self.state.dimensions.state.reconsider_hidden_dimensions();
            }
            "view.origin" => {
                self.reset_navigation();
                self.state.camera.center_origin();
            }
            "view.grid.toggle" => {
                self.state.grid_visible = !self.state.grid_visible;
            }
            "selection.clear" => {
                self.state.editor.set_selection([]);
                self.state.dimensions.state.focus = None;
            }
            "dimensions.mode" => {
                self.state.dimensions.state.mode = mode(
                    request
                        .payload
                        .get("mode")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("Missing dimension mode")?,
                )?;
                self.state.dimensions.context.navigation_active = false;
                self.state.dimensions.state.focus = None;
                self.state.dimensions.context.hovered = None;
                self.state.dimension_hover = None;
            }
            "dimensions.focus" => {
                let id = request
                    .payload
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("Missing dimension ID")?;
                self.state.dimensions.state.focus = Some(
                    *self
                        .state
                        .dimensions
                        .ids
                        .get(id)
                        .ok_or("Stale dimension ID")?,
                );
                self.state.dimensions.context.navigation_active = false;
                if self.state.dimensions.state.mode == DimensionDisplayMode::Hidden {
                    self.state.dimensions.state.mode = DimensionDisplayMode::Focused;
                }
            }
            "dimensions.clearPins" => {
                self.state.dimensions.state.pins.clear();
                self.state.dimensions.context.navigation_active = false;
            }
            "dimensions.pin" => {
                let id = request
                    .payload
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("Missing dimension ID")?;
                let key = *self
                    .state
                    .dimensions
                    .ids
                    .get(id)
                    .ok_or("Stale dimension ID")?;
                let pinned = request
                    .payload
                    .get("pinned")
                    .and_then(serde_json::Value::as_bool)
                    .ok_or("Missing pin state")?;
                if !pinned {
                    self.state.dimensions.state.pins.retain(|p| *p != key);
                } else if !self.state.dimensions.state.pins.contains(&key) {
                    if self.state.dimensions.state.pins.len()
                        >= DimensionPresentationState::MAX_PINS
                    {
                        return Err("At most four measurements can be pinned".into());
                    }
                    self.state.dimensions.state.pins.push(key);
                }
                self.state.dimensions.context.navigation_active = false;
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
                self.state.dimension_hover = if self.state.pan.is_some()
                    || self.state.dimensions.context.navigation_active
                {
                    None
                } else {
                    crate::dimension_hover_target(
                        &self.state.scene,
                        &self.state.editor,
                        self.state.dimension_hover,
                        position,
                    )
                };
                self.state.dimensions.context.hovered =
                    self.state.dimension_hover.map(|(item, _)| item);
            }
            "dimensions.hover.clear" => {
                self.state.dimensions.context.hovered = None;
                self.state.dimension_hover = None;
            }
            "dimensions.navigation.begin" => {
                self.state.dimensions.context.navigation_active = true;
                self.state.dimensions.context.hovered = None;
                self.state.dimension_hover = None;
            }
            "dimensions.navigation.end" => {
                self.state.dimensions.context.navigation_active = false;
            }
            _ => return Err("Command requires the authoritative workbench".into()),
        }
        if transient && previous_context == self.state.dimensions.context {
            return Ok(None);
        }
        self.compose(before != self.state.editor.selection_presentation_state())
    }
    /// Prepares native controls and annotations for a detached frame, without rendering or solving.
    ///
    /// # Errors
    /// Rejects invalid scene reprojection or unavailable native curve controls.
    pub fn prepare_frame(&mut self, context: DetachedFrameContext) -> Result<(), String> {
        if self.state.scene.viewport != self.state.camera.viewport() {
            self.state
                .scene
                .reproject_viewport(self.state.camera.viewport())
                .map_err(|e| e.to_string())?;
        }
        self.state
            .editor
            .populate_curve_controls(&mut self.state.scene)
            .map_err(|e| e.to_string())?;
        self.state.dimensions.context.selection = self.state.editor.selection().to_vec();
        if let DetachedFrameContext::Authoring { active_dimension } = context {
            self.state.dimensions.context.active = active_dimension;
            // An authored value first enters this detached scene during the
            // tool preview. Let its native explicit-interest rule reveal it;
            // navigation-only frames otherwise retain the existing callouts.
            if self.state.dimensions.context.active.is_some() {
                self.state.dimensions.context.navigation_active = false;
            }
        }
        self.state.dimensions.state.apply(
            &mut self.state.scene,
            &self.state.dimensions.layout,
            &self.state.dimensions.context,
        );
        Ok(())
    }
    fn compose(
        &mut self,
        selection_changed: bool,
    ) -> Result<Option<DetachedInteractionUpdate>, String> {
        self.prepare_frame(DetachedFrameContext::Navigation)?;
        Ok(Some(DetachedInteractionUpdate {
            state: self.state(),
            selection_changed,
            server_frame_compatible: false,
        }))
    }
    /// Installs only disposable prediction presentation; this never creates editing authority.
    ///
    /// # Errors
    /// Rejects invalid cameras, selection correspondence or stale dimension preferences.
    pub fn prepare_prediction(
        &mut self,
        scene: EditorScene,
        state: DetachedInteractionState,
        mapping: &PresentationMapping,
    ) -> Result<(), String> {
        self.replace_prediction_scene(scene, &state.visibility, mapping)?;
        self.state.camera = camera(state.viewport)?;
        self.state.grid_visible = state.grid_visible;
        self.state
            .editor
            .restore_selection_presentation(
                &self.state.scene,
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
        self.state.dimensions.state.mode = mode(&state.dimension_mode)?;
        if state.dimension_pins.len() > DimensionPresentationState::MAX_PINS {
            return Err("Too many prediction dimension pins".into());
        }
        let key = |id: &String| {
            self.state
                .dimensions
                .ids
                .get(id)
                .copied()
                .ok_or_else(|| "Stale prediction dimension".to_string())
        };
        self.state.dimensions.state.pins = state
            .dimension_pins
            .iter()
            .map(key)
            .collect::<Result<_, _>>()?;
        self.state.dimensions.state.focus = state.dimension_focus.as_ref().map(key).transpose()?;
        self.state.dimensions.context.navigation_active = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
