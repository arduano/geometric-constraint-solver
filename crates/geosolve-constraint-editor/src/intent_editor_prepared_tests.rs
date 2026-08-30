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

fn add_fillet(
    session: &mut ProjectionalEditorSession,
    viewport: Viewport,
    symbol: &str,
    radius: f64,
    first: [f64; 2],
    second: [f64; 2],
) -> ComputedFeatureId {
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        session
            .activate_feature_authoring(
                &mut state,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(radius),
                    ..FeatureAuthoringOptions::default()
                },
                &[],
                key(symbol),
            )
            .expect("activate grouped Fillet fixture"),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    let scene = session.scene(viewport, 0.5).expect("accepted source scene");
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen(first),
                PickTolerance::default(),
                key(symbol),
            )
            .expect("first grouped Fillet parent"),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    let scene = session.scene(viewport, 0.5).expect("first-parent scene");
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen(second),
                PickTolerance::default(),
                key(symbol),
            )
            .expect("second grouped Fillet parent"),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    session
        .apply_computed_fillet_preview(&mut state, key(symbol))
        .expect("publish grouped Fillet fixture");
    let node = session
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&key(symbol))
        .expect("Fillet declaration symbol")
        .id;
    let accepted = session
        .coordinator()
        .accepted_materialization()
        .expect("accepted Fillet fixture");
    let features = accepted
        .ownership
        .nodes
        .iter()
        .find(|owner| owner.node == node)
        .expect("Fillet logical owner")
        .owned
        .iter()
        .filter_map(|binding| match binding {
            IntentNativeBinding::ComputedFeature(feature) => Some(*feature),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [feature] = features.as_slice() else {
        panic!("one computed feature expected for {symbol}")
    };
    *feature
}

fn two_fillet_fixture(
    raw: u128,
    radii: [f64; 2],
) -> (ProjectionalEditorSession, Viewport, [ComputedFeatureId; 2]) {
    let (mut session, viewport) = fixture(raw, 10.0);
    let first = add_fillet(
        &mut session,
        viewport,
        "shared radius first",
        radii[0],
        [3.0, 0.0],
        [4.0, 1.0],
    );
    let second = add_fillet(
        &mut session,
        viewport,
        "shared radius second",
        radii[1],
        [1.0, 3.0],
        [0.0, 2.0],
    );
    (session, viewport, [first, second])
}

fn radius_drag_target(
    session: &mut ProjectionalEditorSession,
    viewport: Viewport,
    feature: ComputedFeatureId,
    pointer_id: u64,
    delta: f64,
) -> (EditorScene, ScreenPoint) {
    let scene = session.scene(viewport, 0.5).expect("two-Fillet scene");
    let rail = scene
        .fillet_affordances
        .iter()
        .find(|affordance| affordance.owner.feature == feature)
        .expect("initiating feature radius rail")
        .radius_rail;
    session
        .pointer_down(&scene, pointer(pointer_id, rail.screen_grip))
        .expect("start grouped radius drag");
    let target = viewport.model_to_screen([
        delta.mul_add(rail.model_derivative[0], rail.model_grip[0]),
        delta.mul_add(rail.model_derivative[1], rail.model_grip[1]),
    ]);
    (scene, target)
}

fn accepted_fillet_radius(session: &ProjectionalEditorSession, feature: ComputedFeatureId) -> f64 {
    let accepted = session
        .coordinator()
        .accepted_materialization()
        .expect("accepted Fillet authority");
    let definition = accepted
        .features
        .feature(feature)
        .expect("accepted Fillet feature");
    let ComputedFeatureDefinition::FilletSet(fillet) = &definition.definition;
    fillet.radius
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

fn fixture_point(session: &ProjectionalEditorSession, symbol: &str) -> DesignPointId {
    let node = session
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&key(symbol))
        .expect("fixture point declaration");
    let port = node
        .port_by_selector(selector(IntentPortRole::Primary, 0))
        .expect("fixture point output");
    let IntentNativeBinding::Point(point) = session
        .coordinator()
        .accepted_materialization()
        .expect("fixture accepted authority")
        .ownership
        .port(port.as_ref(node.id))
        .expect("fixture point binding")
    else {
        panic!("fixture point output must bind one native point")
    };
    point
}

#[test]
fn delegated_point_terminal_consumes_preview_without_implicit_publication() {
    let pointer_id = 87_301;
    let target = [0.5, 0.25];
    let (mut delegated, viewport) = fixture(0x8300_8731, 1.0);
    let point = fixture_point(&delegated, "point.a");
    let before = delegated.coordinator().intent().identity();
    let undo_before = delegated.coordinator().intent().undo_len();
    let scene = delegated.scene(viewport, 0.5).expect("delegated scene");
    delegated
        .pointer_down_exact_point(
            &scene,
            pointer(pointer_id, viewport.model_to_screen([0.0, 0.0])),
            point,
        )
        .expect("delegated point route");
    delegated
        .pointer_move(
            &scene,
            pointer(pointer_id, viewport.model_to_screen(target)),
        )
        .expect("delegated accepted preview");
    let preview_scene = delegated
        .scene(viewport, 0.5)
        .expect("delegated preview scene");
    let terminal = delegated.pointer_up_delegated_point_audited(
        &preview_scene,
        pointer(pointer_id, viewport.model_to_screen(target)),
    );
    assert_eq!(terminal.work.native_preview_attempts(), 0);
    assert_eq!(terminal.work.intent_materialization_attempts(), 0);
    assert_eq!(terminal.work.computed_evaluation_attempts(), 0);
    assert_eq!(terminal.work.history_publications(), 0);
    let proposal = terminal
        .outcome
        .expect("delegated terminal")
        .proposal
        .expect("moved delegated proposal");
    assert_eq!(proposal.intent, before);
    assert_eq!(proposal.pointer_id, pointer_id);
    assert_eq!(proposal.point, point);
    assert_eq!(
        proposal.accepted_position.map(f64::to_bits),
        target.map(f64::to_bits)
    );
    assert_eq!(delegated.coordinator().intent().identity(), before);
    assert_eq!(delegated.coordinator().intent().undo_len(), undo_before);

    let (mut ordinary, viewport) = fixture(0x8300_8732, 1.0);
    let point = fixture_point(&ordinary, "point.a");
    let scene = ordinary.scene(viewport, 0.5).expect("ordinary scene");
    ordinary
        .pointer_down_exact_point(
            &scene,
            pointer(pointer_id, viewport.model_to_screen([0.0, 0.0])),
            point,
        )
        .expect("ordinary point route");
    ordinary
        .pointer_move(
            &scene,
            pointer(pointer_id, viewport.model_to_screen(target)),
        )
        .expect("ordinary accepted preview");
    let preview_scene = ordinary
        .scene(viewport, 0.5)
        .expect("ordinary preview scene");
    let terminal = ordinary.pointer_up_audited(
        &preview_scene,
        pointer(pointer_id, viewport.model_to_screen(target)),
    );
    assert_eq!(terminal.work.intent_materialization_attempts(), 1);
    assert_eq!(terminal.work.history_publications(), 1);
    assert!(
        terminal
            .outcome
            .expect("ordinary terminal")
            .transaction
            .is_some()
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
    let intent_before = session.coordinator().intent().identity();
    let undo_before = session.coordinator().intent().undo_len();

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
    let terminal = session
        .pointer_up(&preview_scene, pointer(8303, target))
        .expect("publish prepared radius");
    assert!(terminal.delegated_computed_fillet_radius.is_none());
    let outcome = terminal.transaction.expect("accepted radius transaction");

    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        session.coordinator().intent().identity().revision.raw(),
        intent_before.revision.raw() + 1,
    );
    assert_eq!(session.coordinator().intent().undo_len(), undo_before + 1);
    assert_one_preview_plan_and_no_terminal_replan(before_drop);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the grouped-delegation regression keeps every consumer preview and the no-inner-transaction terminal contract in one owning-layer case"
)]
fn delegated_grouped_fillet_radius_previews_every_consumer_and_returns_no_inner_transaction() {
    let (mut session, viewport, features) = two_fillet_fixture(0x8700_0001, [0.5, 0.5]);
    let pointer_id = 87_001;
    let (scene, target) =
        radius_drag_target(&mut session, viewport, features[0], pointer_id, 0.125);
    session
        .delegate_computed_fillet_radius_drag(&[features[1], features[0]])
        .expect("authenticate shared radius consumer group");
    let intent_before = session.coordinator().intent().identity();
    let undo_before = session.coordinator().intent().undo_len();

    reset_plan_patch_calls();
    let intermediate = ScreenPoint {
        x: 0.5 * (scene.fillet_affordances[0].radius_rail.screen_grip.x + target.x),
        y: 0.5 * (scene.fillet_affordances[0].radius_rail.screen_grip.y + target.y),
    };
    let mut proposed_radius = None;
    for (frame, sample) in [intermediate, target].into_iter().enumerate() {
        let effects = session
            .pointer_move(&scene, pointer(pointer_id, sample))
            .expect("prepare grouped radius preview frame");
        proposed_radius = effects.iter().find_map(|effect| match effect {
            EditorEffect::PreviewComputedFeatureRadius {
                feature, radius, ..
            } if *feature == features[0] => Some(*radius),
            _ => None,
        });
        assert!(
            proposed_radius.is_some(),
            "preview frame {frame} must acknowledge the initiating feature",
        );
        let preview_scene = session
            .scene(viewport, 0.5)
            .expect("grouped radius preview scene");
        let proposed_radius = proposed_radius.expect("checked above");
        for feature in features {
            let radii = preview_scene
                .computed_curves
                .iter()
                .filter(|curve| curve.owner.feature == feature)
                .map(|curve| curve.radius)
                .collect::<Vec<_>>();
            assert!(!radii.is_empty(), "feature {feature:?} must remain visible");
            assert!(radii.iter().all(|radius| {
                radius.is_finite() && radius.to_bits() == proposed_radius.to_bits()
            }));
        }
        assert_eq!(
            plan_patch_calls(),
            frame + 1,
            "each pointer frame plans the complete group exactly once",
        );
    }
    let proposed_radius = proposed_radius.expect("terminal preview radius");
    let preview_scene = session
        .scene(viewport, 0.5)
        .expect("grouped radius preview scene");
    for feature in features {
        let radii = preview_scene
            .computed_curves
            .iter()
            .filter(|curve| curve.owner.feature == feature)
            .map(|curve| curve.radius)
            .collect::<Vec<_>>();
        assert!(!radii.is_empty(), "feature {feature:?} must remain visible");
        assert!(
            radii.iter().all(|radius| {
                radius.is_finite() && radius.to_bits() == proposed_radius.to_bits()
            })
        );
    }

    let plans_before_release = plan_patch_calls();
    let terminal = session
        .pointer_up(&preview_scene, pointer(pointer_id, target))
        .expect("delegate grouped radius release");
    assert!(terminal.transaction.is_none());
    let proposal = terminal
        .delegated_computed_fillet_radius
        .expect("authenticated outer-owner proposal");
    assert_eq!(proposal.intent, intent_before);
    assert_eq!(proposal.initiating_feature, features[0]);
    assert_eq!(proposal.features, features);
    assert_eq!(proposal.origin_radius.to_bits(), 0.5_f64.to_bits());
    assert_eq!(
        proposal.proposed_radius.to_bits(),
        proposed_radius.to_bits()
    );
    assert_eq!(plan_patch_calls(), plans_before_release);
    assert_eq!(session.coordinator().intent().identity(), intent_before);
    assert_eq!(session.coordinator().intent().undo_len(), undo_before);
    for feature in features {
        assert_eq!(
            accepted_fillet_radius(&session, feature).to_bits(),
            0.5_f64.to_bits(),
            "delegated release must not publish nested feature intent",
        );
    }
    let restored = session
        .scene(viewport, 0.5)
        .expect("accepted scene after delegated release");
    assert!(restored.computed_curves.iter().all(|curve| {
        curve.center.into_iter().all(f64::is_finite) && curve.radius.to_bits() == 0.5_f64.to_bits()
    }));
}

