// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{ColdIntentMaterialization, ColdIntentMaterializer};
use geosolve_sketch::{CurveDefinition, DocumentId, GeometryRole, PersistentId};
use geosolve_sketch_intent::{
    AggregateKind, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentKey,
    IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentOperationOutput,
    IntentOperationOutputKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentPortRole, IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField,
    OperationKind, PatchPortRef,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

fn quantity(value: f64, unit: IntentUnit) -> IntentLiteral {
    IntentLiteral::Quantity { value, unit }
}

fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn alias(node: &str, role: IntentPortRole, index: u16) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(node),
        selector: selector(role, index),
    }
}

fn create(alias: &str, draft: IntentNodeDraft) -> IntentPatchOperation {
    IntentPatchOperation::CreateNode {
        alias: key(alias),
        draft: Box::new(draft),
        cell: None,
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
        selector(IntentPortRole::Primary, 0),
        LeafField::X,
        quantity(position[0], IntentUnit::Length),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::Y,
        quantity(position[1], IntentUnit::Length),
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
        selector(IntentPortRole::Start, 0),
        LeafField::X,
        quantity(start[0], IntentUnit::Length),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::Y,
        quantity(start[1], IntentUnit::Length),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::X,
        quantity(end[0], IntentUnit::Length),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::Y,
        quantity(end[1], IntentUnit::Length),
    )
}

fn segment_with_shared_endpoint(
    name: &str,
    free_position: [f64; 2],
    shared_node: &str,
    shared_is_start: bool,
) -> IntentNodeDraft {
    let (free_role, shared_index) = if shared_is_start {
        (IntentPortRole::End, 0)
    } else {
        (IntentPortRole::Start, 1)
    };
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(name),
    )
    .with_input(
        InputSlot::new(InputRole::Point, shared_index),
        alias(shared_node, IntentPortRole::Primary, 0),
    )
    .with_instance_leaf(
        selector(free_role, 0),
        LeafField::X,
        quantity(free_position[0], IntentUnit::Length),
    )
    .with_instance_leaf(
        selector(free_role, 0),
        LeafField::Y,
        quantity(free_position[1], IntentUnit::Length),
    )
}

fn native(kind: IntentOperationOutputKind) -> IntentOperationOutput {
    IntentOperationOutput::native(kind)
}

fn repeat_native(
    outputs: &mut Vec<IntentOperationOutput>,
    kind: IntentOperationOutputKind,
    count: usize,
) {
    outputs.extend((0..count).map(|_| native(kind)));
}

fn field(draft: IntentNodeDraft, name: &str, value: IntentLiteral) -> IntentNodeDraft {
    draft.with_field(IntentFieldKey(key(name)), value)
}

fn enum_field(draft: IntentNodeDraft, name: &str, value: &str) -> IntentNodeDraft {
    field(draft, name, IntentLiteral::Enum(key(value)))
}

fn operation(name: &str, kind: OperationKind) -> IntentNodeDraft {
    IntentNodeDraft::new(IntentNodeKind::Operation { operation: kind }, key(name))
}

fn operation_with_span(name: &str, kind: OperationKind, source: &str) -> IntentNodeDraft {
    operation(name, kind).with_input(
        InputSlot::new(InputRole::Span, 0),
        alias(source, IntentPortRole::Span, 0),
    )
}

fn cold_materialize(
    raw: u128,
    operations: Vec<IntentPatchOperation>,
) -> Result<ColdIntentMaterialization, String> {
    let session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        operations,
    );
    let mut result = None;
    let planned = session.plan_patch(patch, |candidate| {
        match materializer.materialize(candidate) {
            Ok(value) => {
                let evaluation = geosolve_sketch_intent::IntentEvaluation::Accepted {
                    evidence: value.evidence.clone(),
                };
                result = Some(Ok(value));
                evaluation
            }
            Err(error) => {
                result = Some(Err(error.to_string()));
                materializer.evaluate(candidate)
            }
        }
    });
    assert!(session.graph().nodes().is_empty());
    assert!(session.accepted().is_none());
    assert_eq!(session.undo_len(), 0);
    match (planned, result) {
        (Ok(_), Some(result)) => result,
        (Err(_), Some(Err(error))) => Err(error),
        (Err(error), _) => Err(error.to_string()),
        (Ok(_), None) => panic!("materializer callback ran"),
    }
}

