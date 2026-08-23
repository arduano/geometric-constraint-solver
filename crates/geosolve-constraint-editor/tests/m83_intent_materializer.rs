// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::RefCell;

use geosolve_constraint_editor::{
    ColdIntentMaterialization, ColdIntentMaterializer, IntentMaterializationError,
    IntentNativeBinding, IntentNativeWritableLeaf,
};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, DocumentArcTangencySide,
    DocumentConstraintDefinition, DocumentCurveContinuity, DocumentCurveCurvatureRelation,
    DocumentCurveDirectionRelation, DocumentCurveNormalSide, DocumentDimensionDefinition,
    DocumentExternalBindingId, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentId,
    DocumentLineSide, DocumentParameterId, DocumentParameterKind, ExternalSnapshotDigest,
    ExternalSnapshotEntry, ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1,
    ExternalSnapshotSet, FeatureEndpoint, GeometryRole, ParameterBatch, ParameterBatchEntry,
    ParameterValue, PersistentId, TangentOrientation,
};
use geosolve_sketch_intent::{
    AggregateKind, BootstrapNativeKind, ConstraintKind, DeletePolicy, DimensionKind,
    ExternalInputRevision, ExternalIntentKind, GeometryRecipeKind, IdentityTransitionKind,
    InputRole, InputSlot, IntentBootstrapObject, IntentEvaluation, IntentExternalInputs,
    IntentFieldKey, IntentKey, IntentLiteral, IntentNativeReservationKind, IntentNodeDraft,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortKind,
    IntentPortRole, IntentPortSelector, IntentReservationState, IntentSession, IntentSessionId,
    IntentUnit, LeafField, OperationKind, ParameterIntentKind, PatchPortRef,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

fn selector(role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::Node { role, index: 0 }
}

fn indexed_selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn alias(node: &str, role: IntentPortRole) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(node),
        selector: selector(role),
    }
}

fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn parameter(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Dimensionless,
    }
}

fn angle(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Angle,
    }
}

fn point(name: &str, position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary),
        LeafField::X,
        coordinate(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary),
        LeafField::Y,
        coordinate(position[1]),
    )
}

fn segment(name: &str, start: [f64; 2], end: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::X,
        coordinate(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::Y,
        coordinate(start[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End),
        LeafField::X,
        coordinate(end[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End),
        LeafField::Y,
        coordinate(end[1]),
    )
}

fn circle(name: &str, center: [f64; 2], radius: f64) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::X,
        coordinate(center[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::Y,
        coordinate(center[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target),
        LeafField::Value,
        coordinate(radius),
    )
}

fn circular_arc(
    name: &str,
    center: [f64; 2],
    radius: f64,
    start: f64,
    end: f64,
) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterArc,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::X,
        coordinate(center[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::Y,
        coordinate(center[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target),
        LeafField::Value,
        coordinate(radius),
    )
    .with_instance_leaf(
        indexed_selector(IntentPortRole::Target, 1),
        LeafField::Angle,
        angle(start),
    )
    .with_instance_leaf(
        indexed_selector(IntentPortRole::Target, 2),
        LeafField::Angle,
        angle(end),
    )
}

fn quadratic_bezier(
    name: &str,
    start: [f64; 2],
    control: [f64; 2],
    end: [f64; 2],
) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::QuadraticBezier,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::X,
        coordinate(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::Y,
        coordinate(start[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control),
        LeafField::X,
        coordinate(control[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control),
        LeafField::Y,
        coordinate(control[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End),
        LeafField::X,
        coordinate(end[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End),
        LeafField::Y,
        coordinate(end[1]),
    )
}

fn create(alias_name: &str, draft: IntentNodeDraft) -> IntentPatchOperation {
    IntentPatchOperation::CreateNode {
        alias: key(alias_name),
        draft: Box::new(draft),
        cell: None,
    }
}

fn parameter_declaration(name: &str, kind: &str) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Parameter {
            parameter: ParameterIntentKind::Parameter,
        },
        key(name),
    )
    .with_field(IntentFieldKey(key("kind")), IntentLiteral::Enum(key(kind)))
}

fn relation(
    kind: ConstraintKind,
    inputs: impl IntoIterator<Item = (InputSlot, PatchPortRef)>,
    fields: impl IntoIterator<Item = (&'static str, IntentLiteral)>,
) -> IntentNodeDraft {
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Constraint { constraint: kind },
        key("relation"),
    );
    for (slot, source) in inputs {
        draft = draft.with_input(slot, source);
    }
    for (name, value) in fields {
        draft = draft.with_field(IntentFieldKey(key(name)), value);
    }
    draft
}

fn shared_horizontal_segment_patch(session: &IntentSession, end: [f64; 2]) -> IntentPatch {
    let segment = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("edge"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        alias("origin", IntentPortRole::Primary),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End),
        LeafField::X,
        coordinate(end[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End),
        LeafField::Y,
        coordinate(end[1]),
    );
    let horizontal = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::Horizontal,
        },
        key("edge_horizontal"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("edge", IntentPortRole::Span),
    );
    IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("origin"),
                draft: Box::new(point("origin", [2.0, 3.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("edge"),
                draft: Box::new(segment),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("horizontal"),
                draft: Box::new(horizontal),
                cell: None,
            },
        ],
    )
}

fn cold_materialize_ops(
    raw: u128,
    operations: Vec<IntentPatchOperation>,
) -> ColdIntentMaterialization {
    let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();
    let captured = RefCell::<Option<ColdIntentMaterialization>>::new(None);
    session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                operations,
            ),
            |candidate| {
                let output = materializer.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    captured.into_inner().unwrap()
}

fn assert_independently_validated(output: &ColdIntentMaterialization) {
    let accepted = output.session.accepted_state_for_current_input().unwrap();
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
    assert!(
        accepted
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    assert!(output.validation.hard_residuals_validated);
    assert!(
        output
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
}

#[test]
fn cold_point_segment_horizontal_materialization_is_accepted_and_exactly_owned() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_0101)).unwrap();
    let cold = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(0x8300_0101_0000)),
        1.0,
    )
    .unwrap();
    let captured = RefCell::<Option<ColdIntentMaterialization>>::new(None);
    let plan = session
        .plan_patch(
            shared_horizontal_segment_patch(&session, [8.0, 3.0]),
            |candidate| {
                let output = cold.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    let origin_port = plan
        .aliases()
        .port(&key("origin"), selector(IntentPortRole::Primary))
        .unwrap();
    let start_port = plan
        .aliases()
        .port(&key("edge"), selector(IntentPortRole::Start))
        .unwrap();
    let end_port = plan
        .aliases()
        .port(&key("edge"), selector(IntentPortRole::End))
        .unwrap();
    let curve_port = plan
        .aliases()
        .port(&key("edge"), selector(IntentPortRole::Curve))
        .unwrap();
    let horizontal_port = plan
        .aliases()
        .port(&key("horizontal"), selector(IntentPortRole::Constraint))
        .unwrap();
    session.commit_plan(plan).unwrap();

    let output = captured.into_inner().unwrap();
    let accepted = output.session.accepted_state_for_current_input().unwrap();
    assert_eq!(accepted.document().points().len(), 2);
    assert_eq!(accepted.document().curves().len(), 1);
    assert_eq!(accepted.document().constraints().len(), 1);
    assert_eq!(
        output.ownership.port(origin_port),
        output.ownership.port(start_port)
    );
    assert!(matches!(
        output.ownership.port(end_port),
        Some(IntentNativeBinding::Point(_))
    ));
    assert!(matches!(
        output.ownership.port(curve_port),
        Some(IntentNativeBinding::Curve(_))
    ));
    assert!(matches!(
        output.ownership.port(horizontal_port),
        Some(IntentNativeBinding::Constraint(_))
    ));
    assert!(output.validation.hard_residuals_validated);
    assert!(
        output
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value <= 1.0e-9)
    );

    let IntentNativeBinding::Point(end) = output.ownership.port(end_port).unwrap() else {
        unreachable!();
    };
    let x_leaf = output
        .ownership
        .writable_leaf(IntentNativeWritableLeaf::PointX { point: end })
        .unwrap();
    let y_leaf = output
        .ownership
        .writable_leaf(IntentNativeWritableLeaf::PointY { point: end })
        .unwrap();
    assert_eq!(x_leaf.port, end_port.port);
    assert_eq!(x_leaf.field, LeafField::X);
    assert_eq!(y_leaf.port, end_port.port);
    assert_eq!(y_leaf.field, LeafField::Y);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps create, rebind, cold replay, tombstone, and non-reuse evidence contiguous"
)]
fn rebinding_older_geometry_to_newer_input_preserves_native_reservation_identity() {
    let raw = 0x8300_0102_u128;
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let cold = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();

    let base_plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![create("base", point("base", [0.0, 0.0]))],
            ),
            |candidate| cold.evaluate(candidate),
        )
        .unwrap();
    let base = base_plan
        .aliases()
        .port(&key("base"), selector(IntentPortRole::Primary))
        .unwrap();
    let base_node = base.node;
    session.commit_plan(base_plan).unwrap();

    let older = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("older"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: base },
    )
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, coordinate(4.0))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, coordinate(0.0));
    let older_plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![create("older", older)],
            ),
            |candidate| cold.evaluate(candidate),
        )
        .unwrap();
    let older_node = older_plan.aliases().node(&key("older")).unwrap();
    session.commit_plan(older_plan).unwrap();

    let newer_plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![create("newer", point("newer", [2.0, 0.0]))],
            ),
            |candidate| cold.evaluate(candidate),
        )
        .unwrap();
    let newer = newer_plan
        .aliases()
        .port(&key("newer"), selector(IntentPortRole::Primary))
        .unwrap();
    session.commit_plan(newer_plan).unwrap();

    let before = cold
        .materialize_accepted_authority(session.accepted().unwrap())
        .unwrap();
    let reservation_bindings = session
        .graph()
        .node(older_node)
        .unwrap()
        .reservations
        .keys()
        .map(|reservation| {
            (
                *reservation,
                before.ownership.reservation(*reservation).unwrap(),
            )
        })
        .collect::<Vec<_>>();

    let rebound = RefCell::new(None);
    let rebind_plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::RebindInput {
                    node: older_node,
                    slot: InputSlot::new(InputRole::Point, 0),
                    source: PatchPortRef::Stable { port: newer },
                }],
            ),
            |candidate| {
                let output = cold.materialize(candidate).unwrap();
                let repeated = cold.materialize(candidate).unwrap();
                assert_eq!(output.evidence, repeated.evidence);
                assert_eq!(output.ownership, repeated.ownership);
                let evidence = output.evidence.clone();
                rebound.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    session.commit_plan(rebind_plan).unwrap();

    let output = rebound.into_inner().unwrap();
    for (reservation, binding) in reservation_bindings {
        assert_eq!(output.ownership.reservation(reservation), Some(binding));
    }
    let document = output
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    let line = document.curves().first().unwrap();
    let CurveDefinition::Line { start, end, .. } = line.definition else {
        panic!("rebound geometry must remain a line");
    };
    let start_position = document.point(start).unwrap().position;
    let end_position = document.point(end).unwrap().position;
    assert!((start_position[0] - 2.0).abs() <= f64::EPSILON);
    assert!(start_position[1].abs() <= f64::EPSILON);
    assert!((end_position[0] - 4.0).abs() <= f64::EPSILON);
    assert!(end_position[1].abs() <= f64::EPSILON);
    assert_independently_validated(&output);

    let base_reservation = *session
        .graph()
        .node(base_node)
        .unwrap()
        .reservations
        .keys()
        .next()
        .unwrap();
    let retired_binding = output.ownership.reservation(base_reservation).unwrap();
    let delete = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::DeleteNode {
                    node: base_node,
                    policy: DeletePolicy::RejectDependents,
                }],
            ),
            |candidate| cold.evaluate(candidate),
        )
        .unwrap();
    session.commit_plan(delete).unwrap();
    assert_eq!(
        session.reservations().entries()[&base_reservation].state,
        IntentReservationState::Tombstoned
    );

    let later = RefCell::new(None);
    let later_plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![create("later", point("later", [8.0, 1.0]))],
            ),
            |candidate| {
                let output = cold.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                later.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    let later_port = later_plan
        .aliases()
        .port(&key("later"), selector(IntentPortRole::Primary))
        .unwrap();
    session.commit_plan(later_plan).unwrap();
    let later = later.into_inner().unwrap();
    assert_ne!(later.ownership.port(later_port), Some(retired_binding));
    assert_independently_validated(&later);
}

