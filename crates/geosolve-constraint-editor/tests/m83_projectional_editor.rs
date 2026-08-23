// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, EditorEffect, IntentNativeBinding, IntentSourceTokenTarget, Modifiers,
    PointerInput, ProjectionalEditorSession, ProjectionalIntentCoordinator, ScreenPoint,
    SelectionItem, Viewport,
};
use geosolve_sketch::{DesignPointId, DocumentId, PersistentId};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentFieldKey, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPlanDisposition, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField,
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

fn invalid_segment() -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("invalid.segment"),
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Start,
            index: 0,
        },
        LeafField::X,
        coordinate(0.0),
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Start,
            index: 0,
        },
        LeafField::Y,
        coordinate(0.0),
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::End,
            index: 0,
        },
        LeafField::X,
        coordinate(1.0),
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::End,
            index: 0,
        },
        LeafField::Y,
        coordinate(0.0),
    )
    .with_field(
        IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point([f64::MAX, f64::MAX]),
    )
}

fn fixture() -> (ProjectionalEditorSession, DesignPointId, Viewport) {
    let mut coordinator = ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(0x8300_4001),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(0x8300_4001_u128 << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("point.main"),
    )
    .with_instance_leaf(primary(), LeafField::X, coordinate(1.0))
    .with_instance_leaf(primary(), LeafField::Y, coordinate(2.0));
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();
    let port = outcome.aliases.port(&key("point"), primary()).unwrap();
    let IntentNativeBinding::Point(point) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .unwrap()
    else {
        panic!("the sketch-point primary port must bind one native point")
    };
    (
        ProjectionalEditorSession::new(coordinator),
        point,
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

fn point_position(session: &ProjectionalEditorSession, point: DesignPointId) -> [f64; 2] {
    session
        .coordinator()
        .presentation_session()
        .unwrap()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .point(point)
        .unwrap()
        .position
}

fn assert_pair(actual: [f64; 2], expected: [f64; 2]) {
    assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
}

#[test]
fn pointer_frames_are_transient_and_release_commits_one_intent_transaction() {
    let (mut session, point, viewport) = fixture();
    let initial_identity = session.coordinator().intent().identity();
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        1
    );

    let scene = session.scene(viewport, 0.5).unwrap();
    let origin = viewport.model_to_screen([1.0, 2.0]);
    assert!(
        session
            .pointer_down(&scene, pointer(7, origin))
            .unwrap()
            .iter()
            .any(|effect| matches!(effect, EditorEffect::SelectionChanged(_)))
    );
    assert_eq!(session.editor().selection(), &[SelectionItem::Point(point)]);
    assert_eq!(session.coordinator().intent().identity(), initial_identity);

    let target = viewport.model_to_screen([1.5, 2.25]);
    let preview_effects = session.pointer_move(&scene, pointer(7, target)).unwrap();
    assert_eq!(
        preview_effects,
        vec![EditorEffect::PreviewPointMove {
            point,
            model_position: [1.5, 2.25],
        }]
    );
    assert_eq!(session.coordinator().intent().identity(), initial_identity);
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        1
    );
    assert_pair(point_position(&session, point), [1.5, 2.25]);

    let preview_scene = session.scene(viewport, 0.5).unwrap();
    let outcome = session
        .pointer_up(&preview_scene, pointer(7, target))
        .unwrap();
    assert!(outcome.effects.is_empty());
    assert!(outcome.transaction.is_some());
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        2
    );
    assert_pair(point_position(&session, point), [1.5, 2.25]);

    session.undo().unwrap().unwrap();
    assert_pair(point_position(&session, point), [1.0, 2.0]);
    assert!(session.editor().selection().is_empty());
}

#[test]
fn cancelled_preview_restores_the_exact_accepted_scene_without_history() {
    let (mut session, point, viewport) = fixture();
    let scene = session.scene(viewport, 0.5).unwrap();
    let origin = viewport.model_to_screen([1.0, 2.0]);
    let target = viewport.model_to_screen([-2.0, 3.0]);
    let _ = session.pointer_down(&scene, pointer(9, origin)).unwrap();
    session.pointer_move(&scene, pointer(9, target)).unwrap();
    assert_pair(point_position(&session, point), [-2.0, 3.0]);

    let history_before = session
        .coordinator()
        .intent()
        .history_projection()
        .applied
        .len();
    assert_eq!(
        session.cancel_interaction(),
        vec![EditorEffect::ClearPointPreview]
    );
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before
    );
    assert_pair(point_position(&session, point), [1.0, 2.0]);
    assert_pair(
        session
            .scene(viewport, 0.5)
            .unwrap()
            .points
            .iter()
            .find(|candidate| candidate.id == point)
            .unwrap()
            .model_position,
        [1.0, 2.0],
    );
}

