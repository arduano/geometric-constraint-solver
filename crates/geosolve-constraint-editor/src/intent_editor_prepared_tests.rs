// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{DocumentProfileOffsetOperand, PersistentId};
use geosolve_sketch_intent::{
    GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentNodeDraft,
    IntentPatchOperation, IntentPortRole, IntentPortSelector, IntentSessionId, IntentUnit,
    LeafField, PatchPortRef,
};

use super::*;
use crate::{
    ActivePointerGestureKind, OffsetAuthoringTarget, ScreenPoint,
    intent_coordinator::{plan_patch_calls, reset_plan_patch_calls},
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).expect("test key is valid")
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn point(symbol: &str, position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(symbol),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::X,
        coordinate(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::Y,
        coordinate(position[1]),
    )
}

fn point_alias(alias: &str) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(alias),
        selector: selector(IntentPortRole::Primary, 0),
    }
}

fn line(symbol: &str, start: &str, end: &str, direction: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(symbol),
    )
    .with_input(InputSlot::new(InputRole::Point, 0), point_alias(start))
    .with_input(InputSlot::new(InputRole::Point, 1), point_alias(end))
    .with_field(
        IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point(direction),
    )
}

fn create(alias: &str, draft: IntentNodeDraft) -> IntentPatchOperation {
    IntentPatchOperation::CreateNode {
        alias: key(alias),
        draft: Box::new(draft),
        cell: None,
    }
}

fn fixture(raw: u128, model_scale: f64) -> (ProjectionalEditorSession, Viewport) {
    let mut coordinator = ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            model_scale,
        )
        .expect("test materializer"),
    )
    .expect("test coordinator");
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                create("a", point("point.a", [0.0, 0.0])),
                create("b", point("point.b", [4.0, 0.0])),
                create("c", point("point.c", [4.0, 3.0])),
                create("d", point("point.d", [0.0, 3.0])),
                create("bottom", line("line.bottom", "a", "b", [1.0, 0.0])),
                create("right", line("line.right", "b", "c", [0.0, 1.0])),
                create("top", line("line.top", "c", "d", [-1.0, 0.0])),
                create("left", line("line.left", "d", "a", [0.0, -1.0])),
            ],
        ))
        .expect("base rectangle materializes");
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    (
        ProjectionalEditorSession::new(coordinator),
        Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).expect("test viewport"),
    )
}

fn begin_fillet_authoring(
    session: &mut ProjectionalEditorSession,
    viewport: Viewport,
    symbol: &str,
) -> FeatureAuthoringState {
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        session
            .activate_feature_authoring(
                &mut state,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(0.5),
                    ..FeatureAuthoringOptions::default()
                },
                &[],
                key(symbol),
            )
            .expect("activate Fillet"),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    let scene = session.scene(viewport, 0.5).expect("accepted scene");
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen([3.0, 0.0]),
                PickTolerance::default(),
                key(symbol),
            )
            .expect("first Fillet parent"),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    state
}

fn complete_fillet_authoring(
    session: &mut ProjectionalEditorSession,
    state: &mut FeatureAuthoringState,
    viewport: Viewport,
    symbol: &str,
) {
    let scene = session.scene(viewport, 0.5).expect("accepted scene");
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                state,
                &scene,
                viewport.model_to_screen([4.0, 1.0]),
                PickTolerance::default(),
                key(symbol),
            )
            .expect("second Fillet parent"),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    assert!(session.feature_authoring_preview_matches(state));
}

