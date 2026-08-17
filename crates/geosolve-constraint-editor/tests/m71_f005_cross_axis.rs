// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionPoint, ConstructionProposal, DraftInferenceCandidate, DraftInferenceFamily,
    DraftInferenceRelation, DraftInferenceStatus, DraftPointSlot, EditorEffect, EditorMutation,
    EditorScene, EditorTool, GeometryVisibility, InferredRelation, Modifiers, PointerInput,
    RetainedEditorCoordinator, ScreenPoint, Viewport,
};
use geosolve_sketch::{
    DocumentConstraintDefinition, DocumentEdit, DocumentId, DocumentSolveRequest, PersistentId,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn fixture() -> (
    RetainedEditorCoordinator,
    EditorScene,
    geosolve_sketch::DesignPointId,
    geosolve_sketch::DesignPointId,
) {
    let mut document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(
            0x7100_0005_0000_0000_0000_0000_0000_0001,
        )),
    )
    .expect("document");
    let horizontal_reference = document
        .add_point("horizontal reference", [-4.0, 4.0])
        .expect("horizontal reference");
    let vertical_reference = document
        .add_point("vertical reference", [3.0, -4.0])
        .expect("vertical reference");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained session");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted fixture");
    let scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        session.design_document(),
        Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
        0.5,
    )
    .expect("scene")
    .with_retained_session(&session)
    .expect("authenticated scene");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator
        .editor_mut()
        .set_geometry_visibility(GeometryVisibility {
            reference_geometry: false,
            ..GeometryVisibility::default()
        });
    coordinator.editor_mut().activate_tool(EditorTool::Line);
    (coordinator, scene, horizontal_reference, vertical_reference)
}

