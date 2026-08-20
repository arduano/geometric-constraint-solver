// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    FeatureAuthoringCandidate, FeatureAuthoringOutcome, FeatureAuthoringStage,
    FeatureAuthoringState, FeatureAuthoringTool, FeatureAuthoringTransaction,
    RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition, DocumentSolveRequest,
    RetainedSketchDocumentSession, SketchDocument, SketchHardValidity, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedEdgeGeometry, ComputedFeatureDefinition, ComputedFeatureEvaluationState,
};

struct ClosedTriangleFixture {
    coordinator: RetainedEditorCoordinator,
    points: [DesignPointId; 4],
    spans: [CurveSpan; 3],
}

fn fixture() -> ClosedTriangleFixture {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [
        document
            .add_point("first triangle point", [0.0, 0.0])
            .expect("first point"),
        document
            .add_point("second triangle point", [6.0, 0.0])
            .expect("second point"),
        document
            .add_point("third triangle point", [3.0, 5.0])
            .expect("third point"),
        document
            .add_point("last coincident triangle point", [0.25, -0.15])
            .expect("last point"),
    ];
    let curve = document
        .add_curve(
            "open triangle polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![
                    [1.0, 0.0],
                    [-0.514_495_755_427_526_5, 0.857_492_925_712_544_1],
                    [-0.514_495_755_427_526_5, -0.857_492_925_712_544_1],
                ],
            },
        )
        .expect("open triangle polyline");
    document
        .add_constraint(
            "close triangle endpoints",
            DocumentConstraintDefinition::Coincident {
                first: points[0],
                second: points[3],
            },
        )
        .expect("coincident closure");

    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted coincident triangle");
    let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted triangle");
    let accepted_document = accepted.document();
    assert!(points.iter().all(|point| {
        accepted_document
            .point(*point)
            .expect("accepted triangle point")
            .position
            .into_iter()
            .all(f64::is_finite)
    }));
    assert!(accepted_document.constraints().iter().any(|constraint| {
        matches!(
            constraint.definition,
            DocumentConstraintDefinition::Coincident { first, second }
                if first == points[0] && second == points[3]
        )
    }));
    let first = accepted_document
        .point(points[0])
        .expect("accepted first point")
        .position;
    let last = accepted_document
        .point(points[3])
        .expect("accepted last point")
        .position;
    assert!((first[0] - last[0]).hypot(first[1] - last[1]) <= 1.0e-12);
    let solve = accepted.diagnostics().solve.expect("solve diagnostics");
    assert_eq!(solve.hard_validity, SketchHardValidity::Valid);
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual <= 1.0e-9)
    );

    ClosedTriangleFixture {
        coordinator,
        points,
        spans: [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
    }
}

