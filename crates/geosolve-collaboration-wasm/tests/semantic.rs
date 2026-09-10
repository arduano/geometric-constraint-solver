// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration_wasm::TrustedSemanticHost;
use serde_json::{Value, json};

fn config(process: &str) -> String {
    json!({"documentEpoch":"doc-1","serverEpoch":process,"objects":[{"object":"edge","dependencies":["width"]},{"object":"width"},{"object":"height"}]}).to_string()
}
fn host() -> TrustedSemanticHost {
    TrustedSemanticHost::new(&config("process-1")).unwrap()
}
fn decode(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}
fn snapshot(state: &TrustedSemanticHost) -> Value {
    decode(&state.snapshot().unwrap())
}
fn current(state: &TrustedSemanticHost, name: &str) -> Value {
    decode(&state.current(name).unwrap())
}
fn operation(user: &str, id: &str) -> Value {
    json!({"userId":user,"clientId":format!("tab-{user}"),"requestId":id})
}
fn change(state: &TrustedSemanticHost, name: &str, before: i32, after: i32) -> Value {
    json!({"address":{"target":current(state,name),"property":"value"},"before":before,"after":after})
}
fn commit(state: &mut TrustedSemanticHost, write: &Value) {
    state
        .commit_stage(write["stageId"].as_str().unwrap())
        .unwrap();
}
fn record(
    state: &mut TrustedSemanticHost,
    user: &str,
    id: &str,
    name: &str,
    before: i32,
    after: i32,
) {
    let basis = snapshot(state)["revision"].as_u64().unwrap();
    let write = decode(&state.stage_validated_record(&json!({"basisRevision":basis,"revision":basis+1,"operation":operation(user,id),"changes":[change(state,name,before,after)]}).to_string()).unwrap());
    commit(state, &write);
}
fn transact(state: &mut TrustedSemanticHost, mut request: Value) -> Value {
    let basis = snapshot(state)["revision"].as_u64().unwrap();
    request["basisRevision"] = json!(basis);
    request["revision"] = json!(basis + 1);
    let write = decode(
        &state
            .stage_validated_transaction(&request.to_string())
            .unwrap(),
    );
    commit(state, &write);
    write
}
fn inverse(state: &mut TrustedSemanticHost, prepared: &Value, user: &str, id: &str) {
    let revision = snapshot(state)["revision"].as_u64().unwrap() + 1;
    let write = decode(
        &state
            .stage_validated_inverse(
                prepared["ticket"].as_str().unwrap(),
                &operation(user, id).to_string(),
                &revision.to_string(),
            )
            .unwrap(),
    );
    commit(state, &write);
}

#[test]
fn initial_inventory_allocates_forward_references_and_rejects_invalid_graph_inputs() {
    let state = host();
    assert_eq!(snapshot(&state)["dependencyCount"], 1);
    let plan = decode(
        &state
            .plan_delete(&json!([current(&state, "width")]).to_string())
            .unwrap(),
    );
    assert_eq!(plan["closure"].as_array().unwrap().len(), 2);
    for objects in [
        json!([{"object":"a","dependencies":["absent"]}]),
        json!([{"object":"a"},{"object":"a"}]),
        json!([{"object":"a","dependencies":["a"]}]),
    ] {
        let configuration =
            json!({"documentEpoch":"doc-1","serverEpoch":"process-1","objects":objects});
        assert!(TrustedSemanticHost::new(&configuration.to_string()).is_err());
    }
    let mut configuration = decode(&config("process-1"));
    configuration["limits"] = json!({"maxObjectsIncludingTombstones":2});
    assert!(TrustedSemanticHost::new(&configuration.to_string()).is_err());
    configuration["limits"] = json!({"maxTotalDependencies":1_000_001});
    assert!(TrustedSemanticHost::new(&configuration.to_string()).is_err());
}

