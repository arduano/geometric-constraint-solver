// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    DraftGuideClassification, DraftInferenceCandidate, DraftInferenceFamily,
    DraftInferenceRelation, DraftInferenceStatus, EditorEffect, EditorMutation, EditorScene,
    EditorTool, GeometryVisibility, Modifiers, PointerInput, RetainedEditorCoordinator,
    ScreenPoint, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition, DocumentEdit,
    DocumentId, DocumentSolveRequest, PersistentId, RetainedSketchDocumentSession, SketchDocument,
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
    CurveSpan,
    [DesignPointId; 2],
) {
    let mut document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(
            0x7100_0003_0000_0000_0000_0000_0000_0001,
        )),
    )
    .expect("document");
    let start = document
        .add_point("support start", [-4.0, 1.0])
        .expect("support start");
    let end = document
        .add_point("support end", [4.0, 1.0])
        .expect("support end");
    let support = CurveSpan::line(
        document
            .add_curve(
                "native support",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("native line"),
    );
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
    coordinator.editor_mut().activate_tool(EditorTool::Point);
    (coordinator, scene, support, [start, end])
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

fn publish_midpoint_axis(
    pointer_id: u64,
    raw: [f64; 2],
) -> (
    RetainedEditorCoordinator,
    CurveSpan,
    [DesignPointId; 2],
    DesignPointId,
    geosolve_sketch::DocumentConstraintId,
) {
    let (mut coordinator, scene, support, endpoints) = fixture();
    let baseline_history = coordinator.history_len();
    let midpoint = scene.viewport.model_to_screen([0.0, 1.0]);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, midpoint));
    assert!(matches!(
        resolved_candidate(&coordinator).relations.as_slice(),
        [DraftInferenceRelation::Midpoint { span }] if *span == support
    ));

    let target = scene.viewport.model_to_screen(raw);
    coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, target));
    let candidate = resolved_candidate(&coordinator);
    let horizontal = (raw[1] - 1.0).abs() < (raw[0] - 0.0).abs();
    if horizontal {
        assert!(matches!(
            candidate.relations.as_slice(),
            [DraftInferenceRelation::HorizontalPointToMidpoint { reference }]
                if *reference == support
        ));
        assert_eq!(
            candidate.adjusted_model_position.map(f64::to_bits),
            [raw[0], 1.0].map(f64::to_bits)
        );
        assert!(candidate.guides.iter().any(|guide| {
            guide.classification == DraftGuideClassification::ConstraintBacked
                && guide.family == DraftInferenceFamily::HorizontalPointToMidpoint
        }));
    } else {
        assert!(
            matches!(
                candidate.relations.as_slice(),
                [DraftInferenceRelation::VerticalPointToMidpoint { reference }]
                    if *reference == support
            ),
            "unexpected vertical midpoint candidate: {candidate:#?}"
        );
        assert_eq!(
            candidate.adjusted_model_position.map(f64::to_bits),
            [0.0, raw[1]].map(f64::to_bits)
        );
        assert!(candidate.guides.iter().any(|guide| {
            guide.classification == DraftGuideClassification::ConstraintBacked
                && guide.family == DraftInferenceFamily::VerticalPointToMidpoint
        }));
    }

    let effects = coordinator.pointer_down(&scene, pointer(pointer_id, target));
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("midpoint axis must emit one atomic construction plan");
    let outcome = coordinator
        .apply_editor_effect(commit)
        .expect("coordinator publication")
        .expect("retained mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("midpoint axis must publish inferred construction")
    };
    assert_eq!(result.construction.points.len(), 1);
    assert_eq!(result.constraints.len(), 1);
    assert_eq!(coordinator.history_len(), baseline_history + 1);
    (
        coordinator,
        support,
        endpoints,
        result.construction.points[0],
        result.constraints[0].constraint,
    )
}

#[test]
fn m71_f003_horizontal_midpoint_axis_is_atomic_and_tracks_later_parent_edits() {
    let (mut coordinator, support, [start, end], point, constraint) =
        publish_midpoint_axis(73, [6.0, 1.05]);
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .expect("retained midpoint axis")
            .definition,
        DocumentConstraintDefinition::HorizontalPointToMidpoint { point: actual, line }
            if actual == point && line == support
    ));
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted midpoint axis");
    let solved = accepted
        .document()
        .point(point)
        .expect("accepted point")
        .position;
    assert!((solved[1] - 1.0).abs() <= 1.0e-9);
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );

    for (parent, position) in [(start, [-6.0, 3.0]), (end, [10.0, 5.0])] {
        let expected = coordinator.session().design_identity();
        let outcome = coordinator
            .apply_edit(
                expected,
                DocumentEdit::SetPointPosition {
                    point: parent,
                    position,
                },
            )
            .expect("accepted parent edit");
        assert!(outcome.published_accepted.is_some());
    }
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted moved support");
    let document = accepted.document();
    let p = document.point(point).expect("tracked point").position;
    let a = document.point(start).expect("moved start").position;
    let b = document.point(end).expect("moved end").position;
    assert!((p[1] - 0.5 * (a[1] + b[1])).abs() <= 1.0e-9);
}

#[test]
fn m71_f003_vertical_midpoint_axis_is_constraint_backed() {
    let (coordinator, support, _endpoints, point, constraint) =
        publish_midpoint_axis(74, [0.05, 6.0]);
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .expect("retained midpoint axis")
            .definition,
        DocumentConstraintDefinition::VerticalPointToMidpoint { point: actual, line }
            if actual == point && line == support
    ));
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted midpoint axis");
    let document = accepted.document();
    let p = document.point(point).expect("accepted point").position;
    let CurveDefinition::Line { start, end, .. } =
        document.curve(support.curve).expect("support").definition
    else {
        unreachable!()
    };
    let a = document.point(start).expect("start").position;
    let b = document.point(end).expect("end").position;
    assert!((p[0] - 0.5 * (a[0] + b[0])).abs() <= 1.0e-9);
}
