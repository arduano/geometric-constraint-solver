// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionRelationProvenance, CoordinatorError, DraftAuthoringInput, DraftInferenceInput,
    DraftInferenceRelation, DraftInferenceStatus, EditorEffect, EditorMutation, EditorScene,
    GeometryToolVariant, InferredRelation, Modifiers, PointerInput, ReplayAction,
    RetainedEditorCoordinator, Viewport,
};
use geosolve_sketch::{
    DocumentConstraintDefinition, DocumentSolveRequest, RetainedSketchDocumentSession,
    SketchDocument, SolverConfig,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const POINTER_ID: u64 = 0x7900;
const EPSILON: f64 = 1.0e-9;

fn authoring(
    preferred_candidate: Option<geosolve_constraint_editor::DraftInferenceCandidateId>,
) -> DraftAuthoringInput {
    DraftAuthoringInput {
        inference: DraftInferenceInput {
            suppressed: false,
            preferred_candidate,
        },
        regularized: false,
    }
}

fn scene(coordinator: &RetainedEditorCoordinator, viewport: Viewport) -> EditorScene {
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted state");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.25,
    )
    .expect("editor scene")
    .with_retained_session(coordinator.session())
    .expect("authenticated editor scene")
}

fn input(scene: &EditorScene, model_position: [f64; 2]) -> PointerInput {
    PointerInput {
        pointer_id: POINTER_ID,
        position: scene.viewport.model_to_screen(model_position),
        modifiers: Modifiers::default(),
    }
}

fn press(
    coordinator: &mut RetainedEditorCoordinator,
    scene: &EditorScene,
    model_position: [f64; 2],
    preferred_candidate: Option<geosolve_constraint_editor::DraftInferenceCandidateId>,
) -> Vec<EditorEffect> {
    coordinator.pointer_down_with_draft_authoring(
        scene,
        input(scene, model_position),
        authoring(preferred_candidate),
    )
}

