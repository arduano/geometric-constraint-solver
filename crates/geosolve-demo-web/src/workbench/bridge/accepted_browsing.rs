// SPDX-License-Identifier: GPL-3.0-or-later
//! Accepted model boundary for read-only chrome adapters.
use super::*;
use geosolve_constraint_editor::detached_interaction::{
    DetachedInteractionSeed, DetachedInteractionState,
};
use geosolve_sketch_engine::{AcceptedBrowsingView, EditableDesign, EditableSession};

/// Engine-owned accepted browsing, with canonical source/design admission once.
/// This contains no workbench bridge, source transaction owner or mutation history.
pub(crate) struct AcceptedBrowsingModel {
    view: AcceptedBrowsingView,
    project: Option<String>,
    design: Option<EditableDesign>,
    seed: DetachedInteractionSeed,
    presentation: Option<WorkbenchPresentationPersistence>,
}

impl AcceptedBrowsingModel {
    pub(crate) fn new(encoded: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            project: Option<String>,
            design: Option<EditableDesign>,
            generated: Option<String>,
            presentation: Option<WorkbenchPresentationPersistence>,
            seed: DetachedInteractionSeed,
        }
        let request: Request = decode_request(encoded)?;
        if let Some(presentation) = &request.presentation {
            presentation.validate()?;
        }
        let (session, project, design) = match (request.project, request.design, request.generated)
        {
            (Some(project), Some(design), None) => {
                let design_json =
                    serde_json::to_string(&design).map_err(|error| error.to_string())?;
                let session = EditableSession::open(&project, Some(&design_json))
                    .map_err(|error| error.to_string())?;
                let canonical = session
                    .export_project_json()
                    .map_err(|error| error.to_string())?;
                let expected: serde_json::Value =
                    serde_json::from_str(&project).map_err(|error| error.to_string())?;
                let actual: serde_json::Value =
                    serde_json::from_str(&canonical).map_err(|error| error.to_string())?;
                if expected != actual || design != session.design() {
                    return Err(
                        "Browsing reconstruction disagrees with accepted source/design".into(),
                    );
                }
                (
                    session.browsing().map_err(|error| error.to_string())?,
                    Some(canonical),
                    Some(design),
                )
            }
            (None, None, Some(generated)) => {
                let accepted = geosolve_sketch_engine::SketchEngine::new()
                    .evaluate_generated_json(&generated)
                    .map_err(|error| error.to_string())?;
                if request.seed.scene_key != accepted.result().result_id {
                    return Err("Generated browsing seed belongs to a different result".into());
                }
                (
                    accepted.browsing().map_err(|error| error.to_string())?,
                    None,
                    None,
                )
            }
            _ => {
                return Err("Expected one managed project/design or one generated artifact".into());
            }
        };
        let view = AcceptedBrowsingView::new(session, request.seed.clone())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            view,
            project,
            design,
            seed: request.seed,
            presentation: request.presentation,
        })
    }

    pub(crate) fn view(&self) -> &AcceptedBrowsingView {
        &self.view
    }
    pub(crate) fn project(&self) -> &str {
        self.project.as_deref().expect("managed browsing project")
    }
    pub(crate) fn design(&self) -> &EditableDesign {
        self.design.as_ref().expect("managed browsing design")
    }
    pub(crate) fn update(&mut self, state: DetachedInteractionState) -> Result<(), String> {
        self.view.update(state).map_err(|error| error.to_string())
    }
}