#[test]
fn cold_reconstruction_is_order_independent_and_rejected_intent_has_no_native_publication() {
    let first = IntentSession::with_id(IntentSessionId::from_raw(0x8300_0102)).unwrap();
    let second = IntentSession::with_id(IntentSessionId::from_raw(0x8300_0102)).unwrap();
    let document = DocumentId(PersistentId::from_u128(0x8300_0102_0000));
    let materializer = ColdIntentMaterializer::with_default_policy(document, 1.0).unwrap();

    let first_capture = RefCell::new(None);
    let first_plan = first
        .plan_patch(
            shared_horizontal_segment_patch(&first, [8.0, 3.0]),
            |candidate| {
                let result = materializer.materialize(candidate).unwrap();
                let evidence = result.evidence.clone();
                first_capture.replace(Some(result));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    let mut reversed = shared_horizontal_segment_patch(&second, [8.0, 3.0])
        .operations()
        .to_vec();
    reversed.reverse();
    let second_capture = RefCell::new(None);
    let second_plan = second
        .plan_patch(
            IntentPatch::new(
                second.identity(),
                IntentPatchPolicy::RequireAccepted,
                reversed,
            ),
            |candidate| {
                let result = materializer.materialize(candidate).unwrap();
                let evidence = result.evidence.clone();
                second_capture.replace(Some(result));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    assert_eq!(first_plan.target().graph, second_plan.target().graph);
    assert_eq!(first_plan.target().instance, second_plan.target().instance);
    let first_materialized = first_capture.into_inner().unwrap();
    let second_materialized = second_capture.into_inner().unwrap();
    assert_eq!(first_materialized.ownership, second_materialized.ownership);
    assert_eq!(first_materialized.evidence, second_materialized.evidence);

    let retained = IntentSession::with_id(IntentSessionId::from_raw(0x8300_0103)).unwrap();
    let unsupported = IntentPatch::new(
        retained.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("bootstrap"),
            draft: Box::new(IntentNodeDraft::new(
                IntentNodeKind::Bootstrap {
                    object: IntentBootstrapObject::new(
                        BootstrapNativeKind::Document,
                        key("test-codec"),
                        Vec::new(),
                    )
                    .unwrap(),
                },
                key("bootstrap"),
            )),
            cell: None,
        }],
    );
    let rejected = retained.plan_patch(unsupported, |candidate| materializer.evaluate(candidate));
    assert!(rejected.is_err());
    assert!(retained.accepted().is_none());
    assert!(retained.graph().nodes().is_empty());
}

#[test]
fn cold_geometry_inventory_materializes_every_exactly_reconstructible_recipe() {
    let cases = [
        (GeometryRecipeKind::SketchPoint, (1, 0, 0)),
        (GeometryRecipeKind::Segment, (2, 0, 1)),
        (GeometryRecipeKind::Polyline, (4, 0, 1)),
        (GeometryRecipeKind::MidpointLine, (3, 0, 1)),
        (GeometryRecipeKind::TwoPointAlignedRectangle, (4, 0, 4)),
        (GeometryRecipeKind::ThreePointCornerRectangle, (4, 0, 4)),
        (GeometryRecipeKind::CenterRectangle, (5, 0, 5)),
        (GeometryRecipeKind::ThreePointCenterRectangle, (5, 0, 5)),
        (GeometryRecipeKind::CenterRadiusCircle, (1, 1, 1)),
        (GeometryRecipeKind::TwoPointDiameterCircle, (1, 1, 1)),
        (GeometryRecipeKind::ThreePointCircle, (1, 1, 1)),
        (GeometryRecipeKind::CenterArc, (1, 3, 1)),
        (GeometryRecipeKind::ThreePointArc, (1, 3, 1)),
        (GeometryRecipeKind::CenterAxesEllipse, (2, 1, 1)),
        (GeometryRecipeKind::AxisEndpointsEllipse, (2, 1, 1)),
        (GeometryRecipeKind::CenterAxesEllipticalArc, (2, 3, 1)),
        (GeometryRecipeKind::AxisEndpointsEllipticalArc, (2, 3, 1)),
        (GeometryRecipeKind::QuadraticBezier, (3, 0, 1)),
        (GeometryRecipeKind::CubicBezier, (4, 0, 1)),
        (GeometryRecipeKind::RationalQuadraticConic, (2, 1, 1)),
        (GeometryRecipeKind::Parabola, (2, 2, 1)),
        (GeometryRecipeKind::Hyperbola, (2, 3, 1)),
        (GeometryRecipeKind::OpenControlNurbs, (4, 4, 1)),
        (GeometryRecipeKind::PeriodicControlNurbs, (4, 4, 1)),
    ];

    for (index, (recipe, expected)) in cases.into_iter().enumerate() {
        let raw = 0x8300_1000 + u128::try_from(index).unwrap();
        let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
        let materializer = ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap();
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("geometry"),
                draft: Box::new(
                    IntentNodeDraft::new(IntentNodeKind::Geometry { recipe }, key("geometry"))
                        .with_dynamic_children(
                            if matches!(
                                recipe,
                                GeometryRecipeKind::Polyline
                                    | GeometryRecipeKind::OpenControlNurbs
                                    | GeometryRecipeKind::PeriodicControlNurbs
                            ) {
                                4
                            } else {
                                0
                            },
                        ),
                ),
                cell: None,
            }],
        );
        let captured = RefCell::<Option<ColdIntentMaterialization>>::new(None);
        let _plan = session
            .plan_patch(patch, |candidate| {
                let output = materializer.materialize(candidate).unwrap_or_else(|error| {
                    panic!("{recipe:?} failed cold materialization: {error}")
                });
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            })
            .unwrap();
        let output = captured.into_inner().unwrap();
        let accepted = output.session.accepted_state_for_current_input().unwrap();
        let document = accepted.document();
        assert_eq!(document.points().len(), expected.0, "{recipe:?}");
        assert_eq!(document.scalars().len(), expected.1, "{recipe:?}");
        assert_eq!(document.curves().len(), expected.2, "{recipe:?}");
        assert!(output.validation.hard_residuals_validated, "{recipe:?}");
        assert!(
            output
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value <= 1.0e-9),
            "{recipe:?}"
        );
    }
}

