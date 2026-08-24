// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    IntentRpcRequest, IntentRpcSession, MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES,
};
use geosolve_demo_web::intent_rpc;
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentUnit,
    LeafField,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

fn sessions() -> (IntentRpcSession, IntentRpcSession) {
    (
        intent_rpc::empty_session_from_raw(0x8300_5001, 0x8300_5001_0000, 1.0).unwrap(),
        intent_rpc::empty_session_from_raw(0x8300_5001, 0x8300_5001_0000, 1.0).unwrap(),
    )
}

fn create_point_request(session: &IntentRpcSession) -> String {
    let selector = IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    };
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("rpc.parity.point"),
    )
    .with_instance_leaf(
        selector,
        LeafField::X,
        IntentLiteral::Quantity {
            value: 2.5,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        selector,
        LeafField::Y,
        IntentLiteral::Quantity {
            value: -1.25,
            unit: IntentUnit::Length,
        },
    );
    serde_json::to_string(&IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(draft),
                cell: None,
            }],
        )),
    })
    .unwrap()
}

#[test]
fn dom_free_web_adapter_matches_the_shared_native_rpc_byte_for_byte() {
    let (mut native, mut wasm_surface) = sessions();
    let request = create_point_request(&native);
    let native_receipt = native.apply_json(&request);
    assert_eq!(
        native_receipt,
        wasm_surface.apply_json(&request),
        "the wasm-bindgen handle delegates to this same DOM-free byte protocol"
    );
    assert!(!native_receipt.contains("\"snapshot\":"));
    assert!(native_receipt.len() < MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES);
    assert_eq!(
        native.apply_json(r#"{"method":"snapshot"}"#),
        wasm_surface.apply_json(r#"{"method":"snapshot"}"#)
    );
    assert_eq!(
        native.apply_json(r#"{"method":"undo"}"#),
        wasm_surface.apply_json(r#"{"method":"undo"}"#)
    );
    assert_eq!(
        native.apply_json(r#"{"method":"redo"}"#),
        wasm_surface.apply_json(r#"{"method":"redo"}"#)
    );
}
