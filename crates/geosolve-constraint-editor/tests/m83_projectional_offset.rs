// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::BTreeSet, sync::Arc};

use geosolve_constraint_editor::{
    ActivePointerGestureKind, ColdIntentMaterializer, EditorEffect, GeometryInteractionPolicy,
    IntentNativeBinding, Modifiers, OffsetAuthoringOutcome, OffsetAuthoringState,
    OffsetAuthoringTarget, PickTolerance, ProfileOffsetDirectionState, ProjectionalEditorError,
    ProjectionalEditorSession, ProjectionalIntentCoordinator, ProjectionalProfileOffsetError,
    ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveSpan, DocumentDimensionDefinition, DocumentDimensionId, DocumentId,
    DocumentProfileOffsetOperand, OperationControl, OperationOutcome, PersistentId,
    RetainedSketchDocumentSession, SketchHardValidity,
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

fn assert_native_session_independently_valid(session: &RetainedSketchDocumentSession) {
    let accepted = session
        .accepted_state_for_current_input()
        .expect("current accepted native geometry");
    let solve = accepted.solve_result();
    let report = solve.unstable_core_report();
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(
        report.hard_residual_max.is_finite() && report.hard_residual_max <= 1.0e-9,
        "{report:#?}"
    );
    assert!(
        solve
            .acceptance_hard_residual_max
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        "{solve:#?}"
    );
    let diagnostics = accepted.diagnostics();
    let independent = diagnostics.solve.expect("accepted solve diagnostics");
    assert_eq!(independent.hard_validity, SketchHardValidity::Valid);
    assert!(independent.hard_residuals_validated);
    assert!(
        independent
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        "{independent:#?}"
    );

    let document = accepted.document();
    assert!(document.model_scale().is_finite() && document.model_scale() > 0.0);
    assert!(
        document
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    for curve in document.curves() {
        for span in document.curve_spans(curve.id).unwrap() {
            for parameter in [0.0, 0.5, 1.0] {
                let jet = document.evaluate_curve_jet(span, parameter).unwrap();
                assert!(
                    [
                        jet.position.x,
                        jet.position.y,
                        jet.first_derivative.x,
                        jet.first_derivative.y,
                        jet.second_derivative.x,
                        jet.second_derivative.y,
                        jet.third_derivative.x,
                        jet.third_derivative.y,
                    ]
                    .into_iter()
                    .all(f64::is_finite),
                    "non-finite accepted jet for {span:?} at {parameter}: {jet:?}"
                );
            }
        }
    }
}

fn native_span_by_label(session: &ProjectionalEditorSession, label: &str) -> CurveSpan {
    let document = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let curve = document
        .curves()
        .iter()
        .find(|curve| curve.label == label)
        .unwrap();
    document.curve_spans(curve.id).unwrap()[0]
}

fn provisional_inventory_from_document_delta(
    accepted: &geosolve_sketch::SketchDocument,
    preview: &geosolve_sketch::SketchDocument,
) -> Vec<SelectionItem> {
    let accepted_points = accepted
        .points()
        .iter()
        .map(|point| point.id)
        .collect::<BTreeSet<_>>();
    let accepted_curves = accepted
        .curves()
        .iter()
        .map(|curve| curve.id)
        .collect::<BTreeSet<_>>();
    let accepted_constraints = accepted
        .constraints()
        .iter()
        .map(|constraint| constraint.id)
        .collect::<BTreeSet<_>>();
    let accepted_dimensions = accepted
        .dimensions()
        .iter()
        .map(|dimension| dimension.id)
        .collect::<BTreeSet<_>>();

    let mut items = preview
        .points()
        .iter()
        .filter(|point| !accepted_points.contains(&point.id))
        .map(|point| SelectionItem::Point(point.id))
        .chain(
            preview
                .curves()
                .iter()
                .filter(|curve| !accepted_curves.contains(&curve.id))
                .flat_map(|curve| preview.curve_spans(curve.id).unwrap())
                .map(SelectionItem::Curve),
        )
        .chain(
            preview
                .constraints()
                .iter()
                .filter(|constraint| !accepted_constraints.contains(&constraint.id))
                .map(|constraint| SelectionItem::Constraint(constraint.id)),
        )
        .chain(
            preview
                .dimensions()
                .iter()
                .filter(|dimension| !accepted_dimensions.contains(&dimension.id))
                .map(|dimension| SelectionItem::Dimension(dimension.id)),
        )
        .collect::<Vec<_>>();
    items.sort_unstable();
    items.dedup();
    items
}

fn assert_same_scene_geometry(
    actual: &geosolve_constraint_editor::EditorScene,
    expected: &geosolve_constraint_editor::EditorScene,
) {
    assert_eq!(actual.viewport, expected.viewport);
    assert_eq!(actual.points, expected.points);
    assert_eq!(actual.curves, expected.curves);
    assert_eq!(actual.datums, expected.datums);
    assert_eq!(actual.computed_curves, expected.computed_curves);
    assert_eq!(actual.annotations, expected.annotations);
    assert_eq!(actual.constraint_entries, expected.constraint_entries);
}

fn assert_same_document_semantics(
    actual: &geosolve_sketch::SketchDocument,
    expected: &geosolve_sketch::SketchDocument,
) {
    assert_eq!(actual.id(), expected.id());
    assert_eq!(
        actual.model_scale().to_bits(),
        expected.model_scale().to_bits()
    );
    assert_eq!(actual.points(), expected.points());
    assert_eq!(actual.scalars(), expected.scalars());
    assert_eq!(actual.curves(), expected.curves());
    assert_eq!(actual.contacts(), expected.contacts());
    assert_eq!(actual.trim_views(), expected.trim_views());
    assert_eq!(actual.constraints(), expected.constraints());
    assert_eq!(actual.dimensions(), expected.dimensions());
    assert_eq!(actual.parameters(), expected.parameters());
    assert_eq!(actual.parameter_bindings(), expected.parameter_bindings());
    assert_eq!(actual.external_bindings(), expected.external_bindings());
    assert_eq!(actual.source_order(), expected.source_order());
    for curve in expected.curves() {
        assert_eq!(
            actual.geometry_role(curve.id),
            expected.geometry_role(curve.id)
        );
    }
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
    let document = session.presentation_session().unwrap().design_document();
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
#[allow(
    clippy::too_many_lines,
    reason = "one authoring regression keeps last-valid preview, history-free release, Apply, and Undo evidence contiguous"
)]
fn projectional_offset_authoring_distance_drag_is_history_free_until_apply() {
    let mut session = fixture(false, 0x8300_0ff5_0024);
    let viewport = Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).unwrap();
    let mut state = offset_state(&session, false);
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let intent_before = session.coordinator().intent().clone();
    let accepted_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .clone();

    assert!(
        session
            .refresh_offset_authoring_preview(&state, key("offset.authoring.drag"))
            .unwrap()
    );
    let dimension = session
        .offset_authoring_provisional_items()
        .iter()
        .find_map(|item| match item {
            SelectionItem::Dimension(dimension) => Some(*dimension),
            _ => None,
        })
        .expect("the cold Profile Offset preview owns one provisional dimension");
    let (scene, press, derivative) = offset_drag_geometry(&session, dimension, viewport);
    let origin = viewport.screen_to_model(press);
    assert!(
        session
            .pointer_down_offset_authoring_distance(
                &state,
                &scene,
                pointer(86, press),
                key("offset.authoring.drag"),
            )
            .unwrap()
            .is_some()
    );
    assert_eq!(
        session.editor().active_pointer_gesture().unwrap().kind,
        ActivePointerGestureKind::OffsetDistance
    );

    let first_target = viewport.model_to_screen([
        0.25f64.mul_add(derivative[0], origin[0]),
        0.25f64.mul_add(derivative[1], origin[1]),
    ]);
    let first_distance = session
        .pointer_move_offset_authoring_distance(&mut state, &scene, pointer(86, first_target))
        .unwrap()
        .into_iter()
        .find_map(|effect| match effect {
            EditorEffect::PreviewAcceptedProfileOffsetDistance { distance, .. } => Some(distance),
            _ => None,
        })
        .expect("the finite authoring sample must cold-materialize");
    assert_eq!(first_distance.to_bits(), 0.75_f64.to_bits());
    let last_valid_state = state.clone();
    let last_valid_json = session
        .presentation_session()
        .unwrap()
        .design_document()
        .to_draft_v5_json()
        .unwrap();

    let invalid_target = viewport.model_to_screen([
        (-1.0f64).mul_add(derivative[0], origin[0]),
        (-1.0f64).mul_add(derivative[1], origin[1]),
    ]);
    assert!(
        session
            .pointer_move_offset_authoring_distance(
                &mut state,
                &session.scene(viewport, 0.5).unwrap(),
                pointer(86, invalid_target),
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(state, last_valid_state);
    assert_eq!(
        session
            .presentation_session()
            .unwrap()
            .design_document()
            .to_draft_v5_json()
            .unwrap(),
        last_valid_json
    );

    let final_target = viewport.model_to_screen([
        0.4f64.mul_add(derivative[0], origin[0]),
        0.4f64.mul_add(derivative[1], origin[1]),
    ]);
    let latest_scene = session.scene(viewport, 0.5).unwrap();
    let final_distance = session
        .pointer_move_offset_authoring_distance(
            &mut state,
            &latest_scene,
            pointer(86, final_target),
        )
        .unwrap()
        .into_iter()
        .find_map(|effect| match effect {
            EditorEffect::PreviewAcceptedProfileOffsetDistance { distance, .. } => Some(distance),
            _ => None,
        })
        .expect("a newer valid sample must replace the held authoring preview");
    assert_eq!(final_distance.to_bits(), 0.9_f64.to_bits());
    assert!(
        session
            .pointer_up_offset_authoring_distance(
                &mut state,
                &session.scene(viewport, 0.5).unwrap(),
                pointer(86, final_target),
            )
            .unwrap()
    );
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        accepted_before.evidence
    );
    assert_native_session_independently_valid(session.presentation_session().unwrap());

    let outcome = session
        .apply_profile_offset_preview(&mut state, key("offset.authoring.drag"))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    assert_independently_valid(&session);

    session.undo().unwrap().unwrap();
    let undone = session.coordinator().accepted_materialization().unwrap();
    assert_same_document_semantics(
        undone.session.design_document(),
        accepted_before.session.design_document(),
    );
    assert_eq!(
        session.coordinator().intent().graph().nodes(),
        intent_before.graph().nodes()
    );
    assert_eq!(
        session.coordinator().intent().instance().values(),
        intent_before.instance().values()
    );
    assert_native_session_independently_valid(session.presentation_session().unwrap());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one public-boundary matrix compares face and ordered-chain semantic/canvas collection without hiding either lifecycle"
)]
fn projectional_offset_activation_and_semantic_canvas_picks_are_equivalent() {
    let viewport = Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).unwrap();
    let policy = GeometryInteractionPolicy::default();
    let tolerance = PickTolerance::default();

    let mut semantic_face = fixture(true, 0x8300_0ff5_0020);
    let intent_before = semantic_face.coordinator().intent().clone();
    let mut semantic_face_state = OffsetAuthoringState::default();
    assert!(matches!(
        semantic_face
            .activate_offset_authoring(&mut semantic_face_state)
            .unwrap(),
        OffsetAuthoringOutcome::ModeEntered(_)
    ));
    assert!(semantic_face_state.is_active());
    assert_eq!(
        semantic_face_state.index().unwrap().input(),
        semantic_face
            .presentation_session()
            .unwrap()
            .accepted_prepared_input()
            .unwrap()
    );
    assert_eq!(
        semantic_face_state.distance().unwrap().to_bits(),
        0.1_f64.to_bits()
    );
    assert!(
        semantic_face
            .offset_authoring_provisional_items()
            .is_empty()
    );
    assert_eq!(semantic_face.coordinator().intent(), &intent_before);

    let face = semantic_face_state.index().unwrap().faces()[0].key.clone();
    let mut canvas_face_state = semantic_face_state.clone();
    let scene = semantic_face.scene(viewport, 0.5).unwrap();
    assert!(matches!(
        semantic_face_state.pick_target(OffsetAuthoringTarget::Face(face.clone())),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    let position = viewport.model_to_screen([2.0, 1.5]);
    assert!(matches!(
        canvas_face_state.hover_at(&scene, position, tolerance, policy),
        OffsetAuthoringOutcome::HoverChanged(Some(ref hover))
            if hover.target == OffsetAuthoringTarget::Face(face.clone())
                && hover.availability.is_available()
    ));
    assert!(matches!(
        canvas_face_state.pick_at(&scene, position, tolerance, policy),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert_eq!(canvas_face_state.operand(), semantic_face_state.operand());
    assert_eq!(
        canvas_face_state.candidate(),
        semantic_face_state.candidate()
    );

    let mut semantic_chain = fixture(false, 0x8300_0ff5_0021);
    let mut semantic_chain_state = OffsetAuthoringState::default();
    semantic_chain
        .activate_offset_authoring(&mut semantic_chain_state)
        .unwrap();
    let bottom = native_span_by_label(&semantic_chain, "line.bottom");
    let right = native_span_by_label(&semantic_chain, "line.right");
    let mut canvas_chain_state = semantic_chain_state.clone();
    let scene = semantic_chain.scene(viewport, 0.5).unwrap();
    for span in [bottom, right] {
        assert!(matches!(
            semantic_chain_state.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    for (model, span) in [([2.0, 0.0], bottom), ([4.0, 1.5], right)] {
        let position = viewport.model_to_screen(model);
        assert!(matches!(
            canvas_chain_state.hover_at(&scene, position, tolerance, policy),
            OffsetAuthoringOutcome::HoverChanged(Some(ref hover))
                if hover.target == OffsetAuthoringTarget::Span(span)
                    && hover.availability.is_available()
        ));
        assert!(matches!(
            canvas_chain_state.pick_at(&scene, position, tolerance, policy),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    assert_eq!(canvas_chain_state.operand(), semantic_chain_state.operand());
    assert_eq!(
        canvas_chain_state.candidate(),
        semantic_chain_state.candidate()
    );
    assert_eq!(semantic_chain.coordinator().intent().undo_len(), 1);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact lifecycle test keeps cold inventory, publication, collector refresh, independent validation, and Undo evidence contiguous"
)]
fn projectional_face_offset_cold_preview_applies_once_reactivates_and_undoes() {
    let mut session = fixture(true, 0x8300_0ff5_0022);
    let viewport = Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).unwrap();
    let intent_before = session.coordinator().intent().clone();
    let history_before = intent_before.history_projection();
    let design_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .clone();
    let accepted_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let scene_before = session.scene(viewport, 0.5).unwrap();

    let mut state = OffsetAuthoringState::default();
    session.activate_offset_authoring(&mut state).unwrap();
    let original_index = state.index().unwrap().clone();
    let original_input = original_index.input();
    let face = original_index.faces()[0].key.clone();
    assert!(matches!(
        state.pick_target(OffsetAuthoringTarget::Face(face)),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert!(matches!(
        state.set_distance(0.5),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    let durable_identity = session.coordinator().intent().identity();
    let durable_history = session.coordinator().intent().history_projection();
    let durable_evidence = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    assert!(
        session
            .refresh_offset_authoring_preview(&state, key("offset.preview.face"))
            .unwrap()
    );
    assert!(session.offset_authoring_preview_matches(&state));
    assert_eq!(session.coordinator().intent().identity(), durable_identity);
    assert_eq!(
        session.coordinator().intent().history_projection(),
        durable_history
    );
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        durable_evidence
    );

    let durable_document = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .clone();
    let preview_session = session.presentation_session().unwrap();
    assert_native_session_independently_valid(preview_session);
    let preview_document = preview_session.design_document().clone();
    let preview_accepted_json = preview_session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .to_draft_v5_json()
        .unwrap();
    let preview_design_json = preview_document.to_draft_v5_json().unwrap();
    let expected_inventory =
        provisional_inventory_from_document_delta(&durable_document, &preview_document);
    assert_eq!(
        session.offset_authoring_provisional_items(),
        expected_inventory
    );
    assert_eq!(expected_inventory.len(), 9);
    assert_eq!(
        expected_inventory
            .iter()
            .filter(|item| matches!(item, SelectionItem::Point(_)))
            .count(),
        4
    );
    assert_eq!(
        expected_inventory
            .iter()
            .filter(|item| matches!(item, SelectionItem::Curve(_)))
            .count(),
        4
    );
    assert_eq!(
        expected_inventory
            .iter()
            .filter(|item| matches!(item, SelectionItem::Dimension(_)))
            .count(),
        1
    );
    assert!(
        expected_inventory
            .iter()
            .all(|item| !matches!(item, SelectionItem::Constraint(_)))
    );
    let preview_scene = session.scene(viewport, 0.5).unwrap();
    assert_eq!(preview_scene.points.len(), scene_before.points.len() + 4);
    assert_eq!(preview_scene.curves.len(), scene_before.curves.len() + 4);

    let history_before_apply = session.coordinator().intent().undo_len();
    let outcome = session
        .apply_profile_offset_preview(&mut state, key("offset.preview.face"))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before_apply + 1
    );
    assert!(state.operand().is_none());
    assert!(!session.offset_authoring_preview_matches(&state));
    assert!(session.offset_authoring_provisional_items().is_empty());
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .to_draft_v5_json()
            .unwrap(),
        preview_design_json
    );
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .to_draft_v5_json()
            .unwrap(),
        preview_accepted_json
    );
    assert_same_scene_geometry(&session.scene(viewport, 0.5).unwrap(), &preview_scene);
    assert_independently_valid(&session);
    assert_native_session_independently_valid(session.presentation_session().unwrap());

    assert!(matches!(
        session.activate_offset_authoring(&mut state).unwrap(),
        OffsetAuthoringOutcome::ModeEntered(_)
    ));
    let refreshed_index = state.index().unwrap();
    assert!(!Arc::ptr_eq(&original_index, refreshed_index));
    assert_ne!(refreshed_index.input(), original_input);
    assert_eq!(
        refreshed_index.input(),
        session
            .presentation_session()
            .unwrap()
            .accepted_prepared_input()
            .unwrap()
    );
    assert!(state.operand().is_none());
    assert_eq!(state.distance().unwrap().to_bits(), 0.5_f64.to_bits());
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before_apply + 1
    );

    session.undo().unwrap().unwrap();
    let restored_intent = session.coordinator().intent();
    assert_eq!(
        restored_intent.graph().nodes(),
        intent_before.graph().nodes()
    );
    assert_eq!(
        restored_intent.instance().values(),
        intent_before.instance().values()
    );
    assert_eq!(
        restored_intent.organization().default_cell(),
        intent_before.organization().default_cell()
    );
    assert_eq!(
        restored_intent.organization().cell_order(),
        intent_before.organization().cell_order()
    );
    assert_eq!(
        restored_intent.organization().cells(),
        intent_before.organization().cells()
    );
    assert_eq!(
        restored_intent.organization().node_names(),
        intent_before.organization().node_names()
    );
    assert_eq!(
        restored_intent.external_inputs(),
        intent_before.external_inputs()
    );
    assert_eq!(restored_intent.undo_len(), intent_before.undo_len());
    assert_eq!(
        restored_intent.history_projection().applied,
        history_before.applied
    );
    assert_eq!(restored_intent.redo_len(), 1);
    let restored = session.coordinator().accepted_materialization().unwrap();
    assert_same_document_semantics(restored.session.design_document(), &design_before);
    assert_same_document_semantics(
        restored
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &accepted_before,
    );
    assert_same_scene_geometry(&session.scene(viewport, 0.5).unwrap(), &scene_before);
    assert_native_session_independently_valid(session.presentation_session().unwrap());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one failure matrix proves both stale topology and invalid distance preserve complete projectional authority"
)]
fn projectional_offset_stale_topology_and_invalid_distance_cannot_apply() {
    let viewport = Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).unwrap();
    let mut session = fixture(false, 0x8300_0ff5_0023);
    let mut stale = OffsetAuthoringState::default();
    session.activate_offset_authoring(&mut stale).unwrap();
    for label in ["line.bottom", "line.right"] {
        let span = native_span_by_label(&session, label);
        assert!(matches!(
            stale.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    assert!(matches!(
        stale.set_distance(0.5),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    assert!(
        session
            .refresh_offset_authoring_preview(&stale, key("offset.stale"))
            .unwrap()
    );
    assert!(session.offset_authoring_preview_matches(&stale));
    let stale_before_mutation = stale.clone();

    session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![create(
                "unrelated-stale",
                point("point.unrelated.stale", [10.0, 10.0]),
            )],
        ))
        .unwrap();
    assert!(!session.offset_authoring_preview_matches(&stale));
    let intent_before_failure = session.coordinator().intent().clone();
    let evidence_before_failure = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let document_before_failure = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .to_draft_v5_json()
        .unwrap();
    let scene_before_failure = session.scene(viewport, 0.5).unwrap();

    assert!(matches!(
        session.refresh_offset_authoring_preview(&stale, key("offset.stale")),
        Err(ProjectionalEditorError::ProfileOffset(
            ProjectionalProfileOffsetError::StaleCandidate
        ))
    ));
    assert!(matches!(
        session.apply_profile_offset_preview(&mut stale, key("offset.stale")),
        Err(ProjectionalEditorError::AuthoringPreviewIdentityMismatch)
    ));
    assert!(matches!(
        session.apply_profile_offset(&mut stale, key("offset.stale.direct")),
        Err(ProjectionalEditorError::ProfileOffset(
            ProjectionalProfileOffsetError::StaleCandidate
        ))
    ));
    assert_eq!(stale, stale_before_mutation);
    assert_eq!(session.coordinator().intent(), &intent_before_failure);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before_failure
    );
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .to_draft_v5_json()
            .unwrap(),
        document_before_failure
    );
    assert_same_scene_geometry(
        &session.scene(viewport, 0.5).unwrap(),
        &scene_before_failure,
    );

    let mut invalid = OffsetAuthoringState::default();
    session.activate_offset_authoring(&mut invalid).unwrap();
    let bottom = native_span_by_label(&session, "line.bottom");
    assert!(matches!(
        invalid.pick_target(OffsetAuthoringTarget::Span(bottom)),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert!(
        session
            .refresh_offset_authoring_preview(&invalid, key("offset.invalid"))
            .unwrap()
    );
    assert!(matches!(
        invalid.set_distance(0.0),
        OffsetAuthoringOutcome::Warning(_)
    ));
    assert!(invalid.candidate().is_none());
    assert!(
        !session
            .refresh_offset_authoring_preview(&invalid, key("offset.invalid"))
            .unwrap()
    );
    assert!(session.offset_authoring_provisional_items().is_empty());

    let invalid_before_failure = invalid.clone();
    let intent_before_invalid = session.coordinator().intent().clone();
    let evidence_before_invalid = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let scene_before_invalid = session.scene(viewport, 0.5).unwrap();
    assert!(matches!(
        session.apply_profile_offset_preview(&mut invalid, key("offset.invalid")),
        Err(ProjectionalEditorError::AuthoringCandidateIncomplete)
    ));
    assert!(matches!(
        session.apply_profile_offset(&mut invalid, key("offset.invalid.direct")),
        Err(ProjectionalEditorError::ProfileOffset(
            ProjectionalProfileOffsetError::IncompleteCandidate
        ))
    ));
    assert!(matches!(
        invalid.set_distance(f64::NAN),
        OffsetAuthoringOutcome::Warning(_)
    ));
    assert_eq!(invalid, invalid_before_failure);
    assert_eq!(session.coordinator().intent(), &intent_before_invalid);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before_invalid
    );
    assert_same_scene_geometry(
        &session.scene(viewport, 0.5).unwrap(),
        &scene_before_invalid,
    );
    assert_native_session_independently_valid(session.presentation_session().unwrap());
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

    session.set_selection([SelectionItem::Dimension(dimension)]);
    assert_eq!(session.selected_declaration(), Some(operation));
    assert!(session.set_selected_declaration(Some(aggregates[0])));
    assert_eq!(
        session.selected_declaration(),
        Some(operation),
        "a private grouped operand selects its sole visible Offset owner",
    );
    assert!(session.editor().selection().is_empty());
    session.delete_selected_declaration().unwrap();
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
fn retained_invalid_offset_deletes_by_stable_declaration_without_accepted_ownership() {
    let mut session = fixture(true, 0x8300_0ff5_0024);
    let mut state = offset_state(&session, true);
    session
        .apply_profile_offset(&mut state, key("offset.face.retained.invalid"))
        .unwrap();
    let dimension = profile_offset_dimension(&session);
    let operation = profile_offset_node(&session);
    let aggregate = session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find(|node| matches!(node.kind, IntentNodeKind::Aggregate { .. }))
        .unwrap()
        .id;
    assert_eq!(
        session
            .edit_profile_offset_direction(dimension, ProfileOffsetDirectionState::Inward)
            .unwrap()
            .disposition,
        IntentPlanDisposition::Accepted,
    );
    assert!(session.set_selected_declaration(Some(operation)));
    let accepted_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();

    let invalid = session
        .edit_profile_offset_distance(dimension, 10.0)
        .unwrap();
    assert_eq!(invalid.disposition, IntentPlanDisposition::RetainedFailed);
    assert_eq!(session.selected_declaration(), Some(operation));
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        accepted_before,
    );

    let deleted = session.delete_selected_declaration().unwrap();
    assert_eq!(deleted.disposition, IntentPlanDisposition::Accepted);
    let graph = session.coordinator().intent().graph();
    assert!(graph.node(operation).is_none());
    assert!(graph.node(aggregate).is_none());
    assert_eq!(session.selected_declaration(), None);
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