fn activate(fixture: &ClosedTriangleFixture) -> FeatureAuthoringState {
    let snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("feature-authoring snapshot");
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(
            &snapshot,
            snapshot.sketch_document(),
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    state
}

fn candidate(outcome: &FeatureAuthoringOutcome) -> &FeatureAuthoringCandidate {
    match outcome {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
        other => panic!("expected a complete Fillet candidate, got {other:?}"),
    }
}

fn publish_three_corner_feature(
    fixture: &mut ClosedTriangleFixture,
    transaction: FeatureAuthoringTransaction,
) {
    let candidate = candidate(&transaction.outcome).clone();
    assert_eq!(candidate.corners().len(), 3);
    let actual_pairs = candidate
        .corners()
        .iter()
        .map(|corner| {
            std::collections::BTreeSet::from([
                corner.corner.first.source.span,
                corner.corner.second.source.span,
            ])
        })
        .collect::<std::collections::BTreeSet<_>>();
    let expected_pairs = std::collections::BTreeSet::from([
        std::collections::BTreeSet::from([fixture.spans[0], fixture.spans[1]]),
        std::collections::BTreeSet::from([fixture.spans[1], fixture.spans[2]]),
        std::collections::BTreeSet::from([fixture.spans[2], fixture.spans[0]]),
    ]);
    assert_eq!(actual_pairs, expected_pairs);
    let preview = transaction
        .preview
        .expect("complete three-corner preview metadata");
    let mutation = fixture
        .coordinator
        .apply_feature_authoring_preview(preview.token, &candidate)
        .expect("three-corner closure publication");
    let feature = fixture
        .coordinator
        .feature_document()
        .feature(mutation.value)
        .expect("published closure feature");
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition else {
        panic!("expected FilletSet feature");
    };
    assert_eq!(fillet.corners.len(), 3);
    let current = fixture
        .coordinator
        .computed_snapshot()
        .expect("current closure snapshot");
    assert!(matches!(
        current
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == feature.id)
            .expect("closure feature evaluation")
            .state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    assert_eq!(
        current
            .edges()
            .iter()
            .filter(|edge| matches!(edge.geometry, ComputedEdgeGeometry::CircularArc(_)))
            .count(),
        3
    );
}

fn verify_point_path(mut fixture: ClosedTriangleFixture, closure_point: DesignPointId) {
    let mut point_state = activate(&fixture);

    for (expected_corners, point) in [(1, fixture.points[1]), (2, fixture.points[2])] {
        let transaction = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut point_state,
                &[(SelectionItem::Point(point), None)],
                "closed triangle point Fillets",
            )
            .expect("ordinary interior triangle corner transaction");
        assert_eq!(
            candidate(&transaction.outcome).corners().len(),
            expected_corners
        );
        assert!(transaction.preview.is_some());
    }
    let closure = fixture
        .coordinator
        .transact_feature_authoring_pick_items(
            &mut point_state,
            &[(SelectionItem::Point(closure_point), None)],
            "coincident closure point Fillet",
        )
        .expect("closure point transaction");
    publish_three_corner_feature(&mut fixture, closure);
}

fn verify_curve_pair_path(mut fixture: ClosedTriangleFixture, order: [usize; 2]) {
    let mut curve_state = activate(&fixture);

    for (expected_corners, point) in [(1, fixture.points[1]), (2, fixture.points[2])] {
        let transaction = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut curve_state,
                &[(SelectionItem::Point(point), None)],
                "closed triangle point Fillets before curve-pair closure",
            )
            .expect("ordinary interior triangle corner transaction");
        assert_eq!(
            candidate(&transaction.outcome).corners().len(),
            expected_corners
        );
        assert!(transaction.preview.is_some());
    }
    let first = fixture
        .coordinator
        .transact_feature_authoring_pick_items(
            &mut curve_state,
            &[(
                (SelectionItem::Curve(fixture.spans[order[0]])),
                Some(if order[0] == 2 { 0.75 } else { 0.25 }),
            )],
            "closing triangle last span",
        )
        .expect("first closing-span pick");
    assert!(matches!(
        first.outcome,
        FeatureAuthoringOutcome::Collecting {
            ref pending,
            ref guidance,
        } if pending.len() == 1
            && guidance.completed_corners == 2
            && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
    ));
    assert!(fixture.coordinator.feature_authoring_preview().is_none());
    let second = fixture
        .coordinator
        .transact_feature_authoring_pick_items(
            &mut curve_state,
            &[(
                (SelectionItem::Curve(fixture.spans[order[1]])),
                Some(if order[1] == 2 { 0.75 } else { 0.25 }),
            )],
            "closing triangle first span",
        )
        .expect("second closing-span transaction");
    publish_three_corner_feature(&mut fixture, second);
}

#[test]
fn m70b_f003_coincident_triangle_closure_is_filletable_by_point_or_curve_pair() {
    for closure_point in [0, 3] {
        let fixture = fixture();
        let point = fixture.points[closure_point];
        verify_point_path(fixture, point);
    }
    verify_curve_pair_path(fixture(), [2, 0]);
    verify_curve_pair_path(fixture(), [0, 2]);
}