fn offset_state(session: &mut ProjectionalEditorSession) -> OffsetAuthoringState {
    let mut state = OffsetAuthoringState::default();
    assert!(matches!(
        session
            .activate_offset_authoring(&mut state)
            .expect("activate Profile Offset"),
        OffsetAuthoringOutcome::ModeEntered(_)
    ));
    let face = state.index().expect("offset topology index").faces()[0]
        .key
        .clone();
    assert!(matches!(
        state.pick_target(OffsetAuthoringTarget::Face(face)),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert!(matches!(
        state.set_distance(0.5),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    state
}

fn profile_offset_dimension(session: &ProjectionalEditorSession) -> DocumentDimensionId {
    let dimensions = session
        .coordinator()
        .accepted_materialization()
        .expect("accepted Offset")
        .session
        .design_document()
        .dimensions()
        .iter()
        .filter(|dimension| {
            matches!(
                dimension.definition,
                DocumentDimensionDefinition::ProfileOffset { .. }
            )
        })
        .map(|dimension| dimension.id)
        .collect::<Vec<_>>();
    let [dimension] = dimensions.as_slice() else {
        panic!("exactly one Profile Offset dimension expected")
    };
    *dimension
}

fn offset_drag_geometry(
    session: &ProjectionalEditorSession,
    dimension: DocumentDimensionId,
    viewport: Viewport,
) -> (EditorScene, ScreenPoint, [f64; 2]) {
    let document = session
        .presentation_session()
        .expect("presented Offset")
        .design_document();
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &document
        .dimension(dimension)
        .expect("Offset dimension")
        .definition
    else {
        unreachable!()
    };
    let edge = match operand {
        DocumentProfileOffsetOperand::Face { outer, .. } => outer.edges[0],
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => chain.edges[0],
    };
    let source = document
        .evaluate_curve_jet(edge.source.curve, 0.5)
        .expect("source jet");
    let target = document
        .evaluate_curve_jet(edge.target.curve, 0.5)
        .expect("target jet");
    let normal = source.differential().expect("regular source").left_normal;
    let separation = [
        target.position.x - source.position.x,
        target.position.y - source.position.y,
    ];
    let sign = if separation[0].mul_add(normal.x, separation[1] * normal.y) > 0.0 {
        1.0
    } else {
        -1.0
    };
    let derivative = [normal.x * sign, normal.y * sign];
    let scene = session.scene(viewport, 0.5).expect("Offset scene");
    let curve = scene
        .curves
        .iter()
        .find(|curve| curve.span == edge.target.curve)
        .expect("offset target curve");
    let first = curve.screen_polyline.first().expect("target samples");
    let last = curve.screen_polyline.last().expect("target samples");
    let press = ScreenPoint {
        x: 0.5 * (first.x + last.x),
        y: 0.5 * (first.y + last.y),
    };
    (scene, press, derivative)
}

const fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers {
            shift: false,
            control: false,
            command: false,
        },
    }
}

fn assert_one_preview_plan_and_no_terminal_replan(before_terminal: usize) {
    assert_eq!(
        before_terminal, 1,
        "the accepted preview must plan and cold-materialize exactly once"
    );
    assert_eq!(
        plan_patch_calls(),
        before_terminal,
        "terminal publication must consume the exact prepared transaction"
    );
}

#[test]
fn fillet_authoring_apply_consumes_its_exact_prepared_transaction() {
    let (mut session, viewport) = fixture(0x8300_8301, 10.0);
    let symbol = "Fillet prepared Apply";
    let mut state = begin_fillet_authoring(&mut session, viewport, symbol);

    reset_plan_patch_calls();
    complete_fillet_authoring(&mut session, &mut state, viewport, symbol);
    let before_apply = plan_patch_calls();
    let outcome = session
        .apply_computed_fillet_preview(&mut state, key(symbol))
        .expect("publish prepared Fillet");

    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_one_preview_plan_and_no_terminal_replan(before_apply);
}

#[test]
fn profile_offset_authoring_apply_consumes_its_exact_prepared_transaction() {
    let (mut session, _) = fixture(0x8300_8302, 1.0);
    let symbol = "Offset prepared Apply";
    let mut state = offset_state(&mut session);

    reset_plan_patch_calls();
    assert!(
        session
            .refresh_offset_authoring_preview(&state, key(symbol))
            .expect("prepare Offset preview")
    );
    let before_apply = plan_patch_calls();
    let outcome = session
        .apply_profile_offset_preview(&mut state, key(symbol))
        .expect("publish prepared Offset");

    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_one_preview_plan_and_no_terminal_replan(before_apply);
}

#[test]
fn accepted_fillet_radius_drop_consumes_its_exact_prepared_transaction() {
    let (mut session, viewport) = fixture(0x8300_8303, 10.0);
    let symbol = "Fillet prepared drop";
    let mut state = begin_fillet_authoring(&mut session, viewport, symbol);
    complete_fillet_authoring(&mut session, &mut state, viewport, symbol);
    session
        .apply_computed_fillet_preview(&mut state, key(symbol))
        .expect("create accepted Fillet");
    let scene = session.scene(viewport, 0.5).expect("Fillet scene");
    let rail = scene.fillet_affordances[0].radius_rail;
    session
        .pointer_down(&scene, pointer(8303, rail.screen_grip))
        .expect("start radius drag");
    assert_eq!(
        session
            .editor()
            .active_pointer_gesture()
            .expect("gesture")
            .kind,
        ActivePointerGestureKind::FilletRadius
    );
    let origin = viewport.screen_to_model(rail.screen_grip);
    let target = viewport.model_to_screen([
        0.25f64.mul_add(rail.model_derivative[0], origin[0]),
        0.25f64.mul_add(rail.model_derivative[1], origin[1]),
    ]);

    reset_plan_patch_calls();
    assert!(
        session
            .pointer_move(&scene, pointer(8303, target))
            .expect("prepare radius preview")
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewComputedFeatureRadius { .. }))
    );
    let before_drop = plan_patch_calls();
    let preview_scene = session.scene(viewport, 0.5).expect("radius preview scene");
    let outcome = session
        .pointer_up(&preview_scene, pointer(8303, target))
        .expect("publish prepared radius")
        .transaction
        .expect("accepted radius transaction");

    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_one_preview_plan_and_no_terminal_replan(before_drop);
}

#[test]
fn accepted_profile_offset_distance_drop_consumes_its_exact_prepared_transaction() {
    let (mut session, viewport) = fixture(0x8300_8304, 1.0);
    let mut state = offset_state(&mut session);
    session
        .apply_profile_offset(&mut state, key("Offset prepared drop"))
        .expect("create accepted Offset");
    let dimension = profile_offset_dimension(&session);
    let (scene, press, derivative) = offset_drag_geometry(&session, dimension, viewport);
    session
        .pointer_down(&scene, pointer(8304, press))
        .expect("start Offset distance drag");
    assert_eq!(
        session
            .editor()
            .active_pointer_gesture()
            .expect("gesture")
            .kind,
        ActivePointerGestureKind::OffsetDistance
    );
    let origin = viewport.screen_to_model(press);
    let target = viewport.model_to_screen([
        0.25f64.mul_add(derivative[0], origin[0]),
        0.25f64.mul_add(derivative[1], origin[1]),
    ]);

    reset_plan_patch_calls();
    assert!(
        session
            .pointer_move(&scene, pointer(8304, target))
            .expect("prepare Offset preview")
            .iter()
            .any(|effect| matches!(
                effect,
                EditorEffect::PreviewAcceptedProfileOffsetDistance { .. }
            ))
    );
    let before_drop = plan_patch_calls();
    let preview_scene = session.scene(viewport, 0.5).expect("Offset preview scene");
    let outcome = session
        .pointer_up(&preview_scene, pointer(8304, target))
        .expect("publish prepared Offset distance")
        .transaction
        .expect("accepted Offset transaction");

    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_one_preview_plan_and_no_terminal_replan(before_drop);
}