#[test]
fn delegated_grouped_fillet_radius_cancel_restores_every_consumer_without_history() {
    let (mut session, viewport, features) = two_fillet_fixture(0x8700_0004, [0.5, 0.5]);
    let pointer_id = 87_004;
    let (scene, target) =
        radius_drag_target(&mut session, viewport, features[0], pointer_id, 0.125);
    session
        .delegate_computed_fillet_radius_drag(&features)
        .expect("authenticate cancellable shared radius group");
    let intent_before = session.coordinator().intent().identity();
    let undo_before = session.coordinator().intent().undo_len();

    session
        .pointer_move(&scene, pointer(pointer_id, target))
        .expect("preview cancellable grouped radius");
    let preview = session
        .scene(viewport, 0.5)
        .expect("grouped preview before cancellation");
    assert!(
        preview.computed_curves.iter().all(|curve| {
            curve.radius.is_finite() && curve.radius.to_bits() != 0.5_f64.to_bits()
        })
    );

    session.cancel_interaction();
    assert!(session.editor().active_pointer_gesture().is_none());
    assert_eq!(session.coordinator().intent().identity(), intent_before);
    assert_eq!(session.coordinator().intent().undo_len(), undo_before);
    for feature in features {
        assert_eq!(
            accepted_fillet_radius(&session, feature).to_bits(),
            0.5_f64.to_bits(),
        );
    }
    let restored = session
        .scene(viewport, 0.5)
        .expect("accepted grouped scene after cancellation");
    assert!(restored.computed_curves.iter().all(|curve| {
        curve.center.into_iter().all(f64::is_finite) && curve.radius.to_bits() == 0.5_f64.to_bits()
    }));
}

