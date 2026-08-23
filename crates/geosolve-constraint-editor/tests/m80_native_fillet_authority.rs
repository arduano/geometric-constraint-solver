// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentNativeLineFilletIds,
    DocumentSolveRequest, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};

fn assert_native_fillet_ids_exist(
    coordinator: &RetainedEditorCoordinator,
    ids: &DocumentNativeLineFilletIds,
) {
    let document = coordinator.session().design_document();
    assert!(
        ids.source_lines
            .iter()
            .all(|curve| document.curve(*curve).is_some())
    );
    assert!(document.point(ids.removed_corner).is_none());
    assert!(
        ids.contact_points
            .iter()
            .all(|point| document.point(*point).is_some())
    );
    assert!(document.curve(ids.arc).is_some());
    assert!(document.point(ids.center).is_some());
    assert!(
        [
            ids.radius,
            ids.start_angle,
            ids.end_angle,
            ids.contact_parameters[0],
            ids.contact_parameters[1],
            ids.radius_target,
        ]
        .into_iter()
        .all(|scalar| document.scalar(scalar).is_some())
    );
    assert!(
        ids.contacts
            .iter()
            .all(|contact| document.contact(*contact).is_some())
    );
    assert!(
        ids.tangencies
            .iter()
            .all(|constraint| document.constraint(*constraint).is_some())
    );
    assert!(document.dimension(ids.radius_dimension).is_some());
}

fn assert_current_native_solution(coordinator: &RetainedEditorCoordinator) {
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current independently accepted native result");
    let report = accepted.solve_result().unstable_core_report();
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
}