fn assert_accepted_case(
    raw: u128,
    operations: Vec<IntentPatchOperation>,
) -> ColdIntentMaterialization {
    let output = cold_materialize(raw, operations.clone()).unwrap();
    let repeated = cold_materialize(raw, operations).unwrap();
    assert_eq!(output.evidence, repeated.evidence);
    assert_eq!(output.ownership, repeated.ownership);
    assert!(output.validation.hard_residuals_validated);
    assert!(
        output
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value <= 1.0e-9)
    );
    let accepted = output.session.accepted_state_for_current_input().unwrap();
    for point in accepted.document().points() {
        assert!(point.position.into_iter().all(f64::is_finite));
    }
    for scalar in accepted.document().scalars() {
        assert!(scalar.value.is_finite());
    }
    for curve in accepted.document().curves() {
        match &curve.definition {
            CurveDefinition::Line {
                branch_direction, ..
            } => assert!(
                branch_direction.iter().copied().all(f64::is_finite)
                    && branch_direction[0].hypot(branch_direction[1]) > 0.0
            ),
            CurveDefinition::CircularArc { .. } => {}
            other => panic!("operation matrix gained an unchecked curve family: {other:?}"),
        }
    }
    output
}

fn rectangle_outputs() -> Vec<IntentOperationOutput> {
    let mut outputs = Vec::new();
    outputs.extend((0..4).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Point)));
    outputs.extend((0..4).map(|_| IntentOperationOutput::curve(1)));
    outputs.extend(
        (0..5).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Constraint)),
    );
    outputs
        .extend((0..2).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Scalar)));
    outputs.extend(
        (0..2).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Dimension)),
    );
    outputs
}

fn mirror_outputs() -> Vec<IntentOperationOutput> {
    vec![
        native(IntentOperationOutputKind::Point),
        native(IntentOperationOutputKind::Point),
        IntentOperationOutput::curve(1),
        native(IntentOperationOutputKind::Constraint),
        native(IntentOperationOutputKind::Constraint),
    ]
}

fn chamfer_outputs() -> Vec<IntentOperationOutput> {
    use IntentOperationOutputKind as K;
    vec![
        native(K::Point),
        native(K::Point),
        IntentOperationOutput::curve(1),
        native(K::Scalar),
        native(K::Contact),
        native(K::Scalar),
        native(K::Contact),
        native(K::Constraint),
        native(K::Constraint),
        native(K::Scalar),
        native(K::Scalar),
        native(K::Dimension),
        native(K::Dimension),
    ]
}

fn associative_fillet_outputs() -> Vec<IntentOperationOutput> {
    use IntentOperationOutputKind as K;
    vec![
        native(K::Point),
        native(K::Scalar),
        native(K::Scalar),
        native(K::Scalar),
        IntentOperationOutput::curve(1),
        native(K::Scalar),
        native(K::Contact),
        native(K::Scalar),
        native(K::Contact),
        native(K::Constraint),
        native(K::Scalar),
        native(K::Dimension),
    ]
}

fn polygon_outputs(sides: usize) -> Vec<IntentOperationOutput> {
    let mut outputs = Vec::new();
    repeat_native(&mut outputs, IntentOperationOutputKind::Point, sides);
    outputs.extend((0..sides).map(|_| IntentOperationOutput::curve(1)));
    outputs
}

fn slot_outputs() -> Vec<IntentOperationOutput> {
    use IntentOperationOutputKind as K;
    let mut outputs = Vec::new();
    repeat_native(&mut outputs, K::Point, 6);
    outputs.extend((0..2).map(|_| IntentOperationOutput::curve(1)));
    for _ in 0..2 {
        repeat_native(&mut outputs, K::Scalar, 3);
        outputs.push(IntentOperationOutput::curve(1));
    }
    for _ in 0..4 {
        outputs.push(native(K::Scalar));
        outputs.push(native(K::Contact));
        outputs.push(native(K::Constraint));
    }
    repeat_native(&mut outputs, K::Constraint, 6);
    outputs
}

