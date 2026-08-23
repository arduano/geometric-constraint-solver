// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::RefCell;

use geosolve_constraint_editor::{
    ColdIntentMaterialization, ColdIntentMaterializer, IntentMaterializationError,
    IntentNativeBinding, IntentNativeWritableLeaf,
};
use geosolve_sketch::{DocumentId, ExternalSnapshotSet, ParameterBatch, PersistentId};
use geosolve_sketch_intent::{
    AggregateKind, BootstrapNativeKind, ConstraintKind, DimensionKind, ExternalInputRevision,
    GeometryRecipeKind, InputRole, InputSlot, IntentBootstrapObject, IntentEvaluation,
    IntentExternalInputs, IntentFieldKey, IntentKey, IntentLiteral, IntentNodeDraft,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField, OperationKind,
    PatchPortRef,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

fn selector(role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::Node { role, index: 0 }
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
