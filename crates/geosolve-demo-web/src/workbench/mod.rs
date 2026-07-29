// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(any(target_arch = "wasm32", test))]
mod action_surface;
#[cfg(any(target_arch = "wasm32", test))]
mod effect_adapter;
#[cfg(any(target_arch = "wasm32", test))]
mod evidence;
#[cfg(any(target_arch = "wasm32", test))]
mod panels;
#[cfg(any(target_arch = "wasm32", test))]
mod persistence;
#[cfg(target_arch = "wasm32")]
mod platform;
#[cfg(any(target_arch = "wasm32", test))]
mod scenario_fixtures;
#[cfg(any(target_arch = "wasm32", test))]
mod scenarios;
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
        EditorScene, EditorTool, Modifiers, NurbsConstructionOptions, PickTolerance, PointerInput,
        ProvisionalInferenceCandidate, RetainedEditorCoordinator, SelectionItem,
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

    use super::persistence::{LEGACY_STORAGE_KEY, STORAGE_KEY, WorkspaceSnapshot};

    struct Workbench {
        coordinator: RetainedEditorCoordinator,
        authoring: AuthoringState,
        scenarios: super::scenarios::ScenarioWorkbenchState,
        camera: super::scene::CanvasCamera,
        pan_gesture: Option<PanGesture>,
        construction_preview: Option<ConstructionPreview>,
        inference_preview: Option<ProvisionalInferenceCandidate>,
        notice: String,
        problems_open: bool,
    }

    impl Workbench {
        fn interaction_coordinator_mut(&mut self) -> &mut RetainedEditorCoordinator {
            self.scenarios
                .coordinator_for_interaction_mut(&mut self.coordinator)
        }

        fn resolve_projected_point_move(
            &mut self,
            pointer_id: u64,
            request_id: u64,
            point: geosolve_sketch::DesignPointId,
            model_position: [f64; 2],
        ) -> Vec<EditorEffect> {
            self.scenarios.resolve_projected_point_move(
                &mut self.coordinator,
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
            scenarios: super::scenarios::ScenarioWorkbenchState::new(),
            camera: super::scene::CanvasCamera::default(),
            pan_gesture: None,
            construction_preview: None,
            inference_preview: None,
            notice,
            problems_open: false,
        }));
        render(document, &workbench)?;
        install_clicks(document, &workbench)?;
        install_scenario_flyout_state(document)?;
        install_canvas(document, &workbench)?;
        install_keyboard(document, &workbench)?;
        Ok(())
    }

    fn install_scenario_flyout_state(document: &Document) -> Result<(), JsValue> {
        let menu = required(document, "wb-scenario-menu")?;
        for name in ["pointerover", "focusin"] {
            let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let Some(branch) = scenario_branch(&event) else {
                    return;
                };
                set_branch_expanded(&branch, true);
            });
            menu.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
            callback.forget();
        }
        for name in ["pointerout", "focusout"] {
            let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let Some(branch) = scenario_branch(&event) else {
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

    fn scenario_branch(event: &Event) -> Option<Element> {
        event
            .target()?
            .dyn_into::<Element>()
            .ok()?
            .closest(".wb-scenario-branch")
            .ok()
            .flatten()
    }

    fn set_branch_expanded(branch: &Element, expanded: bool) {
        if let Ok(Some(trigger)) = branch.query_selector("[data-scenario-group-trigger]") {
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
        let design = snapshot.design_document()?;
        let session = if let Some(accepted) = snapshot.accepted_document()? {
            RetainedSketchDocumentSession::restore_design_with_accepted(
                design,
                accepted,
                snapshot.revisions(),
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
        } else {
            RetainedSketchDocumentSession::restore_design(
                design,
                snapshot.revisions(),
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
        }
        .map_err(|error| error.to_string())?;
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
            let target = origin
                .closest(concat!(
                    "[data-wb-tool], [data-wb-authoring], [data-editor-item], [data-wb-action], ",
                    "[data-scenario-id], [data-scenario-action], [data-scenario-control], ",
                    "[data-scenario-group-trigger]"
                ))
                .ok()
                .flatten()
                .unwrap_or(origin);
            let mut selected_scenario = false;
            let mut exited_scenarios = false;
            if target.has_attribute("data-scenario-group-trigger") {
                return;
            } else if let Some(tool) = target
                .get_attribute("data-wb-tool")
                .and_then(|key| tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                if wb.scenarios.is_active() {
                    wb.notice = "Exit the active review scenario before ordinary editing".into();
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                    return;
                }
                if let Err(error) =
                    update_construction_options(&callback_document, wb.coordinator.editor_mut())
                {
                    wb.notice = error;
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                    return;
                }
                wb.authoring.deactivate();
                let effects = wb.coordinator.editor_mut().activate_tool(tool);
                dispatch_effects(&mut wb, effects);
                wb.notice = format!("{} tool active", tool_key(tool));
            } else if let Some(tool) = target
                .get_attribute("data-wb-authoring")
                .and_then(|key| super::action_surface::authoring_tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                activate_authoring(&callback_document, &mut wb, tool);
            } else if target.has_attribute("data-editor-item") {
                if let Some(item) = selection_item(&target) {
                    let modifiers = event
                        .dyn_ref::<MouseEvent>()
                        .map(|event| Modifiers {
                            shift: event.shift_key(),
                            control: event.ctrl_key(),
                            command: event.meta_key(),
                        })
                        .unwrap_or_default();
                    let mut wb = callback_workbench.borrow_mut();
                    if wb.authoring.active_tool().is_some() && !wb.scenarios.is_active() {
                        let input = if target
                            .closest("#wb-viewport")
                            .is_ok_and(|viewport| viewport.is_some())
                        {
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
                    } else {
                        let coordinator = wb.interaction_coordinator_mut();
                        coordinator.editor_mut().select_item(item, modifiers);
                    }
                }
            } else if let Some(key) = target.get_attribute("data-scenario-id") {
                let mut wb = callback_workbench.borrow_mut();
                selected_scenario = select_scenario(&mut wb, &key);
            } else if let Some(action) = target.get_attribute("data-scenario-action") {
                perform_scenario_action(&mut callback_workbench.borrow_mut(), &action);
            } else if let Some(control) = target.get_attribute("data-scenario-control") {
                exited_scenarios =
                    perform_scenario_control(&mut callback_workbench.borrow_mut(), &control);
            } else if let Some(action) = target.get_attribute("data-wb-action") {
                perform_action(
                    &callback_document,
                    &mut callback_workbench.borrow_mut(),
                    &action,
                );
            }
            save(&callback_workbench.borrow());
            let _ = render(&callback_document, &callback_workbench);
            if selected_scenario {
                close_scenario_selector(&callback_document);
                focus_scenario_guide(&callback_document);
            } else if exited_scenarios {
                focus_by_id(&callback_document, "wb-scenario-trigger");
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
            |coordinator, scene, input| coordinator.editor_mut().pointer_down(scene, input),
        )?;
        install_pointer_move_listener(document, workbench, &viewport, &pointer_moves)?;
        install_pointer_up_listener(document, workbench, &viewport, &pointer_moves)?;

        let cancel_document = document.clone();
        let cancel_workbench = Rc::clone(workbench);
        let cancel_pointer_moves = Rc::clone(&pointer_moves);
        let cancel = Closure::<dyn FnMut(PointerEvent)>::new(move |_event| {
            cancel_pointer_moves.borrow_mut().drain_before_terminal();
            let mut wb = cancel_workbench.borrow_mut();
            let effects = wb.interaction_coordinator_mut().editor_mut().cancel();
            dispatch_effects(&mut wb, effects);
            wb.notice = "Interaction canceled".into();
            drop(wb);
            let _ = render(&cancel_document, &cancel_workbench);
        });
        viewport
            .add_event_listener_with_callback("pointercancel", cancel.as_ref().unchecked_ref())?;
        cancel.forget();

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
            if wb.scenarios.is_active() {
                return;
            }
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
                let effects = wb
                    .interaction_coordinator_mut()
                    .editor_mut()
                    .pointer_move(&scene, input);
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
            if wb.authoring.active_tool().is_some() {
                return;
            }
            if let Some(pending) = callback_pointer_moves.borrow_mut().drain_before_terminal() {
                let effects = wb
                    .interaction_coordinator_mut()
                    .editor_mut()
                    .pointer_move(&scene, pending);
                dispatch_effects(&mut wb, effects);
            }
            let coordinator = wb.interaction_coordinator_mut();
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
            let effects = {
                let coordinator = wb.interaction_coordinator_mut();
                transition(coordinator, &scene, input)
            };
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
                && required(&callback_document, "wb-scenario-selector")
                    .is_ok_and(|selector| selector.has_attribute("open"))
            {
                event.prevent_default();
                close_scenario_selector(&callback_document);
                focus_by_id(&callback_document, "wb-scenario-trigger");
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
            if wb.scenarios.is_active() {
                return;
            }
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
                    let result = wb
                        .interaction_coordinator_mut()
                        .apply_editor_effect(&effect);
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
                    match wb
                        .interaction_coordinator_mut()
                        .apply_editor_effect(&effect)
                    {
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
                    wb.interaction_coordinator_mut().clear_transient();
                }
                EditorEffect::SelectionChanged(_) => {}
                EditorEffect::CommitPointMove { .. } => {
                    match wb
                        .interaction_coordinator_mut()
                        .apply_editor_effect(&effect)
                    {
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
        if !wb.scenarios.ordinary_action_allowed(action) {
            wb.notice = "Exit the active review scenario before ordinary editing".into();
            return;
        }
        let result = match action {
            "new" => empty_coordinator().map(|coordinator| {
                wb.coordinator = coordinator;
                wb.authoring.deactivate();
                wb.camera.reset();
                wb.construction_preview = None;
                wb.inference_preview = None;
            }),
            "undo" => wb.coordinator.undo().map_err(|error| error.to_string()),
            "redo" => wb.coordinator.redo().map_err(|error| error.to_string()),
            "cancel" => {
                if wb.authoring.active_tool().is_some() {
                    let document = wb.coordinator.session().design_document().clone();
                    let outcome = wb.authoring.cancel(&document);
                    handle_authoring_outcome(wb, outcome);
                } else {
                    let effects = wb.coordinator.editor_mut().cancel();
                    dispatch_effects(wb, effects);
                }
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
        wb.notice = result.map_or_else(
            |error| error,
            |()| match action {
                "problems" | "cancel" | "dimension-target" => wb.notice.clone(),
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

    fn select_scenario(wb: &mut Workbench, key: &str) -> bool {
        match wb.scenarios.select_key(key) {
            Ok(()) => {
                wb.authoring.deactivate();
                fit_camera(wb);
                wb.notice = format!(
                    "{} loaded; this review state is ephemeral and ordinary save is disabled",
                    wb.scenarios.selected_title().unwrap_or("Scenario")
                );
                true
            }
            Err(error) => {
                wb.notice = error;
                false
            }
        }
    }

    fn perform_scenario_action(wb: &mut Workbench, action: &str) {
        wb.notice = wb
            .scenarios
            .perform_key(action)
            .map_or_else(|error| error, |observation| observation.summary());
    }

    fn perform_scenario_control(wb: &mut Workbench, control: &str) -> bool {
        match control {
            "reset" => {
                wb.notice = wb.scenarios.reset().map_or_else(
                    |error| error,
                    |()| {
                        fit_camera(wb);
                        "Selected scenario reset to its deterministic starting state".into()
                    },
                );
                false
            }
            "exit" => {
                wb.scenarios.exit();
                wb.notice =
                    "Exited review scenarios; the ordinary pre-existing workspace is restored"
                        .into();
                true
            }
            _ => false,
        }
    }

    fn activate_authoring(document: &Document, wb: &mut Workbench, tool: AuthoringTool) {
        if wb.scenarios.is_active() {
            wb.notice = "Exit the active review scenario before ordinary editing".into();
            return;
        }
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
        let coordinator = wb.scenarios.coordinator_for_render(&wb.coordinator);
        let source = coordinator
            .solved_preview_session()
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
        let coordinator = wb.scenarios.coordinator_for_render(&wb.coordinator);
        required(document, "workbench-root")?.set_attribute(
            "data-history-length",
            &coordinator.history_len().to_string(),
        )?;
        let scene = editor_scene(&wb);
        let source = coordinator
            .solved_preview_session()
            .unwrap_or(coordinator.session());
        let accepted = source.accepted_state();
        let selection = coordinator.editor().selection();
        let pending = wb
            .authoring
            .pending()
            .iter()
            .map(|operand| operand.item)
            .collect::<Vec<_>>();
        let construction_preview = if wb.scenarios.is_active() {
            None
        } else {
            wb.construction_preview.as_ref()
        };
        required(document, "wb-viewport")?.set_inner_html(&super::scene::svg_markup_with_pending(
            scene.as_ref(),
            accepted,
            selection,
            &pending,
            construction_preview,
            coordinator.current_problem_metadata().as_ref(),
            wb.camera.viewport(),
        ));
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
        render_scenario_ui(document, &wb.scenarios)?;
        let problems = required(document, "wb-problems")?;
        if wb.problems_open || problem != "No current solver problem" {
            problems.remove_attribute("hidden")?;
        } else {
            problems.set_attribute("hidden", "")?;
        }
        let guide = required(document, "wb-draft-guide")?;
        if wb.authoring.active_tool().is_some()
            || coordinator.editor().can_complete_draft()
            || (!wb.scenarios.is_active() && wb.construction_preview.is_some())
        {
            guide.remove_attribute("hidden")?;
        } else {
            guide.set_attribute("hidden", "")?;
        }
        let guide_text = wb.authoring.active_tool().map_or_else(
            || draft_guide_text(coordinator.editor().tool()).to_owned(),
            |tool| {
                format!(
                    "{} · {} pending · Escape clears/exits",
                    authoring_tool_label(tool),
                    wb.authoring.pending().len()
                )
            },
        );
        required(document, "wb-draft-guide-text")?.set_text_content(Some(&guide_text));
        if wb.authoring.active_tool().is_some() {
            required(document, "wb-guide-finish")?.set_attribute("hidden", "")?;
        } else {
            required(document, "wb-guide-finish")?.remove_attribute("hidden")?;
        }
        for tool in [
            EditorTool::Select,
            EditorTool::Point,
            EditorTool::Line,
            EditorTool::Polyline,
            EditorTool::Rectangle,
            EditorTool::Circle,
            EditorTool::CounterClockwiseArc,
            EditorTool::QuadraticBezier,
            EditorTool::CubicBezier,
            EditorTool::Ellipse,
            EditorTool::EllipticalArc,
            EditorTool::RationalQuadraticConic,
            EditorTool::Parabola,
            EditorTool::Hyperbola,
            EditorTool::Nurbs,
        ] {
            if let Some(button) =
                document.query_selector(&format!("[data-wb-tool=\"{}\"]", tool_key(tool)))?
            {
                set_disabled(&button, wb.scenarios.is_active())?;
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
            wb.scenarios.is_active(),
        )?;
        render_dimension_target_editor(document, coordinator, wb.scenarios.is_active())?;
        render_branch_editor(document, coordinator, wb.scenarios.is_active())?;
        required(document, "workbench-root")?
            .set_attribute("data-editor-adapter", "retained-coordinator")?;
        Ok(())
    }

    fn render_action_availability(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
        authoring: &AuthoringState,
        scenario_active: bool,
    ) -> Result<(), JsValue> {
        for key in ["new", "finish", "cancel", "clear-selection"] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_disabled(
                    &button,
                    scenario_active || (key == "finish" && authoring.active_tool().is_some()),
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
                set_action_state(&button, state(action), scenario_active)?;
            }
        }
        for (key, _, intent) in super::action_surface::CONSTRAINT_ACTIONS {
            if let Some(button) =
                document.query_selector(&format!("[data-wb-authoring=\"{key}\"]"))?
            {
                set_disabled(&button, scenario_active)?;
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
                set_disabled(&button, scenario_active)?;
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
        for id in [
            "wb-authoring-curvature",
            "wb-authoring-tangent-orientation",
            "wb-authoring-continuity",
            "wb-authoring-first-rate",
            "wb-authoring-second-rate",
            "wb-authoring-dimension-mode",
            "wb-authoring-angle-orientation",
        ] {
            set_disabled(&required(document, id)?, scenario_active)?;
        }
        Ok(())
    }

    fn render_dimension_target_editor(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
        scenario_active: bool,
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
            input.set_disabled(scenario_active);
        }
        required(document, "wb-dimension-target-meta")?.set_text_content(Some(&meta));
        if let Some(button) = document.query_selector("[data-wb-action=\"dimension-target\"]")? {
            set_disabled(&button, scenario_active)?;
        }
        Ok(())
    }

    fn render_branch_editor(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
        scenario_active: bool,
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
        set_disabled(&contact_button, scenario_active)?;
        set_disabled(&angle_button, scenario_active)?;
        Ok(())
    }

    fn render_scenario_ui(
        document: &Document,
        scenarios: &super::scenarios::ScenarioWorkbenchState,
    ) -> Result<(), JsValue> {
        let selected_key = scenarios.selected_key().unwrap_or("");
        let menu = required(document, "wb-scenario-menu")?;
        if menu.get_attribute("data-selected-scenario").as_deref() != Some(selected_key) {
            menu.set_inner_html(&scenarios.menu_markup());
            menu.set_attribute("data-selected-scenario", selected_key)?;
        }
        required(document, "wb-scenario-current")?.set_text_content(Some(
            scenarios
                .selected_title()
                .unwrap_or("Choose a review scenario"),
        ));

        let guide = required(document, "wb-scenario-guide")?;
        if let Some(markup) = scenarios.guide_markup() {
            if guide.get_attribute("data-selected-scenario").as_deref() != Some(selected_key) {
                guide.set_inner_html(&format!(
                    "{markup}<div id=\"wb-scenario-transcript\"></div><div id=\"wb-scenario-evidence\"></div>"
                ));
                guide.set_attribute("data-selected-scenario", selected_key)?;
            }
            guide.remove_attribute("hidden")?;
            required(document, "wb-scenario-transcript")?
                .set_inner_html(&scenarios.transcript_markup());
            required(document, "wb-scenario-evidence")?
                .set_inner_html(&scenarios.evidence_markup());
        } else {
            if guide.has_attribute("data-selected-scenario") {
                guide.set_inner_html("");
                guide.remove_attribute("data-selected-scenario")?;
            }
            guide.set_attribute("hidden", "")?;
        }
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

    fn set_action_state(
        element: &Element,
        state: ActionState,
        scenario_active: bool,
    ) -> Result<(), JsValue> {
        let disabled = scenario_active || state != ActionState::Enabled;
        set_disabled(element, disabled)?;
        if scenario_active {
            element.set_attribute("data-disabled-reason", "scenario-active")
        } else if let ActionState::Disabled(reason) = state {
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
        let Some(snapshot) = wb.scenarios.persistence_snapshot(&wb.coordinator) else {
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
    const fn tool_key(tool: EditorTool) -> &'static str {
        match tool {
            EditorTool::Select => "select",
            EditorTool::Point => "point",
            EditorTool::Line => "line",
            EditorTool::Polyline => "polyline",
            EditorTool::Rectangle => "rectangle",
            EditorTool::Circle => "circle",
            EditorTool::CounterClockwiseArc => "arc",
            EditorTool::QuadraticBezier => "quadratic-bezier",
            EditorTool::CubicBezier => "cubic-bezier",
            EditorTool::Ellipse => "ellipse",
            EditorTool::EllipticalArc => "elliptical-arc",
            EditorTool::RationalQuadraticConic => "rational-conic",
            EditorTool::Parabola => "parabola",
            EditorTool::Hyperbola => "hyperbola",
            EditorTool::Nurbs => "nurbs",
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

    fn close_scenario_selector(document: &Document) {
        if let Ok(selector) = required(document, "wb-scenario-selector") {
            let _ = selector.remove_attribute("open");
        }
    }

    fn focus_scenario_guide(document: &Document) {
        if let Ok(heading) = required(document, "wb-scenario-title") {
            focus(heading);
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
        Modifiers, PointerInput, ScreenPoint, SelectionItem,
    };
    use geosolve_sketch::{CurveDefinition, CurveSpan, SketchDocument};

    use super::{AuthoringItemInput, PointerMoveQueue, owns_authoring_pick};

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
}