#[test]
fn retained_invalid_intent_keeps_the_prior_scene_and_transient_selection() {
    let (mut session, point, viewport) = fixture();
    session.set_selection([SelectionItem::Point(point)]);
    let accepted_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();

    let outcome = session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::CreateNode {
                alias: key("unsupported"),
                draft: Box::new(invalid_segment()),
                cell: None,
            }],
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::RetainedFailed);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        accepted_before
    );
    assert_eq!(session.editor().selection(), &[SelectionItem::Point(point)]);
    assert_pair(point_position(&session, point), [1.0, 2.0]);
    assert_pair(
        session
            .scene(viewport, 0.5)
            .unwrap()
            .points
            .iter()
            .find(|candidate| candidate.id == point)
            .unwrap()
            .model_position,
        [1.0, 2.0],
    );
}

#[test]
fn click_only_point_route_is_cancelled_before_the_next_press() {
    let (mut session, _point, viewport) = fixture();
    let scene = session.scene(viewport, 0.5).unwrap();
    let origin = viewport.model_to_screen([1.0, 2.0]);
    let _ = session.pointer_down(&scene, pointer(11, origin)).unwrap();
    let click = session.pointer_up(&scene, pointer(11, origin)).unwrap();
    assert!(click.effects.is_empty());
    assert!(click.transaction.is_none());

    let scene = session.scene(viewport, 0.5).unwrap();
    let _ = session.pointer_down(&scene, pointer(12, origin)).unwrap();
    assert_eq!(
        session.cancel_interaction(),
        Vec::<EditorEffect>::new(),
        "a second point press must prepare and cancel its own fresh route",
    );
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        1
    );
}

#[test]
fn logical_selection_source_edit_and_closure_delete_share_the_intent_history() {
    let (mut session, point, viewport) = fixture();
    let node = *session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .keys()
        .next()
        .unwrap();
    assert!(session.set_selected_declaration(Some(node)));
    let projection = session.workbench_projection();
    assert_eq!(
        session
            .selected_inspector(&projection)
            .expect("selected Inspector")
            .node,
        node
    );
    let name_token = projection
        .structured_source
        .tokens
        .iter()
        .find(|token| token.target == IntentSourceTokenTarget::NodeName { node })
        .unwrap()
        .id;
    let accepted_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let renamed = session
        .edit_source_token(&projection, name_token, r#""Renamed point""#)
        .unwrap();
    assert_eq!(renamed.disposition, IntentPlanDisposition::OrganizationOnly);
    assert_eq!(session.selected_declaration(), Some(node));
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        accepted_before
    );
    assert_eq!(
        session
            .selected_inspector(&session.workbench_projection())
            .unwrap()
            .name
            .as_str(),
        "Renamed point"
    );

    let deleted = session.delete_declaration(node).unwrap();
    assert_eq!(deleted.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(session.selected_declaration(), None);
    assert!(
        session.workbench_projection().outline[0]
            .declarations
            .is_empty()
    );
    assert!(session.scene(viewport, 0.5).unwrap().points.is_empty());

    session.undo().unwrap().unwrap();
    assert_pair(point_position(&session, point), [1.0, 2.0]);
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        2,
        "create plus organization rename remain after undoing deletion",
    );
}

#[test]
fn canonical_restore_cold_rebuilds_without_a_flat_coordinator() {
    let (session, point, viewport) = fixture();
    let json = session.coordinator().intent().to_canonical_json().unwrap();
    let identity = session.coordinator().intent().identity();
    let document = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .validation
        .document;
    let restored =
        ProjectionalEditorSession::restore(IntentSession::from_json(&json).unwrap(), document, 1.0)
            .unwrap();
    assert_eq!(restored.coordinator().intent().identity(), identity);
    assert_pair(point_position(&restored, point), [1.0, 2.0]);
    assert_pair(
        restored
            .scene(viewport, 0.5)
            .unwrap()
            .points
            .iter()
            .find(|candidate| candidate.id == point)
            .unwrap()
            .model_position,
        [1.0, 2.0],
    );
}
