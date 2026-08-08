// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    CoordinatorError, FeatureAuthoringCandidate, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringPick, FeatureAuthoringStage, FeatureAuthoringState, FeatureAuthoringTool,
    FeatureAuthoringWarning, FeatureAuthoringWarningKind, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentEdit, DocumentSolveRequest,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_features::{ComputedFeatureAuthoringSnapshot, ComputedFeatureFailure};

struct AuthoringFixture {
    coordinator: RetainedEditorCoordinator,
    points: [DesignPointId; 4],
    spans: [CurveSpan; 3],
}

fn fixture() -> AuthoringFixture {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [
        document.add_point("p0", [0.0, 0.0]).expect("p0"),
        document.add_point("p1", [4.0, 0.0]).expect("p1"),
        document.add_point("p2", [4.0, 4.0]).expect("p2"),
        document.add_point("p3", [8.0, 4.0]).expect("p3"),
    ];
    let curve = document
        .add_curve(
            "three-span polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
            },
        )
        .expect("polyline");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
    AuthoringFixture {
        coordinator: RetainedEditorCoordinator::new(session).expect("coordinator"),
        points,
        spans: [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
    }
}

fn activate(
    fixture: &AuthoringFixture,
) -> (FeatureAuthoringState, ComputedFeatureAuthoringSnapshot) {
    let snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("authoring snapshot");
    let document = snapshot.sketch_document();
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(&snapshot, document, FeatureAuthoringTool::Fillet, &[]),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    (state, snapshot)
}

fn pick(fixture: &AuthoringFixture, span: usize, parameter: f64) -> FeatureAuthoringPick {
    let mut picks = fixture
        .coordinator
        .feature_authoring_picks_for_item(
            SelectionItem::Curve(fixture.spans[span]),
            Some(parameter),
        )
        .expect("exact curve pick");
    assert_eq!(picks.len(), 1);
    picks.pop().expect("one exact curve pick")
}

fn candidate(outcome: FeatureAuthoringOutcome) -> FeatureAuthoringCandidate {
    match outcome {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
        | FeatureAuthoringOutcome::Apply(candidate) => candidate,
        other => panic!("expected complete Fillet candidate, got {other:?}"),
    }
}

fn warning(outcome: FeatureAuthoringOutcome) -> FeatureAuthoringWarning {
    match outcome {
        FeatureAuthoringOutcome::Warning(warning) => warning,
        other => panic!("expected authoring warning, got {other:?}"),
    }
}

fn complete_first_corner(
    fixture: &AuthoringFixture,
    state: &mut FeatureAuthoringState,
    snapshot: &ComputedFeatureAuthoringSnapshot,
) -> FeatureAuthoringCandidate {
    assert!(matches!(
        state.pick_many(snapshot, [pick(fixture, 0, 0.75)]),
        FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
    ));
    candidate(state.pick_many(snapshot, [pick(fixture, 1, 0.25)]))
}

#[test]
fn explicit_and_unspecified_radius_options_resolve_to_one_positive_candidate_value() {
    let fixture = fixture();
    let cases: [(Option<f64>, f64); 2] = [(None, 1.0), (Some(0.625), 0.625)];

    for (requested, expected) in cases {
        let (mut state, snapshot) = activate(&fixture);
        let outcome = state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: requested,
                ..FeatureAuthoringOptions::default()
            },
        );
        assert!(matches!(
            outcome,
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        assert_eq!(
            state.options().fillet_radius.map(f64::to_bits),
            Some(expected.to_bits()),
            "an unspecified option must preserve the initialized model-scale default"
        );

        let resolved = complete_first_corner(&fixture, &mut state, &snapshot);
        assert_eq!(resolved.radius().to_bits(), expected.to_bits());
    }
}

#[test]
fn every_invalid_radius_update_is_state_neutral_at_preview_ready() {
    let fixture = fixture();
    let (mut ready, snapshot) = activate(&fixture);
    let original = complete_first_corner(&fixture, &mut ready, &snapshot);

    for radius in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let before = ready.clone();
        let outcome = ready.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(radius),
                ..ready.options()
            },
        );
        assert!(matches!(
            outcome,
            FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
                kind: FeatureAuthoringWarningKind::InvalidRadius,
                ..
            })
        ));
        assert_eq!(ready, before, "invalid radius {radius:?} changed state");
        assert_eq!(candidate(ready.apply()), original);
    }
}

