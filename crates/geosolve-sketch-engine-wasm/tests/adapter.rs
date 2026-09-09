// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_engine_wasm::EngineAdapter;
use serde_json::{Value, json};

#[test]
fn string_adapter_preserves_accepted_output_and_explicit_result_lifetime() {
    let mut adapter = EngineAdapter::new();
    assert!(adapter.last_accepted().unwrap().is_none());
    let artifact = json!({ "format": "geosolve-generated-sketch-v1", "sdk_abi": "geosolve-sketch-code-v2",
        "declarations": [{ "identity": ["point"], "family": "geometry.sketchPoint", "arguments": {
            "kind": "object", "value": { "point": { "kind": "array", "value": [
                { "kind": "number", "value": 4 }, { "kind": "number", "value": 7 } ] } } } }],
        "applications": [], "parameters": [], "groups": [], "suppressions": [], "document": {},
        "output": { "kind": "reference", "value": { "identity": ["point"], "path": ["point"], "kind": "point" } },
    });
    let accepted = adapter.evaluate_generated(&artifact.to_string()).unwrap();
    assert!(adapter.evaluate_generated("{}").is_err());
    assert_eq!(
        adapter.last_accepted().unwrap().as_deref(),
        Some(accepted.as_str())
    );
    let result: Value = serde_json::from_str(&accepted).unwrap();
    assert_eq!(
        result["geometry"]["points"][0]["position"],
        json!([4.0, 7.0])
    );
    let id = result["result_id"].as_str().unwrap();
    assert!(adapter.release_result(id));
    assert!(!adapter.release_result(id));
    assert!(adapter.export_profiles(id, 0.02).is_err());
    assert!(adapter.export_profiles_for_output(id, 0.02, "").is_err());
    assert!(adapter.last_accepted().unwrap().is_some());
}

#[test]
fn named_profile_wire_is_bound_to_retained_result_and_output() {
    let mut adapter = EngineAdapter::new();
    let artifact = json!({ "format": "geosolve-generated-sketch-v1", "sdk_abi": "geosolve-sketch-code-v2",
        "declarations": [{ "identity": ["circle"], "family": "geometry.centerRadiusCircle", "arguments": {
            "kind": "object", "value": { "center": { "kind": "array", "value": [
                { "kind": "number", "value": 4 }, { "kind": "number", "value": 7 } ] },
                "radius": { "kind": "unit", "value": { "unit": "mm", "value": 2 } } } } }],
        "applications": [], "parameters": [], "groups": [], "suppressions": [], "document": {},
        "output": { "kind": "object", "value": { "bore": { "kind": "reference", "value": {
            "identity": ["circle"], "path": ["curve"], "kind": "curve" } } } },
    });
    let accepted = adapter.evaluate_generated(&artifact.to_string()).unwrap();
    let result: Value = serde_json::from_str(&accepted).unwrap();
    let id = result["result_id"].as_str().unwrap();
    let exported = adapter
        .export_profiles_for_output(id, 0.02, "/bore")
        .unwrap();
    let profile: Value = serde_json::from_str(&exported).unwrap();
    assert_eq!(profile["evaluation"]["result_id"], id);
    assert_eq!(profile["regions"].as_array().unwrap().len(), 1);
    assert!(
        adapter
            .export_profiles_for_output(id, 0.02, "/unknown")
            .is_err()
    );
    assert!(adapter.evaluate_generated("{}").is_err());
    assert_eq!(
        adapter
            .export_profiles_for_output(id, 0.02, "/bore")
            .unwrap(),
        exported
    );
    assert!(adapter.release_result(id));
    assert!(
        adapter
            .export_profiles_for_output(id, 0.02, "/bore")
            .is_err()
    );
}

#[test]
fn editable_wire_binds_session_and_expected_token_without_cross_session_redirection() {
    let compiled = geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(
        "../../geosolve-sketch-engine/tests/fixtures/session-radius-2.json"
    ))
    .unwrap();
    let project = geosolve_sketch_code::CodeProject::managed(
        geosolve_sketch_code::ProjectKey("adapter-editable".into()),
        compiled,
    )
    .unwrap()
    .to_canonical_json()
    .unwrap();
    let mut adapter = EngineAdapter::new();
    let first: Value = serde_json::from_str(
        &adapter
            .open_editable_session(&json!({"project":project}).to_string())
            .unwrap(),
    )
    .unwrap();
    let second: Value = serde_json::from_str(
        &adapter
            .open_editable_session(&json!({"project":project}).to_string())
            .unwrap(),
    )
    .unwrap();
    let id = first["token"]["session"].as_u64().unwrap().to_string();
    let second_id = second["token"]["session"].as_u64().unwrap().to_string();
    assert!(
        adapter
            .undo_editable(
                &json!({"session":first["token"]["session"],"expected":second["token"]})
                    .to_string()
            )
            .is_err()
    );
    assert_eq!(
        serde_json::from_str::<Value>(&adapter.editable_session_state(&id).unwrap()).unwrap(),
        first
    );
    assert_eq!(
        serde_json::from_str::<Value>(&adapter.editable_session_state(&second_id).unwrap())
            .unwrap(),
        second
    );
    let design: Value =
        serde_json::from_str(&adapter.export_editable_design(&id).unwrap()).unwrap();
    assert_eq!(design["format"], "geosolve-design-v1");
    assert!(adapter.close_editable_session(&id));
    assert!(!adapter.close_editable_session(&id));
    assert!(adapter.editable_session_state(&id).is_err());
    assert!(
        adapter
            .export_profiles(first["result"]["result_id"].as_str().unwrap(), 0.02)
            .is_ok()
    );
}