#[test]
fn geometry_role_is_exact_for_profile_and_construction_recipes() {
    for (index, role) in [GeometryRole::Profile, GeometryRole::Construction]
        .into_iter()
        .enumerate()
    {
        let role_literal = match role {
            GeometryRole::Profile => "profile",
            GeometryRole::Construction => "construction",
        };
        let draft = segment("edge", [0.0, 0.0], [2.0, 0.0]).with_field(
            IntentFieldKey(key("role")),
            IntentLiteral::Enum(key(role_literal)),
        );
        let output = cold_materialize_ops(
            0x8300_1e00 + u128::try_from(index).unwrap(),
            vec![create("edge", draft)],
        );
        let document = output
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document();
        assert_eq!(document.geometry_role(document.curves()[0].id), Some(role));
        assert_independently_validated(&output);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the focused test constructs and checks both compact alias recipes end to end"
)]
fn compact_two_point_recipes_alias_contiguous_existing_operands() {
    let raw = 0x8300_1e05_u128;
    let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();
    let rectangle = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::ThreePointCenterRectangle,
        },
        key("rectangle"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        alias("center", IntentPortRole::Primary),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 1),
        alias("corner", IntentPortRole::Primary),
    );
    let conic = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::RationalQuadraticConic,
        },
        key("conic"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        alias("center", IntentPortRole::Primary),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 1),
        alias("end", IntentPortRole::Primary),
    );
    let captured = RefCell::new(None);
    let plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![
                    create("center", point("center", [0.0, 0.0])),
                    create("corner", point("corner", [1.0, 1.0])),
                    create("end", point("end", [2.0, 0.0])),
                    create("rectangle", rectangle),
                    create("conic", conic),
                ],
            ),
            |candidate| {
                let output = materializer.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    let center = plan
        .aliases()
        .port(&key("center"), selector(IntentPortRole::Primary))
        .unwrap();
    let corner = plan
        .aliases()
        .port(&key("corner"), selector(IntentPortRole::Primary))
        .unwrap();
    let end = plan
        .aliases()
        .port(&key("end"), selector(IntentPortRole::Primary))
        .unwrap();
    let output = captured.into_inner().unwrap();
    assert_eq!(
        output.ownership.port(center),
        output.ownership.port(
            plan.aliases()
                .port(
                    &key("rectangle"),
                    indexed_selector(IntentPortRole::Center, 0)
                )
                .unwrap()
        )
    );
    assert_eq!(
        output.ownership.port(corner),
        output.ownership.port(
            plan.aliases()
                .port(
                    &key("rectangle"),
                    indexed_selector(IntentPortRole::Corner, 0)
                )
                .unwrap()
        )
    );
    assert_eq!(
        output.ownership.port(center),
        output.ownership.port(
            plan.aliases()
                .port(&key("conic"), selector(IntentPortRole::Start))
                .unwrap()
        )
    );
    assert_eq!(
        output.ownership.port(end),
        output.ownership.port(
            plan.aliases()
                .port(&key("conic"), selector(IntentPortRole::End))
                .unwrap()
        )
    );
    assert_independently_validated(&output);
}

#[test]
fn parameter_annotation_and_identity_declarations_lower_without_new_equations() {
    let point_draft = point("point", [1.0, 2.0]);
    let activation = parameter_declaration("active", "activation");
    let annotation = IntentNodeDraft::new(IntentNodeKind::Annotation, key("annotation"))
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            alias("point", IntentPortRole::Primary),
        )
        .with_field(
            IntentFieldKey(key("offset")),
            IntentLiteral::Point([12.0, -4.0]),
        );
    let identity = IntentNodeDraft::new(
        IntentNodeKind::Identity {
            transition: IdentityTransitionKind::Alias,
            port_kind: IntentPortKind::Point,
        },
        key("point_alias"),
    )
    .with_input(
        InputSlot::new(InputRole::Identity, 0),
        alias("point", IntentPortRole::Primary),
    );
    let output = cold_materialize_ops(
        0x8300_1e10,
        vec![
            create("point", point_draft),
            create("active", activation),
            create("annotation", annotation),
            create("point_alias", identity),
        ],
    );
    let document = output
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert_eq!(
        document.parameters()[0].kind,
        DocumentParameterKind::Activation
    );
    assert!(document.parameter_bindings().is_empty());
    assert_eq!(document.constraints().len(), 0);
    assert_eq!(document.dimensions().len(), 0);
    assert_independently_validated(&output);
}