#[test]
fn globally_rejected_option_and_radius_refreshes_preserve_the_exact_valid_batch() {
    let mut fixture = fixture();
    let (mut state, snapshot) = activate(&fixture);
    let initial = fixture
        .coordinator
        .transact_feature_authoring_pick_items(
            &mut state,
            &[
                (SelectionItem::Point(fixture.points[1]), None),
                (SelectionItem::Point(fixture.points[2]), None),
            ],
            "two adjacent corners",
        )
        .expect("valid shared-radius preview");
    let initial_candidate = candidate(initial.outcome);
    let initial_metadata = initial.preview.expect("held exact preview");
    let initial_state = state.clone();

    let mut refresh_trial = state.clone();
    let oversized_candidate = candidate(refresh_trial.set_options(
        &snapshot,
        FeatureAuthoringOptions {
            fillet_radius: Some(3.0),
            ..refresh_trial.options()
        },
    ));
    let refresh_error = fixture
        .coordinator
        .refresh_feature_authoring_preview(initial_metadata.input, &oversized_candidate)
        .expect_err("crossed source intervals must reject a radius refresh");
    assert!(matches!(
        refresh_error,
        CoordinatorError::FeatureAuthoringPreviewRejected(
            ComputedFeatureFailure::ConsumedSourceInterval { .. }
                | ComputedFeatureFailure::EndpointClaimConflict { .. }
        )
    ));
    assert_eq!(state, initial_state);
    assert_eq!(
        fixture
            .coordinator
            .feature_authoring_preview()
            .expect("valid preview retained after refresh rejection")
            .metadata(),
        &initial_metadata
    );

    let oversized_options = FeatureAuthoringOptions {
        fillet_radius: Some(3.0),
        ..state.options()
    };
    let option_error = fixture
        .coordinator
        .transact_feature_authoring_options(
            &mut state,
            oversized_options,
            None,
            "rejected oversized corners",
        )
        .expect_err("crossed source intervals must reject an option transaction");
    assert!(matches!(
        option_error,
        CoordinatorError::FeatureAuthoringPreviewRejected(
            ComputedFeatureFailure::ConsumedSourceInterval { .. }
                | ComputedFeatureFailure::EndpointClaimConflict { .. }
        )
    ));
    assert_eq!(state, initial_state);
    assert_eq!(candidate(state.apply()), initial_candidate);
    assert_eq!(
        fixture
            .coordinator
            .feature_authoring_preview()
            .expect("valid preview retained after option rejection")
            .metadata(),
        &initial_metadata
    );

    let resized_options = FeatureAuthoringOptions {
        fillet_radius: Some(0.75),
        ..state.options()
    };
    let retry = fixture
        .coordinator
        .transact_feature_authoring_options(
            &mut state,
            resized_options,
            None,
            "resized adjacent corners",
        )
        .expect("smaller shared-radius retry");
    let resized = candidate(retry.outcome);
    let resized_metadata = retry.preview.expect("resized exact preview");
    assert_eq!(resized.radius().to_bits(), 0.75_f64.to_bits());
    assert_eq!(
        state.options().fillet_radius.map(f64::to_bits),
        Some(0.75_f64.to_bits())
    );
    assert_ne!(resized_metadata.token, initial_metadata.token);
    fixture
        .coordinator
        .apply_feature_authoring_preview(resized_metadata.token, &resized)
        .expect("publish exact resized retry");
}

