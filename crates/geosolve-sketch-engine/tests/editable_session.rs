// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::float_cmp,
    reason = "fixtures require exact accepted scalar/point and history preservation"
)]

use geosolve_sketch::{CurveDefinition, DesignScalarId};
use geosolve_sketch_code::{CodeDraftProvenance, CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine::{AcceptedEvaluation, EditableSession};

fn project(radius: u32) -> CodeProject {
    let json = match radius {
        2 => include_str!("fixtures/session-radius-2.json"),
        5 => include_str!("fixtures/session-radius-5.json"),
        _ => panic!("unsupported fixture"),
    };
    CodeProject::managed(
        ProjectKey("headless-session".into()),
        CompiledManagedSource::from_json(json).unwrap(),
    )
    .unwrap()
}
fn radius(accepted: &AcceptedEvaluation) -> f64 {
    let CurveDefinition::Circle { radius, .. } =
        accepted.result().geometry.curves[0].curve.definition
    else {
        panic!("expected circle")
    };
    scalar(accepted, radius)
}
fn scalar(accepted: &AcceptedEvaluation, id: DesignScalarId) -> f64 {
    accepted
        .result()
        .geometry
        .scalars
        .iter()
        .find(|scalar| scalar.id == id)
        .unwrap()
        .value
}

#[test]
fn project_history_authenticates_tokens_and_retains_exact_accepted_results_on_failure() {
    let first = project(2).to_canonical_json().unwrap();
    let next = project(5).to_canonical_json().unwrap();
    let mut session = EditableSession::open(&first, None).unwrap();
    let initial = session.state();
    let old = session.accepted().clone();
    assert!(initial.result.capabilities.managed_source_edits);
    assert!(!initial.can_undo && !initial.can_redo);
    assert_eq!(radius(&old), 2.0);
    let updated = session.apply_project(&initial.token, &next).unwrap();
    assert_eq!(radius(&updated), 5.0);
    assert_eq!(radius(&old), 2.0);
    assert!(session.state().can_undo);
    assert!(session.apply_project(&initial.token, &first).is_err());
    let before = session.state();
    assert!(session.apply_project(&before.token, "{}").is_err());
    let mut foreign = project(2);
    foreign.project = ProjectKey("foreign".into());
    assert!(
        session
            .apply_project(&before.token, &foreign.to_canonical_json().unwrap())
            .is_err()
    );
    let other = EditableSession::open(&first, None).unwrap();
    assert!(session.undo(other.token()).is_err());
    assert_eq!(session.state(), before);
    let restored = session.undo(&before.token).unwrap();
    assert_eq!(restored.result(), old.result());
    let token = session.token().clone();
    let redone = session.redo(&token).unwrap();
    assert_eq!(redone.result(), updated.result());
    assert_eq!(
        old.export_profiles(0.02).unwrap()["regions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let token = session.token().clone();
    session.undo(&token).unwrap();
    let token = session.token().clone();
    session.apply_project(&token, &next).unwrap();
    assert!(!session.state().can_redo);
    let token = session.token().clone();
    assert_eq!(radius(&session.undo(&token).unwrap()), 2.0);
}

#[test]
fn semantic_design_restores_point_override_without_checkpoint_or_solved_geometry() {
    let project = project(2);
    let json = project.to_canonical_json().unwrap();
    let mut session = EditableSession::open(&json, None).unwrap();
    // Obtain authored writable addresses from the ordinary shared expansion.
    let materialized = geosolve_sketch_code::materialize_code_project_cold(
        &project,
        &session.design().generated,
        geosolve_sketch_intent::IntentSessionId::from_raw(980_123),
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(980_124)),
        1.0,
    )
    .unwrap();
    let writable = &materialized.expansion.writable_points[0];
    let mut overlay = session.design().overrides;
    overlay
        .set_points_atomically(
            writable.edit.point_updates([10.0, 20.0]),
            CodeDraftProvenance::CanvasDrag,
        )
        .unwrap();
    let token = session.token().clone();
    let moved = session.apply_overlay(&token, overlay).unwrap();
    assert!(
        moved
            .result()
            .geometry
            .points
            .iter()
            .any(|point| point.position == [10.0, 20.0])
    );
    let design = serde_json::to_string(&session.design()).unwrap();
    assert!(!design.contains("editor_checkpoint") && !design.contains("accepted_state"));
    let restored = EditableSession::open(&json, Some(&design)).unwrap();
    assert_eq!(serde_json::to_string(&restored.design()).unwrap(), design);
    assert!(!restored.state().can_undo);
    assert!(
        restored
            .accepted()
            .result()
            .validation
            .hard_residuals_validated
    );
    assert!(
        restored
            .accepted()
            .result()
            .geometry
            .points
            .iter()
            .any(|point| point.position == [10.0, 20.0])
    );
    let token = session.token().clone();
    let undone = session.undo(&token).unwrap();
    assert!(
        undone
            .result()
            .geometry
            .points
            .iter()
            .any(|point| point.position == [0.0, 0.0])
    );
    let mut invalid = restored.design();
    invalid.project = ProjectKey("foreign".into());
    assert!(EditableSession::open(&json, Some(&serde_json::to_string(&invalid).unwrap())).is_err());
    let mut forged = project;
    forged
        .managed
        .source
        .push_str("// changed without compiler receipt");
    assert!(EditableSession::open(&serde_json::to_string(&forged).unwrap(), None).is_err());
}

#[test]
fn persistable_history_restores_both_directions_and_retains_failed_edits() {
    let first = project(2).to_canonical_json().unwrap();
    let next = project(5).to_canonical_json().unwrap();
    let mut session = EditableSession::open_persistable(&first, None).unwrap();
    let initial_geometry = session.accepted().result().geometry.clone();
    let initial_design = session.design();
    let token = session.token().clone();
    session.apply_project(&token, &next).unwrap();
    let middle_geometry = session.accepted().result().geometry.clone();
    let middle_design = session.design();
    let token = session.token().clone();
    session.apply_project(&token, &first).unwrap();
    let token = session.token().clone();
    session.undo(&token).unwrap();
    assert!(session.state().can_undo && session.state().can_redo);
    let wire = session.export_history().unwrap();
    let mut restored = EditableSession::restore_history(&wire).unwrap();
    assert_eq!(restored.export_history().unwrap(), wire);
    assert_eq!(restored.token(), session.token());
    assert_eq!(restored.export_project_json().unwrap(), next);
    assert_eq!(restored.design(), middle_design);
    assert_eq!(restored.accepted().result().geometry, middle_geometry);
    assert_eq!(radius(restored.accepted()), 5.0);
    assert!(
        restored
            .accepted()
            .result()
            .validation
            .hard_residuals_validated
    );
    assert!(
        restored
            .accepted()
            .result()
            .validation
            .all_active_features_current
    );
    let before = restored.state();
    assert!(restored.apply_project(&before.token, "{}").is_err());
    assert_eq!(restored.state(), before);
    assert_eq!(restored.export_history().unwrap(), wire);
    restored.undo(&before.token).unwrap();
    assert_eq!(restored.accepted().result().geometry, initial_geometry);
    assert_eq!(restored.design(), initial_design);
    assert_eq!(restored.export_project_json().unwrap(), first);
    assert!(!restored.state().can_undo && restored.state().can_redo);
    let token = restored.token().clone();
    restored.redo(&token).unwrap();
    assert_eq!(restored.accepted().result().geometry, middle_geometry);
    let token = restored.token().clone();
    restored.redo(&token).unwrap();
    assert_eq!(radius(restored.accepted()), 2.0);
    assert!(restored.state().can_undo && !restored.state().can_redo);
    let final_wire = restored.export_history().unwrap();
    let cold = EditableSession::restore_history(&final_wire).unwrap();
    assert_eq!(cold.export_history().unwrap(), final_wire);
    assert_eq!(
        cold.accepted().result().geometry,
        restored.accepted().result().geometry
    );
}

#[test]
fn history_rejects_corrupt_nested_authority_and_ephemeral_export() {
    let first = project(2).to_canonical_json().unwrap();
    let next = project(5).to_canonical_json().unwrap();
    assert!(
        EditableSession::open(&first, None)
            .unwrap()
            .export_history()
            .is_err()
    );
    let mut session = EditableSession::open_persistable(&first, None).unwrap();
    let token = session.token().clone();
    session.apply_project(&token, &next).unwrap();
    let token = session.token().clone();
    session.apply_project(&token, &first).unwrap();
    let token = session.token().clone();
    session.undo(&token).unwrap();
    let wire = session.export_history().unwrap();
    for direction in ["snapshot", "undo", "redo"] {
        for checkpoint in ["editor_checkpoint", "accepted_editor_checkpoint"] {
            let mut corrupt: serde_json::Value = serde_json::from_str(&wire).unwrap();
            let snapshot = if direction == "snapshot" {
                &mut corrupt["snapshot"]
            } else {
                &mut corrupt[direction][0]["snapshot"]
            };
            snapshot[checkpoint] = serde_json::json!("corrupt native checkpoint");
            assert!(EditableSession::restore_history(&corrupt.to_string()).is_err());
        }
    }
    assert_eq!(session.export_history().unwrap(), wire);
    assert_eq!(radius(session.accepted()), 5.0);
}

#[test]
fn existing_source_workspace_preserves_dirty_utf8_draft_and_legacy_history() {
    use geosolve_sketch_code::authoring_persistence::SourceWorkspaceOrigin;
    use geosolve_sketch_engine::SourceWorkspacePresentation;
    let first = project(2).to_canonical_json().unwrap();
    let next = project(5).to_canonical_json().unwrap();
    let mut session = EditableSession::open_persistable(&first, None).unwrap();
    let token = session.token().clone();
    session.apply_project(&token, &next).unwrap();
    let presentation = SourceWorkspacePresentation {
        origin: SourceWorkspaceOrigin::Authored,
        selected_file: "sketch.ts".into(),
        managed_draft: "// unfinished draft 😀\ninvalid".into(),
        draft_diagnostic: None,
    };
    let encoded = session.export_source_workspace(&presentation).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(wire["version"], "geosolve-code-workbench-v5");
    assert_eq!(wire["managed_draft"], presentation.managed_draft);
    for version in ["geosolve-code-workbench-v4", "geosolve-code-workbench-v5"] {
        let mut candidate = wire.clone();
        candidate["version"] = serde_json::json!(version);
        if version.ends_with("v4") {
            candidate["session"] = serde_json::json!(session.export_history().unwrap());
        }
        let (mut restored, personal) =
            EditableSession::restore_source_workspace(&candidate.to_string()).unwrap();
        assert_eq!(personal, presentation);
        assert_eq!(
            restored.export_source_workspace(&personal).unwrap(),
            encoded
        );
        assert_eq!(restored.export_project_json().unwrap(), next);
        assert_eq!(
            restored.export_history().unwrap(),
            session.export_history().unwrap()
        );
        assert_eq!(
            restored.accepted().result().geometry,
            session.accepted().result().geometry
        );
        let token = restored.token().clone();
        restored.undo(&token).unwrap();
        assert_eq!(radius(restored.accepted()), 2.0);
        assert_eq!(restored.export_project_json().unwrap(), first);
    }
    let mut foreign = wire.clone();
    foreign["project"] = serde_json::json!(first);
    assert!(EditableSession::restore_source_workspace(&foreign.to_string()).is_err());
    let mut missing_file = presentation.clone();
    missing_file.selected_file = "missing.ts".into();
    assert!(session.export_source_workspace(&missing_file).is_err());
    let duplicate = encoded.replacen(
        "\"managed_draft\":",
        "\"managed_draft\":\"forged\",\"managed_draft\":",
        1,
    );
    assert!(EditableSession::restore_source_workspace(&duplicate).is_err());
    assert_eq!(
        session.export_source_workspace(&presentation).unwrap(),
        encoded
    );
}
