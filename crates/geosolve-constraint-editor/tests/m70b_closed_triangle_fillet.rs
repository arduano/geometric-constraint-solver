// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    FeatureAuthoringCandidate, FeatureAuthoringOutcome, FeatureAuthoringStage,
    FeatureAuthoringState, FeatureAuthoringTool, FeatureAuthoringWarningKind,
    RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition, DocumentSolveRequest,
    RetainedSketchDocumentSession, SketchDocument, SketchHardValidity, SolverConfig,
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

fn characterize_point_path(mut fixture: ClosedTriangleFixture) {
    let feature_identity = fixture.coordinator.feature_document().identity();
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
    let before_closure = point_state.clone();
    let held_preview = fixture
        .coordinator
        .feature_authoring_preview()
        .expect("two valid corners remain previewed")
        .metadata()
        .clone();
    for closure_point in [fixture.points[0], fixture.points[3]] {
        let closure = fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut point_state,
                &[(SelectionItem::Point(closure_point), None)],
                "coincident closure point Fillet",
            )
            .expect("closure-point warning is a typed transaction outcome");
        assert!(matches!(
            closure.outcome,
            FeatureAuthoringOutcome::Warning(ref warning)
                if warning.kind == FeatureAuthoringWarningKind::WrongOperandKind
        ));
        assert!(closure.preview.is_none());
        assert_eq!(point_state, before_closure);
        assert_eq!(
            fixture
                .coordinator
                .feature_authoring_preview()
                .expect("last valid two-corner preview retained")
                .metadata(),
            &held_preview
        );
    }
    assert_eq!(
        fixture.coordinator.feature_document().identity(),
        feature_identity
    );
    assert!(fixture.coordinator.feature_document().features().is_empty());
}

fn characterize_curve_pair_path(mut fixture: ClosedTriangleFixture) {
    let feature_identity = fixture.coordinator.feature_document().identity();
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
            &[(SelectionItem::Curve(fixture.spans[2]), Some(0.75))],
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
    let before_second = curve_state.clone();
    let second = fixture
        .coordinator
        .transact_feature_authoring_pick_items(
            &mut curve_state,
            &[(SelectionItem::Curve(fixture.spans[0]), Some(0.25))],
            "closing triangle first span",
        )
        .expect("second closing-span warning is a typed transaction outcome");
    assert!(matches!(
        second.outcome,
        FeatureAuthoringOutcome::Warning(ref warning)
            if warning.kind == FeatureAuthoringWarningKind::DuplicateSupport
                && warning.message
                    == "same-curve Fillet parents must be adjacent spans of one open polyline"
    ));
    assert!(second.preview.is_none());
    assert_eq!(curve_state, before_second);
    assert!(fixture.coordinator.feature_authoring_preview().is_none());
    assert_eq!(
        fixture.coordinator.feature_document().identity(),
        feature_identity
    );
    assert!(fixture.coordinator.feature_document().features().is_empty());
}

#[test]
fn m70b_f003_coincident_triangle_closure_is_not_filletable_by_point_or_curve_pair() {
    // Open-finding characterization: when repair is authorized, convert this test to require one
    // three-corner preview/publication through both authoring paths. Until then it freezes the
    // exact typed rejection and transactional-retention signature without weakening the clean
    // workspace test gate.
    characterize_point_path(fixture());
    characterize_curve_pair_path(fixture());
}
