// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AuthoringMutation, AuthoringOperand, AuthoringOperandKind, AuthoringOutcome, AuthoringState,
    AuthoringTool, AuthoringWarning, ConstraintIntent, ConstructionCommitPlan, ConstructionPoint,
    ConstructionProposal, ConstructionRelationDefinition, DisabledReason, DraftInferenceEngine,
    DraftInferenceFrame, DraftInferenceInput, DraftInferenceLimits, DraftInferenceRelation,
    DraftInferenceSample, DraftInferenceSceneInputCollection, DraftInferenceStatus,
    DraftInferenceSubject, DraftPointSlot, DraftSpanSlot, EditorScene, GeometryInteractionPolicy,
    GeometryVisibility, InferredRelation, PickTolerance, ResolvedConstraintKind,
    RetainedEditorCoordinator, SceneAnnotationGeometry, SceneAnnotationKind, SceneConstraintGlyph,
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

fn datum_axis_symmetry_fixture() -> (
    SketchDocument,
    geosolve_sketch::DesignPointId,
    geosolve_sketch::DesignPointId,
) {
    let mut document = SketchDocument::new(1.0).expect("document");
    let first = document.add_point("first", [2.0, 3.0]).expect("point");
    let second = document.add_point("second", [4.0, -1.0]).expect("point");
    (document, first, second)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn datum_axis_symmetry_preselection_accepts_every_operand_permutation() {
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
    for datum in [SketchDatum::XAxis, SketchDatum::YAxis] {
        for permutation in [
            [0_u8, 1, 2],
            [1, 0, 2],
            [0, 2, 1],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let (document, first, second) = datum_axis_symmetry_fixture();
            let (mut coordinator, _) = retained_scene(document, viewport);
            let values = [
                SelectionItem::Point(first),
                SelectionItem::Point(second),
                SelectionItem::Datum(datum),
            ];
            let operands =
                permutation.map(|index| AuthoringOperand::selected(values[usize::from(index)]));
            let mut authoring = AuthoringState::default();
            let AuthoringOutcome::Apply(application) = authoring.activate(
                coordinator.session().design_document(),
                AuthoringTool::Constraint(ConstraintIntent::Symmetric),
                &operands,
            ) else {
                panic!("{datum:?} permutation {permutation:?} must apply")
            };
            assert_eq!(
                application.resolved_constraint,
                Some(ResolvedConstraintKind::SymmetricAboutDatumAxis)
            );
            assert!(matches!(
                application.operands.as_slice(),
                [
                    AuthoringOperand {
                        item: SelectionItem::Point(_),
                        ..
                    },
                    AuthoringOperand {
                        item: SelectionItem::Point(_),
                        ..
                    },
                    AuthoringOperand {
                        item: SelectionItem::Datum(actual),
                        ..
                    }
                ] if *actual == datum
            ));
            let history = (coordinator.history_len(), coordinator.history_cursor());
            let AuthoringMutation::Constraint(outcome) = coordinator
                .apply_authoring(coordinator.session().design_identity(), &application)
                .expect("datum-axis symmetry application")
            else {
                panic!("symmetric authoring must create a constraint")
            };
            assert!(outcome.published_accepted.is_some());
            assert_eq!(
                (coordinator.history_len(), coordinator.history_cursor()),
                (history.0 + 1, history.1 + 1)
            );
            let definition = &coordinator
                .session()
                .design_document()
                .constraint(outcome.value)
                .expect("stored symmetry")
                .definition;
            let DocumentConstraintDefinition::SymmetricAboutDatumAxis {
                first: stored_first,
                second: stored_second,
                axis,
            } = definition
            else {
                panic!("axis symmetry must retain its exact definition")
            };
            assert_ne!(stored_first, stored_second);
            assert!([first, second].contains(stored_first));
            assert!([first, second].contains(stored_second));
            assert_eq!(*axis, datum.coordinate_axis().unwrap());
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "one datum-axis symmetry checkpoint covers active authoring, scene ownership, lifecycle and reload"
)]
fn datum_axis_symmetry_active_authoring_scene_lifecycle_and_reload_are_exact() {
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
    let (document, first, second) = datum_axis_symmetry_fixture();
    let (mut coordinator, _) = retained_scene(document, viewport);
    let tool = AuthoringTool::Constraint(ConstraintIntent::Symmetric);
    let selected = AuthoringOperand::selected;
    let mut authoring = AuthoringState::default();
    assert!(matches!(
        authoring.activate(coordinator.session().design_document(), tool, &[]),
        AuthoringOutcome::ModeEntered {
            expected,
            ..
        } if expected == [AuthoringOperandKind::Point]
    ));
    assert!(matches!(
        authoring.pick(
            coordinator.session().design_document(),
            selected(SelectionItem::Point(first)),
        ),
        AuthoringOutcome::Collecting {
            expected,
            ..
        } if expected == [AuthoringOperandKind::Point]
    ));
    assert!(matches!(
        authoring.pick(
            coordinator.session().design_document(),
            selected(SelectionItem::Point(first)),
        ),
        AuthoringOutcome::Warning(AuthoringWarning {
            reason: DisabledReason::SameSemanticOperand,
            ..
        })
    ));
    assert_eq!(authoring.pending().len(), 1);
    assert!(matches!(
        authoring.pick(
            coordinator.session().design_document(),
            selected(SelectionItem::Point(second)),
        ),
        AuthoringOutcome::Collecting {
            expected,
            ..
        } if expected == [AuthoringOperandKind::Line, AuthoringOperandKind::DatumAxis]
    ));
    let history_before_rejections = (coordinator.history_len(), coordinator.history_cursor());
    let payload_before_rejections = coordinator.checkpoint().design_json().to_owned();
    assert!(matches!(
        authoring.pick(
            coordinator.session().design_document(),
            selected(SelectionItem::Datum(SketchDatum::Origin)),
        ),
        AuthoringOutcome::Warning(AuthoringWarning {
            reason: DisabledReason::WrongOperandKind,
            expected,
            ..
        }) if expected == [AuthoringOperandKind::Line, AuthoringOperandKind::DatumAxis]
    ));
    assert_eq!(authoring.pending().len(), 2);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history_before_rejections
    );
    assert_eq!(
        coordinator.checkpoint().design_json(),
        payload_before_rejections
    );

    let AuthoringOutcome::Apply(application) = authoring.pick(
        coordinator.session().design_document(),
        selected(SelectionItem::Datum(SketchDatum::XAxis)),
    ) else {
        panic!("X axis must complete active symmetry authoring")
    };
    assert_eq!(
        application.resolved_constraint,
        Some(ResolvedConstraintKind::SymmetricAboutDatumAxis)
    );
    let AuthoringMutation::Constraint(outcome) = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .expect("axis symmetry")
    else {
        panic!("axis symmetry must create a constraint")
    };
    assert!(outcome.published_accepted.is_some());
    let constraint = outcome.value;
    authoring.transaction_finished();
    assert!(authoring.pending().is_empty());
    assert_eq!(authoring.active_tool(), Some(tool));

    let saved = coordinator
        .persistence_checkpoint()
        .expect("draft-v5 checkpoint");
    assert!(saved.design_uses_draft_v5());
    assert!(saved.accepted_uses_draft_v5());
    assert!(saved.accepted_belongs_to_current_design());
    let accepted = coordinator
        .session()
        .accepted_state()
        .expect("accepted symmetry");
    let scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        coordinator.session().design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.5,
    )
    .expect("symmetry scene");
    let expected_operands = vec![
        SelectionItem::Point(first),
        SelectionItem::Point(second),
        SelectionItem::Datum(SketchDatum::XAxis),
    ];
    let entry = scene
        .constraint_entries
        .iter()
        .find(|entry| entry.id == constraint)
        .expect("constraint entry");
    assert_eq!(entry.glyph, SceneConstraintGlyph::Symmetry);
    assert_eq!(entry.operands, expected_operands);
    let annotation = scene
        .annotations
        .iter()
        .find(|annotation| annotation.item == SelectionItem::Constraint(constraint))
        .expect("constraint annotation");
    assert_eq!(
        annotation.kind,
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Symmetry)
    );
    assert_eq!(annotation.operands, expected_operands);
    assert!(annotation.is_visible(&[SelectionItem::Datum(SketchDatum::XAxis)], None, &[]));
    let SceneAnnotationGeometry::Glyph { markers } = &annotation.geometry else {
        panic!("symmetry must publish glyph geometry")
    };
    assert_eq!(markers.len(), 1);
    let first_screen = scene
        .points
        .iter()
        .find(|point| point.id == first)
        .unwrap()
        .screen_position;
    let second_screen = scene
        .points
        .iter()
        .find(|point| point.id == second)
        .unwrap()
        .screen_position;
    let semantic_anchor = ScreenPoint {
        x: 0.5 * (first_screen.x + second_screen.x),
        y: 0.5 * (first_screen.y + second_screen.y),
    };
    assert_eq!(markers[0].leader_from, Some(semantic_anchor));
    assert_eq!(
        markers[0].anchor,
        ScreenPoint {
            x: semantic_anchor.x + 24.0,
            y: semantic_anchor.y - 24.0,
        },
        "M76 keeps the exact paired-point midpoint as the leader origin while placing the movable mark clear of its operands"
    );

    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Constraint(constraint)]);
    coordinator
        .set_selected_suppressed(coordinator.session().design_identity(), true)
        .expect("suppress symmetry");
    assert!(
        coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .unwrap()
            .suppressed
    );
    coordinator.undo().expect("undo suppression");
    assert!(
        !coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .unwrap()
            .suppressed
    );
    coordinator.redo().expect("redo suppression");
    coordinator.undo().expect("restore active symmetry");

    coordinator
        .delete_selected(coordinator.session().design_identity())
        .expect("delete symmetry");
    assert!(
        coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .is_none()
    );
    coordinator.undo().expect("undo delete");
    assert!(
        coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .is_some()
    );
    coordinator.redo().expect("redo delete");
    coordinator
        .reload(&saved)
        .expect("reload draft-v5 symmetry");
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .unwrap()
            .definition,
        DocumentConstraintDefinition::SymmetricAboutDatumAxis {
            axis: DocumentCoordinateAxis::X,
            ..
        }
    ));
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
                curve_roles: Vec::new(),
                relations: vec![ConstructionRelationDefinition::auto_inference(
                    InferredRelation::CoincidentWithOrigin {
                        point: DraftPointSlot::Created { point_index: 0 },
                    },
                )],
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
                curve_roles: vec![GeometryRole::Profile],
                relations: vec![
                    ConstructionRelationDefinition::auto_inference(
                        InferredRelation::PointOnDatumAxis {
                            point: DraftPointSlot::Created { point_index: 1 },
                            axis: DocumentCoordinateAxis::X,
                        },
                    ),
                    ConstructionRelationDefinition::auto_inference(InferredRelation::Vertical {
                        line: DraftSpanSlot::Created {
                            curve_index: 0,
                            segment: 0,
                        },
                    }),
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