#[test]
fn staging_reads_committed_state_and_rejects_foreign_or_replayed_durability_tokens() {
    let mut state = host();
    let mut other = host();
    let before = state.checkpoint().unwrap();
    let request=json!({"basisRevision":0,"revision":1,"operation":operation("alice","one"),"changes":[change(&state,"width",12,14)]}).to_string();
    let write = decode(&state.stage_validated_record(&request).unwrap());
    let foreign = decode(&other.stage_validated_record(&request).unwrap());
    assert_eq!(state.checkpoint().unwrap(), before);
    assert_eq!(snapshot(&state)["revision"], 0);
    assert!(state.stage_validated_record(&request).is_err());
    assert!(state.prepare_undo("alice").is_err());
    assert!(
        state
            .commit_stage(foreign["stageId"].as_str().unwrap())
            .is_err()
    );
    assert!(
        state
            .fail_stage(foreign["stageId"].as_str().unwrap())
            .is_err()
    );
    assert_eq!(snapshot(&state)["hasPendingStage"], true);
    commit(&mut state, &write);
    commit(&mut other, &foreign);
    assert_eq!(snapshot(&state)["revision"], 1);
    assert_eq!(
        decode(&state.checkpoint().unwrap())["historyJson"],
        write["historyJson"]
    );
    assert!(
        state
            .commit_stage(write["stageId"].as_str().unwrap())
            .is_err()
    );
    assert!(state.stage_validated_record(&request).is_err());
}

#[test]
fn same_value_contributions_preserve_ownership_in_both_actor_orders() {
    for (first, second) in [("alice", "bob"), ("bob", "alice")] {
        let mut state = host();
        record(&mut state, first, "first", "width", 12, 14);
        let old_ticket = decode(&state.prepare_undo(first).unwrap());
        record(&mut state, second, "second", "width", 14, 14);
        assert!(state.prepare_undo(first).unwrap_err().contains("owns"));
        assert!(
            state
                .stage_validated_inverse(
                    old_ticket["ticket"].as_str().unwrap(),
                    &operation(first, "stale").to_string(),
                    "3"
                )
                .is_err()
        );
        let undo = decode(&state.prepare_undo(second).unwrap());
        assert_eq!(undo["changes"][0]["before"], 14);
        assert_eq!(undo["changes"][0]["after"], 14);
        inverse(&mut state, &undo, second, "undo");
        let undo = decode(&state.prepare_undo(first).unwrap());
        assert_eq!(undo["changes"][0]["after"], 12);
        let address = undo["changes"][0]["address"].to_string();
        assert_eq!(
            decode(&state.property_owner(&address).unwrap()),
            operation(first, "first")
        );
    }
}

#[test]
fn stale_delete_cannot_gain_dependents_or_shrink_reviewed_closure_via_same_transaction() {
    let mut state = host();
    record(&mut state, "alice", "width", "width", 12, 14);
    let original = current(&state, "width");
    let plan = decode(&state.plan_delete(&json!([original]).to_string()).unwrap());
    transact(
        &mut state,
        json!({"create":[{"object":"late","dependencies":[{"kind":"existing","target":original}]}]}),
    );
    assert!(state.authenticate_delete(&plan.to_string()).is_err());
    let before = state.checkpoint().unwrap();
    let attempted=json!({"basisRevision":2,"revision":3,"deletions":[plan],"dependencies":[{"target":{"kind":"existing","target":current(&state,"late")},"dependencies":[]}],"create":[{"object":"should-not-exist"}]}).to_string();
    assert!(state.stage_validated_transaction(&attempted).is_err());
    assert_eq!(state.checkpoint().unwrap(), before);
    let current_plan = decode(&state.plan_delete(&json!([original]).to_string()).unwrap());
    let write = transact(
        &mut state,
        json!({"deletions":[current_plan],"create":[{"object":"width"},{"object":"child","dependencies":[{"kind":"created","object":"parent"}]},{"object":"parent","dependencies":[{"kind":"created","object":"width"}]}]}),
    );
    assert_eq!(write["created"].as_array().unwrap().len(), 3);
    assert!(
        current(&state, "width")["generation"].as_u64().unwrap()
            > original["generation"].as_u64().unwrap()
    );
    assert!(state.authenticate(&original.to_string()).is_err());
    assert!(
        state
            .prepare_undo("alice")
            .unwrap_err()
            .contains("recreated")
    );
    assert_eq!(current(&state, "edge"), Value::Null);
    assert_eq!(current(&state, "late"), Value::Null);
    assert_eq!(snapshot(&state)["historyRevision"], 1);
    assert_eq!(snapshot(&state)["revision"], 3);
}

