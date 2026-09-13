// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent standalone host policies over the shared authoring owner.
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey, SketchCodeSession};
use geosolve_sketch_engine::{EditableSession, PersistentSourceApply};

fn circle(radius: u32) -> CodeProject {
    let bytes = match radius {
        2 => include_str!("fixtures/session-radius-2.json"),
        5 => include_str!("fixtures/session-radius-5.json"),
        _ => panic!("unsupported fixture"),
    };
    CodeProject::managed(
        ProjectKey("standalone-owner".into()),
        CompiledManagedSource::from_json(bytes).unwrap(),
    )
    .unwrap()
}

#[test]
fn typed_history_admission_rejects_corrupt_retained_native_checkpoints() {
    let first = circle(2);
    let session =
        EditableSession::open_persistable(&first.to_canonical_json().unwrap(), None).unwrap();
    let wire = session.export_history().unwrap();
    let cold =
        EditableSession::from_source_session(SketchCodeSession::from_json(&wire).unwrap()).unwrap();
    assert_eq!(cold.export_history().unwrap(), wire);
    for direction in ["undo", "redo"] {
        let mut history = session.source_session().clone();
        let valid = history.pointer_frame_checkpoint().clone();
        for checkpoint in [serde_json::json!("invalid native checkpoint"), valid] {
            let snapshot = history.snapshot();
            let edit = history
                .prepare_project_overlay(
                    history.identity(),
                    snapshot.interaction_overlay.clone(),
                    snapshot.expansion.clone().unwrap(),
                    checkpoint,
                    "opaque host checkpoint",
                )
                .unwrap();
            history.apply_prepared(edit).unwrap();
        }
        if direction == "redo" {
            history.undo().unwrap().unwrap();
            history.undo().unwrap().unwrap();
        }
        let opaque = SketchCodeSession::from_json(&history.to_canonical_json().unwrap()).unwrap();
        assert!(
            EditableSession::from_source_session(opaque).is_err(),
            "{direction}"
        );
    }
    assert_eq!(session.export_history().unwrap(), wire);
}

fn contact(limited: bool) -> CodeProject {
    let bytes = if limited {
        include_str!(
            "../../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-limited.json"
        )
    } else {
        include_str!(
            "../../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-base.json"
        )
    };
    CodeProject::managed(
        ProjectKey("standalone-contact".into()),
        CompiledManagedSource::from_json(bytes).unwrap(),
    )
    .unwrap()
}

#[test]
fn retained_source_failure_keeps_accepted_authority_through_history_and_cold_restore() {
    let project = contact(false);
    let candidate = contact(true);
    let mut session =
        EditableSession::open_persistable(&project.to_canonical_json().unwrap(), None).unwrap();
    let accepted = session.accepted().result().clone();
    let checkpoint = session.source_session().pointer_frame_checkpoint().clone();
    let token = session.token().clone();
    let PersistentSourceApply::RetainedFailure {
        receipt,
        diagnostic,
    } = session
        .apply_project_retaining_failure(candidate.clone())
        .unwrap()
    else {
        panic!("infeasible range must retain failure");
    };
    assert_eq!(receipt.before, token);
    assert!(receipt.retained_failure);
    assert!(diagnostic.contains("independently validated solution"));
    assert_eq!(session.accepted().result(), &accepted);
    assert_eq!(
        session.source_session().pointer_frame_checkpoint(),
        &checkpoint
    );
    assert_eq!(
        session.source_session().snapshot().code_project.as_ref(),
        Some(&candidate)
    );
    assert_eq!(
        session
            .source_session()
            .snapshot()
            .accepted_code_project
            .as_ref(),
        Some(&project)
    );
    let failed = session.export_history().unwrap();
    let mut cold =
        EditableSession::from_source_session(SketchCodeSession::from_json(&failed).unwrap())
            .unwrap();
    assert_eq!(cold.export_history().unwrap(), failed);
    assert_eq!(cold.accepted().result().geometry, accepted.geometry);
    assert!(cold.accepted().result().validation.hard_residuals_validated);
    assert!(
        cold.accepted()
            .result()
            .validation
            .all_active_features_current
    );
    assert!(cold.source_session().snapshot().failure.is_some());
    cold.step_source_history(true).unwrap().unwrap();
    assert!(cold.source_session().snapshot().failure.is_none());
    assert_eq!(
        cold.source_session().snapshot().code_project.as_ref(),
        Some(&project)
    );
    assert_eq!(cold.accepted().result().geometry, accepted.geometry);
    cold.step_source_history(false).unwrap().unwrap();
    assert!(cold.source_session().snapshot().failure.is_some());
    assert_eq!(
        cold.source_session().snapshot().code_project.as_ref(),
        Some(&candidate)
    );
    assert_eq!(cold.accepted().result().geometry, accepted.geometry);
    assert_eq!(
        cold.source_session().pointer_frame_checkpoint(),
        &checkpoint
    );
    assert_eq!(session.export_history().unwrap(), failed);
}

#[test]
fn structured_native_host_mutation_preserves_atomic_receipts_and_history_labels() {
    use geosolve_sketch_code::{
        ManagedPathSegment, ManagedSketchMutation, ManagedValue, PreparedManagedMutationReceipt,
        SemanticOutputPath, SemanticSymbol, UnitLiteral, derive_managed_value_mutation,
    };
    let original =
        CompiledManagedSource::from_json(include_str!("fixtures/authoring-radius-2.json")).unwrap();
    let changed =
        CompiledManagedSource::from_json(include_str!("fixtures/authoring-radius-5.json")).unwrap();
    let mutation = derive_managed_value_mutation(
        &original,
        &SemanticSymbol("bore".into()),
        &SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]),
        ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 5.0,
        }),
    )
    .unwrap();
    let project = CodeProject::managed(ProjectKey("standalone-mutation".into()), original).unwrap();
    let mut session =
        EditableSession::open_persistable(&project.to_canonical_json().unwrap(), None).unwrap();
    let prepared = session
        .prepare_structured_source_mutation(ManagedSketchMutation::SetValues {
            values: vec![mutation],
        })
        .unwrap();
    let receipt = PreparedManagedMutationReceipt {
        ticket_digest: prepared.request().ticket.ticket_digest.clone(),
        base_source_digest: prepared.request().ticket.accepted_source_digest.clone(),
        candidate_source_digest: changed.ir.source_digest.clone(),
        compiled: changed,
    };
    let before = session.export_history().unwrap();
    let mut forged = receipt.clone();
    forged.ticket_digest = "0".repeat(64);
    assert!(
        session
            .apply_canvas_source_mutation(&prepared, forged)
            .is_err()
    );
    assert_eq!(session.export_history().unwrap(), before);
    let published = session
        .apply_canvas_source_mutation(&prepared, receipt.clone())
        .unwrap();
    assert_eq!(published.label, "Apply managed source");
    assert!(
        session
            .accepted()
            .result()
            .validation
            .hard_residuals_validated
    );
    assert!(
        session
            .accepted()
            .result()
            .validation
            .all_active_features_current
    );
    let after = session.export_history().unwrap();
    assert!(
        session
            .apply_canvas_source_mutation(&prepared, receipt)
            .is_err()
    );
    assert_eq!(session.export_history().unwrap(), after);
    let undo = session.step_source_history(true).unwrap().unwrap();
    assert_eq!(undo.label, "Undo Apply managed source");
    let redo = session.step_source_history(false).unwrap().unwrap();
    assert_eq!(redo.label, "Redo Apply managed source");
}
