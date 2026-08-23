// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::RefCell;

use geosolve_constraint_editor::{
    ColdIntentMaterialization, ColdIntentMaterializer, IntentNativeBinding,
    IntentNativeWritableLeaf,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_intent::{
    ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentEvaluation, IntentKey,
    IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSession, IntentSessionId,
    IntentUnit, LeafField, PatchPortRef,
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
            alias: key("circle"),
            draft: Box::new(IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::CenterRadiusCircle,
                },
                key("circle"),
            )),
            cell: None,
        }],
    );
    let rejected = retained.plan_patch(unsupported, |candidate| materializer.evaluate(candidate));
    assert!(rejected.is_err());
    assert!(retained.accepted().is_none());
    assert!(retained.graph().nodes().is_empty());
}