#[test]
fn external_binding_and_snapshot_reference_require_exact_host_revision() {
    // Native IDs are allocated from the document namespace in deterministic
    // reservation order; use the first post-document identity in the host set.
    let raw = 0x8300_1e20_u128;
    let binding = DocumentExternalBindingId(PersistentId::from_u128((raw << 32) + 1));
    let snapshots = ExternalSnapshotSet::new(
        7,
        vec![ExternalSnapshotEntry {
            binding,
            source_revision: 7,
            source_digest: ExternalSnapshotDigest::from_bytes([7; 32]),
            feature: ExternalSnapshotFeatureV1::Point {
                position: [3.0, 4.0],
                scale: 1.0,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        }],
    )
    .unwrap();
    let inputs = IntentExternalInputs::new(
        ExternalInputRevision::from_raw(1),
        ParameterBatch::default()
            .to_canonical_json()
            .unwrap()
            .into_bytes(),
        snapshots.to_canonical_json().unwrap().into_bytes(),
    )
    .unwrap();
    let external = IntentNodeDraft::new(
        IntentNodeKind::External {
            external: ExternalIntentKind::Binding,
        },
        key("host_point"),
    )
    .with_field(
        IntentFieldKey(key("feature_kind")),
        IntentLiteral::Enum(key("point")),
    );
    let reference = IntentNodeDraft::new(
        IntentNodeKind::External {
            external: ExternalIntentKind::SnapshotReference,
        },
        key("snapshot"),
    )
    .with_input(
        InputSlot::new(InputRole::External, 0),
        alias("host_point", IntentPortRole::External),
    )
    .with_field(
        IntentFieldKey(key("snapshot_revision")),
        IntentLiteral::Natural(7),
    );
    let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();
    let captured = RefCell::new(None);
    session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![
                    IntentPatchOperation::ReplaceExternalInputs { inputs },
                    create("host_point", external),
                    create("snapshot", reference),
                ],
            ),
            |candidate| {
                let output = materializer.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    let output = captured.into_inner().unwrap();
    assert_eq!(
        output
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .external_bindings()
            .len(),
        1
    );
    assert_independently_validated(&output);
}

#[test]
fn activation_parameter_binding_consumes_the_exact_native_host_input() {
    let raw = 0x8300_1e30_u128;
    let document_raw = raw << 32;
    let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(document_raw)),
        1.0,
    )
    .unwrap();
    let declarations = || {
        let binding = IntentNodeDraft::new(
            IntentNodeKind::Parameter {
                parameter: ParameterIntentKind::Binding,
            },
            key("binding"),
        )
        .with_input(
            InputSlot::new(InputRole::Parameter, 0),
            alias("active", IntentPortRole::Parameter),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            alias("point", IntentPortRole::Primary),
        );
        vec![
            create("point", point("point", [1.0, 2.0])),
            create("active", parameter_declaration("active", "activation")),
            create("binding", binding),
        ]
    };

    let predicted = RefCell::new(None);
    assert!(
        session
            .plan_patch(
                IntentPatch::new(
                    session.identity(),
                    IntentPatchPolicy::RequireAccepted,
                    declarations(),
                ),
                |candidate| {
                    let offset = candidate
                        .reservations()
                        .entries()
                        .values()
                        .position(|record| record.kind == IntentNativeReservationKind::Parameter)
                        .unwrap();
                    predicted.replace(Some(DocumentParameterId(PersistentId::from_u128(
                        document_raw + u128::try_from(offset).unwrap() + 1,
                    ))));
                    materializer.evaluate(candidate)
                },
            )
            .is_err()
    );
    let parameter = predicted.into_inner().unwrap();
    let inputs = IntentExternalInputs::new(
        ExternalInputRevision::from_raw(1),
        ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Activation(true),
            }],
        )
        .unwrap()
        .to_canonical_json()
        .unwrap()
        .into_bytes(),
        ExternalSnapshotSet::default()
            .to_canonical_json()
            .unwrap()
            .into_bytes(),
    )
    .unwrap();
    let captured = RefCell::new(None);
    session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                std::iter::once(IntentPatchOperation::ReplaceExternalInputs { inputs })
                    .chain(declarations())
                    .collect(),
            ),
            |candidate| {
                let output = materializer.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    let output = captured.into_inner().unwrap();
    let accepted = output.session.accepted_state_for_current_input().unwrap();
    assert_eq!(accepted.document().parameter_bindings().len(), 1);
    assert_eq!(
        output.session.parameter_batch().entries()[0].parameter,
        parameter
    );
    assert_independently_validated(&output);
}

#[test]
fn all_parameter_kinds_and_reference_output_are_exactly_persistent() {
    let reference = IntentNodeDraft::new(
        IntentNodeKind::Dimension {
            dimension: DimensionKind::Radius,
        },
        key("radius_reference"),
    )
    .with_input(
        InputSlot::new(InputRole::Curve, 0),
        alias("circle", IntentPortRole::Curve),
    )
    .with_field(
        IntentFieldKey(key("mode")),
        IntentLiteral::Enum(key("reference")),
    );
    let output_declaration = IntentNodeDraft::new(
        IntentNodeKind::Parameter {
            parameter: ParameterIntentKind::Output,
        },
        key("radius_output"),
    )
    .with_input(
        InputSlot::new(InputRole::Parameter, 0),
        alias("length", IntentPortRole::Parameter),
    )
    .with_input(
        InputSlot::new(InputRole::Dimension, 0),
        alias("reference", IntentPortRole::Dimension),
    );
    let result = cold_materialize_ops(
        0x8300_1e40,
        vec![
            create("circle", circle("circle", [0.0, 0.0], 1.0)),
            create("reference", reference),
            create("length", parameter_declaration("length", "length")),
            create("angle", parameter_declaration("angle", "angle")),
            create(
                "dimensionless",
                parameter_declaration("dimensionless", "dimensionless"),
            ),
            create(
                "activation",
                parameter_declaration("activation", "activation"),
            ),
            create("radius_output", output_declaration),
        ],
    );
    let document = result
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    let kinds = document
        .parameters()
        .iter()
        .map(|parameter| parameter.kind)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        kinds,
        [
            DocumentParameterKind::Length,
            DocumentParameterKind::Angle,
            DocumentParameterKind::Dimensionless,
            DocumentParameterKind::Activation,
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(document.parameter_outputs().len(), 1);
    assert_independently_validated(&result);
}

#[test]
fn one_edge_profile_offset_dimension_reconstructs_exact_source_target_pair() {
    let aggregate = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("source_chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("source", IntentPortRole::Span),
    );
    let offset = IntentNodeDraft::new(
        IntentNodeKind::Dimension {
            dimension: DimensionKind::ProfileOffset,
        },
        key("offset"),
    )
    .with_input(
        InputSlot::new(InputRole::Chain, 0),
        alias("source_chain", IntentPortRole::Chain),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("target", IntentPortRole::Span),
    )
    .with_field(
        IntentFieldKey(key("mode")),
        IntentLiteral::Enum(key("driving")),
    )
    .with_field(
        IntentFieldKey(key("side")),
        IntentLiteral::Enum(key("left")),
    )
    .with_field(
        IntentFieldKey(key("source_traversal")),
        IntentLiteral::Enum(key("forward")),
    )
    .with_field(
        IntentFieldKey(key("target_traversal")),
        IntentLiteral::Enum(key("forward")),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target),
        LeafField::Value,
        coordinate(2.0),
    );
    let result = cold_materialize_ops(
        0x8300_1e50,
        vec![
            create("source", segment("source", [0.0, 0.0], [4.0, 0.0])),
            create("target", segment("target", [0.0, 2.0], [4.0, 2.0])),
            create("source_chain", aggregate),
            create("offset", offset),
        ],
    );
    let document = result
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } =
        &document.dimensions()[0].definition
    else {
        panic!("expected profile offset dimension");
    };
    let geosolve_sketch::DocumentProfileOffsetOperand::OpenChain { side, chain } = operand else {
        panic!("expected open-chain operand");
    };
    assert_eq!(*side, DocumentLineSide::Left);
    assert_eq!(chain.edges.len(), 1);
    let source = document
        .curves()
        .iter()
        .find(|curve| curve.label == "source")
        .unwrap();
    let target = document
        .curves()
        .iter()
        .find(|curve| curve.label == "target")
        .unwrap();
    assert_eq!(
        chain.edges[0].source.curve,
        document.curve_spans(source.id).unwrap()[0]
    );
    assert_eq!(
        chain.edges[0].target.curve,
        document.curve_spans(target.id).unwrap()[0]
    );
    assert_independently_validated(&result);
}

#[test]
fn canonical_host_inputs_reach_the_native_session_and_noncanonical_bytes_fail_closed() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_1f00)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(0x8300_1f00_0000)),
        1.0,
    )
    .unwrap();
    let parameters = ParameterBatch::default();
    let external = ExternalSnapshotSet::default();
    let inputs = IntentExternalInputs::new(
        ExternalInputRevision::from_raw(1),
        parameters.to_canonical_json().unwrap().into_bytes(),
        external.to_canonical_json().unwrap().into_bytes(),
    )
    .unwrap();
    let plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![
                    IntentPatchOperation::ReplaceExternalInputs { inputs },
                    IntentPatchOperation::CreateNode {
                        alias: key("point"),
                        draft: Box::new(point("point", [1.0, 2.0])),
                        cell: None,
                    },
                ],
            ),
            |candidate| materializer.evaluate(candidate),
        )
        .unwrap();
    session.commit_plan(plan).unwrap();
    let accepted = materializer
        .materialize_accepted_authority(session.accepted().unwrap())
        .unwrap();
    assert_eq!(accepted.session.parameter_batch(), &parameters);
    assert_eq!(accepted.session.external_snapshot_set(), &external);

    let mut noncanonical_parameters = parameters.to_canonical_json().unwrap().into_bytes();
    noncanonical_parameters.push(b' ');
    let invalid_inputs = IntentExternalInputs::new(
        ExternalInputRevision::from_raw(2),
        noncanonical_parameters,
        Vec::new(),
    )
    .unwrap();
    let exact_before = session.to_canonical_json().unwrap();
    assert!(
        session
            .plan_patch(
                IntentPatch::new(
                    session.identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::ReplaceExternalInputs {
                        inputs: invalid_inputs,
                    }],
                ),
                |candidate| materializer.evaluate(candidate),
            )
            .is_err()
    );
    assert_eq!(session.to_canonical_json().unwrap(), exact_before);
}

