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
fn should_restore_selected_option(selected: Option<&str>, options: &[(&str, &str)]) -> bool {
    selected.is_some_and(|selected| options.iter().any(|(value, _)| *value == selected))
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
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::str::FromStr as _;

    use geosolve_constraint_editor::{
        ActionChoice, ActionState, BranchAction, ConicConstructionOptions, ConstraintActionRequest,
        ConstraintIntent, ConstraintRelationChoice, ConstructionPreview, ContactActionChoice,
        CoordinatorActionKind, DimensionActionRequest, DimensionKind, DisabledReason, EditorEffect,
        EditorScene, EditorTool, Modifiers, NurbsConstructionOptions, PointerInput,
        ProvisionalInferenceCandidate, RetainedEditorCoordinator, SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        ContactBranchEdit, ContactDomain, ContactNeighborhood, CurveId, CurveSpan, DesignPointId,
        DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineForm, DocumentConstraintId,
        DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
        DocumentCurveNormalSide, DocumentCurveSpanRef, DocumentDimensionId, DocumentDimensionMode,
        DocumentHyperbolaBranch, DocumentSolveRequest, PersistentId, RetainedSketchDocumentSession,
        SketchDocument, TangentOrientation,
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
                    "[data-wb-tool], [data-editor-item], [data-wb-action], ",
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
                let effects = wb.coordinator.editor_mut().activate_tool(tool);
                dispatch_effects(&mut wb, effects);
                wb.notice = format!("{} tool active", tool_key(tool));
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
                    let coordinator = wb.interaction_coordinator_mut();
                    coordinator.editor_mut().select_item(item, modifiers);
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
                if wb.pan_gesture.is_some() {
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
                if wb.pan_gesture.is_some() {
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
                wb.camera.reset();
                wb.construction_preview = None;
                wb.inference_preview = None;
            }),
            "undo" => wb.coordinator.undo().map_err(|error| error.to_string()),
            "redo" => wb.coordinator.redo().map_err(|error| error.to_string()),
            "cancel" => {
                let effects = wb.coordinator.editor_mut().cancel();
                dispatch_effects(wb, effects);
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
            "constraint" => apply_constraint(document, wb),
            "dimension" => apply_dimension(document, wb),
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
        wb.notice = result.map_or_else(
            |error| error,
            |()| match action {
                "problems" => wb.notice.clone(),
                _ => "Action retained".into(),
            },
        );
    }

    fn select_scenario(wb: &mut Workbench, key: &str) -> bool {
        match wb.scenarios.select_key(key) {
            Ok(()) => {
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

    fn apply_constraint(document: &Document, wb: &mut Workbench) -> Result<(), String> {
        let key = select_value(document, "wb-constraint-kind").unwrap_or_else(|| "fixed".into());
        let intent =
            constraint_from_key(&key).ok_or_else(|| "unknown constraint action".to_owned())?;
        let expected = wb.coordinator.session().design_identity();
        let action = CoordinatorActionKind::Constraint(intent);
        let choices = wb.coordinator.action_choices(action);
        let contacts = choices
            .iter()
            .filter_map(|choice| match choice {
                ActionChoice::Contact { .. } => Some(contact_choice(document, choice)),
                ActionChoice::AngleOrientation { .. }
                | ActionChoice::CurveDirection { .. }
                | ActionChoice::EqualCurvature { .. }
                | ActionChoice::Continuity { .. } => None,
            })
            .collect::<Result<Vec<_>, _>>()?;
        let relation = relation_choice(document, &choices)?;
        wb.coordinator
            .apply_constraint_action(
                expected,
                ConstraintActionRequest {
                    intent,
                    label: key.replace('-', " "),
                    contacts,
                    relation,
                },
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn apply_dimension(document: &Document, wb: &mut Workbench) -> Result<(), String> {
        let kind = select_value(document, "wb-dimension-kind")
            .as_deref()
            .and_then(dimension_from_key)
            .ok_or_else(|| "unknown dimension action".to_owned())?;
        let mode = if select_value(document, "wb-dimension-mode").as_deref() == Some("reference") {
            DocumentDimensionMode::Reference
        } else {
            DocumentDimensionMode::Driving
        };
        let angle_orientation =
            if select_value(document, "wb-angle-orientation").as_deref() == Some("clockwise") {
                DocumentAngleOrientation::Clockwise
            } else {
                DocumentAngleOrientation::CounterClockwise
            };
        let expected = wb.coordinator.session().design_identity();
        wb.coordinator
            .apply_dimension_action(
                expected,
                DimensionActionRequest {
                    kind,
                    mode,
                    label: dimension_key(kind).replace('-', " "),
                    angle_orientation,
                },
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn contact_choice(
        document: &Document,
        choice: &ActionChoice,
    ) -> Result<ContactActionChoice, String> {
        let ActionChoice::Contact {
            operand,
            span,
            domains,
            neighborhoods,
            tangent_orientations,
            ..
        } = choice
        else {
            return Err("contact branch metadata expected".into());
        };
        let suffix = operand.to_string();
        let domain_key = select_value(document, &format!("wb-contact-domain-{suffix}"))
            .ok_or_else(|| "contact domain control is missing".to_owned())?;
        let domain = domains
            .iter()
            .copied()
            .find(|domain| contact_domain_key(*domain) == domain_key)
            .ok_or_else(|| "selected contact domain is unavailable".to_owned())?;
        let neighborhood_key = select_value(document, &format!("wb-contact-neighborhood-{suffix}"))
            .ok_or_else(|| "contact neighborhood control is missing".to_owned())?;
        let neighborhood = neighborhoods
            .iter()
            .copied()
            .find(|value| contact_neighborhood_key(*value) == neighborhood_key)
            .ok_or_else(|| "selected contact neighborhood is unavailable".to_owned())?;
        let entered_parameter = input_value(document, &format!("wb-contact-parameter-{suffix}"))
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .ok_or_else(|| "contact parameter must be finite".to_owned())?;
        let parameter = match (domain, neighborhood) {
            (ContactDomain::Bounded { lower, .. }, ContactNeighborhood::Start) => lower,
            (ContactDomain::Bounded { upper, .. }, ContactNeighborhood::End) => upper,
            _ => entered_parameter,
        };
        let winding = input_value(document, &format!("wb-contact-winding-{suffix}"))
            .and_then(|value| value.parse::<i32>().ok())
            .ok_or_else(|| "contact winding must be an integer".to_owned())?;
        let tangent_orientation = if tangent_orientations.is_empty() {
            None
        } else {
            let key = select_value(document, &format!("wb-contact-orientation-{suffix}"))
                .ok_or_else(|| "tangent orientation control is missing".to_owned())?;
            tangent_orientations
                .iter()
                .copied()
                .find(|orientation| tangent_orientation_key(*orientation) == key)
        };
        if !tangent_orientations.is_empty() && tangent_orientation.is_none() {
            return Err("selected tangent orientation is unavailable".into());
        }
        Ok(ContactActionChoice {
            support: DocumentCurveSpanRef {
                span: *span,
                winding,
            },
            domain,
            parameter,
            neighborhood,
            tangent_orientation,
        })
    }

    fn relation_choice(
        document: &Document,
        choices: &[ActionChoice],
    ) -> Result<Option<ConstraintRelationChoice>, String> {
        let key = select_value(document, "wb-relation-kind").unwrap_or_default();
        for choice in choices {
            match choice {
                ActionChoice::CurveDirection { values } => {
                    return values
                        .iter()
                        .copied()
                        .find(|value| curve_direction_option(*value).0 == key)
                        .map(ConstraintRelationChoice::CurveDirection)
                        .map(Some)
                        .ok_or_else(|| "selected curve-direction branch is unavailable".into());
                }
                ActionChoice::EqualCurvature { values } => {
                    return values
                        .iter()
                        .copied()
                        .find(|value| curvature_option(*value).0 == key)
                        .map(ConstraintRelationChoice::EqualCurvature)
                        .map(Some)
                        .ok_or_else(|| "selected curvature branch is unavailable".into());
                }
                ActionChoice::Continuity { values } => {
                    let mut continuity = values
                        .iter()
                        .copied()
                        .find(|value| continuity_option(*value).0 == key)
                        .ok_or_else(|| "selected continuity order is unavailable".to_owned())?;
                    if matches!(continuity, DocumentCurveContinuity::ParametricC2 { .. }) {
                        let first_rate = finite_positive_input(
                            document,
                            "wb-continuity-first-rate",
                            "first C2 rate",
                        )?;
                        let second_rate = finite_positive_input(
                            document,
                            "wb-continuity-second-rate",
                            "second C2 rate",
                        )?;
                        continuity = DocumentCurveContinuity::ParametricC2 {
                            first_rate,
                            second_rate,
                        };
                    }
                    return Ok(Some(ConstraintRelationChoice::Continuity(continuity)));
                }
                ActionChoice::Contact { .. } | ActionChoice::AngleOrientation { .. } => {}
            }
        }
        Ok(None)
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
        let construction_preview = if wb.scenarios.is_active() {
            None
        } else {
            wb.construction_preview.as_ref()
        };
        required(document, "wb-viewport")?.set_inner_html(&super::scene::svg_markup(
            scene.as_ref(),
            accepted,
            selection,
            construction_preview,
            coordinator.current_problem_metadata().as_ref(),
            wb.camera.viewport(),
        ));
        let design = coordinator.session().design_document();
        required(document, "wb-tree")?
            .set_inner_html(&super::panels::tree_markup(design, selection));
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
        if coordinator.editor().can_complete_draft()
            || (!wb.scenarios.is_active() && wb.construction_preview.is_some())
        {
            guide.remove_attribute("hidden")?;
        } else {
            guide.set_attribute("hidden", "")?;
        }
        required(document, "wb-draft-guide-text")?
            .set_text_content(Some(draft_guide_text(coordinator.editor().tool())));
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
        render_action_availability(document, coordinator, wb.scenarios.is_active())?;
        render_branch_editor(document, coordinator, wb.scenarios.is_active())?;
        required(document, "workbench-root")?
            .set_attribute("data-editor-adapter", "retained-coordinator")?;
        Ok(())
    }

    fn render_action_availability(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
        scenario_active: bool,
    ) -> Result<(), JsValue> {
        set_select_options(
            document,
            "wb-constraint-kind",
            super::action_surface::CONSTRAINT_ACTIONS
                .into_iter()
                .map(|(key, label, intent)| {
                    (
                        key,
                        coordinator
                            .resolved_constraint(intent)
                            .map_or(label, |resolved| resolved.label()),
                    )
                }),
        )?;
        set_select_options(
            document,
            "wb-dimension-kind",
            super::action_surface::DIMENSION_ACTIONS
                .into_iter()
                .map(|(key, label, _)| (key, label)),
        )?;
        for key in ["new", "finish", "cancel", "clear-selection"] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_disabled(&button, scenario_active)?;
            }
        }
        for id in [
            "wb-constraint-kind",
            "wb-dimension-kind",
            "wb-dimension-mode",
            "wb-angle-orientation",
        ] {
            set_disabled(&required(document, id)?, scenario_active)?;
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
        let constraint_key =
            select_value(document, "wb-constraint-kind").unwrap_or_else(|| "lock".into());
        if let Some(intent) = constraint_from_key(&constraint_key)
            && let Some(button) = document.query_selector("[data-wb-action=\"constraint\"]")?
        {
            set_action_state(
                &button,
                state(CoordinatorActionKind::Constraint(intent)),
                scenario_active,
            )?;
            render_action_choices(
                document,
                &coordinator.action_choices(CoordinatorActionKind::Constraint(intent)),
                scenario_active,
            )?;
        }
        let dimension_kind = select_value(document, "wb-dimension-kind")
            .as_deref()
            .and_then(dimension_from_key)
            .unwrap_or(DimensionKind::PointDistance);
        let mode = if select_value(document, "wb-dimension-mode").as_deref() == Some("reference") {
            DocumentDimensionMode::Reference
        } else {
            DocumentDimensionMode::Driving
        };
        if let Some(button) = document.query_selector("[data-wb-action=\"dimension\"]")? {
            set_action_state(
                &button,
                state(CoordinatorActionKind::Dimension(dimension_kind, mode)),
                scenario_active,
            )?;
        }
        set_disabled(
            &required(document, "wb-angle-orientation")?,
            scenario_active || dimension_kind != DimensionKind::OrientedAngle,
        )?;
        Ok(())
    }

    fn render_action_choices(
        document: &Document,
        choices: &[ActionChoice],
        scenario_active: bool,
    ) -> Result<(), JsValue> {
        for operand in 0..=1_u8 {
            required(document, &format!("wb-contact-choice-{operand}"))?
                .set_attribute("hidden", "")?;
        }
        required(document, "wb-relation-choice")?.set_attribute("hidden", "")?;
        for choice in choices {
            match choice {
                ActionChoice::Contact {
                    operand,
                    span,
                    domains,
                    default_parameter,
                    neighborhoods,
                    tangent_orientations,
                    default_winding,
                } => {
                    let section = required(document, &format!("wb-contact-choice-{operand}"))?;
                    section.remove_attribute("hidden")?;
                    set_disabled(&section, scenario_active)?;
                    required(document, &format!("wb-contact-span-{operand}"))?.set_text_content(
                        Some(&format!("Curve {} · span {}", span.curve, span.segment)),
                    );
                    set_select_options(
                        document,
                        &format!("wb-contact-domain-{operand}"),
                        domains.iter().copied().map(|domain| {
                            (contact_domain_key(domain), contact_domain_label(domain))
                        }),
                    )?;
                    set_select_options(
                        document,
                        &format!("wb-contact-neighborhood-{operand}"),
                        neighborhoods.iter().copied().map(|neighborhood| {
                            (
                                contact_neighborhood_key(neighborhood),
                                contact_neighborhood_label(neighborhood),
                            )
                        }),
                    )?;
                    set_select_options(
                        document,
                        &format!("wb-contact-orientation-{operand}"),
                        tangent_orientations.iter().copied().map(|orientation| {
                            (
                                tangent_orientation_key(orientation),
                                tangent_orientation_label(orientation),
                            )
                        }),
                    )?;
                    set_input_default(
                        document,
                        &format!("wb-contact-parameter-{operand}"),
                        *default_parameter,
                    )?;
                    set_input_default(
                        document,
                        &format!("wb-contact-winding-{operand}"),
                        *default_winding,
                    )?;
                }
                ActionChoice::CurveDirection { values } => {
                    show_relation_options(
                        document,
                        values.iter().copied().map(curve_direction_option),
                        scenario_active,
                    )?;
                }
                ActionChoice::EqualCurvature { values } => {
                    show_relation_options(
                        document,
                        values.iter().copied().map(curvature_option),
                        scenario_active,
                    )?;
                }
                ActionChoice::Continuity { values } => {
                    show_relation_options(
                        document,
                        values.iter().copied().map(continuity_option),
                        scenario_active,
                    )?;
                }
                ActionChoice::AngleOrientation { .. } => {}
            }
        }
        Ok(())
    }

    fn show_relation_options<I>(
        document: &Document,
        options: I,
        scenario_active: bool,
    ) -> Result<(), JsValue>
    where
        I: IntoIterator<Item = (&'static str, &'static str)>,
    {
        let section = required(document, "wb-relation-choice")?;
        section.remove_attribute("hidden")?;
        set_disabled(&section, scenario_active)?;
        set_select_options(document, "wb-relation-kind", options)
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

    fn set_select_options<I>(document: &Document, id: &str, options: I) -> Result<(), JsValue>
    where
        I: IntoIterator<Item = (&'static str, &'static str)>,
    {
        let element = required(document, id)?;
        let selected = element
            .clone()
            .dyn_into::<HtmlSelectElement>()
            .ok()
            .map(|select| select.value());
        let options = options.into_iter().collect::<Vec<_>>();
        let restore_selected = super::should_restore_selected_option(selected.as_deref(), &options);
        let markup = options
            .into_iter()
            .map(|(value, label)| format!("<option value=\"{value}\">{label}</option>"))
            .collect::<String>();
        if element.inner_html() != markup {
            element.set_inner_html(&markup);
            if restore_selected && let Some(selected) = selected {
                element
                    .dyn_into::<HtmlSelectElement>()?
                    .set_value(&selected);
            }
        }
        Ok(())
    }

    fn set_input_default<T: ToString>(
        document: &Document,
        id: &str,
        value: T,
    ) -> Result<(), JsValue> {
        let element = required(document, id)?;
        let input = element.clone().dyn_into::<HtmlInputElement>()?;
        let next = value.to_string();
        let previous = element.get_attribute("data-headless-default");
        if previous
            .as_ref()
            .is_none_or(|previous| input.value().is_empty() || input.value() == *previous)
        {
            input.set_value(&next);
        }
        element.set_attribute("data-headless-default", &next)?;
        Ok(())
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
    fn constraint_from_key(key: &str) -> Option<ConstraintIntent> {
        super::action_surface::constraint_from_key(key)
    }
    fn dimension_from_key(key: &str) -> Option<DimensionKind> {
        super::action_surface::dimension_from_key(key)
    }
    fn dimension_key(kind: DimensionKind) -> &'static str {
        super::action_surface::dimension_key(kind)
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
    const fn curve_direction_option(
        relation: DocumentCurveDirectionRelation,
    ) -> (&'static str, &'static str) {
        match relation {
            DocumentCurveDirectionRelation::Tangent {
                orientation: TangentOrientation::Aligned,
            } => ("tangent-aligned", "Tangent · aligned"),
            DocumentCurveDirectionRelation::Tangent {
                orientation: TangentOrientation::Opposed,
            } => ("tangent-opposed", "Tangent · opposed"),
            DocumentCurveDirectionRelation::Normal {
                side: DocumentCurveNormalSide::Left,
            } => ("normal-left", "Normal · left side"),
            DocumentCurveDirectionRelation::Normal {
                side: DocumentCurveNormalSide::Right,
            } => ("normal-right", "Normal · right side"),
        }
    }
    const fn curvature_option(
        relation: DocumentCurveCurvatureRelation,
    ) -> (&'static str, &'static str) {
        match relation {
            DocumentCurveCurvatureRelation::Signed => ("signed", "Signed curvature"),
            DocumentCurveCurvatureRelation::MagnitudeSameSign => {
                ("magnitude-same-sign", "Magnitude · same sign")
            }
            DocumentCurveCurvatureRelation::MagnitudeOppositeSign => {
                ("magnitude-opposite-sign", "Magnitude · opposite sign")
            }
        }
    }
    const fn continuity_option(
        continuity: DocumentCurveContinuity,
    ) -> (&'static str, &'static str) {
        match continuity {
            DocumentCurveContinuity::G0 => ("g0", "G0 · position"),
            DocumentCurveContinuity::G1 => ("g1", "G1 · tangent"),
            DocumentCurveContinuity::G2 => ("g2", "G2 · curvature"),
            DocumentCurveContinuity::ParametricC2 { .. } => ("c2", "C2 · parametric"),
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
    use geosolve_constraint_editor::{Modifiers, PointerInput, ScreenPoint};

    use super::{PointerMoveQueue, should_restore_selected_option};

    #[test]
    fn dynamic_choice_selects_first_new_default_instead_of_restoring_an_invalid_value() {
        let periodic = [("periodic", "Periodic")];
        assert!(!should_restore_selected_option(Some(""), &periodic));
        assert!(!should_restore_selected_option(Some("bounded"), &periodic));
        assert!(should_restore_selected_option(Some("periodic"), &periodic));
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
}
