// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration::{
    SharedTextLimits,
    document::{AcceptedFilePatch, SourceDocument, SourceDocumentError, WorkingSourceChange},
    source_patch::{SourceEdit, SourcePatch, source_digest},
};
use std::collections::BTreeMap;
fn document() -> SourceDocument {
    SourceDocument::new(
        "document-epoch".into(),
        "validated-initial".into(),
        BTreeMap::from([
            ("main.ts".into(), "const width = 12;\n".into()),
            ("helper.ts".into(), "export const height = 8;\n".into()),
        ]),
        b"server",
        SharedTextLimits::default(),
    )
    .unwrap()
}
fn patch(source: &str, before: &str, after: &str) -> SourcePatch {
    let byte = source.find(before).unwrap();
    let start = source[..byte].encode_utf16().count();
    SourcePatch {
        base_source_digest: source_digest(source),
        candidate_source_digest: source_digest(&source.replacen(before, after, 1)),
        edits: vec![SourceEdit {
            start,
            end: start + before.encode_utf16().count(),
            expected: before.into(),
            replacement: after.into(),
        }],
    }
}
#[test]
fn accepted_canvas_update_retains_invalid_working_draft_and_pending_notice_durably() {
    let mut state = document();
    let source_patch = patch(&state.accepted().files["main.ts"], "12", "14");
    let prepared = state
        .prepare_canvas_update(&[AcceptedFilePatch {
            path: "main.ts".into(),
            patch: source_patch,
        }])
        .unwrap();
    state
        .working_mut()
        .splice("main.ts", 0, 17, "const width =")
        .unwrap();
    let invalid = state.working().save();
    state
        .publish_validated(
            &prepared,
            "validated-width14".into(),
            &state.working().revision(),
            &[WorkingSourceChange::Pending {
                path: "main.ts".into(),
                reason: "Target syntax is incomplete".into(),
            }],
        )
        .unwrap();
    assert_eq!(state.accepted().files["main.ts"], "const width = 14;\n");
    assert_eq!(state.working().save(), invalid);
    assert_eq!(state.reconciliation_pending().len(), 1);
    let restored = SourceDocument::from_json(
        &state.to_json().unwrap(),
        b"new-server",
        SharedTextLimits::default(),
    )
    .unwrap();
    assert_eq!(restored.accepted(), state.accepted());
    assert_eq!(restored.working().capture(), state.working().capture());
    assert_eq!(
        restored.reconciliation_pending(),
        state.reconciliation_pending()
    );
}
#[test]
fn canvas_solve_does_not_block_typing_and_completion_rechecks_latest_text() {
    let mut state = document();
    let source_patch = patch(&state.accepted().files["main.ts"], "12", "14");
    let prepared = state
        .prepare_canvas_update(&[AcceptedFilePatch {
            path: "main.ts".into(),
            patch: source_patch.clone(),
        }])
        .unwrap();
    let old_working = state.working().revision();
    state
        .working_mut()
        .splice("helper.ts", 0, 0, "// typing while solve is held\n")
        .unwrap();
    let before = state.to_json().unwrap();
    assert_eq!(
        state.publish_validated(
            &prepared,
            "new-input".into(),
            &old_working,
            &[WorkingSourceChange::Reconciled {
                path: "main.ts".into(),
                patch: source_patch.clone()
            }]
        ),
        Err(SourceDocumentError::StaleWorking)
    );
    assert_eq!(state.to_json().unwrap(), before);
    state
        .publish_validated(
            &prepared,
            "new-input".into(),
            &state.working().revision(),
            &[WorkingSourceChange::Reconciled {
                path: "main.ts".into(),
                patch: source_patch,
            }],
        )
        .unwrap();
    assert!(
        state
            .working()
            .capture()
            .text("helper.ts")
            .unwrap()
            .starts_with("// typing")
    );
    assert_eq!(
        state.working().capture().text("main.ts"),
        Some("const width = 14;\n")
    );
}
#[test]
fn explicit_apply_captures_immutable_source_and_later_typing_stays_unapplied() {
    let mut state = document();
    state.working_mut().splice("main.ts", 14, 2, "14").unwrap();
    let capture = state.capture_apply().unwrap();
    assert!(!state.apply_needs_rebase(&capture).unwrap());
    let prepared = state
        .prepare_apply_update(&capture, capture.working().files().clone())
        .unwrap();
    state.working_mut().splice("main.ts", 14, 2, "16").unwrap();
    state
        .publish_validated(
            &prepared,
            "validated-captured14".into(),
            &state.working().revision(),
            &[WorkingSourceChange::Captured {
                path: "main.ts".into(),
            }],
        )
        .unwrap();
    assert_eq!(state.accepted().files["main.ts"], "const width = 14;\n");
    assert_eq!(
        state.working().capture().text("main.ts"),
        Some("const width = 16;\n")
    );
    assert_eq!(
        capture.working().text("main.ts"),
        Some("const width = 14;\n")
    );
    assert_eq!(
        capture.accepted_basis().files["main.ts"],
        "const width = 12;\n"
    );
    assert!(state.apply_needs_rebase(&capture).unwrap());
}
#[test]
fn publication_is_atomic_across_files_and_replayed_preparation_is_stale() {
    let mut state = document();
    let width = patch(&state.accepted().files["main.ts"], "12", "14");
    let height = patch(&state.accepted().files["helper.ts"], "8", "9");
    let prepared = state
        .prepare_canvas_update(&[
            AcceptedFilePatch {
                path: "main.ts".into(),
                patch: width.clone(),
            },
            AcceptedFilePatch {
                path: "helper.ts".into(),
                patch: height.clone(),
            },
        ])
        .unwrap();
    let before = state.to_json().unwrap();
    let mut forged = height.clone();
    forged.base_source_digest = "forged".into();
    assert_eq!(
        state.publish_validated(
            &prepared,
            "new-input".into(),
            &state.working().revision(),
            &[
                WorkingSourceChange::Reconciled {
                    path: "main.ts".into(),
                    patch: width.clone()
                },
                WorkingSourceChange::Reconciled {
                    path: "helper.ts".into(),
                    patch: forged
                }
            ]
        ),
        Err(SourceDocumentError::Patch)
    );
    assert_eq!(state.to_json().unwrap(), before);
    let updates = [
        WorkingSourceChange::Reconciled {
            path: "main.ts".into(),
            patch: width,
        },
        WorkingSourceChange::Reconciled {
            path: "helper.ts".into(),
            patch: height,
        },
    ];
    state
        .publish_validated(
            &prepared,
            "new-input".into(),
            &state.working().revision(),
            &updates,
        )
        .unwrap();
    let after = state.to_json().unwrap();
    assert_eq!(
        state.publish_validated(
            &prepared,
            "newer-input".into(),
            &state.working().revision(),
            &updates
        ),
        Err(SourceDocumentError::StaleAccepted)
    );
    assert_eq!(state.to_json().unwrap(), after);
}
#[test]
fn applying_file_lifecycle_preserves_captured_ids_and_newer_source() {
    let mut state = document();
    let original_id = state.working().file_id("helper.ts").unwrap();
    state
        .working_mut()
        .rename_file("helper.ts", "renamed.ts")
        .unwrap();
    let capture = state.capture_apply().unwrap();
    assert_eq!(capture.file_ids()["renamed.ts"], original_id);
    let prepared = state
        .prepare_apply_update(&capture, capture.working().files().clone())
        .unwrap();
    state
        .working_mut()
        .splice("renamed.ts", 0, 0, "// later\n")
        .unwrap();
    state
        .publish_validated(
            &prepared,
            "renamed-input".into(),
            &state.working().revision(),
            &[
                WorkingSourceChange::Captured {
                    path: "helper.ts".into(),
                },
                WorkingSourceChange::Captured {
                    path: "renamed.ts".into(),
                },
            ],
        )
        .unwrap();
    assert!(!state.accepted().files.contains_key("helper.ts"));
    assert!(
        state
            .working()
            .capture()
            .text("renamed.ts")
            .unwrap()
            .starts_with("// later\n")
    );
}