#[test]
fn lifecycle_and_property_batch_fail_atomically_then_commit_together() {
    let mut state = host();
    record(&mut state, "alice", "first", "width", 12, 14);
    let before = state.checkpoint().unwrap();
    let mut request = json!({"basisRevision":1,"revision":2,"create":[{"object":"new"}],"record":{"operation":operation("alice","second"),"changes":[change(&state,"width",12,16)]}});
    assert!(
        state
            .stage_validated_transaction(&request.to_string())
            .is_err()
    );
    assert_eq!(state.checkpoint().unwrap(), before);
    assert_eq!(current(&state, "new"), Value::Null);
    request["record"]["changes"][0]["before"] = json!(14);
    let write = decode(
        &state
            .stage_validated_transaction(&request.to_string())
            .unwrap(),
    );
    assert_eq!(current(&state, "new"), Value::Null);
    commit(&mut state, &write);
    assert_ne!(current(&state, "new"), Value::Null);
    assert_eq!(snapshot(&state)["historyRevision"], 2);
    let request=json!({"basisRevision":2,"revision":3,"dependencies":[{"target":{"kind":"created","object":"new"},"dependencies":[]}]}).to_string();
    assert!(state.stage_validated_transaction(&request).is_err());
}

#[test]
fn restart_preserves_exact_history_redo_and_rejects_foreign_checkpoints_and_tickets() {
    let mut state = host();
    record(&mut state, "alice", "width", "width", 12, 14);
    record(&mut state, "bob", "height", "height", 8, 9);
    let undo = decode(&state.prepare_undo("alice").unwrap());
    inverse(&mut state, &undo, "alice", "undo");
    let old = decode(&state.prepare_redo("alice").unwrap());
    let checkpoint = state.checkpoint().unwrap();
    let mut restored = TrustedSemanticHost::restore(&config("new-process"), &checkpoint).unwrap();
    assert_eq!(restored.checkpoint().unwrap(), checkpoint);
    assert!(
        restored
            .stage_validated_inverse(
                old["ticket"].as_str().unwrap(),
                &operation("alice", "redo").to_string(),
                "4"
            )
            .is_err()
    );
    let redo = decode(&restored.prepare_redo("alice").unwrap());
    inverse(&mut restored, &redo, "alice", "redo");
    assert_eq!(snapshot(&restored)["revision"], 4);
    let address = change(&restored, "height", 8, 9)["address"].to_string();
    assert_eq!(
        decode(&restored.property_owner(&address).unwrap()),
        operation("bob", "height")
    );
    let mut foreign = decode(&checkpoint);
    foreign["documentEpoch"] = json!("other");
    assert!(TrustedSemanticHost::restore(&config("new"), &foreign.to_string()).is_err());
    let mut ahead = decode(&checkpoint);
    ahead["revision"] = json!(2);
    assert!(TrustedSemanticHost::restore(&config("new"), &ahead.to_string()).is_err());
    let mut corrupt = decode(&checkpoint);
    let mut history = decode(corrupt["historyJson"].as_str().unwrap());
    history["properties"] = json!([]);
    corrupt["historyJson"] = json!(history.to_string());
    assert!(TrustedSemanticHost::restore(&config("new"), &corrupt.to_string()).is_err());
}

