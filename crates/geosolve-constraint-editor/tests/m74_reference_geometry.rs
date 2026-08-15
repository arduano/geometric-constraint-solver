// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AuthoringOutcome, AuthoringState, AuthoringTool, ConstraintIntent, ConstructionCommitPlan,
    ConstructionPoint, ConstructionProposal, DraftInferenceEngine, DraftInferenceFrame,
    DraftInferenceInput, DraftInferenceLimits, DraftInferenceRelation, DraftInferenceSample,
    DraftInferenceSceneInputCollection, DraftInferenceStatus, DraftInferenceSubject,
    DraftPointSlot, DraftSpanSlot, EditorScene, GeometryInteractionPolicy, GeometryVisibility,
    InferredRelation, PickTolerance, ResolvedConstraintKind, RetainedEditorCoordinator,
    ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentCoordinateAxis,
    DocumentSolveRequest, GeometryRole, RetainedSketchDocumentSession, SketchDatum, SketchDocument,
    SolverConfig,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

fn retained_scene(
    document: SketchDocument,
    viewport: Viewport,
) -> (RetainedEditorCoordinator, EditorScene) {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained datum fixture");
    let accepted = session.accepted_state().expect("accepted datum fixture");
    let scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        session.design_identity(),
        accepted.document(),
        session.design_document(),
        viewport,
        0.5,
    )
    .expect("datum scene");
    (
        RetainedEditorCoordinator::new(session).expect("datum coordinator"),
        scene,
    )
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "one picking regression keeps native priority, pixel boundaries, visibility, authoring, and Fit neutrality together"
)]
fn intrinsic_datum_picking_is_pixel_bounded_native_first_and_fit_neutral() {
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
    let mut document = SketchDocument::new(1.0).expect("document");
    let start = document
        .add_point("origin point", [0.0, 0.0])
        .expect("point");
    let end = document.add_point("line end", [2.0, 0.0]).expect("point");
    let line = document
        .add_curve(
            "native X-axis line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let (_, scene) = retained_scene(document, viewport);
    assert_eq!(scene.datums.len(), 3);
    assert_eq!(scene.model_bounds(), Some(([0.0, 0.0], [2.0, 0.0])));

    let origin = viewport.model_to_screen([0.0, 0.0]);
    assert_eq!(
        scene
            .hit_test(origin, PickTolerance::default())
            .map(|hit| hit.item),
        Some(SelectionItem::Point(start)),
        "a native point must outrank the intrinsic Origin"
    );
    assert_eq!(
        scene
            .hit_test(
                viewport.model_to_screen([1.0, 0.0]),
                PickTolerance::default()
            )
            .map(|hit| hit.item),
        Some(SelectionItem::Curve(CurveSpan::line(line))),
        "native curve geometry must outrank a coincident datum axis"
    );
    assert_eq!(
        scene
            .hit_test(
                viewport.model_to_screen([8.0, 0.0]),
                PickTolerance::default()
            )
            .map(|hit| hit.item),
        Some(SelectionItem::Datum(SketchDatum::XAxis))
    );

    let empty = SketchDocument::new(1.0).expect("empty document");
    let (_, empty_scene) = retained_scene(empty, viewport);
    assert_eq!(
        empty_scene.model_bounds(),
        None,
        "datums never enlarge Fit bounds"
    );
    let mut authoring = AuthoringState::default();
    assert!(matches!(
        authoring.activate(
            &SketchDocument::new(1.0).expect("authoring document"),
            AuthoringTool::Constraint(ConstraintIntent::Coincident),
            &[],
        ),
        AuthoringOutcome::ModeEntered { .. }
    ));
    let authoring_document = SketchDocument::new(1.0).expect("authoring document");
    assert!(matches!(
        authoring.pick_at_with_policy(
            &authoring_document,
            &empty_scene,
            viewport.model_to_screen([8.0, 0.0]),
            PickTolerance::default(),
            GeometryInteractionPolicy::default(),
        ),
        AuthoringOutcome::Collecting { operands, .. }
            if operands.len() == 1
                && operands[0].item == SelectionItem::Datum(SketchDatum::XAxis)
    ));
    let diagonal = 6.0 / 2.0_f64.sqrt();
    assert_eq!(
        empty_scene
            .hit_test(
                ScreenPoint {
                    x: origin.x + diagonal,
                    y: origin.y + diagonal,
                },
                PickTolerance::default(),
            )
            .map(|hit| hit.item),
        Some(SelectionItem::Datum(SketchDatum::Origin))
    );
    assert!(
        empty_scene
            .hit_test(
                ScreenPoint {
                    x: origin.x + 4.3,
                    y: origin.y + 4.3,
                },
                PickTolerance::default(),
            )
            .is_none(),
        "outside both the six-pixel Origin and four-pixel axis bands must miss"
    );
    assert_eq!(
        empty_scene
            .hit_test(
                ScreenPoint {
                    x: origin.x + 100.0,
                    y: origin.y + 4.0,
                },
                PickTolerance::default(),
            )
            .map(|hit| hit.item),
        Some(SelectionItem::Datum(SketchDatum::XAxis))
    );
    assert!(
        empty_scene
            .hit_test(
                ScreenPoint {
                    x: origin.x + 100.0,
                    y: origin.y + 4.01,
                },
                PickTolerance::default(),
            )
            .is_none()
    );

    let hidden = GeometryInteractionPolicy {
        visibility: GeometryVisibility {
            reference_geometry: false,
            ..GeometryVisibility::default()
        },
        ..GeometryInteractionPolicy::default()
    };
    assert!(
        empty_scene
            .hit_test_with_policy(origin, PickTolerance::default(), hidden)
            .is_none()
    );

    let offscreen_viewport =
        Viewport::new([1000.0, 700.0], [0.0, -7.02], 50.0).expect("offscreen viewport");
    let (_, offscreen_scene) = retained_scene(
        SketchDocument::new(1.0).expect("offscreen document"),
        offscreen_viewport,
    );
    assert!(!offscreen_scene.datums[0].is_visible_in_viewport(offscreen_viewport));
    assert!(!offscreen_scene.datums[1].is_visible_in_viewport(offscreen_viewport));
    assert!(
        offscreen_scene
            .hit_test(ScreenPoint { x: 700.0, y: 0.0 }, PickTolerance::default(),)
            .is_none(),
        "an Origin and X axis outside the painted plane must expose no edge hit surface"
    );
    assert_eq!(
        offscreen_scene
            .hit_test(ScreenPoint { x: 500.0, y: 100.0 }, PickTolerance::default())
            .map(|hit| hit.item),
        Some(SelectionItem::Datum(SketchDatum::YAxis)),
        "the independently visible Y axis must remain pickable while Origin is off-screen"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn datum_contextual_relations_are_order_symmetric_and_axis_parallelism_is_ordinary() {
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
    let mut document = SketchDocument::new(1.0).expect("document");
    let first = document.add_point("first", [2.0, 3.0]).expect("point");
    let second = document.add_point("second", [4.0, 5.0]).expect("point");
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [2.0_f64.sqrt() / 2.0; 2],
            },
        )
        .expect("line");
    let (mut coordinator, _) = retained_scene(document, viewport);
    let span = SelectionItem::Curve(CurveSpan::line(line));
    let point = SelectionItem::Point(first);

    for (intent, operands, expected) in [
        (
            ConstraintIntent::Coincident,
            [point, SelectionItem::Datum(SketchDatum::Origin)],
            Some(ResolvedConstraintKind::CoincidentWithOrigin),
        ),
        (
            ConstraintIntent::Coincident,
            [point, SelectionItem::Datum(SketchDatum::XAxis)],
            Some(ResolvedConstraintKind::PointOnDatumAxis),
        ),
        (
            ConstraintIntent::Collinear,
            [span, SelectionItem::Datum(SketchDatum::YAxis)],
            Some(ResolvedConstraintKind::CollinearWithDatumAxis),
        ),
        (
            ConstraintIntent::Parallel,
            [span, SelectionItem::Datum(SketchDatum::XAxis)],
            Some(ResolvedConstraintKind::HorizontalLine),
        ),
        (
            ConstraintIntent::Parallel,
            [span, SelectionItem::Datum(SketchDatum::YAxis)],
            Some(ResolvedConstraintKind::VerticalLine),
        ),
        (
            ConstraintIntent::Perpendicular,
            [span, SelectionItem::Datum(SketchDatum::XAxis)],
            Some(ResolvedConstraintKind::VerticalLine),
        ),
        (
            ConstraintIntent::Perpendicular,
            [span, SelectionItem::Datum(SketchDatum::YAxis)],
            Some(ResolvedConstraintKind::HorizontalLine),
        ),
    ] {
        coordinator.editor_mut().set_selection(operands);
        assert_eq!(coordinator.resolved_constraint(intent), expected);
        coordinator
            .editor_mut()
            .set_selection(operands.into_iter().rev());
        assert_eq!(coordinator.resolved_constraint(intent), expected);
    }

    coordinator
        .editor_mut()
        .set_selection([span, SelectionItem::Datum(SketchDatum::Origin)]);
    assert_eq!(
        coordinator.resolved_constraint(ConstraintIntent::Collinear),
        None
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "authenticated datum inference owns exact Cartesian projections and one atomic publication lifecycle"
)]
fn datum_inference_and_atomic_publication_match_on_native_and_wasm() {
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
    let document = SketchDocument::new(1.0).expect("document");
    let (mut coordinator, scene) = retained_scene(document, viewport);
    let scene = scene
        .with_retained_session(coordinator.session())
        .expect("authenticated datum scene");

    let resolve = |sample: [f64; 2], span_start: Option<[f64; 2]>| {
        let collected = scene.draft_inference_scene_inputs(
            scene.viewport.model_to_screen(sample),
            DraftInferenceSubject::PointOperand,
            DraftInferenceLimits::default(),
        );
        let DraftInferenceSceneInputCollection::Complete(inputs) = collected else {
            panic!("empty datum scene must remain within inference limits")
        };
        let mut engine = DraftInferenceEngine::default();
        engine
            .resolve(
                &DraftInferenceFrame::from_scene_with_semantic_centers(
                    &scene,
                    GeometryInteractionPolicy::default(),
                    DraftInferenceSample {
                        raw_screen_position: scene.viewport.model_to_screen(sample),
                        subject: DraftInferenceSubject::PointOperand,
                        span_start,
                    },
                    inputs.anchors,
                    inputs.semantic_centers,
                ),
                DraftInferenceInput::default(),
            )
            .expect("datum inference")
    };

    let origin = resolve([0.06, 0.06], None);
    let DraftInferenceStatus::Resolved { candidate } = origin.status else {
        panic!("Origin must resolve")
    };
    let origin = origin
        .candidates
        .iter()
        .find(|value| value.id == candidate)
        .expect("Origin winner");
    assert_eq!(
        origin.relations,
        [DraftInferenceRelation::CoincidentWithOrigin]
    );
    assert_eq!(origin.adjusted_model_position, [0.0, 0.0]);

    let bundled = resolve([2.04, 0.04], Some([2.0, -4.0]));
    let DraftInferenceStatus::Resolved { candidate } = bundled.status else {
        panic!("orthogonal axis bundle must resolve")
    };
    let bundled = bundled
        .candidates
        .iter()
        .find(|value| value.id == candidate)
        .expect("bundle winner");
    assert_eq!(
        bundled.relations,
        [
            DraftInferenceRelation::PointOnDatumAxis {
                axis: DocumentCoordinateAxis::X,
            },
            DraftInferenceRelation::Vertical,
        ]
    );
    assert_eq!(bundled.adjusted_model_position, [2.0, 0.0]);

    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted input");
    let origin_commit = coordinator
        .apply_construction_plan(
            &expected,
            &ConstructionCommitPlan {
                proposal: ConstructionProposal::Point {
                    point: ConstructionPoint::New([0.06, 0.06]),
                },
                role: GeometryRole::Profile,
                relations: vec![InferredRelation::CoincidentWithOrigin {
                    point: DraftPointSlot::Created { point_index: 0 },
                }],
            },
        )
        .expect("atomic Origin commit");
    assert!(origin_commit.published_accepted.is_some());
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(origin_commit.value.constraints[0].constraint)
            .expect("Origin constraint")
            .definition,
        DocumentConstraintDefinition::CoincidentWithOrigin { .. }
    ));

    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted input");
    let line_commit = coordinator
        .apply_construction_plan(
            &expected,
            &ConstructionCommitPlan {
                proposal: ConstructionProposal::Line {
                    start: ConstructionPoint::New([2.0, -4.0]),
                    end: ConstructionPoint::New([2.04, 0.04]),
                },
                role: GeometryRole::Profile,
                relations: vec![
                    InferredRelation::PointOnDatumAxis {
                        point: DraftPointSlot::Created { point_index: 1 },
                        axis: DocumentCoordinateAxis::X,
                    },
                    InferredRelation::Vertical {
                        line: DraftSpanSlot::Created {
                            curve_index: 0,
                            segment: 0,
                        },
                    },
                ],
            },
        )
        .expect("atomic datum-axis bundle commit");
    assert!(line_commit.published_accepted.is_some());
    assert_eq!(line_commit.value.constraints.len(), 2);
    let definitions = line_commit
        .value
        .constraints
        .iter()
        .map(|created| {
            &coordinator
                .session()
                .design_document()
                .constraint(created.constraint)
                .expect("bundle constraint")
                .definition
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        definitions.as_slice(),
        [
            DocumentConstraintDefinition::PointOnDatumAxis {
                axis: DocumentCoordinateAxis::X,
                ..
            },
            DocumentConstraintDefinition::Vertical { .. }
        ]
    ));

    let committed = coordinator
        .session()
        .design_document()
        .to_draft_v5_json()
        .expect("committed draft v5");
    coordinator.undo().expect("atomic undo");
    assert_eq!(
        coordinator.session().design_document().constraints().len(),
        1
    );
    coordinator.redo().expect("atomic redo");
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .to_draft_v5_json()
            .expect("redone draft v5"),
        committed
    );
}