#[test]
fn durable_apply_restores_exact_captured_heads_and_lifetimes_after_later_edits() {
    let mut state = document();
    state.working_mut().splice("main.ts", 14, 2, "14").unwrap();
    state
        .working_mut()
        .rename_file("helper.ts", "renamed.ts")
        .unwrap();
    let capture = state.capture_apply().unwrap();
    let encoded = state.encode_apply_capture(&capture).unwrap();
    state.working_mut().splice("main.ts", 14, 2, "16").unwrap();
    state.working_mut().remove_file("renamed.ts").unwrap();
    state
        .working_mut()
        .create_file("renamed.ts", "// replacement lifetime")
        .unwrap();
    let restored = SourceDocument::from_json(
        &state.to_json().unwrap(),
        b"restart",
        SharedTextLimits::default(),
    )
    .unwrap();
    let recovered = restored
        .restore_apply_capture(&encoded, capture.accepted_basis())
        .unwrap();
    assert_eq!(recovered.working(), capture.working());
    assert_eq!(recovered.file_ids(), capture.file_ids());
    assert_ne!(
        recovered.file_ids()["renamed.ts"],
        restored.working().file_id("renamed.ts").unwrap()
    );
    assert_eq!(
        recovered.working().text("main.ts"),
        Some("const width = 14;\n")
    );
    assert_eq!(
        restored.working().capture().text("main.ts"),
        Some("const width = 16;\n")
    );
    let before = restored.to_json().unwrap();
    for field in [
        "files",
        "fileIds",
        "acceptedBasis",
        "documentEpoch",
        "workingRevision",
    ] {
        let mut changed: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        match field {
            "files" => changed[field]["main.ts"] = "const width = 99;".into(),
            "fileIds" => {
                changed[field]["renamed.ts"] =
                    restored.working().file_id("renamed.ts").unwrap().into();
            }
            "acceptedBasis" => changed[field]["acceptedInput"] = "other-input".into(),
            "documentEpoch" => changed[field] = "other-epoch".into(),
            _ => changed[field]["heads"] = serde_json::json!(["00".repeat(32)]),
        }
        assert!(
            restored
                .restore_apply_capture(&changed.to_string(), capture.accepted_basis())
                .is_err(),
            "{field}"
        );
        assert_eq!(restored.to_json().unwrap(), before);
    }
}