fn pattern_outputs() -> Vec<IntentOperationOutput> {
    vec![
        native(IntentOperationOutputKind::Point),
        native(IntentOperationOutputKind::Point),
        IntentOperationOutput::curve(1),
        native(IntentOperationOutputKind::Point),
        native(IntentOperationOutputKind::Point),
        IntentOperationOutput::curve(1),
    ]
}

fn open_line_offset_outputs() -> Vec<IntentOperationOutput> {
    vec![
        native(IntentOperationOutputKind::Point),
        native(IntentOperationOutputKind::Point),
        IntentOperationOutput::curve(1),
        native(IntentOperationOutputKind::Scalar),
        native(IntentOperationOutputKind::Dimension),
    ]
}

fn two_loop_line_offset_outputs() -> Vec<IntentOperationOutput> {
    let mut outputs = Vec::new();
    for _ in 0..2 {
        repeat_native(&mut outputs, IntentOperationOutputKind::Point, 4);
        outputs.extend((0..4).map(|_| IntentOperationOutput::curve(1)));
    }
    outputs.push(native(IntentOperationOutputKind::Scalar));
    outputs.push(native(IntentOperationOutputKind::Dimension));
    outputs
}

fn rectangle_draft(outputs: Vec<IntentOperationOutput>) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::Rectangle,
        },
        key("rectangle"),
    )
    .with_field(
        IntentFieldKey(key("origin")),
        IntentLiteral::Point([1.0, 2.0]),
    )
    .with_field(
        IntentFieldKey(key("width")),
        quantity(4.0, IntentUnit::Length),
    )
    .with_field(
        IntentFieldKey(key("height")),
        quantity(3.0, IntentUnit::Length),
    )
    .with_field(
        IntentFieldKey(key("role")),
        IntentLiteral::Enum(key("profile")),
    )
    .with_operation_outputs(outputs)
}

fn materialize_rectangle(
    raw: u128,
    outputs: Vec<IntentOperationOutput>,
) -> Result<ColdIntentMaterialization, String> {
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
            alias: key("rectangle"),
            draft: Box::new(rectangle_draft(outputs)),
            cell: None,
        }],
    );
    let mut failure = None;
    let plan = session
        .plan_patch(patch, |candidate| {
            match materializer.materialize(candidate) {
                Ok(value) => {
                    let evaluation = geosolve_sketch_intent::IntentEvaluation::Accepted {
                        evidence: value.evidence.clone(),
                    };
                    failure = Some(Ok(value));
                    evaluation
                }
                Err(error) => {
                    failure = Some(Err(error.to_string()));
                    materializer.evaluate(candidate)
                }
            }
        })
        .map_err(|error| error.to_string())?;
    let _ = plan;
    failure.expect("materializer callback ran")
}

#[test]
fn rectangle_operation_consumes_authenticated_outputs_and_is_cold_deterministic() {
    let first = materialize_rectangle(0x8301_0001, rectangle_outputs()).unwrap();
    let second = materialize_rectangle(0x8301_0001, rectangle_outputs()).unwrap();
    assert_eq!(first.evidence, second.evidence);
    assert_eq!(first.ownership, second.ownership);
    let document = first
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert_eq!(document.points().len(), 4);
    assert_eq!(document.curves().len(), 4);
    assert_eq!(document.constraints().len(), 5);
    assert_eq!(document.scalars().len(), 2);
    assert_eq!(document.dimensions().len(), 2);
    assert!(first.validation.hard_residuals_validated);
    assert!(
        first
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value <= 1.0e-9)
    );
}

#[test]
fn rectangle_operation_rejects_wrong_output_kind_or_count_without_result() {
    let mut wrong_kind = rectangle_outputs();
    wrong_kind[0] = IntentOperationOutput::native(IntentOperationOutputKind::Scalar);
    assert!(materialize_rectangle(0x8301_0002, wrong_kind).is_err());

    let mut wrong_count = rectangle_outputs();
    wrong_count.pop();
    assert!(materialize_rectangle(0x8301_0003, wrong_count).is_err());
}