fn publish_plan(
    coordinator: &mut RetainedEditorCoordinator,
    effects: &[EditorEffect],
) -> geosolve_constraint_editor::ConstructionCommitResult {
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("construction-plan effect");
    let token = match commit {
        EditorEffect::CommitConstructionPlan { token, .. } => *token,
        _ => unreachable!("filtered construction-plan effect"),
    };
    let outcome = coordinator
        .apply_editor_effect(commit)
        .expect("accepted atomic construction")
        .expect("construction mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("expected inferred construction result");
    };
    assert!(outcome.published_accepted.is_some());
    assert!(
        coordinator
            .acknowledge_construction_commit(token, true)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
    result
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "one public-boundary regression preserves the exact reported gesture, two wraps, publication, history and replay contract"
)]
fn m79_midpoint_line_default_keeps_anchor_and_omits_fully_redundant_direction() {
    let viewport = Viewport::new([1_000.0, 800.0], [0.0, 0.0], 50.0).expect("viewport");
    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted empty session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

    let initial_scene = scene(&coordinator, viewport);
    let _ = coordinator
        .editor_mut()
        .activate_geometry_tool(GeometryToolVariant::CenterRectangle);
    assert!(
        press(&mut coordinator, &initial_scene, [0.0, 0.0], None)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
    );
    let rectangle = press(&mut coordinator, &initial_scene, [2.0, 1.0], None);
    publish_plan(&mut coordinator, &rectangle);

    let rectangle_scene = scene(&coordinator, viewport);
    let replay_seed = coordinator.session().clone();
    let rectangle_document = coordinator
        .session()
        .design_document()
        .to_draft_v5_json()
        .expect("rectangle canonical document");
    let rectangle_history = (coordinator.history_len(), coordinator.history_cursor());
    let rectangle_transcript_len = coordinator.transcript().len();
    let rectangle_accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted rectangle")
        .identity();
    let _ = coordinator
        .editor_mut()
        .activate_geometry_tool(GeometryToolVariant::MidpointLine);
    let _ = press(&mut coordinator, &rectangle_scene, [0.0, 0.0], None);
    let hover = coordinator.editor_mut().pointer_move_with_draft_authoring(
        &rectangle_scene,
        input(&rectangle_scene, [2.0, 0.0]),
        authoring(None),
    );
    assert!(
        hover
            .iter()
            .any(|effect| matches!(effect, EditorEffect::DraftInferenceChanged(Some(_))))
    );
    let resolution = coordinator
        .editor()
        .draft_inference_resolution()
        .expect("midpoint candidate cohort");
    let DraftInferenceStatus::Resolved { candidate } = resolution.status else {
        panic!("expected ranked midpoint candidate: {resolution:?}");
    };
    let selected = resolution
        .candidates
        .iter()
        .find(|value| value.id == candidate)
        .expect("selected candidate");
    assert!(matches!(
        selected.relations.as_slice(),
        [
            DraftInferenceRelation::Midpoint { .. },
            DraftInferenceRelation::Horizontal
        ]
    ));

    assert_eq!(resolution.candidates.len(), 5);
    assert!(matches!(
        resolution.candidates[0].relations.as_slice(),
        [
            DraftInferenceRelation::Midpoint { .. },
            DraftInferenceRelation::Horizontal
        ]
    ));
    assert!(matches!(
        resolution.candidates[1].relations.as_slice(),
        [DraftInferenceRelation::Midpoint { .. }]
    ));
    assert!(matches!(
        resolution.candidates[2].relations.as_slice(),
        [
            DraftInferenceRelation::PointOnCurve { .. },
            DraftInferenceRelation::Horizontal
        ]
    ));
    assert!(matches!(
        resolution.candidates[3].relations.as_slice(),
        [DraftInferenceRelation::PointOnCurve { .. }]
    ));
    assert!(matches!(
        resolution.candidates[4].relations.as_slice(),
        [DraftInferenceRelation::Horizontal]
    ));

    let sealed_candidates = resolution.candidates.clone();
    let mut cycled = resolution.clone();
    let mut first_wrap = Vec::new();
    let mut publications = Vec::new();
    for step in 0..(sealed_candidates.len() * 2) {
        let next = cycled
            .next_cycle_candidate_id()
            .expect("multi-candidate cohort remains cycleable");
        coordinator.editor_mut().pointer_move_with_draft_authoring(
            &rectangle_scene,
            input(&rectangle_scene, [2.0, 0.0]),
            authoring(Some(next)),
        );
        cycled = coordinator
            .editor()
            .draft_inference_resolution()
            .expect("sealed cycle publication")
            .clone();
        assert!(matches!(
            cycled.status,
            DraftInferenceStatus::Resolved { candidate: resolved } if resolved == next
        ));
        assert_eq!(cycled.candidates, sealed_candidates);
        if step < sealed_candidates.len() {
            first_wrap.push(next);
            publications.push(cycled.clone());
        } else {
            assert_eq!(next, first_wrap[step - sealed_candidates.len()]);
            assert_eq!(cycled, publications[step - sealed_candidates.len()]);
        }
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .to_draft_v5_json()
                .expect("cycling canonical document"),
            rectangle_document
        );
        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            rectangle_history
        );
        assert_eq!(coordinator.transcript().len(), rectangle_transcript_len);
        assert_eq!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .expect("cycling retains accepted rectangle")
                .identity(),
            rectangle_accepted
        );
    }
    assert_eq!(
        first_wrap,
        sealed_candidates
            .iter()
            .skip(1)
            .chain(sealed_candidates.iter().take(1))
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>()
    );

    coordinator.editor_mut().pointer_move_with_draft_authoring(
        &rectangle_scene,
        input(&rectangle_scene, [2.0, 0.0]),
        authoring(Some(candidate)),
    );

    let terminal = press(
        &mut coordinator,
        &rectangle_scene,
        [2.0, 0.0],
        Some(candidate),
    );
    let commit = terminal
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("default midpoint construction plan");
    let EditorEffect::CommitConstructionPlan { expected, plan, .. } = commit else {
        unreachable!("filtered construction plan")
    };
    let generic_rejection = coordinator.apply_construction_plan(expected.as_ref(), plan);
    assert!(matches!(
        generic_rejection,
        Err(CoordinatorError::RedundantInferredConstruction { .. })
    ));
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .to_draft_v5_json()
            .expect("generic rejection canonical document"),
        rectangle_document
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        rectangle_history
    );
    assert_eq!(coordinator.transcript().len(), rectangle_transcript_len);

    let result = publish_plan(&mut coordinator, &terminal);
    let auto_constraints = result
        .constraints
        .iter()
        .filter(|constraint| constraint.provenance == ConstructionRelationProvenance::AutoInference)
        .collect::<Vec<_>>();
    assert_eq!(auto_constraints.len(), 1);
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(auto_constraints[0].constraint)
            .expect("retained auto constraint")
            .definition,
        DocumentConstraintDefinition::Midpoint { .. }
    ));
    assert!(result.constraints.iter().all(|constraint| {
        !matches!(
            coordinator
                .session()
                .design_document()
                .constraint(constraint.constraint)
                .expect("retained construction constraint")
                .definition,
            DocumentConstraintDefinition::Horizontal { .. }
        ) || constraint.provenance != ConstructionRelationProvenance::AutoInference
    }));
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted midpoint line");
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| { point.position.into_iter().all(f64::is_finite) })
    );
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual.is_finite() && residual <= EPSILON)
    );

    let committed_document = coordinator
        .session()
        .design_document()
        .to_draft_v5_json()
        .expect("committed canonical document");
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (rectangle_history.0 + 1, rectangle_history.1 + 1)
    );
    let effective_replay_action = coordinator
        .transcript()
        .last()
        .expect("effective replay action")
        .clone();
    let ReplayAction::ConstructionPlan {
        plan: effective_plan,
        ..
    } = &effective_replay_action
    else {
        panic!("expected effective construction-plan replay")
    };
    assert_eq!(effective_plan.relations.len() + 1, plan.relations.len());
    assert!(effective_plan.relations.iter().all(|definition| {
        definition.provenance != ConstructionRelationProvenance::AutoInference
            || !matches!(definition.relation, InferredRelation::Horizontal { .. })
    }));
    assert!(effective_plan.relations.iter().any(|definition| {
        definition.provenance == ConstructionRelationProvenance::AutoInference
            && matches!(definition.relation, InferredRelation::Midpoint { .. })
    }));
    let committed_identity_high_water = coordinator
        .session()
        .persistent_identity_high_water()
        .clone();

    coordinator.undo().expect("one-step midpoint-line Undo");
    let mut expected_undone =
        SketchDocument::from_draft_v5_json(&rectangle_document).expect("rectangle checkpoint");
    expected_undone
        .retain_persistent_identity_high_water(&committed_identity_high_water)
        .expect("retained midpoint-line identity high-water");
    assert_eq!(
        coordinator.session().design_document(),
        &expected_undone,
        "Undo restores the rectangle semantics without rewinding allocated identities"
    );
    assert_eq!(
        coordinator.session().persistent_identity_high_water(),
        &committed_identity_high_water
    );
    coordinator.redo().expect("one-step midpoint-line Redo");
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .to_draft_v5_json()
            .expect("redo canonical document"),
        committed_document
    );

    let mut replay = RetainedEditorCoordinator::new(replay_seed).expect("replay coordinator");
    replay
        .replay(&effective_replay_action)
        .expect("effective midpoint-line plan replay");
    assert_eq!(
        replay
            .session()
            .design_document()
            .to_draft_v5_json()
            .expect("replayed canonical document"),
        committed_document
    );
}
