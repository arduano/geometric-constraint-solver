// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use geosolve_constraint_editor::{
    ActivePointerGestureKind, ColdIntentMaterializer, EditorEffect, IntentNativeBinding, Modifiers,
    OffsetAuthoringOutcome, OffsetAuthoringState, OffsetAuthoringTarget,
    ProfileOffsetDirectionState, ProjectionalEditorError, ProjectionalEditorSession,
    ProjectionalIntentCoordinator, ProjectionalProfileOffsetError, ScreenPoint, Viewport,
};
use geosolve_sketch::{
    CurveSpan, DocumentDimensionDefinition, DocumentDimensionId, DocumentId,
    DocumentProfileOffsetOperand, OperationControl, OperationOutcome, PersistentId,
};
use geosolve_sketch_intent::{
    AggregateKind, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentKey,
    IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPlanDisposition, IntentPortRole, IntentPortSelector, IntentSessionId,
    IntentUnit, LeafField, OperationKind, PatchPortRef,
};
use geosolve_sketch_topology::{OffsetOperandRequest, PreparedOffsetOperandQuery};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
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

fn fixture(closed: bool, raw: u128) -> ProjectionalEditorSession {
    let mut coordinator = ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let mut operations = vec![
        create("a", point("point.a", [0.0, 0.0])),
        create("b", point("point.b", [4.0, 0.0])),
        create("c", point("point.c", [4.0, 3.0])),
        create("bottom", line("line.bottom", "a", "b", [1.0, 0.0])),
        create("right", line("line.right", "b", "c", [0.0, 1.0])),
    ];
    if closed {
        operations.extend([
            create("d", point("point.d", [0.0, 3.0])),
            create("top", line("line.top", "c", "d", [-1.0, 0.0])),
            create("left", line("line.left", "d", "a", [0.0, -1.0])),
        ]);
    }
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    ProjectionalEditorSession::new(coordinator)
}

