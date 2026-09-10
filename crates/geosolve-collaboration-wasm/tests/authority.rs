// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration_wasm::TrustedDocumentHost;
use serde_json::{Value, json};

fn configuration(epoch: &str) -> String {
    json!({"documentId":"doc","documentEpoch":"generation-1","serverEpoch":epoch,"initialInput":"initial-model"}).to_string()
}
fn host() -> (TrustedDocumentHost, Value) {
    let mut state = TrustedDocumentHost::new(&configuration("process-1")).unwrap();
    let connection = decode(
        &state
            .connect("alice", "editor", "client-a", "session-a")
            .unwrap(),
    );
    (state, connection)
}
fn decode(value: &str) -> Value {
    serde_json::from_str(value).unwrap()
}
fn request(connection: &Value, id: &str) -> String {
    json!({"connection":connection,"requestId":id,"command":{"kind":"semantic","basisRevision":0,"payload":{"target":"width","value":12.5}}}).to_string()
}
fn stage(state: &mut TrustedDocumentHost, connection: &Value, id: &str) -> Value {
    decode(&state.stage_admission(&request(connection, id)).unwrap())
}
fn commit(state: &mut TrustedDocumentHost, write: &Value) -> Value {
    decode(
        &state
            .commit_stage(write["stageDigest"].as_str().unwrap())
            .unwrap(),
    )
}

#[test]
fn delayed_persistence_has_no_speculative_receipt_or_visible_revision() {
    let (mut state, connection) = host();
    let write = stage(&mut state, &connection, "one");
    assert_eq!(write["status"], "staged");
    assert!(write.get("receipt").is_none());
    assert_eq!(decode(&state.snapshot().unwrap())["latestSequence"], 0);
    assert_eq!(state.checkpoint().unwrap(), "[]");
    assert_eq!(
        state.receipt(&connection.to_string(), "one").unwrap(),
        "null"
    );
    assert!(state.begin_next().is_err());
    assert!(
        state
            .connect("bob", "editor", "client-b", "session-b")
            .is_err()
    );
    assert!(state.disconnect("session-a").is_err());
    assert!(state.stage_admission(&request(&connection, "two")).is_err());
    assert!(state.commit_stage("forged").is_err());
    assert_eq!(
        decode(&state.resume(&connection.to_string(), 0.0).unwrap())["records"],
        json!([])
    );
    let receipt = commit(&mut state, &write);
    assert_eq!(receipt["admission"], 1);
    assert!(receipt["outcome"].is_null());
    assert!(
        state
            .commit_stage(write["stageDigest"].as_str().unwrap())
            .is_err()
    );
    let ticket = decode(&state.begin_next().unwrap().unwrap());
    assert_eq!(ticket["acceptedInput"], "initial-model");
    let terminal = decode(
        &state
            .stage_validated_completion(
                ticket["ticket"].as_str().unwrap(),
                &json!({"status":"accepted","acceptedInput":"model-1","summary":"width accepted"})
                    .to_string(),
            )
            .unwrap(),
    );
    assert_eq!(decode(&state.snapshot().unwrap())["acceptedRevision"], 0);
    assert!(decode(&state.receipt(&connection.to_string(), "one").unwrap())["outcome"].is_null());
    let final_receipt = commit(&mut state, &terminal);
    assert_eq!(final_receipt["outcome"]["revision"], 1);
    assert_eq!(
        decode(&state.snapshot().unwrap())["acceptedInput"],
        "model-1"
    );
    assert_eq!(
        stage(&mut state, &connection, "one")["receipt"],
        final_receipt
    );
}

