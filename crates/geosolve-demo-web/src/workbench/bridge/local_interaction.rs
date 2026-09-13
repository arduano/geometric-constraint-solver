// SPDX-License-Identifier: GPL-3.0-or-later
//! Detached canvas navigation and independently retained accepted-model browsing.
//! Navigation owns no solver. Browsing can describe source edits but cannot publish them.

use super::*;
#[cfg(test)]
use geosolve_constraint_editor::CurvePickContext;
use geosolve_constraint_editor::{
    DimensionPresentationState, GeometryInteractionPolicy, SelectionPresentationState, Viewport,
};
use std::collections::BTreeMap;

mod authoring_presentation;
mod presence;
#[cfg(test)]
mod replacement_tests;
mod visibility;
pub(super) use visibility::browsing_visibility_seed;
#[cfg(test)]
mod visibility_tests;

const FORMAT: &str = "geosolve-local-interaction-v1";

pub(super) use geosolve_constraint_editor::DetachedDimensionPresentation as LocalDimensionSeed;
use geosolve_constraint_editor::PresentationMapping;

use geosolve_constraint_editor::detached_interaction::{
    DetachedCanvas, DetachedCanvasState, DetachedFrameContext,
    DetachedInteractionSeed as InteractionSeed, DetachedInteractionState as InteractionState,
    DetachedInteractionUpdate, camera, mode,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InteractionUpdate {
    frame: FrameSnapshot,
    state: InteractionState,
    selection_changed: bool,
    server_frame_compatible: bool,
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
        // Retain the unhidden accepted geometry so a tab can restore personal
        // visibility without a server round trip or reconstructing the model.
        let full_scene;
        let scene = if self.explorer_visibility.hidden_rows.is_empty() {
            scene
        } else {
            full_scene = self
                .authority
                .scene_presentation(
                    self.camera.viewport(),
                    super::super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
                )
                .scene
                .ok_or("No complete accepted visibility scene")?;
            &full_scene
        };
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
            visibility_seed: visibility::visibility_seed(self, scene),
            visibility: self.local_visibility_state(),
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
            || self
                .editor()
                .editor()
                .geometry_draft_status()
                .is_some_and(|draft| draft.completed_stages > 0)
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
        if state.visibility != self.local_visibility_state() {
            self.validate_local_visibility(&state.visibility)?;
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
        self.apply_local_visibility(&state.visibility);
        self.apply_local_dimension_preferences(
            mode,
            &state.dimension_pins,
            state.dimension_focus.as_deref(),
        );
        Ok(())
    }
}

pub(crate) use super::accepted_browsing::BrowsingPresentation;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PredictionGuide {
    Point {
        position: [f64; 2],
    },
    Polyline {
        points: Vec<[f64; 2]>,
        closed: bool,
    },
    Rectangle {
        first: [f64; 2],
        second: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    ArcRadius {
        center: [f64; 2],
        start: [f64; 2],
    },
    EllipticalArcSupport {
        center: [f64; 2],
        major_axis_point: [f64; 2],
        support_points: Vec<[f64; 2]>,
        trim_start: Option<[f64; 2]>,
    },
    ControlPolygon {
        curve_kind: PredictionCurveKind,
        points: Vec<[f64; 2]>,
    },
    CircularArc {
        center: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
        radius: f64,
        sweep_radians: f64,
        large_arc: bool,
        sweep: geosolve_sketch::DocumentArcSweep,
    },
    AdvancedCurve {
        curve_kind: PredictionCurveKind,
        control_points: Vec<[f64; 2]>,
        curve_points: Vec<[f64; 2]>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum PredictionCurveKind {
    QuadraticBezier,
    CubicBezier,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    Nurbs,
}
impl PredictionCurveKind {
    fn native(self) -> geosolve_constraint_editor::AdvancedConstructionKind {
        use geosolve_constraint_editor::AdvancedConstructionKind as Kind;
        match self {
            Self::QuadraticBezier => Kind::QuadraticBezier,
            Self::CubicBezier => Kind::CubicBezier,
            Self::Ellipse => Kind::Ellipse,
            Self::EllipticalArc => Kind::EllipticalArc,
            Self::RationalQuadraticConic => Kind::RationalQuadraticConic,
            Self::Parabola => Kind::Parabola,
            Self::Hyperbola => Kind::Hyperbola,
            Self::Nurbs => Kind::Nurbs,
        }
    }
}
enum PredictionStage {
    Geometry(geosolve_constraint_editor::ConstructionPreviewGeometry),
    Guide(geosolve_constraint_editor::ConstructionPreview),
}
impl PredictionStage {
    fn borrowed(&self) -> geosolve_sketch_render::PredictionPreview<'_> {
        match self {
            Self::Geometry(geometry) => {
                geosolve_sketch_render::PredictionPreview::Geometry(geometry)
            }
            Self::Guide(guide) => geosolve_sketch_render::PredictionPreview::Guide(guide),
        }
    }
}
impl PredictionGuide {
    fn preview(self) -> PredictionStage {
        use geosolve_constraint_editor::ConstructionPreview as Guide;
        use geosolve_constraint_editor::ConstructionPreviewGeometry as Geometry;
        let geometry = match self {
            Self::Point { position } => Geometry::Point { position },
            Self::Polyline { mut points, closed } => {
                if closed && let Some(first) = points.first().copied() {
                    points.push(first);
                }
                Geometry::Polyline { points }
            }
            Self::Rectangle { first, second } => Geometry::Rectangle { first, second },
            Self::Circle { center, radius } => Geometry::Circle { center, radius },
            Self::CircularArc {
                center,
                start,
                end,
                radius,
                sweep_radians,
                large_arc,
                sweep,
            } => Geometry::CircularArc {
                center,
                start,
                end,
                radius,
                sweep_radians,
                large_arc,
                sweep,
            },
            Self::AdvancedCurve {
                curve_kind,
                control_points,
                curve_points,
            } => Geometry::AdvancedCurve {
                kind: curve_kind.native(),
                control_points,
                curve_points,
            },
            Self::ArcRadius { center, start } => {
                return PredictionStage::Guide(Guide::ArcRadiusGuide { center, start });
            }
            Self::EllipticalArcSupport {
                center,
                major_axis_point,
                support_points,
                trim_start,
            } => {
                return PredictionStage::Guide(Guide::EllipticalArcSupport {
                    center,
                    major_axis_point,
                    support_points,
                    trim_start,
                });
            }
            Self::ControlPolygon { curve_kind, points } => {
                return PredictionStage::Guide(Guide::ControlPolygon {
                    kind: curve_kind.native(),
                    points,
                });
            }
        };
        PredictionStage::Geometry(geometry)
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
        operation: Option<authoring_presentation::OperationPresentation>,
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
    local
        .native
        .prepare_prediction(scene, state.clone(), &mapping)?;
    let mut frame = local.compose_frame_with_operation(request.operation.as_ref())?;
    if let Some(construction) = request.construction {
        let preview = construction.preview.map(PredictionGuide::preview);
        let guides = construction
            .inference_guides
            .into_iter()
            .map(PredictionGuide::inference)
            .collect::<Result<Vec<_>, _>>()?;
        let overlay = geosolve_sketch_render::compose_prediction_stage(
            state.viewport,
            preview.as_ref().map(PredictionStage::borrowed),
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
    native: DetachedCanvas,
    presence: presence::PresenceState,
}
impl std::ops::Deref for LocalInteraction {
    type Target = DetachedCanvasState;
    fn deref(&self) -> &Self::Target {
        &self.native
    }
}
impl LocalInteraction {
    pub(crate) fn new(encoded: &str) -> Result<Self, String> {
        Ok(Self {
            native: DetachedCanvas::new(encoded)?,
            presence: presence::PresenceState::default(),
        })
    }
    fn state(&self) -> InteractionState {
        self.native.state()
    }
    pub(crate) fn state_json(&self) -> Result<String, String> {
        self.native.state_json()
    }
    pub(crate) fn export_presentation_json(&self) -> Result<String, String> {
        self.native.export_presentation_json()
    }
    pub(crate) fn authoring_pointer_json(&self, encoded: &str) -> Result<String, String> {
        self.native.authoring_pointer_json(encoded)
    }
    fn compose(&mut self, selection_changed: bool) -> Result<String, String> {
        let frame = self.compose_frame()?;
        serde_json::to_string(&InteractionUpdate {
            frame,
            state: self.state(),
            selection_changed,
            server_frame_compatible: false,
        })
        .map_err(|e| e.to_string())
    }
    fn paint_update(&self, update: Option<DetachedInteractionUpdate>) -> Result<String, String> {
        let Some(update) = update else {
            return Ok("null".into());
        };
        let frame = self.render_frame_with_operation(None)?;
        serde_json::to_string(&InteractionUpdate {
            frame,
            state: update.state,
            selection_changed: update.selection_changed,
            server_frame_compatible: update.server_frame_compatible,
        })
        .map_err(|e| e.to_string())
    }
    pub(crate) fn replace_json(&mut self, encoded: &str) -> Result<String, String> {
        let (native, update) = self.native.prepare_replacement(encoded)?;
        let next = Self {
            native,
            presence: presence::PresenceState::default(),
        };
        let result = next.paint_update(Some(update))?;
        *self = next;
        Ok(result)
    }
    pub(crate) fn restore_selection_json(&mut self, encoded: &str) -> Result<String, String> {
        let update = self.native.restore_selection_json(encoded)?;
        self.paint_update(update)
    }
    pub(crate) fn wheel_json(&mut self, encoded: &str) -> Result<String, String> {
        let update = self.native.wheel_json(encoded)?;
        self.paint_update(update)
    }
    pub(crate) fn resize_json(&mut self, encoded: &str) -> Result<String, String> {
        let update = self.native.resize_json(encoded)?;
        self.paint_update(update)
    }
    pub(crate) fn cancel_json(&mut self, encoded: &str) -> Result<String, String> {
        let update = self.native.cancel_json(encoded)?;
        self.paint_update(update)
    }
    pub(crate) fn pointer_json(&mut self, encoded: &str) -> Result<String, String> {
        let update = self.native.pointer_json(encoded)?;
        self.paint_update(update)
    }
    pub(crate) fn dispatch_json(&mut self, encoded: &str) -> Result<String, String> {
        let update = self.native.dispatch_json(encoded)?;
        self.paint_update(update)
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
    pub(super) fn compose_frame(&mut self) -> Result<FrameSnapshot, String> {
        self.compose_frame_with_operation(None)
    }
    fn compose_frame_with_operation(
        &mut self,
        operation: Option<&authoring_presentation::OperationPresentation>,
    ) -> Result<FrameSnapshot, String> {
        self.native.prepare_frame(operation.map_or(
            DetachedFrameContext::Navigation,
            |operation| {
                DetachedFrameContext::Authoring {
                    active_dimension: operation
                        .provisional
                        .iter()
                        .find(|item| matches!(item, SelectionItem::Dimension(_)))
                        .copied(),
                }
            },
        ))?;
        self.render_frame_with_operation(operation)
    }
    fn render_frame_with_operation(
        &self,
        operation: Option<&authoring_presentation::OperationPresentation>,
    ) -> Result<FrameSnapshot, String> {
        let offset = operation.and_then(authoring_presentation::OperationPresentation::offset);
        let mut pending = operation.map_or_else(Vec::new, |operation| operation.pending.clone());
        if let Some(offset) = &offset {
            pending.extend(&offset.pending);
        }
        let mut scene = geosolve_sketch_render::compose_draw_frame(
            Some(&self.scene),
            None,
            &[],
            self.editor.selection(),
            &pending,
            operation.map_or(&[], |operation| operation.provisional.as_slice()),
            operation.map_or_else(
                || self.editor.hover_state(),
                authoring_presentation::OperationPresentation::hover,
            ),
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
            offset.as_ref(),
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
            browsing.model.view().scene().presentation_document().id()
        );
        let accepted = std::ptr::from_ref(
            browsing
                .model
                .view()
                .session()
                .editor()
                .coordinator()
                .accepted_materialization()
                .unwrap(),
        );
        let identity = browsing
            .model
            .view()
            .session()
            .editor()
            .coordinator()
            .intent()
            .identity();
        let code_identity = browsing
            .model
            .view()
            .session()
            .source()
            .unwrap()
            .token()
            .clone();
        let before_project = browsing.export_project_json().unwrap();
        let before_design = browsing.workspace_design_json().unwrap();
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
                    .model
                    .view()
                    .session()
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
                        .model
                        .view()
                        .session()
                        .editor()
                        .coordinator()
                        .accepted_materialization()
                        .unwrap()
                ),
                accepted
            );
            assert_eq!(
                browsing
                    .model
                    .view()
                    .session()
                    .editor()
                    .coordinator()
                    .intent()
                    .identity(),
                identity
            );
            assert_eq!(
                browsing.model.view().session().source().unwrap().token(),
                &code_identity
            );
            assert_eq!(browsing.export_project_json().unwrap(), before_project);
            assert_eq!(browsing.workspace_design_json().unwrap(), before_design);
        }
        let before = browsing
            .model
            .view()
            .session()
            .editor()
            .editor()
            .selection()
            .to_vec();
        let mut stale: serde_json::Value =
            serde_json::from_str(&local.state_json().unwrap()).unwrap();
        stale["sceneKey"] = "obsolete".into();
        assert!(browsing.update_json(&stale.to_string()).is_err());
        assert_eq!(
            browsing
                .model
                .view()
                .session()
                .editor()
                .editor()
                .selection(),
            before
        );
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
        let original = browsing.export_project_json().unwrap();
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
            .model
            .view()
            .session()
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
        native.validate(browsing.model.view().scene()).unwrap();
        state.curve_picks[0].parameter = -1.0;
        assert!(
            browsing
                .update_json(&serde_json::to_string(&state).unwrap())
                .is_err()
        );
        assert_eq!(
            browsing
                .model
                .view()
                .session()
                .editor()
                .editor()
                .selection_presentation_state(),
            native
        );
        assert_eq!(browsing.export_project_json().unwrap(), original);
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
            browsing.export_project_json().unwrap(),
            browsing.workspace_design_json().unwrap(),
        );
        let accepted = std::ptr::from_ref(
            browsing
                .model
                .view()
                .session()
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
        let extraction = describe(
            "authoring.parameter.extract",
            serde_json::json!({"authority":authority,"id":control["id"],"label":"Named length","description":"Preserve the driving span","isKeyParameter":true}),
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &browsing.describe_json(&extraction).unwrap()
            )
            .unwrap(),
            serde_json::json!({"mutation":"extract_parameter","declaration":"length","path":["value"],"symbol":"parameter1","variable":"parameter1","presentation":{"label":"Named length","description":"Preserve the driving span","isKeyParameter":true}})
        );
        assert_eq!(
            browsing.model.project(),
            serde_json::from_str::<serde_json::Value>(&before.0).unwrap()["contents"]
                .as_str()
                .unwrap()
        );
        let mut stale_extraction: serde_json::Value = serde_json::from_str(&extraction).unwrap();
        stale_extraction["payload"]["authority"] = serde_json::json!("older source authority");
        assert!(
            browsing
                .describe_json(&stale_extraction.to_string())
                .is_err()
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
                browsing.export_project_json().unwrap(),
                browsing.workspace_design_json().unwrap()
            ),
            before
        );
        assert_eq!(
            std::ptr::from_ref(
                browsing
                    .model
                    .view()
                    .session()
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
    fn operation_prediction_paint_preserves_native_pending_and_offset_guides_after_navigation() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("operation-paint".into()),
            compiled,
        )
        .unwrap()
        .to_canonical_json()
        .unwrap();
        let mut bridge = WorkbenchBridge::restore(&project).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
        let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let source = bridge.export_project_json().unwrap();
        let curve = local.scene.curves.first().unwrap().clone();
        let span = curve.span;
        let active_dimension = local
            .scene
            .annotations
            .iter()
            .find(|annotation| matches!(annotation.item, SelectionItem::Dimension(_)))
            .unwrap()
            .item;
        local
            .dispatch_json(
                r#"{"version":2,"command":"dimensions.mode","payload":{"mode":"hidden"}}"#,
            )
            .unwrap();
        let start = local
            .scene
            .viewport
            .screen_to_model(curve.screen_polyline[0]);
        let end = local
            .scene
            .viewport
            .screen_to_model(*curve.screen_polyline.last().unwrap());
        local
            .wheel_json(r#"{"version":2,"x":300,"y":250,"deltaX":0,"deltaY":-70,"ctrl":false}"#)
            .unwrap();
        let input = serde_json::json!({"scene":pair["seed"]["scene"],"view":{"seed":pair["seed"],"state":local.state()},
            "operation":{"pending":[SelectionItem::Curve(span)],"provisional":[active_dimension],"hover":null,"context_owner":null,
                "offset":{"pending":[SelectionItem::Curve(span)],"unavailable":[],"unavailable_message":null,
                    "chain":{"spans":[{"span":span,"traversal":"forward"}],
                        "start":{"span":span,"endpoint":"start","model_position":start},
                        "end":{"span":span,"endpoint":"end","model_position":end}}}}});
        let frame: serde_json::Value =
            serde_json::from_str(&authoring_preview_json(&input.to_string()).unwrap()).unwrap();
        let items = frame["scene"]["items"].as_array().unwrap();
        assert!(items.iter().all(|item| item["interactive"] == false));
        assert!(
            items
                .iter()
                .any(|item| item["className"] == "wb-offset-chain-direction")
        );
        assert!(
            items.iter().any(|item| item["className"]
                .as_str()
                .is_some_and(|class| class.contains("dimension"))
                && item["visible"] != false),
            "the active authored dimension remains visible in Hidden mode"
        );
        assert!(
            items
                .iter()
                .any(|item| item["style"]["stroke"] == "#79bfc4")
        );
        let expected = local.camera.viewport().model_to_screen(start);
        assert!(
            items
                .iter()
                .any(|item| item["center"] == serde_json::json!([expected.x, expected.y]))
        );
        assert_eq!(bridge.export_project_json().unwrap(), source);
    }

    #[test]
    fn authoring_prediction_renders_staged_and_advanced_native_guides() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":2}"#).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
        let local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
        let source = bridge.export_project_json().unwrap();
        let cases = [
            (
                serde_json::json!({"kind":"arc_radius","center":[0,0],"start":[3,0]}),
                "wb-draft-radius",
            ),
            (
                serde_json::json!({"kind":"elliptical_arc_support","center":[0,0],"major_axis_point":[3,0],"support_points":[[3,0],[0,2],[-3,0]],"trim_start":[3,0]}),
                "wb-draft-ellipse-support",
            ),
            (
                serde_json::json!({"kind":"control_polygon","curve_kind":"cubic_bezier","points":[[0,0],[3,2],[6,2]]}),
                "wb-draft-control-polygon",
            ),
            (
                serde_json::json!({"kind":"circular_arc","center":[0,0],"start":[3,0],"end":[0,3],"radius":3,"sweep_radians":std::f64::consts::FRAC_PI_2,"large_arc":false,"sweep":"counter_clockwise"}),
                "wb-draft-arc",
            ),
            (
                serde_json::json!({"kind":"advanced_curve","curve_kind":"quadratic_bezier","control_points":[[0,0],[3,2],[6,0]],"curve_points":[[0,0],[3,1],[6,0]]}),
                "wb-draft-advanced-curve",
            ),
        ];
        for (preview, class) in cases {
            let input = serde_json::json!({"scene":pair["seed"]["scene"],"view":{"seed":pair["seed"],"state":local.state()},"construction":{"preview":preview,"inference_guides":[]}});
            let frame: serde_json::Value =
                serde_json::from_str(&authoring_preview_json(&input.to_string()).unwrap()).unwrap();
            let items = frame["scene"]["items"].as_array().unwrap();
            assert!(
                items.iter().any(|item| item["className"] == class),
                "missing {class}"
            );
            assert!(items.iter().all(|item| item["interactive"] == false));
        }
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
