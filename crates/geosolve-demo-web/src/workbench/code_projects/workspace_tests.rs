// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn m98_semantic_design_reconstructs_point_override_without_solved_geometry() {
    let compiled = CompiledManagedSource::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/geosolve-sketch-code/test/fixtures/managed-empty-circle.json"
    )))
    .unwrap();
    let project = CodeProject::managed(ProjectKey("folder-design".into()), compiled).unwrap();
    let (initial, _) =
        CodeProjectWorkbench::open_project(CodeProjectOrigin::Authored, project.clone()).unwrap();
    let expansion = &initial.materialized.as_ref().unwrap().expansion;
    let writable = expansion.writable_points.first().unwrap();
    let mut design = initial.workspace_design();
    design
        .overrides
        .set_points_atomically(
            writable.edit.point_updates([10.0, 20.0]),
            geosolve_sketch_code::CodeDraftProvenance::CanvasDrag,
        )
        .unwrap();
    let json = serde_json::to_string(&design).unwrap();
    assert!(!json.contains("editor_checkpoint"));
    assert!(!json.contains("accepted_state"));
    let decoded = serde_json::from_str(&json).unwrap();
    let (restored, editor) =
        CodeProjectWorkbench::open_workspace_design(project.clone(), decoded).unwrap();
    let point = restored
        .materialized
        .as_ref()
        .unwrap()
        .expansion
        .writable_points
        .first()
        .unwrap();
    let preview = TerminalPointPreview::from_accepted(&editor).unwrap();
    assert_eq!(preview.position(&point.handle), Some([10.0, 20.0]));
    assert_eq!(restored.project, project);
    assert_eq!(
        serde_json::to_string(&restored.workspace_design()).unwrap(),
        json
    );
    assert!(!restored.session.can_undo());
    let accepted = editor.coordinator().accepted_materialization().unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    let mut foreign = design;
    foreign.project = ProjectKey("foreign".into());
    assert!(CodeProjectWorkbench::open_workspace_design(project, foreign).is_err());
}
