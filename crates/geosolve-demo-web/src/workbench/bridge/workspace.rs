// SPDX-License-Identifier: GPL-3.0-or-later
//! Complete file-workspace updates use the retained code-project transaction.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceProjectPayload {
    project: String,
    #[serde(default)]
    design: Option<super::super::code_projects::WorkspaceDesign>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceGeneratorPayload {
    artifact: String,
}

impl WorkbenchBridge {
    pub(crate) fn workspace_design_json(&self) -> Result<String, String> {
        let Some(code) = &self.code_project else {
            return Ok("null".into());
        };
        serde_json::to_string(&code.workspace_design()).map_err(|error| error.to_string())
    }

    pub(super) fn apply_workspace_generator(
        &mut self,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        let request: WorkspaceGeneratorPayload = decode_payload(payload)?;
        let generated = geosolve_sketch_code::GeneratedSketchArtifact::from_json(&request.artifact)
            .map_err(|error| error.to_string())?;
        let digest = geosolve_sketch_intent::intent_content_digest(request.artifact.as_bytes());
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest.bytes()[..16]);
        let identity = u128::from_be_bytes(bytes).max(1);
        let materialized = geosolve_sketch_code::materialize_generated_sketch_cold(
            &generated,
            geosolve_sketch_intent::IntentSessionId::from_raw(identity),
            geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(identity)),
            1.0,
        )
        .map_err(|error| error.to_string())?;
        let authority = WorkbenchDocumentAuthority::from_projectional_editor(materialized.editor)?;
        self.cancel_active_interaction(None)?;
        self.pending_managed_mutation = None;
        self.authority = authority;
        self.code_project = None;
        self.samples = super::super::samples::SampleCatalogState::default();
        self.reset_transient_tools();
        self.retained_scene = None;
        self.last_error = None;
        self.preserve_frame_once = false;
        self.bump_revision();
        self.notice = "Generator result accepted".into();
        Ok(())
    }

    pub(super) fn apply_workspace_project(
        &mut self,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        let request: WorkspaceProjectPayload = decode_payload(payload)?;
        let project = geosolve_sketch_code::CodeProject::from_json(&request.project)
            .map_err(|error| error.to_string())?;
        let live_project = self
            .code_project
            .as_ref()
            .ok_or("workspace requires a code project")?;
        if live_project.workspace_project_key() != project.project {
            return Err("workspace update belongs to a different project".into());
        }
        if let Some(design) = request.design {
            let (candidate, editor) = CodeProjectWorkbench::open_workspace_design(project, design)?;
            let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)?;
            self.cancel_active_interaction(None)?;
            self.authority = authority;
            self.code_project = Some(candidate);
            self.pending_managed_mutation = None;
            self.reconcile_explorer_visibility();
            self.retained_scene = None;
            self.last_error = None;
            self.preserve_frame_once = false;
            self.bump_revision();
            self.notice = "Project and design overrides accepted".into();
            return Ok(());
        }
        let code = self
            .code_project
            .as_ref()
            .ok_or("workspace requires a code project")?;
        let (candidate, publication) = code.prepare_workspace_project(project)?;
        let authority = WorkbenchDocumentAuthority::from_projectional_editor(*publication.editor)?;
        self.cancel_active_interaction(None)?;
        self.pending_managed_mutation = None;
        self.authority = authority;
        self.code_project = Some(candidate);
        self.reconcile_explorer_visibility();
        self.retained_scene = None;
        self.last_error = None;
        self.preserve_frame_once = false;
        self.bump_revision();
        self.notice = "Project files accepted".into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};

    #[test]
    fn m98_complete_workspace_update_retains_history_and_rejects_foreign_project() {
        let fixture = |bytes| CompiledManagedSource::from_json(bytes).unwrap();
        let base = fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-empty-circle.json"
        )));
        let next = fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-two-circles.json"
        )));
        let project = CodeProject::managed(ProjectKey("folder-project".into()), base).unwrap();
        let initial = project.to_canonical_json().unwrap();
        let mut bridge = WorkbenchBridge::restore(&initial).unwrap();
        let update = CodeProject::managed(project.project.clone(), next).unwrap();
        let candidate = update.to_canonical_json().unwrap();
        bridge
            .apply_workspace_project(serde_json::json!({ "project": candidate }))
            .unwrap();
        let exported: serde_json::Value =
            serde_json::from_str(&bridge.export_project_json().unwrap()).unwrap();
        let accepted = CodeProject::from_json(exported["contents"].as_str().unwrap()).unwrap();
        assert_eq!(accepted, update);
        bridge.step_history(true).unwrap();
        let exported: serde_json::Value =
            serde_json::from_str(&bridge.export_project_json().unwrap()).unwrap();
        assert_eq!(
            CodeProject::from_json(exported["contents"].as_str().unwrap()).unwrap(),
            project
        );
        bridge.step_history(false).unwrap();
        let before = bridge.persistence_contents().unwrap();
        let mut foreign = update;
        foreign.project = ProjectKey("foreign".into());
        assert!(
            bridge
                .apply_workspace_project(
                    serde_json::json!({ "project": foreign.to_canonical_json().unwrap() })
                )
                .is_err()
        );
        assert_eq!(bridge.persistence_contents().unwrap(), before);
        let mut design = bridge.code_project.as_ref().unwrap().workspace_design();
        design.project = foreign.project.clone();
        assert!(
            bridge
                .apply_workspace_project(serde_json::json!({
                    "project": foreign.to_canonical_json().unwrap(), "design": design,
                }))
                .is_err()
        );
        assert_eq!(bridge.persistence_contents().unwrap(), before);
    }
}