/// Read-only accepted chrome with personal presentation caches. This never owns
/// a workbench, code transaction, compiler job, or durable history.
pub(crate) struct BrowsingPresentation {
    pub(super) model: AcceptedBrowsingModel,
    dimensions: DimensionBridgeState,
    navigation: NavigationState,
}
impl BrowsingPresentation {
    pub(crate) fn new(encoded: &str) -> Result<Self, String> {
        Ok(Self {
            model: AcceptedBrowsingModel::new(encoded)?,
            dimensions: DimensionBridgeState::default(),
            navigation: NavigationState::default(),
        })
    }
    pub(crate) fn initialize_json(&mut self) -> Result<String, String> {
        let view = &mut self.model.view;
        let session = view.session();
        let read = ChromeRead {
            editor: session.editor(),
            code_project: CodeChrome::accepted(session),
            dimension_instance: self.dimensions.instance(),
            pending: false,
            interaction_blocked: false,
            hidden_rows: &self.model.seed.visibility.hidden_rows,
        };
        let visibility = super::local_interaction::browsing_visibility_seed(
            &read,
            view.scene(),
            view.source_mapping(),
        )?;
        let mut scene = view.scene().clone();
        let mut dimensions =
            self.dimensions
                .initialize_browsing(&read, &mut scene, view.dimensions());
        dimensions.state.mode = self.model.seed.dimensions.state.mode;
        if let Some(presentation) = &self.model.presentation {
            DimensionBridgeState::restore_browsing_preferences(
                &mut dimensions,
                &presentation.dimensions,
                view.source_mapping(),
            )?;
        }
        let seed = &mut self.model.seed;
        seed.dimensions = view
            .initialize_presentation(dimensions, visibility.clone())
            .map_err(|error| error.to_string())?;
        seed.visibility_seed = visibility;
        if let Some(presentation) = &self.model.presentation {
            seed.visibility = geosolve_constraint_editor::VisibilityState {
                hidden_rows: presentation.hidden_rows.iter().cloned().collect(),
                isolate_restore: presentation
                    .isolate_restore
                    .as_ref()
                    .map(|rows| rows.iter().cloned().collect()),
                construction_visible: presentation.construction_visible,
            };
            seed.visibility.reconcile(&seed.visibility_seed);
        }
        let mut canvas = super::local_interaction::LocalInteraction::new(
            &serde_json::to_string(seed).map_err(|error| error.to_string())?,
        )?;
        let state =
            serde_json::from_str(&canvas.state_json()?).map_err(|error| error.to_string())?;
        let frame = canvas.compose_frame()?;
        self.apply_state(state)?;
        let chrome = self.chrome()?;
        let snapshot = self.initial_snapshot(frame, chrome)?;
        serde_json::to_string(&serde_json::json!({
            "snapshot": snapshot, "seed": self.model.seed,
            "toolCatalog": super::super::command_manifest::tool_catalog(),
        }))
        .map_err(|error| error.to_string())
    }

    fn initial_snapshot(
        &self,
        frame: FrameSnapshot,
        chrome: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let files =
            if let Some(project) = &self.model.project {
                let project = geosolve_sketch_code::CodeProject::from_json(project)
                    .map_err(|error| error.to_string())?;
                let mut files = vec![SourceFileSnapshot {
                    path: "sketch.ts".into(),
                    language: "typescript",
                    contents: project.managed.source,
                    read_only: false,
                }];
                files.extend(project.custom_files.iter().map(|(path, module)| {
                    SourceFileSnapshot {
                        path: path.clone(),
                        language: source_language(path),
                        contents: module.contents.clone(),
                        read_only: true,
                    }
                }));
                files
            } else {
                Vec::new()
            };
        let mut snapshot = chrome;
        let fields = snapshot.as_object_mut().ok_or("invalid browsing chrome")?;
        fields.insert("version".into(), serde_json::json!(PROTOCOL_VERSION));
        fields.insert("revision".into(), serde_json::json!(0));
        fields.insert(
            "project".into(),
            serde_json::json!({"title":self.model.seed.title, "status":"accepted"}),
        );
        fields.insert(
            "frame".into(),
            serde_json::to_value(frame).map_err(|error| error.to_string())?,
        );
        fields.insert(
            "source".into(),
            serde_json::to_value(SourceSnapshot {
                selected_path: "sketch.ts".into(),
                files,
                dirty: false,
            })
            .map_err(|error| error.to_string())?,
        );
        let selected_geometry_role = fields.remove("selectedGeometryRole");
        fields.insert(
            "presentation".into(),
            serde_json::json!({
                "activeTool":"select", "gridVisible":self.model.seed.grid_visible,
                "constructionVisible":self.model.seed.visibility.construction_visible,
                "visibilityRestoreAvailable":self.model.seed.visibility.isolate_restore.is_some(),
                "canUndo":false,"canRedo":false,"canFinish":false,"geometryRole":"profile",
            }),
        );
        if let Some(role) = selected_geometry_role.filter(|value| !value.is_null()) {
            fields
                .get_mut("presentation")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or("invalid browsing presentation")?
                .insert("selectedGeometryRole".into(), role);
        }
        for key in ["selection", "authoringDocument"] {
            if fields.get(key).is_some_and(serde_json::Value::is_null) {
                fields.remove(key);
            }
        }
        fields.remove("constructionVisible");
        fields.remove("visibilityRestoreAvailable");
        Ok(snapshot)
    }

