// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::{
    CellId, CellTarget, DeletePolicy, ExternalInputRevision, GeometryRecipeKind, InputRole,
    InputSlot, IntentExternalInputs, IntentFieldKey, IntentKey, IntentLiteral, IntentNodeDraft,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortKind,
    IntentPortRef, IntentPortRole, IntentPortSelector, IntentSession, IntentSessionId, IntentUnit,
    LeafField, LeafRef, NodeId, PatchPortRef, PortId,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).expect("fixture keys are valid")
}

fn field(value: &str) -> IntentFieldKey {
    IntentFieldKey(key(value))
}

fn node(value: u64) -> NodeId {
    NodeId::from_raw(value)
}

fn port(value: u64) -> PortId {
    PortId::from_raw(value)
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive wire fixture keeps all twelve closed patch variants reviewable together"
)]
fn representative_patch() -> IntentPatch {
    let session = IntentSession::with_id(IntentSessionId::from_raw(
        0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
    ))
    .expect("fixture session is valid");
    let expected = session.identity();

    let stable_point = IntentPortRef {
        node: node(0x10),
        port: port(0x20),
        kind: IntentPortKind::Point,
    };
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("segment.main"),
    )
    .with_display_name(key("Main segment"))
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: stable_point },
    )
    .with_input(
        InputSlot::new(InputRole::Point, 1),
        PatchPortRef::Alias {
            node: key("created-point"),
            selector: IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
        },
    )
    .with_field(field("flag"), IntentLiteral::Boolean(true))
    .with_field(field("integer"), IntentLiteral::Integer(-7))
    .with_field(field("natural"), IntentLiteral::Natural(12))
    .with_field(field("text"), IntentLiteral::Text(key("fixture text")))
    .with_field(field("enum"), IntentLiteral::Enum(key("clockwise")))
    .with_field(field("point"), IntentLiteral::Point([1.25, -2.5]))
    .with_field(
        field("quantity"),
        IntentLiteral::Quantity {
            value: 3.75,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Start,
            index: 0,
        },
        LeafField::X,
        IntentLiteral::Quantity {
            value: 4.5,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Start,
            index: 0,
        },
        LeafField::Y,
        IntentLiteral::Quantity {
            value: -6.25,
            unit: IntentUnit::Length,
        },
    );
    draft.dynamic_children = 2;
    draft.suppressed = false;

    let cascade = BTreeSet::from([node(0x30), node(0x31)]);
    let operations = vec![
        IntentPatchOperation::CreateNode {
            alias: key("segment"),
            draft: Box::new(draft),
            cell: Some(CellTarget::Alias {
                alias: key("geometry-cell"),
            }),
        },
        IntentPatchOperation::DeleteNode {
            node: node(0x30),
            policy: DeletePolicy::Cascade {
                exact_nodes: cascade,
            },
        },
        IntentPatchOperation::SetSuppressed {
            node: node(0x40),
            suppressed: true,
        },
        IntentPatchOperation::SetDefinitionField {
            node: node(0x41),
            field: field("sweep"),
            value: IntentLiteral::Enum(key("counterclockwise")),
        },
        IntentPatchOperation::SetInstanceLeaf {
            leaf: LeafRef {
                node: node(0x42),
                port: port(0x52),
                field: LeafField::Angle,
            },
            value: IntentLiteral::Quantity {
                value: 0.75,
                unit: IntentUnit::Angle,
            },
        },
        IntentPatchOperation::RebindInput {
            node: node(0x43),
            slot: InputSlot::new(InputRole::Curve, 2),
            source: PatchPortRef::Stable {
                port: IntentPortRef {
                    node: node(0x44),
                    port: port(0x54),
                    kind: IntentPortKind::Curve,
                },
            },
        },
        IntentPatchOperation::RenameNode {
            node: node(0x45),
            name: key("Renamed declaration"),
        },
        IntentPatchOperation::MoveDeclaration {
            node: node(0x46),
            cell: CellTarget::Stable {
                cell: CellId::from_raw(2),
            },
            before: Some(node(0x47)),
        },
        IntentPatchOperation::CreateCell {
            alias: key("geometry-cell"),
            name: key("Geometry"),
            before: Some(CellId::from_raw(3)),
        },
        IntentPatchOperation::DeleteCell {
            cell: CellId::from_raw(4),
        },
        IntentPatchOperation::ReorderCells {
            exact_order: vec![
                CellId::from_raw(1),
                CellId::from_raw(3),
                CellId::from_raw(2),
            ],
        },
        IntentPatchOperation::ReplaceExternalInputs {
            inputs: IntentExternalInputs::new(
                ExternalInputRevision::from_raw(5),
                vec![1, 2, 3],
                vec![8, 13, 21],
            )
            .expect("fixture external inputs are bounded"),
        },
    ];

    IntentPatch::new(expected, IntentPatchPolicy::RetainFailedIntent, operations)
}

