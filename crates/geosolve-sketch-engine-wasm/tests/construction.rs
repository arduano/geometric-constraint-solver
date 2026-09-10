// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine_wasm::EngineAdapter;
use serde_json::{Value, json};
fn decode(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}
fn open(adapter: &mut EngineAdapter) -> Value {
    let project = CodeProject::managed(
        ProjectKey("construction".into()),
        CompiledManagedSource::from_json(include_str!(
            "../../geosolve-sketch-engine/tests/fixtures/authoring-radius-2.json"
        ))
        .unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap();
    decode(
        &adapter
            .open_editable_session(&json!({"project":project}).to_string())
            .unwrap(),
    )
}
fn begin(state: &Value) -> String {
    json!({"session":state["token"]["session"],"expected":state["token"],"gesture_id":71,
        "tool":"segment","role":"profile","viewport":{"screen_size":[800,600],"model_center":[0,0],"pixels_per_model_unit":5}}).to_string()
}
fn route(state: &Value, held: &Value) -> Value {
    json!({"session":state["token"]["session"],"ticket":held["ticket"],"gesture_id":71})
}
fn command(adapter: &mut EngineAdapter, state: &Value) -> Value {
    let held = decode(&adapter.begin_editable_construction(&begin(state)).unwrap());
    let routing = route(state, &held);
    for (i, position) in [[40.0, 40.0], [60.0, 50.0]].into_iter().enumerate() {
        let mut sample = routing.clone();
        sample["sample"] = json!({"sequence":i+1,"input":{"event":"click","position":position,"suppressed":true,"regularized":false}});
        let frame = decode(
            &adapter
                .advance_editable_construction(&sample.to_string())
                .unwrap(),
        );
        assert_eq!(frame["completed"], i == 1);
        assert_eq!(frame["diagnostic"], Value::Null);
    }
    assert!(
        decode(
            &adapter
                .editable_construction_scene(&routing.to_string())
                .unwrap()
        )
        .is_object()
    );
    let terminal = decode(
        &adapter
            .finish_editable_construction(&routing.to_string())
            .unwrap(),
    );
    assert!(
        adapter
            .finish_editable_construction(&routing.to_string())
            .is_err()
    );
    terminal
}
fn prepare(adapter: &mut EngineAdapter, state: &Value, command: &Value) -> Value {
    decode(&adapter.prepare_editable_construction(&json!({"session":state["token"]["session"],"expected":state["token"],"command":command}).to_string()).unwrap())
}
fn resolve(state: &Value, prepared: &Value) -> String {
    let fixtures = decode(include_str!(
        "../../geosolve-sketch-engine/tests/fixtures/construction-compilations.json"
    ));
    let compiled = &fixtures["segment"];
    json!({"session":state["token"]["session"],"ticket":prepared["ticket"],"receipt":{
        "ticketDigest":prepared["request"]["ticket"]["ticketDigest"],"baseSourceDigest":prepared["request"]["current"]["ir"]["source_digest"],
        "candidateSourceDigest":compiled["ir"]["source_digest"],"compiled":compiled}}).to_string()
}
fn ticket(state: &Value, held: &Value) -> String {
    json!({"session":state["token"]["session"],"ticket":held["ticket"]}).to_string()
}

#[test]
fn adapter_construction_stages_compiler_receipt_and_durable_candidate_without_publication() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let other = open(&mut adapter);
    let id = state["token"]["session"].to_string();
    let command = command(&mut adapter, &state);
    let prepared = prepare(&mut adapter, &state, &command);
    assert!(
        adapter
            .resolve_editable_construction(&resolve(&other, &prepared))
            .is_err()
    );
    let mut forged = decode(&resolve(&state, &prepared));
    forged["receipt"]["ticketDigest"] = "forged".into();
    assert!(
        adapter
            .resolve_editable_construction(&forged.to_string())
            .is_err()
    );
    assert_eq!(decode(&adapter.editable_session_state(&id).unwrap()), state);
    let staged = decode(
        &adapter
            .resolve_editable_construction(&resolve(&state, &prepared))
            .unwrap(),
    );
    assert!(
        adapter
            .resolve_editable_construction(&resolve(&state, &prepared))
            .is_err()
    );
    assert_eq!(decode(&adapter.editable_session_state(&id).unwrap()), state);
    assert!(
        adapter
            .apply_editable_construction_commit(&ticket(&other, &staged))
            .is_err()
    );
    let accepted = decode(
        &adapter
            .apply_editable_construction_commit(&ticket(&state, &staged))
            .unwrap(),
    );
    assert_eq!(accepted["result"]["geometry"], staged["result"]["geometry"]);
    assert_eq!(
        accepted["result"]["geometry"]["points"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        accepted["result"]["validation"]["hard_residuals_validated"],
        true
    );
    assert_eq!(
        adapter.editable_source_design_digest(&id).unwrap(),
        staged["source_design_digest"]
    );
    assert!(
        adapter
            .apply_editable_construction_commit(&ticket(&state, &staged))
            .is_err()
    );
    let mut restored = EngineAdapter::new();
    let cold = decode(
        &restored
            .open_editable_session(
                &json!({"project":staged["project"],"design":staged["design"].to_string()})
                    .to_string(),
            )
            .unwrap(),
    );
    assert_eq!(
        restored
            .editable_source_design_digest(&cold["token"]["session"].to_string())
            .unwrap(),
        staged["source_design_digest"]
    );
}

#[test]
fn construction_handle_bounds_cancellation_and_disposal_restore_capacity() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let other = open(&mut adapter);
    let command = command(&mut adapter, &state);
    let prepared = prepare(&mut adapter, &state, &command);
    let mut held = Vec::new();
    for _ in 0..23 {
        held.push(decode(
            &adapter.begin_editable_construction(&begin(&state)).unwrap(),
        ));
    }
    assert!(
        adapter
            .begin_editable_construction(&begin(&state))
            .unwrap_err()
            .contains("24")
    );
    // Resolution replaces the compiler handle, so a full table must still permit progress.
    let staged = decode(
        &adapter
            .resolve_editable_construction(&resolve(&state, &prepared))
            .unwrap(),
    );
    assert!(
        adapter
            .release_editable_construction(&ticket(&other, &staged))
            .is_err()
    );
    adapter
        .release_editable_construction(&ticket(&state, &staged))
        .unwrap();
    assert!(
        adapter
            .apply_editable_construction_commit(&ticket(&state, &staged))
            .is_err()
    );
    let wrong = route(&other, &held[0]);
    assert!(
        adapter
            .cancel_editable_construction(&wrong.to_string())
            .is_err()
    );
    let routing = route(&state, &held[0]);
    let mut sample = routing.clone();
    sample["sample"] = json!({"sequence":2,"input":{"event":"complete"}});
    assert!(
        adapter
            .advance_editable_construction(&sample.to_string())
            .is_err()
    );
    adapter
        .cancel_editable_construction(&routing.to_string())
        .unwrap();
    assert!(
        adapter
            .editable_construction_scene(&routing.to_string())
            .is_err()
    );
    assert!(adapter.close_editable_session(&state["token"]["session"].to_string()));
    assert!(
        adapter
            .editable_construction_scene(&route(&state, &held[1]).to_string())
            .is_err()
    );
    adapter.begin_editable_construction(&begin(&other)).unwrap();
}