fn offset_state(session: &ProjectionalEditorSession, face: bool) -> OffsetAuthoringState {
    let native = session.coordinator().presentation_session().unwrap();
    let query =
        PreparedOffsetOperandQuery::capture(native, OffsetOperandRequest::default()).unwrap();
    let OperationOutcome::Completed { value, .. } =
        query.execute(OperationControl::unlimited()).unwrap()
    else {
        panic!("offset topology query stopped")
    };
    let index = Arc::new(value.operand_index.unwrap());
    let mut state = OffsetAuthoringState::default();
    assert!(matches!(
        state.activate(index.clone(), 1.0),
        OffsetAuthoringOutcome::ModeEntered(_)
    ));
    if face {
        let face = index.faces()[0].key.clone();
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Face(face)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    } else {
        let document = native.design_document();
        for label in ["line.bottom", "line.right"] {
            let curve = document
                .curves()
                .iter()
                .find(|curve| curve.label == label)
                .unwrap();
            let span = document.curve_spans(curve.id).unwrap()[0];
            assert!(matches!(
                state.pick_target(OffsetAuthoringTarget::Span(span)),
                OffsetAuthoringOutcome::OperandChanged { .. }
            ));
        }
    }
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
        .unwrap()
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

fn profile_offset_node(session: &ProjectionalEditorSession) -> geosolve_sketch_intent::NodeId {
    let nodes = session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .filter(|node| {
            matches!(
                node.kind,
                IntentNodeKind::Operation {
                    operation: OperationKind::ProfileOffset
                }
            )
        })
        .map(|node| node.id)
        .collect::<Vec<_>>();
    let [node] = nodes.as_slice() else {
        panic!("exactly one Profile Offset declaration expected")
    };
    *node
}

fn assert_independently_valid(session: &ProjectionalEditorSession) {
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

const fn pointer(
    pointer_id: u64,
    position: ScreenPoint,
) -> geosolve_constraint_editor::PointerInput {
    geosolve_constraint_editor::PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers {
            shift: false,
            control: false,
            command: false,
        },
    }
}

fn offset_drag_geometry(
    session: &ProjectionalEditorSession,
    dimension: DocumentDimensionId,
    viewport: Viewport,
) -> (
    geosolve_constraint_editor::EditorScene,
    ScreenPoint,
    [f64; 2],
) {
    let document = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let definition = &document.dimension(dimension).unwrap().definition;
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = definition else {
        unreachable!()
    };
    let edge = match operand {
        DocumentProfileOffsetOperand::Face { outer, .. } => outer.edges[0],
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => chain.edges[0],
    };
    let source = document.evaluate_curve_jet(edge.source.curve, 0.5).unwrap();
    let target = document.evaluate_curve_jet(edge.target.curve, 0.5).unwrap();
    let normal = source.differential().unwrap().left_normal;
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
    let scene = session.scene(viewport, 0.5).unwrap();
    let curve = scene
        .curves
        .iter()
        .find(|curve| curve.span == edge.target.curve)
        .unwrap();
    let first = curve.screen_polyline.first().unwrap();
    let last = curve.screen_polyline.last().unwrap();
    let press = ScreenPoint {
        x: 0.5 * (first.x + last.x),
        y: 0.5 * (first.y + last.y),
    };
    (scene, press, derivative)
}

#[test]
fn face_offset_is_one_transaction_with_exact_properties_and_stable_native_identity() {
    let mut session = fixture(true, 0x8300_0ff5_0001);
    let mut state = offset_state(&session, true);
    let history_before = session.coordinator().intent().undo_len();
    let outcome = session
        .apply_profile_offset(&mut state, key("offset.face"))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    assert!(state.operand().is_none());

    let operation = session
        .coordinator()
        .intent()
        .graph()
        .node(profile_offset_node(&session))
        .unwrap();
    assert_eq!(
        operation.fields[&IntentFieldKey(key("distance"))],
        coordinate(0.5)
    );
    assert_eq!(
        operation.fields[&IntentFieldKey(key("direction"))],
        IntentLiteral::Enum(key("outward"))
    );
    assert_eq!(
        session
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .filter(|node| matches!(node.kind, IntentNodeKind::Aggregate { .. }))
            .count(),
        1
    );
    let dimension = profile_offset_dimension(&session);
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let binding_owner_count = accepted
        .ownership
        .nodes
        .iter()
        .filter(|node| {
            node.owned
                .contains(&IntentNativeBinding::Dimension(dimension))
        })
        .count();
    assert_eq!(binding_owner_count, 1);
    assert_eq!(accepted.session.design_document().curves().len(), 8);
    assert_independently_valid(&session);

    let target_before = match &accepted
        .session
        .design_document()
        .dimension(dimension)
        .unwrap()
        .definition
    {
        DocumentDimensionDefinition::ProfileOffset { target, .. } => *target,
        _ => unreachable!(),
    };
    assert_eq!(
        session
            .edit_profile_offset_distance(dimension, 0.75)
            .unwrap()
            .disposition,
        IntentPlanDisposition::Accepted
    );
    assert_eq!(profile_offset_dimension(&session), dimension);
    let document = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let DocumentDimensionDefinition::ProfileOffset { target, .. } =
        &document.dimension(dimension).unwrap().definition
    else {
        unreachable!()
    };
    assert_eq!(*target, target_before);
    assert_eq!(
        document.scalar(*target).unwrap().value.to_bits(),
        0.75_f64.to_bits()
    );
    assert_eq!(
        session
            .edit_profile_offset_direction(dimension, ProfileOffsetDirectionState::Inward)
            .unwrap()
            .disposition,
        IntentPlanDisposition::Accepted
    );
    assert_eq!(profile_offset_dimension(&session), dimension);
    assert_independently_valid(&session);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle fixture keeps Offset deletion, dependent closure, Undo, Redo, and exact output identity evidence contiguous"
)]
fn open_chain_delete_cascades_downstream_and_undo_redo_restore_exact_outputs() {
    let mut session = fixture(false, 0x8300_0ff5_0002);
    let mut state = offset_state(&session, false);
    session
        .apply_profile_offset(&mut state, key("offset.chain"))
        .unwrap();
    let dimension = profile_offset_dimension(&session);
    let operation = profile_offset_node(&session);
    let operand_aggregates = session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .filter(|node| matches!(node.kind, IntentNodeKind::Aggregate { .. }))
        .map(|node| node.id)
        .collect::<Vec<_>>();
    assert_eq!(operand_aggregates.len(), 1);
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let output_bindings = accepted
        .ownership
        .nodes
        .iter()
        .find(|node| node.node == operation)
        .unwrap()
        .owned
        .clone();
    let target_span = output_bindings
        .iter()
        .find_map(|binding| match binding {
            IntentNativeBinding::Curve(curve) => Some(CurveSpan::line(*curve)),
            _ => None,
        })
        .unwrap();
    let target_port = accepted
        .ownership
        .ports
        .iter()
        .find_map(|(port, binding)| {
            (*binding == IntentNativeBinding::CurveSpan(target_span)).then_some(*port)
        })
        .unwrap();
    let downstream = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("downstream.target.chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: target_port },
    );
    let outcome = session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![create("downstream", downstream)],
        ))
        .unwrap();
    let downstream = outcome.aliases.node(&key("downstream")).unwrap();
    let history_before_delete = session.coordinator().intent().undo_len();
    session.delete_profile_offset(dimension).unwrap();
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before_delete + 1
    );
    assert!(
        session
            .coordinator()
            .intent()
            .graph()
            .node(operation)
            .is_none()
    );
    assert!(
        session
            .coordinator()
            .intent()
            .graph()
            .node(downstream)
            .is_none()
    );
    assert!(operand_aggregates.iter().all(|aggregate| {
        session
            .coordinator()
            .intent()
            .graph()
            .node(*aggregate)
            .is_none()
    }));
    assert!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .dimension(dimension)
            .is_none()
    );

    session.undo().unwrap().unwrap();
    assert_eq!(profile_offset_dimension(&session), dimension);
    let restored = session.coordinator().accepted_materialization().unwrap();
    let restored_outputs = restored
        .ownership
        .nodes
        .iter()
        .find(|node| node.node == operation)
        .unwrap()
        .owned
        .clone();
    assert_eq!(restored_outputs, output_bindings);
    assert!(
        session
            .coordinator()
            .intent()
            .graph()
            .node(downstream)
            .is_some()
    );
    assert!(operand_aggregates.iter().all(|aggregate| {
        session
            .coordinator()
            .intent()
            .graph()
            .node(*aggregate)
            .is_some()
    }));
    session.redo().unwrap().unwrap();
    assert!(
        session
            .coordinator()
            .intent()
            .graph()
            .node(operation)
            .is_none()
    );
    assert!(
        session
            .coordinator()
            .intent()
            .graph()
            .node(downstream)
            .is_none()
    );
    assert!(operand_aggregates.iter().all(|aggregate| {
        session
            .coordinator()
            .intent()
            .graph()
            .node(*aggregate)
            .is_none()
    }));
}