    fn read(&self) -> ChromeRead<'_> {
        let session = self.model.view().session();
        ChromeRead {
            editor: session.editor(),
            code_project: CodeChrome::accepted(session),
            dimension_instance: self.dimensions.instance(),
            pending: false,
            interaction_blocked: false,
            hidden_rows: &self
                .model
                .view()
                .state()
                .expect("accepted browsing state installed")
                .visibility
                .hidden_rows,
        }
    }
    fn apply_state(&mut self, state: DetachedInteractionState) -> Result<(), String> {
        self.model.update(state)?;
        let view = self.model.view();
        let session = view.session();
        let read = ChromeRead {
            editor: session.editor(),
            code_project: CodeChrome::accepted(session),
            dimension_instance: self.dimensions.instance(),
            pending: false,
            interaction_blocked: false,
            hidden_rows: &view
                .state()
                .expect("accepted browsing state installed")
                .visibility
                .hidden_rows,
        };
        self.dimensions.install_browsing(&read, view);
        Ok(())
    }
    pub(crate) fn update_json(&mut self, encoded: &str) -> Result<String, String> {
        self.apply_state(decode_request(encoded)?)?;
        serde_json::to_string(&self.chrome()?).map_err(|error| error.to_string())
    }
    fn chrome(&mut self) -> Result<serde_json::Value, String> {
        let view = self.model.view();
        let session = view.session();
        let state = view.state().expect("accepted browsing state installed");
        let read = ChromeRead {
            editor: session.editor(),
            code_project: CodeChrome::accepted(session),
            dimension_instance: self.dimensions.instance(),
            pending: false,
            interaction_blocked: false,
            hidden_rows: &state.visibility.hidden_rows,
        };
        let navigation = self.navigation.snapshot(&read, Some(view.scene()));
        let explorer = self
            .navigation
            .explorer_snapshot(session.editor(), Some(view.scene()));
        let selection = self
            .navigation
            .selection_snapshot(&read, Some(view.scene()), &navigation);
        let selected_geometry_role = read
            .editor()
            .selected_geometry_role_state()
            .ok()
            .flatten()
            .map(|state| match state {
                GeometryRoleSelectionState::Profile => "profile",
                GeometryRoleSelectionState::Construction => "construction",
                GeometryRoleSelectionState::Mixed => "mixed",
            });
        Ok(serde_json::json!({
            "explorer":explorer, "navigation":navigation,
            "dimensions":self.dimensions.browsing_snapshot(&view.dimensions().ids)?,
            "authoringDocument":read.authoring_document_snapshot(), "selection":selection,
            "parameters":read.parameter_snapshot(), "problems":read.accepted_problems(),
            "selectedGeometryRole":selected_geometry_role, "constructionVisible":state.visibility.construction_visible,
            "visibilityRestoreAvailable":state.visibility.isolate_restore.is_some(),
        }))
    }
    pub(crate) fn navigate_json(&mut self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            state: DetachedInteractionState,
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
        self.apply_state(request.state)?;
        let prepared = match request.command.as_str() {
            "navigation.rows.select" => self.navigation.prepare_rows_json(
                &self.read(),
                Some(self.model.view().scene()),
                false,
                request.payload,
            )?,
            "navigation.source.select" => self.navigation.prepare_source_json(
                &self.read(),
                Some(self.model.view().scene()),
                false,
                request.payload,
            )?,
            _ => unreachable!("whitelisted navigation"),
        };
        self.model
            .view
            .restore_navigation(prepared.selection, prepared.logical_owner)
            .map_err(|error| error.to_string())?;
        self.navigation = prepared.navigation;
        let state = self
            .model
            .view()
            .state()
            .expect("accepted browsing state installed")
            .clone();
        self.apply_state(state.clone())?;
        serde_json::to_string(&serde_json::json!({"state":state, "chrome":self.chrome()?}))
            .map_err(|error| error.to_string())
    }
    pub(crate) fn describe_json(&mut self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            state: DetachedInteractionState,
            authority: String,
            command: String,
            payload: serde_json::Value,
        }
        let request: Request = decode_request(encoded)?;
        self.apply_state(request.state)?;
        let read = self.read();
        read.validate_metadata_authority(&request.authority)?;
        let mutation = match request.command.as_str() {
            "parameter.edit" => {
                let payload: ParameterPayload = decode_payload(request.payload)?;
                read.parameter_source_mutation(&payload.id, payload.value)?
            }
            "dimensions.edit" => {
                let payload: ParameterPayload = decode_payload(request.payload)?;
                self.dimensions.browsing_source_mutation(
                    &read,
                    self.model.view(),
                    &payload.id,
                    &payload.value,
                )?
            }
            "authoring.metadata.set" => Some(read.metadata_source_mutation(request.payload)?),
            "authoring.parameter.extract" => {
                Some(read.extraction_source_mutation(request.payload)?)
            }
            "declaration.move" => {
                read.declaration_move_mutation(&decode_payload(request.payload)?)?
            }
            "declaration.suppression.set" => {
                let payload: DeclarationSuppressionPayload = decode_payload(request.payload)?;
                Some(read.suppression_source_mutation(&payload.id, payload.suppressed)?)
            }
            "declaration.delete" => {
                let payload: SelectionPayload = decode_payload(request.payload)?;
                if let Some(DeclarationRowTarget::Managed {
                    closure_role: ManagedDeclarationClosureRole::Helper { root },
                    ..
                }) = read.declaration_row_target(&payload.id)
                {
                    return Err(format!(
                        "Profile Offset helper declarations cannot be deleted independently of `{}`",
                        root.0
                    ));
                }
                Some(ManagedSketchMutation::Delete {
                    target: read.managed_row_target(&payload.id)?,
                })
            }
            _ => return Err("Unsupported native source edit description".into()),
        };
        serde_json::to_string(&mutation).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
