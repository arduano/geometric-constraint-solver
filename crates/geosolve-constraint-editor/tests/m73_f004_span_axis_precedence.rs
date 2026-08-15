// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionPoint, ConstructionProposal, DraftInferenceCandidate, DraftInferenceFamily,
    DraftInferenceRelation, DraftInferenceStatus, EditorEffect, EditorMutation, EditorScene,
    EditorTool, InferredRelation, Modifiers, PointerInput, RetainedEditorCoordinator, ScreenPoint,
    Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition, DocumentId,
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

struct Fixture {
    coordinator: RetainedEditorCoordinator,
    scene: EditorScene,
    start: DesignPointId,
    horizontal_midpoint: CurveSpan,
    vertical_midpoint: CurveSpan,
}

fn fixture() -> Fixture {
    let mut document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(
            0x7300_0004_0000_0000_0000_0000_0000_0001,
        )),
    )
    .expect("document");
    let start = document.add_point("line start", [0.0, 0.0]).expect("start");
    let horizontal_midpoint = add_support(
        &mut document,
        "horizontal midpoint guide",
        [-6.0, -2.0],
        [-2.0, 2.0],
    );
    let vertical_midpoint = add_support(
        &mut document,
        "vertical midpoint guide",
        [2.0, -6.0],
        [6.0, -2.0],
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
    coordinator.editor_mut().activate_tool(EditorTool::Line);
    Fixture {
        coordinator,
        scene,
        start,
        horizontal_midpoint,
        vertical_midpoint,
    }
}

fn add_support(
    document: &mut SketchDocument,
    label: &str,
    start_position: [f64; 2],
    end_position: [f64; 2],
) -> CurveSpan {
    let branch = [
        end_position[0] - start_position[0],
        end_position[1] - start_position[1],
    ];
    let branch_norm = f64::hypot(branch[0], branch[1]);
    let start = document
        .add_point(format!("{label} start"), start_position)
        .expect("support start");
    let end = document
        .add_point(format!("{label} end"), end_position)
        .expect("support end");
    CurveSpan::line(
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [branch[0] / branch_norm, branch[1] / branch_norm],
                },
            )
            .expect("support"),
    )
}

fn resolution(
    coordinator: &RetainedEditorCoordinator,
) -> &geosolve_constraint_editor::DraftInferenceResolution {
    coordinator
        .editor()
        .draft_inference_resolution()
        .expect("inference resolution")
}

fn resolved_candidate(coordinator: &RetainedEditorCoordinator) -> &DraftInferenceCandidate {
    let resolution = resolution(coordinator);
    let DraftInferenceStatus::Resolved { candidate } = resolution.status else {
        panic!("expected resolved candidate: {resolution:#?}");
    };
    resolution
        .candidates
        .iter()
        .find(|value| value.id == candidate)
        .expect("resolved candidate value")
}

fn begin_at_existing_start(fixture: &mut Fixture, pointer_id: u64) {
    let start = fixture.scene.viewport.model_to_screen([0.0, 0.0]);
    fixture
        .coordinator
        .editor_mut()
        .pointer_move(&fixture.scene, pointer(pointer_id, start));
    assert!(matches!(
        resolved_candidate(&fixture.coordinator).relations.as_slice(),
        [DraftInferenceRelation::PointIdentity { point }] if *point == fixture.start
    ));
    let effects = fixture
        .coordinator
        .pointer_down(&fixture.scene, pointer(pointer_id, start));
    assert!(
        effects
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
    );
}