#[test]
fn uncertain_persistence_requires_recovery_and_inverse_handles_are_bounded() {
    let mut state = host();
    record(&mut state, "alice", "first", "width", 12, 14);
    let mut handles = Vec::new();
    for _ in 0..32 {
        handles.push(decode(&state.prepare_undo("alice").unwrap()));
    }
    assert!(
        state
            .prepare_undo("alice")
            .unwrap_err()
            .contains("capacity")
    );
    state
        .release(handles[0]["ticket"].as_str().unwrap())
        .unwrap();
    assert!(state.prepare_undo("alice").is_ok());
    let write = decode(
        &state
            .stage_validated_inverse(
                handles[1]["ticket"].as_str().unwrap(),
                &operation("alice", "undo").to_string(),
                "2",
            )
            .unwrap(),
    );
    let durable=json!({"documentEpoch":"doc-1","revision":2,"targetsJson":write["targetsJson"],"historyJson":write["historyJson"]}).to_string();
    state
        .fail_stage(write["stageId"].as_str().unwrap())
        .unwrap();
    assert_eq!(snapshot(&state)["revision"], 1);
    assert_eq!(snapshot(&state)["needsRecovery"], true);
    assert!(
        state
            .prepare_undo("alice")
            .unwrap_err()
            .contains("recovery")
    );
    assert!(
        state
            .commit_stage(write["stageId"].as_str().unwrap())
            .is_err()
    );
    let mut restored = TrustedSemanticHost::restore(&config("new"), &durable).unwrap();
    assert_eq!(snapshot(&restored)["revision"], 2);
    assert!(restored.prepare_redo("alice").is_ok());
}