fn resolved_candidate(coordinator: &RetainedEditorCoordinator) -> &DraftInferenceCandidate {
    let resolution = coordinator
        .editor()
        .draft_inference_resolution()
        .expect("inference resolution");
    let DraftInferenceStatus::Resolved { candidate } = resolution.status else {
        panic!("expected resolved candidate: {resolution:#?}");
    };
    resolution
        .candidates
        .iter()
        .find(|value| value.id == candidate)
        .expect("resolved candidate value")
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one public regression keeps preview, atomic plan, independent residual, history, and later-edit evidence together"
)]
fn m71_f005_distinct_horizontal_and_vertical_point_axes_commit_one_intersection() {
    let (mut coordinator, scene, horizontal_reference, vertical_reference) = fixture();
    let baseline_history = coordinator.history_len();
    let pointer_id = 77;

    let start = scene.viewport.model_to_screen([0.0, 0.0]);
    coordinator.pointer_down(&scene, pointer(pointer_id, start));

    for (position, expected) in [
        ([-4.0, 4.0], horizontal_reference),
        ([3.0, -4.0], vertical_reference),
    ] {
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(pointer_id, scene.viewport.model_to_screen(position)),
        );
        assert!(matches!(
            resolved_candidate(&coordinator).relations.as_slice(),
            [DraftInferenceRelation::PointIdentity { point }] if *point == expected
        ));
    }

    let raw = scene.viewport.model_to_screen([3.04, 4.05]);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, raw));
    let candidate = resolved_candidate(&coordinator);
    assert_eq!(
        candidate.adjusted_model_position.map(f64::to_bits),
        [3.0, 4.0].map(f64::to_bits)
    );
    assert_eq!(
        candidate.relations,
        vec![
            DraftInferenceRelation::HorizontalPoints {
                reference: horizontal_reference,
            },
            DraftInferenceRelation::VerticalPoints {
                reference: vertical_reference,
            },
        ]
    );
    assert_eq!(candidate.ranking.persistent_relation_count, 2);
    assert_eq!(candidate.guides.len(), 2);
    assert!(candidate.guides.iter().all(|guide| {
        guide.classification
            == geosolve_constraint_editor::DraftGuideClassification::ConstraintBacked
            && matches!(
                guide.family,
                DraftInferenceFamily::HorizontalPoints | DraftInferenceFamily::VerticalPoints
            )
    }));

    let effects = coordinator.pointer_down(&scene, pointer(pointer_id, raw));
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("cross-axis construction plan");
    let EditorEffect::CommitConstructionPlan { plan, .. } = commit else {
        unreachable!()
    };
    assert!(matches!(
        plan.proposal,
        ConstructionProposal::Line {
            start: ConstructionPoint::New(start),
            end: ConstructionPoint::New(end),
        } if start.map(f64::to_bits) == [0.0, 0.0].map(f64::to_bits)
            && end.map(f64::to_bits) == [3.0, 4.0].map(f64::to_bits)
    ));
    assert!(matches!(
        plan.relation_payloads().as_slice(),
        [
            InferredRelation::HorizontalPoints {
                first: DraftPointSlot::Created { point_index: 1 },
                second: DraftPointSlot::Existing(horizontal),
            },
            InferredRelation::VerticalPoints {
                first: DraftPointSlot::Created { point_index: 1 },
                second: DraftPointSlot::Existing(vertical),
            },
        ] if *horizontal == horizontal_reference && *vertical == vertical_reference
    ));

    let outcome = coordinator
        .apply_editor_effect(commit)
        .expect("coordinator publication")
        .expect("retained mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("cross-axis bundle must publish inferred construction")
    };
    assert_eq!(result.constraints.len(), 2);
    assert_eq!(coordinator.history_len(), baseline_history + 1);

    let endpoint = result.construction.points[1];
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(result.constraints[0].constraint)
            .expect("retained horizontal-points constraint")
            .definition,
        DocumentConstraintDefinition::HorizontalPoints { first, second }
            if first == endpoint && second == horizontal_reference
    ));
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(result.constraints[1].constraint)
            .expect("retained vertical-points constraint")
            .definition,
        DocumentConstraintDefinition::VerticalPoints { first, second }
            if first == endpoint && second == vertical_reference
    ));
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted cross-axis bundle");
    let document = accepted.document();
    let placed = document
        .point(endpoint)
        .expect("accepted endpoint")
        .position;
    let horizontal = document
        .point(horizontal_reference)
        .expect("accepted horizontal reference")
        .position;
    let vertical = document
        .point(vertical_reference)
        .expect("accepted vertical reference")
        .position;
    assert!(
        placed
            .into_iter()
            .chain(horizontal)
            .chain(vertical)
            .all(f64::is_finite)
    );
    assert!((placed[1] - horizontal[1]).abs() <= 1.0e-9);
    assert!((placed[0] - vertical[0]).abs() <= 1.0e-9);
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );

    let mut previous_endpoint = placed;
    for (reference, position, changed_axis) in [
        (horizontal_reference, [-4.0, 5.0], 1),
        (vertical_reference, [2.0, -4.0], 0),
    ] {
        let expected = coordinator.session().design_identity();
        let edit = coordinator
            .apply_edit(
                expected,
                DocumentEdit::SetPointPosition {
                    point: reference,
                    position,
                },
            )
            .expect("compatible source-reference edit");
        assert!(edit.published_accepted.is_some());
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted source-reference edit");
        let document = accepted.document();
        let placed = document.point(endpoint).expect("tracked endpoint").position;
        let horizontal = document
            .point(horizontal_reference)
            .expect("edited horizontal reference")
            .position;
        let vertical = document
            .point(vertical_reference)
            .expect("edited vertical reference")
            .position;
        assert!(
            placed
                .into_iter()
                .chain(horizontal)
                .chain(vertical)
                .all(f64::is_finite)
        );
        assert!((placed[1] - horizontal[1]).abs() <= 1.0e-9);
        assert!((placed[0] - vertical[0]).abs() <= 1.0e-9);
        assert!(
            (placed[changed_axis] - previous_endpoint[changed_axis]).abs() > 1.0e-6,
            "editing each source reference must move the endpoint on that source's axis"
        );
        assert!(
            (placed[1 - changed_axis] - previous_endpoint[1 - changed_axis]).abs() <= 1.0e-9,
            "editing one source reference must preserve the independently constrained axis"
        );
        assert!(result.constraints.iter().all(|relation| {
            coordinator
                .session()
                .design_document()
                .constraint(relation.constraint)
                .is_some()
        }));
        assert!(
            accepted
                .solve_result()
                .acceptance_hard_residual_max
                .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
        );
        previous_endpoint = placed;
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one polyline regression keeps stage handoff, atomic plan, and independent accepted-state evidence together"
)]
fn m71_f005_polyline_distinct_point_axes_commit_one_intersection() {
    let (mut coordinator, scene, horizontal_reference, vertical_reference) = fixture();
    coordinator.editor_mut().activate_tool(EditorTool::Polyline);
    let pointer_id = 78;

    coordinator.pointer_down(
        &scene,
        pointer(pointer_id, scene.viewport.model_to_screen([0.0, 0.0])),
    );
    for position in [[-4.0, 4.0], [3.0, -4.0]] {
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(pointer_id, scene.viewport.model_to_screen(position)),
        );
    }

    let raw = scene.viewport.model_to_screen([3.04, 4.05]);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, raw));
    let candidate = resolved_candidate(&coordinator);
    assert_eq!(
        candidate.adjusted_model_position.map(f64::to_bits),
        [3.0, 4.0].map(f64::to_bits)
    );
    assert_eq!(
        candidate.relations,
        vec![
            DraftInferenceRelation::HorizontalPoints {
                reference: horizontal_reference,
            },
            DraftInferenceRelation::VerticalPoints {
                reference: vertical_reference,
            },
        ]
    );

    let stage = coordinator.pointer_down(&scene, pointer(pointer_id, raw));
    assert!(
        stage
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
    );
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, raw));
    assert_eq!(
        resolved_candidate(&coordinator).relations,
        vec![
            DraftInferenceRelation::HorizontalPoints {
                reference: horizontal_reference,
            },
            DraftInferenceRelation::VerticalPoints {
                reference: vertical_reference,
            },
        ],
        "both positional references survive the ordinary polyline-stage handoff"
    );

    let effects = coordinator
        .editor_mut()
        .complete_draft(scene.design_identity);
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("polyline cross-axis construction plan");
    let EditorEffect::CommitConstructionPlan { plan, .. } = commit else {
        unreachable!()
    };
    assert!(matches!(
        plan.relation_payloads().as_slice(),
        [
            InferredRelation::HorizontalPoints {
                first: DraftPointSlot::Created { point_index: 1 },
                second: DraftPointSlot::Existing(horizontal),
            },
            InferredRelation::VerticalPoints {
                first: DraftPointSlot::Created { point_index: 1 },
                second: DraftPointSlot::Existing(vertical),
            },
        ] if *horizontal == horizontal_reference && *vertical == vertical_reference
    ));

    let outcome = coordinator
        .apply_editor_effect(commit)
        .expect("coordinator publication")
        .expect("retained polyline mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("polyline cross-axis bundle must publish inferred construction")
    };
    assert_eq!(result.constraints.len(), 2);
    let endpoint = result.construction.points[1];
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted polyline cross-axis bundle");
    let document = accepted.document();
    let placed = document
        .point(endpoint)
        .expect("accepted endpoint")
        .position;
    assert!(placed.into_iter().all(f64::is_finite));
    assert!(
        (placed[1] - document.point(horizontal_reference).unwrap().position[1]).abs() <= 1.0e-9
    );
    assert!((placed[0] - document.point(vertical_reference).unwrap().position[0]).abs() <= 1.0e-9);
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );
}