#[test]
fn face_offset_delete_removes_every_typed_loop_operand_and_undo_restores_them() {
    let mut session = fixture(true, 0x8300_0ff5_0004);
    let mut state = offset_state(&session, true);
    session
        .apply_profile_offset(&mut state, key("offset.face.delete"))
        .unwrap();
    let dimension = profile_offset_dimension(&session);
    let operation = profile_offset_node(&session);
    let aggregates = session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .filter(|node| matches!(node.kind, IntentNodeKind::Aggregate { .. }))
        .map(|node| node.id)
        .collect::<Vec<_>>();
    assert_eq!(aggregates.len(), 1);

    session.delete_profile_offset(dimension).unwrap();
    let graph = session.coordinator().intent().graph();
    assert!(graph.node(operation).is_none());
    assert!(aggregates.iter().all(|node| graph.node(*node).is_none()));
    assert!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .dimension(dimension)
            .is_none()
    );

    session.undo().unwrap().unwrap();
    let graph = session.coordinator().intent().graph();
    assert!(graph.node(operation).is_some());
    assert!(aggregates.iter().all(|node| graph.node(*node).is_some()));
    assert_eq!(profile_offset_dimension(&session), dimension);
    assert_independently_valid(&session);
}

#[test]
fn stale_candidates_and_invalid_properties_change_no_history_or_authority() {
    let mut session = fixture(false, 0x8300_0ff5_0003);
    let mut stale = offset_state(&session, false);
    let history_before = session.coordinator().intent().undo_len();
    session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![create("unrelated", point("point.unrelated", [10.0, 10.0]))],
        ))
        .unwrap();
    let accepted_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    assert!(matches!(
        session.apply_profile_offset(&mut stale, key("offset.stale")),
        Err(ProjectionalEditorError::ProfileOffset(
            ProjectionalProfileOffsetError::StaleCandidate
        ))
    ));
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        accepted_before
    );

    let mut invalid = offset_state(&session, false);
    assert!(matches!(
        invalid.set_distance(0.0),
        OffsetAuthoringOutcome::Warning(_)
    ));
    assert!(matches!(
        session.apply_profile_offset(&mut invalid, key("offset.invalid")),
        Err(ProjectionalEditorError::ProfileOffset(
            ProjectionalProfileOffsetError::IncompleteCandidate
        ))
    ));
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );

    let mut valid = offset_state(&session, false);
    session
        .apply_profile_offset(&mut valid, key("offset.valid"))
        .unwrap();
    let dimension = profile_offset_dimension(&session);
    let history_after_apply = session.coordinator().intent().undo_len();
    assert!(matches!(
        session.edit_profile_offset_distance(dimension, f64::NAN),
        Err(ProjectionalEditorError::ProfileOffset(
            ProjectionalProfileOffsetError::InvalidDistance
        ))
    ));
    assert!(matches!(
        session.edit_profile_offset_direction(dimension, ProfileOffsetDirectionState::Outward),
        Err(ProjectionalEditorError::ProfileOffset(
            ProjectionalProfileOffsetError::DirectionFamilyMismatch
        ))
    ));
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_after_apply
    );
    assert_independently_valid(&session);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps preview, exact publication, independent validity, and Undo evidence contiguous"
)]
fn accepted_offset_drag_previews_without_history_then_commits_once_and_undoes() {
    let mut session = fixture(false, 0x8300_0ff5_0010);
    let mut state = offset_state(&session, false);
    session
        .apply_profile_offset(&mut state, key("offset.drag"))
        .unwrap();
    let dimension = profile_offset_dimension(&session);
    let viewport = Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).unwrap();
    let (scene, press, derivative) = offset_drag_geometry(&session, dimension, viewport);
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let evidence_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();

    session.pointer_down(&scene, pointer(81, press)).unwrap();
    assert_eq!(
        session.editor().active_pointer_gesture().unwrap().kind,
        ActivePointerGestureKind::OffsetDistance
    );
    let origin = viewport.screen_to_model(press);
    let target = viewport.model_to_screen([
        0.25f64.mul_add(derivative[0], origin[0]),
        0.25f64.mul_add(derivative[1], origin[1]),
    ]);
    let effects = session.pointer_move(&scene, pointer(81, target)).unwrap();
    effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::PreviewAcceptedProfileOffsetDistance { distance, .. } => Some(*distance),
            _ => None,
        })
        .expect("the finite accepted Offset sample must cold-materialize");
    let target = viewport.model_to_screen([
        0.4f64.mul_add(derivative[0], origin[0]),
        0.4f64.mul_add(derivative[1], origin[1]),
    ]);
    let distance = session
        .pointer_move(&scene, pointer(81, target))
        .unwrap()
        .into_iter()
        .find_map(|effect| match effect {
            EditorEffect::PreviewAcceptedProfileOffsetDistance { distance, .. } => Some(distance),
            _ => None,
        })
        .expect("the newer accepted Offset sample must replace the prior preview");
    assert_eq!(distance.to_bits(), 0.9_f64.to_bits());
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before
    );

    let preview_scene = session.scene(viewport, 0.5).unwrap();
    let outcome = session
        .pointer_up(&preview_scene, pointer(81, target))
        .unwrap();
    assert_eq!(
        outcome.transaction.unwrap().disposition,
        IntentPlanDisposition::Accepted
    );
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let DocumentDimensionDefinition::ProfileOffset { target, .. } = accepted
        .session
        .design_document()
        .dimension(dimension)
        .unwrap()
        .definition
    else {
        unreachable!()
    };
    assert_eq!(
        accepted
            .session
            .design_document()
            .scalar(target)
            .unwrap()
            .value
            .to_bits(),
        distance.to_bits()
    );
    assert_independently_valid(&session);

    session.undo().unwrap().unwrap();
    let undone = session.coordinator().accepted_materialization().unwrap();
    let DocumentDimensionDefinition::ProfileOffset { target, .. } = undone
        .session
        .design_document()
        .dimension(dimension)
        .unwrap()
        .definition
    else {
        unreachable!()
    };
    assert_eq!(
        undone
            .session
            .design_document()
            .scalar(target)
            .unwrap()
            .value
            .to_bits(),
        0.5_f64.to_bits()
    );
}

