// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine_wasm::EngineAdapter;
use serde_json::{Value, json};

fn project() -> String {
    CodeProject::managed(
        ProjectKey("adapter-point".into()),
        CompiledManagedSource::from_json(include_str!(
            "../../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
        ))
        .unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap()
}
fn decode(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}
fn open(adapter: &mut EngineAdapter) -> Value {
    decode(
        &adapter
            .open_editable_session(&json!({"project":project()}).to_string())
            .unwrap(),
    )
}
fn id(state: &Value) -> String {
    state["token"]["session"].to_string()
}
fn begin_request(adapter: &EngineAdapter, state: &Value) -> String {
    let targets = decode(&adapter.editable_point_gesture_targets(&id(state)).unwrap());
    let target = targets
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["position"] == json!([0.0, 0.0]))
        .unwrap();
    json!({"session":state["token"]["session"],"expected":state["token"],"target":target["target"],"gesture_id":7,
        "viewport":{"screen_size":[800.0,600.0],"model_center":[0.0,0.0],"pixels_per_model_unit":10.0}}).to_string()
}
fn route(state: &Value, gesture: &Value) -> Value {
    json!({"session":state["token"]["session"],"ticket":gesture["ticket"],"gesture_id":7})
}
fn sample(route: &Value, sequence: u32, position: [f64; 2]) -> String {
    let mut sample = route.clone();
    sample["sample"] = json!({"sequence":sequence,"position":position});
    sample.to_string()
}
fn command(adapter: &mut EngineAdapter, state: &Value) -> Value {
    let gesture = decode(
        &adapter
            .begin_editable_point_gesture(&begin_request(adapter, state))
            .unwrap(),
    );
    let route = route(state, &gesture);
    for sequence in 1..=4 {
        let at = f64::from(sequence);
        let frame = decode(
            &adapter
                .advance_editable_point_gesture(&sample(&route, sequence, [at, at]))
                .unwrap(),
        );
        assert_eq!(frame["accepted"], true);
        assert_eq!(frame["work"]["native_preview_attempts"], 1);
        assert_eq!(frame["work"]["intent_materialization_attempts"], 0);
        assert_eq!(frame["work"]["history_publications"], 0);
    }
    let scene: Value = serde_json::from_str(
        &adapter
            .editable_point_gesture_scene(&route.to_string())
            .unwrap(),
    )
    .unwrap();
    assert!(scene.is_object());
    let terminal = decode(
        &adapter
            .finish_editable_point_gesture(&route.to_string())
            .unwrap(),
    );
    assert!(
        adapter
            .finish_editable_point_gesture(&route.to_string())
            .is_err()
    );
    terminal["command"].clone()
}
fn prepare(adapter: &mut EngineAdapter, state: &Value, command: &Value) -> Value {
    decode(&adapter.prepare_editable_point_commit(&json!({"session":state["token"]["session"],"expected":state["token"],"command":command}).to_string()).unwrap())
}
fn commit_request(state: &Value, prepared: &Value) -> String {
    json!({"session":state["token"]["session"],"ticket":prepared["ticket"]}).to_string()
}
fn independent_bar(result: &Value) {
    let points = result["geometry"]["points"].as_array().unwrap();
    assert_eq!(points.len(), 2);
    let position = |i: usize, axis: usize| points[i]["position"][axis].as_f64().unwrap();
    let dx = position(1, 0) - position(0, 0);
    let dy = position(1, 1) - position(0, 1);
    assert!(dx.is_finite() && dy.is_finite());
    assert!((dx.hypot(dy) - 20.0).abs() < 1e-9);
    assert!(dy.abs() < 1e-9);
    assert!(dx > 0.0);
    assert_eq!(result["validation"]["hard_residuals_validated"], true);
}

#[test]
fn retained_wire_frames_and_independent_staged_commit_restore_complete_input() {
    let mut client = EngineAdapter::new();
    let client_state = open(&mut client);
    let before = client.editable_session_state(&id(&client_state)).unwrap();
    let command = command(&mut client, &client_state);
    assert_eq!(
        client.editable_session_state(&id(&client_state)).unwrap(),
        before
    );
    let mut server = EngineAdapter::new();
    let state = open(&mut server);
    let prepared = prepare(&mut server, &state, &command);
    assert_eq!(
        decode(&server.editable_session_state(&id(&state)).unwrap()),
        state
    );
    independent_bar(&prepared["result"]);
    let mut restored = EngineAdapter::new();
    let cold = decode(
        &restored
            .open_editable_session(
                &json!({"project":prepared["project"],"design":prepared["design"].to_string()})
                    .to_string(),
            )
            .unwrap(),
    );
    assert_eq!(
        restored.editable_source_design_digest(&id(&cold)).unwrap(),
        prepared["source_design_digest"]
    );
    independent_bar(&cold["result"]);
    let request = commit_request(&state, &prepared);
    let accepted = decode(&server.apply_editable_point_commit(&request).unwrap());
    independent_bar(&accepted["result"]);
    assert_eq!(
        accepted["result"]["geometry"],
        prepared["result"]["geometry"]
    );
    // Cold native IDs are evaluation-scoped; semantic addresses and source seeds are durable.
    assert_eq!(
        restored.editable_point_gesture_targets(&id(&cold)).unwrap(),
        server
            .editable_point_gesture_targets(&id(&accepted))
            .unwrap()
    );
    assert_eq!(
        cold["result"]["geometry"]["curves"][0]["curve"]["definition"]["branch_direction"],
        accepted["result"]["geometry"]["curves"][0]["curve"]["definition"]["branch_direction"]
    );
    assert_eq!(
        accepted["token"]["revision"].as_u64().unwrap(),
        state["token"]["revision"].as_u64().unwrap() + 1
    );
    assert!(server.apply_editable_point_commit(&request).is_err());
    assert_eq!(
        decode(&server.editable_session_state(&id(&state)).unwrap()),
        accepted
    );
    assert!(server.prepare_editable_point_commit(&json!({"session":state["token"]["session"],"expected":accepted["token"],"command":command}).to_string()).is_err());
}