#[test]
fn durable_apply_uses_authenticated_old_model_basis_after_canvas_acceptance() {
    let mut state = document();
    state.working_mut().splice("main.ts", 14, 2, "14").unwrap();
    let capture = state.capture_apply().unwrap();
    let encoded = state.encode_apply_capture(&capture).unwrap();
    let helper_patch = patch(&state.accepted().files["helper.ts"], "8", "10");
    let prepared = state
        .prepare_canvas_update(&[AcceptedFilePatch {
            path: "helper.ts".into(),
            patch: helper_patch.clone(),
        }])
        .unwrap();
    state
        .publish_validated(
            &prepared,
            "new-canvas".into(),
            &state.working().revision(),
            &[WorkingSourceChange::Reconciled {
                path: "helper.ts".into(),
                patch: helper_patch,
            }],
        )
        .unwrap();
    let restored = SourceDocument::from_json(
        &state.to_json().unwrap(),
        b"restart",
        SharedTextLimits::default(),
    )
    .unwrap();
    let recovered = restored
        .restore_apply_capture(&encoded, capture.accepted_basis())
        .unwrap();
    assert!(restored.apply_needs_rebase(&recovered).unwrap());
    assert_eq!(
        recovered.working().text("helper.ts"),
        Some("export const height = 8;\n")
    );
    assert_eq!(
        restored.accepted().files["helper.ts"],
        "export const height = 10;\n"
    );
    assert!(
        restored
            .restore_apply_capture(&encoded, restored.accepted())
            .is_err()
    );
}