#[test]
fn duplicate_second_support_retains_the_first_pick_and_a_distinct_retry_completes() {
    let fixture = fixture();
    let (mut state, snapshot) = activate(&fixture);
    let first = pick(&fixture, 0, 0.75);
    assert!(matches!(
        state.pick_many(&snapshot, [first.clone()]),
        FeatureAuthoringOutcome::Collecting { ref pending, .. }
            if pending == std::slice::from_ref(&first)
    ));
    let before_duplicate = state.clone();

    let duplicate = warning(state.pick_many(&snapshot, [pick(&fixture, 0, 0.25)]));
    assert_eq!(
        duplicate.kind,
        FeatureAuthoringWarningKind::DuplicateSupport
    );
    assert_eq!(state, before_duplicate);

    let resolved = candidate(state.pick_many(&snapshot, [pick(&fixture, 1, 0.25)]));
    assert_eq!(resolved.corners().len(), 1);
    let sources = [
        resolved.corners()[0].corner.first.source.span,
        resolved.corners()[0].corner.second.source.span,
    ];
    assert!(sources.contains(&fixture.spans[0]));
    assert!(sources.contains(&fixture.spans[1]));
}

#[test]
fn oversized_semantic_preselection_stops_before_incidence_resolution_and_is_state_neutral() {
    let fixture = fixture();
    let (mut state, snapshot) = activate(&fixture);
    assert!(matches!(
        state.pick_many(&snapshot, [pick(&fixture, 0, 0.75)]),
        FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
    ));
    let before = state.clone();
    let oversized = vec![(SelectionItem::Point(fixture.points[1]), None); 16_385];

    let outcome = state.pick_items(&snapshot, snapshot.sketch_document(), &oversized);

    assert!(matches!(
        outcome,
        FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
            kind: FeatureAuthoringWarningKind::WorkStopped,
            ..
        })
    ));
    assert_eq!(state, before);
}

#[derive(Clone, Copy, Debug)]
enum IncompleteStage {
    Empty,
    Pending,
}

fn incomplete_state(
    fixture: &AuthoringFixture,
    stage: IncompleteStage,
) -> (FeatureAuthoringState, ComputedFeatureAuthoringSnapshot) {
    let (mut authoring, snapshot) = activate(fixture);
    if matches!(stage, IncompleteStage::Pending) {
        assert!(matches!(
            authoring.pick_many(&snapshot, [pick(fixture, 0, 0.75)]),
            FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
        ));
    }
    (authoring, snapshot)
}

#[test]
fn incomplete_apply_is_state_neutral_at_empty_and_pending_stages() {
    let fixture = fixture();
    for stage in [IncompleteStage::Empty, IncompleteStage::Pending] {
        let (state, _snapshot) = incomplete_state(&fixture, stage);
        let before = state.clone();
        let warning = warning(state.apply());
        assert_eq!(warning.kind, FeatureAuthoringWarningKind::IncompleteCorner);
        assert_eq!(warning.stage, state.guidance().stage);
        assert_eq!(state, before, "Apply changed incomplete {stage:?} state");
    }
}

#[derive(Clone, Copy, Debug)]
enum EscapeStage {
    Empty,
    Pending,
    Ready,
}

#[test]
fn escape_exits_empty_immediately_and_clears_nonempty_before_exiting() {
    let fixture = fixture();
    for stage in [EscapeStage::Empty, EscapeStage::Pending, EscapeStage::Ready] {
        let (mut state, snapshot) = activate(&fixture);
        match stage {
            EscapeStage::Empty => {}
            EscapeStage::Pending => {
                let _ = state.pick_many(&snapshot, [pick(&fixture, 0, 0.75)]);
            }
            EscapeStage::Ready => {
                let _ = complete_first_corner(&fixture, &mut state, &snapshot);
            }
        }

        if matches!(stage, EscapeStage::Empty) {
            assert_eq!(state.cancel(), FeatureAuthoringOutcome::ModeExited);
            assert_eq!(state.active_tool(), None);
            continue;
        }

        assert!(matches!(
            state.cancel(),
            FeatureAuthoringOutcome::CandidateCleared(ref guidance)
                if guidance.stage == FeatureAuthoringStage::PickFirstFilletCurve
                    && guidance.completed_corners == 0
        ));
        assert_eq!(state.active_tool(), Some(FeatureAuthoringTool::Fillet));
        assert_eq!(state.completed_corner_count(), 0);
        assert_eq!(state.cancel(), FeatureAuthoringOutcome::ModeExited);
        assert_eq!(state.active_tool(), None);
    }
}

