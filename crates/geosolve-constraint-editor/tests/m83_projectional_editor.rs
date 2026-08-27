// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, ConstraintEditor, ConstructionCommitPlan, ConstructionPoint,
    ConstructionProposal, ConstructionRelationDefinition, DraftSpanSlot, EditorEffect,
    GeometryRoleSelectionState, GeometryToolVariant, InferredRelation, IntentNativeBinding,
    IntentSourceTokenTarget, Modifiers, PointerInput, ProjectionalEditorError,
    ProjectionalEditorSession, ProjectionalIntentCoordinator, ScreenPoint, SelectionItem, Viewport,
    projectional_construction_patch,
};
use geosolve_sketch::{
    CurveSpan, DesignPointId, DocumentId, GeometryRole, OperationControl, PersistentId,
    SketchDatum, cancellation_pair,
};
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

const fn start() -> IntentPortSelector {
    IntentPortSelector::Node {
        role: IntentPortRole::Start,
        index: 0,
    }
}

const fn end() -> IntentPortSelector {
    IntentPortSelector::Node {
        role: IntentPortRole::End,
        index: 0,
    }
}

const fn span() -> IntentPortSelector {
    IntentPortSelector::Node {
        role: IntentPortRole::Span,
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
    fixture_with_preview_control(None)
}

fn fixture_with_preview_control(
    preview_control: Option<OperationControl>,
) -> (ProjectionalEditorSession, DesignPointId, Viewport) {
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
    let session = match preview_control {
        Some(control) => ProjectionalEditorSession::with_editor_and_control(
            coordinator,
            ConstraintEditor::default(),
            control,
        ),
        None => ProjectionalEditorSession::new(coordinator),
    };
    (
        session,
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

fn aligned_rectangle_plan() -> ConstructionCommitPlan {
    ConstructionCommitPlan {
        proposal: ConstructionProposal::RectangleLoop {
            points: vec![
                ConstructionPoint::New([-2.0, -1.5]),
                ConstructionPoint::New([2.0, 1.5]),
                ConstructionPoint::New([2.0, -1.5]),
                ConstructionPoint::New([-2.0, 1.5]),
            ],
            corners: [0, 2, 1, 3],
            center: None,
        },
        curve_roles: vec![GeometryRole::Profile; 4],
        relations: [
            InferredRelation::Horizontal {
                line: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                },
            },
            InferredRelation::Vertical {
                line: DraftSpanSlot::Created {
                    curve_index: 1,
                    segment: 0,
                },
            },
            InferredRelation::Horizontal {
                line: DraftSpanSlot::Created {
                    curve_index: 2,
                    segment: 0,
                },
            },
            InferredRelation::Vertical {
                line: DraftSpanSlot::Created {
                    curve_index: 3,
                    segment: 0,
                },
            },
        ]
        .into_iter()
        .map(ConstructionRelationDefinition::recipe_intrinsic)
        .collect(),
    }
}

fn shared_corner_rectangle_diagonal_fixture() -> (
    ProjectionalEditorSession,
    DesignPointId,
    CurveSpan,
    Viewport,
) {
    let (mut session, _seed, viewport) = fixture();
    let rectangle = aligned_rectangle_plan();
    let rectangle = {
        let accepted = session.coordinator().accepted_materialization().unwrap();
        projectional_construction_patch(
            session.coordinator().intent().identity(),
            session.coordinator().intent(),
            &accepted.ownership,
            GeometryToolVariant::TwoPointAlignedRectangle,
            &rectangle,
        )
        .unwrap()
    };
    let rectangle_alias = rectangle.geometry_alias.clone();
    let rectangle = session.apply_patch(rectangle.patch).unwrap();
    let corner_port = rectangle
        .aliases
        .port(
            &rectangle_alias,
            IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 0,
            },
        )
        .unwrap();
    let opposite_port = rectangle
        .aliases
        .port(
            &rectangle_alias,
            IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 2,
            },
        )
        .unwrap();
    let ownership = &session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .ownership;
    let IntentNativeBinding::Point(corner) = ownership.port(corner_port).unwrap() else {
        panic!("rectangle corner must own a native point");
    };
    let IntentNativeBinding::Point(opposite) = ownership.port(opposite_port).unwrap() else {
        panic!("rectangle opposite corner must own a native point");
    };
    let document = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let line = ConstructionCommitPlan {
        proposal: ConstructionProposal::Line {
            start: ConstructionPoint::Existing {
                id: corner,
                position: document.point(corner).unwrap().position,
            },
            end: ConstructionPoint::Existing {
                id: opposite,
                position: document.point(opposite).unwrap().position,
            },
        },
        curve_roles: vec![GeometryRole::Profile],
        relations: Vec::new(),
    };
    let line = {
        let accepted = session.coordinator().accepted_materialization().unwrap();
        projectional_construction_patch(
            session.coordinator().intent().identity(),
            session.coordinator().intent(),
            &accepted.ownership,
            GeometryToolVariant::Segment,
            &line,
        )
        .unwrap()
    };
    let line_alias = line.geometry_alias.clone();
    let line = session.apply_patch(line.patch).unwrap();
    let line_port = line.aliases.port(&line_alias, span()).unwrap();
    let IntentNativeBinding::CurveSpan(diagonal) = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(line_port)
        .unwrap()
    else {
        panic!("shared-corner segment must own one native span");
    };
    (session, corner, diagonal, viewport)
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
fn retained_camera_scene_reauthenticates_after_preview_cancel_and_accepted_replacement() {
    let (mut session, _, viewport) = fixture();
    let origin = viewport.model_to_screen([1.0, 2.0]);
    let target = viewport.model_to_screen([1.5, 2.25]);
    let destination = Viewport::new([800.0, 600.0], [3.0, -2.0], 85.0).unwrap();

    let mut accepted_scene = session.scene(viewport, 0.5).unwrap();
    session
        .reproject_scene(&mut accepted_scene, destination)
        .expect("the current accepted cache is camera-reprojectable");

    let origin_scene = session.scene(viewport, 0.5).unwrap();
    session
        .pointer_down(&origin_scene, pointer(70, origin))
        .unwrap();
    session
        .pointer_move(&origin_scene, pointer(70, target))
        .unwrap();
    let mut preview_scene = session.scene(viewport, 0.5).unwrap();
    session.cancel_interaction();
    assert!(matches!(
        session.reproject_scene(&mut preview_scene, destination),
        Err(ProjectionalEditorError::SceneAuthorityMismatch)
    ));

    let origin_scene = session.scene(viewport, 0.5).unwrap();
    session
        .pointer_down(&origin_scene, pointer(71, origin))
        .unwrap();
    session
        .pointer_move(&origin_scene, pointer(71, target))
        .unwrap();
    let terminal_scene = session.scene(viewport, 0.5).unwrap();
    session
        .pointer_up(&terminal_scene, pointer(71, target))
        .unwrap();
    assert!(matches!(
        session.reproject_scene(&mut accepted_scene, destination),
        Err(ProjectionalEditorError::SceneAuthorityMismatch)
    ));

    let mut committed_scene = session.scene(viewport, 0.5).unwrap();
    session
        .reproject_scene(&mut committed_scene, destination)
        .expect("the newly committed scene is current");
    session.undo().unwrap().unwrap();
    assert!(matches!(
        session.reproject_scene(&mut committed_scene, viewport),
        Err(ProjectionalEditorError::SceneAuthorityMismatch)
    ));

    let mut undone_scene = session.scene(viewport, 0.5).unwrap();
    session
        .reproject_scene(&mut undone_scene, destination)
        .expect("the Undo scene is current");
    session.redo().unwrap().unwrap();
    assert!(matches!(
        session.reproject_scene(&mut undone_scene, viewport),
        Err(ProjectionalEditorError::SceneAuthorityMismatch)
    ));

    let mut redone_scene = session.scene(viewport, 0.5).unwrap();
    session
        .reproject_scene(&mut redone_scene, destination)
        .expect("the Redo scene is current");
}

#[test]
fn rejected_terminal_sample_commits_the_last_visible_accepted_point_preview_once() {
    let (cancel, token) = cancellation_pair();
    let mut control = OperationControl::unlimited();
    control.token = token;
    let (mut session, point, viewport) = fixture_with_preview_control(Some(control));
    let history_before = session.coordinator().intent().undo_len();
    let origin = viewport.model_to_screen([1.0, 2.0]);
    let accepted_target = viewport.model_to_screen([2.0, 3.0]);
    let rejected_terminal = viewport.model_to_screen([5.0, -4.0]);

    let scene = session.scene(viewport, 0.5).unwrap();
    session.pointer_down(&scene, pointer(8, origin)).unwrap();
    assert_eq!(
        session
            .pointer_move(&scene, pointer(8, accepted_target))
            .unwrap(),
        vec![EditorEffect::PreviewPointMove {
            point,
            model_position: [2.0, 3.0],
        }],
    );
    assert_pair(point_position(&session, point), [2.0, 3.0]);

    cancel.cancel();
    let preview_scene = session.scene(viewport, 0.5).unwrap();
    assert!(
        session
            .pointer_move(&preview_scene, pointer(8, rejected_terminal))
            .unwrap()
            .is_empty(),
        "controlled rejection must retain the visibly accepted preview",
    );
    assert_pair(point_position(&session, point), [2.0, 3.0]);

    let preview_scene = session.scene(viewport, 0.5).unwrap();
    let outcome = session
        .pointer_up(&preview_scene, pointer(8, rejected_terminal))
        .unwrap();
    assert!(outcome.transaction.is_some());
    assert_pair(point_position(&session, point), [2.0, 3.0]);
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    session.undo().unwrap().unwrap();
    assert_pair(point_position(&session, point), [1.0, 2.0]);
}

#[test]
fn shared_corner_rectangle_diagonal_drag_commits_the_visible_preview() {
    let (mut session, corner, diagonal, viewport) = shared_corner_rectangle_diagonal_fixture();
    let origin = point_position(&session, corner);
    let history_before = session.coordinator().intent().undo_len();
    let branch_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .curve_branch_direction(diagonal)
        .unwrap();
    let deltas = [
        [1.0, 0.5],
        [-0.35, 0.8],
        [0.7, -0.4],
        [-0.6, -0.6],
        [0.9, 0.2],
        [-0.25, 0.45],
    ];
    for (index, delta) in deltas.into_iter().enumerate() {
        let current = point_position(&session, corner);
        let target = [current[0] + delta[0], current[1] + delta[1]];
        let pointer_id = 83 + u64::try_from(index).unwrap();
        let scene = session.scene(viewport, 0.5).unwrap();
        session
            .pointer_down(
                &scene,
                pointer(pointer_id, viewport.model_to_screen(current)),
            )
            .unwrap();
        let effects = session
            .pointer_move(
                &scene,
                pointer(pointer_id, viewport.model_to_screen(target)),
            )
            .unwrap();
        let preview = effects
            .iter()
            .find_map(|effect| match effect {
                EditorEffect::PreviewPointMove {
                    point,
                    model_position,
                } if *point == corner => Some(*model_position),
                _ => None,
            })
            .expect("shared-corner drag must publish one accepted preview");
        assert_pair(point_position(&session, corner), preview);

        let preview_scene = session.scene(viewport, 0.5).unwrap();
        let outcome = session
            .pointer_up(
                &preview_scene,
                pointer(pointer_id, viewport.model_to_screen(target)),
            )
            .unwrap();
        assert!(outcome.transaction.is_some());
        assert_pair(point_position(&session, corner), preview);
        assert_eq!(
            session.coordinator().intent().undo_len(),
            history_before + index + 1
        );
        let accepted = session.coordinator().accepted_materialization().unwrap();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual <= 1.0e-9)
        );
        assert_eq!(
            accepted
                .session
                .design_document()
                .curve_branch_direction(diagonal)
                .unwrap()
                .map(f64::to_bits),
            branch_before.map(f64::to_bits),
            "the explicit diagonal branch is not derived preview metadata",
        );
    }

    for _ in deltas {
        session.undo().unwrap().unwrap();
    }
    assert_pair(point_position(&session, corner), origin);
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
    let scene = session.scene(viewport, 0.5).unwrap();
    let origin = viewport.model_to_screen([1.0, 2.0]);
    assert!(matches!(
        session.pointer_down(&scene, pointer(10, origin)),
        Err(ProjectionalEditorError::RetainedIntentDirectManipulationUnavailable)
    ));
    assert!(session.editor().active_pointer_gesture().is_none());
    assert_pair(point_position(&session, point), [1.0, 2.0]);
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
fn native_and_design_selection_project_to_one_exact_declaration_owner() {
    let (mut session, point, _viewport) = fixture();
    let node = *session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .keys()
        .next()
        .unwrap();

    assert_eq!(session.selected_declaration(), None);
    session.set_selection([SelectionItem::Point(point)]);
    assert_eq!(session.selected_declaration(), Some(node));

    assert!(session.set_selected_declaration(Some(node)));
    assert!(session.editor().selection().is_empty());
    assert_eq!(session.selected_declaration(), Some(node));

    session.set_selection([SelectionItem::Datum(SketchDatum::XAxis)]);
    assert_eq!(session.selected_declaration(), None);
    assert!(matches!(
        session.delete_selected_declaration(),
        Err(ProjectionalEditorError::MissingDeleteSelection)
    ));

    session.set_selection([SelectionItem::Point(point)]);
    let history_before = session.coordinator().intent().undo_len();
    assert_eq!(
        session.delete_selected_declaration().unwrap().disposition,
        IntentPlanDisposition::Accepted
    );
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    assert!(session.coordinator().intent().graph().node(node).is_none());
    assert_eq!(session.selected_declaration(), None);
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
#[allow(clippy::too_many_lines)]
fn selected_curve_role_toggle_edits_owning_declarations_atomically() {
    let mut coordinator = ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(0x8300_4002),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(0x8300_4002_u128 << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let segment = |symbol: &str, y: f64, role: Option<&str>| {
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            key(symbol),
        )
        .with_instance_leaf(start(), LeafField::X, coordinate(0.0))
        .with_instance_leaf(start(), LeafField::Y, coordinate(y))
        .with_instance_leaf(end(), LeafField::X, coordinate(2.0))
        .with_instance_leaf(end(), LeafField::Y, coordinate(y));
        if let Some(role) = role {
            draft.with_field(IntentFieldKey(key("role")), IntentLiteral::Enum(key(role)))
        } else {
            draft
        }
    };
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("profile"),
                    draft: Box::new(segment("profile", 0.0, None)),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("construction"),
                    draft: Box::new(segment("construction", 1.0, Some("construction"))),
                    cell: None,
                },
            ],
        ))
        .unwrap();
    let native_span = |alias: &str| {
        let port = outcome.aliases.port(&key(alias), span()).unwrap();
        let IntentNativeBinding::CurveSpan(span) = coordinator
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(port)
            .unwrap()
        else {
            panic!("segment span must bind one native curve")
        };
        span
    };
    let profile = native_span("profile");
    let construction = native_span("construction");
    let mut editor = ProjectionalEditorSession::new(coordinator);
    editor.set_selection([
        SelectionItem::Curve(profile),
        SelectionItem::Curve(construction),
    ]);
    assert_eq!(
        editor.selected_geometry_role_state().unwrap(),
        Some(GeometryRoleSelectionState::Mixed)
    );
    let history_before = editor.coordinator().intent().undo_len();
    let toggled = editor.toggle_selected_geometry_role().unwrap();
    assert_eq!(toggled.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(editor.coordinator().intent().undo_len(), history_before + 1);
    let document = editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    assert_eq!(
        document.geometry_role(profile.curve),
        Some(GeometryRole::Construction)
    );
    assert_eq!(
        document.geometry_role(construction.curve),
        Some(GeometryRole::Construction)
    );
    editor.undo().unwrap().unwrap();
    let document = editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    assert_eq!(
        document.geometry_role(profile.curve),
        Some(GeometryRole::Profile)
    );
    assert_eq!(
        document.geometry_role(construction.curve),
        Some(GeometryRole::Construction)
    );

    editor.set_selection([SelectionItem::Datum(SketchDatum::XAxis)]);
    assert!(matches!(
        editor.toggle_selected_geometry_role(),
        Err(ProjectionalEditorError::ProtectedGeometryRoleSelection)
    ));
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
