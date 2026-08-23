// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::IntentRpcRequest;

#[test]
fn checked_typescript_patch_is_the_exact_rust_apply_patch_request() {
    let patch =
        include_str!("../../../packages/geosolve-intent/test/fixtures/rust-intent-patch-v1.json")
            .trim();
    let expected = format!(r#"{{"method":"apply_patch","patch":{patch}}}"#);

    let decoded: IntentRpcRequest =
        serde_json::from_str(&expected).expect("TypeScript parity fixture is Rust-decodable RPC");
    let IntentRpcRequest::ApplyPatch { patch } = &decoded else {
        panic!("fixture must remain the typed apply-patch request")
    };
    assert_eq!(patch.operations().len(), 12);
    assert_eq!(
        serde_json::to_string(&decoded).expect("closed RPC request serializes"),
        expected
    );
}

#[test]
fn obsolete_string_method_envelope_is_not_a_rust_rpc_request() {
    let obsolete = r#"{"method":"intent.apply","canonicalJson":"{}"}"#;
    assert!(serde_json::from_str::<IntentRpcRequest>(obsolete).is_err());
}
