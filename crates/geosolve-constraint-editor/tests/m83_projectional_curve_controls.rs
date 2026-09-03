// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AuthoringApplication, AuthoringOperand, AuthoringOptions, AuthoringTool,
    ColdIntentMaterializer, ConstraintIntent, EditorEffect, IntentNativeBinding, Modifiers,
    PointerInput, ProjectionalCoordinatorError, ProjectionalEditorError, ProjectionalEditorSession,
    ProjectionalIntentCoordinator, ResolvedConstraintKind, ScreenPoint, SelectionItem, Viewport,
    projectional_application_patch,
};
use geosolve_sketch::{
    CurveDefinition, CurveId, CurveSpan, DocumentCurveControlId, DocumentCurveControlKind,
    DocumentId, OperationControl, PersistentId,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentFieldKey, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole, IntentPortSelector,
    IntentSessionId, IntentUnit, LeafField, NodeId,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn quantity(value: f64, unit: IntentUnit) -> IntentLiteral {
    IntentLiteral::Quantity { value, unit }
}

fn point_leaf(
    draft: IntentNodeDraft,
    role: IntentPortRole,
    index: u16,
    value: [f64; 2],
) -> IntentNodeDraft {
    draft
        .with_instance_leaf(
            selector(role, index),
            LeafField::X,
            quantity(value[0], IntentUnit::Length),
        )
        .with_instance_leaf(
            selector(role, index),
            LeafField::Y,
            quantity(value[1], IntentUnit::Length),
        )
}

fn coordinator(raw: u128) -> ProjectionalIntentCoordinator {
    ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap()
}

fn create_geometry(
    coordinator: &mut ProjectionalIntentCoordinator,
    alias: &str,
    draft: IntentNodeDraft,
) -> (NodeId, CurveId) {
    let alias = key(alias);
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();
    let node = outcome.aliases.node(&alias).unwrap();
    let port = outcome
        .aliases
        .port(&alias, selector(IntentPortRole::Curve, 0))
        .unwrap();
    let IntentNativeBinding::Curve(curve) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .unwrap()
    else {
        panic!("geometry curve port must bind a native curve")
    };
    (node, curve)
}

fn quadratic_draft() -> IntentNodeDraft {
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::QuadraticBezier,
        },
        key("quadratic.main"),
    );
    let draft = point_leaf(draft, IntentPortRole::Start, 0, [0.0, 0.0]);
    let draft = point_leaf(draft, IntentPortRole::Control, 0, [1.0, 2.0]);
    point_leaf(draft, IntentPortRole::End, 0, [3.0, 0.0])
}

fn circle_draft() -> IntentNodeDraft {
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key("circle.main"),
    );
    point_leaf(draft, IntentPortRole::Center, 0, [0.0, 0.0]).with_instance_leaf(
        selector(IntentPortRole::Target, 0),
        LeafField::Value,
        quantity(2.0, IntentUnit::Length),
    )
}

fn segment_draft() -> IntentNodeDraft {
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("segment.main"),
    )
    .with_field(
        IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point([-1.0, 0.0]),
    );
    let draft = point_leaf(draft, IntentPortRole::Start, 0, [2.0, 1.0]);
    point_leaf(draft, IntentPortRole::End, 0, [-2.0, 1.0])
}

fn rational_draft() -> IntentNodeDraft {
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::RationalQuadraticConic,
        },
        key("rational.main"),
    )
    .with_field(
        IntentFieldKey(key("weighted_middle")),
        IntentLiteral::Point([4.0, 4.0]),
    );
    let draft = point_leaf(draft, IntentPortRole::Start, 0, [0.0, 0.0]);
    point_leaf(draft, IntentPortRole::End, 0, [4.0, 0.0]).with_instance_leaf(
        selector(IntentPortRole::Target, 0),
        LeafField::Weight,
        quantity(2.0, IntentUnit::Dimensionless),
    )
}

fn control(
    coordinator: &ProjectionalIntentCoordinator,
    curve: CurveId,
    kind: DocumentCurveControlKind,
) -> DocumentCurveControlId {
    coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .curve_controls(curve)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id.kind == kind)
        .unwrap()
        .id
}

fn accepted_control_position(
    coordinator: &ProjectionalIntentCoordinator,
    control: DocumentCurveControlId,
) -> [f64; 2] {
    coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .curve_controls(control.curve)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == control)
        .unwrap()
        .position
}

fn accepted_revision(coordinator: &ProjectionalIntentCoordinator) -> u64 {
    coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .identity()
        .revision()
        .get()
}

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn assert_pair(actual: [f64; 2], expected: [f64; 2]) {
    assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
}

