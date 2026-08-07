// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    EditorScene, FeatureAuthoringCandidate, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringPick, FeatureAuthoringStage, FeatureAuthoringState, FeatureAuthoringTool,
    FeatureAuthoringWarningKind, PickTolerance, RetainedEditorCoordinator, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentFilletTrimEndpoint, DocumentSolveRequest,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedFeatureAuthoringSnapshot, ComputedFeatureDefinition,
    ComputedFeatureEvaluationState,
};

const DEFAULT_RADIUS: f64 = 1.0;

struct TwoLineInteractionFixture {
    coordinator: RetainedEditorCoordinator,
    scene: EditorScene,
    spans: [CurveSpan; 2],
    pick_positions: [[f64; 2]; 2],
}

fn coordinator_and_scene(document: SketchDocument) -> (RetainedEditorCoordinator, EditorScene) {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
    let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted state");
    let scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        Viewport::new([800.0, 600.0], [2.0, 2.0], 50.0).expect("viewport"),
        0.5,
    )
    .expect("accepted scene");
    (coordinator, scene)
}

fn two_line_interaction_fixture() -> TwoLineInteractionFixture {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let corner = document.add_point("corner", [4.0, 0.0]).expect("corner");
    let end = document.add_point("end", [4.0, 4.0]).expect("end");
    let first = document
        .add_curve(
            "first ordinary line",
            CurveDefinition::Line {
                start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    let second = document
        .add_curve(
            "second ordinary line",
            CurveDefinition::Line {
                start: corner,
                end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second line");
    let spans = [CurveSpan::line(first), CurveSpan::line(second)];
    let (coordinator, scene) = coordinator_and_scene(document);
    TwoLineInteractionFixture {
        coordinator,
        scene,
        spans,
        pick_positions: [[3.0, 0.0], [4.0, 1.0]],
    }
}

fn activate_authoring(
    coordinator: &RetainedEditorCoordinator,
) -> (
    FeatureAuthoringState,
    ComputedFeatureAuthoringSnapshot,
    SketchDocument,
) {
    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("feature-authoring snapshot");
    let document = coordinator
        .operation_authoring_document()
        .expect("current accepted authoring document")
        .clone();
    let mut authoring = FeatureAuthoringState::default();
    assert!(matches!(
        authoring.activate(&snapshot, &document, FeatureAuthoringTool::Fillet, &[]),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    (authoring, snapshot, document)
}

fn pick_at(
    authoring: &mut FeatureAuthoringState,
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    scene: &EditorScene,
    model_position: [f64; 2],
) -> FeatureAuthoringOutcome {
    authoring.pick_at(
        snapshot,
        document,
        scene,
        scene.viewport.model_to_screen(model_position),
        PickTolerance::default(),
    )
}

fn picks_at(
    fixture: &TwoLineInteractionFixture,
    model_position: [f64; 2],
) -> Vec<FeatureAuthoringPick> {
    let hit = fixture
        .scene
        .native_authoring_hit_test(
            fixture.scene.viewport.model_to_screen(model_position),
            PickTolerance::default(),
        )
        .expect("native mid-span hit");
    assert!(matches!(hit.item, SelectionItem::Curve(_)));
    fixture
        .coordinator
        .feature_authoring_picks_for_item(hit.item, hit.curve_parameter)
        .expect("coordinator-stamped Fillet pick")
}

fn expect_candidate_after_second_pick(
    outcome: FeatureAuthoringOutcome,
) -> FeatureAuthoringCandidate {
    match outcome {
        FeatureAuthoringOutcome::PreviewRequested {
            candidate,
            guidance,
        } => {
            assert_eq!(guidance.stage, FeatureAuthoringStage::PreviewReady);
            assert_eq!(guidance.completed_corners, 1);
            candidate
        }
        FeatureAuthoringOutcome::Warning(warning) => {
            panic!("the distinct second ordinary line was rejected: {warning:?}")
        }
        other => panic!("the second ordinary line did not complete a Fillet corner: {other:?}"),
    }
}

fn exercise_blank_radius_two_line_chain(order: [usize; 2]) {
    let mut fixture = two_line_interaction_fixture();
    let snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("feature-authoring snapshot");
    let accepted_document = fixture
        .coordinator
        .operation_authoring_document()
        .expect("current accepted authoring document")
        .clone();
    let mut authoring = FeatureAuthoringState::default();
    assert!(matches!(
        authoring.activate(
            &snapshot,
            &accepted_document,
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    assert_eq!(
        authoring.options().fillet_radius.map(f64::to_bits),
        Some(DEFAULT_RADIUS.to_bits()),
        "activation must initialize the model-scale default"
    );

    // This is the presentation-independent equivalent of the workbench's blank
    // optional radius field. It must mean "no explicit override", not erase the
    // required default established by activation.
    let _ = authoring.set_options(
        &snapshot,
        FeatureAuthoringOptions {
            fillet_radius: None,
            flip_first_side: false,
            flip_second_side: false,
            alternate_arc: false,
        },
    );

    let first_index = order[0];
    let first = authoring.pick_many(
        &snapshot,
        picks_at(&fixture, fixture.pick_positions[first_index]),
    );
    assert!(matches!(
        first,
        FeatureAuthoringOutcome::Collecting {
            ref pending,
            ref guidance,
        } if pending.len() == 1
            && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
            && guidance.completed_corners == 0
    ));

    let second_index = order[1];
    let candidate = expect_candidate_after_second_pick(authoring.pick_many(
        &snapshot,
        picks_at(&fixture, fixture.pick_positions[second_index]),
    ));
    assert_eq!(candidate.corners().len(), 1);
    assert_eq!(candidate.radius().to_bits(), DEFAULT_RADIUS.to_bits());
    assert_eq!(
        [
            candidate.corners()[0].corner.first.source.span,
            candidate.corners()[0].corner.second.source.span,
        ],
        fixture.spans,
        "persistent parent order must canonicalize independently of pick order"
    );

    let preview = fixture
        .coordinator
        .prepare_feature_authoring_preview(
            fixture.coordinator.feature_document().identity(),
            &candidate,
            "two-line black-box Fillet",
        )
        .expect("exact computed Fillet preview");
    let applied = match authoring.apply() {
        FeatureAuthoringOutcome::Apply(candidate) => candidate,
        other => panic!("complete Fillet candidate did not apply: {other:?}"),
    };
    assert_eq!(applied, candidate);
    let mutation = fixture
        .coordinator
        .apply_feature_authoring_preview(preview.token, &applied)
        .expect("exact preview publication");
    assert!(
        fixture
            .coordinator
            .feature_document()
            .feature(mutation.value)
            .is_some()
    );
    assert_eq!(fixture.coordinator.feature_document().features().len(), 1);
    assert!(fixture.coordinator.computed_snapshot().is_some());
}

#[test]
fn blank_radius_keeps_default_through_sequential_mid_span_fillet_picks_and_publication() {
    exercise_blank_radius_two_line_chain([0, 1]);
}

#[test]
fn reverse_mid_span_pick_order_keeps_the_same_blank_radius_fillet_contract() {
    exercise_blank_radius_two_line_chain([1, 0]);
}

#[test]
fn exact_shared_endpoint_of_two_separate_lines_completes_one_corner_in_one_pick() {
    let fixture = two_line_interaction_fixture();
    let (mut authoring, snapshot, document) = activate_authoring(&fixture.coordinator);

    let candidate = expect_candidate_after_second_pick(pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &fixture.scene,
        [4.0, 0.0],
    ));

    assert_eq!(candidate.corners().len(), 1);
    assert_eq!(authoring.completed_corner_count(), 1);
    assert_eq!(
        authoring.guidance().stage,
        FeatureAuthoringStage::PreviewReady
    );
    assert_eq!(
        [
            candidate.corners()[0].corner.first.source.span,
            candidate.corners()[0].corner.second.source.span,
        ],
        fixture.spans
    );
    assert!(matches!(
        authoring.apply(),
        FeatureAuthoringOutcome::Apply(ref applied) if applied == &candidate
    ));
}

#[test]
fn pending_line_then_shared_endpoint_uses_only_the_other_span_without_a_dangling_pick() {
    let fixture = two_line_interaction_fixture();
    let (mut authoring, snapshot, document) = activate_authoring(&fixture.coordinator);

    let first = pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &fixture.scene,
        fixture.pick_positions[0],
    );
    assert!(matches!(
        first,
        FeatureAuthoringOutcome::Collecting {
            ref pending,
            ref guidance,
        } if pending.len() == 1
            && pending[0].curve.source.span == fixture.spans[0]
            && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
    ));

    let candidate = expect_candidate_after_second_pick(pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &fixture.scene,
        [4.0, 0.0],
    ));
    assert_eq!(candidate.corners().len(), 1);
    assert_eq!(authoring.completed_corner_count(), 1);
    assert_eq!(
        authoring.guidance().stage,
        FeatureAuthoringStage::PreviewReady
    );
    assert!(matches!(
        authoring.apply(),
        FeatureAuthoringOutcome::Apply(ref applied) if applied == &candidate
    ));
    let sources = [
        candidate.corners()[0].corner.first.source.span,
        candidate.corners()[0].corner.second.source.span,
    ];
    assert_eq!(sources, fixture.spans);
}

fn crossing_lines_fixture() -> TwoLineInteractionFixture {
    let mut document = SketchDocument::new(10.0).expect("document");
    let horizontal_start = document
        .add_point("horizontal start", [-4.0, 0.0])
        .expect("horizontal start");
    let horizontal_end = document
        .add_point("horizontal end", [0.2, 0.0])
        .expect("horizontal end");
    let vertical_start = document
        .add_point("vertical start", [0.0, -4.0])
        .expect("vertical start");
    let vertical_end = document
        .add_point("vertical end", [0.0, 0.2])
        .expect("vertical end");
    let horizontal = document
        .add_curve(
            "crossing horizontal",
            CurveDefinition::Line {
                start: horizontal_start,
                end: horizontal_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("crossing horizontal");
    let vertical = document
        .add_curve(
            "crossing vertical",
            CurveDefinition::Line {
                start: vertical_start,
                end: vertical_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("crossing vertical");
    let spans = [CurveSpan::line(horizontal), CurveSpan::line(vertical)];
    let (coordinator, scene) = coordinator_and_scene(document);
    TwoLineInteractionFixture {
        coordinator,
        scene,
        spans,
        pick_positions: [[0.0, 0.0], [0.0, 0.0]],
    }
}

#[test]
fn repeated_crossing_click_falls_through_duplicate_support_to_the_other_line() {
    let fixture = crossing_lines_fixture();
    let (mut authoring, snapshot, document) = activate_authoring(&fixture.coordinator);
    let first = pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &fixture.scene,
        [0.0, 0.0],
    );
    assert!(matches!(
        first,
        FeatureAuthoringOutcome::Collecting {
            ref pending,
            ref guidance,
        } if pending.len() == 1
            && pending[0].curve.source.span == fixture.spans[0]
            && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
    ));

    let candidate = expect_candidate_after_second_pick(pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &fixture.scene,
        [0.0, 0.0],
    ));
    assert_eq!(candidate.corners().len(), 1);
    assert_eq!(
        authoring.guidance().stage,
        FeatureAuthoringStage::PreviewReady
    );
    let sources = [
        candidate.corners()[0].corner.first.source.span,
        candidate.corners()[0].corner.second.source.span,
    ];
    assert_eq!(sources, fixture.spans);
}

#[test]
fn lone_line_endpoint_falls_through_its_inapplicable_point_to_the_curve() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [4.0, 0.0]).expect("end");
    let span = CurveSpan::line(
        document
            .add_curve(
                "lone ordinary line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("lone line"),
    );
    let (coordinator, scene) = coordinator_and_scene(document);
    let (mut authoring, snapshot, document) = activate_authoring(&coordinator);
    let outcome = pick_at(&mut authoring, &snapshot, &document, &scene, [0.0, 0.0]);
    assert!(matches!(
        outcome,
        FeatureAuthoringOutcome::Collecting {
            ref pending,
            ref guidance,
        } if pending.len() == 1
            && pending[0].curve.source.span == span
            && pending[0].curve.parameter.to_bits() == 0.0_f64.to_bits()
            && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
    ));
}

#[test]
fn high_valence_junction_warns_instead_of_selecting_an_arbitrary_underlying_line() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let junction = document
        .add_point("three-way junction", [0.0, 0.0])
        .expect("junction");
    let endpoints = [[4.0, 0.0], [0.0, 4.0], [-4.0, 0.0]].map(|position| {
        document
            .add_point("junction endpoint", position)
            .expect("endpoint")
    });
    let spans = [
        (junction, endpoints[0], [1.0, 0.0]),
        (junction, endpoints[1], [0.0, 1.0]),
        (endpoints[2], junction, [1.0, 0.0]),
    ]
    .map(|(start, end, branch_direction)| {
        CurveSpan::line(
            document
                .add_curve(
                    "junction line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction,
                    },
                )
                .expect("junction line"),
        )
    });
    let (coordinator, scene) = coordinator_and_scene(document);
    let (mut authoring, snapshot, document) = activate_authoring(&coordinator);
    let before = authoring.clone();

    let ambiguous = pick_at(&mut authoring, &snapshot, &document, &scene, [0.0, 0.0]);

    assert!(matches!(
        ambiguous,
        FeatureAuthoringOutcome::Warning(ref warning)
            if warning.kind == FeatureAuthoringWarningKind::AmbiguousTrimSide
    ));
    assert_eq!(authoring, before);
    assert!(matches!(
        pick_at(
            &mut authoring,
            &snapshot,
            &document,
            &scene,
            [2.0, 0.0],
        ),
        FeatureAuthoringOutcome::Collecting { ref pending, .. }
            if pending.len() == 1 && pending[0].curve.source.span == spans[0]
    ));
}

#[test]
fn stale_visible_scene_is_rejected_before_it_can_stamp_a_current_pick() {
    let fixture = two_line_interaction_fixture();
    let (mut authoring, snapshot, document) = activate_authoring(&fixture.coordinator);
    let before = authoring.clone();
    let mut stale_scene = fixture.scene.clone();
    stale_scene.accepted_revision = stale_scene.accepted_revision.saturating_add(1);

    let outcome = pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &stale_scene,
        fixture.pick_positions[0],
    );

    assert!(matches!(
        outcome,
        FeatureAuthoringOutcome::Warning(ref warning)
            if warning.kind == FeatureAuthoringWarningKind::StalePick
    ));
    assert_eq!(authoring, before);
}

#[test]
fn published_feature_selection_does_not_poison_the_next_fillet_batch() {
    let mut fixture = two_line_interaction_fixture();
    let (mut authoring, snapshot, document) = activate_authoring(&fixture.coordinator);
    let candidate = expect_candidate_after_second_pick(pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &fixture.scene,
        [4.0, 0.0],
    ));
    let preview = fixture
        .coordinator
        .prepare_feature_authoring_preview(
            fixture.coordinator.feature_document().identity(),
            &candidate,
            "first Fillet",
        )
        .expect("preview");
    let feature = fixture
        .coordinator
        .apply_feature_authoring_preview(preview.token, &candidate)
        .expect("publication")
        .value;
    assert_eq!(
        authoring.publication_succeeded(),
        FeatureAuthoringOutcome::ModeExited
    );

    let next_snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("next authoring snapshot");
    let next_document = fixture
        .coordinator
        .operation_authoring_document()
        .expect("next accepted document");
    let entered = authoring.activate(
        &next_snapshot,
        next_document,
        FeatureAuthoringTool::Fillet,
        &[(SelectionItem::Feature(feature), None)],
    );

    assert!(matches!(entered, FeatureAuthoringOutcome::ModeEntered(_)));
    assert_eq!(authoring.completed_corner_count(), 0);
    assert_eq!(
        authoring.guidance().stage,
        FeatureAuthoringStage::PickFirstFilletCurve
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn adjacent_fillet_sets_publish_sequentially_through_screen_pick_transactions() {
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
    let spans = [0, 1, 2].map(|segment| CurveSpan { curve, segment });
    let (mut coordinator, scene) = coordinator_and_scene(document);
    let baseline_design = coordinator.session().design_identity();
    let baseline_accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted state")
        .identity();
    let baseline_sketch = coordinator
        .session()
        .export_accepted_json()
        .expect("accepted sketch JSON");
    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("authoring snapshot");
    let accepted_document = coordinator
        .operation_authoring_document()
        .expect("accepted authoring document")
        .clone();
    let mut authoring = FeatureAuthoringState::default();
    assert!(matches!(
        authoring.activate(
            &snapshot,
            &accepted_document,
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));

    let first_support = coordinator
        .transact_feature_authoring_pick_at(
            &mut authoring,
            &scene,
            scene.viewport.model_to_screen([2.0, 0.0]),
            PickTolerance::default(),
            "first adjacent Fillet",
        )
        .expect("first support transaction");
    assert!(matches!(
        first_support.outcome,
        FeatureAuthoringOutcome::Collecting { ref pending, .. }
            if pending.len() == 1 && pending[0].curve.source.span == spans[0]
    ));
    assert!(first_support.preview.is_none());
    let first_corner = coordinator
        .transact_feature_authoring_pick_at(
            &mut authoring,
            &scene,
            scene.viewport.model_to_screen([4.0, 1.0]),
            PickTolerance::default(),
            "first adjacent Fillet",
        )
        .expect("first corner transaction");
    let first_candidate = expect_candidate_after_second_pick(first_corner.outcome);
    let first_preview = first_corner.preview.expect("first exact preview");
    assert!(matches!(
        authoring.apply(),
        FeatureAuthoringOutcome::Apply(ref candidate) if candidate == &first_candidate
    ));
    let first_feature = coordinator
        .apply_feature_authoring_preview(first_preview.token, &first_candidate)
        .expect("publish first Fillet")
        .value;
    assert_eq!(
        authoring.publication_succeeded(),
        FeatureAuthoringOutcome::ModeExited
    );

    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Feature(first_feature)]);
    let second_snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("second authoring snapshot");
    let second_document = coordinator
        .operation_authoring_document()
        .expect("second accepted authoring document")
        .clone();
    assert!(matches!(
        authoring.activate(
            &second_snapshot,
            &second_document,
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Feature(first_feature), None)],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    assert_eq!(authoring.completed_corner_count(), 0);

    let shared_support = coordinator
        .transact_feature_authoring_pick_at(
            &mut authoring,
            &scene,
            scene.viewport.model_to_screen([4.0, 3.0]),
            PickTolerance::default(),
            "second adjacent Fillet",
        )
        .expect("shared support transaction");
    assert!(matches!(
        shared_support.outcome,
        FeatureAuthoringOutcome::Collecting { ref pending, .. }
            if pending.len() == 1 && pending[0].curve.source.span == spans[1]
    ));
    let second_corner = coordinator
        .transact_feature_authoring_pick_at(
            &mut authoring,
            &scene,
            scene.viewport.model_to_screen([5.0, 4.0]),
            PickTolerance::default(),
            "second adjacent Fillet",
        )
        .expect("second corner transaction");
    let second_candidate = expect_candidate_after_second_pick(second_corner.outcome);
    let second_preview = second_corner.preview.expect("second exact preview");
    let second_feature = coordinator
        .apply_feature_authoring_preview(second_preview.token, &second_candidate)
        .expect("publish second Fillet")
        .value;
    assert_ne!(first_feature, second_feature);
    assert_eq!(
        authoring.publication_succeeded(),
        FeatureAuthoringOutcome::ModeExited
    );

    let (first_corner, second_corner) = {
        let first = coordinator
            .feature_document()
            .feature(first_feature)
            .expect("first feature");
        let second = coordinator
            .feature_document()
            .feature(second_feature)
            .expect("second feature");
        let ComputedFeatureDefinition::FilletSet(first) = &first.definition;
        let ComputedFeatureDefinition::FilletSet(second) = &second.definition;
        assert_eq!(first.corners.len(), 1);
        assert_eq!(second.corners.len(), 1);
        (first.corners[0], second.corners[0])
    };
    assert_ne!(first_corner.id, second_corner.id);
    let first_middle = [first_corner.first, first_corner.second]
        .into_iter()
        .find(|parent| parent.source.span == spans[1])
        .expect("first feature middle-span parent");
    let second_middle = [second_corner.first, second_corner.second]
        .into_iter()
        .find(|parent| parent.source.span == spans[1])
        .expect("second feature middle-span parent");
    assert_eq!(
        first_middle.retained_endpoint,
        DocumentFilletTrimEndpoint::Start
    );
    assert_eq!(
        second_middle.retained_endpoint,
        DocumentFilletTrimEndpoint::End
    );
    let owners = [
        ComputedCornerRef {
            feature: first_feature,
            corner: first_corner.id,
        },
        ComputedCornerRef {
            feature: second_feature,
            corner: second_corner.id,
        },
    ];
    let computed = coordinator.computed_snapshot().expect("computed output");
    assert!(owners.iter().all(|owner| {
        computed.feature_evaluations().iter().any(|evaluation| {
            evaluation.feature == owner.feature
                && matches!(
                    &evaluation.state,
                    ComputedFeatureEvaluationState::Current { .. }
                )
        }) && computed.fillet_arc_edge(*owner).is_some()
    }));
    assert_eq!(coordinator.feature_document().features().len(), 2);
    assert_eq!(coordinator.session().design_identity(), baseline_design);
    assert_eq!(
        coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("unchanged accepted sketch")
            .identity(),
        baseline_accepted
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("unchanged accepted sketch JSON"),
        baseline_sketch
    );

    coordinator.undo().expect("undo second Fillet");
    assert!(
        coordinator
            .feature_document()
            .feature(first_feature)
            .is_some()
    );
    assert!(
        coordinator
            .feature_document()
            .feature(second_feature)
            .is_none()
    );
    coordinator.redo().expect("redo second Fillet");
    let ComputedFeatureDefinition::FilletSet(redone) = &coordinator
        .feature_document()
        .feature(second_feature)
        .expect("redone second feature")
        .definition;
    assert_eq!(redone.corners[0].id, second_corner.id);
    let redone_snapshot = coordinator
        .computed_snapshot()
        .expect("redone computed output");
    assert!(owners.iter().all(|owner| {
        redone_snapshot
            .feature_evaluations()
            .iter()
            .any(|evaluation| {
                evaluation.feature == owner.feature
                    && matches!(
                        &evaluation.state,
                        ComputedFeatureEvaluationState::Current { .. }
                    )
            })
    }));
}

#[test]
fn pathologically_crowded_click_is_bounded_and_state_neutral() {
    let mut document = SketchDocument::new(10.0).expect("document");
    for index in 0..257 {
        document
            .add_point(format!("coincident {index}"), [0.0, 0.0])
            .expect("coincident point");
    }
    let (coordinator, scene) = coordinator_and_scene(document);
    let (mut authoring, snapshot, document) = activate_authoring(&coordinator);
    let before = authoring.clone();

    let outcome = pick_at(&mut authoring, &snapshot, &document, &scene, [0.0, 0.0]);

    assert!(matches!(
        outcome,
        FeatureAuthoringOutcome::Warning(ref warning)
            if warning.kind == FeatureAuthoringWarningKind::WorkStopped
    ));
    assert_eq!(authoring, before);
}

#[test]
fn empty_canvas_pick_is_distinct_from_a_native_warning_and_state_neutral() {
    let fixture = two_line_interaction_fixture();
    let (mut authoring, snapshot, document) = activate_authoring(&fixture.coordinator);
    let before = authoring.clone();

    let outcome = pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &fixture.scene,
        [-100.0, -100.0],
    );

    assert!(matches!(
        outcome,
        FeatureAuthoringOutcome::NoNativeHit(ref guidance)
            if guidance.stage == FeatureAuthoringStage::PickFirstFilletCurve
    ));
    assert_eq!(authoring, before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn nonduplicate_pair_failure_does_not_silently_choose_a_lower_overlapping_curve() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let pending_start = document
        .add_point("pending start", [-4.0, 0.0])
        .expect("pending start");
    let shared_corner = document
        .add_point("shared corner", [0.0, 0.0])
        .expect("shared corner");
    let parallel_start = document
        .add_point("parallel start", [-4.0, 1.0])
        .expect("parallel start");
    let parallel_end = document
        .add_point("parallel end", [4.0, 1.0])
        .expect("parallel end");
    let valid_end = document
        .add_point("valid end", [0.0, 3.0])
        .expect("valid end");
    let pending_span = CurveSpan::line(
        document
            .add_curve(
                "pending horizontal",
                CurveDefinition::Line {
                    start: pending_start,
                    end: shared_corner,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("pending line"),
    );
    let _parallel_span = CurveSpan::line(
        document
            .add_curve(
                "lower-priority parallel",
                CurveDefinition::Line {
                    start: parallel_start,
                    end: parallel_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("parallel line"),
    );
    let valid_span = CurveSpan::line(
        document
            .add_curve(
                "overlapping vertical",
                CurveDefinition::Line {
                    start: shared_corner,
                    end: valid_end,
                    branch_direction: [0.0, 1.0],
                },
            )
            .expect("valid line"),
    );
    let (coordinator, scene) = coordinator_and_scene(document);
    let (mut authoring, snapshot, document) = activate_authoring(&coordinator);
    assert!(matches!(
        pick_at(
            &mut authoring,
            &snapshot,
            &document,
            &scene,
            [-2.0, 0.0],
        ),
        FeatureAuthoringOutcome::Collecting { ref pending, .. }
            if pending.len() == 1 && pending[0].curve.source.span == pending_span
    ));
    let before_failure = authoring.clone();
    let failed = pick_at(&mut authoring, &snapshot, &document, &scene, [0.0, 1.0]);

    assert!(matches!(
        failed,
        FeatureAuthoringOutcome::Warning(ref warning)
            if warning.kind == FeatureAuthoringWarningKind::SingularFillet
    ));
    assert_eq!(authoring, before_failure);
    let recovered = expect_candidate_after_second_pick(pick_at(
        &mut authoring,
        &snapshot,
        &document,
        &scene,
        [0.0, 0.5],
    ));
    let sources = [
        recovered.corners()[0].corner.first.source.span,
        recovered.corners()[0].corner.second.source.span,
    ];
    assert!(sources.contains(&pending_span));
    assert!(sources.contains(&valid_span));
}
