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