#[test]
fn stale_pick_is_rejected_without_consumption_and_current_picks_still_work() {
    let mut fixture = fixture();
    let stale_pick = pick(&fixture, 0, 0.75);
    fixture
        .coordinator
        .apply_edit(
            fixture.coordinator.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point: fixture.points[0],
                position: [-1.0, 0.0],
            },
        )
        .expect("accepted source edit");
    let (mut authoring, current_snapshot) = activate(&fixture);
    let before_stale = authoring.clone();

    let rejected = warning(authoring.pick_many(&current_snapshot, [stale_pick]));
    assert_eq!(rejected.kind, FeatureAuthoringWarningKind::StalePick);
    assert_eq!(authoring, before_stale);
    assert!(matches!(
        authoring.pick_many(&current_snapshot, [pick(&fixture, 0, 0.75)]),
        FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
    ));
    assert_eq!(
        candidate(authoring.pick_many(&current_snapshot, [pick(&fixture, 1, 0.25)]))
            .corners()
            .len(),
        1
    );
}

#[test]
fn completed_corner_and_next_pending_pick_are_both_retained_until_the_pair_finishes() {
    let fixture = fixture();
    let (mut state, snapshot) = activate(&fixture);
    let first = complete_first_corner(&fixture, &mut state, &snapshot);
    assert_eq!(first.corners().len(), 1);

    let next_first = pick(&fixture, 1, 0.75);
    assert!(matches!(
        state.pick_many(&snapshot, [next_first.clone()]),
        FeatureAuthoringOutcome::Collecting {
            ref pending,
            ref guidance,
        } if pending == std::slice::from_ref(&next_first)
            && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
            && guidance.completed_corners == 1
    ));
    assert_eq!(state.completed_corner_count(), 1);
    let before_incomplete_apply = state.clone();
    assert_eq!(
        warning(state.apply()).kind,
        FeatureAuthoringWarningKind::IncompleteCorner
    );
    assert_eq!(state, before_incomplete_apply);

    let grouped = candidate(state.pick_many(&snapshot, [pick(&fixture, 2, 0.25)]));
    assert_eq!(grouped.corners().len(), 2);
    assert_eq!(state.completed_corner_count(), 2);
    assert_eq!(state.guidance().stage, FeatureAuthoringStage::PreviewReady);
}

#[test]
fn pending_curve_and_its_incident_point_complete_exactly_one_corner() {
    let fixture = fixture();
    let (mut authoring, snapshot) = activate(&fixture);
    let pending = pick(&fixture, 0, 0.75);
    assert!(matches!(
        authoring.pick_many(&snapshot, [pending]),
        FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
    ));
    let document = snapshot.sketch_document();

    let resolved = candidate(authoring.pick_items(
        &snapshot,
        document,
        &[(SelectionItem::Point(fixture.points[1]), None)],
    ));

    assert_eq!(resolved.corners().len(), 1);
    assert_eq!(authoring.completed_corner_count(), 1);
    assert_eq!(
        authoring.guidance().stage,
        FeatureAuthoringStage::PreviewReady
    );
    let parents = [
        &resolved.corners()[0].corner.first,
        &resolved.corners()[0].corner.second,
    ];
    assert!(parents.iter().any(|parent| {
        parent.source.span == fixture.spans[0]
            && parent.picked_parameter.to_bits() == 0.75_f64.to_bits()
    }));
    assert!(parents.iter().any(|parent| {
        parent.source.span == fixture.spans[1]
            && parent.picked_parameter.to_bits() == 0.25_f64.to_bits()
    }));
    assert_eq!(candidate(authoring.apply()), resolved);
}

