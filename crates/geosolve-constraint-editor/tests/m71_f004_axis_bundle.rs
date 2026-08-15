// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionPoint, ConstructionProposal, DraftInferenceCandidate, DraftInferenceFamily,
    DraftInferenceRelation, DraftInferenceStatus, DraftPointSlot, DraftSpanSlot, EditorEffect,
    EditorMutation, EditorScene, EditorTool, GeometryVisibility, InferredRelation, Modifiers,
    PointerInput, RetainedEditorCoordinator, ScreenPoint, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentEdit, DocumentId,
    DocumentSolveRequest, PersistentId, RetainedSketchDocumentSession, SketchDocument,
    SolverConfig,
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
) {
    let mut document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(
            0x7100_0004_0000_0000_0000_0000_0000_0001,
        )),
    )
    .expect("document");
    let reference = document
        .add_point("side reference", [-4.0, 4.0])
        .expect("side reference");
    document
        .add_constraint(
            "fix side reference",
            DocumentConstraintDefinition::FixedPoint {
                point: reference,
                target: [-4.0, 4.0],
            },
        )
        .expect("fixed side reference");
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
    let _ = coordinator
        .editor_mut()
        .set_geometry_visibility(GeometryVisibility {
            reference_geometry: false,
            ..GeometryVisibility::default()
        });
    coordinator.editor_mut().activate_tool(EditorTool::Line);
    (coordinator, scene, reference)
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
    reason = "one end-to-end regression keeps preview, plan, accepted residual, history, and later-edit evidence together"
)]
fn m71_f004_vertical_line_and_horizontal_point_axis_commit_as_one_bundle() {
    let (mut coordinator, scene, reference) = fixture();
    let baseline_history = coordinator.history_len();
    let pointer_id = 75;

    let start = scene.viewport.model_to_screen([0.0, 0.0]);
    let prefix = coordinator.pointer_down(&scene, pointer(pointer_id, start));
    assert!(
        prefix
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
    );

    let side_reference = scene.viewport.model_to_screen([-4.0, 4.0]);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, side_reference));
    assert!(matches!(
        resolved_candidate(&coordinator).relations.as_slice(),
        [DraftInferenceRelation::PointIdentity { point }] if *point == reference
    ));

    let raw = scene.viewport.model_to_screen([0.04, 4.05]);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, raw));
    let candidate = resolved_candidate(&coordinator);
    assert_eq!(
        candidate.adjusted_model_position.map(f64::to_bits),
        [0.0, 4.0].map(f64::to_bits)
    );
    assert_eq!(
        candidate.relations,
        vec![
            DraftInferenceRelation::HorizontalPoints { reference },
            DraftInferenceRelation::Vertical,
        ]
    );
    assert!(
        candidate
            .guides
            .iter()
            .any(|guide| guide.family == DraftInferenceFamily::HorizontalPoints)
    );
    assert!(
        candidate
            .guides
            .iter()
            .any(|guide| guide.family == DraftInferenceFamily::Vertical)
    );

    let effects = coordinator.pointer_down(&scene, pointer(pointer_id, raw));
    let plans = effects
        .iter()
        .filter(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .collect::<Vec<_>>();
    assert_eq!(plans.len(), 1);
    let EditorEffect::CommitConstructionPlan { plan, .. } = plans[0] else {
        unreachable!()
    };
    assert!(matches!(
        plan.proposal,
        ConstructionProposal::Line {
            start: ConstructionPoint::New(start),
            end: ConstructionPoint::New(end),
        } if start.map(f64::to_bits) == [0.0, 0.0].map(f64::to_bits)
            && end.map(f64::to_bits) == [0.0, 4.0].map(f64::to_bits)
    ));
    assert!(matches!(
        plan.relations.as_slice(),
        [
            InferredRelation::HorizontalPoints {
                first: DraftPointSlot::Created { point_index: 1 },
                second: DraftPointSlot::Existing(actual_reference),
            },
            InferredRelation::Vertical {
                line: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                },
            },
        ] if *actual_reference == reference
    ));

    let outcome = coordinator
        .apply_editor_effect(plans[0])
        .expect("coordinator publication")
        .expect("retained mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("axis bundle must publish inferred construction")
    };
    assert_eq!(result.construction.points.len(), 2);
    assert_eq!(result.construction.curves.len(), 1);
    assert_eq!(result.constraints.len(), 2);
    assert_eq!(coordinator.history_len(), baseline_history + 1);

    let [start_point, end_point] = result.construction.points.as_slice() else {
        unreachable!()
    };
    let line = CurveSpan::line(result.construction.curves[0]);
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(result.constraints[0].constraint)
            .expect("horizontal-points constraint")
            .definition,
        DocumentConstraintDefinition::HorizontalPoints { first, second }
            if first == *end_point && second == reference
    ));
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(result.constraints[1].constraint)
            .expect("vertical constraint")
            .definition,
        DocumentConstraintDefinition::Vertical { line: actual } if actual == line
    ));

    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted axis bundle");
    let document = accepted.document();
    let a = document
        .point(*start_point)
        .expect("accepted start")
        .position;
    let b = document
        .point(*end_point)
        .expect("accepted endpoint")
        .position;
    let r = document
        .point(reference)
        .expect("accepted reference")
        .position;
    assert!(a.into_iter().chain(b).chain(r).all(f64::is_finite));
    assert!((a[0] - b[0]).abs() <= 1.0e-9);
    assert!((b[1] - r[1]).abs() <= 1.0e-9);
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );

    let expected = coordinator.session().design_identity();
    let edit = coordinator
        .apply_edit(
            expected,
            DocumentEdit::SetPointPosition {
                point: *start_point,
                position: [2.0, 0.0],
            },
        )
        .expect("compatible later edit");
    assert!(edit.published_accepted.is_some());
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted later edit");
    let document = accepted.document();
    let CurveDefinition::Line {
        start: actual_start,
        end: actual_end,
        ..
    } = document
        .curve(line.curve)
        .expect("retained line")
        .definition
    else {
        unreachable!()
    };
    let a = document.point(actual_start).expect("edited start").position;
    let b = document
        .point(actual_end)
        .expect("tracked endpoint")
        .position;
    let r = document.point(reference).expect("fixed reference").position;
    assert!((a[0] - b[0]).abs() <= 1.0e-9);
    assert!((b[1] - r[1]).abs() <= 1.0e-9);
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );
}