#[test]
fn tangent_arc_materializes_exact_contact_parameters_and_relation_pair() {
    let source = segment("source", [0.0, 0.0], [1.0, 0.0]);
    let tangent = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::TangentArc,
        },
        key("tangent"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("source", IntentPortRole::Span),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::X,
        coordinate(1.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::Y,
        coordinate(1.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target),
        LeafField::Value,
        coordinate(1.0),
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Target,
            index: 1,
        },
        LeafField::Angle,
        IntentLiteral::Quantity {
            value: -std::f64::consts::FRAC_PI_2,
            unit: IntentUnit::Angle,
        },
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Target,
            index: 2,
        },
        LeafField::Angle,
        IntentLiteral::Quantity {
            value: 0.0,
            unit: IntentUnit::Angle,
        },
    );
    let output = cold_materialize_ops(
        0x8300_2000,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("source"),
                draft: Box::new(source),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("tangent"),
                draft: Box::new(tangent),
                cell: None,
            },
        ],
    );
    let accepted = output.session.accepted_state_for_current_input().unwrap();
    assert_eq!(accepted.document().curves().len(), 2);
    assert_eq!(accepted.document().contacts().len(), 2);
    assert_eq!(accepted.document().constraints().len(), 1);
    assert_eq!(accepted.document().scalars().len(), 5);
}

#[test]
fn equation_free_aggregates_validate_exact_open_and_periodic_closed_topology() {
    let second = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("second"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        alias("first", IntentPortRole::End),
    )
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, coordinate(2.0))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, coordinate(0.0));
    let chain = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("first", IntentPortRole::Span),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        alias("second", IntentPortRole::Span),
    );
    let profile = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::ClosedProfile,
        },
        key("profile"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("circle", IntentPortRole::Span),
    );
    let output = cold_materialize_ops(
        0x8300_2001,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("first"),
                draft: Box::new(segment("first", [0.0, 0.0], [1.0, 0.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("second"),
                draft: Box::new(second),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("circle"),
                draft: Box::new(circle("circle", [5.0, 0.0], 1.0)),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("chain"),
                draft: Box::new(chain),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("profile"),
                draft: Box::new(profile),
                cell: None,
            },
        ],
    );
    assert_eq!(output.ownership.aggregates.len(), 2);
    let open = output
        .ownership
        .aggregates
        .iter()
        .find(|aggregate| !aggregate.closed)
        .unwrap();
    let closed = output
        .ownership
        .aggregates
        .iter()
        .find(|aggregate| aggregate.closed)
        .unwrap();
    assert_eq!(open.spans.len(), 2);
    assert_eq!(closed.spans.len(), 1);
}

