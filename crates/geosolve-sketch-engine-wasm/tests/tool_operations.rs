// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine_wasm::EngineAdapter;
use serde_json::{Value, json};
fn decode(value: &str) -> Value {
    serde_json::from_str(value).unwrap()
}
fn open(adapter: &mut EngineAdapter) -> Value {
    let project = CodeProject::managed(
        ProjectKey("tool-operations".into()),
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
#[test]
fn native_tool_adapter_keeps_prediction_receipt_and_publication_separate() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let other = open(&mut adapter);
    let id = state["token"]["session"].to_string();
    let held=decode(&adapter.begin_editable_tool_operation(&json!({"session":state["token"]["session"],"expected":state["token"],"gesture_id":71,"tool":"radius","viewport":{"screen_size":[800,600],"model_center":[0,0],"pixels_per_model_unit":5},"options":{"authoring_options":{"tangent_orientation":"aligned","curvature_relation":"signed","continuity":{"kind":"g1"},"dimension_mode":"reference","angle_orientation":"counter_clockwise"}}}).to_string()).unwrap());
    assert_eq!(held["frame"]["completed"], false);
    assert_eq!(
        held["frame"]["authoring_options"]["dimension_mode"],
        "reference"
    );
    let route =
        json!({"session":state["token"]["session"],"ticket":held["ticket"],"gesture_id":71});
    let mut sample = route.clone();
    sample["sample"] = json!({"sequence":1,"input":{"event":"click","position":[2,0]}});
    let frame = decode(
        &adapter
            .advance_editable_tool_operation(&sample.to_string())
            .unwrap(),
    );
    assert_eq!(frame["completed"], true);
    assert_eq!(frame["diagnostic"], Value::Null);
    assert!(
        adapter
            .advance_editable_tool_operation(&sample.to_string())
            .is_err()
    );
    let presentation = decode(
        &adapter
            .editable_tool_operation_presentation(&route.to_string())
            .unwrap(),
    );
    assert!(presentation["bindings"].is_object());
    assert!(presentation["scene"].is_string());
    let command = decode(
        &adapter
            .finish_editable_tool_operation(&route.to_string())
            .unwrap(),
    );
    assert!(
        adapter
            .finish_editable_tool_operation(&route.to_string())
            .is_err()
    );
    let prepared=decode(&adapter.prepare_editable_tool_operation(&json!({"session":state["token"]["session"],"expected":state["token"],"command":command}).to_string()).unwrap());
    let compilations = decode(include_str!(
        "../../geosolve-sketch-engine/tests/fixtures/tool-operation-compilations.json"
    ));
    let compiled = &compilations["radius"];
    let mut resolve = json!({"session":state["token"]["session"],"ticket":prepared["ticket"],"receipt":{"ticketDigest":prepared["request"]["ticket"]["ticketDigest"],"baseSourceDigest":prepared["request"]["current"]["ir"]["source_digest"],"candidateSourceDigest":compiled["ir"]["source_digest"],"compiled":compiled}});
    let mut forged = resolve.clone();
    forged["receipt"]["ticketDigest"] = "forged".into();
    assert!(
        adapter
            .resolve_editable_tool_operation(&forged.to_string())
            .is_err()
    );
    resolve["session"] = other["token"]["session"].clone();
    assert!(
        adapter
            .resolve_editable_tool_operation(&resolve.to_string())
            .is_err()
    );
    resolve["session"] = state["token"]["session"].clone();
    assert_eq!(decode(&adapter.editable_session_state(&id).unwrap()), state);
    let commit = decode(
        &adapter
            .resolve_editable_tool_operation(&resolve.to_string())
            .unwrap(),
    );
    assert_eq!(decode(&adapter.editable_session_state(&id).unwrap()), state);
    let apply = json!({"session":state["token"]["session"],"ticket":commit["ticket"]});
    let accepted = decode(
        &adapter
            .apply_editable_tool_operation_commit(&apply.to_string())
            .unwrap(),
    );
    assert_eq!(
        accepted["token"]["revision"].as_u64().unwrap(),
        state["token"]["revision"].as_u64().unwrap() + 1
    );
    assert_eq!(
        accepted["result"]["validation"]["hard_residuals_validated"],
        true
    );
    assert!(
        adapter
            .apply_editable_tool_operation_commit(&apply.to_string())
            .is_err()
    );
}

#[test]
fn retained_tool_trace_enforces_the_adapter_reservation_across_requests() {
    let mut adapter = EngineAdapter::new();
    let state = open(&mut adapter);
    let session = &state["token"]["session"];
    let held = decode(&adapter.begin_editable_tool_operation(&json!({
        "session":session,"expected":state["token"],"gesture_id":72,"tool":"parallel",
        "viewport":{"screen_size":[800,600],"model_center":[0,0],"pixels_per_model_unit":5}
    }).to_string()).unwrap());
    let route = json!({"session":session,"ticket":held["ticket"],"gesture_id":72});
    let operands = vec![json!({"target":"datum","datum":"x_axis"}); 256];
    let mut request = route.clone();
    let mut rejected = false;
    for sequence in 1..=140 {
        request["sample"] =
            json!({"sequence":sequence,"input":{"event":"pick_selection","operands":operands}});
        let before = adapter
            .editable_tool_operation_presentation(&route.to_string())
            .unwrap();
        if let Err(error) = adapter.advance_editable_tool_operation(&request.to_string()) {
            assert!(error.contains("trace byte limit"), "{error}");
            assert_eq!(
                adapter
                    .editable_tool_operation_presentation(&route.to_string())
                    .unwrap(),
                before
            );
            request["sample"] = json!({"sequence":sequence,"input":{"event":"reset"}});
            let recovered = decode(
                &adapter
                    .advance_editable_tool_operation(&request.to_string())
                    .unwrap(),
            );
            assert_eq!(recovered["sequence"], sequence);
            rejected = true;
            break;
        }
    }
    assert!(
        rejected,
        "many individually admissible requests must share one trace byte budget"
    );
    assert_eq!(
        decode(
            &adapter
                .editable_session_state(&session.to_string())
                .unwrap()
        ),
        state
    );
    adapter
        .cancel_editable_tool_operation(&route.to_string())
        .unwrap();
}