#[test]
fn uncertain_append_poison_requires_exact_journal_replay_and_new_sessions() {
    let (mut state, connection) = host();
    let write = stage(&mut state, &connection, "one");
    let durable = decode(write["recordJson"].as_str().unwrap());
    state
        .fail_stage(write["stageDigest"].as_str().unwrap())
        .unwrap();
    assert_eq!(decode(&state.snapshot().unwrap())["needsRecovery"], true);
    assert_eq!(state.checkpoint().unwrap(), "[]");
    assert!(state.stage_admission(&request(&connection, "one")).is_err());
    assert!(state.begin_next().is_err());
    assert!(state.connect("alice", "editor", "client-a", "new").is_err());
    let mut recovered =
        TrustedDocumentHost::restore(&configuration("process-2"), &json!([durable]).to_string())
            .unwrap();
    assert!(recovered.receipt(&connection.to_string(), "one").is_err());
    let renewed = decode(
        &recovered
            .connect("alice", "editor", "client-a", "session-new")
            .unwrap(),
    );
    let duplicate = stage(&mut recovered, &renewed, "one");
    assert_eq!(duplicate["status"], "duplicate");
    assert_eq!(duplicate["receipt"]["admission"], 1);
    assert!(!duplicate.as_object().unwrap().contains_key("recordJson"));
    assert!(recovered.begin_next().unwrap().is_some());
}

#[test]
fn tickets_roles_corrupt_restore_and_fractional_resume_refuse_atomically() {
    let (mut state, connection) = host();
    let viewer = decode(
        &state
            .connect("viewer", "viewer", "client-v", "session-v")
            .unwrap(),
    );
    assert!(state.stage_admission(&request(&viewer, "one")).is_err());
    let mut forged = connection.clone();
    forged["userId"] = json!("other");
    assert!(state.stage_admission(&request(&forged, "one")).is_err());
    let write = stage(&mut state, &connection, "one");
    commit(&mut state, &write);
    let ticket = decode(&state.begin_next().unwrap().unwrap());
    let rejected =
        json!({"status":"rejected","code":"invalid","message":"independent validation refused"})
            .to_string();
    assert!(
        state
            .stage_validated_completion("fabricated", &rejected)
            .is_err()
    );
    let before = state.checkpoint().unwrap();
    let terminal = decode(
        &state
            .stage_validated_completion(ticket["ticket"].as_str().unwrap(), &rejected)
            .unwrap(),
    );
    commit(&mut state, &terminal);
    assert_eq!(decode(&state.snapshot().unwrap())["acceptedRevision"], 0);
    assert!(
        state
            .stage_validated_completion(ticket["ticket"].as_str().unwrap(), &rejected)
            .is_err()
    );
    assert!(state.resume(&connection.to_string(), 0.1).is_err());
    assert!(state.resume(&connection.to_string(), f64::NAN).is_err());
    let mut corrupt = decode(&before);
    corrupt[0]["digest"] = json!("wrong");
    assert!(
        TrustedDocumentHost::restore(&configuration("process-2"), &corrupt.to_string()).is_err()
    );
}

#[test]
fn native_stage_and_worker_handles_cannot_cross_identical_host_instances() {
    let (mut first, connection_a) = host();
    let (mut second, connection_b) = host();
    let write_a = stage(&mut first, &connection_a, "one");
    let write_b = stage(&mut second, &connection_b, "one");
    assert_ne!(write_a["stageDigest"], write_b["stageDigest"]);
    assert!(
        second
            .commit_stage(write_a["stageDigest"].as_str().unwrap())
            .is_err()
    );
    commit(&mut first, &write_a);
    commit(&mut second, &write_b);
    let ticket_a = decode(&first.begin_next().unwrap().unwrap());
    let ticket_b = decode(&second.begin_next().unwrap().unwrap());
    assert_ne!(ticket_a["ticket"], ticket_b["ticket"]);
    assert!(second.stage_validated_completion(ticket_a["ticket"].as_str().unwrap(),&json!({"status":"accepted","acceptedInput":"wrong-model","summary":"foreign worker"}).to_string()).is_err());
    assert_eq!(decode(&second.snapshot().unwrap())["acceptedRevision"], 0);
}
