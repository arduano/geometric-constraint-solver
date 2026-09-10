// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration::{
    SharedTextDocument, SharedTextLimits,
    source_patch::{SourceEdit, SourcePatch, source_digest},
};
use geosolve_collaboration_wasm::TrustedSourceHost;
use serde_json::{Value, json};
fn config(process: &str) -> String {
    json!({"documentEpoch":"doc-1","serverEpoch":process,"initialInput":"initial-model","files":{"main.ts":"const width = 12;\n","helper.ts":"export const = (\n"}}).to_string()
}
fn host() -> TrustedSourceHost {
    TrustedSourceHost::new(&config("process-1"), b"server").unwrap()
}
fn decode(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}
fn snapshot(state: &TrustedSourceHost) -> Value {
    decode(&state.snapshot().unwrap())
}
fn width_patch() -> Value {
    let old = "const width = 12;\n";
    let new = "const width = 14;\n";
    serde_json::to_value(SourcePatch {
        base_source_digest: source_digest(old),
        candidate_source_digest: source_digest(new),
        edits: vec![SourceEdit {
            start: 14,
            end: 16,
            expected: "12".into(),
            replacement: "14".into(),
        }],
    })
    .unwrap()
}
fn edit(state: &mut TrustedSourceHost, edits: &Value) -> Value {
    let basis = snapshot(state)["working"]["revision"].to_string();
    let write = decode(&state.stage_host_edits(&basis, &edits.to_string()).unwrap());
    decode(
        &state
            .commit_stage(write["stageId"].as_str().unwrap())
            .unwrap(),
    )
}
#[test]
fn text_ack_staging_and_model_preparation_remain_independent() {
    let mut state = host();
    let prepared = decode(
        &state
            .prepare_canvas_update(&json!([{"path":"main.ts","patch":width_patch()}]).to_string())
            .unwrap(),
    );
    let original = snapshot(&state);
    let old_heads = original["working"]["revision"].to_string();
    let write=decode(&state.stage_host_edits(&old_heads,&json!([{"kind":"splice","path":"main.ts","start_utf16":0,"delete_utf16":17,"insert":"const width ="}]).to_string()).unwrap());
    assert_eq!(snapshot(&state)["working"], original["working"]);
    assert_eq!(snapshot(&state)["sequence"], 0);
    assert!(state.capture_apply().is_err());
    assert!(state.commit_stage("foreign").is_err());
    state
        .commit_stage(write["stageId"].as_str().unwrap())
        .unwrap();
    let invalid = snapshot(&state);
    let updates =
        json!([{"kind":"pending","path":"main.ts","reason":"incomplete target"}]).to_string();
    assert!(
        state
            .stage_validated_publication(
                prepared["ticket"].as_str().unwrap(),
                "model-14",
                &old_heads,
                &updates
            )
            .is_err()
    );
    let terminal = decode(
        &state
            .stage_validated_publication(
                prepared["ticket"].as_str().unwrap(),
                "model-14",
                &invalid["working"]["revision"].to_string(),
                &updates,
            )
            .unwrap(),
    );
    assert_eq!(snapshot(&state)["accepted"], original["accepted"]);
    let durable = terminal["checkpointJson"].as_str().unwrap().to_owned();
    state
        .commit_stage(terminal["stageId"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        snapshot(&state)["accepted"]["files"]["main.ts"],
        "const width = 14;\n"
    );
    assert_eq!(snapshot(&state)["working"], invalid["working"]);
    assert_eq!(
        snapshot(&state)["pendingNotices"].as_array().unwrap().len(),
        1
    );
    let restored =
        TrustedSourceHost::restore(&config("process-2"), b"new-server", &durable).unwrap();
    assert_eq!(snapshot(&restored), snapshot(&state));
}
#[test]
fn durable_apply_capture_restores_old_heads_basis_and_ids_after_later_typing() {
    let mut state = host();
    edit(
        &mut state,
        &json!([{"kind":"splice","path":"main.ts","start_utf16":14,"delete_utf16":2,"insert":"14"}]),
    );
    let captured = decode(&state.capture_apply().unwrap());
    edit(
        &mut state,
        &json!([{"kind":"splice","path":"main.ts","start_utf16":14,"delete_utf16":2,"insert":"16"}]),
    );
    let mut restored = TrustedSourceHost::restore(
        &config("process-2"),
        b"new-server",
        &state.checkpoint().unwrap(),
    )
    .unwrap();
    assert!(
        restored
            .prepare_apply_update(
                captured["handle"].as_str().unwrap(),
                &captured["working"]["files"].to_string()
            )
            .is_err()
    );
    let restored_capture = decode(
        &restored
            .restore_apply_capture(
                captured["captureJson"].as_str().unwrap(),
                &captured["acceptedBasis"].to_string(),
            )
            .unwrap(),
    );
    assert_eq!(restored_capture["working"], captured["working"]);
    assert_eq!(restored_capture["fileIds"], captured["fileIds"]);
    let prepared = decode(
        &restored
            .prepare_apply_update(
                restored_capture["handle"].as_str().unwrap(),
                &restored_capture["working"]["files"].to_string(),
            )
            .unwrap(),
    );
    let terminal = decode(
        &restored
            .stage_validated_publication(
                prepared["ticket"].as_str().unwrap(),
                "model-14",
                &snapshot(&restored)["working"]["revision"].to_string(),
                &json!([{"kind":"captured","path":"main.ts"}]).to_string(),
            )
            .unwrap(),
    );
    restored
        .commit_stage(terminal["stageId"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        snapshot(&restored)["accepted"]["files"]["main.ts"],
        "const width = 14;\n"
    );
    assert_eq!(
        snapshot(&restored)["working"]["files"]["main.ts"],
        "const width = 16;\n"
    );
    let mut forged = decode(captured["captureJson"].as_str().unwrap());
    forged["files"]["main.ts"] = json!("forged");
    assert!(
        restored
            .restore_apply_capture(&forged.to_string(), &captured["acceptedBasis"].to_string())
            .is_err()
    );
}
#[test]
fn authenticated_text_failure_and_uncertain_durability_retain_committed_state() {
    let mut state = host();
    let mut client = SharedTextDocument::load(
        &state.text_checkpoint(),
        b"client",
        SharedTextLimits::default(),
    )
    .unwrap();
    let basis = client.revision();
    client.splice("main.ts", 14, 2, "16").unwrap();
    let changes = serde_json::to_string(&client.changes_since(&basis).unwrap()).unwrap();
    let before = snapshot(&state);
    assert!(state.stage_text_changes(&changes, b"forged").is_err());
    assert_eq!(snapshot(&state), before);
    let write = decode(&state.stage_text_changes(&changes, b"client").unwrap());
    assert_eq!(snapshot(&state)["working"], before["working"]);
    let durable = write["checkpointJson"].as_str().unwrap().to_owned();
    state
        .fail_stage(write["stageId"].as_str().unwrap())
        .unwrap();
    assert_eq!(snapshot(&state)["needsRecovery"], true);
    assert!(state.capture_apply().is_err());
    assert!(state.stage_text_changes(&changes, b"client").is_err());
    let restored =
        TrustedSourceHost::restore(&config("new-process"), b"new-server", &durable).unwrap();
    assert_eq!(
        snapshot(&restored)["working"]["files"]["main.ts"],
        "const width = 16;\n"
    );
    assert_eq!(snapshot(&restored)["accepted"], before["accepted"]);
}
#[test]
fn bounded_capture_handles_release_and_foreign_envelopes_refuse() {
    let mut state = host();
    let mut handles = Vec::new();
    for _ in 0..32 {
        handles.push(
            decode(&state.capture_apply().unwrap())["handle"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    assert!(state.capture_apply().is_err());
    state.release_handle(&handles[0]).unwrap();
    assert!(state.capture_apply().is_ok());
    assert!(state.apply_needs_rebase(&handles[0]).is_err());
    let mut foreign = decode(&state.checkpoint().unwrap());
    foreign["documentEpoch"] = json!("other");
    assert!(TrustedSourceHost::restore(&config("new"), b"new", &foreign.to_string()).is_err());
}

#[test]
fn restore_authenticates_inner_epoch_and_source_model_sequence_relationship() {
    let mut state = host();
    let prepared = decode(
        &state
            .prepare_canvas_update(&json!([{"path":"main.ts","patch":width_patch()}]).to_string())
            .unwrap(),
    );
    let terminal = decode(
        &state
            .stage_validated_publication(
                prepared["ticket"].as_str().unwrap(),
                "model-14",
                &snapshot(&state)["working"]["revision"].to_string(),
                &json!([{"kind":"reconciled","path":"main.ts","patch":width_patch()}]).to_string(),
            )
            .unwrap(),
    );
    state
        .commit_stage(terminal["stageId"].as_str().unwrap())
        .unwrap();
    let checkpoint = decode(&state.checkpoint().unwrap());
    let mut invalid_sequence = checkpoint.clone();
    invalid_sequence["sequence"] = json!(0);
    assert!(
        TrustedSourceHost::restore(&config("new"), b"new", &invalid_sequence.to_string()).is_err()
    );
    let mut invalid_epoch = checkpoint.clone();
    let mut source = decode(checkpoint["sourceJson"].as_str().unwrap());
    source["documentEpoch"] = json!("other");
    invalid_epoch["sourceJson"] = json!(source.to_string());
    assert!(
        TrustedSourceHost::restore(&config("new"), b"new", &invalid_epoch.to_string()).is_err()
    );
}

#[test]
fn known_unpersisted_source_stage_discards_without_poison_and_old_stage_cannot_reappear() {
    let mut state = host();
    let before = state.checkpoint().unwrap();
    let revision = snapshot(&state)["working"]["revision"].to_string();
    let edits =
        json!([{"kind":"splice","path":"main.ts","start_utf16":14,"delete_utf16":2,"insert":"14"}])
            .to_string();
    let first = decode(&state.stage_host_edits(&revision, &edits).unwrap());
    state
        .discard_unpersisted_stage(first["stageId"].as_str().unwrap())
        .unwrap();
    assert_eq!(state.checkpoint().unwrap(), before);
    assert_eq!(snapshot(&state)["needsRecovery"], false);
    let second = decode(&state.stage_host_edits(&revision, &edits).unwrap());
    assert_ne!(first["stageId"], second["stageId"]);
    assert!(
        state
            .commit_stage(first["stageId"].as_str().unwrap())
            .is_err()
    );
    state
        .fail_stage(second["stageId"].as_str().unwrap())
        .unwrap();
    assert!(
        state
            .discard_unpersisted_stage(second["stageId"].as_str().unwrap())
            .is_err()
    );
    assert_eq!(snapshot(&state)["needsRecovery"], true);
    assert_eq!(state.checkpoint().unwrap(), before);
}

fn user_operation(user: &str, id: &str) -> String {
    json!({"userId":user,"clientId":format!("tab-{user}"),"requestId":id}).to_string()
}
fn commit_user_stage(state: &mut TrustedSourceHost, write: &str) {
    let write = decode(write);
    state
        .commit_stage(write["stageId"].as_str().unwrap())
        .unwrap();
}
fn user_type(state: &mut TrustedSourceHost, user: &str, id: &str, insert: &str) {
    let mut client = SharedTextDocument::load(
        &state.text_checkpoint(),
        user.as_bytes(),
        SharedTextLimits::default(),
    )
    .unwrap();
    let basis = client.revision();
    client.splice("main.ts", 14, 2, insert).unwrap();
    let write = state
        .stage_user_text_changes(
            &json!(client.changes_since(&basis).unwrap()).to_string(),
            user.as_bytes(),
            &user_operation(user, id),
        )
        .unwrap();
    commit_user_stage(state, &write);
}
#[test]
fn personal_text_staging_is_durable_and_checked_after_restart_with_a_new_server_actor() {
    let mut state = host();
    user_type(&mut state, "alice", "first", "14");
    user_type(&mut state, "alice", "second", "16");
    let before = state.checkpoint().unwrap();
    let undo = state
        .stage_user_undo(&user_operation("alice", "undo-second"))
        .unwrap();
    assert_eq!(state.checkpoint().unwrap(), before);
    assert_eq!(
        decode(&state.user_history("alice").unwrap())["undoCount"],
        2
    );
    let first = decode(&undo);
    state
        .discard_unpersisted_stage(first["stageId"].as_str().unwrap())
        .unwrap();
    let retry = state
        .stage_user_undo(&user_operation("alice", "undo-second"))
        .unwrap();
    assert_ne!(decode(&retry)["stageId"], first["stageId"]);
    commit_user_stage(&mut state, &retry);
    let saved = state.checkpoint().unwrap();
    let mut state = TrustedSourceHost::restore(&config("new"), b"new-server", &saved).unwrap();
    assert_eq!(
        snapshot(&state)["working"]["files"]["main.ts"],
        "const width = 14;\n"
    );
    let undo = state
        .stage_user_undo(&user_operation("alice", "undo-first"))
        .unwrap();
    commit_user_stage(&mut state, &undo);
    assert_eq!(
        snapshot(&state)["working"]["files"]["main.ts"],
        "const width = 12;\n"
    );
    let saved = state.checkpoint().unwrap();
    let mut state = TrustedSourceHost::restore(&config("third"), b"third-server", &saved).unwrap();
    let redo = state
        .stage_user_redo(&user_operation("alice", "redo-first"))
        .unwrap();
    commit_user_stage(&mut state, &redo);
    assert_eq!(
        snapshot(&state)["working"]["files"]["main.ts"],
        "const width = 14;\n"
    );
    assert_eq!(snapshot(&state)["accepted"]["modelRevision"], 0);
}
#[test]
fn personal_file_lifecycle_and_foreign_same_value_ownership_share_one_source_timeline() {
    let mut state = host();
    user_type(&mut state, "alice", "type", "14");
    user_type(&mut state, "bob", "same", "14");
    assert!(
        state
            .stage_user_undo(&user_operation("alice", "blocked"))
            .is_err()
    );
    let undo = state
        .stage_user_undo(&user_operation("bob", "undo"))
        .unwrap();
    commit_user_stage(&mut state, &undo);
    let original = snapshot(&state)["fileIds"]["main.ts"].clone();
    let basis = snapshot(&state)["working"]["revision"].to_string();
    let write = state
        .stage_user_file_edits(
            &basis,
            &json!([{"kind":"remove_file","path":"main.ts"}]).to_string(),
            &user_operation("alice", "delete"),
        )
        .unwrap();
    commit_user_stage(&mut state, &write);
    let saved = state.checkpoint().unwrap();
    let mut state = TrustedSourceHost::restore(&config("new"), b"new-server", &saved).unwrap();
    let write = state
        .stage_user_undo(&user_operation("alice", "restore"))
        .unwrap();
    commit_user_stage(&mut state, &write);
    assert_ne!(snapshot(&state)["fileIds"]["main.ts"], original);
    let write = state
        .stage_user_undo(&user_operation("alice", "undo-type"))
        .unwrap();
    commit_user_stage(&mut state, &write);
    assert_eq!(
        snapshot(&state)["working"]["files"]["main.ts"],
        "const width = 12;\n"
    );
}
