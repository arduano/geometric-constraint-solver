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
fn operation_apply_available(
    state: &geosolve_constraint_editor::OperationAuthoringState,
    coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
) -> bool {
    let Some(candidate) = state.candidate() else {
        return false;
    };
    state.candidate_confirmed()
        && candidate.is_confirmed()
        && coordinator.operation_preview().is_some_and(|preview| {
            preview.metadata().apply_ready && preview.matches_candidate(candidate)
        })
}

#[cfg(any(target_arch = "wasm32", test))]
fn operation_preview_reusable(
    candidate: &geosolve_constraint_editor::OperationAuthoringCandidate,
    coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
) -> bool {
    coordinator.operation_preview().is_some_and(|preview| {
        preview.matches_candidate(candidate)
            && preview.metadata().apply_ready == candidate.is_confirmed()
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn recover_operation_preview_failure(
    state: &mut geosolve_constraint_editor::OperationAuthoringState,
    candidate: &geosolve_constraint_editor::OperationAuthoringCandidate,
) -> bool {
    let retry_radius = !candidate.is_confirmed();
    state.preview_failed();
    retry_radius
}

#[cfg(any(target_arch = "wasm32", test))]
fn operation_canvas_hit(
    scene: &geosolve_constraint_editor::EditorScene,
    position: geosolve_constraint_editor::ScreenPoint,
    source: &geosolve_sketch::SketchDocument,
) -> Option<geosolve_constraint_editor::Hit> {
    scene.hit_test_for_document(
        position,
        geosolve_constraint_editor::PickTolerance::default(),
        source,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn operation_geometry_hover(
    scene: &geosolve_constraint_editor::EditorScene,
    position: geosolve_constraint_editor::ScreenPoint,
    source: &geosolve_sketch::SketchDocument,
) -> Option<geosolve_constraint_editor::SelectionItem> {
    operation_canvas_hit(scene, position, source).map(|hit| hit.item)
}

#[cfg(any(target_arch = "wasm32", test))]
const fn operation_stage_accepts_geometry(
    stage: geosolve_constraint_editor::OperationAuthoringStage,
) -> bool {
    matches!(
        stage,
        geosolve_constraint_editor::OperationAuthoringStage::PickFirstFilletCurve
            | geosolve_constraint_editor::OperationAuthoringStage::PickSecondFilletCurve
    )
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
        geosolve_constraint_editor::SelectionItem::Constraint(_)
        | geosolve_constraint_editor::SelectionItem::Dimension(_) => None,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn route_operation_canvas_pointer_down(
    state: &mut geosolve_constraint_editor::OperationAuthoringState,
    coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
    scene: &geosolve_constraint_editor::EditorScene,
    source: &geosolve_sketch::SketchDocument,
    position: geosolve_constraint_editor::ScreenPoint,
) -> geosolve_constraint_editor::OperationAuthoringOutcome {
    let model_position = scene.viewport.screen_to_model(position);
    let Some(tool) = state.active_tool() else {
        return geosolve_constraint_editor::OperationAuthoringOutcome::Inactive;
    };
    if !operation_stage_accepts_geometry(state.guidance().stage) {
        // Radius placement and preview review are positional/confirmation stages,
        // not later geometry picks. Give them exclusive ownership before querying
        // preview/source geometry so the live fillet arc cannot consume a click.
        return state.pointer_down_picks(source, &[], model_position);
    }
    let Some(hit) = operation_canvas_hit(scene, position, source) else {
        return state.pointer_down_picks(source, &[], model_position);
    };
    match coordinator.operation_picks_for_item(tool, hit.item, hit.curve_parameter) {
        Ok(picks) => state.pointer_down_picks(source, &picks, model_position),
        Err(_) => state.pick_item(source, hit.item, hit.curve_parameter),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn route_operation_item_pick(
    state: &mut geosolve_constraint_editor::OperationAuthoringState,
    document: &geosolve_sketch::SketchDocument,
    item: geosolve_constraint_editor::SelectionItem,
    curve_parameter: Option<f64>,
    stamped_picks: Option<Vec<geosolve_constraint_editor::OperationAuthoringPick>>,
) -> geosolve_constraint_editor::OperationAuthoringOutcome {
    match stamped_picks {
        Some(picks) => state.pick_many(document, picks),
        None => state.pick_item(document, item, curve_parameter),
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
        AuthoringState, AuthoringTool, BranchAction, ConicConstructionOptions, ConstructionPreview,
        CoordinatorActionKind, DimensionTargetDisplayUnit, DisabledReason, EditorEffect,
        EditorHoverState, EditorHoverTarget, EditorScene, EditorTool, Modifiers,
        NurbsConstructionOptions, OperationAuthoringOptions, OperationAuthoringOutcome,
        OperationAuthoringPreviewOutcome, OperationAuthoringState, OperationAuthoringTool,
        PickTolerance, PointerInput, ProvisionalInferenceCandidate, RetainedEditorCoordinator,
        SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        ContactBranchEdit, ContactDomain, ContactNeighborhood, CurveId, CurveSpan, DesignPointId,
        DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineForm, DocumentConstraintId,
        DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentDimensionId,
        DocumentDimensionMode, DocumentHyperbolaBranch, DocumentSolveRequest, PersistentId,
        RetainedSketchDocumentSession, SketchDocument, TangentOrientation,
    };
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::JsValue;
    use web_sys::{
        Document, Element, Event, HtmlElement, HtmlInputElement, HtmlSelectElement, KeyboardEvent,
        MouseEvent, PointerEvent, WheelEvent,
    };

    use super::persistence::{
        LEGACY_STORAGE_KEY, PREVIOUS_STORAGE_KEY, STORAGE_KEY, WorkspaceSnapshot,
    };

    struct Workbench {
        coordinator: RetainedEditorCoordinator,
        authoring: AuthoringState,
        operation_authoring: OperationAuthoringState,
        samples: super::samples::SampleCatalogState,
        camera: super::scene::CanvasCamera,
        pan_gesture: Option<PanGesture>,
        operation_pointer_position: Option<geosolve_constraint_editor::ScreenPoint>,
        fillet_options_open: bool,
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
            operation_authoring: OperationAuthoringState::default(),
            samples: super::samples::SampleCatalogState::default(),
            camera: super::scene::CanvasCamera::default(),
            pan_gesture: None,
            operation_pointer_position: None,
            fillet_options_open: false,
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
        for (key, _, tool) in super::action_surface::OPERATION_ACTIONS {
            let Some(button) =
                document.query_selector(&format!("[data-wb-operation=\"{key}\"]"))?
            else {
                continue;
            };
            let Some(icon) = button.query_selector(".wb-operation-icon")? else {
                continue;
            };
            icon.set_inner_html(&super::icons::operation_icon_markup(tool));
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
        RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())
    }

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
                    .closest(".wb-palette-flyout, .wb-operation-options")
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
                    "[data-wb-tool], [data-wb-authoring], [data-wb-operation], ",
                    "[data-editor-item], [data-wb-action], [data-sample-id], ",
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
                wb.operation_authoring.deactivate();
                wb.coordinator.clear_operation_preview();
                wb.operation_pointer_position = None;
                let effects = wb.coordinator.editor_mut().activate_tool(tool);
                dispatch_effects(&mut wb, effects);
                wb.notice = format!("{} tool active", super::icons::geometry_tool_key(tool));
            } else if let Some(tool) = target
                .get_attribute("data-wb-authoring")
                .and_then(|key| super::action_surface::authoring_tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                wb.operation_authoring.deactivate();
                wb.coordinator.clear_operation_preview();
                activate_authoring(&callback_document, &mut wb, tool);
            } else if let Some(tool) = target
                .get_attribute("data-wb-operation")
                .and_then(|key| super::action_surface::operation_tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                activate_operation_authoring(&callback_document, &mut wb, tool);
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
                    if wb.operation_authoring.active_tool().is_some() {
                        let input = if is_canvas_item {
                            super::AuthoringItemInput::CanvasClick
                        } else {
                            super::AuthoringItemInput::TreeClick
                        };
                        if super::owns_authoring_pick(input) {
                            handle_operation_item_pick(&mut wb, item, None);
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
                selected_sample = open_sample(&mut wb, &key);
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
                    .closest(".wb-operation-options")
                    .ok()
                    .flatten()
                    .is_some()
                {
                    let mut wb = change_workbench.borrow_mut();
                    update_operation_options(&change_document, &mut wb);
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
        Ok(())
    }

    fn install_canvas(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
    ) -> Result<(), JsValue> {
        let viewport = required(document, "wb-viewport")?;
        let pointer_moves = Rc::new(RefCell::new(super::PointerMoveQueue::default()));
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
        let cancel_pointer_moves = Rc::clone(&pointer_moves);
        let cancel = Closure::<dyn FnMut(PointerEvent)>::new(move |_event| {
            cancel_pointer_moves.borrow_mut().drain_before_terminal();
            let mut wb = cancel_workbench.borrow_mut();
            let effects = wb.coordinator.editor_mut().cancel();
            dispatch_effects(&mut wb, effects);
            wb.notice = "Interaction canceled".into();
            drop(wb);
            let _ = render(&cancel_document, &cancel_workbench);
        });
        viewport
            .add_event_listener_with_callback("pointercancel", cancel.as_ref().unchecked_ref())?;
        cancel.forget();

        let leave_document = document.clone();
        let leave_workbench = Rc::clone(workbench);
        let leave = Closure::<dyn FnMut(PointerEvent)>::new(move |_event| {
            let mut wb = leave_workbench.borrow_mut();
            let operation_hover_changed = wb.operation_pointer_position.take().is_some();
            let effects = wb.coordinator.editor_mut().pointer_leave();
            if effects.is_empty() && !operation_hover_changed {
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
            let input = {
                let wb = callback_workbench.borrow();
                if wb.pan_gesture.is_some() || wb.authoring.active_tool().is_some() {
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
                if wb.operation_authoring.active_tool().is_some() {
                    wb.operation_pointer_position = Some(input.position);
                    let model_position = scene.viewport.screen_to_model(input.position);
                    let Some(operation_document) = operation_document(&wb) else {
                        return;
                    };
                    let outcome = wb
                        .operation_authoring
                        .hover(&operation_document, model_position);
                    handle_operation_outcome(&mut wb, outcome);
                } else {
                    let effects = wb.coordinator.editor_mut().pointer_move(&scene, input);
                    dispatch_effects(&mut wb, effects);
                }
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
            if event_targets_problem_marker(&event) {
                return;
            }
            let mut wb = callback_workbench.borrow_mut();
            if wb.pan_gesture.is_some() {
                return;
            }
            let Some(scene) = editor_scene(&wb) else {
                return;
            };
            let Some(input) = pointer_input(&callback_viewport, scene.viewport, &event) else {
                return;
            };
            if wb.authoring.active_tool().is_some()
                || wb.operation_authoring.active_tool().is_some()
            {
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
            save(&wb);
            drop(wb);
            let _ = render(&callback_document, &callback_workbench);
        });
        viewport
            .add_event_listener_with_callback("pointerup", callback.as_ref().unchecked_ref())?;
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

    fn install_pan_listeners(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
        viewport: &Element,
    ) -> Result<(), JsValue> {
        for name in ["pointerdown", "pointermove", "pointerup", "pointercancel"] {
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
                    "pointerup" | "pointercancel"
                        if wb
                            .pan_gesture
                            .is_some_and(|gesture| gesture.pointer_id == event.pointer_id()) =>
                    {
                        event.prevent_default();
                        wb.pan_gesture = None;
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
            if event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[data-problem-marker]").ok().flatten())
                .is_some()
            {
                return;
            }
            let mut wb = callback_workbench.borrow_mut();
            if wb.pan_gesture.is_some() {
                return;
            }
            let Some(scene) = editor_scene(&wb) else {
                return;
            };
            let Some(input) = pointer_input(&callback_viewport, scene.viewport, &event) else {
                return;
            };
            if wb.operation_authoring.active_tool().is_some() {
                if event.button() != 0 {
                    return;
                }
                wb.operation_pointer_position = Some(input.position);
                let Some(operation_document) = operation_document(&wb) else {
                    wb.notice = "Helper operations require accepted geometry".into();
                    return;
                };
                let outcome = {
                    let Workbench {
                        coordinator,
                        operation_authoring,
                        ..
                    } = &mut *wb;
                    super::route_operation_canvas_pointer_down(
                        operation_authoring,
                        coordinator,
                        &scene,
                        &operation_document,
                        input.position,
                    )
                };
                handle_operation_outcome(&mut wb, outcome);
                save(&wb);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if wb.authoring.active_tool().is_some() {
                if event.button() != 0 {
                    return;
                }
                if super::owns_authoring_pick(super::AuthoringItemInput::CanvasPointerDown)
                    && let Some(hit) = scene.hit_test(input.position, PickTolerance::default())
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
            save(&wb);
            drop(wb);
            let _ = render(&callback_document, &callback_workbench);
        });
        viewport.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_keyboard(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if event.key() == "Escape"
                && required(&callback_document, "wb-sample-selector")
                    .is_ok_and(|selector| selector.has_attribute("open"))
            {
                event.prevent_default();
                close_sample_selector(&callback_document);
                focus_by_id(&callback_document, "wb-sample-trigger");
                return;
            }
            if event.key() == "Escape" && callback_workbench.borrow().fillet_options_open {
                event.prevent_default();
                callback_workbench.borrow_mut().fillet_options_open = false;
                let _ = render(&callback_document, &callback_workbench);
                focus_by_id(&callback_document, "wb-operation-fillet-options-trigger");
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
                if wb.operation_authoring.active_tool().is_some() {
                    handle_operation_item_pick(&mut wb, item, None);
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
            if event.key() == "Escape" && wb.operation_authoring.active_tool().is_some() {
                event.prevent_default();
                let outcome = wb.operation_authoring.cancel();
                handle_operation_outcome(&mut wb, outcome);
                drop(wb);
                let _ = render(&callback_document, &callback_workbench);
                return;
            }
            if event.key() == "Enter" && wb.operation_authoring.active_tool().is_some() {
                event.prevent_default();
                let outcome = wb.operation_authoring.enter();
                handle_operation_outcome(&mut wb, outcome);
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

    fn perform_action(document: &Document, wb: &mut Workbench, action: &str) {
        let result = match action {
            "new" => empty_coordinator().map(|coordinator| {
                wb.coordinator = coordinator;
                wb.authoring.deactivate();
                wb.operation_authoring.deactivate();
                wb.camera.reset();
                wb.operation_pointer_position = None;
                wb.fillet_options_open = false;
                wb.construction_preview = None;
                wb.inference_preview = None;
            }),
            "undo" => wb.coordinator.undo().map_err(|error| error.to_string()),
            "redo" => wb.coordinator.redo().map_err(|error| error.to_string()),
            "cancel" => {
                if wb.operation_authoring.active_tool().is_some() {
                    let outcome = wb.operation_authoring.cancel();
                    handle_operation_outcome(wb, outcome);
                } else if wb.authoring.active_tool().is_some() {
                    let document = wb.coordinator.session().design_document().clone();
                    let outcome = wb.authoring.cancel(&document);
                    handle_authoring_outcome(wb, outcome);
                } else {
                    let effects = wb.coordinator.editor_mut().cancel();
                    dispatch_effects(wb, effects);
                }
                Ok(())
            }
            "operation-apply" => {
                let outcome = wb.operation_authoring.apply();
                handle_operation_outcome(wb, outcome);
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
            "delete" => {
                let expected = wb.coordinator.session().design_identity();
                wb.coordinator
                    .delete_selected(expected)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
            "dimension-target" => apply_dimension_target(document, wb),
            "contact-branches" => apply_contact_branches(document, wb),
            "angle-orientation" => apply_angle_orientation(document, wb),
            "fillet-options" => {
                wb.fillet_options_open = !wb.fillet_options_open;
                Ok(())
            }
            "fillet-options-close" => {
                wb.fillet_options_open = false;
                Ok(())
            }
            "problems" => {
                wb.problems_open = !wb.problems_open;
                Ok(())
            }
            "zoom-in" => {
                wb.camera.zoom_about(
                    geosolve_constraint_editor::ScreenPoint { x: 500.0, y: 350.0 },
                    1.25,
                );
                Ok(())
            }
            "zoom-out" => {
                wb.camera.zoom_about(
                    geosolve_constraint_editor::ScreenPoint { x: 500.0, y: 350.0 },
                    0.8,
                );
                Ok(())
            }
            "zoom-fit" => {
                fit_camera(wb);
                Ok(())
            }
            _ => Ok(()),
        };
        if result.is_ok() && wb.authoring.active_tool().is_some() {
            let document = wb.coordinator.session().design_document().clone();
            let _ = wb.authoring.reconcile(&document);
        }
        if result.is_ok()
            && wb.operation_authoring.active_tool().is_some()
            && matches!(
                action,
                "undo"
                    | "redo"
                    | "delete"
                    | "dimension-target"
                    | "contact-branches"
                    | "angle-orientation"
            )
        {
            if let (Some(document), Some(input)) = (
                operation_document(wb),
                wb.coordinator.operation_authoring_input(),
            ) {
                let outcome = wb
                    .operation_authoring
                    .reconcile_exact_input(&document, input);
                handle_operation_outcome(wb, outcome);
            } else {
                wb.coordinator.clear_operation_preview();
                wb.operation_authoring.transaction_finished();
                wb.notice = "Workspace changed; select new helper-operation operands".into();
            }
        }
        wb.notice = result.map_or_else(
            |error| error,
            |()| match action {
                "problems"
                | "cancel"
                | "dimension-target"
                | "operation-apply"
                | "fillet-options"
                | "fillet-options-close" => wb.notice.clone(),
                _ => "Action retained".into(),
            },
        );
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

    fn open_sample(wb: &mut Workbench, key: &str) -> bool {
        match wb.samples.open_key(key) {
            Ok(coordinator) => {
                wb.coordinator = coordinator;
                wb.authoring.deactivate();
                wb.operation_authoring.deactivate();
                wb.operation_pointer_position = None;
                wb.fillet_options_open = false;
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
        wb.operation_pointer_position = None;
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
                    | SelectionItem::Dimension(_) => None,
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

    fn activate_operation_authoring(
        document: &Document,
        wb: &mut Workbench,
        tool: OperationAuthoringTool,
    ) {
        wb.operation_pointer_position = None;
        let Some(operation_document) = operation_document(wb) else {
            wb.notice = "Helper operations require current accepted geometry".into();
            return;
        };
        let options = match operation_options(document) {
            Ok(options) => options,
            Err(error) => {
                wb.coordinator.clear_operation_preview();
                wb.operation_authoring.deactivate();
                wb.notice = error;
                return;
            }
        };
        wb.coordinator.clear_operation_preview();
        let _ = wb
            .operation_authoring
            .set_options(&operation_document, options);
        let selection = match wb.coordinator.operation_authoring_preselection(tool) {
            Ok(selection) => selection,
            Err(error) => {
                wb.operation_authoring.deactivate();
                wb.notice =
                    format!("The current selection cannot start this helper operation: {error}");
                return;
            }
        };
        wb.authoring.deactivate();
        let effects = wb
            .coordinator
            .editor_mut()
            .activate_tool(EditorTool::Select);
        dispatch_effects(wb, effects);
        let outcome = wb
            .operation_authoring
            .activate(&operation_document, tool, &selection);
        handle_operation_outcome(wb, outcome);
    }

    fn handle_operation_item_pick(
        wb: &mut Workbench,
        item: SelectionItem,
        curve_parameter: Option<f64>,
    ) {
        let Some(operation_document) = operation_document(wb) else {
            wb.notice = "Helper operations require current accepted geometry".into();
            return;
        };
        let stamped_picks = wb.operation_authoring.active_tool().and_then(|tool| {
            wb.coordinator
                .operation_picks_for_item(tool, item, curve_parameter)
                .ok()
        });
        let outcome = super::route_operation_item_pick(
            &mut wb.operation_authoring,
            &operation_document,
            item,
            curve_parameter,
            stamped_picks,
        );
        handle_operation_outcome(wb, outcome);
    }

    fn update_operation_options(document: &Document, wb: &mut Workbench) {
        let options = match operation_options(document) {
            Ok(options) => options,
            Err(error) => {
                wb.coordinator.clear_operation_preview();
                wb.operation_authoring.preview_failed();
                wb.notice = error;
                return;
            }
        };
        let Some(operation_document) = operation_document(wb) else {
            wb.coordinator.clear_operation_preview();
            wb.operation_authoring.transaction_finished();
            wb.notice = "Helper operations require current accepted geometry".into();
            return;
        };
        let outcome = wb
            .operation_authoring
            .set_options(&operation_document, options);
        handle_operation_outcome(wb, outcome);
    }

    fn operation_options(document: &Document) -> Result<OperationAuthoringOptions, String> {
        Ok(OperationAuthoringOptions {
            fillet_radius: optional_positive_input(
                document,
                "wb-operation-fillet-radius",
                "fillet radius",
            )?,
            fillet_radius_mode: if select_value(document, "wb-operation-fillet-radius-mode")
                .as_deref()
                == Some("driving")
            {
                DocumentDimensionMode::Driving
            } else {
                DocumentDimensionMode::Reference
            },
            fillet_flip_first_side: input_checked(document, "wb-operation-fillet-flip-first"),
            fillet_flip_second_side: input_checked(document, "wb-operation-fillet-flip-second"),
            fillet_alternate_arc: input_checked(document, "wb-operation-fillet-alternate-arc"),
        })
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

    fn operation_document(wb: &Workbench) -> Option<SketchDocument> {
        wb.coordinator.operation_authoring_document().cloned()
    }

    fn handle_operation_outcome(wb: &mut Workbench, outcome: OperationAuthoringOutcome) {
        wb.coordinator.observe_operation_authoring_outcome(&outcome);
        match outcome {
            OperationAuthoringOutcome::ModeEntered(guidance) => {
                wb.notice = format!("{} · Escape exits", guidance.message);
            }
            OperationAuthoringOutcome::Collecting { guidance, .. } => {
                wb.notice = guidance.message.to_owned();
            }
            OperationAuthoringOutcome::PreviewRequested {
                candidate,
                guidance,
            } => {
                if super::operation_preview_reusable(&candidate, &wb.coordinator) {
                    let metadata = wb
                        .coordinator
                        .operation_preview()
                        .expect("reusable preview checked above")
                        .metadata()
                        .clone();
                    wb.notice = if metadata.apply_ready {
                        "Accepted preview ready · Apply or press Enter".into()
                    } else {
                        guidance.message.to_owned()
                    };
                    return;
                }
                match wb.coordinator.prepare_operation_preview(&candidate) {
                    Ok(OperationAuthoringPreviewOutcome::Ready(metadata)) => {
                        wb.notice = if metadata.apply_ready {
                            "Accepted preview ready · Apply or press Enter".into()
                        } else {
                            guidance.message.to_owned()
                        };
                    }
                    Ok(OperationAuthoringPreviewOutcome::Warning(warning)) => {
                        wb.coordinator.clear_operation_preview();
                        let retry_radius = super::recover_operation_preview_failure(
                            &mut wb.operation_authoring,
                            &candidate,
                        );
                        wb.notice = if retry_radius {
                            format!("{} · move to try another radius", warning.message)
                        } else {
                            format!("{} · select new operands to retry", warning.message)
                        };
                    }
                    Err(error) => {
                        wb.coordinator.clear_operation_preview();
                        let retry_radius = super::recover_operation_preview_failure(
                            &mut wb.operation_authoring,
                            &candidate,
                        );
                        wb.notice = if retry_radius {
                            format!(
                                "Operation preview failed: {error} · move to try another radius"
                            )
                        } else {
                            format!(
                                "Operation preview failed: {error} · select new operands to retry"
                            )
                        };
                    }
                }
            }
            OperationAuthoringOutcome::Apply(candidate) => {
                let ready = wb.coordinator.operation_preview().and_then(|preview| {
                    (preview.metadata().apply_ready && preview.matches_candidate(&candidate))
                        .then_some(preview.metadata().token)
                });
                let result = ready.map_or_else(
                    || Err("the accepted preview is no longer current".to_owned()),
                    |token| {
                        wb.coordinator
                            .apply_operation_preview(token, &candidate)
                            .map(|_| ())
                            .map_err(|error| error.to_string())
                    },
                );
                wb.coordinator.clear_operation_preview();
                wb.operation_authoring.transaction_finished();
                wb.notice = result.map_or_else(
                    |error| format!("Operation was not applied: {error} · select new operands"),
                    |()| "Operation accepted · select the next operands".into(),
                );
            }
            OperationAuthoringOutcome::Warning(warning) => {
                wb.notice = warning.message;
            }
            OperationAuthoringOutcome::CandidateCleared(guidance) => {
                wb.notice = format!("Operands cleared · {}", guidance.message);
            }
            OperationAuthoringOutcome::ModeExited => {
                wb.operation_pointer_position = None;
                wb.notice = "Helper operation authoring exited; Select active".into();
            }
            OperationAuthoringOutcome::Inactive => {}
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
        if let Some(preview) = coordinator.operation_preview() {
            return preview.scene(wb.camera.viewport(), 0.8).ok();
        }
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        let accepted = source.accepted_state()?;
        EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            wb.camera.viewport(),
            0.8,
        )
        .ok()
    }

    fn fit_camera(wb: &mut Workbench) {
        if let Some(scene) = editor_scene(wb) {
            wb.camera.fit_scene(&scene);
        }
    }

    fn render(document: &Document, workbench: &Rc<RefCell<Workbench>>) -> Result<(), JsValue> {
        let wb = workbench.borrow();
        let coordinator = &wb.coordinator;
        required(document, "workbench-root")?.set_attribute(
            "data-history-length",
            &coordinator.history_len().to_string(),
        )?;
        required(document, "workbench-root")?.set_attribute(
            "data-operation-preview",
            if coordinator.operation_preview().is_some() {
                "accepted"
            } else {
                "none"
            },
        )?;
        let scene = editor_scene(&wb);
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        let accepted = coordinator
            .operation_preview()
            .map(|preview| preview.accepted_state())
            .or_else(|| source.accepted_state());
        let selection = coordinator.editor().selection();
        let mut pending = wb
            .authoring
            .pending()
            .iter()
            .map(|operand| operand.item)
            .collect::<Vec<_>>();
        pending.extend(wb.operation_authoring.picks().iter().map(|pick| pick.item));
        if let (Some(scene), Some(preview)) = (scene.as_ref(), coordinator.operation_preview()) {
            pending.extend(
                scene
                    .curves
                    .iter()
                    .filter(|curve| {
                        preview
                            .metadata()
                            .created_curves
                            .contains(&curve.span.curve)
                    })
                    .map(|curve| SelectionItem::Curve(curve.span)),
            );
            pending.extend(
                scene
                    .points
                    .iter()
                    .filter(|point| preview.metadata().created_points.contains(&point.id))
                    .map(|point| SelectionItem::Point(point.id)),
            );
        }
        pending.sort_unstable();
        pending.dedup();
        let construction_preview = wb.construction_preview.as_ref();
        let operation_hover = if wb.operation_authoring.active_tool().is_some()
            && super::operation_stage_accepts_geometry(wb.operation_authoring.guidance().stage)
        {
            scene.as_ref().and_then(|scene| {
                wb.operation_pointer_position.and_then(|position| {
                    coordinator
                        .operation_authoring_document()
                        .and_then(|source| super::operation_geometry_hover(scene, position, source))
                })
            })
        } else {
            None
        };
        let hover = if wb.operation_authoring.active_tool().is_some() {
            EditorHoverState {
                target: operation_hover.map(EditorHoverTarget::Geometry),
                context_owner: None,
            }
        } else {
            coordinator.editor().hover_state()
        };
        required(document, "wb-viewport")?.set_inner_html(&super::scene::svg_markup_with_context(
            scene.as_ref(),
            accepted,
            selection,
            &pending,
            hover,
            construction_preview,
            coordinator.current_problem_metadata().as_ref(),
            wb.camera.viewport(),
        ));
        mark_geometry_hover(
            document,
            hover.target.and_then(|target| match target {
                EditorHoverTarget::Geometry(item) => Some(item),
                EditorHoverTarget::Annotation(_) => None,
            }),
        )?;
        let design = coordinator.session().design_document();
        required(document, "wb-tree")?.set_inner_html(&super::panels::tree_markup_with_pending(
            design, selection, &pending,
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
        let problem = problem_text(coordinator);
        required(document, "wb-problem-text")?
            .set_inner_html(&super::panels::problem_markup(&problem));
        required(document, "wb-redundancy")?.set_inner_html(
            &super::panels::accepted_redundancy_markup(coordinator.accepted_redundancy()),
        );
        required(document, "wb-host-state")?
            .set_inner_html(&super::panels::host_state_markup(coordinator.session()));
        required(document, "wb-production-topology")?.set_inner_html(
            &super::panels::production_topology_markup(coordinator.session()),
        );
        render_sample_ui(document, &wb.samples)?;
        let problems = required(document, "wb-problems")?;
        if wb.problems_open || problem != "No current solver problem" {
            problems.remove_attribute("hidden")?;
        } else {
            problems.set_attribute("hidden", "")?;
        }
        let guide = required(document, "wb-draft-guide")?;
        if wb.authoring.active_tool().is_some()
            || wb.operation_authoring.active_tool().is_some()
            || coordinator.editor().can_complete_draft()
            || wb.construction_preview.is_some()
        {
            guide.remove_attribute("hidden")?;
        } else {
            guide.set_attribute("hidden", "")?;
        }
        let guide_text = if wb.operation_authoring.active_tool().is_some() {
            wb.operation_authoring.guidance().message.to_owned()
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
        if wb.authoring.active_tool().is_some() || wb.operation_authoring.active_tool().is_some() {
            required(document, "wb-guide-finish")?.set_attribute("hidden", "")?;
        } else {
            required(document, "wb-guide-finish")?.remove_attribute("hidden")?;
        }
        let apply = required(document, "wb-guide-apply")?;
        if super::operation_apply_available(&wb.operation_authoring, coordinator) {
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
        render_action_availability(
            document,
            coordinator,
            &wb.authoring,
            &wb.operation_authoring,
        )?;
        render_operation_options(document, &wb.operation_authoring)?;
        render_fillet_options_overlay(document, wb.fillet_options_open)?;
        render_dimension_target_editor(document, coordinator)?;
        render_branch_editor(document, coordinator)?;
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
        let trigger = required(document, "wb-operation-fillet-options-trigger")?;
        trigger.set_attribute("aria-expanded", if open { "true" } else { "false" })?;
        let overlay = required(document, "wb-operation-options-overlay")?;
        if !open {
            overlay.set_attribute("hidden", "")?;
            overlay.remove_attribute("style")?;
            return Ok(());
        }
        overlay.remove_attribute("hidden")?;
        reposition_fillet_options_overlay(document)
    }

    fn reposition_fillet_options_overlay(document: &Document) -> Result<(), JsValue> {
        let overlay = required(document, "wb-operation-options-overlay")?;
        if overlay.has_attribute("hidden") {
            return Ok(());
        }
        let trigger_rect =
            required(document, "wb-operation-fillet-trigger")?.get_bounding_client_rect();
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
        operation_authoring: &OperationAuthoringState,
    ) -> Result<(), JsValue> {
        for key in ["new", "finish", "cancel", "clear-selection"] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_disabled(
                    &button,
                    key == "finish"
                        && (authoring.active_tool().is_some()
                            || operation_authoring.active_tool().is_some()),
                )?;
            }
        }
        let actions = coordinator.actions();
        let state = |action| {
            actions
                .iter()
                .find(|value| value.action == action)
                .map(|value| value.state)
                .unwrap_or(ActionState::Disabled(DisabledReason::WrongOperandKind))
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
        for (key, _, tool) in super::action_surface::OPERATION_ACTIONS {
            if let Some(button) =
                document.query_selector(&format!("[data-wb-operation=\"{key}\"]"))?
            {
                button.set_attribute(
                    "aria-pressed",
                    if operation_authoring.active_tool() == Some(tool) {
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
            "wb-operation-fillet-radius",
            "wb-operation-fillet-radius-mode",
            "wb-operation-fillet-flip-first",
            "wb-operation-fillet-flip-second",
            "wb-operation-fillet-alternate-arc",
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

    fn render_operation_options(
        document: &Document,
        state: &OperationAuthoringState,
    ) -> Result<(), JsValue> {
        let options = state.options();
        render_optional_number(
            document,
            "wb-operation-fillet-radius",
            options.fillet_radius,
        )?;
        if let Ok(select) =
            required(document, "wb-operation-fillet-radius-mode")?.dyn_into::<HtmlSelectElement>()
        {
            select.set_value(match options.fillet_radius_mode {
                DocumentDimensionMode::Driving => "driving",
                DocumentDimensionMode::Reference => "reference",
            });
        }
        for (id, checked) in [
            (
                "wb-operation-fillet-flip-first",
                options.fillet_flip_first_side,
            ),
            (
                "wb-operation-fillet-flip-second",
                options.fillet_flip_second_side,
            ),
            (
                "wb-operation-fillet-alternate-arc",
                options.fillet_alternate_arc,
            ),
        ] {
            if let Ok(input) = required(document, id)?.dyn_into::<HtmlInputElement>() {
                input.set_checked(checked);
            }
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

    fn problem_text(coordinator: &RetainedEditorCoordinator) -> String {
        coordinator.current_problem_metadata().map_or_else(
            || "No current solver problem".into(),
            |problem| problem.message,
        )
    }

    fn save(wb: &Workbench) {
        let snapshot = WorkspaceSnapshot::from_checkpoint(wb.coordinator.checkpoint());
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
        let id = PersistentId::from_str(&target.get_attribute("data-persistent-id")?).ok()?;
        match target.get_attribute("data-editor-item")?.as_str() {
            "point" => Some(SelectionItem::Point(DesignPointId(id))),
            "curve" => Some(SelectionItem::Curve(CurveSpan {
                curve: CurveId(id),
                segment: target.get_attribute("data-editor-segment")?.parse().ok()?,
            })),
            "constraint" => Some(SelectionItem::Constraint(DocumentConstraintId(id))),
            "dimension" => Some(SelectionItem::Dimension(DocumentDimensionId(id))),
            _ => None,
        }
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

    fn input_checked(document: &Document, id: &str) -> bool {
        document
            .get_element_by_id(id)
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
            .is_some_and(|input| input.checked())
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
        EditorHoverState, Modifiers, OperationAuthoringCandidate, OperationAuthoringOutcome,
        OperationAuthoringPreviewMetadata, OperationAuthoringPreviewOutcome,
        OperationAuthoringState, OperationAuthoringTool, PickTolerance, PointerInput,
        RetainedEditorCoordinator, ScreenPoint, SelectionItem, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        CurveDefinition, CurveSpan, DesignPointId, DocumentEdit, DocumentSolveRequest,
        RetainedSketchDocumentSession, SketchDocument,
    };

    use super::{
        AuthoringItemInput, OverlayRect, PointerMoveQueue, canvas_overlay_position,
        change_owns_option_control_click, geometry_hover_selector, operation_apply_available,
        operation_canvas_hit, operation_geometry_hover, operation_preview_reusable,
        operation_stage_accepts_geometry, owns_authoring_pick,
        palette_details_overlay_reflow_listener, recover_operation_preview_failure,
        route_operation_canvas_pointer_down, route_operation_item_pick,
    };

    fn fillet_operation_fixture() -> (
        RetainedEditorCoordinator,
        [CurveSpan; 2],
        [DesignPointId; 3],
    ) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0]]
            .map(|position| document.add_point("corner point", position).expect("point"));
        let curve = document
            .add_curve(
                "corner support",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
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
            ],
            points,
        )
    }

    fn prepare_confirmed_fillet(
        coordinator: &mut RetainedEditorCoordinator,
        state: &mut OperationAuthoringState,
        corner: DesignPointId,
    ) -> (
        OperationAuthoringCandidate,
        OperationAuthoringPreviewMetadata,
    ) {
        let document = coordinator
            .operation_authoring_document()
            .expect("accepted operation document")
            .clone();
        let picks = coordinator
            .operation_picks_for_item(
                OperationAuthoringTool::Fillet,
                SelectionItem::Point(corner),
                None,
            )
            .expect("expanded corner picks");
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let activated = state.pick_many(&document, picks);
        coordinator.observe_operation_authoring_outcome(&activated);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = activated else {
            panic!("fillet corner should request a radius preview");
        };
        let unconfirmed = coordinator
            .prepare_operation_preview(&candidate)
            .expect("prepare default-radius preview");
        assert!(matches!(
            unconfirmed,
            OperationAuthoringPreviewOutcome::Ready(metadata) if !metadata.apply_ready
        ));
        let confirmed = state.confirm(&document, [1.8, 0.2]);
        coordinator.observe_operation_authoring_outcome(&confirmed);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = confirmed else {
            panic!("radius confirmation should request a preview");
        };
        let prepared = coordinator
            .prepare_operation_preview(&candidate)
            .expect("prepare confirmed preview");
        let OperationAuthoringPreviewOutcome::Ready(metadata) = prepared else {
            panic!("confirmed fillet preview should be accepted");
        };
        assert!(metadata.apply_ready);
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
    #[allow(
        clippy::too_many_lines,
        reason = "exact preview token, candidate, warning, and stale-input gates form one lifecycle"
    )]
    fn operation_apply_requires_the_exact_current_accepted_preview() {
        let (mut coordinator, spans, points) = fillet_operation_fixture();
        let mut state = OperationAuthoringState::default();
        let (_, _) = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        assert!(operation_apply_available(&state, &coordinator));

        let document = coordinator
            .operation_authoring_document()
            .expect("operation document")
            .clone();
        let mut options = state.options();
        options.fillet_alternate_arc = !options.fillet_alternate_arc;
        let changed = state.set_options(&document, options);
        coordinator.observe_operation_authoring_outcome(&changed);
        assert!(matches!(
            changed,
            OperationAuthoringOutcome::PreviewRequested { .. }
        ));
        assert!(coordinator.operation_preview().is_some());
        assert!(
            !operation_apply_available(&state, &coordinator),
            "an older accepted token must not enable Apply for a changed candidate"
        );
        assert!(matches!(
            coordinator.apply_operation_preview(
                coordinator
                    .operation_preview()
                    .expect("old preview")
                    .metadata()
                    .token,
                state.candidate().expect("changed candidate")
            ),
            Err(geosolve_constraint_editor::CoordinatorError::OperationPreviewMismatch)
        ));
        assert!(coordinator.operation_preview().is_none());

        let (_, _) = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        assert!(operation_apply_available(&state, &coordinator));
        let cancelled = state.cancel();
        coordinator.observe_operation_authoring_outcome(&cancelled);
        assert!(matches!(
            cancelled,
            OperationAuthoringOutcome::CandidateCleared(_)
        ));
        assert!(coordinator.operation_preview().is_none());
        assert!(!operation_apply_available(&state, &coordinator));

        let (_, _) = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        let warning = state.pointer_down(&document, None, [f64::NAN, 0.0]);
        coordinator.observe_operation_authoring_outcome(&warning);
        assert!(matches!(warning, OperationAuthoringOutcome::Warning(_)));
        assert!(coordinator.operation_preview().is_none());
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
        assert!(!operation_apply_available(&state, &coordinator));

        let source_pick = coordinator
            .operation_pick_for_item(SelectionItem::Curve(spans[0]), Some(0.5))
            .expect("fillet parent pick");
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        assert!(matches!(
            state.pick(&document, source_pick.clone()),
            OperationAuthoringOutcome::Collecting { .. }
        ));
        let duplicate = state.pick(&document, source_pick.clone());
        coordinator.observe_operation_authoring_outcome(&duplicate);
        assert!(matches!(duplicate, OperationAuthoringOutcome::Warning(_)));
        assert_eq!(
            state.picks(),
            &[source_pick],
            "a correctable second-pick warning must retain the valid prefix"
        );
        state.transaction_finished();

        let (_, _) = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        let switched = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        coordinator.observe_operation_authoring_outcome(&switched);
        assert!(matches!(
            switched,
            OperationAuthoringOutcome::ModeEntered(_)
        ));
        assert!(coordinator.operation_preview().is_none());

        let (_, _) = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        let changed = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "external edit".into(),
                    position: [8.0, 3.0],
                },
            )
            .expect("accepted external edit");
        assert!(changed.published_accepted.is_some());
        let current_document = coordinator
            .operation_authoring_document()
            .expect("current operation document")
            .clone();
        let current_input = coordinator
            .operation_authoring_input()
            .expect("current operation input");
        let reconciled = state.reconcile_exact_input(&current_document, current_input);
        coordinator.observe_operation_authoring_outcome(&reconciled);
        assert!(matches!(
            reconciled,
            OperationAuthoringOutcome::Warning(ref warning)
                if warning.kind
                    == geosolve_constraint_editor::OperationAuthoringWarningKind::StalePick
        ));
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
        assert!(coordinator.operation_preview().is_none());
    }

    #[test]
    fn rejected_operation_item_pick_revokes_confirmed_preview_but_retains_valid_prefix() {
        let (mut coordinator, spans, points) = fillet_operation_fixture();
        let mut state = OperationAuthoringState::default();
        let _ = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        assert!(state.candidate_confirmed());
        assert!(coordinator.operation_preview().is_some());

        let document = coordinator
            .operation_authoring_document()
            .expect("operation document")
            .clone();
        let invalid_confirmed = route_operation_item_pick(
            &mut state,
            &document,
            SelectionItem::Point(points[0]),
            None,
            None,
        );
        coordinator.observe_operation_authoring_outcome(&invalid_confirmed);
        assert!(matches!(
            invalid_confirmed,
            OperationAuthoringOutcome::Warning(ref warning)
                if warning.kind
                    == geosolve_constraint_editor::OperationAuthoringWarningKind::FilletCornerNotInterior
        ));
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
        assert!(coordinator.operation_preview().is_none());

        let source_pick = coordinator
            .operation_pick_for_item(SelectionItem::Curve(spans[0]), Some(0.5))
            .expect("stamped fillet parent");
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let collecting = route_operation_item_pick(
            &mut state,
            &document,
            SelectionItem::Curve(spans[0]),
            Some(0.5),
            Some(vec![source_pick.clone()]),
        );
        coordinator.observe_operation_authoring_outcome(&collecting);
        assert!(matches!(
            collecting,
            OperationAuthoringOutcome::Collecting { .. }
        ));

        let invalid_second = route_operation_item_pick(
            &mut state,
            &document,
            SelectionItem::Point(points[0]),
            None,
            None,
        );
        coordinator.observe_operation_authoring_outcome(&invalid_second);
        assert!(matches!(
            invalid_second,
            OperationAuthoringOutcome::Warning(ref warning)
                if warning.kind
                    == geosolve_constraint_editor::OperationAuthoringWarningKind::FilletCornerNotInterior
        ));
        assert_eq!(state.picks(), &[source_pick]);
        assert!(state.candidate().is_none());
        assert!(coordinator.operation_preview().is_none());
    }

    #[test]
    fn accepted_operation_preview_renders_with_its_own_provenance() {
        let (mut coordinator, _, points) = fillet_operation_fixture();
        let mut state = OperationAuthoringState::default();
        let (_, metadata) = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        let preview = coordinator.operation_preview().expect("accepted preview");
        let viewport = Viewport::new([1000.0, 700.0], [2.0, 1.0], 50.0).expect("viewport");
        let scene = preview.scene(viewport, 0.8).expect("preview scene");
        let markup = super::scene::svg_markup_with_context(
            Some(&scene),
            Some(preview.accepted_state()),
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            viewport,
        );
        assert!(markup.contains("data-scene-provenance=\"accepted\""));
        assert!(markup.contains(&format!(
            "data-accepted-revision=\"{}\"",
            metadata.accepted.revision().get()
        )));
        assert!(markup.contains(&metadata.primary_created_curve.0.to_string()));
    }

    #[test]
    fn one_physical_canvas_click_contributes_one_operation_transition() {
        let (coordinator, spans, _) = fillet_operation_fixture();
        let document = coordinator
            .operation_authoring_document()
            .expect("operation document")
            .clone();
        let pick = coordinator
            .operation_pick_for_item(SelectionItem::Curve(spans[0]), Some(0.5))
            .expect("pick");
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let outcomes = [
            AuthoringItemInput::CanvasPointerDown,
            AuthoringItemInput::CanvasClick,
        ]
        .into_iter()
        .filter(|input| owns_authoring_pick(*input))
        .map(|_| state.pointer_down(&document, Some(pick.clone()), [2.0, 0.0]))
        .collect::<Vec<_>>();
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(
            outcomes[0],
            OperationAuthoringOutcome::Collecting { .. }
        ));
        assert_eq!(state.picks().len(), 1);
        assert!(!state.candidate_confirmed());
    }

    #[test]
    fn web_preview_failure_policy_retries_unconfirmed_radius_but_ends_confirmed_failure() {
        let (coordinator, _, points) = fillet_operation_fixture();
        let document = coordinator
            .operation_authoring_document()
            .expect("operation document")
            .clone();
        let picks = coordinator
            .operation_picks_for_item(
                OperationAuthoringTool::Fillet,
                SelectionItem::Point(points[1]),
                None,
            )
            .expect("expanded corner picks");
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } =
            state.pick_many(&document, picks)
        else {
            panic!("two parents must request the initial radius preview");
        };
        assert!(recover_operation_preview_failure(&mut state, &candidate));
        assert_eq!(state.picks().len(), 2);
        assert!(state.candidate().is_none());
        assert_eq!(
            state.guidance().stage,
            geosolve_constraint_editor::OperationAuthoringStage::PlaceFilletRadius
        );

        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } =
            state.confirm(&document, [1.8, 0.2])
        else {
            panic!("a subsequent valid radius must rebuild a confirmed preview");
        };
        assert!(!recover_operation_preview_failure(&mut state, &candidate));
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "corner expansion, live preview hit, and shared canvas routing form one regression"
    )]
    fn fillet_preview_arc_cannot_intercept_its_radius_confirmation_click() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0]]
            .map(|position| document.add_point("corner point", position).expect("point"));
        let curve = document
            .add_curve(
                "corner support",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
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
        let operation_document = coordinator
            .operation_authoring_document()
            .expect("accepted operation document")
            .clone();
        let picks = coordinator
            .operation_picks_for_item(
                OperationAuthoringTool::Fillet,
                SelectionItem::Point(points[1]),
                None,
            )
            .expect("expanded corner picks");
        assert_eq!(picks.len(), 2);

        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&operation_document, OperationAuthoringTool::Fillet, &[]);
        let routed = route_operation_item_pick(
            &mut state,
            &operation_document,
            SelectionItem::Point(points[1]),
            None,
            Some(picks),
        );
        coordinator.observe_operation_authoring_outcome(&routed);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = routed else {
            panic!("one routed corner item must stage a fillet preview");
        };
        assert_eq!(state.picks().len(), 2);
        assert_eq!(
            state.picks()[0].curve_span(),
            Some(CurveSpan { curve, segment: 0 })
        );
        assert_eq!(
            state.picks()[1].curve_span(),
            Some(CurveSpan { curve, segment: 1 })
        );
        assert!(!candidate.is_confirmed());
        assert!(!state.candidate_confirmed());

        let radius_position = [1.8, 0.2];
        let hovered = state.hover(&operation_document, radius_position);
        coordinator.observe_operation_authoring_outcome(&hovered);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = hovered else {
            panic!("valid radius hover must update the fillet preview: {hovered:?}");
        };
        let OperationAuthoringPreviewOutcome::Ready(metadata) = coordinator
            .prepare_operation_preview(&candidate)
            .expect("accepted fillet preview")
        else {
            panic!("fillet preview must be accepted");
        };
        let viewport = Viewport::new([800.0, 600.0], [1.0, 1.0], 100.0).expect("viewport");
        let preview_scene = coordinator
            .operation_preview()
            .expect("held fillet preview")
            .scene(viewport, 0.8)
            .expect("fillet preview scene");
        let preview_curve = preview_scene
            .curves
            .iter()
            .find(|curve| curve.span.curve == metadata.primary_created_curve)
            .expect("preview fillet curve");
        let preview_position = viewport.model_to_screen(radius_position);
        assert!(matches!(
            preview_scene.hit_test(preview_position, PickTolerance::default()),
            Some(hit) if hit.item == SelectionItem::Curve(preview_curve.span)
        ));
        assert_eq!(
            operation_canvas_hit(&preview_scene, preview_position, &operation_document),
            None,
            "a preview-only arc is a blank operation operand, so this click places the radius"
        );
        assert_eq!(
            operation_geometry_hover(&preview_scene, preview_position, &operation_document),
            None,
            "the same preview foreground is also a hover barrier"
        );

        let confirmed = route_operation_canvas_pointer_down(
            &mut state,
            &coordinator,
            &preview_scene,
            &operation_document,
            preview_position,
        );
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = confirmed else {
            panic!("preview-arc radius placement must confirm the fillet: {confirmed:?}");
        };
        assert!(candidate.is_confirmed());
        assert!(state.candidate_confirmed());
    }

    #[test]
    fn operation_geometry_hover_matches_click_at_the_exact_curve_tolerance() {
        let (coordinator, spans, _) = fillet_operation_fixture();
        let operation_document = coordinator
            .operation_authoring_document()
            .expect("accepted operation document")
            .clone();
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted source");
        let viewport = Viewport::new([800.0, 600.0], [1.0, 1.0], 100.0).expect("viewport");
        let scene = geosolve_constraint_editor::EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport,
            0.8,
        )
        .expect("source scene");
        let curve = scene
            .curves
            .iter()
            .find(|curve| curve.span == spans[0])
            .expect("first parent curve");
        let first = curve
            .screen_polyline
            .first()
            .copied()
            .expect("first sample");
        let last = curve.screen_polyline.last().copied().expect("last sample");
        let tolerance = PickTolerance::default().curve_pixels;
        let boundary = ScreenPoint {
            x: 0.5 * (first.x + last.x),
            y: 0.5 * (first.y + last.y) + tolerance,
        };
        let click = operation_canvas_hit(&scene, boundary, &operation_document)
            .expect("the inclusive click boundary must acquire the line");
        assert_eq!(click.item, SelectionItem::Curve(spans[0]));
        assert_eq!(
            operation_geometry_hover(&scene, boundary, &operation_document),
            Some(click.item)
        );
        assert!(
            geometry_hover_selector(click.item)
                .expect("geometry selector")
                .contains(&spans[0].curve.to_string())
        );
        let outside = ScreenPoint {
            y: boundary.y + 1.0e-6,
            ..boundary
        };
        assert_eq!(
            operation_canvas_hit(&scene, outside, &operation_document),
            None
        );
        assert_eq!(
            operation_geometry_hover(&scene, outside, &operation_document),
            None
        );
        assert!(operation_stage_accepts_geometry(
            geosolve_constraint_editor::OperationAuthoringStage::PickFirstFilletCurve
        ));
        assert!(operation_stage_accepts_geometry(
            geosolve_constraint_editor::OperationAuthoringStage::PickSecondFilletCurve
        ));
        assert!(!operation_stage_accepts_geometry(
            geosolve_constraint_editor::OperationAuthoringStage::PlaceFilletRadius
        ));
        assert!(!operation_stage_accepts_geometry(
            geosolve_constraint_editor::OperationAuthoringStage::PreviewReady
        ));
    }

    #[test]
    fn operation_hover_forwarding_is_headless_and_preview_reuse_is_bounded() {
        let (mut coordinator, _, points) = fillet_operation_fixture();
        let operation_document = coordinator
            .operation_authoring_document()
            .expect("accepted operation document")
            .clone();
        let mut inactive_stage = OperationAuthoringState::default();
        let _ = inactive_stage.activate(&operation_document, OperationAuthoringTool::Fillet, &[]);
        let before = inactive_stage.clone();
        assert!(matches!(
            inactive_stage.hover(&operation_document, [2.0, 2.0]),
            OperationAuthoringOutcome::Collecting { ref picks, .. } if picks.is_empty()
        ));
        assert_eq!(inactive_stage, before);

        let mut state = OperationAuthoringState::default();
        let (_, metadata) = prepare_confirmed_fillet(&mut coordinator, &mut state, points[1]);
        let document = coordinator
            .operation_authoring_document()
            .expect("operation document")
            .clone();
        let hovered = state.hover(&document, [1.8, 0.2]);
        coordinator.observe_operation_authoring_outcome(&hovered);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = hovered else {
            panic!("confirmed hover should retain the exact preview candidate");
        };
        assert!(operation_preview_reusable(&candidate, &coordinator));
        assert_eq!(
            coordinator
                .operation_preview()
                .expect("held preview")
                .metadata()
                .token,
            metadata.token,
            "reusable same-side hover must not allocate another preview generation"
        );
    }
}
