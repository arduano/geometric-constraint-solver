// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, ConstructionProposal, EditorEffect, GeometryDraftIssue,
    GeometryToolVariant, IntentNativeBinding, Modifiers, PointerInput, ProjectionalEditorError,
    ProjectionalEditorSession, ProjectionalIntentCoordinator, ScreenPoint, Viewport,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPlanDisposition, IntentPortRole,
    IntentPortSelector, IntentSessionId, IntentUnit, LeafField,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

const fn primary() -> IntentPortSelector {
    IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    }
}

fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn fixture() -> (ProjectionalEditorSession, Viewport) {
    let raw = 0x8300_7201_u128;
    let mut coordinator = ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let seed = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("seed.point"),
    )
    .with_instance_leaf(primary(), LeafField::X, coordinate(-100.0))
    .with_instance_leaf(primary(), LeafField::Y, coordinate(-100.0));
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("seed"),
                draft: Box::new(seed),
                cell: None,
            }],
        ))
        .unwrap();
    let port = outcome.aliases.port(&key("seed"), primary()).unwrap();
    assert!(matches!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(port),
        Some(IntentNativeBinding::Point(_))
    ));
    (
        ProjectionalEditorSession::new(coordinator),
        Viewport::new([800.0, 600.0], [0.0, 0.0], 50.0).unwrap(),
    )
}

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn terminal_segment_effect(
    session: &mut ProjectionalEditorSession,
    viewport: Viewport,
    pointer_id: u64,
    start: [f64; 2],
    end: [f64; 2],
) -> EditorEffect {
    let _ = session
        .editor_mut()
        .activate_geometry_tool(GeometryToolVariant::Segment);
    let scene = session.scene(viewport, 0.5).unwrap();
    let first = session
        .pointer_down(&scene, pointer(pointer_id, viewport.model_to_screen(start)))
        .unwrap();
    assert!(
        first
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
    );
    let scene = session.scene(viewport, 0.5).unwrap();
    let terminal = session
        .pointer_down(&scene, pointer(pointer_id, viewport.model_to_screen(end)))
        .unwrap();
    let commits = terminal
        .into_iter()
        .filter(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .collect::<Vec<_>>();
    assert_eq!(commits.len(), 1);
    commits.into_iter().next().unwrap()
}

fn accepted_counts(session: &ProjectionalEditorSession) -> (usize, usize, usize) {
    let document = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    (
        document.points().len(),
        document.curves().len(),
        document.constraints().len(),
    )
}

fn assert_independent_validation(session: &ProjectionalEditorSession) {
    let accepted = session.coordinator().accepted_materialization().unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    assert!(
        accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
}

#[test]
fn terminal_plan_publishes_one_intent_transaction_and_undo_restores_scene() {
    let (mut session, viewport) = fixture();
    let history_before = session
        .coordinator()
        .intent()
        .history_projection()
        .applied
        .len();
    let counts_before = accepted_counts(&session);
    let effect = terminal_segment_effect(&mut session, viewport, 17, [1.0, 1.0], [3.0, 2.25]);
    let outcome = session.apply_construction_editor_effect(&effect).unwrap();

    assert_eq!(
        outcome.transaction.disposition,
        IntentPlanDisposition::Accepted
    );
    assert!(
        outcome
            .effects
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before + 1
    );
    assert_eq!(accepted_counts(&session), (counts_before.0 + 2, 1, 0));
    assert_independent_validation(&session);
    assert_eq!(session.editor().pending_construction_commit_token(), None);
    assert_eq!(
        session
            .editor()
            .geometry_draft_status()
            .unwrap()
            .completed_stages,
        0
    );

    session.undo().unwrap().unwrap();
    assert_eq!(accepted_counts(&session), counts_before);
    assert_independent_validation(&session);
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before
    );
}

#[test]
fn substituted_and_stale_terminals_do_not_consume_the_current_draft() {
    let (mut session, viewport) = fixture();
    let genuine = terminal_segment_effect(&mut session, viewport, 23, [1.0, 1.0], [3.0, 2.25]);
    let pending = session
        .editor()
        .pending_construction_commit_token()
        .unwrap();
    let mut substituted = genuine.clone();
    let EditorEffect::CommitConstructionPlan { plan, .. } = &mut substituted else {
        unreachable!()
    };
    plan.proposal = ConstructionProposal::Line {
        start: geosolve_constraint_editor::ConstructionPoint::New([1.0, 1.0]),
        end: geosolve_constraint_editor::ConstructionPoint::New([4.0, 2.0]),
    };
    assert!(matches!(
        session.apply_construction_editor_effect(&substituted),
        Err(ProjectionalEditorError::ConstructionCommitMismatch)
    ));
    assert_eq!(
        session.editor().pending_construction_commit_token(),
        Some(pending)
    );
    assert_eq!(
        session
            .editor()
            .geometry_draft_status()
            .unwrap()
            .completed_stages,
        1
    );

    session.apply_construction_editor_effect(&genuine).unwrap();
    let history_after_first = session
        .coordinator()
        .intent()
        .history_projection()
        .applied
        .len();
    let current = terminal_segment_effect(&mut session, viewport, 29, [5.0, 0.0], [7.0, 1.0]);
    let current_token = session
        .editor()
        .pending_construction_commit_token()
        .unwrap();
    assert!(matches!(
        session.apply_construction_editor_effect(&genuine),
        Err(ProjectionalEditorError::ConstructionCommitMismatch)
    ));
    assert_eq!(
        session.editor().pending_construction_commit_token(),
        Some(current_token)
    );
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_after_first
    );
    session.apply_construction_editor_effect(&current).unwrap();
    assert_independent_validation(&session);
}

#[test]
fn authenticated_stale_input_restores_the_correction_ready_draft() {
    let (mut session, viewport) = fixture();
    let effect = terminal_segment_effect(&mut session, viewport, 31, [1.0, 1.0], [3.0, 2.25]);
    let intervening = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("intervening.point"),
    )
    .with_instance_leaf(primary(), LeafField::X, coordinate(20.0))
    .with_instance_leaf(primary(), LeafField::Y, coordinate(20.0));
    session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("intervening"),
                draft: Box::new(intervening),
                cell: None,
            }],
        ))
        .unwrap();
    let history_after_intervening = session
        .coordinator()
        .intent()
        .history_projection()
        .applied
        .len();
    let counts_after_intervening = accepted_counts(&session);

    // The editor still authenticates its exact terminal envelope, but the
    // accepted native input has advanced. Rejection must restore the retained
    // prefix rather than consuming it or publishing against stale geometry.
    assert!(matches!(
        session.apply_construction_editor_effect(&effect),
        Err(ProjectionalEditorError::ConstructionInputMismatch)
    ));
    assert_eq!(session.editor().pending_construction_commit_token(), None);
    let status = session.editor().geometry_draft_status().unwrap();
    assert_eq!(status.completed_stages, 1);
    assert_eq!(status.issue, Some(GeometryDraftIssue::ConstructionRejected));
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_after_intervening
    );
    assert_eq!(accepted_counts(&session), counts_after_intervening);
    assert_independent_validation(&session);

    let scene = session.scene(viewport, 0.5).unwrap();
    let corrected = session
        .pointer_down(&scene, pointer(31, viewport.model_to_screen([3.5, 2.5])))
        .unwrap()
        .into_iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("the retained prefix accepts one corrected terminal sample");
    session
        .apply_construction_editor_effect(&corrected)
        .unwrap();
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_after_intervening + 1
    );
    assert_eq!(accepted_counts(&session).1, counts_after_intervening.1 + 1);
    assert_independent_validation(&session);
}
