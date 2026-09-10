// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine_wasm::EngineAdapter;
use serde_json::{Value, json};
fn compiled(radius: u32) -> CompiledManagedSource {
    CompiledManagedSource::from_json(match radius {
        2 => include_str!("../../geosolve-sketch-engine/tests/fixtures/authoring-radius-2.json"),
        5 => include_str!("../../geosolve-sketch-engine/tests/fixtures/authoring-radius-5.json"),
        _ => unreachable!(),
    })
    .unwrap()
}
fn project() -> String {
    CodeProject::managed(ProjectKey("adapter-authoring".into()), compiled(2))
        .unwrap()
        .to_canonical_json()
        .unwrap()
}
fn open(adapter: &mut EngineAdapter) -> Value {
    serde_json::from_str(
        &adapter
            .open_editable_session(&json!({"project":project()}).to_string())
            .unwrap(),
    )
    .unwrap()
}
fn prepare(adapter: &mut EngineAdapter, state: &Value, action: &Value) -> Value {
    serde_json::from_str(&adapter.prepare_editable_authoring(&json!({"session":state["token"]["session"],"expected":state["token"],"action":action}).to_string()).unwrap()).unwrap()
}
fn receipt(prepared: &Value, radius: u32) -> Value {
    let compiled = compiled(radius);
    json!({"ticketDigest":prepared["request"]["ticket"]["ticketDigest"],"baseSourceDigest":prepared["request"]["ticket"]["acceptedSourceDigest"],"candidateSourceDigest":compiled.ir.source_digest,"compiled":compiled})
}
fn apply(state: &Value, prepared: &Value, receipt: &Value) -> String {
    json!({"session":state["token"]["session"],"ticket":prepared["ticket"],"receipt":receipt})
        .to_string()
}
fn value_action() -> Value {
    json!({"kind":"values","writes":[{"declaration":"bore","path":["radius"],"value":{"kind":"unit","value":{"unit":"mm","value":5.0}}}]})
}

#[test]
fn held_native_authoring_receipt_is_exact_one_use_and_preserves_rejected_state() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let id = state["token"]["session"].to_string();
    let before = adapter.editable_session_state(&id).unwrap();
    let prepared = prepare(&mut adapter, &state, &value_action());
    assert_eq!(adapter.editable_session_state(&id).unwrap(), before);
    assert!(
        adapter
            .apply_editable_authoring(&apply(&state, &prepared, &receipt(&prepared, 2)))
            .is_err()
    );
    let mut forged = receipt(&prepared, 5);
    forged["ticketDigest"] = "forged".into();
    assert!(
        adapter
            .apply_editable_authoring(&apply(&state, &prepared, &forged))
            .is_err()
    );
    assert_eq!(adapter.editable_session_state(&id).unwrap(), before);
    let valid = apply(&state, &prepared, &receipt(&prepared, 5));
    let result: Value =
        serde_json::from_str(&adapter.apply_editable_authoring(&valid).unwrap()).unwrap();
    assert_eq!(
        result["state"]["result"]["validation"]["hard_residuals_validated"],
        true
    );
    assert_eq!(
        result["state"]["result"]["geometry"]["scalars"][0]["value"],
        5.0
    );
    assert_eq!(result["valueChanges"][0]["before"]["value"]["value"], 2.0);
    let accepted = adapter.editable_session_state(&id).unwrap();
    assert!(adapter.apply_editable_authoring(&valid).is_err());
    assert_eq!(adapter.editable_session_state(&id).unwrap(), accepted);
}

#[test]
fn foreign_sessions_and_released_preparation_cannot_reconstruct_native_authority() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let other = open(&mut adapter);
    let prepared = prepare(&mut adapter, &state, &value_action());
    assert!(
        adapter
            .apply_editable_authoring(&apply(&other, &prepared, &receipt(&prepared, 5)))
            .is_err()
    );
    assert!(adapter.release_editable_authoring(prepared["ticket"].as_str().unwrap()));
    assert!(
        adapter
            .apply_editable_authoring(&apply(&state, &prepared, &receipt(&prepared, 5)))
            .is_err()
    );
    let prepared = prepare(&mut adapter, &state, &value_action());
    assert!(adapter.close_editable_session(&state["token"]["session"].to_string()));
    assert!(!adapter.release_editable_authoring(prepared["ticket"].as_str().unwrap()));
    assert!(
        adapter
            .export_editable_project(&state["token"]["session"].to_string())
            .is_err()
    );
}

#[test]
fn exact_captured_source_receipt_and_exported_project_design_rebuild_identical_input() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let id = state["token"]["session"].to_string();
    let prepared = prepare(
        &mut adapter,
        &state,
        &json!({"kind":"source","source":compiled(5).normalized_source}),
    );
    adapter
        .apply_editable_authoring(&apply(&state, &prepared, &receipt(&prepared, 5)))
        .unwrap();
    let digest = adapter.editable_source_design_digest(&id).unwrap();
    let project = adapter.export_editable_project(&id).unwrap();
    let design = adapter.export_editable_design(&id).unwrap();
    let mut restored = EngineAdapter::new();
    let reopened: Value = serde_json::from_str(
        &restored
            .open_editable_session(&json!({"project":project,"design":design}).to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        restored
            .editable_source_design_digest(&reopened["token"]["session"].to_string())
            .unwrap(),
        digest
    );
    assert_eq!(reopened["result"]["geometry"]["scalars"][0]["value"], 5.0);
}