#[test]
fn invalid_routing_sequences_cancellation_and_session_disposal_preserve_authority() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let other = open(&mut adapter);
    let gesture = decode(
        &adapter
            .begin_editable_point_gesture(&begin_request(&adapter, &state))
            .unwrap(),
    );
    let route = route(&state, &gesture);
    let mut wrong = route.clone();
    wrong["session"] = other["token"]["session"].clone();
    assert!(
        adapter
            .cancel_editable_point_gesture(&wrong.to_string())
            .is_err()
    );
    assert!(
        adapter
            .advance_editable_point_gesture(&sample(&wrong, 1, [4.0, 4.0]))
            .is_err()
    );
    wrong = route.clone();
    wrong["gesture_id"] = json!(8);
    assert!(
        adapter
            .finish_editable_point_gesture(&wrong.to_string())
            .is_err()
    );
    assert!(
        adapter
            .advance_editable_point_gesture(&sample(&route, 2, [4.0, 4.0]))
            .is_err()
    );
    assert!(
        adapter
            .advance_editable_point_gesture(&sample(&route, 1, [f64::NAN, 4.0]))
            .is_err()
    );
    adapter
        .advance_editable_point_gesture(&sample(&route, 1, [4.0, 4.0]))
        .unwrap();
    assert!(
        adapter
            .advance_editable_point_gesture(&sample(&route, 1, [8.0, 8.0]))
            .is_err()
    );
    adapter
        .cancel_editable_point_gesture(&route.to_string())
        .unwrap();
    assert!(
        adapter
            .editable_point_gesture_scene(&route.to_string())
            .is_err()
    );
    assert_eq!(
        decode(&adapter.editable_session_state(&id(&state)).unwrap()),
        state
    );
    let gesture = decode(
        &adapter
            .begin_editable_point_gesture(&begin_request(&adapter, &state))
            .unwrap(),
    );
    let pending = route_for(&state, &gesture);
    let command = command(&mut adapter, &state);
    let prepared = prepare(&mut adapter, &state, &command);
    assert!(adapter.close_editable_session(&id(&state)));
    assert!(adapter.editable_point_gesture_scene(&pending).is_err());
    assert!(
        adapter
            .apply_editable_point_commit(&commit_request(&state, &prepared))
            .is_err()
    );
}
fn route_for(state: &Value, gesture: &Value) -> String {
    route(state, gesture).to_string()
}

#[test]
fn foreign_stale_and_released_preparations_cannot_publish() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let other = open(&mut adapter);
    let command = command(&mut adapter, &state);
    let prepared = prepare(&mut adapter, &state, &command);
    let competing = prepare(&mut adapter, &state, &command);
    assert!(
        adapter
            .apply_editable_point_commit(&commit_request(&other, &prepared))
            .is_err()
    );
    assert!(
        adapter
            .release_editable_point_commit(&commit_request(&other, &prepared))
            .is_err()
    );
    let mut foreign = EngineAdapter::new();
    open(&mut foreign);
    assert!(
        foreign
            .apply_editable_point_commit(&commit_request(&state, &prepared))
            .is_err()
    );
    adapter
        .apply_editable_point_commit(&commit_request(&state, &competing))
        .unwrap();
    let accepted = adapter.editable_session_state(&id(&state)).unwrap();
    assert!(
        adapter
            .apply_editable_point_commit(&commit_request(&state, &prepared))
            .is_err()
    );
    assert_eq!(
        adapter.editable_session_state(&id(&state)).unwrap(),
        accepted
    );
    adapter
        .release_editable_point_commit(&commit_request(&state, &prepared))
        .unwrap();
    assert!(
        adapter
            .release_editable_point_commit(&commit_request(&state, &prepared))
            .is_err()
    );
}

#[test]
fn point_handle_and_wire_bounds_release_capacity_without_changing_accepted_state() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let input = begin_request(&adapter, &state);
    let mut gestures = Vec::new();
    for _ in 0..8 {
        gestures.push(decode(
            &adapter.begin_editable_point_gesture(&input).unwrap(),
        ));
    }
    assert!(
        adapter
            .begin_editable_point_gesture(&input)
            .unwrap_err()
            .contains("eight")
    );
    adapter
        .cancel_editable_point_gesture(&route_for(&state, &gestures[0]))
        .unwrap();
    adapter.begin_editable_point_gesture(&input).unwrap();
    assert!(adapter.close_editable_session(&id(&state)));
    let state = open(&mut adapter);
    let command = command(&mut adapter, &state);
    let mut prepared = Vec::new();
    for _ in 0..8 {
        prepared.push(prepare(&mut adapter, &state, &command));
    }
    let input =
        json!({"session":state["token"]["session"],"expected":state["token"],"command":command})
            .to_string();
    assert!(
        adapter
            .prepare_editable_point_commit(&input)
            .unwrap_err()
            .contains("eight")
    );
    adapter
        .release_editable_point_commit(&commit_request(&state, &prepared[0]))
        .unwrap();
    prepare(&mut adapter, &state, &command);
    assert!(
        adapter
            .advance_editable_point_gesture(&" ".repeat(1024 * 1024 + 1))
            .unwrap_err()
            .contains("byte limit")
    );
    assert_eq!(
        decode(&adapter.editable_session_state(&id(&state)).unwrap()),
        state
    );
}