#[test]
fn disconnected_aggregate_rejects_without_any_native_publication() {
    let session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_2003)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(0x8300_2003_0000)),
        1.0,
    )
    .unwrap();
    let chain = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("first", IntentPortRole::Span),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        alias("second", IntentPortRole::Span),
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("first"),
                draft: Box::new(segment("first", [0.0, 0.0], [1.0, 0.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("second"),
                draft: Box::new(segment("second", [3.0, 0.0], [4.0, 0.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("chain"),
                draft: Box::new(chain),
                cell: None,
            },
        ],
    );
    assert!(
        session
            .plan_patch(patch, |candidate| materializer.evaluate(candidate))
            .is_err()
    );
    assert!(session.graph().nodes().is_empty());
    assert!(session.accepted().is_none());
}

#[test]
fn closed_polyline_and_non_default_nurbs_publish_every_logical_span() {
    let closed = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        },
        key("closed"),
    )
    .with_dynamic_children(4)
    .with_field(IntentFieldKey(key("closed")), IntentLiteral::Boolean(true));
    let nurbs = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::OpenControlNurbs,
        },
        key("nurbs"),
    )
    .with_dynamic_children(5)
    .with_field(IntentFieldKey(key("degree")), IntentLiteral::Natural(2))
    .with_field(
        IntentFieldKey(key("gauge_index")),
        IntentLiteral::Natural(1),
    );
    let regularized = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRectangle,
        },
        key("regularized"),
    )
    .with_field(
        IntentFieldKey(key("regularized")),
        IntentLiteral::Boolean(true),
    );
    let output = cold_materialize_ops(
        0x8300_2002,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("closed"),
                draft: Box::new(closed),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("nurbs"),
                draft: Box::new(nurbs),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("regularized"),
                draft: Box::new(regularized),
                cell: None,
            },
        ],
    );
    let polyline = output
        .ownership
        .aggregates
        .iter()
        .find(|aggregate| aggregate.closed)
        .unwrap();
    assert_eq!(polyline.spans.len(), 4);
    let nurbs_curve = output
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .curves()
        .iter()
        .find(|curve| {
            matches!(
                curve.definition,
                geosolve_sketch::CurveDefinition::Nurbs { .. }
            )
        })
        .unwrap()
        .id;
    let logical_nurbs_spans = output
        .ownership
        .ports
        .iter()
        .filter(|(_, binding)| {
            matches!(binding, IntentNativeBinding::CurveSpan(span) if span.curve == nurbs_curve)
        })
        .count();
    assert_eq!(logical_nurbs_spans, 3);
    assert_eq!(
        output
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .constraints()
            .len(),
        6,
        "center rectangle owns four axis, one midpoint, and one regularization relation"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table-driven relation inventory keeps every supported exact lowering visible"
)]
fn cold_relation_inventory_uses_exact_native_constraint_source_pairs() {
    struct Case {
        kind: ConstraintKind,
        inputs: Vec<(InputSlot, PatchPortRef)>,
        fields: Vec<(&'static str, IntentLiteral)>,
    }
    let p = |index| {
        alias(
            if index == 0 { "p0" } else { "p1" },
            IntentPortRole::Primary,
        )
    };
    let span = |index| {
        alias(
            if index == 0 { "line0" } else { "line1" },
            IntentPortRole::Span,
        )
    };
    let curve = |index| {
        alias(
            if index == 0 { "circle0" } else { "circle1" },
            IntentPortRole::Curve,
        )
    };
    let point_pair = || {
        vec![
            (InputSlot::new(InputRole::Point, 0), p(0)),
            (InputSlot::new(InputRole::Point, 1), p(1)),
        ]
    };
    let span_pair = || {
        vec![
            (InputSlot::new(InputRole::Span, 0), span(0)),
            (InputSlot::new(InputRole::Span, 1), span(1)),
        ]
    };
    let axis = IntentLiteral::Enum(key("x"));
    let cases = vec![
        Case {
            kind: ConstraintKind::FixedPoint,
            inputs: vec![(InputSlot::new(InputRole::Point, 0), p(0))],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::FixedCoordinate,
            inputs: vec![(InputSlot::new(InputRole::Point, 0), p(0))],
            fields: vec![("axis", axis.clone()), ("target", coordinate(0.0))],
        },
        Case {
            kind: ConstraintKind::CoincidentWithOrigin,
            inputs: vec![(InputSlot::new(InputRole::Point, 0), p(0))],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::PointOnDatumAxis,
            inputs: vec![(InputSlot::new(InputRole::Point, 0), p(0))],
            fields: vec![("axis", axis.clone())],
        },
        Case {
            kind: ConstraintKind::Coincident,
            inputs: point_pair(),
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::Horizontal,
            inputs: vec![(InputSlot::new(InputRole::Span, 0), span(0))],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::Vertical,
            inputs: vec![(InputSlot::new(InputRole::Span, 0), span(0))],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::HorizontalPoints,
            inputs: point_pair(),
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::VerticalPoints,
            inputs: point_pair(),
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::HorizontalPointToMidpoint,
            inputs: vec![
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (InputSlot::new(InputRole::Span, 0), span(0)),
            ],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::VerticalPointToMidpoint,
            inputs: vec![
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (InputSlot::new(InputRole::Span, 0), span(0)),
            ],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::Parallel,
            inputs: span_pair(),
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::Perpendicular,
            inputs: span_pair(),
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::CollinearWithDatumAxis,
            inputs: vec![(InputSlot::new(InputRole::Span, 0), span(0))],
            fields: vec![("axis", axis.clone())],
        },
        Case {
            kind: ConstraintKind::Concentric,
            inputs: vec![
                (InputSlot::new(InputRole::Curve, 0), curve(0)),
                (InputSlot::new(InputRole::Curve, 1), curve(1)),
            ],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::Collinear,
            inputs: span_pair(),
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::EqualLength,
            inputs: span_pair(),
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::EqualRadius,
            inputs: vec![
                (InputSlot::new(InputRole::Curve, 0), curve(0)),
                (InputSlot::new(InputRole::Curve, 1), curve(1)),
            ],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::Midpoint,
            inputs: vec![
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (InputSlot::new(InputRole::Span, 0), span(0)),
            ],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::SymmetricAboutLine,
            inputs: vec![
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (InputSlot::new(InputRole::Point, 1), p(1)),
                (InputSlot::new(InputRole::Span, 0), span(0)),
            ],
            fields: vec![],
        },
        Case {
            kind: ConstraintKind::SymmetricAboutDatumAxis,
            inputs: point_pair(),
            fields: vec![("axis", axis)],
        },
        Case {
            kind: ConstraintKind::CircleCircleTangency,
            inputs: vec![
                (InputSlot::new(InputRole::Curve, 0), curve(0)),
                (InputSlot::new(InputRole::Curve, 1), curve(1)),
            ],
            fields: vec![("center_direction", IntentLiteral::Point([1.0, 0.0]))],
        },
    ];

    for (index, case) in cases.into_iter().enumerate() {
        let mut relation = IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: case.kind,
            },
            key("relation"),
        );
        for (slot, source) in case.inputs {
            relation = relation.with_input(slot, source);
        }
        for (name, value) in case.fields {
            relation = relation.with_field(IntentFieldKey(key(name)), value);
        }
        let operations = vec![
            IntentPatchOperation::CreateNode {
                alias: key("p0"),
                draft: Box::new(point("p0", [0.0, 0.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("p1"),
                draft: Box::new(point("p1", [0.0, 1.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("line0"),
                draft: Box::new(segment("line0", [-1.0, 0.0], [1.0, 0.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("line1"),
                draft: Box::new(segment("line1", [0.0, 1.0], [2.0, 2.0])),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("circle0"),
                draft: Box::new(circle("circle0", [0.0, 0.0], 1.0)),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("circle1"),
                draft: Box::new(circle("circle1", [2.0, 0.0], 1.0)),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("relation"),
                draft: Box::new(relation),
                cell: None,
            },
        ];
        let output = cold_materialize_ops(0x8300_3000 + index as u128, operations);
        let accepted = output.session.accepted_state_for_current_input().unwrap();
        assert_eq!(
            accepted.document().constraints().len(),
            1,
            "{:?}",
            case.kind
        );
        assert!(
            output.validation.hard_residuals_validated,
            "{:?}",
            case.kind
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one focused table keeps the contact-owning relation branches exact and reviewable"
)]
fn contact_relations_materialize_exact_parameters_domains_and_orientations() {
    let point_on_curve = cold_materialize_ops(
        0x8300_3800,
        vec![
            create("point", point("point", [0.0, 0.0])),
            create("line", segment("line", [-1.0, 0.0], [1.0, 0.0])),
            create(
                "relation",
                relation(
                    ConstraintKind::PointOnCurve,
                    [
                        (
                            InputSlot::new(InputRole::Point, 0),
                            alias("point", IntentPortRole::Primary),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("line", IntentPortRole::Span),
                        ),
                    ],
                    [
                        ("contact_parameter", parameter(0.5)),
                        ("contact_domain", IntentLiteral::Enum(key("bounded"))),
                        ("contact_neighborhood", IntentLiteral::Enum(key("local"))),
                        ("contact_neighborhood_lower", parameter(0.25)),
                        ("contact_neighborhood_upper", parameter(0.75)),
                        ("contact_orientation", IntentLiteral::Enum(key("none"))),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&point_on_curve);
    let accepted = point_on_curve
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 1);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert_eq!(document.scalars().len(), 1);
    assert_eq!(
        document.contacts()[0].domain,
        ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0
        }
    );
    assert_eq!(
        document.contacts()[0].neighborhood,
        ContactNeighborhood::Local {
            lower: 0.25,
            upper: 0.75
        }
    );
    assert_eq!(document.contacts()[0].tangent_orientation, None);
    assert!(
        (document
            .scalar(document.contacts()[0].parameter)
            .unwrap()
            .value
            - 0.5)
            .abs()
            <= f64::EPSILON
    );
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::PointOnCurve { .. }
    ));

    let line_circle = cold_materialize_ops(
        0x8300_3801,
        vec![
            create("line", segment("line", [-2.0, 0.0], [2.0, 0.0])),
            create("circle", circle("circle", [0.0, 1.0], 1.0)),
            create(
                "relation",
                relation(
                    ConstraintKind::LineCircleTangency,
                    [
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("line", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Curve, 0),
                            alias("circle", IntentPortRole::Curve),
                        ),
                    ],
                    [
                        ("side", IntentLiteral::Enum(key("left"))),
                        ("first_contact_parameter", parameter(0.5)),
                        (
                            "first_contact_domain",
                            IntentLiteral::Enum(key("supporting_line")),
                        ),
                        (
                            "first_contact_orientation",
                            IntentLiteral::Enum(key("aligned")),
                        ),
                        (
                            "second_contact_parameter",
                            parameter(3.0 * std::f64::consts::FRAC_PI_2),
                        ),
                        (
                            "second_contact_domain",
                            IntentLiteral::Enum(key("periodic")),
                        ),
                        (
                            "second_contact_domain_period",
                            parameter(std::f64::consts::TAU),
                        ),
                        ("second_contact_winding", IntentLiteral::Integer(2)),
                        (
                            "second_contact_orientation",
                            IntentLiteral::Enum(key("aligned")),
                        ),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&line_circle);
    let accepted = line_circle
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 2);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert_eq!(document.scalars().len(), 3);
    assert_eq!(document.contacts()[0].domain, ContactDomain::SupportingLine);
    assert_eq!(
        document.contacts()[1].domain,
        ContactDomain::Periodic {
            period: std::f64::consts::TAU
        }
    );
    assert_eq!(document.contacts()[1].winding, 2);
    assert!(
        document
            .contacts()
            .iter()
            .all(|contact| contact.tangent_orientation == Some(TangentOrientation::Aligned))
    );
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::LineCircleTangency {
            side: DocumentLineSide::Left,
            ..
        }
    ));

    let circle_arc = cold_materialize_ops(
        0x8300_3802,
        vec![
            create("circle", circle("circle", [3.0, 0.0], 1.0)),
            create(
                "arc",
                circular_arc(
                    "arc",
                    [0.0, 0.0],
                    2.0,
                    -std::f64::consts::FRAC_PI_2,
                    std::f64::consts::FRAC_PI_2,
                ),
            ),
            create(
                "relation",
                relation(
                    ConstraintKind::CircleArcTangency,
                    [
                        (
                            InputSlot::new(InputRole::Curve, 0),
                            alias("circle", IntentPortRole::Curve),
                        ),
                        (
                            InputSlot::new(InputRole::Curve, 1),
                            alias("arc", IntentPortRole::Curve),
                        ),
                    ],
                    [
                        ("side", IntentLiteral::Enum(key("outside_arc"))),
                        ("first_contact_parameter", parameter(std::f64::consts::PI)),
                        (
                            "first_contact_orientation",
                            IntentLiteral::Enum(key("opposed")),
                        ),
                        ("second_contact_parameter", parameter(0.5)),
                        (
                            "second_contact_orientation",
                            IntentLiteral::Enum(key("opposed")),
                        ),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&circle_arc);
    let accepted = circle_arc
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 2);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::CircleArcTangency {
            side: DocumentArcTangencySide::OutsideArc,
            ..
        }
    ));
    assert!(
        document
            .contacts()
            .iter()
            .all(|contact| contact.tangent_orientation == Some(TangentOrientation::Opposed))
    );

    let line_curve = cold_materialize_ops(
        0x8300_3803,
        vec![
            create("line", segment("line", [0.0, 0.0], [1.0, 0.0])),
            create(
                "bezier",
                quadratic_bezier("bezier", [0.0, 0.0], [1.0, 0.0], [2.0, 1.0]),
            ),
            create(
                "relation",
                relation(
                    ConstraintKind::LineCurveTangency,
                    [
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("line", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 1),
                            alias("bezier", IntentPortRole::Span),
                        ),
                    ],
                    [
                        ("endpoint", IntentLiteral::Enum(key("start"))),
                        ("contact_parameter", parameter(0.0)),
                        ("contact_neighborhood", IntentLiteral::Enum(key("start"))),
                        ("contact_orientation", IntentLiteral::Enum(key("aligned"))),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&line_curve);
    let accepted = line_curve
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 1);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert_eq!(
        document.contacts()[0].neighborhood,
        ContactNeighborhood::Start
    );
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::LineCurveTangency {
            endpoint: FeatureEndpoint::Start,
            ..
        }
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the differential relation inventory is clearest as exact accepted fixtures"
)]
fn curve_pair_and_differential_relations_preserve_their_explicit_branch_state() {
    let curve_contact = cold_materialize_ops(
        0x8300_3810,
        vec![
            create("first", segment("first", [-1.0, 0.0], [1.0, 0.0])),
            create("second", segment("second", [0.0, -1.0], [0.0, 1.0])),
            create(
                "relation",
                relation(
                    ConstraintKind::CurveCurveContact,
                    [
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("first", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 1),
                            alias("second", IntentPortRole::Span),
                        ),
                    ],
                    [
                        ("first_contact_parameter", parameter(0.5)),
                        ("second_contact_parameter", parameter(0.5)),
                        (
                            "first_contact_orientation",
                            IntentLiteral::Enum(key("none")),
                        ),
                        (
                            "second_contact_orientation",
                            IntentLiteral::Enum(key("none")),
                        ),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&curve_contact);
    let accepted = curve_contact
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 2);
    assert_eq!(document.scalars().len(), 2);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert!(
        document
            .contacts()
            .iter()
            .all(|contact| contact.tangent_orientation.is_none())
    );
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::CurveCurveContact { .. }
    ));

    let curve_tangency = cold_materialize_ops(
        0x8300_3811,
        vec![
            create(
                "first",
                quadratic_bezier("first", [0.0, 0.0], [1.0, 0.0], [2.0, 1.0]),
            ),
            create(
                "second",
                quadratic_bezier("second", [0.0, 0.0], [1.0, 0.0], [2.0, -1.0]),
            ),
            create(
                "relation",
                relation(
                    ConstraintKind::CurveCurveTangency,
                    [
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("first", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 1),
                            alias("second", IntentPortRole::Span),
                        ),
                    ],
                    [
                        ("first_contact_parameter", parameter(0.0)),
                        (
                            "first_contact_neighborhood",
                            IntentLiteral::Enum(key("start")),
                        ),
                        (
                            "first_contact_orientation",
                            IntentLiteral::Enum(key("aligned")),
                        ),
                        ("second_contact_parameter", parameter(0.0)),
                        (
                            "second_contact_neighborhood",
                            IntentLiteral::Enum(key("start")),
                        ),
                        (
                            "second_contact_orientation",
                            IntentLiteral::Enum(key("aligned")),
                        ),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&curve_tangency);
    let accepted = curve_tangency
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 2);
    assert!(
        document
            .contacts()
            .iter()
            .all(|contact| contact.tangent_orientation == Some(TangentOrientation::Aligned))
    );
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::CurveCurveTangency { .. }
    ));

    let curve_direction = cold_materialize_ops(
        0x8300_3812,
        vec![
            create("line", segment("line", [0.0, 2.0], [1.0, 2.0])),
            create(
                "curve",
                quadratic_bezier("curve", [0.0, 0.0], [1.0, 0.0], [2.0, 1.0]),
            ),
            create(
                "relation",
                relation(
                    ConstraintKind::CurveDirection,
                    [
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("line", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 1),
                            alias("curve", IntentPortRole::Span),
                        ),
                    ],
                    [
                        ("relation", IntentLiteral::Enum(key("tangent"))),
                        ("orientation", IntentLiteral::Enum(key("aligned"))),
                        ("contact_parameter", parameter(0.0)),
                        ("contact_neighborhood", IntentLiteral::Enum(key("start"))),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&curve_direction);
    let accepted = curve_direction
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 1);
    assert_eq!(document.contacts()[0].tangent_orientation, None);
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::CurveDirection {
            relation: DocumentCurveDirectionRelation::Tangent {
                orientation: TangentOrientation::Aligned
            },
            ..
        }
    ));

    let equal_curvature = cold_materialize_ops(
        0x8300_3813,
        vec![
            create("first", segment("first", [-1.0, 0.0], [1.0, 0.0])),
            create("second", segment("second", [-1.0, 1.0], [1.0, 1.0])),
            create(
                "relation",
                relation(
                    ConstraintKind::EqualCurvature,
                    [
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("first", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 1),
                            alias("second", IntentPortRole::Span),
                        ),
                    ],
                    [
                        ("relation", IntentLiteral::Enum(key("signed"))),
                        ("first_contact_parameter", parameter(0.25)),
                        ("second_contact_parameter", parameter(0.75)),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&equal_curvature);
    let accepted = equal_curvature
        .session
        .accepted_state_for_current_input()
        .unwrap();
    assert!(matches!(
        accepted.document().constraints()[0].definition,
        DocumentConstraintDefinition::EqualCurvature {
            relation: DocumentCurveCurvatureRelation::Signed,
            ..
        }
    ));

    let endpoint_continuity = cold_materialize_ops(
        0x8300_3814,
        vec![
            create("incoming", segment("incoming", [0.0, 0.0], [2.0, 0.0])),
            create("outgoing", segment("outgoing", [2.0, 0.0], [6.0, 0.0])),
            create(
                "relation",
                relation(
                    ConstraintKind::EndpointContinuity,
                    [
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("incoming", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 1),
                            alias("outgoing", IntentPortRole::Span),
                        ),
                    ],
                    [
                        ("continuity", IntentLiteral::Enum(key("parametric_c2"))),
                        ("parameter_ratio", parameter(2.0)),
                        ("first_contact_parameter", parameter(1.0)),
                        (
                            "first_contact_neighborhood",
                            IntentLiteral::Enum(key("end")),
                        ),
                        ("second_contact_parameter", parameter(0.0)),
                        (
                            "second_contact_neighborhood",
                            IntentLiteral::Enum(key("start")),
                        ),
                    ],
                ),
            ),
        ],
    );
    assert_independently_validated(&endpoint_continuity);
    let accepted = endpoint_continuity
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 2);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::EndpointContinuity {
            continuity: DocumentCurveContinuity::ParametricC2 {
                first_rate: 2.0,
                second_rate: 1.0
            },
            ..
        }
    ));
}

fn fillet_fixture(kind: ConstraintKind) -> ColdIntentMaterialization {
    let arc = circular_arc(
        "arc",
        [1.0, 1.0],
        1.0,
        -std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    )
    .with_field(
        IntentFieldKey(key("sweep")),
        IntentLiteral::Enum(key("clockwise")),
    );
    let mut fields = vec![
        ("first_side", IntentLiteral::Enum(key("left"))),
        ("second_side", IntentLiteral::Enum(key("left"))),
        (
            "endpoint_order",
            IntentLiteral::Enum(key("first_then_second")),
        ),
        ("first_contact_parameter", parameter(0.75)),
        ("first_contact_domain", IntentLiteral::Enum(key("bounded"))),
        (
            "first_contact_neighborhood",
            IntentLiteral::Enum(key("interior")),
        ),
        ("second_contact_parameter", parameter(0.25)),
        ("second_contact_domain", IntentLiteral::Enum(key("bounded"))),
        (
            "second_contact_neighborhood",
            IntentLiteral::Enum(key("interior")),
        ),
    ];
    if kind == ConstraintKind::CurveCurveFillet {
        fields.extend([
            ("first_trim_endpoint", IntentLiteral::Enum(key("end"))),
            ("second_trim_endpoint", IntentLiteral::Enum(key("start"))),
        ]);
    }
    cold_materialize_ops(
        if kind == ConstraintKind::LineLineFillet {
            0x8300_3820
        } else {
            0x8300_3821
        },
        vec![
            create("first", segment("first", [-2.0, 0.0], [2.0, 0.0])),
            create("second", segment("second", [0.0, 2.0], [0.0, -2.0])),
            create("arc", arc),
            create(
                "relation",
                relation(
                    kind,
                    [
                        (
                            InputSlot::new(InputRole::Curve, 0),
                            alias("arc", IntentPortRole::Curve),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 0),
                            alias("first", IntentPortRole::Span),
                        ),
                        (
                            InputSlot::new(InputRole::Span, 1),
                            alias("second", IntentPortRole::Span),
                        ),
                    ],
                    fields,
                ),
            ),
        ],
    )
}

#[test]
fn native_fillet_declarations_lower_without_reconstructing_or_inferring_their_branch() {
    let line_line = fillet_fixture(ConstraintKind::LineLineFillet);
    assert_independently_validated(&line_line);
    let accepted = line_line
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 2);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert!(document.contacts().iter().all(|contact| {
        contact.domain
            == (ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            })
            && contact.neighborhood == ContactNeighborhood::Interior
            && contact.tangent_orientation.is_none()
    }));
    let DocumentConstraintDefinition::LineLineFillet {
        arc,
        first_side,
        second_side,
        endpoint_order,
        ..
    } = document.constraints()[0].definition
    else {
        panic!("expected native line-line fillet")
    };
    assert_eq!(first_side, DocumentCurveNormalSide::Left);
    assert_eq!(second_side, DocumentCurveNormalSide::Left);
    assert_eq!(endpoint_order, DocumentFilletEndpointOrder::FirstThenSecond);
    assert!(matches!(
        document.curve(arc).unwrap().definition,
        CurveDefinition::CircularArc { .. }
    ));

    let curve_curve = fillet_fixture(ConstraintKind::CurveCurveFillet);
    assert_independently_validated(&curve_curve);
    let accepted = curve_curve
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document = accepted.document();
    assert_eq!(document.contacts().len(), 2);
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.source_order().len(), 1);
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::CurveCurveFillet {
            first_side: DocumentCurveNormalSide::Left,
            first_trim_endpoint: DocumentFilletTrimEndpoint::End,
            second_side: DocumentCurveNormalSide::Left,
            second_trim_endpoint: DocumentFilletTrimEndpoint::Start,
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            ..
        }
    ));
}

#[test]
fn malformed_contact_branch_rejects_without_graph_or_accepted_publication() {
    let raw = 0x8300_3830;
    let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();
    let invalid = relation(
        ConstraintKind::PointOnCurve,
        [
            (
                InputSlot::new(InputRole::Point, 0),
                alias("point", IntentPortRole::Primary),
            ),
            (
                InputSlot::new(InputRole::Span, 0),
                alias("line", IntentPortRole::Span),
            ),
        ],
        [
            ("contact_parameter", parameter(0.5)),
            ("contact_orientation", IntentLiteral::Enum(key("aligned"))),
        ],
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            create("point", point("point", [0.0, 0.0])),
            create("line", segment("line", [-1.0, 0.0], [1.0, 0.0])),
            create("relation", invalid),
        ],
    );
    assert!(
        session
            .plan_patch(patch, |candidate| materializer.evaluate(candidate))
            .is_err()
    );
    assert!(session.graph().nodes().is_empty());
    assert!(session.accepted().is_none());
    assert_eq!(session.undo_len(), 0);
    assert_eq!(session.redo_len(), 0);
}

#[test]
fn cold_dimension_inventory_materializes_exact_target_and_source_pairs() {
    let cases = [
        (
            DimensionKind::PointDistance,
            vec![InputRole::Point, InputRole::Point],
        ),
        (DimensionKind::CurveLength, vec![InputRole::Span]),
        (DimensionKind::Radius, vec![InputRole::Curve]),
        (DimensionKind::Diameter, vec![InputRole::Curve]),
        (
            DimensionKind::OrientedAngle,
            vec![InputRole::Span, InputRole::Span],
        ),
        (
            DimensionKind::SupportingLineOffset,
            vec![InputRole::Span, InputRole::Span],
        ),
        (
            DimensionKind::ExactTranslatedSegmentOffset,
            vec![InputRole::Span, InputRole::Span],
        ),
    ];
    for (index, (kind, roles)) in cases.into_iter().enumerate() {
        let mut dimension = IntentNodeDraft::new(
            IntentNodeKind::Dimension { dimension: kind },
            key("dimension"),
        )
        .with_field(
            IntentFieldKey(key("mode")),
            IntentLiteral::Enum(key("reference")),
        );
        for (operand, role) in roles.into_iter().enumerate() {
            let source = match role {
                InputRole::Point => alias(
                    if operand == 0 { "p0" } else { "p1" },
                    IntentPortRole::Primary,
                ),
                InputRole::Span => alias(
                    if operand == 0 { "line0" } else { "line1" },
                    IntentPortRole::Span,
                ),
                InputRole::Curve => alias("circle0", IntentPortRole::Curve),
                _ => unreachable!(),
            };
            dimension = dimension.with_input(
                InputSlot::new(role, u16::try_from(operand).unwrap()),
                source,
            );
        }
        let output = cold_materialize_ops(
            0x8300_4000 + index as u128,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("p0"),
                    draft: Box::new(point("p0", [0.0, 0.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("p1"),
                    draft: Box::new(point("p1", [3.0, 4.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("line0"),
                    draft: Box::new(segment("line0", [0.0, 0.0], [2.0, 0.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("line1"),
                    draft: Box::new(segment("line1", [0.0, 1.0], [2.0, 1.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("circle0"),
                    draft: Box::new(circle("circle0", [0.0, 0.0], 2.0)),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("dimension"),
                    draft: Box::new(dimension),
                    cell: None,
                },
            ],
        );
        let accepted = output.session.accepted_state_for_current_input().unwrap();
        assert_eq!(accepted.document().dimensions().len(), 1, "{kind:?}");
        assert_eq!(accepted.document().scalars().len(), 2, "{kind:?}");
        assert!(output.validation.hard_residuals_validated, "{kind:?}");
    }
}

#[test]
fn operation_only_and_topology_owned_declarations_reject_without_publication() {
    let raw = 0x8300_5000;
    let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::Rectangle,
        },
        key("operation"),
    )
    .with_field(
        IntentFieldKey(key("origin")),
        IntentLiteral::Point([0.0, 0.0]),
    )
    .with_field(IntentFieldKey(key("width")), coordinate(2.0))
    .with_field(IntentFieldKey(key("height")), coordinate(1.0));
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("operation"),
            draft: Box::new(draft),
            cell: None,
        }],
    );
    assert!(
        session
            .plan_patch(patch, |candidate| materializer.evaluate(candidate))
            .is_err()
    );
    assert!(session.graph().nodes().is_empty());
    assert!(session.accepted().is_none());
}

#[test]
fn accepted_authority_cold_reconstruction_is_exact_and_tamper_evident() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_6000)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(0x8300_6000_0000)),
        1.0,
    )
    .unwrap();
    let plan = session
        .plan_patch(
            shared_horizontal_segment_patch(&session, [8.0, 3.0]),
            |candidate| materializer.evaluate(candidate),
        )
        .unwrap();
    session.commit_plan(plan).unwrap();
    let authority = session.accepted().unwrap().clone();

    let reconstructed = materializer
        .materialize_accepted_authority(&authority)
        .unwrap();
    assert_eq!(reconstructed.evidence, authority.evidence);
    assert_eq!(
        reconstructed.validation.semantic, authority.target,
        "cold validation must authenticate the persisted semantic identity"
    );
    assert!(reconstructed.validation.hard_residuals_validated);

    let mut stale_identity = authority.clone();
    stale_identity.target.graph = IntentSession::with_id(IntentSessionId::from_raw(0x8300_6001))
        .unwrap()
        .identity()
        .graph;
    assert!(matches!(
        materializer.materialize_accepted_authority(&stale_identity),
        Err(IntentMaterializationError::AcceptedAuthorityIdentityMismatch)
    ));

    let mut reauthenticated_tamper = authority.clone();
    reauthenticated_tamper.evidence.ownership.push(b' ');
    reauthenticated_tamper.evidence =
        geosolve_sketch_intent::MaterializationEvidence::new_host_artifacts(
            reauthenticated_tamper.evidence.external_inputs,
            reauthenticated_tamper.evidence.materialization.clone(),
            reauthenticated_tamper.evidence.ownership.clone(),
            reauthenticated_tamper.evidence.host_validation.clone(),
        )
        .unwrap();
    assert!(matches!(
        materializer.materialize_accepted_authority(&reauthenticated_tamper),
        Err(IntentMaterializationError::AcceptedAuthorityEvidenceMismatch)
    ));

    let mut tampered_artifact = authority;
    tampered_artifact.evidence.materialization.push(b' ');
    assert!(matches!(
        materializer.materialize_accepted_authority(&tampered_artifact),
        Err(IntentMaterializationError::AcceptedAuthorityEvidenceMismatch)
    ));
}
