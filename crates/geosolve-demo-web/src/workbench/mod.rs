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
mod scene;
#[cfg(any(target_arch = "wasm32", test))]
mod uat;

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
        uat: super::uat::UatWorkbenchState,
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
            uat: super::uat::UatWorkbenchState::new(),
            construction_preview: None,
            inference_preview: None,
            notice,
            problems_open: false,
        }));
        render(document, &workbench)?;
        install_clicks(document, &workbench)?;
        install_canvas(document, &workbench)?;
        install_keyboard(document, &workbench)?;
        Ok(())
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
                .closest("[data-wb-tool], [data-editor-item], [data-wb-action], [data-uat-action]")
                .ok()
                .flatten()
                .unwrap_or(origin);
            if let Some(tool) = target
                .get_attribute("data-wb-tool")
                .and_then(|key| tool_from_key(&key))
            {
                let mut wb = callback_workbench.borrow_mut();
                if wb.uat.is_active() {
                    wb.notice = "Exit disposable M52 UAT before ordinary editing".into();
                    drop(wb);
                    let _ = render(&callback_document, &callback_workbench);
                    return;
                }
                let effects = wb.coordinator.editor_mut().activate_tool(tool);
                dispatch_effects(&mut wb, effects);
                wb.notice = format!("{} tool active", tool_key(tool));
            } else if target.has_attribute("data-editor-item") {
                if callback_workbench.borrow().uat.is_active() {
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
            } else if let Some(action) = target.get_attribute("data-uat-action") {
                perform_uat_action(&mut callback_workbench.borrow_mut(), &action);
            } else if let Some(action) = target.get_attribute("data-wb-action") {
                perform_action(
                    &callback_document,
                    &mut callback_workbench.borrow_mut(),
                    &action,
                );
            }
            save(&callback_workbench.borrow());
            let _ = render(&callback_document, &callback_workbench);
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
            if wb.uat.is_active() {
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
            if wb.uat.is_active() {
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
            if wb.uat.is_active() {
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
            if event.key() == "Enter"
                && let Some(target) = event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                && matches!(target.tag_name().as_str(), "BUTTON" | "SELECT" | "INPUT")
            {
                if let Ok(button) = target.dyn_into::<HtmlElement>()
                    && button.tag_name() == "BUTTON"
                {
                    event.prevent_default();
                    button.click();
                }
                return;
            }
            let mut wb = callback_workbench.borrow_mut();
            if wb.uat.is_active() {
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
        if !wb.uat.ordinary_action_allowed(action) {
            wb.notice = "Exit disposable M52 UAT before ordinary editing".into();
            return;
        }
        let result = match action {
            "new" => empty_coordinator().map(|coordinator| {
                wb.coordinator = coordinator;
                wb.uat.exit();
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

    fn perform_uat_action(wb: &mut Workbench, action: &str) {
        use super::uat::UatAction;

        if action == "load" {
            match wb.uat.load() {
                Ok(()) => {
                    wb.notice =
                        "Disposable M52 UAT candidate loaded; ordinary save is disabled".into();
                }
                Err(error) => wb.notice = error,
            }
            return;
        }
        if action == "exit" {
            wb.uat.exit();
            wb.notice = "Exited M52 UAT; ordinary pre-existing workspace restored".into();
            return;
        }
        let Some(action) = UatAction::from_browser_key(action) else {
            return;
        };
        wb.notice = wb
            .uat
            .perform(action)
            .map_or_else(|error| error, |observation| observation.summary());
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
        let coordinator = wb.uat.coordinator_for_render(&wb.coordinator);
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
        let coordinator = wb.uat.coordinator_for_render(&wb.coordinator);
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
        required(document, "wb-viewport")?.set_inner_html(&super::scene::svg_markup(
            scene.as_ref(),
            accepted,
            selection,
            wb.construction_preview.as_ref(),
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
        let uat_panel = required(document, "wb-uat-panel")?;
        if let Some(markup) = wb.uat.panel_markup() {
            uat_panel.set_inner_html(&markup);
            uat_panel.set_attribute("data-uat-active", "true")?;
        } else {
            uat_panel.set_inner_html(super::uat::inactive_panel_markup());
            uat_panel.remove_attribute("data-uat-active")?;
        }
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
                set_disabled(&button, wb.uat.is_active())?;
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
        render_action_availability(document, coordinator, wb.uat.is_active())?;
        required(document, "workbench-root")?
            .set_attribute("data-editor-adapter", "retained-coordinator")?;
        Ok(())
    }

    fn render_action_availability(
        document: &Document,
        coordinator: &RetainedEditorCoordinator,
        uat_active: bool,
    ) -> Result<(), JsValue> {
        for key in ["new", "finish", "cancel", "clear-selection"] {
            if let Some(button) = document.query_selector(&format!("[data-wb-action=\"{key}\"]"))? {
                set_disabled(&button, uat_active)?;
            }
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
                set_disabled(&button, uat_active || !enabled(action))?;
            }
        }
        let constraint_key =
            select_value(document, "wb-constraint-kind").unwrap_or_else(|| "fixed".into());
        if let Some(kind) = constraint_from_key(&constraint_key)
            && let Some(button) = document.query_selector("[data-wb-action=\"constraint\"]")?
        {
            set_disabled(
                &button,
                uat_active || !enabled(CoordinatorActionKind::Constraint(kind)),
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
            set_disabled(&button, uat_active || !dimension_enabled)?;
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
        let Some(snapshot) = wb.uat.persistence_snapshot(&wb.coordinator) else {
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
    fn required(document: &Document, id: &str) -> Result<Element, JsValue> {
        document
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing #{id}")))
    }
}
