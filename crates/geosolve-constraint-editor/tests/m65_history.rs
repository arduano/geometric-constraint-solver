// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{LifecycleStatus, RetainedEditorCoordinator};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentCommandEffect, DocumentConstraintDefinition,
    DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit, DocumentSolveRequest,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the rejected-design edit, Undo/Redo restore, and accepted/candidate topology audits form one linear history contract"
)]
fn redo_of_structurally_distinct_rejected_design_preserves_each_topology() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let first = document.add_point("first", [0.0, 0.0]).expect("point");
    let second = document.add_point("second", [2.0, 0.0]).expect("point");
    let line = document
        .add_curve(
            "fixed line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    for (label, point, target) in [
        ("fix first", first, [0.0, 0.0]),
        ("fix second", second, [2.0, 0.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .expect("fixed point");
    }
    let conflicting_target = document
        .add_scalar(
            "driving distance",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("target");
    document
        .add_dimension(
            "accepted distance",
            DocumentDimensionDefinition::PointDistance {
                first,
                second,
                target: conflicting_target,
            },
            DocumentDimensionMode::Driving,
        )
        .expect("accepted driving dimension");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
    let baseline = session.accepted_state().expect("accepted state");
    let accepted_json = session
        .export_accepted_json()
        .expect("accepted JSON")
        .expect("accepted graph");
    let accepted_mappings = baseline.mappings().clone();
    let accepted_audit = baseline.solve_result().display_audit.clone();
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

    let conflicting = coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::SetScalarValue {
                scalar: conflicting_target,
                value: 3.0,
            },
        )
        .expect("retained contradictory target edit");
    assert!(conflicting.published_accepted.is_none());
    assert_eq!(
        coordinator.lifecycle().status,
        LifecycleStatus::RejectedAttempt
    );
    let structural = coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateConstraint {
                label: "candidate-only horizontal".into(),
                definition: DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(line),
                },
            },
        )
        .expect("structurally distinct rejected edit");
    assert!(structural.published_accepted.is_none());
    let rejected_design_json = coordinator
        .session()
        .export_design_json()
        .expect("rejected design JSON");
    let rejected_constraint = coordinator
        .session()
        .design_document()
        .constraints()
        .last()
        .expect("candidate-only constraint");
    let rejected_source = rejected_constraint.source_id;
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(conflicting_target)
            .expect("candidate target")
            .value
            .to_bits(),
        3.0f64.to_bits()
    );

    coordinator
        .undo()
        .expect("undo structurally distinct rejected design");
    coordinator
        .redo()
        .expect("redo structurally distinct rejected design");

    let restored = coordinator.session();
    assert_eq!(
        restored.export_design_json().expect("redone design JSON"),
        rejected_design_json,
        "Redo must retain the structurally distinct design graph"
    );
    assert_eq!(
        coordinator.lifecycle().status,
        LifecycleStatus::RejectedAttempt,
        "Redo must not publish an accepted solve of the older topology under the new design"
    );
    assert!(
        restored.last_attempt().accepted_state_identity().is_none(),
        "the rejected candidate must not gain a false accepted publication"
    );
    assert!(
        !restored
            .last_attempt()
            .solve_result()
            .expect("rejected solve evidence")
            .accepted()
    );
    assert_eq!(
        restored
            .design_document()
            .scalar(conflicting_target)
            .expect("restored contradictory target")
            .value
            .to_bits(),
        3.0f64.to_bits(),
        "accepted numerical seeding must not overwrite a candidate equation coefficient"
    );

    let attempted_mappings = restored
        .last_attempt()
        .mappings()
        .expect("candidate mappings");
    assert!(
        attempted_mappings
            .source_mappings()
            .iter()
            .any(|mapping| mapping.source_id == rejected_source),
        "attempt mappings must describe the candidate topology"
    );
    let attempted_diagnostics = restored.latest_attempt_diagnostics();
    assert!(
        attempted_diagnostics
            .sources
            .iter()
            .any(|source| source.source == rejected_source),
        "attempt audit must describe the candidate source"
    );

    let accepted = restored.accepted_state().expect("retained accepted state");
    assert_eq!(
        restored
            .export_accepted_json()
            .expect("accepted JSON")
            .expect("retained accepted graph"),
        accepted_json,
        "Redo must retain the exact prior accepted graph"
    );
    assert_ne!(
        accepted.design_identity(),
        restored.design_identity(),
        "the retained accepted state must not claim the rejected design identity"
    );
    assert_eq!(accepted.mappings(), &accepted_mappings);
    assert_eq!(accepted.solve_result().display_audit, accepted_audit);
    assert!(
        accepted
            .mappings()
            .runtime_source(rejected_source)
            .is_none(),
        "accepted mappings must not contain the rejected candidate source"
    );
    assert!(
        accepted
            .diagnostics()
            .sources
            .iter()
            .all(|source| source.source != rejected_source),
        "accepted audit must remain scoped to the accepted topology"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "candidate construction and the shared Undo/Redo/reload fallback assertions form one end-to-end restore lifecycle"
)]
fn invalid_topology_dependent_warm_merge_falls_back_for_undo_redo_and_reload() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let first = document.add_point("first", [0.0, 0.0]).expect("point");
    let second = document.add_point("second", [0.0, 0.0]).expect("point");
    for (label, point) in [("fix first", first), ("fix second", second)] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint {
                    point,
                    target: [0.0, 0.0],
                },
            )
            .expect("fixed point");
    }
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
    let baseline = session.accepted_state().expect("accepted state");
    assert_eq!(
        baseline
            .document()
            .point(first)
            .expect("first")
            .position
            .map(f64::to_bits),
        baseline
            .document()
            .point(second)
            .expect("second")
            .position
            .map(f64::to_bits),
        "accepted numerical state must collapse the future line endpoints"
    );
    let accepted_json = session
        .export_accepted_json()
        .expect("accepted JSON")
        .expect("accepted graph");
    let accepted_mappings = baseline.mappings().clone();
    let accepted_audit = baseline.solve_result().display_audit.clone();
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateConstraint {
                label: "conflicting first position".into(),
                definition: DocumentConstraintDefinition::FixedPoint {
                    point: first,
                    target: [1.0, 0.0],
                },
            },
        )
        .expect("retained conflict");
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point: second,
                position: [2.0, 0.0],
            },
        )
        .expect("retained candidate position");
    let created_line = coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateCurve {
                label: "candidate-only line".into(),
                definition: CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            },
        )
        .expect("retained candidate line");
    let line = match created_line.value {
        DocumentCommandEffect::CreatedCurve(line) => line,
        ref other => panic!("unexpected line effect: {other:?}"),
    };
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateConstraint {
                label: "candidate-only horizontal".into(),
                definition: DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(line),
                },
            },
        )
        .expect("retained candidate constraint");
    assert_eq!(
        coordinator.lifecycle().status,
        LifecycleStatus::RejectedAttempt
    );
    let rejected_design_json = coordinator
        .session()
        .export_design_json()
        .expect("candidate design JSON");
    let rejected_source = coordinator
        .session()
        .design_document()
        .constraints()
        .last()
        .expect("candidate constraint")
        .source_id;
    let saved = coordinator.checkpoint().clone();
    let assert_rejected_restore = |restored: &RetainedSketchDocumentSession| {
        assert_eq!(
            restored.export_design_json().expect("restored design JSON"),
            rejected_design_json
        );
        assert_eq!(
            restored
                .export_accepted_json()
                .expect("accepted JSON")
                .expect("accepted graph"),
            accepted_json
        );
        assert!(restored.last_attempt().accepted_state_identity().is_none());
        assert!(
            restored
                .last_attempt()
                .mappings()
                .expect("candidate mappings")
                .source_mappings()
                .iter()
                .any(|mapping| mapping.source_id == rejected_source)
        );
        assert!(
            restored
                .latest_attempt_diagnostics()
                .sources
                .iter()
                .any(|source| source.source == rejected_source)
        );
        let accepted = restored.accepted_state().expect("retained accepted");
        assert_eq!(accepted.mappings(), &accepted_mappings);
        assert_eq!(accepted.solve_result().display_audit, accepted_audit);
        assert!(
            accepted
                .diagnostics()
                .sources
                .iter()
                .all(|source| source.source != rejected_source)
        );
    };

    coordinator.undo().expect(
        "Undo must fall back from accepted positions that collapse the candidate-only line",
    );
    assert_eq!(
        coordinator.lifecycle().status,
        LifecycleStatus::RejectedAttempt
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON")
            .expect("accepted graph"),
        accepted_json
    );
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("retained accepted")
            .mappings(),
        &accepted_mappings
    );
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("retained accepted")
            .solve_result()
            .display_audit,
        accepted_audit
    );

    coordinator.redo().expect(
        "Redo must fall back from accepted positions that collapse the candidate-only line",
    );
    assert_rejected_restore(coordinator.session());

    coordinator
        .reload(&saved)
        .expect("cross-process-style checkpoint reload must use the same fallback");
    assert_rejected_restore(coordinator.session());
}