fn assert_feature_semantics_match(
    actual: &RetainedEditorCoordinator,
    expected: &RetainedEditorCoordinator,
) {
    let actual = actual.feature_document();
    let expected = expected.feature_document();
    assert_eq!(actual.id(), expected.id());
    assert_eq!(actual.sketch_document(), expected.sketch_document());
    assert_eq!(
        actual.allocator_high_water(),
        expected.allocator_high_water()
    );
    assert_eq!(actual.features(), expected.features());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one external-authority fixture keeps accepted-seed publication, exact replay, residual, ID and history evidence together"
)]
fn native_fillet_publication_uses_the_accepted_preview_not_opposed_retained_seeds() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let first_outer = document.add_point("first outer", [5.0, 0.0]).unwrap();
    let corner = document.add_point("sharp corner", [0.0, 0.0]).unwrap();
    let second_outer = document.add_point("second outer", [0.0, -5.0]).unwrap();
    let first_line = document
        .add_curve(
            "first line",
            CurveDefinition::Line {
                start: first_outer,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second_line = document
        .add_curve(
            "second line",
            CurveDefinition::Line {
                start: corner,
                end: second_outer,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    for (label, point, target) in [
        ("fix first outer", first_outer, [-3.0, 0.0]),
        ("fix second outer", second_outer, [0.0, 3.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    document
        .add_constraint(
            "horizontal first line",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(first_line),
            },
        )
        .unwrap();
    document
        .add_constraint(
            "vertical second line",
            DocumentConstraintDefinition::Vertical {
                line: CurveSpan::line(second_line),
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .expect("fixed accepted line-line corner");
    let replay_session = session.clone();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    let replay_features = coordinator.feature_document().clone();
    let snapshot = coordinator.feature_authoring_snapshot().unwrap();
    let accepted = snapshot.sketch_document().clone();
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(&snapshot, &accepted, FeatureAuthoringTool::Fillet, &[]),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    assert!(matches!(
        state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(1.0),
                ..FeatureAuthoringOptions::default()
            },
        ),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    let transaction = coordinator
        .transact_feature_authoring_pick_items(
            &mut state,
            &[(SelectionItem::Point(corner), None)],
            "accepted-seed native Fillet",
        )
        .unwrap();
    let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } = transaction.outcome else {
        panic!("accepted line-line corner must produce a preview");
    };
    let preview = transaction.preview.expect("exact held preview");
    coordinator
        .native_feature_authoring_availability(preview.token, &candidate)
        .expect("the complete held native patch must be available");

    let published = coordinator
        .apply_feature_authoring_native_profile(preview.token, &candidate)
        .expect("Apply must consume the accepted-scene native patch");
    assert_eq!(published.value.source_lines, [first_line, second_line]);
    assert_native_fillet_ids_exist(&coordinator, &published.value);
    assert_current_native_solution(&coordinator);

    let replay_action = coordinator
        .transcript()
        .last()
        .expect("native publication replay action")
        .clone();
    let committed_design = coordinator.session().export_design_json().unwrap();
    let committed_accepted = coordinator.session().export_accepted_json().unwrap();
    let committed_features = coordinator.feature_document().to_json().unwrap();
    let committed_feature_states = coordinator
        .computed_snapshot()
        .expect("native publication computed parity")
        .feature_evaluations()
        .to_vec();
    let committed_high_water = coordinator
        .session()
        .persistent_identity_high_water()
        .clone();

    let mut replayed =
        RetainedEditorCoordinator::with_features(replay_session, replay_features).unwrap();
    let mut base_with_committed_high_water = replayed.session().clone();
    base_with_committed_high_water
        .retain_persistent_identity_high_water(&committed_high_water)
        .expect("expected Undo high-water");
    let base_design = base_with_committed_high_water.export_design_json().unwrap();
    let base_accepted = base_with_committed_high_water
        .export_accepted_json()
        .unwrap();
    let base_history = (replayed.history_len(), replayed.history_cursor());
    replayed
        .replay(&replay_action)
        .expect("fresh coordinator replays native publication");

    assert_eq!(
        replayed.session().export_design_json().unwrap(),
        committed_design
    );
    assert_eq!(
        replayed.session().export_accepted_json().unwrap(),
        committed_accepted
    );
    assert_eq!(replayed.session().design_identity(), published.design);
    let replayed_accepted = replayed
        .session()
        .accepted_state_for_current_input()
        .expect("replayed current accepted state")
        .identity();
    assert_eq!(Some(replayed_accepted), published.published_accepted);
    assert_eq!(
        replayed.feature_document().to_json().unwrap(),
        committed_features
    );
    assert_eq!(
        replayed
            .computed_snapshot()
            .expect("replayed computed parity")
            .feature_evaluations(),
        committed_feature_states
    );
    assert_eq!(
        replayed.session().persistent_identity_high_water(),
        &committed_high_water
    );
    assert_eq!(
        (replayed.history_len(), replayed.history_cursor()),
        (base_history.0 + 1, base_history.1 + 1)
    );
    assert_eq!(replayed.transcript(), std::slice::from_ref(&replay_action));
    assert_native_fillet_ids_exist(&replayed, &published.value);
    assert_current_native_solution(&replayed);

    replayed.undo().expect("undo replayed native publication");
    assert_eq!(
        replayed.session().export_design_json().unwrap(),
        base_design
    );
    assert_eq!(
        replayed.session().export_accepted_json().unwrap(),
        base_accepted
    );
    assert_feature_semantics_match(&replayed, &coordinator);
    assert_eq!(replayed.history_cursor(), base_history.1);
    assert_eq!(
        replayed.session().persistent_identity_high_water(),
        &committed_high_water,
        "Undo must not make native publication identities reusable"
    );

    replayed.redo().expect("redo replayed native publication");
    assert_eq!(
        replayed.session().export_design_json().unwrap(),
        committed_design
    );
    assert_eq!(
        replayed.session().export_accepted_json().unwrap(),
        committed_accepted
    );
    assert_eq!(
        replayed.session().persistent_identity_high_water(),
        &committed_high_water
    );
    assert_feature_semantics_match(&replayed, &coordinator);
    assert_native_fillet_ids_exist(&replayed, &published.value);
    assert_current_native_solution(&replayed);
}