#[test]
fn canonical_rust_patch_matches_the_checked_typescript_fixture() {
    let patch = representative_patch();
    let actual = serde_json::to_string(&patch).expect("fixture patch serializes");
    let expected =
        include_str!("../../../packages/geosolve-intent/test/fixtures/rust-intent-patch-v1.json")
            .trim();

    assert_eq!(actual, expected);
    let decoded: IntentPatch = serde_json::from_str(expected).expect("fixture is Rust-decodable");
    assert_eq!(decoded, patch);
}

#[test]
fn cascade_roots_policy_matches_the_checked_typescript_fixture() {
    let operation = IntentPatchOperation::DeleteNode {
        node: node(0x30),
        policy: DeletePolicy::CascadeRoots {
            exact_roots: BTreeSet::from([node(0x30), node(0x32)]),
            exact_nodes: BTreeSet::from([node(0x30), node(0x31), node(0x32)]),
        },
    };
    let actual = serde_json::to_string(&operation).expect("multi-root delete serializes");
    let expected = include_str!(
        "../../../packages/geosolve-intent/test/fixtures/rust-intent-cascade-roots-v1.json"
    )
    .trim();

    assert_eq!(actual, expected);
    let decoded: IntentPatchOperation =
        serde_json::from_str(expected).expect("fixture is Rust-decodable");
    assert_eq!(decoded, operation);
}

#[test]
fn bootstrap_point_ejection_matches_the_closed_typescript_operation_shape() {
    let operation = IntentPatchOperation::EjectBootstrapPoint { node: node(0x48) };
    let actual = serde_json::to_string(&operation).expect("bootstrap ejection serializes");

    assert_eq!(
        actual,
        r#"{"operation":"eject_bootstrap_point","node":"0000000000000048"}"#,
    );
    let decoded: IntentPatchOperation =
        serde_json::from_str(&actual).expect("TypeScript operation shape is Rust-decodable");
    assert_eq!(decoded, operation);
}

#[test]
fn simplified_pre_parity_shapes_are_rejected_by_the_rust_wire() {
    let patch = representative_patch();
    let value = serde_json::to_value(patch).expect("fixture patch serializes");
    let expected = value
        .get("expected")
        .cloned()
        .expect("fixture has expected identity");

    let simplified = serde_json::json!({
        "expected": expected["digest"],
        "policy": "require_accepted",
        "operations": [{
            "op": "set_instance_leaf",
            "port": {
                "session": "session",
                "node": "node",
                "port": "port",
                "kind": "point"
            },
            "field": "x",
            "value": 1
        }],
        "session": "session"
    });

    assert!(serde_json::from_value::<IntentPatch>(simplified).is_err());
}

#[test]
fn fixture_maps_use_string_slots_selectors_and_leaf_refs() {
    let value = serde_json::to_value(representative_patch()).expect("fixture patch serializes");
    let operations = value["operations"]
        .as_array()
        .expect("fixture operations are an array");
    let create = operations
        .iter()
        .find(|operation| operation["operation"] == "create_node")
        .expect("fixture has create-node operation");
    let draft = &create["draft"];

    assert!(draft["inputs"].get("point:0000").is_some());
    assert!(draft["inputs"].get("point:0001").is_some());
    assert!(draft["initial_instance"].get("node:start:0000").is_some());

    let leaf_operation = operations
        .iter()
        .find(|operation| operation["operation"] == "set_instance_leaf")
        .expect("fixture has instance-leaf operation");
    assert_eq!(
        leaf_operation["leaf"],
        serde_json::Value::String("0000000000000042:0000000000000052:angle".to_owned())
    );

    let expected_keys = value["expected"]
        .as_object()
        .expect("expected is the full identity")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        expected_keys,
        BTreeSet::from([
            "digest".to_owned(),
            "external_inputs".to_owned(),
            "graph".to_owned(),
            "instance".to_owned(),
            "organization".to_owned(),
            "reservations".to_owned(),
            "revision".to_owned(),
            "session".to_owned(),
        ])
    );
}

#[test]
fn fixture_has_no_unordered_map_accident_from_test_construction() {
    let patch = representative_patch();
    let first = serde_json::to_string(&patch).expect("fixture patch serializes");
    let second = serde_json::to_string(&patch).expect("fixture patch reserializes");
    assert_eq!(first, second);

    let _: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&first).expect("fixture top-level object remains a deterministic map");
}

#[test]
fn rust_float_format_matches_the_checked_typescript_fixture() {
    let values = [
        IntentLiteral::Quantity {
            value: 0.0,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: -0.0,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: 1e-7,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: 1e-6,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: 1e-5,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: 1e15,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: 1e16,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: 1.234_567_890_123_456e17,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Quantity {
            value: 1e20,
            unit: IntentUnit::Length,
        },
        IntentLiteral::Point([1e-6, -0.0]),
        IntentLiteral::Integer(i64::MIN),
        IntentLiteral::Integer(i64::MAX),
        IntentLiteral::Natural(u64::MAX),
    ];
    let actual = serde_json::to_string(&values).expect("literal fixture serializes");
    let expected = include_str!(
        "../../../packages/geosolve-intent/test/fixtures/rust-intent-literals-v1.json"
    )
    .trim();
    assert_eq!(actual, expected);
}
