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