fn apply_relation(
    coordinator: &mut ProjectionalIntentCoordinator,
    intent: ConstraintIntent,
    operands: Vec<AuthoringOperand>,
    resolved: ResolvedConstraintKind,
) {
    let accepted = coordinator.accepted_materialization().unwrap();
    let translated = projectional_application_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &accepted.ownership,
        accepted.session.design_document(),
        accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &AuthoringApplication {
            tool: AuthoringTool::Constraint(intent),
            operands,
            options: AuthoringOptions::default(),
            resolved_constraint: Some(resolved),
        },
    )
    .unwrap();
    coordinator.apply_patch(translated.patch).unwrap();
}

#[test]
fn point_backed_curve_control_uses_exact_reverse_leaves_and_one_history_step() {
    let mut coordinator = coordinator(0x8300_7101);
    let (_, curve) = create_geometry(&mut coordinator, "quadratic", quadratic_draft());
    let control = control(
        &coordinator,
        curve,
        DocumentCurveControlKind::ControlPoint { ordinal: 1 },
    );
    let before_identity = coordinator.intent().identity();
    let before_history = coordinator.intent().history_projection();
    let expected = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_identity();

    coordinator
        .begin_curve_control_drag(17, accepted_revision(&coordinator), expected, control)
        .unwrap();
    let preview = coordinator
        .preview_curve_control_drag(
            17,
            1,
            expected,
            control,
            [1.5, 2.5],
            OperationControl::unlimited(),
        )
        .unwrap()
        .unwrap();
    assert_pair(preview.accepted_position, [1.5, 2.5]);
    assert_eq!(coordinator.intent().identity(), before_identity);
    assert_eq!(coordinator.intent().history_projection(), before_history);
    assert_pair(
        coordinator
            .presentation_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .curve_controls(curve)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == control)
            .unwrap()
            .position,
        [1.5, 2.5],
    );

    assert!(
        coordinator
            .finish_curve_control_drag(17, 1, expected, control)
            .unwrap()
            .is_some()
    );
    assert_pair(accepted_control_position(&coordinator, control), [1.5, 2.5]);
    assert_eq!(
        coordinator.intent().history_projection().applied.len(),
        before_history.applied.len() + 1
    );
    coordinator.undo().unwrap().unwrap();
    assert_pair(accepted_control_position(&coordinator, control), [1.0, 2.0]);
}

#[test]
fn scalar_control_keeps_last_valid_sample_across_rejection_and_stale_input() {
    let mut coordinator = coordinator(0x8300_7102);
    let (_, curve) = create_geometry(&mut coordinator, "circle", circle_draft());
    let control = control(&coordinator, curve, DocumentCurveControlKind::Radius);
    let expected = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_identity();
    let before_identity = coordinator.intent().identity();
    let before_history = coordinator.intent().history_projection();
    coordinator
        .begin_curve_control_drag(18, accepted_revision(&coordinator), expected, control)
        .unwrap();
    assert!(
        coordinator
            .preview_curve_control_drag(
                18,
                4,
                expected,
                control,
                [3.0, 0.0],
                OperationControl::unlimited(),
            )
            .unwrap()
            .is_some()
    );
    assert!(
        coordinator
            .preview_curve_control_drag(
                18,
                5,
                expected,
                control,
                [0.0, 0.0],
                OperationControl::unlimited(),
            )
            .unwrap()
            .is_none(),
        "a zero-radius inverse projection must reject without erasing request 4",
    );
    assert!(matches!(
        coordinator.preview_curve_control_drag(
            18,
            3,
            expected,
            control,
            [4.0, 0.0],
            OperationControl::unlimited(),
        ),
        Err(ProjectionalCoordinatorError::StaleDragSample)
    ));
    assert_eq!(coordinator.intent().identity(), before_identity);
    assert_eq!(coordinator.intent().history_projection(), before_history);
    assert_pair(
        coordinator
            .presentation_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .curve_controls(curve)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == control)
            .unwrap()
            .position,
        [3.0, 0.0],
    );
    assert!(
        coordinator
            .finish_curve_control_drag(18, 4, expected, control)
            .unwrap()
            .is_some()
    );
    assert_pair(accepted_control_position(&coordinator, control), [3.0, 0.0]);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one underconstrained radius terminal keeps its complete accepted numerical continuation and history"
)]
fn constrained_radius_control_retains_the_exact_terminal_preview() {
    let mut coordinator = coordinator(0x8300_7106);
    let (_, line) = create_geometry(&mut coordinator, "line", segment_draft());
    let (_, circle) = create_geometry(&mut coordinator, "circle", circle_draft());
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let CurveDefinition::Line {
        start: line_start,
        end: line_end,
        ..
    } = document.curve(line).unwrap().definition
    else {
        panic!("segment fixture must lower to one line");
    };
    let CurveDefinition::Circle {
        center: circle_center,
        ..
    } = document.curve(circle).unwrap().definition
    else {
        panic!("circle fixture must lower to one circle");
    };

    for point in [line_start, circle_center] {
        apply_relation(
            &mut coordinator,
            ConstraintIntent::Lock,
            vec![AuthoringOperand::selected(SelectionItem::Point(point))],
            ResolvedConstraintKind::FixedPoint,
        );
    }
    apply_relation(
        &mut coordinator,
        ConstraintIntent::Tangent,
        vec![
            AuthoringOperand::picked(SelectionItem::Curve(CurveSpan::line(line)), Some(0.5)),
            AuthoringOperand::picked(
                SelectionItem::Curve(CurveSpan::line(circle)),
                Some(std::f64::consts::FRAC_PI_2),
            ),
        ],
        ResolvedConstraintKind::CurveTangency,
    );

    let before = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let line_end_before = before.point(line_end).unwrap().position;
    let control = control(&coordinator, circle, DocumentCurveControlKind::Radius);
    let expected = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_identity();
    let history_before = coordinator.intent().undo_len();

    coordinator
        .begin_curve_control_drag(23, accepted_revision(&coordinator), expected, control)
        .unwrap();
    coordinator
        .preview_curve_control_drag(
            23,
            1,
            expected,
            control,
            [1.5, 0.0],
            OperationControl::unlimited(),
        )
        .unwrap()
        .expect("the constrained radius has one accepted terminal preview");
    let preview = coordinator
        .presentation_session()
        .unwrap()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    assert_ne!(
        preview.point(line_end).unwrap().position.map(f64::to_bits),
        line_end_before.map(f64::to_bits),
        "the tangent companion must move with the radius control",
    );

    coordinator
        .finish_curve_control_drag(23, 1, expected, control)
        .unwrap()
        .expect("the constrained radius terminal must publish");
    let accepted = coordinator.accepted_materialization().unwrap();
    assert_eq!(
        accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &preview,
    );
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    assert_eq!(coordinator.intent().undo_len(), history_before + 1);
    coordinator
        .undo()
        .unwrap()
        .expect("control drag must be undoable");
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &before,
    );
    coordinator
        .redo()
        .unwrap()
        .expect("control drag must be redoable");
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &preview,
    );
}