impl BrowsingPresentation {
    pub(super) fn export_project_json(&self) -> Result<String, String> {
        serde_json::to_string(&ExportSnapshot {
            version: PROTOCOL_VERSION,
            filename: "project.json",
            contents: self.model.project().to_owned(),
        })
        .map_err(|error| error.to_string())
    }
    pub(super) fn workspace_design_json(&self) -> Result<String, String> {
        serde_json::to_string(self.model.design()).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsing_initialize_enriches_engine_seed_in_its_original_namespace() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("native-browser-start".into()),
            compiled,
        )
        .unwrap()
        .to_canonical_json()
        .unwrap();
        let session = EditableSession::open(&project, None).unwrap();
        let seed = session.interaction_seed(None).unwrap();
        assert!(seed.dimensions.ids.is_empty());
        let source_scene = EditorScene::from_detached_json(&seed.scene).unwrap();
        let mut browser = BrowsingPresentation::new(
            &serde_json::json!({
                "project":project,"design":session.design(),"seed":seed,
            })
            .to_string(),
        )
        .unwrap();
        let startup: serde_json::Value =
            serde_json::from_str(&browser.initialize_json().unwrap()).unwrap();
        assert_eq!(startup["seed"]["scene"], seed.scene);
        assert_eq!(startup["seed"]["sceneKey"], seed.scene_key);
        assert_eq!(
            startup["seed"]["bindings"],
            serde_json::to_value(&seed.bindings).unwrap()
        );
        assert!(
            !startup["snapshot"]["frame"]["scene"]["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let enriched: DetachedInteractionSeed =
            serde_json::from_value(startup["seed"].clone()).unwrap();
        assert!(!enriched.dimensions.ids.is_empty());
        for key in enriched.dimensions.ids.values() {
            assert_eq!(key.document, source_scene.presentation_document().id());
        }
        let mut local =
            super::super::local_interaction::LocalInteraction::new(&startup["seed"].to_string())
                .unwrap();
        let state: serde_json::Value = serde_json::from_str(&local.state_json().unwrap()).unwrap();
        let selected = browser.navigate_json(&serde_json::json!({
            "state":state,"command":"navigation.rows.select","payload":{
                "authority":startup["snapshot"]["navigation"]["authority"],"ids":["managed:bar"],"mode":"replace"
            }
        }).to_string()).unwrap();
        let selected: serde_json::Value = serde_json::from_str(&selected).unwrap();
        assert!(
            !selected["state"]["selection"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        local
            .restore_selection_json(
                &serde_json::json!({"expected":state,"state":selected["state"]}).to_string(),
            )
            .unwrap();
        let mut state: DetachedInteractionState =
            serde_json::from_str(&local.state_json().unwrap()).unwrap();
        state.viewport.pixels_per_model_unit *= 1.2;
        browser
            .update_json(&serde_json::to_string(&state).unwrap())
            .unwrap();
        assert_eq!(browser.model.project(), project);
        assert_eq!(
            browser.model.view().session().accepted().result().result_id,
            session.accepted().result().result_id
        );
        assert!(browser.initialize_json().is_err());
    }

    #[test]
    fn accepted_browsing_model_maps_a_foreign_scene_without_owning_a_workbench() {
        let compiled =
            geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
            )))
            .unwrap();
        let project = geosolve_sketch_code::CodeProject::managed(
            geosolve_sketch_code::ProjectKey("accepted-browser-adapter".into()),
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
        let mut browser = AcceptedBrowsingModel::new(
            &serde_json::json!({"project":project,"design":design,"seed":pair["seed"]}).to_string(),
        )
        .unwrap();
        let local = geosolve_constraint_editor::detached_interaction::DetachedCanvas::new(
            &pair["seed"].to_string(),
        )
        .unwrap();
        let mut state = local.state();
        state.selection = vec![SelectionItem::Point(local.scene.points[0].id)];
        let before = browser.project().to_owned();
        let native = std::ptr::from_ref(
            browser
                .view()
                .session()
                .editor()
                .coordinator()
                .accepted_materialization()
                .unwrap(),
        );
        for zoom in [3.0, 7.0, 12.0] {
            state.viewport.pixels_per_model_unit = zoom;
            browser.update(state.clone()).unwrap();
            assert_eq!(
                browser.view().source_selection().unwrap().items,
                state.selection
            );
            assert_eq!(browser.project(), before);
            assert_eq!(
                std::ptr::from_ref(
                    browser
                        .view()
                        .session()
                        .editor()
                        .coordinator()
                        .accepted_materialization()
                        .unwrap()
                ),
                native
            );
            assert_eq!(serde_json::to_value(browser.design()).unwrap(), design);
        }
        let accepted = browser.view().state().cloned();
        state.dimension_pins = vec!["foreign dimension".into()];
        assert!(browser.update(state).is_err());
        assert_eq!(browser.view().state(), accepted.as_ref());
        assert_eq!(browser.project(), before);
    }
}