#[test]
fn edit_only_operations_lower_with_zero_outputs_and_keep_finite_accepted_geometry() {
    let split = enum_field(
        field(
            operation_with_span("split", OperationKind::Split, "support"),
            "parameter",
            quantity(0.5, IntentUnit::Dimensionless),
        ),
        "retained",
        "before",
    );
    let split_output = assert_accepted_case(
        0x8301_0100,
        vec![
            create("split", split),
            create("support", segment("support", [0.0, 0.0], [4.0, 0.0])),
        ],
    );
    assert_eq!(split_output.session.design_document().curves().len(), 1);

    let break_operation = enum_field(
        field(
            field(
                operation_with_span("break", OperationKind::Break, "support"),
                "start",
                quantity(0.25, IntentUnit::Dimensionless),
            ),
            "end",
            quantity(0.75, IntentUnit::Dimensionless),
        ),
        "retained",
        "before",
    );
    assert_accepted_case(
        0x8301_0101,
        vec![
            create("break", break_operation),
            create("support", segment("support", [0.0, 0.0], [4.0, 0.0])),
        ],
    );

    let trim = enum_field(
        field(
            operation_with_span("trim", OperationKind::Trim, "support"),
            "parameter",
            quantity(0.5, IntentUnit::Dimensionless),
        ),
        "retained",
        "after",
    );
    assert_accepted_case(
        0x8301_0102,
        vec![
            create("trim", trim),
            create("support", segment("support", [0.0, 0.0], [4.0, 0.0])),
        ],
    );

    let extend = enum_field(
        operation("extend", OperationKind::Extend)
            .with_input(
                InputSlot::new(InputRole::Span, 0),
                alias("source", IntentPortRole::Span, 0),
            )
            .with_input(
                InputSlot::new(InputRole::Span, 1),
                alias("target", IntentPortRole::Span, 0),
            ),
        "endpoint",
        "end",
    );
    let extend_output = assert_accepted_case(
        0x8301_0103,
        vec![
            create("extend", extend),
            create("target", segment("target", [2.0, -1.0], [2.0, 1.0])),
            create("source", segment("source", [0.0, 0.0], [1.0, 0.0])),
        ],
    );
    let source = extend_output
        .session
        .design_document()
        .curves()
        .iter()
        .find(|curve| curve.label == "source")
        .unwrap();
    let CurveDefinition::Line { end, .. } = source.definition else {
        panic!("extended source remains a line");
    };
    assert_eq!(
        extend_output
            .session
            .design_document()
            .point(end)
            .unwrap()
            .position
            .map(f64::to_bits),
        [2.0_f64.to_bits(), 0.0_f64.to_bits()]
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive matrix keeps all eight constructive operation inventories auditable"
)]
fn constructive_operations_authenticate_exact_outputs_and_downstream_ports() {
    let mirror = operation("mirror", OperationKind::Mirror)
        .with_input(
            InputSlot::new(InputRole::Curve, 0),
            alias("source", IntentPortRole::Curve, 0),
        )
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            alias("axis", IntentPortRole::Span, 0),
        )
        .with_operation_outputs(mirror_outputs());
    let mirror_output = assert_accepted_case(
        0x8301_0200,
        vec![
            create("mirror", mirror),
            create("axis", segment("axis", [0.0, -2.0], [0.0, 2.0])),
            create("source", segment("source", [1.0, 0.0], [2.0, 1.0])),
        ],
    );
    assert_eq!(mirror_output.session.design_document().curves().len(), 3);

    let chamfer = field(
        field(
            operation("chamfer", OperationKind::Chamfer)
                .with_input(
                    InputSlot::new(InputRole::Span, 0),
                    alias("first", IntentPortRole::Span, 0),
                )
                .with_input(
                    InputSlot::new(InputRole::Span, 1),
                    alias("second", IntentPortRole::Span, 0),
                )
                .with_operation_outputs(chamfer_outputs()),
            "first_distance",
            quantity(1.0, IntentUnit::Length),
        ),
        "second_distance",
        quantity(1.0, IntentUnit::Length),
    );
    let chamfer_output = assert_accepted_case(
        0x8301_0201,
        vec![
            create("chamfer", chamfer),
            create(
                "first",
                segment_with_shared_endpoint("first", [4.0, 0.0], "corner", true),
            ),
            create(
                "second",
                segment_with_shared_endpoint("second", [0.0, 4.0], "corner", true),
            ),
            create("corner", point("corner", [0.0, 0.0])),
        ],
    );
    assert_eq!(chamfer_output.session.design_document().curves().len(), 3);

    let mut fillet = operation("fillet", OperationKind::AssociativeFillet)
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            alias("first", IntentPortRole::Span, 0),
        )
        .with_input(
            InputSlot::new(InputRole::Span, 1),
            alias("second", IntentPortRole::Span, 0),
        )
        .with_operation_outputs(associative_fillet_outputs());
    for (name, value) in [
        ("radius", quantity(1.0, IntentUnit::Length)),
        ("radius_mode", IntentLiteral::Enum(key("driving"))),
        (
            "endpoint_order",
            IntentLiteral::Enum(key("first_then_second")),
        ),
        ("sweep", IntentLiteral::Enum(key("counter_clockwise"))),
        ("first_parameter", quantity(0.75, IntentUnit::Dimensionless)),
        ("first_winding", IntentLiteral::Integer(0)),
        ("first_neighborhood", IntentLiteral::Enum(key("local"))),
        (
            "first_local_lower",
            quantity(0.5, IntentUnit::Dimensionless),
        ),
        (
            "first_local_upper",
            quantity(0.95, IntentUnit::Dimensionless),
        ),
        ("first_normal_side", IntentLiteral::Enum(key("left"))),
        ("first_trim_endpoint", IntentLiteral::Enum(key("end"))),
        ("first_periodic_anchor", IntentLiteral::Boolean(false)),
        (
            "second_parameter",
            quantity(0.25, IntentUnit::Dimensionless),
        ),
        ("second_winding", IntentLiteral::Integer(0)),
        ("second_neighborhood", IntentLiteral::Enum(key("local"))),
        (
            "second_local_lower",
            quantity(0.05, IntentUnit::Dimensionless),
        ),
        (
            "second_local_upper",
            quantity(0.5, IntentUnit::Dimensionless),
        ),
        ("second_normal_side", IntentLiteral::Enum(key("left"))),
        ("second_trim_endpoint", IntentLiteral::Enum(key("start"))),
        ("second_periodic_anchor", IntentLiteral::Boolean(false)),
    ] {
        fillet = field(fillet, name, value);
    }
    let fillet_output = assert_accepted_case(
        0x8301_0202,
        vec![
            create("fillet", fillet),
            create(
                "first",
                segment_with_shared_endpoint("first", [0.0, 0.0], "corner", false),
            ),
            create(
                "second",
                segment_with_shared_endpoint("second", [4.0, 4.0], "corner", true),
            ),
            create("corner", point("corner", [4.0, 0.0])),
        ],
    );
    assert!(
        fillet_output
            .session
            .design_document()
            .curves()
            .iter()
            .any(|curve| matches!(curve.definition, CurveDefinition::CircularArc { .. }))
    );

    let polygon = enum_field(
        field(
            field(
                field(
                    operation("polygon", OperationKind::RegularPolygon)
                        .with_operation_outputs(polygon_outputs(5)),
                    "center",
                    IntentLiteral::Point([0.0, 0.0]),
                ),
                "radius",
                quantity(2.0, IntentUnit::Length),
            ),
            "sides",
            IntentLiteral::Natural(5),
        ),
        "role",
        "construction",
    );
    let polygon = field(polygon, "rotation", quantity(0.2, IntentUnit::Angle));
    let polygon_output = assert_accepted_case(0x8301_0203, vec![create("polygon", polygon)]);
    assert!(
        polygon_output
            .session
            .design_document()
            .curves()
            .iter()
            .all(|curve| polygon_output
                .session
                .design_document()
                .geometry_role(curve.id)
                == Some(GeometryRole::Construction))
    );

    let slot = enum_field(
        field(
            field(
                field(
                    operation("slot", OperationKind::Slot).with_operation_outputs(slot_outputs()),
                    "first_center",
                    IntentLiteral::Point([0.0, 0.0]),
                ),
                "second_center",
                IntentLiteral::Point([4.0, 0.0]),
            ),
            "radius",
            quantity(1.0, IntentUnit::Length),
        ),
        "role",
        "profile",
    );
    let slot_output = assert_accepted_case(0x8301_0204, vec![create("slot", slot)]);
    assert_eq!(slot_output.session.design_document().curves().len(), 4);

    let pattern = field(
        field(
            operation("pattern", OperationKind::LinearPattern)
                .with_dynamic_children(1)
                .with_input(
                    InputSlot::new(InputRole::Curve, 0),
                    alias("source", IntentPortRole::Curve, 0),
                )
                .with_operation_outputs(pattern_outputs()),
            "instances",
            IntentLiteral::Natural(3),
        ),
        "step",
        IntentLiteral::Point([0.0, 2.0]),
    );
    let pattern_output = assert_accepted_case(
        0x8301_0205,
        vec![
            create("pattern", pattern),
            create("source", segment("source", [0.0, 0.0], [1.0, 0.0])),
        ],
    );
    assert_eq!(pattern_output.session.design_document().curves().len(), 3);

    let chain = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("source", IntentPortRole::Span, 0),
    );
    let offset = enum_field(
        enum_field(
            field(
                operation("offset", OperationKind::ProfileOffset)
                    .with_input(
                        InputSlot::new(InputRole::Chain, 0),
                        alias("chain", IntentPortRole::Chain, 0),
                    )
                    .with_operation_outputs(open_line_offset_outputs()),
                "distance",
                quantity(1.0, IntentUnit::Length),
            ),
            "side",
            "left",
        ),
        "first_traversal",
        "forward",
    );
    let offset_output = assert_accepted_case(
        0x8301_0206,
        vec![
            create("offset", offset),
            create("chain", chain),
            create("source", segment("source", [0.0, 0.0], [4.0, 0.0])),
        ],
    );
    assert_eq!(offset_output.session.design_document().curves().len(), 2);

    let outer = field(
        field(
            field(
                enum_field(
                    operation("outer", OperationKind::Rectangle)
                        .with_operation_outputs(rectangle_outputs()),
                    "role",
                    "profile",
                ),
                "origin",
                IntentLiteral::Point([-4.0, -4.0]),
            ),
            "width",
            quantity(8.0, IntentUnit::Length),
        ),
        "height",
        quantity(8.0, IntentUnit::Length),
    );
    let hole = field(
        field(
            field(
                enum_field(
                    operation("hole", OperationKind::Rectangle)
                        .with_operation_outputs(rectangle_outputs()),
                    "role",
                    "profile",
                ),
                "origin",
                IntentLiteral::Point([-1.0, -1.0]),
            ),
            "width",
            quantity(2.0, IntentUnit::Length),
        ),
        "height",
        quantity(2.0, IntentUnit::Length),
    );
    let mut outer_profile = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::ClosedProfile,
        },
        key("outer_profile"),
    );
    let mut hole_profile = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::ClosedProfile,
        },
        key("hole_profile"),
    );
    for index in 0..4 {
        outer_profile = outer_profile.with_input(
            InputSlot::new(InputRole::Span, index),
            alias("outer", IntentPortRole::Span, index),
        );
        hole_profile = hole_profile.with_input(
            InputSlot::new(InputRole::Span, index),
            alias("hole", IntentPortRole::Span, index),
        );
    }
    let face_offset = enum_field(
        field(
            operation("face_offset", OperationKind::ProfileOffset)
                .with_input(
                    InputSlot::new(InputRole::Profile, 0),
                    alias("outer_profile", IntentPortRole::Profile, 0),
                )
                .with_input(
                    InputSlot::new(InputRole::Profile, 1),
                    alias("hole_profile", IntentPortRole::Profile, 0),
                )
                .with_operation_outputs(two_loop_line_offset_outputs()),
            "distance",
            quantity(0.5, IntentUnit::Length),
        ),
        "direction",
        "outward",
    );
    let face_output = assert_accepted_case(
        0x8301_0208,
        vec![
            create("face_offset", face_offset),
            create("hole_profile", hole_profile),
            create("outer_profile", outer_profile),
            create("hole", hole),
            create("outer", outer),
        ],
    );
    assert_eq!(face_output.session.design_document().curves().len(), 16);

    let rectangle = rectangle_draft(rectangle_outputs());
    let rectangle_span_chain = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("rectangle_span_chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("rectangle", IntentPortRole::Span, 0),
    );
    let downstream_mirror = operation("downstream_mirror", OperationKind::Mirror)
        .with_input(
            InputSlot::new(InputRole::Curve, 0),
            alias("rectangle", IntentPortRole::Result, 4),
        )
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            alias("axis", IntentPortRole::Span, 0),
        )
        .with_operation_outputs(mirror_outputs());
    let downstream = assert_accepted_case(
        0x8301_0207,
        vec![
            create("downstream_mirror", downstream_mirror),
            create("rectangle_span_chain", rectangle_span_chain),
            create("rectangle", rectangle),
            create("axis", segment("axis", [0.0, -5.0], [0.0, 5.0])),
        ],
    );
    assert_eq!(downstream.session.design_document().curves().len(), 6);
}