#[test]
fn unrelated_atomic_corner_warns_without_consuming_the_pending_curve() {
    let fixture = fixture();
    let (mut authoring, snapshot) = activate(&fixture);
    let pending = pick(&fixture, 0, 0.75);
    let _ = authoring.pick_many(&snapshot, [pending]);
    let before_unrelated = authoring.clone();
    let document = snapshot.sketch_document();

    let rejected = warning(authoring.pick_items(
        &snapshot,
        document,
        &[(SelectionItem::Point(fixture.points[2]), None)],
    ));

    assert_eq!(
        rejected.kind,
        FeatureAuthoringWarningKind::AmbiguousTrimSide
    );
    assert_eq!(authoring, before_unrelated);
    assert_eq!(
        candidate(authoring.pick_items(
            &snapshot,
            document,
            &[(SelectionItem::Point(fixture.points[1]), None)],
        ))
        .corners()
        .len(),
        1,
        "the original pending curve must remain available for a valid incident-corner retry"
    );
}

#[test]
fn completed_old_revision_rejects_fresh_picks_and_option_edits_without_mixing() {
    let mut fixture = fixture();
    let (mut authoring, old_snapshot) = activate(&fixture);
    let old_candidate = complete_first_corner(&fixture, &mut authoring, &old_snapshot);
    let before_source_edit = authoring.clone();
    fixture
        .coordinator
        .apply_edit(
            fixture.coordinator.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point: fixture.points[3],
                position: [9.0, 4.0],
            },
        )
        .expect("accepted unrelated source edit");
    let fresh_snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("fresh authoring snapshot");
    assert_ne!(old_snapshot.sketch_input(), fresh_snapshot.sketch_input());
    assert_ne!(
        old_snapshot.accepted_state_identity(),
        fresh_snapshot.accepted_state_identity()
    );

    let fresh_pick = pick(&fixture, 2, 0.25);
    let rejected_pick = warning(authoring.pick_many(&fresh_snapshot, [fresh_pick]));
    assert_eq!(rejected_pick.kind, FeatureAuthoringWarningKind::StalePick);
    assert_eq!(authoring, before_source_edit);

    let rejected_options = warning(authoring.set_options(
        &fresh_snapshot,
        FeatureAuthoringOptions {
            fillet_radius: Some(0.5),
            ..authoring.options()
        },
    ));
    assert_eq!(
        rejected_options.kind,
        FeatureAuthoringWarningKind::StalePick
    );
    assert_eq!(authoring, before_source_edit);
    assert_eq!(candidate(authoring.apply()), old_candidate);
}

#[test]
fn activation_preserves_atomic_corner_meaning_across_preselection_shapes() {
    let fixture = fixture();
    let snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("authoring snapshot");
    let document = snapshot.sketch_document();

    let cases = [
        vec![(SelectionItem::Point(fixture.points[1]), None)],
        vec![
            (SelectionItem::Curve(fixture.spans[0]), Some(0.75)),
            (SelectionItem::Curve(fixture.spans[1]), Some(0.25)),
        ],
        vec![
            (SelectionItem::Curve(fixture.spans[0]), Some(0.75)),
            (SelectionItem::Point(fixture.points[1]), None),
        ],
    ];
    for selection in cases {
        let mut authoring = FeatureAuthoringState::default();
        let resolved = candidate(authoring.activate(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            &selection,
        ));
        assert_eq!(resolved.corners().len(), 1);
        assert_eq!(authoring.completed_corner_count(), 1);
        assert_eq!(
            authoring.guidance().stage,
            FeatureAuthoringStage::PreviewReady
        );
        assert!(matches!(
            authoring.apply(),
            FeatureAuthoringOutcome::Apply(ref applied) if applied == &resolved
        ));
    }

    let mut unrelated = FeatureAuthoringState::default();
    let rejected = warning(unrelated.activate(
        &snapshot,
        document,
        FeatureAuthoringTool::Fillet,
        &[
            (SelectionItem::Curve(fixture.spans[0]), Some(0.75)),
            (SelectionItem::Point(fixture.points[2]), None),
        ],
    ));
    assert_eq!(
        rejected.kind,
        FeatureAuthoringWarningKind::AmbiguousTrimSide
    );
    assert_eq!(unrelated.completed_corner_count(), 0);
    assert_eq!(
        unrelated.guidance().stage,
        FeatureAuthoringStage::PickFirstFilletCurve,
        "a rejected compound preselection must not leave its first curve pending"
    );
}