#[test]
fn rational_middle_updates_typed_definition_without_rewriting_its_weight() {
    let mut coordinator = coordinator(0x8300_7103);
    let (node, curve) = create_geometry(&mut coordinator, "rational", rational_draft());
    let control = control(
        &coordinator,
        curve,
        DocumentCurveControlKind::RationalMiddle,
    );
    let expected = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_identity();
    let CurveDefinition::RationalQuadraticConic { middle_weight, .. } = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .curve(curve)
        .unwrap()
        .definition
    else {
        panic!("rational curve expected")
    };
    let weight_leaf = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .writable_leaf(
            geosolve_constraint_editor::IntentNativeWritableLeaf::ScalarValue {
                scalar: middle_weight,
            },
        )
        .unwrap();
    let weight_before = coordinator.intent().instance().values()[&weight_leaf].clone();

    coordinator
        .begin_curve_control_drag(19, accepted_revision(&coordinator), expected, control)
        .unwrap();
    let preview = coordinator
        .preview_curve_control_drag(
            19,
            8,
            expected,
            control,
            [3.0, 1.0],
            OperationControl::unlimited(),
        )
        .unwrap()
        .unwrap();
    assert_pair(preview.accepted_position, [3.0, 1.0]);
    coordinator
        .finish_curve_control_drag(19, 8, expected, control)
        .unwrap()
        .unwrap();
    assert_eq!(
        coordinator.intent().graph().node(node).unwrap().fields
            [&IntentFieldKey(key("weighted_middle"))],
        IntentLiteral::Point([6.0, 2.0]),
    );
    assert_eq!(
        coordinator.intent().instance().values()[&weight_leaf],
        weight_before,
        "spatial middle movement must preserve the independent weight leaf",
    );
    assert_pair(accepted_control_position(&coordinator, control), [3.0, 1.0]);
    coordinator.undo().unwrap().unwrap();
    assert_pair(accepted_control_position(&coordinator, control), [2.0, 2.0]);
}

