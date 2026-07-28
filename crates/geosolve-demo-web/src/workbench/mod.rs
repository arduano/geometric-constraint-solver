// SPDX-License-Identifier: GPL-3.0-or-later

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

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::str::FromStr as _;

    use geosolve_constraint_editor::{
        ActionState, ConstraintKind, ConstructionPreview, CoordinatorActionKind, EditorEffect,
        EditorScene, EditorTool, Modifiers, PointerInput, ProvisionalInferenceCandidate,
        RetainedEditorCoordinator, SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        CurveId, CurveSpan, DesignPointId, DocumentConstraintId, DocumentDimensionId,
        DocumentDimensionMode, DocumentSolveRequest, PersistentId, RetainedSketchDocumentSession,
        SketchDocument,
    };
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::JsValue;
    use web_sys::{
        Document, Element, Event, HtmlElement, HtmlSelectElement, KeyboardEvent, MouseEvent,
        PointerEvent,
    };

    use super::persistence::{STORAGE_KEY, WorkspaceSnapshot};

    struct Workbench {
        coordinator: RetainedEditorCoordinator,
        scenarios: super::scenarios::ScenarioWorkbenchState,
        construction_preview: Option<ConstructionPreview>,
        inference_preview: Option<ProvisionalInferenceCandidate>,
        notice: String,
        problems_open: bool,
    }

    pub(crate) fn install(document: &Document) -> Result<(), JsValue> {
        let storage = super::platform::window()?.local_storage().ok().flatten();
        let snapshot = storage
            .as_ref()
            .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten());
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
        let design =
            SketchDocument::from_json(&snapshot.design_json).map_err(|error| error.to_string())?;
        let session = if let Some(json) = snapshot.accepted_json.as_deref() {
            RetainedSketchDocumentSession::restore_design_with_accepted(
                design,
                SketchDocument::from_json(json).map_err(|error| error.to_string())?,
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
                let effects = wb.coordinator.editor_mut().activate_tool(tool);
                dispatch_effects(&mut wb, effects);
                wb.notice = format!("{} tool active", tool_key(tool));
            } else if target.has_attribute("data-editor-item") {
                if callback_workbench.borrow().scenarios.is_active() {
                    return;
                }
                if let Some(item) = selection_item(&target) {
                    let modifiers = event
                        .dyn_ref::<MouseEvent>()
                        .map(|event| Modifiers {
                            shift: event.shift_key(),
                            control: event.ctrl_key(),
                            command: event.meta_key(),
                        })
                        .unwrap_or_default();
                    callback_workbench
                        .borrow_mut()
                        .coordinator
                        .editor_mut()
                        .select_item(item, modifiers);
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
        let change = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
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
        install_pointer_listener(
            document,
            workbench,
            &viewport,
            "pointerdown",
            |wb, scene, input| wb.coordinator.editor_mut().pointer_down(scene, input),
        )?;
        install_pointer_listener(
            document,
            workbench,
            &viewport,
            "pointermove",
            |wb, scene, input| wb.coordinator.editor_mut().pointer_move(scene, input),
        )?;
        install_pointer_listener(
            document,
            workbench,
            &viewport,
            "pointerup",
            |wb, scene, input| {
                let expected = wb.coordinator.session().design_identity();
                wb.coordinator
                    .editor_mut()
                    .pointer_up(scene, expected, input)
            },
        )?;

        let cancel_document = document.clone();
        let cancel_workbench = Rc::clone(workbench);
        let cancel = Closure::<dyn FnMut(PointerEvent)>::new(move |_event| {
            let mut wb = cancel_workbench.borrow_mut();
            if wb.scenarios.is_active() {
                return;
            }
            let effects = wb.coordinator.editor_mut().cancel();
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
        Ok(())
    }

    fn install_pointer_listener(
        document: &Document,
        workbench: &Rc<RefCell<Workbench>>,
        viewport: &Element,
        name: &str,
        transition: fn(&mut Workbench, &EditorScene, PointerInput) -> Vec<EditorEffect>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_workbench = Rc::clone(workbench);
        let callback_viewport = viewport.clone();
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event| {
            let mut wb = callback_workbench.borrow_mut();
            if wb.scenarios.is_active() {
                return;
            }
            let Some(scene) = editor_scene(&wb) else {
                return;
            };
            let Some(input) = pointer_input(&callback_viewport, scene.viewport, &event) else {
                return;
            };
            let effects = transition(&mut wb, &scene, input);
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
                    let next = wb.coordinator.resolve_projected_point_move(
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
                EditorEffect::SelectionChanged(_) => {}
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
        if !wb.scenarios.ordinary_action_allowed(action) {
            wb.notice = "Exit the active review scenario before ordinary editing".into();
            return;
        }
        let result = match action {
            "new" => empty_coordinator().map(|coordinator| {
                wb.coordinator = coordinator;
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
            "problems" => {
                wb.problems_open = !wb.problems_open;
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
                    |()| "Selected scenario reset to its deterministic starting state".into(),
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
        let kind =
            constraint_from_key(&key).ok_or_else(|| "unknown constraint action".to_owned())?;
        let expected = wb.coordinator.session().design_identity();
        let edit = wb
            .coordinator
            .editor()
            .constraint_edit(
                wb.coordinator.session().design_document(),
                kind,
                key.replace('-', " "),
            )
            .map_err(|error| error.to_string())?;
        wb.coordinator
            .apply_edit(expected, edit)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn apply_dimension(document: &Document, wb: &mut Workbench) -> Result<(), String> {
        let mode = if select_value(document, "wb-dimension-mode").as_deref() == Some("reference") {
            DocumentDimensionMode::Reference
        } else {
            DocumentDimensionMode::Driving
        };
        let expected = wb.coordinator.session().design_identity();
        wb.coordinator
            .add_selected_dimension(expected, mode, "Dimension")
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
            super::scene::viewport(),
            0.8,
        )
        .ok()
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
        render_scenario_ui(document, &wb.scenarios)?;
        let problems = required(document, "wb-problems")?;
        if wb.problems_open || problem != "No current solver problem" {
            problems.remove_attribute("hidden")?;
        } else {
            problems.set_attribute("hidden", "")?;
        }
        let guide = required(document, "wb-draft-guide")?;
        if coordinator.editor().can_complete_draft() {
            guide.remove_attribute("hidden")?;
        } else {
            guide.set_attribute("hidden", "")?;
        }
        for tool in [
            EditorTool::Select,
            EditorTool::Point,
            EditorTool::Line,
            EditorTool::Polyline,
            EditorTool::Rectangle,
            EditorTool::Circle,
            EditorTool::CounterClockwiseArc,
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
        required(document, "workbench-root")?
            .set_attribute("data-editor-adapter", "retained-coordinator")?;
        Ok(())
    }

    fn render_action_availability(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
        scenario_active: bool,
    ) -> Result<(), JsValue> {
        for key in ["new", "finish", "cancel", "clear-selection"] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_disabled(&button, scenario_active)?;
            }
        }
        for id in ["wb-constraint-kind", "wb-dimension-mode"] {
            set_disabled(&required(document, id)?, scenario_active)?;
        }
        let actions = coordinator.actions();
        let enabled = |action| {
            actions
                .iter()
                .find(|value| value.action == action)
                .is_some_and(|value| value.state == ActionState::Enabled)
        };
        for (key, action) in [
            ("undo", CoordinatorActionKind::Undo),
            ("redo", CoordinatorActionKind::Redo),
            ("delete", CoordinatorActionKind::Delete),
        ] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_disabled(&button, scenario_active || !enabled(action))?;
            }
        }
        let constraint_key =
            select_value(document, "wb-constraint-kind").unwrap_or_else(|| "fixed".into());
        if let Some(kind) = constraint_from_key(&constraint_key)
            && let Some(button) = document.query_selector("[data-wb-action=\"constraint\"]")?
        {
            set_disabled(
                &button,
                scenario_active || !enabled(CoordinatorActionKind::Constraint(kind)),
            )?;
        }
        let mode = if select_value(document, "wb-dimension-mode").as_deref() == Some("reference") {
            DocumentDimensionMode::Reference
        } else {
            DocumentDimensionMode::Driving
        };
        let dimension_enabled = enabled(CoordinatorActionKind::PointDistance(mode))
            || enabled(CoordinatorActionKind::SegmentLength(mode));
        if let Some(button) = document.query_selector("[data-wb-action=\"dimension\"]")? {
            set_disabled(&button, scenario_active || !dimension_enabled)?;
        }
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

    fn problem_text(coordinator: &RetainedEditorCoordinator) -> String {
        let problems = coordinator.problems();
        if let Some(failure) = problems.failure {
            return failure.message().to_owned();
        }
        problems.rejection.map_or_else(
            || "No current solver problem".into(),
            |value| format!("{value:?}"),
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
        let rect = viewport.get_bounding_client_rect();
        let pointer_id = u64::try_from(event.pointer_id()).ok()?;
        Some(PointerInput {
            pointer_id,
            position: super::effect_adapter::normalize_client_point(
                super::effect_adapter::ClientRect {
                    left: rect.left(),
                    top: rect.top(),
                    width: rect.width(),
                    height: rect.height(),
                },
                model_viewport.screen_size,
                [f64::from(event.client_x()), f64::from(event.client_y())],
            )?,
            modifiers: Modifiers {
                shift: event.shift_key(),
                control: event.ctrl_key(),
                command: event.meta_key(),
            },
        })
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
            _ => return None,
        })
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
        }
    }
    fn constraint_from_key(key: &str) -> Option<ConstraintKind> {
        Some(match key {
            "fixed" => ConstraintKind::Fixed,
            "coincident" => ConstraintKind::Coincident,
            "horizontal" => ConstraintKind::Horizontal,
            "vertical" => ConstraintKind::Vertical,
            "parallel" => ConstraintKind::Parallel,
            "perpendicular" => ConstraintKind::Perpendicular,
            "equal-length" => ConstraintKind::EqualLength,
            _ => return None,
        })
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