#[test]
fn structural_history_uses_server_allocated_sibling_references_and_one_mixed_timeline() {
    let mut state = host();
    let write = transact(
        &mut state,
        json!({
            "create":[{"object":"parent"},{"object":"child","dependencies":[{"kind":"created","object":"parent"}]}],
            "record":{"operation":operation("alice","create"),"changes":[],"structural":{"created":[
                {"object":"parent","payload":{"source":"parent()"},"position":{"previous":null,"next":{"kind":"created","object":"child"}}},
                {"object":"child","payload":{"source":"child(parent)"},"position":{"previous":{"kind":"created","object":"parent"},"next":null}}
            ]}}
        }),
    );
    let parent = current(&state, "parent");
    let child = current(&state, "child");
    assert_eq!(write["created"], json!([parent, child]));
    record(&mut state, "alice", "edit", "parent", 12, 14);
    record(&mut state, "bob", "independent", "height", 8, 9);
    let undo = decode(&state.prepare_undo("alice").unwrap());
    assert_eq!(undo["contribution"]["requestId"], "edit");
    inverse(&mut state, &undo, "alice", "undo-edit");
    let undo = decode(&state.prepare_undo("alice").unwrap());
    assert_eq!(
        undo["structural"]["delete"]["closure"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    inverse(&mut state, &undo, "alice", "undo-create");
    assert_eq!(current(&state, "parent"), Value::Null);
    let checkpoint = state.checkpoint().unwrap();
    let mut state = TrustedSemanticHost::restore(&config("restarted"), &checkpoint).unwrap();
    let redo = decode(&state.prepare_redo("alice").unwrap());
    assert_eq!(redo["structural"]["create"].as_array().unwrap().len(), 2);
    assert_eq!(
        redo["structural"]["create"][0]["position"]["next"],
        redo["structural"]["create"][1]["target"]
    );
    inverse(&mut state, &redo, "alice", "redo-create");
    assert!(
        current(&state, "parent")["generation"].as_u64().unwrap()
            > parent["generation"].as_u64().unwrap()
    );
    assert!(state.authenticate(&parent.to_string()).is_err());
    let redo = decode(&state.prepare_redo("alice").unwrap());
    assert_eq!(
        redo["changes"][0]["address"]["target"],
        current(&state, "parent")
    );
    inverse(&mut state, &redo, "alice", "redo-edit");
    let address = change(&state, "height", 8, 9)["address"].to_string();
    assert_eq!(
        decode(&state.property_owner(&address).unwrap()),
        operation("bob", "independent")
    );
}

#[test]
fn structural_deletion_restore_stages_fresh_ids_and_discard_does_not_consume_them() {
    let mut state = host();
    let old = current(&state, "width");
    let edge = current(&state, "edge");
    let plan = decode(&state.plan_delete(&json!([old]).to_string()).unwrap());
    transact(
        &mut state,
        json!({"deletions":[plan],"record":{"operation":operation("alice","delete"),"changes":[],"structural":{"deleted":[
            {"target":old,"payload":{"source":"width()"},"position":{"previous":null,"next":null}},
            {"target":edge,"payload":{"source":"edge(width)"},"position":{"previous":null,"next":null}}
        ]}}}),
    );
    let before = state.checkpoint().unwrap();
    let high = snapshot(&state)["highWater"].clone();
    let prepared = decode(&state.prepare_undo("alice").unwrap());
    let ticket = prepared["ticket"].as_str().unwrap();
    let operation = operation("alice", "undo").to_string();
    let first = decode(
        &state
            .stage_validated_inverse(ticket, &operation, "2")
            .unwrap(),
    );
    assert_eq!(state.checkpoint().unwrap(), before);
    assert_eq!(snapshot(&state)["highWater"], high);
    assert_eq!(first["created"].as_array().unwrap().len(), 2);
    state
        .discard_unpersisted_stage(first["stageId"].as_str().unwrap())
        .unwrap();
    assert_eq!(snapshot(&state)["needsRecovery"], false);
    let second = decode(
        &state
            .stage_validated_inverse(ticket, &operation, "2")
            .unwrap(),
    );
    assert_ne!(first["stageId"], second["stageId"]);
    assert_eq!(first["created"], second["created"]);
    assert!(
        state
            .commit_stage(first["stageId"].as_str().unwrap())
            .is_err()
    );
    commit(&mut state, &second);
    assert!(state.authenticate(&old.to_string()).is_err());
    assert!(state.authenticate(&edge.to_string()).is_err());
    let mut restored =
        TrustedSemanticHost::restore(&config("new-process"), &state.checkpoint().unwrap()).unwrap();
    let redo = decode(&restored.prepare_redo("alice").unwrap());
    inverse(&mut restored, &redo, "alice", "redo-delete");
    assert_eq!(current(&restored, "width"), Value::Null);
}

#[test]
fn incomplete_structural_observations_and_same_value_dependency_interference_reject_atomically() {
    let mut state = host();
    let before = state.checkpoint().unwrap();
    let incomplete = json!({"basisRevision":0,"revision":1,"create":[{"object":"new"}],"record":{"operation":operation("alice","create"),"changes":[],"structural":{}}});
    assert!(
        state
            .stage_validated_transaction(&incomplete.to_string())
            .is_err()
    );
    assert_eq!(state.checkpoint().unwrap(), before);
    assert_eq!(snapshot(&state)["hasPendingStage"], false);
    transact(
        &mut state,
        json!({"create":[{"object":"new"}],"record":{"operation":operation("alice","create"),"changes":[],"structural":{"created":[{"object":"new","payload":{"source":"new()"},"position":{"previous":null,"next":null}}]}}}),
    );
    let target = current(&state, "new");
    transact(
        &mut state,
        json!({"dependencies":[{"target":{"kind":"existing","target":target},"dependencies":[]}],"record":{"operation":operation("bob","same-deps"),"changes":[],"structural":{}}}),
    );
    assert!(state.prepare_undo("alice").unwrap_err().contains("owns"));
    let inverse_plan = decode(&state.prepare_undo("bob").unwrap());
    assert_eq!(inverse_plan["changes"], json!([]));
    assert_eq!(
        inverse_plan["structural"]["dependencies"][0]["before"],
        json!([])
    );
    inverse(&mut state, &inverse_plan, "bob", "undo");
    assert!(state.prepare_undo("alice").is_ok());
}