#[test]
fn stale_terminal_sample_rejects_and_restores_prior_authority_without_history() {
    let mut coordinator = coordinator(0x8300_7105);
    let (_, curve) = create_geometry(&mut coordinator, "circle", circle_draft());
    let control = control(&coordinator, curve, DocumentCurveControlKind::Radius);
    let expected = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_identity();
    let evidence_before = coordinator
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let identity_before = coordinator.intent().identity();
    let history_before = coordinator.intent().history_projection();
    assert!(matches!(
        coordinator.begin_curve_control_drag(
            22,
            accepted_revision(&coordinator) + 1,
            expected,
            control,
        ),
        Err(ProjectionalCoordinatorError::StaleCurveControlRoute)
    ));
    coordinator
        .begin_curve_control_drag(22, accepted_revision(&coordinator), expected, control)
        .unwrap();
    coordinator
        .preview_curve_control_drag(
            22,
            9,
            expected,
            control,
            [3.0, 0.0],
            OperationControl::unlimited(),
        )
        .unwrap()
        .unwrap();
    assert!(matches!(
        coordinator.finish_curve_control_drag(22, 8, expected, control),
        Err(ProjectionalCoordinatorError::StaleDragSample)
    ));
    assert_eq!(coordinator.intent().identity(), identity_before);
    assert_eq!(coordinator.intent().history_projection(), history_before);
    assert_eq!(
        coordinator.accepted_materialization().unwrap().evidence,
        evidence_before,
    );
    assert_pair(accepted_control_position(&coordinator, control), [2.0, 0.0]);
    assert_pair(
        coordinator
            .presentation_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .curve_controls(curve)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == control)
            .unwrap()
            .position,
        [2.0, 0.0],
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end gesture fixture keeps pointer-down, preview, cancellation, and sole-history evidence contiguous"
)]
fn projectional_editor_resolves_curve_effects_and_cancel_is_history_free() {
    let mut coordinator = coordinator(0x8300_7104);
    let (_, curve) = create_geometry(&mut coordinator, "circle", circle_draft());
    let mut session = ProjectionalEditorSession::new(coordinator);
    session.set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
    let viewport = Viewport::new([800.0, 600.0], [0.0, 0.0], 50.0).unwrap();
    let scene = session.scene(viewport, 0.5).unwrap();
    let radius = scene
        .curve_controls
        .iter()
        .find(|candidate| candidate.id.kind == DocumentCurveControlKind::Radius)
        .unwrap()
        .clone();
    let history_before = session
        .coordinator()
        .intent()
        .history_projection()
        .applied
        .len();
    let identity_before = session.coordinator().intent().identity();
    assert!(
        session
            .pointer_down(&scene, pointer(20, radius.screen_position))
            .unwrap()
            .is_empty()
    );
    let target = ScreenPoint {
        x: radius.screen_position.x + 50.0,
        y: radius.screen_position.y,
    };
    assert!(matches!(
        session.pointer_move(&scene, pointer(20, target)).unwrap().as_slice(),
        [EditorEffect::PreviewCurveControl { control, .. }] if *control == radius.id
    ));
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before,
    );
    let preview_scene = session.scene(viewport, 0.5).unwrap();
    let destination = Viewport::new([800.0, 600.0], [2.0, -1.0], 85.0).unwrap();
    let mut retained_preview = preview_scene.clone();
    session
        .reproject_scene(&mut retained_preview, destination)
        .expect("the live curve-control preview is camera-reprojectable");
    let outcome = session
        .pointer_up(&preview_scene, pointer(20, target))
        .unwrap();
    assert!(outcome.effects.is_empty());
    assert!(outcome.transaction.is_some());
    assert_pair(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .curve_controls(curve)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == radius.id)
            .unwrap()
            .position,
        [3.0, 0.0],
    );
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before + 1,
    );
    assert!(matches!(
        session.reproject_scene(&mut retained_preview, viewport),
        Err(ProjectionalEditorError::SceneAuthorityMismatch)
    ));

    let scene = session.scene(viewport, 0.5).unwrap();
    let radius = scene
        .curve_controls
        .iter()
        .find(|candidate| candidate.id.kind == DocumentCurveControlKind::Radius)
        .unwrap()
        .clone();
    session
        .pointer_down(&scene, pointer(21, radius.screen_position))
        .unwrap();
    let cancel_target = ScreenPoint {
        x: radius.screen_position.x + 25.0,
        y: radius.screen_position.y,
    };
    session
        .pointer_move(&scene, pointer(21, cancel_target))
        .unwrap();
    let mut canceled_preview = session.scene(viewport, 0.5).unwrap();
    assert_eq!(
        session.cancel_interaction(),
        vec![EditorEffect::ClearCurveControlPreview],
    );
    assert!(matches!(
        session.reproject_scene(&mut canceled_preview, destination),
        Err(ProjectionalEditorError::SceneAuthorityMismatch)
    ));
    assert_eq!(
        session
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before + 1,
    );
    assert_pair(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .curve_controls(curve)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == radius.id)
            .unwrap()
            .position,
        [3.0, 0.0],
    );
}