#[test]
fn m73_f004_line_started_at_existing_point_keeps_only_its_horizontal_relation() {
    let mut fixture = fixture();
    let pointer_id = 91;
    let baseline_history = fixture.coordinator.history_len();
    begin_at_existing_start(&mut fixture, pointer_id);

    let raw = fixture.scene.viewport.model_to_screen([4.0, 0.04]);
    fixture
        .coordinator
        .editor_mut()
        .pointer_move(&fixture.scene, pointer(pointer_id, raw));
    let candidate = resolved_candidate(&fixture.coordinator);
    assert_eq!(
        candidate.relations,
        vec![DraftInferenceRelation::Horizontal]
    );
    assert_eq!(
        candidate.adjusted_model_position.map(f64::to_bits),
        [4.0, 0.0].map(f64::to_bits)
    );

    let effects = fixture
        .coordinator
        .pointer_down(&fixture.scene, pointer(pointer_id, raw));
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("horizontal construction plan");
    let EditorEffect::CommitConstructionPlan { plan, .. } = commit else {
        unreachable!()
    };
    assert!(matches!(
        plan.proposal,
        ConstructionProposal::Line {
            start: ConstructionPoint::Existing { id, .. },
            end: ConstructionPoint::New(end),
        } if id == fixture.start && end.map(f64::to_bits) == [4.0, 0.0].map(f64::to_bits)
    ));
    assert!(matches!(
        plan.relations.as_slice(),
        [InferredRelation::Horizontal { .. }]
    ));

    let outcome = fixture
        .coordinator
        .apply_editor_effect(commit)
        .expect("coordinator publication")
        .expect("retained mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("expected inferred construction")
    };
    assert_eq!(result.constraints.len(), 1);
    assert_eq!(fixture.coordinator.history_len(), baseline_history + 1);
    let line = CurveSpan::line(result.construction.curves[0]);
    assert!(matches!(
        fixture
            .coordinator
            .session()
            .design_document()
            .constraint(result.constraints[0].constraint)
            .expect("horizontal constraint")
            .definition,
        DocumentConstraintDefinition::Horizontal { line: actual } if actual == line
    ));
    let accepted = fixture
        .coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted horizontal line");
    let CurveDefinition::Line { start, end, .. } = accepted
        .document()
        .curve(line.curve)
        .expect("line")
        .definition
    else {
        unreachable!()
    };
    let start = accepted.document().point(start).expect("start").position;
    let end = accepted.document().point(end).expect("end").position;
    assert!(start.into_iter().chain(end).all(f64::is_finite));
    assert!((start[1] - end[1]).abs() <= 1.0e-9);
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );
}

#[test]
fn m73_f004_horizontal_span_suppresses_same_axis_native_midpoint_guide() {
    let mut fixture = fixture();
    let pointer_id = 92;
    begin_at_existing_start(&mut fixture, pointer_id);

    let midpoint = fixture.scene.viewport.model_to_screen([-4.0, 0.0]);
    fixture
        .coordinator
        .editor_mut()
        .pointer_move(&fixture.scene, pointer(pointer_id, midpoint));
    let wake = resolution(&fixture.coordinator);
    assert!(
        resolved_candidate(&fixture.coordinator)
            .relations
            .iter()
            .any(|relation| matches!(
                relation,
                DraftInferenceRelation::Midpoint { span }
                    if *span == fixture.horizontal_midpoint
            )),
        "midpoint wake resolution: {wake:#?}"
    );

    let raw = fixture.scene.viewport.model_to_screen([4.0, 0.04]);
    fixture
        .coordinator
        .editor_mut()
        .pointer_move(&fixture.scene, pointer(pointer_id, raw));
    assert_eq!(resolution(&fixture.coordinator).candidates.len(), 1);
    assert!(matches!(
        resolution(&fixture.coordinator).guides.as_slice(),
        [guide]
            if guide.family == DraftInferenceFamily::Horizontal
                && guide.reference.is_none()
    ));
    let candidate = resolved_candidate(&fixture.coordinator);
    assert_eq!(
        candidate.relations,
        vec![DraftInferenceRelation::Horizontal]
    );
    assert!(candidate.references.iter().all(|reference| {
        !matches!(reference, geosolve_constraint_editor::DraftReferenceAnchor::Midpoint { span, .. }
            if *span == fixture.horizontal_midpoint)
    }));
    assert!(candidate.guides.iter().all(|guide| {
        !matches!(
            guide.family,
            DraftInferenceFamily::HorizontalPoints
                | DraftInferenceFamily::HorizontalPointToMidpoint
        )
    }));
}

#[test]
fn m73_f004_horizontal_span_still_composes_with_orthogonal_native_midpoint_guide() {
    let mut fixture = fixture();
    let pointer_id = 93;
    begin_at_existing_start(&mut fixture, pointer_id);

    let midpoint = fixture.scene.viewport.model_to_screen([4.0, -4.0]);
    fixture
        .coordinator
        .editor_mut()
        .pointer_move(&fixture.scene, pointer(pointer_id, midpoint));

    let raw = fixture.scene.viewport.model_to_screen([4.04, 0.04]);
    fixture
        .coordinator
        .editor_mut()
        .pointer_move(&fixture.scene, pointer(pointer_id, raw));
    let candidate = resolved_candidate(&fixture.coordinator);
    assert_eq!(
        candidate.adjusted_model_position.map(f64::to_bits),
        [4.0, 0.0].map(f64::to_bits)
    );
    assert_eq!(
        candidate.relations,
        vec![
            DraftInferenceRelation::VerticalPointToMidpoint {
                reference: fixture.vertical_midpoint,
            },
            DraftInferenceRelation::Horizontal,
        ]
    );
}