#[test]
fn malformed_unsupported_and_span_count_mismatches_fail_closed() {
    let unsupported_mirror = operation("mirror", OperationKind::Mirror)
        .with_input(
            InputSlot::new(InputRole::Curve, 0),
            alias("circle", IntentPortRole::Curve, 0),
        )
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            alias("axis", IntentPortRole::Span, 0),
        )
        .with_operation_outputs(mirror_outputs());
    let circle = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key("circle"),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center, 0),
        LeafField::X,
        quantity(2.0, IntentUnit::Length),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center, 0),
        LeafField::Y,
        quantity(0.0, IntentUnit::Length),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target, 0),
        LeafField::Value,
        quantity(1.0, IntentUnit::Length),
    );
    assert!(
        cold_materialize(
            0x8301_0300,
            vec![
                create("mirror", unsupported_mirror),
                create("circle", circle),
                create("axis", segment("axis", [0.0, -2.0], [0.0, 2.0])),
            ],
        )
        .is_err()
    );

    let mut wrong_spans = mirror_outputs();
    wrong_spans[2] = IntentOperationOutput::curve(2);
    let wrong_span_mirror = operation("mirror", OperationKind::Mirror)
        .with_input(
            InputSlot::new(InputRole::Curve, 0),
            alias("source", IntentPortRole::Curve, 0),
        )
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            alias("axis", IntentPortRole::Span, 0),
        )
        .with_operation_outputs(wrong_spans);
    assert!(
        cold_materialize(
            0x8301_0301,
            vec![
                create("mirror", wrong_span_mirror),
                create("source", segment("source", [1.0, 0.0], [2.0, 1.0])),
                create("axis", segment("axis", [0.0, -2.0], [0.0, 2.0])),
            ],
        )
        .is_err()
    );

    let disconnected_chain = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("first", IntentPortRole::Span, 0),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        alias("second", IntentPortRole::Span, 0),
    );
    let offset = enum_field(
        enum_field(
            field(
                operation("offset", OperationKind::ProfileOffset)
                    .with_input(
                        InputSlot::new(InputRole::Chain, 0),
                        alias("chain", IntentPortRole::Chain, 0),
                    )
                    .with_operation_outputs(open_line_offset_outputs()),
                "distance",
                quantity(1.0, IntentUnit::Length),
            ),
            "side",
            "left",
        ),
        "first_traversal",
        "forward",
    );
    assert!(
        cold_materialize(
            0x8301_0302,
            vec![
                create("offset", offset),
                create("chain", disconnected_chain),
                create("first", segment("first", [0.0, 0.0], [1.0, 0.0])),
                create("second", segment("second", [3.0, 0.0], [4.0, 0.0])),
            ],
        )
        .is_err()
    );

    let valid_after_rejections = assert_accepted_case(
        0x8301_0302,
        vec![create("rectangle", rectangle_draft(rectangle_outputs()))],
    );
    assert_eq!(
        valid_after_rejections
            .session
            .design_document()
            .curves()
            .len(),
        4
    );
}
