// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(any(target_arch = "wasm32", test))]
mod action_surface;
#[cfg(any(target_arch = "wasm32", test))]
mod effect_adapter;
#[cfg(any(target_arch = "wasm32", test))]
mod icons;
#[cfg(any(target_arch = "wasm32", test))]
mod panels;
#[cfg(any(target_arch = "wasm32", test))]
mod persistence;
#[cfg(target_arch = "wasm32")]
mod platform;
#[cfg(any(target_arch = "wasm32", test))]
mod samples;
#[cfg(any(target_arch = "wasm32", test))]
mod scene;

#[cfg(target_arch = "wasm32")]
const WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS: f64 = 0.25;

#[cfg(any(target_arch = "wasm32", test))]
const CANVAS_BROWSER_DEFAULT_GUARD_EVENTS: [&str; 2] = ["selectstart", "dragstart"];

#[cfg(any(target_arch = "wasm32", test))]
const CANVAS_POINTER_TERMINAL_EVENTS: [&str; 3] =
    ["pointerup", "pointercancel", "lostpointercapture"];

#[cfg(any(target_arch = "wasm32", test))]
const CANVAS_PAN_POINTER_EVENTS: [&str; 3] = ["pointerdown", "pointermove", "pointerup"];

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerCaptureKind {
    Point,
    Fillet,
    Pan,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerOwnership {
    Owned,
    Foreign,
    Uncaptured,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPanPointerDownRoute {
    BeginPan,
    PreserveCapturedInteraction,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapturedCanvasPointer {
    pointer_id: i32,
    kind: CanvasPointerCaptureKind,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerTerminal {
    PointerUp { pointer_id: i32 },
    PointerCancel { pointer_id: i32 },
    LostPointerCapture { pointer_id: i32 },
    InteractionCancel,
    CameraCancel,
}

#[cfg(any(target_arch = "wasm32", test))]
impl CanvasPointerTerminal {
    const fn pointer_id(self) -> Option<i32> {
        match self {
            Self::PointerUp { pointer_id }
            | Self::PointerCancel { pointer_id }
            | Self::LostPointerCapture { pointer_id } => Some(pointer_id),
            Self::InteractionCancel | Self::CameraCancel => None,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerTerminalDisposition {
    Complete,
    Cancel,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanvasPointerTerminalRoute {
    captured: CapturedCanvasPointer,
    disposition: CanvasPointerTerminalDisposition,
    release_platform_capture: bool,
}

/// Browser-only pointer ownership bookkeeping.
///
/// Gesture meaning remains in the headless editor. This state records only which
/// platform pointers the SVG promised to keep delivering so terminal browser
/// events can release or cancel that promise exactly once.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct CanvasPointerCaptures {
    active: Option<CapturedCanvasPointer>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl CanvasPointerCaptures {
    fn begin(&mut self, pointer: CapturedCanvasPointer) -> bool {
        if pointer.pointer_id < 0 || self.active.is_some() {
            return false;
        }
        self.active = Some(pointer);
        true
    }

    fn ownership(&self, pointer_id: i32) -> CanvasPointerOwnership {
        match self.active {
            Some(active) if active.pointer_id == pointer_id => CanvasPointerOwnership::Owned,
            Some(_) => CanvasPointerOwnership::Foreign,
            None => CanvasPointerOwnership::Uncaptured,
        }
    }

    fn contains(&self, pointer_id: i32) -> bool {
        self.ownership(pointer_id) == CanvasPointerOwnership::Owned
    }

    fn is_empty(&self) -> bool {
        self.active.is_none()
    }

    fn route_terminal(
        &mut self,
        terminal: CanvasPointerTerminal,
    ) -> Option<CanvasPointerTerminalRoute> {
        if terminal
            .pointer_id()
            .is_some_and(|pointer_id| !self.contains(pointer_id))
        {
            return None;
        }
        let captured = self.active.take()?;
        Some(CanvasPointerTerminalRoute {
            captured,
            disposition: if matches!(terminal, CanvasPointerTerminal::PointerUp { .. }) {
                CanvasPointerTerminalDisposition::Complete
            } else {
                CanvasPointerTerminalDisposition::Cancel
            },
            release_platform_capture: !matches!(
                terminal,
                CanvasPointerTerminal::LostPointerCapture { .. }
            ),
        })
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn route_canvas_pan_pointer_down(captures: &CanvasPointerCaptures) -> CanvasPanPointerDownRoute {
    if captures.is_empty() {
        CanvasPanPointerDownRoute::BeginPan
    } else {
        CanvasPanPointerDownRoute::PreserveCapturedInteraction
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn canvas_pointer_capture_kind(
    kind: geosolve_constraint_editor::ActivePointerGestureKind,
) -> CanvasPointerCaptureKind {
    match kind {
        geosolve_constraint_editor::ActivePointerGestureKind::Point => {
            CanvasPointerCaptureKind::Point
        }
        geosolve_constraint_editor::ActivePointerGestureKind::FilletRadius
        | geosolve_constraint_editor::ActivePointerGestureKind::FilletContact => {
            CanvasPointerCaptureKind::Fillet
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct PointerMoveQueue {
    pending: Option<geosolve_constraint_editor::PointerInput>,
    next_generation: u64,
    scheduled_generation: Option<u64>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl PointerMoveQueue {
    fn push(&mut self, input: geosolve_constraint_editor::PointerInput) -> Option<u64> {
        self.pending = Some(input);
        if self.scheduled_generation.is_some() {
            return None;
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        self.scheduled_generation = Some(self.next_generation);
        Some(self.next_generation)
    }

    fn take_for_frame(
        &mut self,
        generation: u64,
    ) -> Option<geosolve_constraint_editor::PointerInput> {
        if self.scheduled_generation != Some(generation) {
            return None;
        }
        self.scheduled_generation = None;
        self.pending.take()
    }

    fn cancel_frame(&mut self, generation: u64) {
        if self.scheduled_generation == Some(generation) {
            self.scheduled_generation = None;
        }
    }

    fn drain_before_terminal(&mut self) -> Option<geosolve_constraint_editor::PointerInput> {
        self.scheduled_generation = None;
        self.pending.take()
    }

    /// Invalidates a coalesced ordinary move before an immediately handled
    /// semantic overlay transition.
    ///
    /// The scheduled animation-frame closure will observe the missing
    /// generation and do nothing, so it cannot later clear the newer Fillet
    /// action preview with an older canvas sample.
    fn invalidate_before_immediate_action(&mut self) {
        self.scheduled_generation = None;
        self.pending = None;
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoringItemInput {
    CanvasPointerDown,
    CanvasClick,
    TreeClick,
}

#[cfg(any(target_arch = "wasm32", test))]
const fn owns_authoring_pick(input: AuthoringItemInput) -> bool {
    matches!(
        input,
        AuthoringItemInput::CanvasPointerDown | AuthoringItemInput::TreeClick
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn change_owns_option_control_click(
    tag_name: &str,
    in_palette_options: bool,
    in_construction_options: bool,
    in_branch_editor: bool,
) -> bool {
    matches!(tag_name, "INPUT" | "SELECT" | "OPTION")
        && (in_palette_options || in_construction_options || in_branch_editor)
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct OverlayRect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_arch = "wasm32", test))]
impl OverlayRect {
    fn is_valid(self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.width.is_finite()
            && self.width >= 0.0
            && self.height.is_finite()
            && self.height >= 0.0
    }

    fn right(self) -> f64 {
        self.left + self.width
    }
}

/// Places a palette-triggered surface inside the canvas coordinate system.
///
/// The trigger can live outside the canvas (as the Fillet trigger does), while
/// the returned point is always clamped to the visible canvas inset.
#[cfg(any(target_arch = "wasm32", test))]
fn canvas_overlay_position(
    trigger: OverlayRect,
    canvas: OverlayRect,
    overlay: OverlayRect,
    inset: f64,
    gap: f64,
) -> Option<geosolve_constraint_editor::ScreenPoint> {
    if !trigger.is_valid()
        || !canvas.is_valid()
        || !overlay.is_valid()
        || !inset.is_finite()
        || inset < 0.0
        || !gap.is_finite()
        || gap < 0.0
    {
        return None;
    }
    let minimum_x = inset.min(canvas.width);
    let minimum_y = inset.min(canvas.height);
    let maximum_x = (canvas.width - overlay.width - inset).max(minimum_x);
    let maximum_y = (canvas.height - overlay.height - inset).max(minimum_y);
    Some(geosolve_constraint_editor::ScreenPoint {
        x: (trigger.right() + gap - canvas.left).clamp(minimum_x, maximum_x),
        y: (trigger.top - canvas.top).clamp(minimum_y, maximum_y),
    })
}

/// Native `details` toggle events are not required to bubble. Capture them at
/// the palette boundary so every present or future palette disclosure that can
/// move the Fillet trigger refreshes the canvas-relative overlay position.
#[cfg(any(target_arch = "wasm32", test))]
const fn palette_details_overlay_reflow_listener() -> (&'static str, bool) {
    ("toggle", true)
}

#[cfg(any(target_arch = "wasm32", test))]
fn geometry_hover_selector(item: geosolve_constraint_editor::SelectionItem) -> Option<String> {
    match item {
        geosolve_constraint_editor::SelectionItem::Point(point) => Some(format!(
            "#wb-viewport [data-editor-item=\"point\"][data-persistent-id=\"{point}\"]"
        )),
        geosolve_constraint_editor::SelectionItem::Curve(span) => Some(format!(
            "#wb-viewport [data-editor-item=\"curve\"][data-persistent-id=\"{}\"][data-editor-segment=\"{}\"]",
            span.curve, span.segment
        )),
        geosolve_constraint_editor::SelectionItem::Feature(feature) => Some(format!(
            "#wb-viewport [data-editor-item=\"feature-corner\"][data-feature-id=\"{feature}\"]"
        )),
        geosolve_constraint_editor::SelectionItem::FeatureCorner(owner) => Some(format!(
            "#wb-viewport [data-editor-item=\"feature-corner\"][data-feature-id=\"{}\"][data-feature-corner-id=\"{}\"]",
            owner.feature, owner.corner
        )),
        geosolve_constraint_editor::SelectionItem::Constraint(_)
        | geosolve_constraint_editor::SelectionItem::Dimension(_) => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn markup_fingerprint(value: &str) -> String {
    let hash = value
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("{hash:016x}")
}

/// Browser-render identity paired with one exact headless computed input.
///
/// Owner/action IDs in markup are intentionally insufficient to activate a
/// Fillet branch. A DOM control also carries this opaque stamp, and an event is
/// admitted only while the adapter still holds the exact input that produced
/// that stamp. This prevents an old element from being silently upgraded to a
/// newer feature revision by rebuilding a target from persistent IDs.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct FilletActionRenderAuthority {
    next_stamp: u64,
    active: Option<(
        u64,
        geosolve_sketch_features::ComputedFeatureEvaluationInput,
    )>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl FilletActionRenderAuthority {
    fn reconcile(
        &mut self,
        input: Option<&geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    ) -> Option<u64> {
        let Some(input) = input else {
            self.active = None;
            return None;
        };
        if let Some((stamp, active)) = self.active
            && active == *input
        {
            return Some(stamp);
        }
        let Some(stamp) = self.next_stamp.checked_add(1) else {
            self.active = None;
            return None;
        };
        self.next_stamp = stamp;
        self.active = Some((stamp, *input));
        Some(stamp)
    }

    fn accepts(
        &self,
        stamp: u64,
        input: Option<&geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    ) -> bool {
        matches!((self.active, input), (Some((active_stamp, active)), Some(current))
            if active_stamp == stamp && active == *current)
    }
}

/// Reconciles every painted action below one canvas sample with the
/// headless nearest-action result.
///
/// SVG action hit corridors can overlap. Their paint order is presentation
/// detail, so the topmost corridor must not suppress a closer independently
/// validated action. A stale or foreign stack still produces no route.
#[cfg(any(target_arch = "wasm32", test))]
fn resolve_canvas_fillet_action_candidates(
    scene: &geosolve_constraint_editor::EditorScene,
    position: geosolve_constraint_editor::ScreenPoint,
    painted: impl IntoIterator<Item = geosolve_constraint_editor::SceneFilletActionTarget>,
) -> Option<geosolve_constraint_editor::SceneFilletActionTarget> {
    painted.into_iter().find(|target| {
        scene.resolve_fillet_action(
            geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                position,
                painted: Some(*target),
            },
            geosolve_constraint_editor::PickTolerance::default(),
        ) == Some(*target)
    })
}

/// Revokes one temporary computed-feature owner. Selection cleanup belongs to
/// the headless coordinator so every caller gets identical lifetime semantics.
#[cfg(any(target_arch = "wasm32", test))]
fn revoke_held_feature_authoring_preview(
    coordinator: &mut geosolve_constraint_editor::RetainedEditorCoordinator,
) {
    coordinator.clear_feature_authoring_preview();
}

/// Synchronizes temporary preview lifetime with every headless transition that
/// no longer exposes a complete candidate.
#[cfg(any(target_arch = "wasm32", test))]
fn observe_feature_authoring_preview_lifecycle(
    coordinator: &mut geosolve_constraint_editor::RetainedEditorCoordinator,
    outcome: &geosolve_constraint_editor::FeatureAuthoringOutcome,
) {
    if matches!(
        outcome,
        geosolve_constraint_editor::FeatureAuthoringOutcome::ModeEntered(_)
            | geosolve_constraint_editor::FeatureAuthoringOutcome::Collecting { .. }
            | geosolve_constraint_editor::FeatureAuthoringOutcome::CandidateCleared(_)
            | geosolve_constraint_editor::FeatureAuthoringOutcome::ModeExited
    ) {
        revoke_held_feature_authoring_preview(coordinator);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::str::FromStr as _;

    use geosolve_constraint_editor::{
        ActionState, AuthoringApplication, AuthoringOperand, AuthoringOptions, AuthoringOutcome,
        AuthoringState, AuthoringTool, BranchAction, ComputedSceneState, ConicConstructionOptions,
        ConstructionPreview, CoordinatorActionKind, DimensionTargetDisplayUnit, DisabledReason,
        EditorEffect, EditorHoverTarget, EditorScene, EditorTool, FeatureAuthoringCandidate,
        FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringPick,
        FeatureAuthoringPointerDownOutcome, FeatureAuthoringStage, FeatureAuthoringState,
        FeatureAuthoringTool, FeatureAuthoringTransaction, Modifiers, NurbsConstructionOptions,
        PickTolerance, PointerInput, ProvisionalInferenceCandidate, RetainedEditorCoordinator,
        SceneFilletActionInput, SceneFilletActionTarget, ScreenPoint, SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        ContactBranchEdit, ContactDomain, ContactNeighborhood, CurveId, CurveSpan, DesignPointId,
        DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineForm, DocumentConstraintId,
        DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentDimensionId,
        DocumentDimensionMode, DocumentHyperbolaBranch, DocumentSolveRequest, PersistentId,
        RetainedSketchDocumentSession, SketchDocument, TangentOrientation,
    };
    use geosolve_sketch_features::{ComputedCornerRef, ComputedFeatureCornerId, ComputedFeatureId};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::JsValue;
    use web_sys::{
        Document, Element, Event, FocusEvent, HtmlElement, HtmlInputElement, HtmlSelectElement,
        KeyboardEvent, MouseEvent, PointerEvent, WheelEvent,
    };

    use super::persistence::{
        LEGACY_STORAGE_KEY, OLDER_STORAGE_KEY, PREVIOUS_STORAGE_KEY, STORAGE_KEY, WorkspaceSnapshot,
    };

    struct Workbench {
        coordinator: RetainedEditorCoordinator,
        authoring: AuthoringState,
        feature_authoring: FeatureAuthoringState,
        feature_candidate: Option<FeatureAuthoringCandidate>,
        feature_pending: Vec<FeatureAuthoringPick>,
        samples: super::samples::SampleCatalogState,
        camera: super::scene::CanvasCamera,
        pan_gesture: Option<PanGesture>,
        pointer_captures: super::CanvasPointerCaptures,
        pointer_moves: Rc<RefCell<super::PointerMoveQueue>>,
        fillet_action_render: super::FilletActionRenderAuthority,
        feature_options_open: bool,
        construction_preview: Option<ConstructionPreview>,
        inference_preview: Option<ProvisionalInferenceCandidate>,
        notice: String,
        problems_open: bool,
    }

    impl Workbench {
        fn resolve_projected_point_move(
            &mut self,
            pointer_id: u64,
            request_id: u64,
            point: geosolve_sketch::DesignPointId,
            model_position: [f64; 2],
        ) -> Vec<EditorEffect> {
            self.coordinator.resolve_projected_point_move(
                pointer_id,
                request_id,
                point,
                model_position,
            )
        }
    }

    #[derive(Clone, Copy)]
    struct PanGesture {
        pointer_id: i32,
        origin: geosolve_constraint_editor::ScreenPoint,
        origin_center: [f64; 2],
    }

    pub(crate) fn install(document: &Document) -> Result<(), JsValue> {
        let storage = super::platform::window()?.local_storage().ok().flatten();
        let snapshot = storage.as_ref().and_then(|storage| {
            storage
                .get_item(STORAGE_KEY)
                .ok()
                .flatten()
                .or_else(|| storage.get_item(PREVIOUS_STORAGE_KEY).ok().flatten())
                .or_else(|| storage.get_item(OLDER_STORAGE_KEY).ok().flatten())
                .or_else(|| storage.get_item(LEGACY_STORAGE_KEY).ok().flatten())
        });
        let restored = if let Some(snapshot) = snapshot.as_deref() {
            WorkspaceSnapshot::decode(snapshot).and_then(|value| coordinator_from_snapshot(&value))
        } else {
            empty_coordinator()
        };
        let (coordinator, notice) = match restored {
            Ok(value) => (value, "Ready".to_owned()),
            Err(error) => (
                empty_coordinator().map_err(|error| JsValue::from_str(&error))?,
                format!("Stored workbench could not be restored: {error}"),
            ),
        };
        let workbench = Rc::new(RefCell::new(Workbench {
            coordinator,
            authoring: AuthoringState::default(),
            feature_authoring: FeatureAuthoringState::default(),
            feature_candidate: None,
            feature_pending: Vec::new(),
            samples: super::samples::SampleCatalogState::default(),
            camera: super::scene::CanvasCamera::default(),
            pan_gesture: None,
            pointer_captures: super::CanvasPointerCaptures::default(),
            pointer_moves: Rc::new(RefCell::new(super::PointerMoveQueue::default())),
            fillet_action_render: super::FilletActionRenderAuthority::default(),
            feature_options_open: false,
            construction_preview: None,
            inference_preview: None,
            notice,
            problems_open: false,
        }));
        install_palette_icons(document)?;
        render(document, &workbench)?;
        install_clicks(document, &workbench)?;
        install_sample_flyout_state(document)?;
        install_canvas(document, &workbench)?;
        install_keyboard(document, &workbench)?;
        install_fillet_options_overlay_reposition(document)?;
        Ok(())
    }

    fn install_palette_icons(document: &Document) -> Result<(), JsValue> {
        for (key, tool) in super::icons::GEOMETRY_TOOLS {
            let Some(button) = document.query_selector(&format!("[data-wb-tool=\"{key}\"]"))?
            else {
                continue;
            };
            let Some(icon) = button.query_selector(".wb-geometry-icon")? else {
                continue;
            };
            icon.set_inner_html(&super::icons::geometry_tool_icon_markup(tool));
        }
        for (key, _, intent) in super::action_surface::CONSTRAINT_ACTIONS {
            install_authoring_icon(document, key, AuthoringTool::Constraint(intent))?;
        }
        for (key, _, kind) in super::action_surface::DIMENSION_ACTIONS {
            install_authoring_icon(document, key, AuthoringTool::Dimension(kind))?;
        }
        for (key, _, tool) in super::action_surface::FEATURE_ACTIONS {
            let Some(button) = document.query_selector(&format!("[data-wb-feature=\"{key}\"]"))?
            else {
                continue;
            };
            let Some(icon) = button.query_selector(".wb-feature-icon")? else {
                continue;
            };
            icon.set_inner_html(&super::icons::feature_icon_markup(tool));
        }
        Ok(())
    }

    fn install_authoring_icon(
        document: &Document,
        key: &str,
        tool: AuthoringTool,
    ) -> Result<(), JsValue> {
        let Some(button) = document.query_selector(&format!("[data-wb-authoring=\"{key}\"]"))?
        else {
            return Ok(());
        };
        let Some(icon) = button.query_selector(".wb-authoring-icon")? else {
            return Ok(());
        };
        icon.set_inner_html(&super::icons::authoring_icon_markup(tool));
        Ok(())
    }

    fn install_sample_flyout_state(document: &Document) -> Result<(), JsValue> {
        let menu = required(document, "wb-sample-menu")?;
        for name in ["pointerover", "focusin"] {
            let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let Some(branch) = sample_branch(&event) else {
                    return;
                };
                set_branch_expanded(&branch, true);
            });
            menu.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
            callback.forget();
        }
        for name in ["pointerout", "focusout"] {
            let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let Some(branch) = sample_branch(&event) else {
                    return;
                };
                let still_open = branch.matches(":hover").unwrap_or(false)
                    || branch.matches(":focus-within").unwrap_or(false);
                if !still_open {
                    set_branch_expanded(&branch, false);
                }
            });
            menu.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
            callback.forget();
        }
        Ok(())
    }

    fn install_fillet_options_overlay_reposition(document: &Document) -> Result<(), JsValue> {
        let resize_document = document.clone();
        let resize = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
            let _ = reposition_fillet_options_overlay(&resize_document);
        });
        super::platform::window()?
            .add_event_listener_with_callback("resize", resize.as_ref().unchecked_ref())?;
        resize.forget();

        let scroll_document = document.clone();
        let scroll = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
            let _ = reposition_fillet_options_overlay(&scroll_document);
        });
        required(document, "wb-tool-palette")?
            .add_event_listener_with_callback("scroll", scroll.as_ref().unchecked_ref())?;
        scroll.forget();

        let toggle_document = document.clone();
        let toggle = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
            let _ = reposition_fillet_options_overlay(&toggle_document);
        });
        let (toggle_event, capture) = super::palette_details_overlay_reflow_listener();
        required(document, "wb-tool-palette")?.add_event_listener_with_callback_and_bool(
            toggle_event,
            toggle.as_ref().unchecked_ref(),
            capture,
        )?;
        toggle.forget();
        Ok(())
    }

    fn sample_branch(event: &Event) -> Option<Element> {
        event
            .target()?
            .dyn_into::<Element>()
            .ok()?
            .closest(".wb-sample-branch")
            .ok()
            .flatten()
    }

    fn set_branch_expanded(branch: &Element, expanded: bool) {
        if let Ok(Some(trigger)) = branch.query_selector("[data-sample-group-trigger]") {
            let _ = trigger.set_attribute("aria-expanded", if expanded { "true" } else { "false" });
        }
    }

    fn empty_coordinator() -> Result<RetainedEditorCoordinator, String> {
        coordinator_from_document(SketchDocument::new(10.0).map_err(|error| error.to_string())?)
    }

    fn coordinator_from_document(
        document: SketchDocument,
    ) -> Result<RetainedEditorCoordinator, String> {
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .map_err(|error| error.to_string())?;
        RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())
    }

    fn coordinator_from_snapshot(
        snapshot: &WorkspaceSnapshot,
    ) -> Result<RetainedEditorCoordinator, String> {
        let session =
            snapshot.restore_session(DocumentSolveRequest::default(), SolverConfig::default())?;
        let features = snapshot.feature_document()?;
        RetainedEditorCoordinator::with_features_and_high_water(
            session,
            features,
            snapshot.feature_lifecycle_high_water(),
            snapshot.computed_evaluation_high_water(),
        )
        .map_err(|error| error.to_string())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one delegated root listener keeps click, change and Fillet focus routing ordered"
    )]
    fn install_clicks(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Some(origin) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            if origin
                .closest("[data-problem-marker]")
                .is_ok_and(|marker| marker.is_some())
            {
                return;
            }
            if super::change_owns_option_control_click(
                &origin.tag_name(),
                origin
                    .closest(".wb-palette-flyout, .wb-feature-options")
                    .is_ok_and(|surface| surface.is_some()),
                origin
                    .closest(".wb-construction-options")
                    .is_ok_and(|surface| surface.is_some()),
                origin
                    .closest(".wb-branch-editor")
                    .is_ok_and(|surface| surface.is_some()),
            ) {
                // The later `change` event owns reading the browser-updated value
                // and rendering. Rendering during this bubbled click would restore
                // the old headless value before checkbox/select activation finishes.
                return;
            }
            let target = origin
                .closest(concat!(
                    "[data-wb-tool], [data-wb-authoring], [data-wb-feature], ",
                    "[data-fillet-action], [data-editor-item], [data-wb-action], [data-sample-id], ",
                    "[data-sample-group-trigger]"
                ))
                .ok()
                .flatten()
                .unwrap_or(origin);
            let mut selected_sample = false;
            if target.has_attribute("data-sample-group-trigger") {
                return;
            } else if let Some(tool) = target
                .get_attribute("data-wb-tool")
                .and_then(|key| tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                if let Err(error) =
                    update_construction_options(&callback_document, wb.coordinator.editor_mut())
                {
                    wb.notice = error;
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                    return;
                }
                wb.authoring.deactivate();
                clear_feature_authoring(&mut wb);
                let effects = wb.coordinator.editor_mut().activate_tool(tool);
                dispatch_effects(&mut wb, effects);
                wb.notice = format!("{} tool active", super::icons::geometry_tool_key(tool));
            } else if let Some(tool) = target
                .get_attribute("data-wb-authoring")
                .and_then(|key| super::action_surface::authoring_tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                clear_feature_authoring(&mut wb);
                activate_authoring(&callback_document, &mut wb, tool);
            } else if let Some(tool) = target
                .get_attribute("data-wb-feature")
                .and_then(|key| super::action_surface::feature_tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                activate_feature_authoring(&callback_document, &mut wb, tool);
            } else if target.has_attribute("data-fillet-action") {
                let mut wb = callback_workbench.borrow_mut();
                let Some(scene) = editor_scene(&wb) else {
                    wb.notice = "Fillet action requires current computed geometry".into();
                    return;
                };
                let input = if target.get_attribute("data-fillet-action-input").as_deref()
                    == Some("canvas")
                {
                    let Some(mouse) = event.dyn_ref::<MouseEvent>() else {
                        return;
                    };
                    let Ok(viewport) = required(&callback_document, "wb-viewport") else {
                        return;
                    };
                    let Some(position) = client_screen_point(
                        &viewport,
                        scene.viewport,
                        f64::from(mouse.client_x()),
                        f64::from(mouse.client_y()),
                    ) else {
                        return;
                    };
                    let Some(painted) = resolve_canvas_fillet_action_at_point(
                        &callback_document,
                        &scene,
                        &wb.fillet_action_render,
                        position,
                        mouse.client_x(),
                        mouse.client_y(),
                    ) else {
                        // Pointer-down already routed this position through the
                        // ordinary editor because the painted action was stale,
                        // spoofed or outside its headless control geometry.
                        return;
                    };
                    SceneFilletActionInput::Canvas {
                        position,
                        painted: Some(painted),
                    }
                } else {
                    let Some(painted) =
                        fillet_action_target(&scene, &target, &wb.fillet_action_render)
                    else {
                        wb.notice = "Fillet action is stale or unavailable".into();
                        return;
                    };
                    SceneFilletActionInput::Accessible(painted)
                };
                let effects = wb
                    .coordinator
                    .editor_mut()
                    .activate_fillet_action(&scene, input);
                if effects.is_empty() {
                    wb.notice = "Preview this Fillet branch choice before activating it".into();
                } else {
                    dispatch_effects(&mut wb, effects);
                }
            } else if target.has_attribute("data-editor-item") {
                if let Some(item) = selection_item(&target) {
                    let is_canvas_item = target
                        .closest("#wb-viewport")
                        .is_ok_and(|viewport| viewport.is_some());
                    let is_pointer_click = event
                        .dyn_ref::<MouseEvent>()
                        .is_some_and(|event| event.detail() > 0);
                    let modifiers = event
                        .dyn_ref::<MouseEvent>()
                        .map(|event| Modifiers {
                            shift: event.shift_key(),
                            control: event.ctrl_key(),
                            command: event.meta_key(),
                        })
                        .unwrap_or_default();
                    let mut wb = callback_workbench.borrow_mut();
                    if wb.feature_authoring.active_tool().is_some() {
                        let input = if is_canvas_item {
                            super::AuthoringItemInput::CanvasClick
                        } else {
                            super::AuthoringItemInput::TreeClick
                        };
                        if super::owns_authoring_pick(input) {
                            handle_feature_item_pick(&mut wb, item, None);
                        }
                    } else if wb.authoring.active_tool().is_some() {
                        let input = if is_canvas_item {
                            super::AuthoringItemInput::CanvasClick
                        } else {
                            super::AuthoringItemInput::TreeClick
                        };
                        if super::owns_authoring_pick(input) {
                            let document = wb.coordinator.session().design_document().clone();
                            let outcome = wb
                                .authoring
                                .pick(&document, AuthoringOperand::selected(item));
                            handle_authoring_outcome(&mut wb, outcome);
                        }
                    } else if !is_canvas_item || !is_pointer_click {
                        wb.coordinator.editor_mut().select_item(item, modifiers);
                    }
                }
            } else if let Some(key) = target.get_attribute("data-sample-id") {
                let mut wb = callback_workbench.borrow_mut();
                selected_sample = open_sample(&callback_document, &mut wb, &key);
            } else if let Some(action) = target.get_attribute("data-wb-action") {
                perform_action(
                    &callback_document,
                    &mut callback_workbench.borrow_mut(),
                    &action,
                );
            }
            save(&callback_workbench.borrow());
            let _ = render(&callback_document, &callback_workbench);
            if selected_sample {
                close_sample_selector(&callback_document);
                focus_by_id(&callback_document, "wb-sample-trigger");
            }
        });
        required(document, "workbench-root")?
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
        callback.forget();
        let change_document = document.clone();
        let change_workbench = Rc::clone(workbench);
        let change = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            {
                if target.closest(".wb-branch-editor").ok().flatten().is_some() {
                    return;
                }
                if target
                    .closest(".wb-feature-options")
                    .ok()
                    .flatten()
                    .is_some()
                {
                    let mut wb = change_workbench.borrow_mut();
                    update_feature_options(&change_document, &mut wb);
                    drop(wb);
                } else if target
                    .closest(".wb-construction-options")
                    .ok()
                    .flatten()
                    .is_some()
                {
                    let mut wb = change_workbench.borrow_mut();
                    let result =
                        update_construction_options(&change_document, wb.coordinator.editor_mut());
                    wb.notice = result.map_or_else(
                        |error| error,
                        |()| "Advanced construction options updated".into(),
                    );
                    drop(wb);
                } else if target
                    .closest(".wb-palette-flyout")
                    .ok()
                    .flatten()
                    .is_some()
                {
                    let mut wb = change_workbench.borrow_mut();
                    match authoring_options(&change_document) {
                        Ok(options) => {
                            wb.authoring.set_options(options);
                            wb.notice = "Authoring options updated".into();
                        }
                        Err(error) => wb.notice = error,
                    }
                    drop(wb);
                }
            }
            let _ = render(&change_document, &change_workbench);
        });
        required(document, "workbench-root")?
            .add_event_listener_with_callback("change", change.as_ref().unchecked_ref())?;
        change.forget();
        install_fillet_action_focus(document, workbench)
    }

    fn install_fillet_action_focus(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
    ) -> Result<(), JsValue> {
        let root = required(document, "workbench-root")?;
        let focus_document = document.clone();
        let focus_workbench = Rc::clone(workbench);
        let focus_in = Closure::<dyn FnMut(FocusEvent)>::new(move |event: FocusEvent| {
            let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-fillet-action]").ok().flatten())
                .filter(|target| {
                    target.get_attribute("data-fillet-action-input").as_deref()
                        == Some("accessible")
                })
            else {
                return;
            };
            let mut wb = focus_workbench.borrow_mut();
            wb.pointer_moves
                .borrow_mut()
                .invalidate_before_immediate_action();
            let Some(scene) = editor_scene(&wb) else {
                return;
            };
            let Some(target) = fillet_action_target(&scene, &target, &wb.fillet_action_render)
            else {
                return;
            };
            let effects = wb
                .coordinator
                .editor_mut()
                .preview_fillet_action(&scene, SceneFilletActionInput::Accessible(target));
            if effects.is_empty() {
                return;
            }
            dispatch_effects(&mut wb, effects);
            drop(wb);
            let _ = render(&focus_document, &focus_workbench);
        });
        root.add_event_listener_with_callback("focusin", focus_in.as_ref().unchecked_ref())?;
        focus_in.forget();

        let blur_document = document.clone();
        let blur_workbench = Rc::clone(workbench);
        let focus_out = Closure::<dyn FnMut(FocusEvent)>::new(move |event: FocusEvent| {
            let leaving_action = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-fillet-action]").ok().flatten())
                .is_some_and(|target| {
                    target.get_attribute("data-fillet-action-input").as_deref()
                        == Some("accessible")
                });
            let enters_action = event
                .related_target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-fillet-action]").ok().flatten())
                .is_some_and(|target| {
                    target.get_attribute("data-fillet-action-input").as_deref()
                        == Some("accessible")
                });
            if !leaving_action || enters_action {
                return;
            }
            let mut wb = blur_workbench.borrow_mut();
            let effects = wb.coordinator.editor_mut().clear_fillet_branch_preview();
            if effects.is_empty() {
                return;
            }
            dispatch_effects(&mut wb, effects);
            drop(wb);
            let _ = render(&blur_document, &blur_workbench);
        });
        root.add_event_listener_with_callback("focusout", focus_out.as_ref().unchecked_ref())?;
        focus_out.forget();
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one canvas installer keeps browser pointer ownership and terminal listeners together"
    )]
    fn install_canvas(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
    ) -> Result<(), JsValue> {
        let viewport = required(document, "wb-viewport")?;
        let pointer_moves = Rc::clone(&workbench.borrow().pointer_moves);
        install_canvas_browser_default_guards(&viewport)?;
        install_pan_listeners(document, workbench, &viewport)?;
        install_pointer_listener(
            document,
            workbench,
            &viewport,
            "pointerdown",
            |coordinator, scene, input, problem_items| {
                coordinator.pointer_down_with_problem_items(scene, input, problem_items)
            },
        )?;
        install_pointer_move_listener(document, workbench, &viewport, &pointer_moves)?;
        install_pointer_up_listener(document, workbench, &viewport, &pointer_moves)?;

        let cancel_document = document.clone();
        let cancel_workbench = Rc::clone(workbench);
        let cancel_viewport = viewport.clone();
        let cancel = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let mut wb = cancel_workbench.borrow_mut();
            match wb.pointer_captures.ownership(event.pointer_id()) {
                super::CanvasPointerOwnership::Owned => {
                    let canceled = cancel_captured_canvas_interactions(
                        &cancel_viewport,
                        &mut wb,
                        super::CanvasPointerTerminal::PointerCancel {
                            pointer_id: event.pointer_id(),
                        },
                        "Interaction canceled",
                    );
                    debug_assert!(canceled);
                }
                super::CanvasPointerOwnership::Foreign => return,
                super::CanvasPointerOwnership::Uncaptured => {
                    wb.pointer_moves.borrow_mut().drain_before_terminal();
                    let effects = wb.coordinator.editor_mut().cancel();
                    dispatch_effects(&mut wb, effects);
                    wb.notice = "Interaction canceled".into();
                }
            }
            drop(wb);
            let _ = render(&cancel_document, &cancel_workbench);
        });
        viewport.add_event_listener_with_callback(
            super::CANVAS_POINTER_TERMINAL_EVENTS[1],
            cancel.as_ref().unchecked_ref(),
        )?;
        cancel.forget();

        let lost_document = document.clone();
        let lost_workbench = Rc::clone(workbench);
        let lost_viewport = viewport.clone();
        let lost = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let mut wb = lost_workbench.borrow_mut();
            if !cancel_captured_canvas_interactions(
                &lost_viewport,
                &mut wb,
                super::CanvasPointerTerminal::LostPointerCapture {
                    pointer_id: event.pointer_id(),
                },
                "Interaction canceled because pointer capture was lost",
            ) {
                return;
            }
            drop(wb);
            let _ = render(&lost_document, &lost_workbench);
        });
        viewport.add_event_listener_with_callback(
            super::CANVAS_POINTER_TERMINAL_EVENTS[2],
            lost.as_ref().unchecked_ref(),
        )?;
        lost.forget();

        let leave_document = document.clone();
        let leave_workbench = Rc::clone(workbench);
        let leave = Closure::<dyn FnMut(PointerEvent)>::new(move |_event| {
            let mut wb = leave_workbench.borrow_mut();
            let effects = wb.coordinator.editor_mut().pointer_leave();
            if effects.is_empty() {
                return;
            }
            dispatch_effects(&mut wb, effects);
            drop(wb);
            let _ = render(&leave_document, &leave_workbench);
        });
        viewport
            .add_event_listener_with_callback("pointerleave", leave.as_ref().unchecked_ref())?;
        leave.forget();

        let double_document = document.clone();
        let double_workbench = Rc::clone(workbench);
        let double = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            if event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-problem-marker]").ok().flatten())
                .is_some()
            {
                return;
            }
            event.prevent_default();
            let mut wb = double_workbench.borrow_mut();
            let expected = wb.coordinator.session().design_identity();
            let effects = wb.coordinator.editor_mut().complete_draft(expected);
            dispatch_effects(&mut wb, effects);
            save(&wb);
            drop(wb);
            let _ = render(&double_document, &double_workbench);
        });
        viewport.add_event_listener_with_callback("dblclick", double.as_ref().unchecked_ref())?;
        double.forget();
        install_wheel_zoom(document, workbench, &viewport)?;
        Ok(())
    }

    fn begin_canvas_pointer_capture(
        viewport: &Element,
        wb: &mut Workbench,
        pointer_id: i32,
        kind: super::CanvasPointerCaptureKind,
    ) -> bool {
        if wb.pointer_captures.contains(pointer_id) {
            return true;
        }
        if pointer_id < 0
            || !wb.pointer_captures.is_empty()
            || viewport.set_pointer_capture(pointer_id).is_err()
        {
            return false;
        }
        wb.pointer_captures
            .begin(super::CapturedCanvasPointer { pointer_id, kind })
    }

    fn capture_active_editor_pointer(
        viewport: &Element,
        wb: &mut Workbench,
        pointer_id: i32,
    ) -> Result<bool, ()> {
        let Ok(normalized_pointer_id) = u64::try_from(pointer_id) else {
            return Err(());
        };
        let Some(active) = wb.coordinator.editor().active_pointer_gesture() else {
            return Ok(false);
        };
        if active.pointer_id != normalized_pointer_id {
            return Ok(false);
        }
        begin_canvas_pointer_capture(
            viewport,
            wb,
            pointer_id,
            super::canvas_pointer_capture_kind(active.kind),
        )
        .then_some(true)
        .ok_or(())
    }

    fn release_canvas_pointer_capture(
        viewport: &Element,
        wb: &mut Workbench,
        pointer_id: i32,
    ) -> Option<super::CapturedCanvasPointer> {
        let route = wb
            .pointer_captures
            .route_terminal(super::CanvasPointerTerminal::PointerUp { pointer_id })?;
        debug_assert_eq!(
            route.disposition,
            super::CanvasPointerTerminalDisposition::Complete
        );
        release_routed_canvas_pointer_capture(viewport, route);
        Some(route.captured)
    }

    fn cancel_captured_canvas_interactions(
        viewport: &Element,
        wb: &mut Workbench,
        terminal: super::CanvasPointerTerminal,
        notice: &str,
    ) -> bool {
        let Some(route) = cancel_captured_canvas_state(wb, terminal, notice) else {
            return false;
        };
        release_routed_canvas_pointer_capture(viewport, route);
        true
    }

    fn cancel_captured_canvas_state(
        wb: &mut Workbench,
        terminal: super::CanvasPointerTerminal,
        notice: &str,
    ) -> Option<super::CanvasPointerTerminalRoute> {
        let route = wb.pointer_captures.route_terminal(terminal)?;
        debug_assert_eq!(
            route.disposition,
            super::CanvasPointerTerminalDisposition::Cancel
        );
        wb.pointer_moves.borrow_mut().drain_before_terminal();
        wb.pan_gesture = None;
        let effects = wb.coordinator.editor_mut().cancel();
        dispatch_effects(wb, effects);
        wb.notice = notice.into();
        Some(route)
    }

    fn release_routed_canvas_pointer_capture(
        viewport: &Element,
        route: super::CanvasPointerTerminalRoute,
    ) {
        if route.release_platform_capture && viewport.has_pointer_capture(route.captured.pointer_id)
        {
            let _ = viewport.release_pointer_capture(route.captured.pointer_id);
        }
    }

    fn install_canvas_browser_default_guards(viewport: &Element) -> Result<(), JsValue> {
        for name in super::CANVAS_BROWSER_DEFAULT_GUARD_EVENTS {
            let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                event.prevent_default();
            });
            viewport.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
            callback.forget();
        }
        Ok(())
    }

    fn install_pointer_move_listener(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
        viewport: &Element,
        pointer_moves: &Rc<RefCell<super::PointerMoveQueue>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback_viewport = viewport.clone();
        let callback_pointer_moves = Rc::clone(pointer_moves);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            if event_targets_problem_marker(&event) {
                return;
            }
            if pointer_event_fillet_action(&event).is_some() {
                let mut wb = callback_workbench.borrow_mut();
                if wb.pointer_captures.is_empty() {
                    let Some(scene) = editor_scene(&wb) else {
                        return;
                    };
                    let Some(pointer) = pointer_input(&callback_viewport, scene.viewport, &event)
                    else {
                        return;
                    };
                    let painted = resolve_canvas_fillet_action_at_point(
                        &callback_document,
                        &scene,
                        &wb.fillet_action_render,
                        pointer.position,
                        event.client_x(),
                        event.client_y(),
                    );
                    if let Some(target) = painted {
                        callback_pointer_moves
                            .borrow_mut()
                            .invalidate_before_immediate_action();
                        let effects = wb.coordinator.editor_mut().preview_fillet_action(
                            &scene,
                            SceneFilletActionInput::Canvas {
                                position: pointer.position,
                                painted: Some(target),
                            },
                        );
                        if !effects.is_empty() {
                            dispatch_effects(&mut wb, effects);
                            drop(wb);
                            let _ = render(&callback_document, &callback_workbench);
                        }
                        return;
                    }
                } else if !wb.pointer_captures.contains(event.pointer_id()) {
                    return;
                }
            }
            let input = {
                let wb = callback_workbench.borrow();
                if wb.pan_gesture.is_some() || wb.authoring.active_tool().is_some() {
                    return;
                }
                if !wb.pointer_captures.is_empty()
                    && !wb.pointer_captures.contains(event.pointer_id())
                {
                    return;
                }
                let Some(scene) = editor_scene(&wb) else {
                    return;
                };
                let Some(input) = pointer_input(&callback_viewport, scene.viewport, &event) else {
                    return;
                };
                input
            };
            let Some(generation) = callback_pointer_moves.borrow_mut().push(input) else {
                return;
            };
            let frame_document = callback_document.clone();
            let frame_workbench = Rc::clone(&callback_workbench);
            let frame_pointer_moves = Rc::clone(&callback_pointer_moves);
            let frame = Closure::once_into_js(move || {
                let Some(input) = frame_pointer_moves.borrow_mut().take_for_frame(generation)
                else {
                    return;
                };
                let mut wb = frame_workbench.borrow_mut();
                if wb.pan_gesture.is_some() || wb.authoring.active_tool().is_some() {
                    return;
                }
                let Some(scene) = editor_scene(&wb) else {
                    return;
                };
                let effects = wb.coordinator.editor_mut().pointer_move(&scene, input);
                dispatch_effects(&mut wb, effects);
                save(&wb);
                drop(wb);
                let _ = render(&frame_document, &frame_workbench);
            });
            let scheduled = super::platform::window()
                .and_then(|window| window.request_animation_frame(frame.unchecked_ref()));
            if scheduled.is_err() {
                callback_pointer_moves.borrow_mut().cancel_frame(generation);
            }
        });
        viewport
            .add_event_listener_with_callback("pointermove", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_pointer_up_listener(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
        viewport: &Element,
        pointer_moves: &Rc<RefCell<super::PointerMoveQueue>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback_viewport = viewport.clone();
        let callback_pointer_moves = Rc::clone(pointer_moves);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let mut wb = callback_workbench.borrow_mut();
            let owns_pointer = wb.pointer_captures.contains(event.pointer_id());
            if !owns_pointer
                && (event_targets_problem_marker(&event)
                    || pointer_event_fillet_action(&event).is_some())
            {
                return;
            }
            if !wb.pointer_captures.is_empty() && !wb.pointer_captures.contains(event.pointer_id())
            {
                return;
            }
            if event.button() != 0 {
                return;
            }
            if wb.pan_gesture.is_some() {
                return;
            }
            let Some(scene) = editor_scene(&wb) else {
                if wb.pointer_captures.contains(event.pointer_id()) {
                    cancel_captured_canvas_interactions(
                        &callback_viewport,
                        &mut wb,
                        super::CanvasPointerTerminal::PointerCancel {
                            pointer_id: event.pointer_id(),
                        },
                        "Interaction canceled because the current canvas scene is unavailable",
                    );
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                }
                return;
            };
            let Some(input) = pointer_input(&callback_viewport, scene.viewport, &event) else {
                if wb.pointer_captures.contains(event.pointer_id()) {
                    cancel_captured_canvas_interactions(
                        &callback_viewport,
                        &mut wb,
                        super::CanvasPointerTerminal::PointerCancel {
                            pointer_id: event.pointer_id(),
                        },
                        "Interaction canceled because the terminal pointer sample is invalid",
                    );
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                }
                return;
            };
            if wb.authoring.active_tool().is_some() {
                if wb.pointer_captures.contains(event.pointer_id()) {
                    cancel_captured_canvas_interactions(
                        &callback_viewport,
                        &mut wb,
                        super::CanvasPointerTerminal::PointerCancel {
                            pointer_id: event.pointer_id(),
                        },
                        "Interaction canceled because authoring mode changed",
                    );
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                }
                return;
            }
            if let Some(pending) = callback_pointer_moves.borrow_mut().drain_before_terminal() {
                let effects = wb.coordinator.editor_mut().pointer_move(&scene, pending);
                dispatch_effects(&mut wb, effects);
            }
            let coordinator = &mut wb.coordinator;
            let expected = coordinator.session().design_identity();
            let effects = coordinator.editor_mut().pointer_up(&scene, expected, input);
            dispatch_effects(&mut wb, effects);
            release_canvas_pointer_capture(&callback_viewport, &mut wb, event.pointer_id());
            save(&wb);
            drop(wb);
            let _ = render(&callback_document, &callback_workbench);
        });
        viewport.add_event_listener_with_callback(
            super::CANVAS_POINTER_TERMINAL_EVENTS[0],
            callback.as_ref().unchecked_ref(),
        )?;
        callback.forget();
        Ok(())
    }

    fn event_targets_problem_marker(event: &PointerEvent) -> bool {
        event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
            .and_then(|target| target.closest("[data-problem-marker]").ok().flatten())
            .is_some()
    }

    fn pointer_event_fillet_action(event: &PointerEvent) -> Option<Element> {
        event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
            .and_then(|target| target.closest("[data-fillet-action]").ok().flatten())
    }

    // `PointerEvent` exposes integral CSS pixels while the generated
    // `Document::elements_from_point` binding requires `f32` CSS pixels.
    #[allow(clippy::cast_precision_loss)]
    fn resolve_canvas_fillet_action_at_point(
        document: &Document,
        scene: &EditorScene,
        authority: &super::FilletActionRenderAuthority,
        position: ScreenPoint,
        client_x: i32,
        client_y: i32,
    ) -> Option<SceneFilletActionTarget> {
        super::resolve_canvas_fillet_action_candidates(
            scene,
            position,
            document
                .elements_from_point(client_x as f32, client_y as f32)
                .iter()
                .filter_map(|value| value.dyn_into::<Element>().ok())
                .filter_map(|element| element.closest("[data-fillet-action]").ok().flatten())
                .filter_map(|element| fillet_action_target(scene, &element, authority)),
        )
    }

    /// Returns the stable identity painted directly under this pointer sample.
    /// The coordinator treats it only as an intent hint and independently
    /// validates current preview ownership, scene provenance and arc proximity.
    fn pointer_event_selection_item(event: &PointerEvent) -> Option<SelectionItem> {
        let origin = event.target()?.dyn_into::<Element>().ok()?;
        let target = origin.closest("[data-editor-item]").ok().flatten()?;
        selection_item(&target)
    }

    fn install_pan_listeners(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
        viewport: &Element,
    ) -> Result<(), JsValue> {
        for name in super::CANVAS_PAN_POINTER_EVENTS {
            let callback_document = document.clone();
            let callback_workbench = Rc::clone(workbench);
            let callback_viewport = viewport.clone();
            let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
                let mut wb = callback_workbench.borrow_mut();
                let screen = client_screen_point(
                    &callback_viewport,
                    wb.camera.viewport(),
                    f64::from(event.client_x()),
                    f64::from(event.client_y()),
                );
                match name {
                    "pointerdown" if event.button() == 1 => {
                        let Some(origin) = screen else {
                            return;
                        };
                        event.prevent_default();
                        match super::route_canvas_pan_pointer_down(&wb.pointer_captures) {
                            super::CanvasPanPointerDownRoute::BeginPan => {
                                let effects =
                                    wb.coordinator.editor_mut().clear_fillet_branch_preview();
                                dispatch_effects(&mut wb, effects);
                            }
                            super::CanvasPanPointerDownRoute::PreserveCapturedInteraction => {
                                return;
                            }
                        }
                        if !begin_canvas_pointer_capture(
                            &callback_viewport,
                            &mut wb,
                            event.pointer_id(),
                            super::CanvasPointerCaptureKind::Pan,
                        ) {
                            wb.notice = "Canvas pan canceled because pointer capture failed".into();
                            return;
                        }
                        wb.pan_gesture = Some(PanGesture {
                            pointer_id: event.pointer_id(),
                            origin,
                            origin_center: wb.camera.model_center,
                        });
                        wb.notice = "Panning canvas".into();
                    }
                    "pointermove" => {
                        let Some(gesture) = wb
                            .pan_gesture
                            .filter(|gesture| gesture.pointer_id == event.pointer_id())
                        else {
                            return;
                        };
                        let Some(current) = screen else {
                            return;
                        };
                        event.prevent_default();
                        wb.camera
                            .pan_from(gesture.origin_center, gesture.origin, current);
                        drop(wb);
                        let _ = render(&callback_document, &callback_workbench);
                    }
                    "pointerup"
                        if wb
                            .pan_gesture
                            .is_some_and(|gesture| gesture.pointer_id == event.pointer_id()) =>
                    {
                        event.prevent_default();
                        wb.pan_gesture = None;
                        release_canvas_pointer_capture(
                            &callback_viewport,
                            &mut wb,
                            event.pointer_id(),
                        );
                        wb.notice = "Canvas pan complete".into();
                        drop(wb);
                        let _ = render(&callback_document, &callback_workbench);
                    }
                    _ => {}
                }
            });
            viewport.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
            callback.forget();
        }
        Ok(())
    }

    fn install_wheel_zoom(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
        viewport: &Element,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback_viewport = viewport.clone();
        let callback = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            let mut wb = callback_workbench.borrow_mut();
            if wb.pointer_captures.is_empty() {
                let effects = wb.coordinator.editor_mut().clear_fillet_branch_preview();
                dispatch_effects(&mut wb, effects);
            } else {
                cancel_captured_canvas_interactions(
                    &callback_viewport,
                    &mut wb,
                    super::CanvasPointerTerminal::CameraCancel,
                    "Active drag canceled before canvas zoom",
                );
            }
            let Some(anchor) = client_screen_point(
                &callback_viewport,
                wb.camera.viewport(),
                f64::from(event.client_x()),
                f64::from(event.client_y()),
            ) else {
                return;
            };
            event.prevent_default();
            let factor = (-event.delta_y() * 0.0015).exp();
            if wb.camera.zoom_about(anchor, factor) {
                wb.notice = format!(
                    "Canvas zoom {:.1} px / unit",
                    wb.camera.pixels_per_model_unit
                );
            }
            drop(wb);
            let _ = render(&callback_document, &callback_workbench);
        });
        viewport.add_event_listener_with_callback("wheel", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the pointer-down adapter preserves one explicit priority and capture sequence"
    )]
    fn install_pointer_listener(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
        viewport: &Element,
        name: &str,
        transition: fn(
            &mut RetainedEditorCoordinator,
            &EditorScene,
            PointerInput,
            &[SelectionItem],
        ) -> Vec<EditorEffect>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback_viewport = viewport.clone();
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            if event_targets_problem_marker(&event) {
                return;
            }
            let painted_action = pointer_event_fillet_action(&event);
            let mut wb = callback_workbench.borrow_mut();
            if wb.pan_gesture.is_some() {
                return;
            }
            if event.button() != 0 {
                return;
            }
            if wb.pointer_captures.is_empty() {
                wb.pointer_moves.borrow_mut().drain_before_terminal();
            }
            let Some(scene) = editor_scene(&wb) else {
                return;
            };
            let Some(input) = pointer_input(&callback_viewport, scene.viewport, &event) else {
                return;
            };
            if wb.pointer_captures.is_empty() && painted_action.is_some() {
                let painted = resolve_canvas_fillet_action_at_point(
                    &callback_document,
                    &scene,
                    &wb.fillet_action_render,
                    input.position,
                    event.client_x(),
                    event.client_y(),
                );
                if painted.is_some() {
                    return;
                }
            }
            if wb.feature_authoring.active_tool().is_some() {
                if let Some(outcome) = feature_canvas_pointer_down(
                    &mut wb,
                    &scene,
                    input,
                    pointer_event_selection_item(&event),
                ) {
                    match outcome {
                        FeatureAuthoringPointerDownOutcome::RadiusGesture { effects } => {
                            dispatch_effects(&mut wb, effects);
                        }
                        FeatureAuthoringPointerDownOutcome::NativePick { transaction } => {
                            if matches!(
                                &transaction.outcome,
                                FeatureAuthoringOutcome::NoNativeHit(_)
                            ) {
                                wb.notice =
                                    "Pick a native span or an unambiguous polyline corner".into();
                            } else {
                                handle_feature_transaction(&mut wb, *transaction);
                            }
                        }
                    }
                }
                if capture_active_editor_pointer(&callback_viewport, &mut wb, event.pointer_id())
                    .is_err()
                {
                    let effects = wb.coordinator.editor_mut().cancel();
                    dispatch_effects(&mut wb, effects);
                    wb.notice = "Canvas interaction canceled because pointer capture failed".into();
                }
                save(&wb);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if wb.authoring.active_tool().is_some() {
                if super::owns_authoring_pick(super::AuthoringItemInput::CanvasPointerDown)
                    && let Some(hit) =
                        scene.native_authoring_hit_test(input.position, PickTolerance::default())
                {
                    let document = wb.coordinator.session().design_document().clone();
                    let outcome = wb.authoring.pick(
                        &document,
                        AuthoringOperand::picked(hit.item, hit.curve_parameter),
                    );
                    handle_authoring_outcome(&mut wb, outcome);
                    save(&wb);
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                }
                return;
            }
            let problem_items = wb
                .coordinator
                .current_problem_metadata()
                .map(|problem| {
                    problem
                        .targets
                        .iter()
                        .filter_map(|target| {
                            super::scene::problem_selection_item(*target, Some(&scene))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let effects = { transition(&mut wb.coordinator, &scene, input, &problem_items) };
            dispatch_effects(&mut wb, effects);
            if capture_active_editor_pointer(&callback_viewport, &mut wb, event.pointer_id())
                .is_err()
            {
                let effects = wb.coordinator.editor_mut().cancel();
                dispatch_effects(&mut wb, effects);
                wb.notice = "Canvas interaction canceled because pointer capture failed".into();
            }
            save(&wb);
            drop(wb);
            let _ = render(&callback_document, &callback_workbench);
        });
        viewport.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "keyboard precedence is intentionally visible in one delegated event route"
    )]
    fn install_keyboard(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if event.key() == "Escape" && !callback_workbench.borrow().pointer_captures.is_empty() {
                event.prevent_default();
                let mut wb = callback_workbench.borrow_mut();
                if let Ok(viewport) = required(&callback_document, "wb-viewport") {
                    cancel_captured_canvas_interactions(
                        &viewport,
                        &mut wb,
                        super::CanvasPointerTerminal::InteractionCancel,
                        "Interaction canceled",
                    );
                } else {
                    let _ = cancel_captured_canvas_state(
                        &mut wb,
                        super::CanvasPointerTerminal::InteractionCancel,
                        "Interaction canceled because the canvas is unavailable",
                    );
                }
                save(&wb);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if event.key() == "Escape"
                && required(&callback_document, "wb-sample-selector")
                    .is_ok_and(|selector| selector.has_attribute("open"))
            {
                event.prevent_default();
                close_sample_selector(&callback_document);
                focus_by_id(&callback_document, "wb-sample-trigger");
                return;
            }
            if event.key() == "Escape" && callback_workbench.borrow().feature_options_open {
                event.prevent_default();
                callback_workbench.borrow_mut().feature_options_open = false;
                let _ = render(&callback_document, &callback_workbench);
                focus_by_id(&callback_document, "wb-feature-fillet-options-trigger");
                return;
            }
            if let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                && target
                    .closest("[data-problem-marker]")
                    .is_ok_and(|marker| marker.is_some())
            {
                return;
            }
            if matches!(event.key().as_str(), "Enter" | " ")
                && let Some(target) = event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                    .and_then(|target| target.closest("[data-fillet-action]").ok().flatten())
            {
                event.prevent_default();
                let mut wb = callback_workbench.borrow_mut();
                let Some(scene) = editor_scene(&wb) else {
                    return;
                };
                let Some(target) = fillet_action_target(&scene, &target, &wb.fillet_action_render)
                else {
                    wb.notice = "Fillet action is stale or unavailable".into();
                    return;
                };
                let effects = wb
                    .coordinator
                    .editor_mut()
                    .activate_fillet_action(&scene, SceneFilletActionInput::Accessible(target));
                if effects.is_empty() {
                    wb.notice = "Focus this Fillet branch choice before activating it".into();
                } else {
                    dispatch_effects(&mut wb, effects);
                }
                save(&wb);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if matches!(event.key().as_str(), "Enter" | " ")
                && let Some(target) = event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                && target.has_attribute("data-editor-item")
                && target
                    .closest("#wb-viewport")
                    .is_ok_and(|viewport| viewport.is_some())
                && let Some(item) = selection_item(&target)
            {
                event.prevent_default();
                let modifiers = Modifiers {
                    shift: event.shift_key(),
                    control: event.ctrl_key(),
                    command: event.meta_key(),
                };
                let mut wb = callback_workbench.borrow_mut();
                if wb.feature_authoring.active_tool().is_some() {
                    handle_feature_item_pick(&mut wb, item, None);
                } else {
                    wb.coordinator.editor_mut().select_item(item, modifiers);
                }
                save(&wb);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                && matches!(
                    target.tag_name().as_str(),
                    "A" | "BUTTON" | "INPUT" | "SELECT" | "SUMMARY" | "TEXTAREA"
                )
            {
                if event.key() == "Enter"
                    && target.tag_name() == "BUTTON"
                    && let Ok(button) = target.dyn_into::<HtmlElement>()
                {
                    event.prevent_default();
                    button.click();
                }
                return;
            }
            let mut wb = callback_workbench.borrow_mut();
            if matches!(event.key().as_str(), "Delete" | "Backspace") {
                event.prevent_default();
                perform_action(&callback_document, &mut wb, "delete");
                save(&wb);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if event.key() == "Escape" && wb.authoring.active_tool().is_some() {
                event.prevent_default();
                let document = wb.coordinator.session().design_document().clone();
                let outcome = wb.authoring.cancel(&document);
                handle_authoring_outcome(&mut wb, outcome);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if event.key() == "Escape" && wb.feature_authoring.active_tool().is_some() {
                event.prevent_default();
                let effects = wb.coordinator.editor_mut().cancel();
                dispatch_effects(&mut wb, effects);
                let outcome = wb.feature_authoring.cancel();
                handle_feature_outcome(&mut wb, outcome);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if event.key() == "Enter" && wb.feature_authoring.active_tool().is_some() {
                event.prevent_default();
                let outcome = wb.feature_authoring.enter();
                handle_feature_outcome(&mut wb, outcome);
                save(&wb);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            let effects = match event.key().as_str() {
                "Escape" => wb.coordinator.editor_mut().cancel(),
                "Enter" => {
                    let expected = wb.coordinator.session().design_identity();
                    wb.coordinator.editor_mut().complete_draft(expected)
                }
                _ => return,
            };
            event.prevent_default();
            dispatch_effects(&mut wb, effects);
            save(&wb);
            drop(wb);
            let _ = render(&callback_document, &callback_workbench);
        });
        document.add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive adapter keeps every typed editor effect on a single dispatch path"
    )]
    fn dispatch_effects(wb: &mut Workbench, effects: Vec<EditorEffect>) {
        use super::effect_adapter::{
            ConstructionDispatch, InferenceDispatch, dispatch_construction_effect,
            dispatch_inference_effect,
        };

        let mut pending = VecDeque::from(effects);
        let mut failed_construction_commit = false;
        while let Some(effect) = pending.pop_front() {
            match dispatch_construction_effect(
                &mut wb.construction_preview,
                &effect,
                None,
                &mut failed_construction_commit,
            ) {
                ConstructionDispatch::ApplyCommit => {
                    let result = wb.coordinator.apply_editor_effect(&effect);
                    dispatch_construction_effect(
                        &mut wb.construction_preview,
                        &effect,
                        Some(result.is_ok()),
                        &mut failed_construction_commit,
                    );
                    match result {
                        Ok(Some(_)) => wb.notice = "Edit retained".into(),
                        Ok(None) => {}
                        Err(error) => wb.notice = error.to_string(),
                    }
                    continue;
                }
                ConstructionDispatch::Handled => continue,
                ConstructionDispatch::NotConstruction => {}
            }
            match dispatch_inference_effect(&mut wb.inference_preview, &effect) {
                InferenceDispatch::ApplyCommit => {
                    match wb.coordinator.apply_editor_effect(&effect) {
                        Ok(Some(_)) => wb.notice = "Inference retained".into(),
                        Ok(None) => {}
                        Err(error) => wb.notice = error.to_string(),
                    }
                    continue;
                }
                InferenceDispatch::Handled => {
                    if let EditorEffect::PreviewInference(candidate) = &effect {
                        wb.notice = format!("Inference proposed: {}", candidate.label);
                    }
                    continue;
                }
                InferenceDispatch::NotInference => {}
            }
            match &effect {
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point,
                    model_position,
                } => {
                    let next = wb.resolve_projected_point_move(
                        *pointer_id,
                        *request_id,
                        *point,
                        *model_position,
                    );
                    pending.extend(next);
                }
                EditorEffect::PreviewPointMove { .. } => {
                    wb.notice = "Projected drag preview".into();
                }
                EditorEffect::ClearPointPreview => {
                    wb.coordinator.clear_transient();
                }
                EditorEffect::PreviewComputedFeatureRadius { radius, .. } => {
                    match apply_computed_feature_editor_effect(wb, &effect) {
                        Ok(Some(completed_corners)) => {
                            wb.notice = format!(
                                "Fillet radius preview {radius:.4} · {completed_corners} corner{} retained",
                                if completed_corners == 1 { "" } else { "s" },
                            );
                        }
                        Ok(None) => wb.notice = format!("Fillet radius preview {radius:.4}"),
                        Err(error) => {
                            wb.notice = format!(
                                "Fillet radius sample was rejected; the last valid preview is retained: {error}"
                            );
                        }
                    }
                }
                EditorEffect::CommitComputedFeatureRadius { radius, .. } => {
                    match apply_computed_feature_editor_effect(wb, &effect) {
                        Ok(_) => wb.notice = format!("Fillet radius set to {radius:.4}"),
                        Err(error) => wb.notice = error,
                    }
                }
                EditorEffect::RestoreComputedFeatureRadius { .. } => {
                    match apply_computed_feature_editor_effect(wb, &effect) {
                        Ok(Some(completed_corners)) => {
                            wb.notice = format!(
                                "Fillet radius edit canceled · {completed_corners} corner{} restored",
                                if completed_corners == 1 { "" } else { "s" },
                            );
                        }
                        Ok(None) => wb.notice = "Fillet radius edit canceled".into(),
                        Err(error) => {
                            wb.notice = format!("Fillet radius restore was rejected: {error}");
                        }
                    }
                }
                EditorEffect::ClearComputedFeaturePreview
                | EditorEffect::ClearComputedFeatureContactPreview => {
                    if let Err(error) = apply_computed_feature_editor_effect(wb, &effect) {
                        wb.notice = error;
                    }
                }
                EditorEffect::PreviewComputedFeatureContact { parameter, .. } => {
                    match apply_computed_feature_editor_effect(wb, &effect) {
                        Ok(Some(completed_corners)) => {
                            wb.notice = format!(
                                "Fillet contact preview {parameter:.4} · {completed_corners} corner{} retained",
                                if completed_corners == 1 { "" } else { "s" },
                            );
                        }
                        Ok(None) => wb.notice = format!("Fillet contact preview {parameter:.4}"),
                        Err(error) => {
                            wb.notice = format!(
                                "Fillet contact sample was rejected; the last valid preview is retained: {error}"
                            );
                        }
                    }
                }
                EditorEffect::CommitComputedFeatureContact { .. } => {
                    match apply_computed_feature_editor_effect(wb, &effect) {
                        Ok(_) => wb.notice = "Fillet contact retained".into(),
                        Err(error) => wb.notice = error,
                    }
                }
                EditorEffect::RestoreComputedFeatureContact { .. } => {
                    match apply_computed_feature_editor_effect(wb, &effect) {
                        Ok(_) => wb.notice = "Fillet contact edit canceled".into(),
                        Err(error) => {
                            wb.notice = format!("Fillet contact restore was rejected: {error}");
                        }
                    }
                }
                EditorEffect::FilletBranchPreviewChanged { target } => {
                    if target.is_some() {
                        wb.notice = "Fillet branch preview".into();
                    }
                }
                EditorEffect::CommitComputedFilletAction { .. } => {
                    match apply_computed_feature_editor_effect(wb, &effect) {
                        Ok(Some(completed_corners)) => {
                            wb.notice = format!(
                                "Fillet branch updated · {completed_corners} corner{} retained",
                                if completed_corners == 1 { "" } else { "s" },
                            );
                        }
                        Ok(None) => wb.notice = "Fillet branch action retained".into(),
                        Err(error) => wb.notice = error,
                    }
                }
                EditorEffect::SelectionChanged(_) | EditorEffect::HoverChanged(_) => {}
                EditorEffect::CommitPointMove { .. } => {
                    match wb.coordinator.apply_editor_effect(&effect) {
                        Ok(Some(_)) => wb.notice = "Edit retained".into(),
                        Ok(None) => {}
                        Err(error) => wb.notice = error.to_string(),
                    }
                }
                EditorEffect::PreviewConstruction(_)
                | EditorEffect::ClearConstructionPreview
                | EditorEffect::CommitConstruction { .. } => {
                    unreachable!("construction effects were dispatched above")
                }
                EditorEffect::PreviewInference(_)
                | EditorEffect::CommitInference(_)
                | EditorEffect::ClearInferencePreview => {
                    unreachable!("inference effects were dispatched above")
                }
            }
        }
    }

    fn apply_computed_feature_editor_effect(
        wb: &mut Workbench,
        effect: &EditorEffect,
    ) -> Result<Option<usize>, String> {
        if wb.coordinator.feature_authoring_preview().is_some() {
            wb.coordinator
                .apply_feature_authoring_editor_effect(&mut wb.feature_authoring, effect)
                .map_err(|error| error.to_string())?;
            let candidate = wb
                .coordinator
                .feature_authoring_preview()
                .map(|preview| preview.candidate().clone())
                .ok_or_else(|| {
                    "the current Fillet authoring preview disappeared during its edit".to_owned()
                })?;
            let completed_corners = candidate.corners().len();
            wb.feature_candidate = Some(candidate);
            wb.feature_pending.clear();
            Ok(Some(completed_corners))
        } else {
            wb.coordinator
                .apply_editor_effect(effect)
                .map(|_| None)
                .map_err(|error| error.to_string())
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed workbench action catalog is kept in one auditable dispatch table"
    )]
    fn perform_action(document: &Document, wb: &mut Workbench, action: &str) {
        let result = match action {
            "new" => cancel_before_camera_change(document, wb).and_then(|()| {
                let coordinator = empty_coordinator()?;
                wb.coordinator = coordinator;
                wb.authoring.deactivate();
                clear_feature_authoring(wb);
                wb.camera.reset();
                wb.feature_options_open = false;
                wb.construction_preview = None;
                wb.inference_preview = None;
                Ok(())
            }),
            "undo" => wb.coordinator.undo().map_err(|error| error.to_string()),
            "redo" => wb.coordinator.redo().map_err(|error| error.to_string()),
            "cancel" => cancel_before_camera_change(document, wb).map(|()| {
                if wb.feature_authoring.active_tool().is_some() {
                    let effects = wb.coordinator.editor_mut().cancel();
                    dispatch_effects(wb, effects);
                    let outcome = wb.feature_authoring.cancel();
                    handle_feature_outcome(wb, outcome);
                } else if wb.authoring.active_tool().is_some() {
                    let document = wb.coordinator.session().design_document().clone();
                    let outcome = wb.authoring.cancel(&document);
                    handle_authoring_outcome(wb, outcome);
                } else {
                    let effects = wb.coordinator.editor_mut().cancel();
                    dispatch_effects(wb, effects);
                }
            }),
            "feature-apply" => {
                let outcome = wb.feature_authoring.apply();
                handle_feature_outcome(wb, outcome);
                Ok(())
            }
            "finish" => {
                let expected = wb.coordinator.session().design_identity();
                let effects = wb.coordinator.editor_mut().complete_draft(expected);
                dispatch_effects(wb, effects);
                Ok(())
            }
            "clear-selection" => {
                wb.coordinator.editor_mut().set_selection([]);
                Ok(())
            }
            "delete" => delete_selection(wb),
            "feature-radius" => apply_selected_feature_radius(document, wb),
            "feature-suppression" => toggle_selected_feature_suppression(wb),
            "dimension-target" => apply_dimension_target(document, wb),
            "contact-branches" => apply_contact_branches(document, wb),
            "angle-orientation" => apply_angle_orientation(document, wb),
            "fillet-options" => {
                wb.feature_options_open = !wb.feature_options_open;
                Ok(())
            }
            "fillet-options-close" => {
                wb.feature_options_open = false;
                Ok(())
            }
            "problems" => {
                wb.problems_open = !wb.problems_open;
                Ok(())
            }
            "zoom-in" => cancel_before_camera_change(document, wb).map(|()| {
                wb.camera.zoom_about(
                    geosolve_constraint_editor::ScreenPoint { x: 500.0, y: 350.0 },
                    1.25,
                );
            }),
            "zoom-out" => cancel_before_camera_change(document, wb).map(|()| {
                wb.camera.zoom_about(
                    geosolve_constraint_editor::ScreenPoint { x: 500.0, y: 350.0 },
                    0.8,
                );
            }),
            "zoom-fit" => cancel_before_camera_change(document, wb).map(|()| {
                fit_camera(wb);
            }),
            _ => Ok(()),
        };
        if result.is_ok() && wb.authoring.active_tool().is_some() {
            let document = wb.coordinator.session().design_document().clone();
            let _ = wb.authoring.reconcile(&document);
        }
        if result.is_ok()
            && wb.feature_authoring.active_tool().is_some()
            && matches!(action, "undo" | "redo" | "delete" | "feature-radius")
        {
            clear_feature_authoring(wb);
            wb.notice = "Workspace changed; start a new Fillet batch".into();
        }
        wb.notice = result.map_or_else(
            |error| error,
            |()| match action {
                "problems"
                | "cancel"
                | "dimension-target"
                | "feature-apply"
                | "fillet-options"
                | "fillet-options-close" => wb.notice.clone(),
                _ => "Action retained".into(),
            },
        );
    }

    fn cancel_before_camera_change(document: &Document, wb: &mut Workbench) -> Result<(), String> {
        if wb.pointer_captures.is_empty() {
            let effects = wb.coordinator.editor_mut().clear_fillet_branch_preview();
            dispatch_effects(wb, effects);
            return Ok(());
        }
        let viewport = required(document, "wb-viewport")
            .map_err(|_| "canvas viewport is unavailable".to_owned())?;
        cancel_captured_canvas_interactions(
            &viewport,
            wb,
            super::CanvasPointerTerminal::CameraCancel,
            "Active drag canceled before camera change",
        );
        Ok(())
    }

    fn apply_dimension_target(document: &Document, wb: &mut Workbench) -> Result<(), String> {
        let value = input_value(document, "wb-dimension-target")
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .ok_or_else(|| "dimension target must be finite".to_owned())?;
        let metadata = wb
            .coordinator
            .selected_dimension_target_metadata()
            .ok_or_else(|| "select exactly one dimension".to_owned())?;
        let outcome = wb
            .coordinator
            .set_dimension_display_target(
                wb.coordinator.session().design_identity(),
                metadata.dimension,
                value,
            )
            .map_err(|error| error.to_string())?;
        wb.notice = if outcome.published_accepted.is_some() {
            "Dimension target updated and accepted".into()
        } else {
            "Dimension target retained, but the solve rejected; prior accepted geometry remains"
                .into()
        };
        Ok(())
    }

    fn selected_feature(wb: &Workbench) -> Option<ComputedFeatureId> {
        match wb.coordinator.editor().selection() {
            [SelectionItem::Feature(feature)] => Some(*feature),
            [SelectionItem::FeatureCorner(owner)] => Some(owner.feature),
            _ => None,
        }
    }

    fn delete_selection(wb: &mut Workbench) -> Result<(), String> {
        let selected = wb.coordinator.editor().selection().to_vec();
        let expected_features = wb.coordinator.feature_document().identity();
        match selected.as_slice() {
            [SelectionItem::Feature(feature)] => wb
                .coordinator
                .remove_computed_feature(expected_features, *feature)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            [SelectionItem::FeatureCorner(owner)] => wb
                .coordinator
                .remove_computed_corner(expected_features, *owner)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            _ => {
                let expected = wb.coordinator.session().design_identity();
                wb.coordinator
                    .delete_selected(expected)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
        }
    }

    fn apply_selected_feature_radius(
        document: &Document,
        wb: &mut Workbench,
    ) -> Result<(), String> {
        let feature =
            selected_feature(wb).ok_or_else(|| "select one Fillet set or arc".to_owned())?;
        let radius = finite_positive_input(document, "wb-feature-radius", "Fillet radius")?;
        let expected = wb.coordinator.feature_document().identity();
        wb.coordinator
            .set_computed_fillet_radius(expected, feature, radius)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn toggle_selected_feature_suppression(wb: &mut Workbench) -> Result<(), String> {
        let feature =
            selected_feature(wb).ok_or_else(|| "select one Fillet set or arc".to_owned())?;
        let suppressed = wb
            .coordinator
            .feature_document()
            .feature(feature)
            .ok_or_else(|| "selected Fillet set no longer exists".to_owned())?
            .suppressed;
        let expected = wb.coordinator.feature_document().identity();
        wb.coordinator
            .set_computed_feature_suppressed(expected, feature, !suppressed)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn open_sample(document: &Document, wb: &mut Workbench, key: &str) -> bool {
        if let Err(error) = cancel_before_camera_change(document, wb) {
            wb.notice = error;
            return false;
        }
        match wb.samples.open_key(key) {
            Ok(coordinator) => {
                wb.coordinator = coordinator;
                wb.authoring.deactivate();
                clear_feature_authoring(wb);
                wb.feature_options_open = false;
                wb.construction_preview = None;
                wb.inference_preview = None;
                fit_camera(wb);
                wb.notice = format!(
                    "{} opened as an editable workspace",
                    wb.samples.selected_title().unwrap_or("Sample")
                );
                true
            }
            Err(error) => {
                wb.notice = error;
                false
            }
        }
    }

    fn activate_authoring(document: &Document, wb: &mut Workbench, tool: AuthoringTool) {
        let options = match authoring_options(document) {
            Ok(options) => options,
            Err(error) => {
                wb.notice = error;
                return;
            }
        };
        wb.authoring.set_options(options);
        let snapshot = wb
            .coordinator
            .editor()
            .selection()
            .iter()
            .copied()
            .map(|item| {
                let parameter = match item {
                    SelectionItem::Curve(span) => {
                        wb.coordinator.editor().curve_pick_parameter(span)
                    }
                    SelectionItem::Point(_)
                    | SelectionItem::Constraint(_)
                    | SelectionItem::Dimension(_)
                    | SelectionItem::Feature(_)
                    | SelectionItem::FeatureCorner(_) => None,
                };
                AuthoringOperand::picked(item, parameter)
            })
            .collect::<Vec<_>>();
        let outcome =
            wb.authoring
                .activate(wb.coordinator.session().design_document(), tool, &snapshot);
        let effects = wb
            .coordinator
            .editor_mut()
            .activate_tool(EditorTool::Select);
        dispatch_effects(wb, effects);
        handle_authoring_outcome(wb, outcome);
    }

    fn clear_feature_authoring(wb: &mut Workbench) {
        wb.feature_authoring.deactivate();
        wb.feature_candidate = None;
        wb.feature_pending.clear();
        super::revoke_held_feature_authoring_preview(&mut wb.coordinator);
    }

    fn activate_feature_authoring(
        document: &Document,
        wb: &mut Workbench,
        tool: FeatureAuthoringTool,
    ) {
        let radius = match feature_radius_input(document) {
            Ok(radius) => radius,
            Err(error) => {
                clear_feature_authoring(wb);
                wb.notice = error;
                return;
            }
        };
        let options = FeatureAuthoringOptions {
            fillet_radius: radius,
            ..FeatureAuthoringOptions::default()
        };
        let snapshot = match wb.coordinator.feature_authoring_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                clear_feature_authoring(wb);
                wb.notice = format!("Computed features require current accepted geometry: {error}");
                return;
            }
        };
        let Some(accepted_document) = wb
            .coordinator
            .session()
            .accepted_state_for_current_input()
            .map(|accepted| accepted.document().clone())
        else {
            clear_feature_authoring(wb);
            wb.notice = "Computed features require current accepted geometry".into();
            return;
        };
        let selection = wb
            .coordinator
            .editor()
            .selection()
            .iter()
            .copied()
            .map(|item| {
                let parameter = match item {
                    SelectionItem::Curve(span) => {
                        wb.coordinator.editor().curve_pick_parameter(span)
                    }
                    SelectionItem::Point(_)
                    | SelectionItem::Constraint(_)
                    | SelectionItem::Dimension(_)
                    | SelectionItem::Feature(_)
                    | SelectionItem::FeatureCorner(_) => None,
                };
                (item, parameter)
            })
            .collect::<Vec<_>>();
        wb.authoring.deactivate();
        let effects = wb
            .coordinator
            .editor_mut()
            .activate_tool(EditorTool::Select);
        dispatch_effects(wb, effects);
        let _ = wb
            .feature_authoring
            .activate(&snapshot, &accepted_document, tool, &[]);
        let options_outcome = wb.feature_authoring.set_options(&snapshot, options);
        if matches!(options_outcome, FeatureAuthoringOutcome::Warning(_)) {
            handle_feature_outcome(wb, options_outcome);
            return;
        }
        let label = next_feature_authoring_label(wb);
        match wb.coordinator.transact_feature_authoring_pick_items(
            &mut wb.feature_authoring,
            &selection,
            label,
        ) {
            Ok(transaction) => handle_feature_transaction(wb, transaction),
            Err(error) => {
                wb.notice = format!("Fillet preview is unavailable: {error}");
            }
        }
    }

    fn feature_canvas_pointer_down(
        wb: &mut Workbench,
        scene: &EditorScene,
        input: PointerInput,
        painted_item: Option<SelectionItem>,
    ) -> Option<FeatureAuthoringPointerDownOutcome> {
        let label = next_feature_authoring_label(wb);
        match wb.coordinator.transact_feature_authoring_pointer_down(
            &mut wb.feature_authoring,
            scene,
            input,
            painted_item,
            PickTolerance::default(),
            label,
        ) {
            Ok(outcome) => Some(outcome),
            Err(error) => {
                wb.notice = format!("Fillet preview is unavailable: {error}");
                None
            }
        }
    }

    fn handle_feature_item_pick(
        wb: &mut Workbench,
        item: SelectionItem,
        curve_parameter: Option<f64>,
    ) {
        let label = next_feature_authoring_label(wb);
        match wb.coordinator.transact_feature_authoring_pick_items(
            &mut wb.feature_authoring,
            &[(item, curve_parameter)],
            label,
        ) {
            Ok(transaction) => handle_feature_transaction(wb, transaction),
            Err(error) => {
                wb.notice = format!("Fillet preview is unavailable: {error}");
            }
        }
    }

    fn update_feature_options(document: &Document, wb: &mut Workbench) {
        let radius = match feature_radius_input(document) {
            Ok(radius) => radius,
            Err(error) => {
                wb.notice = error;
                return;
            }
        };
        let label = next_feature_authoring_label(wb);
        match wb.coordinator.transact_feature_authoring_radius(
            &mut wb.feature_authoring,
            radius,
            label,
        ) {
            Ok(transaction) => handle_feature_transaction(wb, transaction),
            Err(error) => {
                wb.notice = format!("Fillet preview is unavailable: {error}");
            }
        }
    }

    fn feature_radius_input(document: &Document) -> Result<Option<f64>, String> {
        optional_positive_input(document, "wb-feature-fillet-radius", "fillet radius")
    }

    fn optional_positive_input(
        document: &Document,
        id: &str,
        label: &str,
    ) -> Result<Option<f64>, String> {
        let value = input_value(document, id).unwrap_or_default();
        if value.trim().is_empty() {
            return Ok(None);
        }
        value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(Some)
            .ok_or_else(|| format!("{label} must be finite and positive"))
    }

    fn next_feature_authoring_label(wb: &Workbench) -> String {
        format!(
            "Fillet {}",
            wb.coordinator.feature_document().features().len() + 1
        )
    }

    /// Consumes a coordinator-accepted state/preview transaction. A complete
    /// candidate already owns its exact held preview, so this path must not
    /// prepare it a second time.
    fn handle_feature_transaction(wb: &mut Workbench, transaction: FeatureAuthoringTransaction) {
        match transaction.outcome {
            FeatureAuthoringOutcome::PreviewRequested {
                candidate,
                guidance,
            } => {
                if transaction.preview.is_none() {
                    wb.notice = "The exact current Fillet preview is unavailable".into();
                    return;
                }
                wb.feature_pending.clear();
                wb.feature_candidate = Some(candidate);
                wb.notice = format!("{} · Apply or press Enter", guidance.message);
            }
            outcome => {
                debug_assert!(transaction.preview.is_none());
                handle_feature_outcome(wb, outcome);
            }
        }
    }

    fn handle_feature_outcome(wb: &mut Workbench, outcome: FeatureAuthoringOutcome) {
        super::observe_feature_authoring_preview_lifecycle(&mut wb.coordinator, &outcome);
        match outcome {
            FeatureAuthoringOutcome::ModeEntered(guidance) => {
                wb.feature_candidate = None;
                wb.feature_pending.clear();
                wb.notice = format!("{} · Escape exits", guidance.message);
            }
            FeatureAuthoringOutcome::NoNativeHit(guidance) => {
                guidance.message.clone_into(&mut wb.notice);
            }
            FeatureAuthoringOutcome::Collecting { pending, guidance } => {
                wb.feature_candidate = None;
                wb.feature_pending = pending;
                guidance.message.clone_into(&mut wb.notice);
            }
            FeatureAuthoringOutcome::PreviewRequested {
                candidate,
                guidance,
            } => {
                wb.feature_pending.clear();
                wb.feature_candidate = None;
                let label = next_feature_authoring_label(wb);
                let expected = wb.coordinator.feature_document().identity();
                match wb
                    .coordinator
                    .prepare_feature_authoring_preview(expected, &candidate, label)
                {
                    Ok(_) => {
                        wb.feature_candidate = Some(candidate);
                        wb.notice = format!("{} · Apply or press Enter", guidance.message);
                    }
                    Err(error) => {
                        super::revoke_held_feature_authoring_preview(&mut wb.coordinator);
                        wb.notice = format!("Fillet preview is unavailable: {error}");
                    }
                }
            }
            FeatureAuthoringOutcome::Apply(candidate) => {
                let preview = wb
                    .coordinator
                    .feature_authoring_preview()
                    .map(|preview| preview.metadata().clone());
                let result = preview.ok_or_else(|| {
                    "the exact current Fillet preview is unavailable; adjust an option to rebuild it"
                        .to_owned()
                });
                match result.and_then(|preview| {
                    wb.coordinator
                        .apply_feature_authoring_preview(preview.token, &candidate)
                        .map_err(|error| error.to_string())
                }) {
                    Ok(mutation) => {
                        wb.coordinator
                            .editor_mut()
                            .set_selection([SelectionItem::Feature(mutation.value)]);
                        let _ = wb.feature_authoring.publication_succeeded();
                        wb.feature_candidate = None;
                        wb.feature_pending.clear();
                        wb.feature_options_open = false;
                        wb.notice = "Computed Fillet set accepted; Select active".into();
                    }
                    Err(error) => {
                        wb.feature_candidate = None;
                        super::revoke_held_feature_authoring_preview(&mut wb.coordinator);
                        wb.notice = format!("Fillet set was not applied: {error}");
                    }
                }
            }
            FeatureAuthoringOutcome::Warning(warning) => {
                wb.notice = warning.message;
            }
            FeatureAuthoringOutcome::CandidateCleared(guidance) => {
                wb.feature_candidate = None;
                wb.feature_pending.clear();
                wb.notice = format!("Fillet batch cleared · {}", guidance.message);
            }
            FeatureAuthoringOutcome::ModeExited => {
                wb.feature_candidate = None;
                wb.feature_pending.clear();
                wb.notice = "Computed feature authoring exited; Select active".into();
            }
            FeatureAuthoringOutcome::Inactive => {}
        }
    }

    fn handle_authoring_outcome(wb: &mut Workbench, outcome: AuthoringOutcome) {
        match outcome {
            AuthoringOutcome::Apply(application) => apply_authoring_application(wb, &application),
            AuthoringOutcome::ModeEntered { tool, expected } => {
                wb.notice = format!(
                    "{} mode: select {} · Escape exits",
                    authoring_tool_label(tool),
                    expected_labels(&expected),
                );
            }
            AuthoringOutcome::Collecting {
                tool,
                operands,
                expected,
            } => {
                wb.notice = format!(
                    "{}: {} operand{} ready; select {}",
                    authoring_tool_label(tool),
                    operands.len(),
                    if operands.len() == 1 { "" } else { "s" },
                    expected_labels(&expected),
                );
            }
            AuthoringOutcome::Warning(warning) => {
                wb.notice = warning.message;
            }
            AuthoringOutcome::PendingCleared { tool, expected } => {
                wb.notice = format!(
                    "{} operands cleared; select {} or Escape again to exit",
                    authoring_tool_label(tool),
                    expected_labels(&expected),
                );
            }
            AuthoringOutcome::ModeExited => {
                wb.notice = "Constraint authoring exited; Select active".into();
            }
            AuthoringOutcome::Inactive => {}
        }
    }

    fn apply_authoring_application(wb: &mut Workbench, application: &AuthoringApplication) {
        let expected = wb.coordinator.session().design_identity();
        let result = wb.coordinator.apply_authoring(expected, application);
        wb.authoring.transaction_finished();
        let _ = wb
            .authoring
            .reconcile(wb.coordinator.session().design_document());
        match result {
            Ok(mutation) => {
                let accepted = match &mutation {
                    geosolve_constraint_editor::AuthoringMutation::Constraint(outcome) => {
                        outcome.published_accepted.is_some()
                    }
                    geosolve_constraint_editor::AuthoringMutation::Dimension(outcome) => {
                        outcome.published_accepted.is_some()
                    }
                };
                let repeated = wb.authoring.active_tool().is_some();
                wb.notice = if !accepted {
                    format!(
                        "{} retained, but the solve rejected; prior accepted geometry remains",
                        authoring_tool_label(application.tool)
                    )
                } else if repeated {
                    format!(
                        "{} accepted; select the next operands",
                        authoring_tool_label(application.tool)
                    )
                } else {
                    format!("{} accepted", authoring_tool_label(application.tool))
                };
            }
            Err(error) => wb.notice = error.to_string(),
        }
    }

    fn authoring_options(document: &Document) -> Result<AuthoringOptions, String> {
        let tangent_orientation = if select_value(document, "wb-authoring-tangent-orientation")
            .as_deref()
            == Some("opposed")
        {
            TangentOrientation::Opposed
        } else {
            TangentOrientation::Aligned
        };
        let curvature_relation = match select_value(document, "wb-authoring-curvature").as_deref() {
            Some("same-sign") => DocumentCurveCurvatureRelation::MagnitudeSameSign,
            Some("opposite-sign") => DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
            _ => DocumentCurveCurvatureRelation::Signed,
        };
        let continuity = match select_value(document, "wb-authoring-continuity").as_deref() {
            Some("g0") => DocumentCurveContinuity::G0,
            Some("g2") => DocumentCurveContinuity::G2,
            Some("c2") => DocumentCurveContinuity::ParametricC2 {
                first_rate: finite_positive_input(
                    document,
                    "wb-authoring-first-rate",
                    "first C2 rate",
                )?,
                second_rate: finite_positive_input(
                    document,
                    "wb-authoring-second-rate",
                    "second C2 rate",
                )?,
            },
            _ => DocumentCurveContinuity::G1,
        };
        let dimension_mode = if select_value(document, "wb-authoring-dimension-mode").as_deref()
            == Some("reference")
        {
            DocumentDimensionMode::Reference
        } else {
            DocumentDimensionMode::Driving
        };
        let angle_orientation = if select_value(document, "wb-authoring-angle-orientation")
            .as_deref()
            == Some("clockwise")
        {
            DocumentAngleOrientation::Clockwise
        } else {
            DocumentAngleOrientation::CounterClockwise
        };
        Ok(AuthoringOptions {
            tangent_orientation,
            curvature_relation,
            continuity,
            dimension_mode,
            angle_orientation,
        })
    }

    fn expected_labels(values: &[geosolve_constraint_editor::AuthoringOperandKind]) -> String {
        values
            .iter()
            .map(|value| value.label())
            .collect::<Vec<_>>()
            .join(" or ")
    }

    fn authoring_tool_label(tool: AuthoringTool) -> &'static str {
        match tool {
            AuthoringTool::Constraint(intent) => super::action_surface::CONSTRAINT_ACTIONS
                .iter()
                .find_map(|(_, label, candidate)| (*candidate == intent).then_some(*label))
                .unwrap_or("Constraint"),
            AuthoringTool::Dimension(kind) => super::action_surface::DIMENSION_ACTIONS
                .iter()
                .find_map(|(_, label, candidate)| (*candidate == kind).then_some(*label))
                .unwrap_or("Dimension"),
        }
    }

    fn finite_positive_input(document: &Document, id: &str, label: &str) -> Result<f64, String> {
        input_value(document, id)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| format!("{label} must be finite and positive"))
    }

    fn apply_contact_branches(document: &Document, wb: &mut Workbench) -> Result<(), String> {
        let actions = wb.coordinator.branch_actions();
        let mut edits = Vec::new();
        for (index, action) in actions.iter().enumerate() {
            let BranchAction::Contact(action) = action else {
                continue;
            };
            let segment = select_value(document, &format!("wb-edit-span-{index}"))
                .and_then(|value| value.parse::<u32>().ok())
                .ok_or_else(|| "contact span must be a valid semantic span".to_owned())?;
            let curve = action
                .spans
                .iter()
                .copied()
                .find(|span| span.segment == segment)
                .ok_or_else(|| "selected contact span is unavailable".to_owned())?;
            let domain_key = select_value(document, &format!("wb-edit-domain-{index}"))
                .ok_or_else(|| "contact domain control is missing".to_owned())?;
            let domain = action
                .domains
                .iter()
                .copied()
                .find(|domain| contact_domain_key(*domain) == domain_key)
                .ok_or_else(|| "selected contact domain is unavailable".to_owned())?;
            let neighborhood_key = select_value(document, &format!("wb-edit-neighborhood-{index}"))
                .ok_or_else(|| "contact neighborhood control is missing".to_owned())?;
            let neighborhood = action
                .neighborhoods
                .iter()
                .copied()
                .find(|value| contact_neighborhood_key(*value) == neighborhood_key)
                .ok_or_else(|| "selected contact neighborhood is unavailable".to_owned())?;
            let value = input_value(document, &format!("wb-edit-parameter-{index}"))
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite())
                .ok_or_else(|| "contact parameter must be finite".to_owned())?;
            let winding = input_value(document, &format!("wb-edit-winding-{index}"))
                .and_then(|value| value.parse::<i32>().ok())
                .ok_or_else(|| "contact winding must be an integer".to_owned())?;
            let orientation_key =
                select_value(document, &format!("wb-edit-orientation-{index}"))
                    .ok_or_else(|| "contact orientation control is missing".to_owned())?;
            let tangent_orientation = action
                .tangent_orientations
                .iter()
                .copied()
                .find(|orientation| {
                    orientation.map_or("none", tangent_orientation_key) == orientation_key
                })
                .ok_or_else(|| "selected tangent orientation is unavailable".to_owned())?;
            edits.push(ContactBranchEdit {
                contact: action.current.contact,
                curve,
                domain,
                value,
                winding,
                neighborhood,
                tangent_orientation,
            });
        }
        if edits.is_empty() {
            return Err("select a contact-owning relation first".into());
        }
        let expected = wb.coordinator.session().design_identity();
        wb.coordinator
            .set_contact_branches(expected, edits)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn apply_angle_orientation(document: &Document, wb: &mut Workbench) -> Result<(), String> {
        let orientation = if select_value(document, "wb-edit-angle-orientation").as_deref()
            == Some("clockwise")
        {
            DocumentAngleOrientation::Clockwise
        } else {
            DocumentAngleOrientation::CounterClockwise
        };
        let expected = wb.coordinator.session().design_identity();
        wb.coordinator
            .set_selected_angle_orientation(expected, orientation)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn editor_scene(wb: &Workbench) -> Option<EditorScene> {
        let coordinator = &wb.coordinator;
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        let accepted = source.accepted_state()?;
        let accepted_input = source.accepted_prepared_input()?;
        match coordinator.computed_scene_state() {
            ComputedSceneState::Current { expected, snapshot } => {
                let mut scene = EditorScene::from_accepted_with_computed(
                    accepted.identity().revision().get(),
                    coordinator.session().design_identity(),
                    accepted.document(),
                    coordinator.session().design_document(),
                    &accepted_input,
                    expected,
                    snapshot,
                    wb.camera.viewport(),
                    super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
                )
                .ok()?;
                let mut action_items = coordinator.editor().selection().to_vec();
                if let Some(preview) = coordinator.feature_authoring_preview() {
                    action_items.push(SelectionItem::Feature(preview.metadata().feature));
                    action_items.sort_unstable();
                    action_items.dedup();
                }
                coordinator
                    .populate_computed_fillet_affordances(
                        &mut scene,
                        &action_items,
                        super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
                    )
                    .ok()?;
                Some(scene)
            }
            ComputedSceneState::Withheld | ComputedSceneState::Absent => {
                EditorScene::from_accepted_for_design(
                    accepted.identity().revision().get(),
                    coordinator.session().design_identity(),
                    accepted.document(),
                    coordinator.session().design_document(),
                    wb.camera.viewport(),
                    super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
                )
                .ok()
            }
        }
    }

    fn fit_camera(wb: &mut Workbench) {
        if let Some(scene) = editor_scene(wb) {
            wb.camera.fit_scene(&scene);
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one render pass synchronizes the complete retained workbench snapshot"
    )]
    fn render(document: &Document, workbench: &Rc<RefCell<Workbench>>) -> Result<(), JsValue> {
        let scene = editor_scene(&workbench.borrow());
        if let Some(scene) = scene.as_ref() {
            let mut wb = workbench.borrow_mut();
            let effects = wb
                .coordinator
                .editor_mut()
                .reconcile_fillet_branch_preview(scene);
            dispatch_effects(&mut wb, effects);
        }
        let fillet_action_stamp = workbench.borrow_mut().fillet_action_render.reconcile(
            scene
                .as_ref()
                .and_then(|scene| scene.computed_input.as_ref()),
        );
        let wb = workbench.borrow();
        let coordinator = &wb.coordinator;
        required(document, "workbench-root")?.set_attribute(
            "data-history-length",
            &coordinator.history_len().to_string(),
        )?;
        required(document, "workbench-root")?.set_attribute(
            "data-feature-preview",
            if wb.feature_candidate.is_some() {
                "ready"
            } else {
                "none"
            },
        )?;
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        let accepted = source.accepted_state();
        let selection = coordinator.editor().selection();
        let mut canvas_selection = selection.to_vec();
        if let Some(preview) = coordinator.feature_authoring_preview() {
            canvas_selection.push(SelectionItem::Feature(preview.metadata().feature));
            canvas_selection.sort_unstable();
            canvas_selection.dedup();
        }
        let mut pending = wb
            .authoring
            .pending()
            .iter()
            .map(|operand| operand.item)
            .collect::<Vec<_>>();
        pending.extend(
            wb.feature_pending
                .iter()
                .map(|pick| SelectionItem::Curve(pick.curve.source.span)),
        );
        pending.sort_unstable();
        pending.dedup();
        let construction_preview = wb.construction_preview.as_ref();
        let hover = coordinator.editor().hover_state();
        let computed_problems = coordinator.computed_feature_problems();
        let active_fillet_preview = coordinator.editor().fillet_branch_preview();
        required(document, "wb-viewport")?.set_inner_html(
            &super::scene::svg_markup_with_computed_context_and_action_stamp(
                scene.as_ref(),
                accepted,
                &computed_problems,
                &canvas_selection,
                &pending,
                hover,
                construction_preview,
                coordinator.current_problem_metadata().as_ref(),
                active_fillet_preview.as_ref(),
                fillet_action_stamp,
                wb.camera.viewport(),
            ),
        );
        mark_geometry_hover(
            document,
            hover.target.and_then(|target| match target {
                EditorHoverTarget::Geometry(item) => Some(item),
                EditorHoverTarget::Annotation(_) => None,
            }),
        )?;
        let design = coordinator.session().design_document();
        required(document, "wb-tree")?.set_inner_html(&super::panels::tree_markup_with_features(
            design,
            coordinator.feature_document(),
            coordinator.computed_snapshot(),
            &computed_problems,
            selection,
            &pending,
        ));
        let lifecycle = coordinator.lifecycle();
        let (key, label) = super::panels::lifecycle_presentation(lifecycle.status);
        let state = required(document, "wb-lifecycle")?;
        state.set_attribute("data-state", key)?;
        state.set_text_content(Some(label));
        required(document, "wb-status-message")?.set_text_content(Some(&wb.notice));
        required(document, "wb-camera-scale")?.set_text_content(Some(&format!(
            "{:.1} px / unit",
            wb.camera.pixels_per_model_unit
        )));
        required(document, "wb-status-count")?.set_text_content(Some(&format!(
            "{} points / {} curves",
            design.points().len(),
            design.curves().len(),
        )));
        required(document, "wb-selection")?
            .set_text_content(Some(&format!("{} selected", selection.len())));
        let problem = problem_text(coordinator, &computed_problems);
        required(document, "wb-problem-text")?
            .set_inner_html(&super::panels::problem_markup(&problem));
        render_sample_ui(document, &wb.samples)?;
        let problems = required(document, "wb-problems")?;
        if wb.problems_open
            || coordinator.current_problem_metadata().is_some()
            || !computed_problems.is_empty()
        {
            problems.remove_attribute("hidden")?;
        } else {
            problems.set_attribute("hidden", "")?;
        }
        let guide = required(document, "wb-draft-guide")?;
        if wb.authoring.active_tool().is_some()
            || wb.feature_authoring.active_tool().is_some()
            || coordinator.editor().can_complete_draft()
            || wb.construction_preview.is_some()
        {
            guide.remove_attribute("hidden")?;
        } else {
            guide.set_attribute("hidden", "")?;
        }
        let guide_text = if wb.feature_authoring.active_tool().is_some() {
            wb.feature_authoring.guidance().message.to_owned()
        } else {
            wb.authoring.active_tool().map_or_else(
                || draft_guide_text(coordinator.editor().tool()).to_owned(),
                |tool| {
                    format!(
                        "{} · {} pending · Escape clears/exits",
                        authoring_tool_label(tool),
                        wb.authoring.pending().len()
                    )
                },
            )
        };
        required(document, "wb-draft-guide-text")?.set_text_content(Some(&guide_text));
        if wb.authoring.active_tool().is_some() || wb.feature_authoring.active_tool().is_some() {
            required(document, "wb-guide-finish")?.set_attribute("hidden", "")?;
        } else {
            required(document, "wb-guide-finish")?.remove_attribute("hidden")?;
        }
        let apply = required(document, "wb-guide-apply")?;
        if wb.feature_candidate.is_some()
            && wb.feature_authoring.guidance().stage == FeatureAuthoringStage::PreviewReady
        {
            apply.remove_attribute("hidden")?;
            set_disabled(&apply, false)?;
        } else {
            apply.set_attribute("hidden", "")?;
            set_disabled(&apply, true)?;
        }
        for (key, tool) in super::icons::GEOMETRY_TOOLS {
            if let Some(button) = document.query_selector(&format!("[data-wb-tool=\"{key}\"]"))? {
                button.set_attribute(
                    "aria-pressed",
                    if tool == coordinator.editor().tool() {
                        "true"
                    } else {
                        "false"
                    },
                )?;
            }
        }
        render_action_availability(document, coordinator, &wb.authoring, &wb.feature_authoring)?;
        render_feature_options(document, &wb.feature_authoring)?;
        render_fillet_options_overlay(document, wb.feature_options_open)?;
        render_dimension_target_editor(document, coordinator)?;
        render_branch_editor(document, coordinator)?;
        render_feature_editor(document, coordinator)?;
        render_fillet_action_panel(document, scene.as_ref(), fillet_action_stamp)?;
        required(document, "workbench-root")?
            .set_attribute("data-editor-adapter", "retained-coordinator")?;
        Ok(())
    }

    fn mark_geometry_hover(
        document: &Document,
        item: Option<SelectionItem>,
    ) -> Result<(), JsValue> {
        let Some(selector) = item.and_then(super::geometry_hover_selector) else {
            return Ok(());
        };
        let Some(element) = document.query_selector(&selector)? else {
            return Ok(());
        };
        let mut classes = element.get_attribute("class").unwrap_or_default();
        if !classes
            .split_ascii_whitespace()
            .any(|class| class == "geometry-hovered")
        {
            classes.push_str(" geometry-hovered");
            element.set_attribute("class", classes.trim())?;
        }
        Ok(())
    }

    fn render_fillet_options_overlay(document: &Document, open: bool) -> Result<(), JsValue> {
        let trigger = required(document, "wb-feature-fillet-options-trigger")?;
        trigger.set_attribute("aria-expanded", if open { "true" } else { "false" })?;
        let overlay = required(document, "wb-feature-options-overlay")?;
        if !open {
            overlay.set_attribute("hidden", "")?;
            overlay.remove_attribute("style")?;
            return Ok(());
        }
        overlay.remove_attribute("hidden")?;
        reposition_fillet_options_overlay(document)
    }

    fn reposition_fillet_options_overlay(document: &Document) -> Result<(), JsValue> {
        let overlay = required(document, "wb-feature-options-overlay")?;
        if overlay.has_attribute("hidden") {
            return Ok(());
        }
        let trigger_rect =
            required(document, "wb-feature-fillet-trigger")?.get_bounding_client_rect();
        let canvas_rect = required(document, "wb-canvas-panel")?.get_bounding_client_rect();
        let overlay_rect = overlay.get_bounding_client_rect();
        let position = super::canvas_overlay_position(
            super::OverlayRect {
                left: trigger_rect.left(),
                top: trigger_rect.top(),
                width: trigger_rect.width(),
                height: trigger_rect.height(),
            },
            super::OverlayRect {
                left: canvas_rect.left(),
                top: canvas_rect.top(),
                width: canvas_rect.width(),
                height: canvas_rect.height(),
            },
            super::OverlayRect {
                left: overlay_rect.left(),
                top: overlay_rect.top(),
                width: overlay_rect.width(),
                height: overlay_rect.height(),
            },
            8.0,
            8.0,
        )
        .unwrap_or(geosolve_constraint_editor::ScreenPoint { x: 8.0, y: 8.0 });
        overlay.set_attribute(
            "style",
            &format!("left: {:.3}px; top: {:.3}px", position.x, position.y),
        )?;
        Ok(())
    }

    fn render_action_availability(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
        authoring: &AuthoringState,
        feature_authoring: &FeatureAuthoringState,
    ) -> Result<(), JsValue> {
        for key in ["new", "finish", "cancel", "clear-selection"] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_disabled(
                    &button,
                    key == "finish"
                        && (authoring.active_tool().is_some()
                            || feature_authoring.active_tool().is_some()),
                )?;
            }
        }
        let actions = coordinator.actions();
        let state = |action| {
            actions.iter().find(|value| value.action == action).map_or(
                ActionState::Disabled(DisabledReason::WrongOperandKind),
                |value| value.state,
            )
        };
        for (key, action) in [
            ("undo", CoordinatorActionKind::Undo),
            ("redo", CoordinatorActionKind::Redo),
            ("delete", CoordinatorActionKind::Delete),
        ] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_action_state(&button, state(action))?;
            }
        }
        for (key, _, intent) in super::action_surface::CONSTRAINT_ACTIONS {
            if let Some(button) =
                document.query_selector(&format!("[data-wb-authoring=\"{key}\"]"))?
            {
                button.set_attribute(
                    "aria-pressed",
                    if authoring.active_tool() == Some(AuthoringTool::Constraint(intent)) {
                        "true"
                    } else {
                        "false"
                    },
                )?;
            }
        }
        for (key, _, kind) in super::action_surface::DIMENSION_ACTIONS {
            if let Some(button) =
                document.query_selector(&format!("[data-wb-authoring=\"{key}\"]"))?
            {
                button.set_attribute(
                    "aria-pressed",
                    if authoring.active_tool() == Some(AuthoringTool::Dimension(kind)) {
                        "true"
                    } else {
                        "false"
                    },
                )?;
            }
        }
        for (key, _, tool) in super::action_surface::FEATURE_ACTIONS {
            if let Some(button) =
                document.query_selector(&format!("[data-wb-feature=\"{key}\"]"))?
            {
                button.set_attribute(
                    "aria-pressed",
                    if feature_authoring.active_tool() == Some(tool) {
                        "true"
                    } else {
                        "false"
                    },
                )?;
            }
        }
        for id in [
            "wb-authoring-curvature",
            "wb-authoring-tangent-orientation",
            "wb-authoring-continuity",
            "wb-authoring-first-rate",
            "wb-authoring-second-rate",
            "wb-authoring-dimension-mode",
            "wb-authoring-angle-orientation",
            "wb-feature-fillet-radius",
        ] {
            set_disabled(&required(document, id)?, false)?;
        }
        Ok(())
    }

    fn render_dimension_target_editor(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
    ) -> Result<(), JsValue> {
        let section = required(document, "wb-dimension-target-editor")?;
        let Some(metadata) = coordinator.selected_dimension_target_metadata() else {
            section.set_attribute("hidden", "")?;
            return Ok(());
        };
        section.remove_attribute("hidden")?;
        let input = required(document, "wb-dimension-target")?;
        let (label, meta) = match metadata.display_unit {
            DimensionTargetDisplayUnit::ModelUnits => {
                input.remove_attribute("max")?;
                (
                    "Dimension target",
                    format!("{:?} · model units", metadata.mode),
                )
            }
            DimensionTargetDisplayUnit::AcuteDegrees => {
                input.set_attribute("max", "90")?;
                (
                    "Acute angle target (degrees)",
                    format!(
                        "{:?} · acute supporting-line angle · directed branch retained",
                        metadata.mode
                    ),
                )
            }
        };
        if let Some(element) = document.query_selector("label[for=\"wb-dimension-target\"]")? {
            element.set_text_content(Some(label));
        }
        if let Ok(input) = input.dyn_into::<HtmlInputElement>() {
            let editing = document
                .active_element()
                .is_some_and(|element| element.id() == "wb-dimension-target");
            if !editing && input.value_as_number().to_bits() != metadata.display_value.to_bits() {
                input.set_value_as_number(metadata.display_value);
            }
            input.set_disabled(false);
        }
        required(document, "wb-dimension-target-meta")?.set_text_content(Some(&meta));
        if let Some(button) = document.query_selector("[data-wb-action=\"dimension-target\"]")? {
            set_disabled(&button, false)?;
        }
        Ok(())
    }

    fn render_feature_options(
        document: &Document,
        state: &FeatureAuthoringState,
    ) -> Result<(), JsValue> {
        let options = state.options();
        render_optional_number(document, "wb-feature-fillet-radius", options.fillet_radius)
    }

    fn render_feature_editor(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
    ) -> Result<(), JsValue> {
        let section = required(document, "wb-feature-editor")?;
        let feature = match coordinator.editor().selection() {
            [SelectionItem::Feature(feature)] => Some(*feature),
            [SelectionItem::FeatureCorner(owner)] => Some(owner.feature),
            _ => None,
        };
        let Some(feature) =
            feature.and_then(|feature| coordinator.feature_document().feature(feature))
        else {
            section.set_attribute("hidden", "")?;
            return Ok(());
        };
        section.remove_attribute("hidden")?;
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &feature.definition;
        if let Ok(input) = required(document, "wb-feature-radius")?.dyn_into::<HtmlInputElement>() {
            let editing = document
                .active_element()
                .is_some_and(|element| element.id() == "wb-feature-radius");
            if !editing && input.value_as_number().to_bits() != fillet.radius.to_bits() {
                input.set_value_as_number(fillet.radius);
            }
            input.set_disabled(false);
        }
        let suppression = required(document, "wb-feature-suppression")?;
        suppression.set_text_content(Some(if feature.suppressed {
            "Unsuppress feature"
        } else {
            "Suppress feature"
        }));
        set_disabled(&suppression, false)?;
        Ok(())
    }

    fn render_fillet_action_panel(
        document: &Document,
        scene: Option<&EditorScene>,
        fillet_action_stamp: Option<u64>,
    ) -> Result<(), JsValue> {
        let panel = required(document, "wb-fillet-actions-panel")?;
        let markup = scene.map_or_else(String::new, |scene| {
            super::scene::fillet_action_panel_markup_with_stamp(scene, fillet_action_stamp)
        });
        let actions = required(document, "wb-fillet-actions")?;
        let fingerprint = super::markup_fingerprint(&markup);
        if actions.get_attribute("data-markup-fingerprint").as_deref() != Some(&fingerprint) {
            actions.set_inner_html(&markup);
            actions.set_attribute("data-markup-fingerprint", &fingerprint)?;
        }
        if markup.is_empty() {
            panel.set_attribute("hidden", "")?;
        } else {
            panel.remove_attribute("hidden")?;
        }
        Ok(())
    }

    fn render_optional_number(
        document: &Document,
        id: &str,
        value: Option<f64>,
    ) -> Result<(), JsValue> {
        if document
            .active_element()
            .is_some_and(|element| element.id() == id)
        {
            return Ok(());
        }
        if let Ok(input) = required(document, id)?.dyn_into::<HtmlInputElement>() {
            match value {
                Some(value) => input.set_value_as_number(value),
                None => input.set_value(""),
            }
        }
        Ok(())
    }

    #[allow(
        clippy::format_collect,
        clippy::too_many_lines,
        reason = "the closed branch DTO catalog is rendered as one auditable HTML fragment"
    )]
    fn render_branch_editor(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
    ) -> Result<(), JsValue> {
        let actions = coordinator.branch_actions();
        let editor = required(document, "wb-branch-editor")?;
        let controls = required(document, "wb-branch-editor-controls")?;
        let contact_button = required(document, "wb-apply-contact-branches")?;
        let angle_button = required(document, "wb-apply-angle-orientation")?;
        if actions.is_empty() {
            editor.set_attribute("hidden", "")?;
            controls.set_inner_html("");
            contact_button.set_attribute("hidden", "")?;
            angle_button.set_attribute("hidden", "")?;
            return Ok(());
        }
        editor.remove_attribute("hidden")?;
        let mut markup = String::new();
        for (index, action) in actions.iter().enumerate() {
            match action {
                BranchAction::Contact(action) => {
                    let current = action.current;
                    let span_options = action
                        .spans
                        .iter()
                        .map(|span| {
                            format!(
                                "<option value=\"{}\"{}>Span {}</option>",
                                span.segment,
                                if *span == current.curve {
                                    " selected"
                                } else {
                                    ""
                                },
                                span.segment
                            )
                        })
                        .collect::<String>();
                    let domain_options = action
                        .domains
                        .iter()
                        .copied()
                        .map(|domain| {
                            format!(
                                "<option value=\"{}\"{}>{}</option>",
                                contact_domain_key(domain),
                                if domain == current.domain {
                                    " selected"
                                } else {
                                    ""
                                },
                                contact_domain_label(domain),
                            )
                        })
                        .collect::<String>();
                    let neighborhood_options = action
                        .neighborhoods
                        .iter()
                        .copied()
                        .map(|neighborhood| {
                            format!(
                                "<option value=\"{}\"{}>{}</option>",
                                contact_neighborhood_key(neighborhood),
                                if contact_neighborhood_key(neighborhood)
                                    == contact_neighborhood_key(current.neighborhood)
                                {
                                    " selected"
                                } else {
                                    ""
                                },
                                contact_neighborhood_label(neighborhood),
                            )
                        })
                        .collect::<String>();
                    let orientation_options = action
                        .tangent_orientations
                        .iter()
                        .map(|orientation| {
                            let (key, label) =
                                orientation.map_or(("none", "Not applicable"), |value| {
                                    (
                                        tangent_orientation_key(value),
                                        tangent_orientation_label(value),
                                    )
                                });
                            format!(
                                "<option value=\"{key}\"{}>{label}</option>",
                                if *orientation == current.tangent_orientation {
                                    " selected"
                                } else {
                                    ""
                                }
                            )
                        })
                        .collect::<String>();
                    markup.push_str(&format!(
                        concat!(
                            "<fieldset data-branch-contact=\"{}\"><legend>Contact {}</legend>",
                            "<label>Span<select id=\"wb-edit-span-{}\">{}</select></label>",
                            "<label>Domain<select id=\"wb-edit-domain-{}\">{}</select></label>",
                            "<label>Parameter<input id=\"wb-edit-parameter-{}\" type=\"number\" step=\"any\" value=\"{}\"/></label>",
                            "<label>Neighborhood<select id=\"wb-edit-neighborhood-{}\">{}</select></label>",
                            "<label>Winding<input id=\"wb-edit-winding-{}\" type=\"number\" step=\"1\" value=\"{}\"/></label>",
                            "<label>Orientation<select id=\"wb-edit-orientation-{}\">{}</select></label></fieldset>"
                        ),
                        current.contact,
                        index + 1,
                        index,
                        span_options,
                        index,
                        domain_options,
                        index,
                        current.value,
                        index,
                        neighborhood_options,
                        index,
                        current.winding,
                        index,
                        orientation_options,
                    ));
                }
                BranchAction::AngleOrientation { current, .. } => {
                    markup.push_str(&format!(
                        concat!(
                            "<label>Angle direction<select id=\"wb-edit-angle-orientation\">",
                            "<option value=\"counter-clockwise\"{}>Counter-clockwise</option>",
                            "<option value=\"clockwise\"{}>Clockwise</option></select></label>"
                        ),
                        if *current == DocumentAngleOrientation::CounterClockwise {
                            " selected"
                        } else {
                            ""
                        },
                        if *current == DocumentAngleOrientation::Clockwise {
                            " selected"
                        } else {
                            ""
                        },
                    ));
                }
            }
        }
        controls.set_inner_html(&markup);
        let has_contacts = actions
            .iter()
            .any(|action| matches!(action, BranchAction::Contact(_)));
        let has_angle = actions
            .iter()
            .any(|action| matches!(action, BranchAction::AngleOrientation { .. }));
        set_hidden(&contact_button, !has_contacts)?;
        set_hidden(&angle_button, !has_angle)?;
        set_disabled(&contact_button, false)?;
        set_disabled(&angle_button, false)?;
        Ok(())
    }

    fn render_sample_ui(
        document: &Document,
        samples: &super::samples::SampleCatalogState,
    ) -> Result<(), JsValue> {
        let selected_key = samples.selected_key().unwrap_or("");
        let menu = required(document, "wb-sample-menu")?;
        if menu.get_attribute("data-selected-sample").as_deref() != Some(selected_key) {
            menu.set_inner_html(&samples.menu_markup());
            menu.set_attribute("data-selected-sample", selected_key)?;
        }
        required(document, "wb-sample-current")?.set_text_content(Some(
            samples
                .selected_title()
                .unwrap_or("Choose an editable sample"),
        ));
        Ok(())
    }

    fn set_disabled(element: &Element, disabled: bool) -> Result<(), JsValue> {
        if disabled {
            element.set_attribute("disabled", "")
        } else {
            element.remove_attribute("disabled")
        }
    }

    fn set_hidden(element: &Element, hidden: bool) -> Result<(), JsValue> {
        if hidden {
            element.set_attribute("hidden", "")
        } else {
            element.remove_attribute("hidden")
        }
    }

    fn set_action_state(element: &Element, state: ActionState) -> Result<(), JsValue> {
        let disabled = state != ActionState::Enabled;
        set_disabled(element, disabled)?;
        if let ActionState::Disabled(reason) = state {
            element.set_attribute("data-disabled-reason", disabled_reason_key(reason))
        } else {
            element.remove_attribute("data-disabled-reason")
        }
    }

    const fn disabled_reason_key(reason: DisabledReason) -> &'static str {
        match reason {
            DisabledReason::EmptySelection => "empty-selection",
            DisabledReason::WrongArity => "wrong-arity",
            DisabledReason::WrongOperandKind => "wrong-operand-kind",
            DisabledReason::MissingObject => "missing-object",
            DisabledReason::InvalidSpan => "invalid-span",
            DisabledReason::AlreadyInRequestedState => "already-in-requested-state",
            DisabledReason::NothingToUndo => "nothing-to-undo",
            DisabledReason::NothingToRedo => "nothing-to-redo",
        }
    }

    fn problem_text(
        coordinator: &RetainedEditorCoordinator,
        computed: &[geosolve_constraint_editor::ComputedFeatureProblemMetadata],
    ) -> String {
        let mut messages = coordinator
            .current_problem_metadata()
            .map(|problem| vec![problem.message])
            .unwrap_or_default();
        messages.extend(computed.iter().map(|problem| {
            problem.feature.map_or_else(
                || format!("Computed feature evaluation: {}", problem.message),
                |feature| format!("Computed feature {feature}: {}", problem.message),
            )
        }));
        if messages.is_empty() {
            "No current solver or computed-feature problem".into()
        } else {
            messages.join(" · ")
        }
    }

    fn save(wb: &Workbench) {
        let Ok(snapshot) = WorkspaceSnapshot::from_coordinator(&wb.coordinator) else {
            return;
        };
        let Ok(json) = snapshot.encode() else {
            return;
        };
        let Ok(window) = super::platform::window() else {
            return;
        };
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item(STORAGE_KEY, &json);
        }
    }

    fn pointer_input(
        viewport: &Element,
        model_viewport: geosolve_constraint_editor::Viewport,
        event: &PointerEvent,
    ) -> Option<PointerInput> {
        let pointer_id = u64::try_from(event.pointer_id()).ok()?;
        Some(PointerInput {
            pointer_id,
            position: client_screen_point(
                viewport,
                model_viewport,
                f64::from(event.client_x()),
                f64::from(event.client_y()),
            )?,
            modifiers: Modifiers {
                shift: event.shift_key(),
                control: event.ctrl_key(),
                command: event.meta_key(),
            },
        })
    }

    fn client_screen_point(
        viewport: &Element,
        model_viewport: geosolve_constraint_editor::Viewport,
        client_x: f64,
        client_y: f64,
    ) -> Option<geosolve_constraint_editor::ScreenPoint> {
        let rect = viewport.get_bounding_client_rect();
        super::effect_adapter::normalize_client_point(
            super::effect_adapter::ClientRect {
                left: rect.left(),
                top: rect.top(),
                width: rect.width(),
                height: rect.height(),
            },
            model_viewport.screen_size,
            [client_x, client_y],
        )
    }

    fn selection_item(target: &Element) -> Option<SelectionItem> {
        match target.get_attribute("data-editor-item")?.as_str() {
            "point" => Some(SelectionItem::Point(DesignPointId(
                PersistentId::from_str(&target.get_attribute("data-persistent-id")?).ok()?,
            ))),
            "curve" => Some(SelectionItem::Curve(CurveSpan {
                curve: CurveId(
                    PersistentId::from_str(&target.get_attribute("data-persistent-id")?).ok()?,
                ),
                segment: target.get_attribute("data-editor-segment")?.parse().ok()?,
            })),
            "constraint" => Some(SelectionItem::Constraint(DocumentConstraintId(
                PersistentId::from_str(&target.get_attribute("data-persistent-id")?).ok()?,
            ))),
            "dimension" => Some(SelectionItem::Dimension(DocumentDimensionId(
                PersistentId::from_str(&target.get_attribute("data-persistent-id")?).ok()?,
            ))),
            "feature" => Some(SelectionItem::Feature(
                ComputedFeatureId::from_str(&target.get_attribute("data-feature-id")?).ok()?,
            )),
            "feature-corner" => Some(SelectionItem::FeatureCorner(ComputedCornerRef {
                feature: ComputedFeatureId::from_str(&target.get_attribute("data-feature-id")?)
                    .ok()?,
                corner: ComputedFeatureCornerId::from_str(
                    &target.get_attribute("data-feature-corner-id")?,
                )
                .ok()?,
            })),
            _ => None,
        }
    }

    fn fillet_action_target(
        scene: &EditorScene,
        target: &Element,
        authority: &super::FilletActionRenderAuthority,
    ) -> Option<SceneFilletActionTarget> {
        let stamp = target
            .get_attribute("data-fillet-action-stamp")?
            .parse::<u64>()
            .ok()?;
        if !authority.accepts(stamp, scene.computed_input.as_ref()) {
            return None;
        }
        let owner = ComputedCornerRef {
            feature: ComputedFeatureId::from_str(&target.get_attribute("data-feature-id")?).ok()?,
            corner: ComputedFeatureCornerId::from_str(
                &target.get_attribute("data-feature-corner-id")?,
            )
            .ok()?,
        };
        let action =
            super::scene::fillet_action_from_key(&target.get_attribute("data-fillet-action")?)?;
        scene.fillet_action_target(owner, action)
    }

    fn tool_from_key(key: &str) -> Option<EditorTool> {
        Some(match key {
            "select" => EditorTool::Select,
            "point" => EditorTool::Point,
            "line" => EditorTool::Line,
            "polyline" => EditorTool::Polyline,
            "rectangle" => EditorTool::Rectangle,
            "circle" => EditorTool::Circle,
            "arc" => EditorTool::CounterClockwiseArc,
            "quadratic-bezier" => EditorTool::QuadraticBezier,
            "cubic-bezier" => EditorTool::CubicBezier,
            "ellipse" => EditorTool::Ellipse,
            "elliptical-arc" => EditorTool::EllipticalArc,
            "rational-conic" => EditorTool::RationalQuadraticConic,
            "parabola" => EditorTool::Parabola,
            "hyperbola" => EditorTool::Hyperbola,
            "nurbs" => EditorTool::Nurbs,
            _ => return None,
        })
    }

    const fn draft_guide_text(tool: EditorTool) -> &'static str {
        match tool {
            EditorTool::Polyline => "Add another vertex or Finish the polyline",
            EditorTool::Nurbs => "Add another control or Finish the NURBS",
            _ => "Click to add the next control",
        }
    }
    fn update_construction_options(
        document: &Document,
        editor: &mut geosolve_constraint_editor::ConstraintEditor,
    ) -> Result<(), String> {
        let number = |id: &str, label: &'static str| {
            input_value(document, id)
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite())
                .ok_or_else(|| format!("{label} must be a finite number"))
        };
        editor
            .set_conic_options(ConicConstructionOptions {
                minor_axis_ratio: number("wb-conic-ratio", "Minor-axis ratio")?,
                arc_start: number("wb-conic-arc-start", "Arc start")?.to_radians(),
                arc_end: number("wb-conic-arc-end", "Arc end")?.to_radians(),
                arc_sweep: if select_value(document, "wb-conic-arc-sweep").as_deref()
                    == Some("clockwise")
                {
                    DocumentArcSweep::Clockwise
                } else {
                    DocumentArcSweep::CounterClockwise
                },
                middle_weight: number("wb-conic-weight", "Rational weight")?,
                trim_start: number("wb-conic-trim-start", "Trim start")?,
                trim_end: number("wb-conic-trim-end", "Trim end")?,
                semi_conjugate: number("wb-conic-semi-conjugate", "Semi-conjugate length")?,
                hyperbola_branch: if select_value(document, "wb-conic-hyperbola-branch").as_deref()
                    == Some("negative")
                {
                    DocumentHyperbolaBranch::Negative
                } else {
                    DocumentHyperbolaBranch::Positive
                },
            })
            .map_err(|error| error.to_string())?;

        let degree = input_value(document, "wb-nurbs-degree")
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(|| "NURBS degree must be a positive integer".to_owned())?;
        let gauge_index = input_value(document, "wb-nurbs-gauge")
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| "NURBS gauge index must be a non-negative integer".to_owned())?;
        let weights_text = input_value(document, "wb-nurbs-weights").unwrap_or_default();
        let weights = if weights_text.trim().is_empty() {
            Vec::new()
        } else {
            weights_text
                .split(',')
                .map(|part| {
                    part.trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| {
                            "NURBS weights must be comma-separated finite numbers".to_owned()
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        editor
            .set_nurbs_options(NurbsConstructionOptions {
                form: if select_value(document, "wb-nurbs-form").as_deref() == Some("periodic") {
                    DocumentBSplineForm::Periodic
                } else {
                    DocumentBSplineForm::Clamped
                },
                degree,
                weights,
                gauge_index,
            })
            .map_err(|error| error.to_string())
    }
    const fn contact_domain_key(domain: ContactDomain) -> &'static str {
        match domain {
            ContactDomain::SupportingLine => "supporting-line",
            ContactDomain::Bounded { .. } => "bounded",
            ContactDomain::Periodic { .. } => "periodic",
        }
    }
    const fn contact_domain_label(domain: ContactDomain) -> &'static str {
        match domain {
            ContactDomain::SupportingLine => "Supporting line",
            ContactDomain::Bounded { .. } => "Bounded span",
            ContactDomain::Periodic { .. } => "Periodic",
        }
    }
    const fn contact_neighborhood_key(neighborhood: ContactNeighborhood) -> &'static str {
        match neighborhood {
            ContactNeighborhood::Interior => "interior",
            ContactNeighborhood::Local { .. } => "local",
            ContactNeighborhood::Start => "start",
            ContactNeighborhood::End => "end",
        }
    }
    const fn contact_neighborhood_label(neighborhood: ContactNeighborhood) -> &'static str {
        match neighborhood {
            ContactNeighborhood::Interior => "Interior",
            ContactNeighborhood::Local { .. } => "Local interval",
            ContactNeighborhood::Start => "Start endpoint",
            ContactNeighborhood::End => "End endpoint",
        }
    }
    const fn tangent_orientation_key(orientation: TangentOrientation) -> &'static str {
        match orientation {
            TangentOrientation::Aligned => "aligned",
            TangentOrientation::Opposed => "opposed",
        }
    }
    const fn tangent_orientation_label(orientation: TangentOrientation) -> &'static str {
        match orientation {
            TangentOrientation::Aligned => "Aligned",
            TangentOrientation::Opposed => "Opposed",
        }
    }
    fn select_value(document: &Document, id: &str) -> Option<String> {
        Some(
            document
                .get_element_by_id(id)?
                .dyn_into::<HtmlSelectElement>()
                .ok()?
                .value(),
        )
    }
    fn input_value(document: &Document, id: &str) -> Option<String> {
        Some(
            document
                .get_element_by_id(id)?
                .dyn_into::<HtmlInputElement>()
                .ok()?
                .value(),
        )
    }

    fn close_sample_selector(document: &Document) {
        if let Ok(selector) = required(document, "wb-sample-selector") {
            let _ = selector.remove_attribute("open");
        }
    }

    fn focus_by_id(document: &Document, id: &str) {
        if let Ok(element) = required(document, id) {
            focus(element);
        }
    }

    fn focus(element: Element) {
        if let Ok(element) = element.dyn_into::<HtmlElement>() {
            let _ = element.focus();
        }
    }

    fn required(document: &Document, id: &str) -> Result<Element, JsValue> {
        document
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing #{id}")))
    }
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        AuthoringOperand, AuthoringOutcome, AuthoringState, AuthoringTool, ConstraintIntent,
        EditorHoverState, FeatureAuthoringCandidate, FeatureAuthoringOptions,
        FeatureAuthoringOutcome, FeatureAuthoringPreviewMetadata, FeatureAuthoringState,
        FeatureAuthoringTool, Modifiers, PickTolerance, PointerInput, RetainedEditorCoordinator,
        ScreenPoint, SelectionItem, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        CurveDefinition, CurveSpan, DesignPointId, DocumentCurveNormalSide, DocumentSolveRequest,
        RetainedSketchDocumentSession, SketchDocument,
    };

    use super::{
        AuthoringItemInput, CANVAS_BROWSER_DEFAULT_GUARD_EVENTS, CANVAS_PAN_POINTER_EVENTS,
        CANVAS_POINTER_TERMINAL_EVENTS, CanvasPanPointerDownRoute, CanvasPointerCaptureKind,
        CanvasPointerCaptures, CanvasPointerOwnership, CanvasPointerTerminal,
        CanvasPointerTerminalDisposition, CapturedCanvasPointer, FilletActionRenderAuthority,
        OverlayRect, PointerMoveQueue, canvas_overlay_position, canvas_pointer_capture_kind,
        change_owns_option_control_click, geometry_hover_selector,
        observe_feature_authoring_preview_lifecycle, owns_authoring_pick,
        palette_details_overlay_reflow_listener, resolve_canvas_fillet_action_candidates,
        revoke_held_feature_authoring_preview, route_canvas_pan_pointer_down,
    };

    #[test]
    fn canvas_browser_defaults_are_blocked_only_inside_the_svg_surface() {
        assert_eq!(
            CANVAS_BROWSER_DEFAULT_GUARD_EVENTS,
            ["selectstart", "dragstart"]
        );

        let html = include_str!("../../index.html");
        let viewport_start = html.find("<svg id=\"wb-viewport\"").expect("canvas SVG");
        let viewport_end = viewport_start
            + html[viewport_start..]
                .find("</svg>")
                .expect("canvas SVG boundary");
        let viewport_tag_end = viewport_start
            + html[viewport_start..]
                .find('>')
                .expect("canvas SVG opening tag");
        assert!(
            html[viewport_start..=viewport_tag_end].contains("draggable=\"false\""),
            "the canvas must opt out of native element dragging"
        );
        let options_overlay = html
            .find("id=\"wb-feature-options-overlay\"")
            .expect("Fillet options overlay");
        let radius_input = html
            .find("id=\"wb-feature-fillet-radius\"")
            .expect("Fillet radius input");
        assert!(viewport_end < options_overlay && options_overlay < radius_input);

        let css = include_str!("../../styles.css");
        let guard_start = css
            .find("#wb-viewport,\n#wb-viewport * {")
            .expect("scoped canvas browser-default guard");
        let guard_end = guard_start
            + css[guard_start..]
                .find('}')
                .expect("canvas browser-default guard boundary");
        let guard = &css[guard_start..=guard_end];
        for declaration in [
            "-webkit-user-select: none;",
            "user-select: none;",
            "-webkit-user-drag: none;",
        ] {
            assert!(guard.contains(declaration), "missing `{declaration}`");
        }
        assert!(!guard.contains("wb-feature-options"));
    }

    #[test]
    fn solver_problem_card_is_a_non_intercepting_canvas_overlay() {
        let html = include_str!("../../index.html");
        let canvas = html.find("id=\"wb-canvas-panel\"").expect("canvas panel");
        let problems = html.find("id=\"wb-problems\"").expect("problem card");
        let inspector = html
            .find("class=\"wb-inspector\"")
            .expect("inspector after canvas");
        assert!(canvas < problems && problems < inspector);
        assert_eq!(html.matches("id=\"wb-problems\"").count(), 1);

        let css = include_str!("../../styles.css");
        let rule_start = css.find(".wb-problems {").expect("problem-card rule");
        let rule_end = rule_start
            + css[rule_start..]
                .find('}')
                .expect("problem-card rule boundary");
        let rule = &css[rule_start..=rule_end];
        for declaration in [
            "position: absolute;",
            "bottom: 0.75rem;",
            "left: 0.75rem;",
            "pointer-events: none;",
        ] {
            assert!(rule.contains(declaration), "missing `{declaration}`");
        }
        for flow_declaration in ["grid-column:", "grid-row:", "min-height:"] {
            assert!(
                !rule.contains(flow_declaration),
                "overlay must not contribute `{flow_declaration}` to workbench layout"
            );
        }
        assert!(css.contains(
            "grid-template: 3.4rem minmax(0, 1fr) 1.8rem / 10.5rem 15rem minmax(36rem, 1fr) 18rem;"
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one table qualifies every production capture owner against every terminal route"
    )]
    fn canvas_pointer_capture_route_machine_has_exact_terminal_ownership() {
        use geosolve_constraint_editor::ActivePointerGestureKind;

        assert_eq!(
            CANVAS_POINTER_TERMINAL_EVENTS,
            ["pointerup", "pointercancel", "lostpointercapture"]
        );
        assert_eq!(
            CANVAS_PAN_POINTER_EVENTS,
            ["pointerdown", "pointermove", "pointerup"],
            "pointercancel has one centralized owner rather than a second pan listener"
        );
        assert_eq!(
            canvas_pointer_capture_kind(ActivePointerGestureKind::Point),
            CanvasPointerCaptureKind::Point
        );
        for fillet_kind in [
            ActivePointerGestureKind::FilletRadius,
            ActivePointerGestureKind::FilletContact,
        ] {
            assert_eq!(
                canvas_pointer_capture_kind(fillet_kind),
                CanvasPointerCaptureKind::Fillet,
                "radius and higher-priority contact overlap routes share exact Fillet capture"
            );
        }

        let terminals = [
            (
                CanvasPointerTerminal::PointerUp { pointer_id: 11 },
                CanvasPointerTerminalDisposition::Complete,
                true,
            ),
            (
                CanvasPointerTerminal::PointerCancel { pointer_id: 11 },
                CanvasPointerTerminalDisposition::Cancel,
                true,
            ),
            (
                CanvasPointerTerminal::LostPointerCapture { pointer_id: 11 },
                CanvasPointerTerminalDisposition::Cancel,
                false,
            ),
            (
                CanvasPointerTerminal::InteractionCancel,
                CanvasPointerTerminalDisposition::Cancel,
                true,
            ),
            (
                CanvasPointerTerminal::CameraCancel,
                CanvasPointerTerminalDisposition::Cancel,
                true,
            ),
        ];
        for kind in [
            CanvasPointerCaptureKind::Point,
            CanvasPointerCaptureKind::Fillet,
            CanvasPointerCaptureKind::Pan,
        ] {
            for (terminal, disposition, release_platform_capture) in terminals {
                let captured = CapturedCanvasPointer {
                    pointer_id: 11,
                    kind,
                };
                let mut route_machine = CanvasPointerCaptures::default();
                assert_eq!(
                    route_machine.ownership(11),
                    CanvasPointerOwnership::Uncaptured,
                    "uncaptured pointercancel remains available to cancel editor drafts"
                );
                assert!(route_machine.begin(captured));
                assert_eq!(route_machine.ownership(11), CanvasPointerOwnership::Owned);
                assert_eq!(route_machine.ownership(12), CanvasPointerOwnership::Foreign);
                assert!(
                    !route_machine.begin(CapturedCanvasPointer {
                        pointer_id: 12,
                        kind,
                    }),
                    "a foreign pointer cannot steal {kind:?} capture"
                );
                assert_eq!(
                    route_machine
                        .route_terminal(CanvasPointerTerminal::PointerCancel { pointer_id: 12 }),
                    None,
                    "a foreign terminal cannot release {kind:?} capture"
                );
                assert!(route_machine.contains(11));

                let route = route_machine
                    .route_terminal(terminal)
                    .expect("the owning terminal must route exactly once");
                assert_eq!(route.captured, captured);
                assert_eq!(route.disposition, disposition);
                assert_eq!(
                    route.release_platform_capture, release_platform_capture,
                    "lostpointercapture is already released by the browser"
                );
                assert!(route_machine.is_empty(), "{kind:?} capture must not strand");
                assert_eq!(
                    route_machine.route_terminal(terminal),
                    None,
                    "a repeated terminal cannot release {kind:?} twice"
                );
            }
        }

        let mut captures = CanvasPointerCaptures::default();
        assert!(!captures.begin(CapturedCanvasPointer {
            pointer_id: -1,
            kind: CanvasPointerCaptureKind::Point,
        }));
    }

    #[test]
    fn canvas_fillet_action_emphasis_requires_headless_preview_routing() {
        let css = include_str!("../../styles.css");
        assert!(css.contains(".wb-fillet-action.previewed .wb-fillet-retained-direction"));
        assert!(css.contains(".wb-fillet-action-hit"));
        assert!(css.contains("stroke-width: 24"));
        assert!(css.contains(
            ".wb-fillet-action[data-fillet-action-input=\"canvas\"]:focus { outline: none; }"
        ));
        let scene_source = include_str!("scene.rs");
        assert!(scene_source.contains("fill=\\\"context-stroke\\\""));
        for browser_owned_selector in [".wb-fillet-action:hover", ".wb-fillet-action:focus"] {
            assert!(
                !css.contains(browser_owned_selector),
                "{browser_owned_selector} would disagree with headless overlap priority"
            );
        }
    }

    #[test]
    fn canvas_pan_pointer_down_preserves_every_existing_capture() {
        let empty = CanvasPointerCaptures::default();
        assert_eq!(
            route_canvas_pan_pointer_down(&empty),
            CanvasPanPointerDownRoute::BeginPan
        );

        for kind in [
            CanvasPointerCaptureKind::Point,
            CanvasPointerCaptureKind::Fillet,
            CanvasPointerCaptureKind::Pan,
        ] {
            let mut route_machine = CanvasPointerCaptures::default();
            assert!(route_machine.begin(CapturedCanvasPointer {
                pointer_id: 11,
                kind,
            }));
            assert_eq!(
                route_canvas_pan_pointer_down(&route_machine),
                CanvasPanPointerDownRoute::PreserveCapturedInteraction,
                "foreign middle-button pointerdown must not steal {kind:?} capture"
            );
            assert!(route_machine.contains(11));
            assert_eq!(route_machine.ownership(12), CanvasPointerOwnership::Foreign);
        }
    }

    #[test]
    fn fillet_local_action_dom_keys_round_trip_semantic_normal_sides() {
        use geosolve_constraint_editor::SceneFilletActionId;

        for first in [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] {
            for second in [
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Right,
            ] {
                let action = SceneFilletActionId::LocalAlternative { first, second };
                let key = super::scene::fillet_action_key(action);
                assert_eq!(super::scene::fillet_action_from_key(&key), Some(action));
                assert!(key.contains(if first == DocumentCurveNormalSide::Left {
                    "left"
                } else {
                    "right"
                }));
            }
        }
        assert_eq!(
            super::scene::fillet_action_from_key("local-alternative-0"),
            None,
            "DOM identity must not regress to visible-list ordinals"
        );
    }

    fn grouped_fillet_fixture() -> (
        RetainedEditorCoordinator,
        [CurveSpan; 3],
        [DesignPointId; 4],
    ) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [6.0, 3.0]]
            .map(|position| document.add_point("corner point", position).expect("point"));
        let curve = document
            .add_curve(
                "corner support",
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
        (
            RetainedEditorCoordinator::new(session).expect("coordinator"),
            [
                CurveSpan { curve, segment: 0 },
                CurveSpan { curve, segment: 1 },
                CurveSpan { curve, segment: 2 },
            ],
            points,
        )
    }

    fn prepare_grouped_fillet(
        coordinator: &mut RetainedEditorCoordinator,
        state: &mut FeatureAuthoringState,
        corners: [DesignPointId; 2],
    ) -> (FeatureAuthoringCandidate, FeatureAuthoringPreviewMetadata) {
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current feature-authoring snapshot");
        let document = snapshot.sketch_document().clone();
        let mut picks = Vec::new();
        for corner in corners {
            picks.extend(
                coordinator
                    .feature_authoring_picks_for_item(SelectionItem::Point(corner), None)
                    .expect("expanded native corner picks"),
            );
        }
        let _ = state.activate(&snapshot, &document, FeatureAuthoringTool::Fillet, &[]);
        assert!(matches!(
            state.set_options(
                &snapshot,
                FeatureAuthoringOptions {
                    fillet_radius: Some(0.5),
                    ..FeatureAuthoringOptions::default()
                },
            ),
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        let outcome = state.pick_many(&snapshot, picks);
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate,
            guidance,
        } = outcome
        else {
            panic!("two complete corners should request one grouped preview: {outcome:?}");
        };
        assert_eq!(guidance.completed_corners, 2);
        let metadata = coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "Grouped Fillet",
            )
            .expect("prepare grouped computed preview");
        (candidate, metadata)
    }

    #[test]
    fn pointer_move_queue_keeps_only_latest_sample_and_terminal_invalidates_old_frame() {
        let input = |x| PointerInput {
            pointer_id: 7,
            position: ScreenPoint { x, y: 3.0 },
            modifiers: Modifiers::default(),
        };
        let mut queue = PointerMoveQueue::default();
        let first_frame = queue.push(input(1.0)).unwrap();
        assert_eq!(queue.push(input(2.0)), None);
        assert_eq!(queue.take_for_frame(first_frame), Some(input(2.0)));
        assert_eq!(queue.take_for_frame(first_frame), None);

        let failed_frame = queue.push(input(2.5)).unwrap();
        queue.cancel_frame(failed_frame);
        let retried_frame = queue.push(input(2.75)).unwrap();
        assert_ne!(retried_frame, failed_frame);
        assert_eq!(queue.take_for_frame(retried_frame), Some(input(2.75)));

        let stale_frame = queue.push(input(3.0)).unwrap();
        assert_eq!(queue.push(input(4.0)), None);
        assert_eq!(queue.drain_before_terminal(), Some(input(4.0)));
        let next_frame = queue.push(input(5.0)).unwrap();
        assert_ne!(next_frame, stale_frame);
        assert_eq!(queue.take_for_frame(stale_frame), None);
        assert_eq!(queue.take_for_frame(next_frame), Some(input(5.0)));

        let stale_before_action = queue.push(input(6.0)).unwrap();
        assert_eq!(queue.push(input(6.5)), None);
        queue.invalidate_before_immediate_action();
        assert_eq!(queue.take_for_frame(stale_before_action), None);
        let after_action = queue.push(input(7.0)).unwrap();
        assert_ne!(after_action, stale_before_action);
        assert_eq!(queue.take_for_frame(after_action), Some(input(7.0)));
    }

    #[test]
    fn fillet_action_render_authority_rejects_stale_dom_stamps_and_inputs() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, first) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let mut authority = FilletActionRenderAuthority::default();
        let first_stamp = authority
            .reconcile(Some(&first.input))
            .expect("first exact action render stamp");
        assert_eq!(authority.reconcile(Some(&first.input)), Some(first_stamp));
        assert!(authority.accepts(first_stamp, Some(&first.input)));

        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current authoring snapshot");
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate: changed, ..
        } = state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.7),
                ..state.options()
            },
        )
        else {
            panic!("radius change should refresh the complete Fillet batch");
        };
        let second = coordinator
            .refresh_feature_authoring_preview(first.input, &changed)
            .expect("new exact computed input");
        assert_ne!(second.input, first.input);
        assert!(
            !authority.accepts(first_stamp, Some(&second.input)),
            "a changed scene must not be upgraded through the old DOM stamp"
        );
        let second_stamp = authority
            .reconcile(Some(&second.input))
            .expect("replacement exact action render stamp");
        assert_ne!(second_stamp, first_stamp);
        assert!(!authority.accepts(first_stamp, Some(&first.input)));
        assert!(authority.accepts(second_stamp, Some(&second.input)));

        assert_eq!(authority.reconcile(None), None);
        assert!(!authority.accepts(second_stamp, Some(&second.input)));
    }

    #[test]
    fn option_inputs_and_selects_defer_render_to_their_change_owner() {
        for tag in ["INPUT", "SELECT", "OPTION"] {
            assert!(change_owns_option_control_click(tag, true, false, false));
            assert!(change_owns_option_control_click(tag, false, true, false));
            assert!(change_owns_option_control_click(tag, false, false, true));
        }
        for tag in ["BUTTON", "DETAILS", "LABEL", "SUMMARY"] {
            assert!(!change_owns_option_control_click(tag, true, true, true));
        }
        assert!(!change_owns_option_control_click(
            "INPUT", false, false, false
        ));
    }

    #[test]
    fn fillet_options_overlay_clamps_against_every_canvas_edge() {
        let canvas = OverlayRect {
            left: 300.0,
            top: 100.0,
            width: 800.0,
            height: 600.0,
        };
        let overlay = OverlayRect {
            left: 0.0,
            top: 0.0,
            width: 224.0,
            height: 260.0,
        };
        let left_palette_trigger = OverlayRect {
            left: 20.0,
            top: 240.0,
            width: 70.0,
            height: 44.0,
        };
        assert_eq!(
            canvas_overlay_position(left_palette_trigger, canvas, overlay, 8.0, 8.0),
            Some(ScreenPoint { x: 8.0, y: 140.0 })
        );

        let high_trigger = OverlayRect {
            left: 500.0,
            top: 50.0,
            width: 60.0,
            height: 30.0,
        };
        assert_eq!(
            canvas_overlay_position(high_trigger, canvas, overlay, 8.0, 8.0),
            Some(ScreenPoint { x: 268.0, y: 8.0 })
        );

        let right_trigger = OverlayRect {
            left: 1_100.0,
            top: 250.0,
            width: 60.0,
            height: 30.0,
        };
        assert_eq!(
            canvas_overlay_position(right_trigger, canvas, overlay, 8.0, 8.0),
            Some(ScreenPoint { x: 568.0, y: 150.0 })
        );

        let low_trigger = OverlayRect {
            left: 600.0,
            top: 690.0,
            width: 60.0,
            height: 30.0,
        };
        assert_eq!(
            canvas_overlay_position(low_trigger, canvas, overlay, 8.0, 8.0),
            Some(ScreenPoint { x: 368.0, y: 332.0 })
        );
    }

    #[test]
    fn fillet_options_overlay_reflows_after_palette_scroll_and_canvas_resize() {
        let overlay = OverlayRect {
            left: 0.0,
            top: 0.0,
            width: 224.0,
            height: 260.0,
        };
        let large_canvas = OverlayRect {
            left: 300.0,
            top: 100.0,
            width: 800.0,
            height: 600.0,
        };
        let trigger = OverlayRect {
            left: 600.0,
            top: 300.0,
            width: 60.0,
            height: 30.0,
        };
        assert_eq!(
            canvas_overlay_position(trigger, large_canvas, overlay, 8.0, 8.0),
            Some(ScreenPoint { x: 368.0, y: 200.0 })
        );

        let scrolled_trigger = OverlayRect {
            top: 40.0,
            ..trigger
        };
        assert_eq!(
            canvas_overlay_position(scrolled_trigger, large_canvas, overlay, 8.0, 8.0),
            Some(ScreenPoint { x: 368.0, y: 8.0 })
        );

        let resized_canvas = OverlayRect {
            width: 420.0,
            height: 320.0,
            ..large_canvas
        };
        assert_eq!(
            canvas_overlay_position(trigger, resized_canvas, overlay, 8.0, 8.0),
            Some(ScreenPoint { x: 188.0, y: 52.0 })
        );
    }

    #[test]
    fn fillet_options_overlay_captures_native_palette_details_reflow() {
        assert_eq!(palette_details_overlay_reflow_listener(), ("toggle", true));

        let html = include_str!("../../index.html");
        let palette_start = html
            .find("id=\"wb-tool-palette\"")
            .expect("tool palette markup");
        let palette = &html[palette_start..];
        let palette_end = palette.find("</aside>").expect("tool palette boundary");
        assert!(
            palette[..palette_end].contains("<details"),
            "the captured toggle policy must cover native palette disclosures"
        );
    }

    #[test]
    fn fillet_options_overlay_rejects_non_finite_layout_measurements() {
        assert_eq!(
            canvas_overlay_position(
                OverlayRect {
                    left: f64::NAN,
                    top: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                OverlayRect {
                    left: 0.0,
                    top: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                OverlayRect {
                    left: 0.0,
                    top: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                8.0,
                8.0,
            ),
            None
        );
    }

    #[test]
    fn canvas_authoring_click_sequence_contributes_one_operand_and_rearms_after_terminal_attempt() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let origin = document.add_point("origin", [0.0, 0.0]).unwrap();
        let first_tip = document.add_point("first tip", [2.0, 0.0]).unwrap();
        let second_tip = document.add_point("second tip", [0.0, 2.0]).unwrap();
        let first = SelectionItem::Curve(CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: origin,
                        end: first_tip,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        ));
        let second = SelectionItem::Curve(CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: origin,
                        end: second_tip,
                        branch_direction: [0.0, 1.0],
                    },
                )
                .unwrap(),
        ));

        let mut horizontal = AuthoringState::default();
        let _ = horizontal.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            &[],
        );
        let horizontal_outcomes = [
            AuthoringItemInput::CanvasPointerDown,
            AuthoringItemInput::CanvasClick,
        ]
        .into_iter()
        .filter(|input| owns_authoring_pick(*input))
        .map(|_| horizontal.pick(&document, AuthoringOperand::selected(first)))
        .collect::<Vec<_>>();
        assert_eq!(horizontal_outcomes.len(), 1);
        assert!(matches!(horizontal_outcomes[0], AuthoringOutcome::Apply(_)));
        horizontal.transaction_finished();
        assert!(horizontal.pending().is_empty());

        let mut normal = AuthoringState::default();
        let _ = normal.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Perpendicular),
            &[],
        );
        let mut normal_outcomes = Vec::new();
        for item in [first, second] {
            for input in [
                AuthoringItemInput::CanvasPointerDown,
                AuthoringItemInput::CanvasClick,
            ] {
                if owns_authoring_pick(input) {
                    normal_outcomes.push(normal.pick(&document, AuthoringOperand::selected(item)));
                }
            }
        }
        assert_eq!(normal_outcomes.len(), 2);
        assert!(matches!(
            normal_outcomes[0],
            AuthoringOutcome::Collecting { .. }
        ));
        assert!(matches!(normal_outcomes[1], AuthoringOutcome::Apply(_)));
        normal.transaction_finished();
        assert!(normal.pending().is_empty());

        assert!(owns_authoring_pick(AuthoringItemInput::TreeClick));
    }

    #[test]
    fn workbench_owns_no_direct_operations_companion_dependency() {
        let manifest = include_str!("../../Cargo.toml");
        assert!(manifest.contains("geosolve-constraint-editor ="));
        assert!(!manifest.contains("geosolve-sketch-ops ="));
    }

    #[test]
    fn restarted_or_incomplete_fillet_collection_revokes_an_older_preview() {
        let (mut coordinator, spans, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (candidate, _) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);

        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("feature-authoring snapshot");
        let document = snapshot.sketch_document().clone();
        let mut restarted = FeatureAuthoringState::default();
        let entered = restarted.activate(&snapshot, &document, FeatureAuthoringTool::Fillet, &[]);
        assert!(matches!(entered, FeatureAuthoringOutcome::ModeEntered(_)));
        observe_feature_authoring_preview_lifecycle(&mut coordinator, &entered);
        assert!(coordinator.feature_authoring_preview().is_none());

        coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "Rebuilt Fillet",
            )
            .expect("rebuilt preview");
        let pending_pick = coordinator
            .feature_authoring_picks_for_item(SelectionItem::Curve(spans[0]), None)
            .expect("one native pending pick");
        let collecting = state.pick_many(&snapshot, pending_pick);
        assert!(matches!(
            collecting,
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        observe_feature_authoring_preview_lifecycle(&mut coordinator, &collecting);
        assert!(coordinator.feature_authoring_preview().is_none());
    }

    #[test]
    fn fillet_clear_cancel_and_exit_purge_only_preview_owned_selection() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let owner = coordinator
            .feature_authoring_preview()
            .expect("preview")
            .corner_bindings()[0]
            .owner;
        coordinator.editor_mut().set_selection([
            SelectionItem::Point(points[0]),
            SelectionItem::Feature(metadata.feature),
            SelectionItem::FeatureCorner(owner),
        ]);
        let cleared = state.cancel();
        assert!(matches!(
            cleared,
            FeatureAuthoringOutcome::CandidateCleared(_)
        ));
        observe_feature_authoring_preview_lifecycle(&mut coordinator, &cleared);
        assert_eq!(
            coordinator.editor().selection(),
            &[SelectionItem::Point(points[0])]
        );

        let mut exited_state = FeatureAuthoringState::default();
        let (_, exited_metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut exited_state, [points[1], points[2]]);
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(exited_metadata.feature)]);
        observe_feature_authoring_preview_lifecycle(
            &mut coordinator,
            &FeatureAuthoringOutcome::ModeExited,
        );
        assert!(coordinator.editor().selection().is_empty());

        let mut cleared_state = FeatureAuthoringState::default();
        let (_, cleared_metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut cleared_state, [points[1], points[2]]);
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(cleared_metadata.feature)]);
        revoke_held_feature_authoring_preview(&mut coordinator);
        assert!(coordinator.feature_authoring_preview().is_none());
        assert!(coordinator.editor().selection().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end assertion binds exact Apply, sketch non-mutation and problem attribution"
    )]
    fn grouped_computed_fillet_apply_requires_the_exact_latest_held_preview() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let ordinary_identity = coordinator.session().design_identity();
        let ordinary_counts = {
            let document = coordinator.session().design_document();
            (
                document.points().len(),
                document.curves().len(),
                document.constraints().len(),
                document.dimensions().len(),
                document.contacts().len(),
                document.trim_views().len(),
            )
        };
        let mut state = FeatureAuthoringState::default();
        let (initial, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        assert_eq!(initial.corners().len(), 2);

        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current authoring snapshot");
        let changed = state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.7),
                ..state.options()
            },
        );
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate: changed, ..
        } = changed
        else {
            panic!("shared-radius edit should re-resolve the complete batch");
        };
        let refreshed = coordinator
            .refresh_feature_authoring_preview(metadata.input, &changed)
            .expect("fresh exact whole-batch preview");
        assert_ne!(refreshed.token, metadata.token);
        assert!(
            coordinator
                .apply_feature_authoring_preview(metadata.token, &changed)
                .is_err()
        );
        assert_eq!(
            coordinator
                .feature_authoring_preview()
                .expect("stale token retains the exact current preview")
                .metadata()
                .token,
            refreshed.token
        );
        let created = coordinator
            .apply_feature_authoring_preview(refreshed.token, &changed)
            .expect("latest candidate and token apply atomically");
        assert_eq!(coordinator.feature_document().features().len(), 1);
        assert!(
            coordinator
                .feature_document()
                .feature(created.value)
                .is_some()
        );
        assert_eq!(coordinator.session().design_identity(), ordinary_identity);
        let document = coordinator.session().design_document();
        assert_eq!(
            (
                document.points().len(),
                document.curves().len(),
                document.constraints().len(),
                document.dimensions().len(),
                document.contacts().len(),
                document.trim_views().len(),
            ),
            ordinary_counts,
            "ordinary Fillet authoring must not add M28 sketch graph objects"
        );
        let feature = coordinator
            .feature_document()
            .feature(created.value)
            .expect("created Fillet set");
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &feature.definition;
        let problem = geosolve_constraint_editor::ComputedFeatureProblemMetadata {
            feature: Some(feature.id),
            corners: vec![fillet.corners[0].id],
            sources: vec![fillet.corners[0].first.source],
            scope: geosolve_constraint_editor::EditorProblemScope::Targeted,
            message: "first corner failed".into(),
        };
        let tree = super::panels::tree_markup_with_features(
            document,
            coordinator.feature_document(),
            coordinator.computed_snapshot(),
            &[problem],
            &[],
            &[],
        );
        let row = |needle: &str| {
            let position = tree.find(needle).expect("feature tree row identity");
            let start = tree[..position]
                .rfind("<button")
                .expect("feature tree row start");
            let end = tree[position..]
                .find("</button>")
                .map(|offset| position + offset)
                .expect("feature tree row end");
            &tree[start..end]
        };
        assert!(row(&format!("data-feature-id=\"{}\"", feature.id)).contains("has-problem"));
        assert!(
            row(&format!(
                "data-feature-corner-id=\"{}\"",
                fillet.corners[0].id
            ))
            .contains("has-problem")
        );
        assert!(
            !row(&format!(
                "data-feature-corner-id=\"{}\"",
                fillet.corners[1].id
            ))
            .contains("has-problem")
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one presentation fixture proves exact-preview, shared-owner and accessible action rendering together"
    )]
    fn grouped_preview_renders_both_corner_arcs_with_feature_provenance() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let preview = coordinator
            .feature_authoring_preview()
            .expect("held grouped preview");
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted source");
        let viewport = Viewport::new([1000.0, 700.0], [3.0, 1.5], 80.0).expect("viewport");
        let mut scene = geosolve_constraint_editor::EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &coordinator
                .session()
                .accepted_prepared_input()
                .expect("current accepted grouped-preview input"),
            &metadata.input,
            preview.snapshot(),
            viewport,
            0.8,
        )
        .expect("exact grouped preview scene");
        assert_eq!(scene.computed_curves.len(), 2);
        let selected = SelectionItem::FeatureCorner(scene.computed_curves[0].owner);
        coordinator
            .populate_computed_fillet_affordances(
                &mut scene,
                &[SelectionItem::Feature(metadata.feature)],
                0.8,
            )
            .expect("grouped Fillet affordances");
        assert_eq!(scene.fillet_affordances.len(), 2);
        let markup = super::scene::svg_markup_with_context(
            Some(&scene),
            Some(accepted),
            &[selected],
            &[],
            EditorHoverState::default(),
            None,
            None,
            viewport,
        );
        assert_eq!(markup.matches("class=\"wb-computed-item").count(), 2);
        assert_eq!(
            markup
                .matches("class=\"wb-computed-item selected shared-radius-affected\"")
                .count(),
            1
        );
        assert_eq!(markup.matches("shared-radius-affected").count(), 2);
        assert_eq!(
            markup.matches("class=\"wb-fillet-radius-grip\"").count(),
            1,
            "the selected Fillet corner exposes one central radius handle"
        );
        assert!(
            !markup.contains("wb-fillet-contact"),
            "Fillet endpoint contact metadata must not render redundant canvas handles"
        );
        assert!(
            !markup.contains("wb-fillet-alternative-ghost"),
            "unpreviewed alternatives must not be painted as CSS-owned ghosts"
        );
        let preview_action = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .find(|action| {
                action.dashed_alternative_arc.is_some()
                    && matches!(
                        action.availability,
                        geosolve_constraint_editor::SceneFilletActionAvailability::Applicable
                    )
            })
            .expect("applicable branch alternative");
        let target = scene
            .fillet_action_target(preview_action.owner, preview_action.id)
            .expect("exact semantic action target");
        let (canvas_target, action_position) = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .filter(|action| {
                matches!(
                    action.availability,
                    geosolve_constraint_editor::SceneFilletActionAvailability::Applicable
                )
            })
            .find_map(|action| {
                let canvas_target = scene.fillet_action_target(action.owner, action.id)?;
                let mut positions = Vec::new();
                if let Some(control) = action.control_geometry {
                    positions.push(control.screen_end);
                    positions.push(geosolve_constraint_editor::ScreenPoint {
                        x: (control.screen_start.x + control.screen_end.x) * 0.5,
                        y: (control.screen_start.y + control.screen_end.y) * 0.5,
                    });
                }
                if let Some(geometry) = &action.dashed_alternative_arc {
                    positions.extend(geometry.screen_polyline.iter().copied());
                }
                positions
                    .into_iter()
                    .find(|position| {
                        scene.resolve_fillet_action(
                            geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                                position: *position,
                                painted: Some(canvas_target),
                            },
                            PickTolerance::default(),
                        ) == Some(canvas_target)
                    })
                    .map(|position| (canvas_target, position))
            })
            .expect("unoccluded branch action hit point");
        assert_eq!(
            scene.resolve_fillet_action(
                geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                    position: action_position,
                    painted: Some(canvas_target),
                },
                PickTolerance::default(),
            ),
            Some(canvas_target)
        );
        let overlapping_paint_order_target = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .filter_map(|action| scene.fillet_action_target(action.owner, action.id))
            .find(|target| *target != canvas_target)
            .expect("another painted action target");
        assert_eq!(
            resolve_canvas_fillet_action_candidates(
                &scene,
                action_position,
                [overlapping_paint_order_target, canvas_target],
            ),
            Some(canvas_target),
            "an overlapping topmost corridor must not suppress the headless nearest action"
        );
        assert_eq!(
            resolve_canvas_fillet_action_candidates(&scene, action_position, std::iter::empty(),),
            None,
            "an invalid DOM stamp must not be upgraded from current geometry"
        );
        let direct = scene
            .fillet_affordances
            .iter()
            .find(|affordances| affordances.owner == canvas_target.owner)
            .expect("selected corner affordances");
        for crowded in [
            direct.contacts[0].screen_position,
            direct.radius_rail.screen_grip,
        ] {
            assert_eq!(
                scene.resolve_fillet_action(
                    geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                        position: crowded,
                        painted: Some(canvas_target),
                    },
                    PickTolerance::default(),
                ),
                Some(canvas_target),
                "a painted and independently verified action must not start a Fillet drag"
            );
        }
        let action_stamp = 73;
        let preview_markup = super::scene::svg_markup_with_computed_context_and_action_stamp(
            Some(&scene),
            Some(accepted),
            &[],
            &[selected],
            &[],
            EditorHoverState::default(),
            None,
            None,
            Some(&target),
            Some(action_stamp),
            viewport,
        );
        assert_eq!(
            preview_markup
                .matches("class=\"wb-fillet-alternative-ghost\"")
                .count(),
            1,
            "only the editor's exact active preview may paint a ghost"
        );
        assert!(preview_markup.contains("wb-fillet-action previewed"));
        assert!(preview_markup.contains(&format!("data-fillet-action-stamp=\"{action_stamp}\"")));
        let panel = super::scene::fillet_action_panel_markup_with_stamp(&scene, Some(action_stamp));
        assert!(panel.contains("data-fillet-action-input=\"accessible\""));
        assert!(panel.contains(&format!("data-fillet-action-stamp=\"{action_stamp}\"")));
        assert!(panel.contains(&format!(
            "data-fillet-action=\"{}\"",
            super::scene::fillet_action_key(target.action)
        )));
        let mut second_only = scene.clone();
        let second_owner = second_only.fillet_affordances[1].owner;
        second_only
            .fillet_affordances
            .retain(|affordances| affordances.owner == second_owner);
        let disabled_action = second_only.fillet_affordances[0]
            .actions
            .first_mut()
            .expect("second corner action");
        disabled_action.availability =
            geosolve_constraint_editor::SceneFilletActionAvailability::Disabled {
                reason: "Retained <root> & rail unavailable".into(),
            };
        let disabled_key = super::scene::fillet_action_key(disabled_action.id);
        let reason_id = format!(
            "wb-fillet-action-reason-{}-{}-{disabled_key}",
            second_owner.feature, second_owner.corner,
        );
        let second_panel = super::scene::fillet_action_panel_markup_with_stamp(&second_only, None);
        assert!(second_panel.contains(&format!(
            "aria-label=\"Fillet corner {} actions\"",
            second_owner.corner,
        )));
        assert!(second_panel.contains(&format!(
            "<strong>Fillet corner {}</strong>",
            second_owner.corner,
        )));
        assert!(second_panel.contains(&format!("aria-describedby=\"{reason_id}\"")));
        assert!(second_panel.contains(&format!(
            "<small id=\"{reason_id}\" class=\"wb-fillet-action-reason\">Unavailable: Retained &lt;root&gt; &amp; rail unavailable</small>"
        )));
        assert!(
            !second_panel.contains("<strong>Corner 1</strong>"),
            "a filtered second corner must not be relabelled as the first persisted corner"
        );
        let retained = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .find_map(|action| action.control_geometry)
            .expect("retained-direction control geometry");
        assert!(preview_markup.contains(&format!(
            "L{:.3} {:.3}",
            retained.screen_end.x, retained.screen_end.y
        )));
        assert!(markup.contains(&format!("data-feature-id=\"{}\"", metadata.feature)));
        assert_eq!(markup.matches("data-computed-edge=").count(), 2);
        assert!(markup.contains("data-scene-provenance=\"accepted\""));
        assert!(markup.contains(&format!(
            "data-accepted-revision=\"{}\"",
            accepted.identity().revision().get()
        )));
        assert!(
            geometry_hover_selector(selected)
                .expect("computed hover selector")
                .contains("data-editor-item=\"feature-corner\"")
        );
        let css = include_str!("../../styles.css");
        for selector in [
            ".wb-computed-item:hover .wb-computed-fillet",
            ".wb-computed-item.geometry-hovered .wb-computed-fillet",
            ".wb-computed-item.selected .wb-computed-fillet",
        ] {
            assert!(
                css.contains(selector),
                "missing computed arc state selector"
            );
        }
    }

    #[test]
    fn native_authoring_hit_remains_available_at_a_computed_fillet_contact() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let preview = coordinator
            .feature_authoring_preview()
            .expect("held grouped preview");
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted source");
        let viewport = Viewport::new([1000.0, 700.0], [3.0, 1.5], 80.0).expect("viewport");
        let scene = geosolve_constraint_editor::EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &coordinator
                .session()
                .accepted_prepared_input()
                .expect("current accepted grouped-preview input"),
            &metadata.input,
            preview.snapshot(),
            viewport,
            0.8,
        )
        .expect("exact grouped preview scene");
        let (contact, source) = preview
            .snapshot()
            .edges()
            .iter()
            .find_map(|edge| match &edge.geometry {
                geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc) => {
                    Some((arc.contacts[0].position, arc.contacts[0].source.span))
                }
                _ => None,
            })
            .expect("computed Fillet contact");
        let hit = scene
            .native_authoring_hit_test(viewport.model_to_screen(contact), PickTolerance::default())
            .expect("native source remains authorable at the computed contact");
        assert_eq!(hit.item, SelectionItem::Curve(source));
        assert!(matches!(hit.item, SelectionItem::Curve(_)));
    }
}