#[test]
fn failed_second_operand_preserves_pending_and_completed_work_for_valid_retry() {
    let fixture = fixture();
    let (mut authoring, snapshot) = activate(&fixture);
    let first_pick = pick(&fixture, 0, 0.75);
    assert!(matches!(
        authoring.pick_many(&snapshot, [first_pick.clone()]),
        FeatureAuthoringOutcome::Collecting { ref pending, .. }
            if pending == std::slice::from_ref(&first_pick)
    ));
    let before_invalid = authoring.clone();

    let rejected = warning(authoring.pick_many(&snapshot, [pick(&fixture, 2, 0.25)]));
    assert_eq!(rejected.kind, FeatureAuthoringWarningKind::DuplicateSupport);
    assert_eq!(authoring, before_invalid);
    assert_eq!(
        candidate(authoring.pick_many(&snapshot, [pick(&fixture, 1, 0.25)]))
            .corners()
            .len(),
        1
    );

    let next_pending = pick(&fixture, 1, 0.75);
    assert!(matches!(
        authoring.pick_many(&snapshot, [next_pending]),
        FeatureAuthoringOutcome::Collecting { ref guidance, .. }
            if guidance.completed_corners == 1
                && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
    ));
    let before_unrelated = authoring.clone();
    let document = snapshot.sketch_document();
    let rejected = warning(authoring.pick_items(
        &snapshot,
        document,
        &[(SelectionItem::Point(fixture.points[0]), None)],
    ));
    assert_eq!(rejected.kind, FeatureAuthoringWarningKind::WrongOperandKind);
    assert_eq!(authoring, before_unrelated);
    let recovered = candidate(authoring.pick_items(
        &snapshot,
        document,
        &[(SelectionItem::Point(fixture.points[2]), None)],
    ));
    assert_eq!(recovered.corners().len(), 2);
}

#[test]
fn undo_redo_and_reattempt_revoke_preview_and_reject_the_old_authoring_batch() {
    let mut fixture = fixture();
    fixture
        .coordinator
        .apply_edit(
            fixture.coordinator.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point: fixture.points[3],
                position: [9.0, 4.0],
            },
        )
        .expect("checkpointed source edit");
    let (mut authoring, old_snapshot) = activate(&fixture);
    let old_candidate = complete_first_corner(&fixture, &mut authoring, &old_snapshot);
    fixture
        .coordinator
        .prepare_feature_authoring_preview(
            fixture.coordinator.feature_document().identity(),
            &old_candidate,
            "temporary old-revision Fillet",
        )
        .expect("held authoring preview");
    assert!(fixture.coordinator.feature_authoring_preview().is_some());

    fixture.coordinator.undo().expect("undo source edit");
    assert!(fixture.coordinator.feature_authoring_preview().is_none());
    let undo_snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("post-undo snapshot");
    let before_stale_retry = authoring.clone();
    assert_eq!(
        warning(authoring.pick_many(&undo_snapshot, [pick(&fixture, 2, 0.25)])).kind,
        FeatureAuthoringWarningKind::StalePick
    );
    assert_eq!(authoring, before_stale_retry);
    assert!(matches!(
        fixture.coordinator.prepare_feature_authoring_preview(
            fixture.coordinator.feature_document().identity(),
            &old_candidate,
            "stale candidate",
        ),
        Err(CoordinatorError::StaleComputedFeatureCandidate)
    ));

    fixture.coordinator.redo().expect("redo source edit");
    let redo_snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("post-redo snapshot");
    assert_eq!(
        warning(authoring.set_options(
            &redo_snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.5),
                ..authoring.options()
            },
        ))
        .kind,
        FeatureAuthoringWarningKind::StalePick
    );
    assert_eq!(authoring, before_stale_retry);

    fixture
        .coordinator
        .reattempt(fixture.coordinator.session().design_identity())
        .expect("reattempt current design");
    assert!(fixture.coordinator.feature_authoring_preview().is_none());
    let reattempt_snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("post-reattempt snapshot");
    assert_eq!(
        warning(authoring.pick_many(&reattempt_snapshot, [pick(&fixture, 2, 0.25)])).kind,
        FeatureAuthoringWarningKind::StalePick
    );
    assert_eq!(authoring, before_stale_retry);
}
