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
    let seed: Value = serde_json::from_str(
        &adapter
            .result_interaction_seed(&json!({"resultId": id}).to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(seed["sceneKey"], id);
    let scene: Value = serde_json::from_str(seed["scene"].as_str().unwrap()).unwrap();
    assert_eq!(scene["points"][0]["model_position"], json!([4.0, 7.0]));
    let workspace = adapter.export_result_workspace(id).unwrap();
    let checkpoint =
        geosolve_constraint_editor::workspace_persistence::WorkspaceSnapshot::decode(&workspace)
            .unwrap();
    let restored =
        geosolve_constraint_editor::workspace_persistence::projectional_editor_from_snapshot(
            &checkpoint,
        )
        .unwrap();
    assert_eq!(
        restored
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .points()[0]
            .position
            .map(f64::to_bits),
        [4.0_f64, 7.0].map(f64::to_bits)
    );
    assert!(
        adapter
            .encode_reproduction(&workspace)
            .unwrap()
            .starts_with("GEOSOLVE_REPRO_V1")
    );
    assert!(adapter.release_result(id));
    assert!(
        adapter
            .result_interaction_seed(&json!({"resultId": id}).to_string())
            .is_err()
    );
    assert!(adapter.export_result_workspace(id).is_err());
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

#[test]
fn restored_history_wire_retains_authority_and_refuses_live_identity_collision() {
    let compiled = geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(
        "../../geosolve-sketch-engine/tests/fixtures/session-radius-2.json"
    ))
    .unwrap();
    let project = geosolve_sketch_code::CodeProject::managed(
        geosolve_sketch_code::ProjectKey("adapter-history".into()),
        compiled,
    )
    .unwrap()
    .to_canonical_json()
    .unwrap();
    let mut adapter = EngineAdapter::new();
    let opened: Value = serde_json::from_str(
        &adapter
            .open_editable_session(
                &json!({"project": project, "persistable_history": true}).to_string(),
            )
            .unwrap(),
    )
    .unwrap();
    let id = opened["token"]["session"].as_u64().unwrap().to_string();
    let history = adapter.export_editable_history(&id).unwrap();
    let state = adapter.editable_session_state(&id).unwrap();
    let accepted = adapter.last_accepted().unwrap();
    assert!(
        adapter
            .restore_editable_history(&history)
            .unwrap_err()
            .contains("already open")
    );
    assert_eq!(adapter.editable_session_state(&id).unwrap(), state);
    assert_eq!(adapter.last_accepted().unwrap(), accepted);
    assert!(adapter.close_editable_session(&id));
    let restored: Value =
        serde_json::from_str(&adapter.restore_editable_history(&history).unwrap()).unwrap();
    assert_eq!(restored["token"], opened["token"]);
    assert_eq!(restored["result"]["geometry"], opened["result"]["geometry"]);
    assert_eq!(adapter.export_editable_history(&id).unwrap(), history);
    let restored_state = adapter.editable_session_state(&id).unwrap();
    assert!(adapter.restore_editable_history("{}").is_err());
    assert_eq!(adapter.editable_session_state(&id).unwrap(), restored_state);
    let presentation = json!({
        "origin": {"kind": "authored"}, "selectedFile": "sketch.ts",
        "managedDraft": "// unfinished 😀", "draftDiagnostic": null,
    });
    let workspace = adapter
        .export_editable_workspace(
            &json!({
                "session": restored["token"]["session"], "expected": restored["token"],
                "presentation": presentation,
            })
            .to_string(),
        )
        .unwrap();
    assert!(
        adapter
            .restore_editable_workspace(&workspace)
            .unwrap_err()
            .contains("already open")
    );
    assert_eq!(adapter.editable_session_state(&id).unwrap(), restored_state);
    assert!(adapter.close_editable_session(&id));
    let loaded: Value =
        serde_json::from_str(&adapter.restore_editable_workspace(&workspace).unwrap()).unwrap();
    assert_eq!(loaded["presentation"], presentation);
    assert_eq!(loaded["state"]["token"], restored["token"]);
    assert_eq!(
        loaded["state"]["result"]["geometry"],
        restored["result"]["geometry"]
    );
    let view = json!({"hiddenRows": ["managed:stale"], "isolateRestore": ["managed:old"],
        "constructionVisible": false, "dimensions": {"mode": "hidden", "pins": []}});
    let outer = adapter.export_editable_workspace(&json!({"session": loaded["state"]["token"]["session"],
        "expected": loaded["state"]["token"], "presentation": presentation, "viewPresentation": view}).to_string()).unwrap();
    let wire: Value = serde_json::from_str(&outer).unwrap();
    assert_eq!(wire["format"], "geosolve-workbench-presentation-v1");
    assert_eq!(wire["project"], workspace);
    assert_eq!(wire["presentation"], view);
    assert!(adapter.close_editable_session(&id));
    let outer_loaded: Value =
        serde_json::from_str(&adapter.restore_editable_workspace(&outer).unwrap()).unwrap();
    assert_eq!(outer_loaded["presentation"], presentation);
    assert_eq!(outer_loaded["viewPresentation"], view);
    assert_eq!(adapter.export_editable_history(&id).unwrap(), history);
    assert!(adapter.close_editable_session(&id));
    let duplicate = outer.replacen(
        "\"format\":",
        "\"format\":\"geosolve-workbench-presentation-v1\",\"format\":",
        1,
    );
    assert!(adapter.restore_editable_workspace(&duplicate).is_err());
    let invalid_rows = outer.replace(
        "[\"managed:stale\"]",
        "[\"managed:stale\",\"managed:stale\"]",
    );
    assert!(adapter.restore_editable_workspace(&invalid_rows).is_err());
    assert!(adapter.restore_editable_workspace(&outer).is_ok());
}