#[test]
fn m71_f004_polyline_horizontal_span_and_vertical_point_axis_share_one_segment() {
    let (mut coordinator, scene, reference) = fixture();
    coordinator.editor_mut().activate_tool(EditorTool::Polyline);
    let pointer_id = 76;

    let start = scene.viewport.model_to_screen([0.0, 0.0]);
    coordinator.pointer_down(&scene, pointer(pointer_id, start));
    let side_reference = scene.viewport.model_to_screen([-4.0, 4.0]);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, side_reference));
    assert!(matches!(
        resolved_candidate(&coordinator).relations.as_slice(),
        [DraftInferenceRelation::PointIdentity { point }] if *point == reference
    ));

    let raw = scene.viewport.model_to_screen([-3.95, 0.04]);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, raw));
    let candidate = resolved_candidate(&coordinator);
    assert_eq!(
        candidate.adjusted_model_position.map(f64::to_bits),
        [-4.0, 0.0].map(f64::to_bits)
    );
    assert_eq!(
        candidate.relations,
        vec![
            DraftInferenceRelation::VerticalPoints { reference },
            DraftInferenceRelation::Horizontal,
        ]
    );
    let stage = coordinator.pointer_down(&scene, pointer(pointer_id, raw));
    assert!(
        stage
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
    );

    let effects = coordinator
        .editor_mut()
        .complete_draft(scene.design_identity);
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("polyline axis bundle plan");
    let EditorEffect::CommitConstructionPlan { plan, .. } = commit else {
        unreachable!()
    };
    assert!(matches!(
        plan.relations.as_slice(),
        [
            InferredRelation::VerticalPoints {
                first: DraftPointSlot::Created { point_index: 1 },
                second: DraftPointSlot::Existing(actual_reference),
            },
            InferredRelation::Horizontal {
                line: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                },
            },
        ] if *actual_reference == reference
    ));
    let outcome = coordinator
        .apply_editor_effect(commit)
        .expect("coordinator publication")
        .expect("retained polyline mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("polyline axis bundle must publish inferred construction")
    };
    assert_eq!(result.constraints.len(), 2);
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted polyline axis bundle");
    let points = &result.construction.points;
    let a = accepted
        .document()
        .point(points[0])
        .expect("accepted polyline start")
        .position;
    let b = accepted
        .document()
        .point(points[1])
        .expect("accepted polyline end")
        .position;
    let r = accepted
        .document()
        .point(reference)
        .expect("accepted reference")
        .position;
    assert!((a[1] - b[1]).abs() <= 1.0e-9);
    assert!((b[0] - r[0]).abs() <= 1.0e-9);
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );
}