#[test]
fn stale_delegated_fillet_radius_group_rejects_and_retains_accepted_authority() {
    let (mut session, viewport, features) = two_fillet_fixture(0x8700_0002, [0.5, 0.5]);
    let pointer_id = 87_002;
    let (_scene, _target) =
        radius_drag_target(&mut session, viewport, features[0], pointer_id, 0.125);
    let intent_before = session.coordinator().intent().identity();
    let undo_before = session.coordinator().intent().undo_len();
    let stale = ComputedFeatureId::from_raw(u64::MAX);

    assert!(matches!(
        session.delegate_computed_fillet_radius_drag(&[features[0], stale]),
        Err(ProjectionalEditorError::StaleDelegatedFilletRadiusFeature(feature))
            if feature == stale
    ));
    assert!(session.editor().active_pointer_gesture().is_none());
    assert_eq!(session.coordinator().intent().identity(), intent_before);
    assert_eq!(session.coordinator().intent().undo_len(), undo_before);
    for feature in features {
        assert_eq!(
            accepted_fillet_radius(&session, feature).to_bits(),
            0.5_f64.to_bits()
        );
    }
}

#[test]
fn mixed_delegated_fillet_radius_group_rejects_and_retains_accepted_authority() {
    let (mut session, viewport, features) = two_fillet_fixture(0x8700_0003, [0.5, 0.75]);
    let pointer_id = 87_003;
    let (_scene, _target) =
        radius_drag_target(&mut session, viewport, features[0], pointer_id, 0.125);
    let intent_before = session.coordinator().intent().identity();
    let undo_before = session.coordinator().intent().undo_len();

    assert!(matches!(
        session.delegate_computed_fillet_radius_drag(&features),
        Err(ProjectionalEditorError::MixedDelegatedFilletRadiusFeature {
            feature,
            expected,
            actual,
        }) if feature == features[1]
            && expected.to_bits() == 0.5_f64.to_bits()
            && actual.to_bits() == 0.75_f64.to_bits()
    ));
    assert!(session.editor().active_pointer_gesture().is_none());
    assert_eq!(session.coordinator().intent().identity(), intent_before);
    assert_eq!(session.coordinator().intent().undo_len(), undo_before);
    assert_eq!(
        accepted_fillet_radius(&session, features[0]).to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(
        accepted_fillet_radius(&session, features[1]).to_bits(),
        0.75_f64.to_bits()
    );
}

#[test]
fn malformed_delegated_fillet_radius_groups_fail_closed_before_pointer_work() {
    let assert_retained = |session: &ProjectionalEditorSession,
                           features: [ComputedFeatureId; 2],
                           identity: IntentSessionIdentity,
                           undo_len: usize| {
        assert!(session.editor().active_pointer_gesture().is_none());
        assert_eq!(session.coordinator().intent().identity(), identity);
        assert_eq!(session.coordinator().intent().undo_len(), undo_len);
        for feature in features {
            assert_eq!(
                accepted_fillet_radius(session, feature).to_bits(),
                0.5_f64.to_bits(),
            );
        }
    };

    let cases = ["empty", "duplicate", "missing", "over_limit"];
    for (index, case) in cases.into_iter().enumerate() {
        let raw = 0x8700_0100 + index as u128;
        let (mut session, viewport, features) = two_fillet_fixture(raw, [0.5, 0.5]);
        let pointer_id = 87_100 + index as u64;
        let (_scene, _target) =
            radius_drag_target(&mut session, viewport, features[0], pointer_id, 0.125);
        let identity = session.coordinator().intent().identity();
        let undo_len = session.coordinator().intent().undo_len();

        let error = match case {
            "empty" => session
                .delegate_computed_fillet_radius_drag(&[])
                .expect_err("empty groups must reject"),
            "duplicate" => session
                .delegate_computed_fillet_radius_drag(&[features[0], features[1], features[1]])
                .expect_err("duplicate features must reject"),
            "missing" => session
                .delegate_computed_fillet_radius_drag(&[features[1]])
                .expect_err("the initiating feature is mandatory"),
            "over_limit" => {
                let oversized =
                    vec![features[0]; MAX_DELEGATED_COMPUTED_FILLET_RADIUS_FEATURES + 1];
                session
                    .delegate_computed_fillet_radius_drag(&oversized)
                    .expect_err("over-limit groups must reject before canonicalization")
            }
            _ => unreachable!(),
        };
        match case {
            "empty" => assert!(matches!(
                error,
                ProjectionalEditorError::EmptyDelegatedFilletRadiusGroup
            )),
            "duplicate" => assert!(matches!(
                error,
                ProjectionalEditorError::DuplicateDelegatedFilletRadiusFeature(feature)
                    if feature == features[1]
            )),
            "missing" => assert!(matches!(
                error,
                ProjectionalEditorError::DelegatedFilletRadiusGroupMissingInitiatingFeature(feature)
                    if feature == features[0]
            )),
            "over_limit" => assert!(matches!(
                error,
                ProjectionalEditorError::DelegatedFilletRadiusGroupCardinalityExceeded {
                    count,
                    limit: MAX_DELEGATED_COMPUTED_FILLET_RADIUS_FEATURES,
                } if count == MAX_DELEGATED_COMPUTED_FILLET_RADIUS_FEATURES + 1
            )),
            _ => unreachable!(),
        }
        assert_retained(&session, features, identity, undo_len);
    }
}

#[test]
fn delegated_fillet_radius_group_rejects_late_or_conflicting_rebinding() {
    let (mut late, viewport, features) = two_fillet_fixture(0x8700_0200, [0.5, 0.5]);
    let (scene, target) = radius_drag_target(&mut late, viewport, features[0], 87_200, 0.125);
    late.pointer_move(&scene, pointer(87_200, target))
        .expect("ordinary pointer work before late delegation");
    assert!(matches!(
        late.delegate_computed_fillet_radius_drag(&features),
        Err(ProjectionalEditorError::DelegatedFilletRadiusGroupTooLate)
    ));
    assert!(late.editor().active_pointer_gesture().is_none());

    let (mut rebound, viewport, features) = two_fillet_fixture(0x8700_0201, [0.5, 0.5]);
    let (_scene, _target) = radius_drag_target(&mut rebound, viewport, features[0], 87_201, 0.125);
    rebound
        .delegate_computed_fillet_radius_drag(&features)
        .expect("first exact group binding");
    rebound
        .delegate_computed_fillet_radius_drag(&features)
        .expect("idempotent exact group rebinding");
    assert!(matches!(
        rebound.delegate_computed_fillet_radius_drag(&[features[0]]),
        Err(ProjectionalEditorError::DelegatedFilletRadiusGroupAlreadyBound)
    ));
    assert!(rebound.editor().active_pointer_gesture().is_none());
}

#[test]
fn grouped_fillet_radius_patch_rejects_repeated_logical_definition_owners() {
    let (session, _viewport, features) = two_fillet_fixture(0x8700_0202, [0.5, 0.5]);
    let accepted = session
        .coordinator()
        .accepted_materialization()
        .expect("accepted two-Fillet authority");
    let mut ownership = accepted.ownership.clone();
    let first_owner = ownership
        .exact_owner(IntentNativeBinding::ComputedFeature(features[0]))
        .expect("first feature owner");
    let second_owner = ownership
        .exact_owner(IntentNativeBinding::ComputedFeature(features[1]))
        .expect("second feature owner");
    ownership
        .nodes
        .iter_mut()
        .find(|node| node.node == second_owner)
        .expect("second owner materialization")
        .owned
        .retain(|binding| *binding != IntentNativeBinding::ComputedFeature(features[1]));
    let first = ownership
        .nodes
        .iter_mut()
        .find(|node| node.node == first_owner)
        .expect("first owner materialization");
    first
        .owned
        .push(IntentNativeBinding::ComputedFeature(features[1]));
    first.owned.sort_unstable();

    assert!(matches!(
        projectional_fillet_radius_group_patch(
            session.coordinator().intent(),
            &ownership,
            &features,
            0.5,
        ),
        Err(ProjectionalEditorError::DuplicateDelegatedFilletRadiusDefinitionOwner(feature))
            if feature == features[1]
    ));
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