#[test]
fn accepted_offset_cancel_invalid_and_stale_releases_publish_nothing() {
    let mut session = fixture(false, 0x8300_0ff5_0011);
    let mut state = offset_state(&session, false);
    session
        .apply_profile_offset(&mut state, key("offset.drag.rollback"))
        .unwrap();
    let dimension = profile_offset_dimension(&session);
    let viewport = Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).unwrap();
    let (scene, press, derivative) = offset_drag_geometry(&session, dimension, viewport);
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let evidence_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let origin = viewport.screen_to_model(press);
    let valid = viewport.model_to_screen([
        0.2f64.mul_add(derivative[0], origin[0]),
        0.2f64.mul_add(derivative[1], origin[1]),
    ]);

    session.pointer_down(&scene, pointer(82, press)).unwrap();
    session.pointer_move(&scene, pointer(82, valid)).unwrap();
    assert!(
        session
            .cancel_interaction()
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearAcceptedProfileOffsetPreview))
    );
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);

    let scene = session.scene(viewport, 0.5).unwrap();
    session.pointer_down(&scene, pointer(83, press)).unwrap();
    session.pointer_move(&scene, pointer(83, valid)).unwrap();
    assert!(matches!(
        session.pointer_up(&scene, pointer(83, valid)),
        Err(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch)
    ));
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);

    let scene = session.scene(viewport, 0.5).unwrap();
    session.pointer_down(&scene, pointer(84, press)).unwrap();
    let invalid = viewport.model_to_screen([
        (-1.0f64).mul_add(derivative[0], origin[0]),
        (-1.0f64).mul_add(derivative[1], origin[1]),
    ]);
    assert!(
        session
            .pointer_move(&scene, pointer(84, invalid))
            .unwrap()
            .is_empty()
    );
    let outcome = session.pointer_up(&scene, pointer(84, invalid)).unwrap();
    assert!(outcome.transaction.is_none());
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before
    );
    assert_independently_valid(&session);

    let scene = session.scene(viewport, 0.5).unwrap();
    session.pointer_down(&scene, pointer(85, press)).unwrap();
    let nonfinite = ScreenPoint {
        x: press.x,
        y: f64::INFINITY,
    };
    let outcome = session.pointer_up(&scene, pointer(85, nonfinite)).unwrap();
    assert!(outcome.transaction.is_none());
    assert!(session.editor().active_pointer_gesture().is_none());
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
}
