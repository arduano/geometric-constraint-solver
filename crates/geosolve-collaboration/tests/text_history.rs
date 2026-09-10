// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration::{
    SharedTextDocument, SharedTextLimits, TextEdit, document::SourceDocument, protocol::OperationId,
};
use std::collections::BTreeMap;
fn op(user: &str, id: &str) -> OperationId {
    OperationId {
        user_id: user.into(),
        client_id: format!("tab-{user}"),
        request_id: id.into(),
    }
}
fn source(text: &str) -> SourceDocument {
    SourceDocument::new(
        "doc".into(),
        "input0".into(),
        BTreeMap::from([("main.ts".into(), text.into())]),
        b"server",
        SharedTextLimits::default(),
    )
    .unwrap()
}
fn type_text(
    source: &mut SourceDocument,
    user: &str,
    id: &str,
    start: usize,
    delete: usize,
    insert: &str,
) {
    let mut client = source.working().fork(user.as_bytes()).unwrap();
    let basis = client.revision();
    client.splice("main.ts", start, delete, insert).unwrap();
    source
        .apply_user_text_changes(
            &client.changes_since(&basis).unwrap(),
            user.as_bytes(),
            op(user, id),
        )
        .unwrap();
}
fn restart(source: &mut SourceDocument) {
    let saved = source.to_json().unwrap();
    *source = SourceDocument::from_json(&saved, b"server", SharedTextLimits::default()).unwrap();
    assert_eq!(source.to_json().unwrap(), saved);
}
fn text(source: &SourceDocument) -> String {
    source.working().capture().text("main.ts").unwrap().into()
}
fn undo(source: &mut SourceDocument, user: &str, id: &str) {
    source.apply_user_text_inverse(op(user, id), false).unwrap();
}
fn redo(source: &mut SourceDocument, user: &str, id: &str) {
    source.apply_user_text_inverse(op(user, id), true).unwrap();
}
#[test]
fn durable_repeated_personal_text_undo_restores_character_lineage_and_preserves_bob() {
    let mut source = source("width=12;height=8;");
    type_text(&mut source, "alice", "first", 6, 2, "14");
    type_text(&mut source, "alice", "second", 6, 2, "16");
    type_text(&mut source, "bob", "height", 16, 1, "9");
    restart(&mut source);
    undo(&mut source, "alice", "undo-second");
    assert_eq!(text(&source), "width=14;height=9;");
    restart(&mut source);
    undo(&mut source, "alice", "undo-first");
    assert_eq!(text(&source), "width=12;height=9;");
    restart(&mut source);
    redo(&mut source, "alice", "redo-first");
    assert_eq!(text(&source), "width=14;height=9;");
    restart(&mut source);
    redo(&mut source, "alice", "redo-second");
    assert_eq!(text(&source), "width=16;height=9;");
    assert_eq!(source.accepted().files["main.ts"], "width=12;height=8;");
}
#[test]
fn same_value_foreign_text_owns_characters_until_its_personal_undo() {
    let mut source = source("width=12;");
    type_text(&mut source, "alice", "first", 6, 2, "14");
    type_text(&mut source, "bob", "same", 6, 2, "14");
    let saved = source.to_json().unwrap();
    assert!(
        source
            .apply_user_text_inverse(op("alice", "blocked"), false)
            .is_err()
    );
    assert_eq!(source.to_json().unwrap(), saved);
    restart(&mut source);
    undo(&mut source, "bob", "undo-same");
    restart(&mut source);
    undo(&mut source, "alice", "undo-first");
    assert_eq!(text(&source), "width=12;");
}
#[test]
fn unicode_batch_and_overlap_reject_atomically_while_disjoint_edits_survive() {
    let mut source = source("a😀bc🐟z");
    let mut client = source.working().fork(b"alice").unwrap();
    let basis = client.revision();
    client
        .edit(&[
            TextEdit::Splice {
                path: "main.ts".into(),
                start_utf16: 5,
                delete_utf16: 2,
                insert: "鱼".into(),
            },
            TextEdit::Splice {
                path: "main.ts".into(),
                start_utf16: 1,
                delete_utf16: 2,
                insert: "🐠".into(),
            },
        ])
        .unwrap();
    source
        .apply_user_text_changes(
            &client.changes_since(&basis).unwrap(),
            b"alice",
            op("alice", "ime"),
        )
        .unwrap();
    type_text(&mut source, "bob", "overlap", 5, 1, "魚");
    let saved = source.to_json().unwrap();
    assert!(
        source
            .apply_user_text_inverse(op("alice", "fail"), false)
            .is_err()
    );
    assert_eq!(source.to_json().unwrap(), saved);
    undo(&mut source, "bob", "undo-overlap");
    restart(&mut source);
    undo(&mut source, "alice", "undo-ime");
    assert_eq!(text(&source), "a😀bc🐟z");
}
#[test]
fn file_rename_preserves_other_text_and_mixed_delete_restore_uses_fresh_identity() {
    let mut source = source("abc");
    let original = source.working().file_id("main.ts").unwrap();
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::RenameFile {
                path: "main.ts".into(),
                new_path: "renamed.ts".into(),
            }],
            op("alice", "rename"),
        )
        .unwrap();
    assert_eq!(source.working().file_id("renamed.ts").unwrap(), original);
    let mut bob = source.working().fork(b"bob").unwrap();
    let basis = bob.revision();
    bob.splice("renamed.ts", 1, 1, "B").unwrap();
    source
        .apply_user_text_changes(
            &bob.changes_since(&basis).unwrap(),
            b"bob",
            op("bob", "type"),
        )
        .unwrap();
    restart(&mut source);
    undo(&mut source, "alice", "undo-rename");
    assert_eq!(text(&source), "aBc");
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::RemoveFile {
                path: "main.ts".into(),
            }],
            op("alice", "delete"),
        )
        .unwrap();
    let mut stale = bob.clone();
    stale.rename_file("renamed.ts", "main.ts").unwrap(); // only checkpoint identity matters below.
    restart(&mut source);
    undo(&mut source, "alice", "restore");
    let fresh = source.working().file_id("main.ts").unwrap();
    assert_ne!(fresh, original);
    restart(&mut source);
    undo(&mut source, "bob", "undo-type");
    assert_eq!(text(&source), "abc");
    restart(&mut source);
    redo(&mut source, "bob", "redo-type");
    assert_eq!(text(&source), "aBc");
    redo(&mut source, "alice", "redo-delete");
    assert!(source.working().capture().files().is_empty());
}
#[test]
fn creation_undo_blocks_later_text_and_deleted_name_reuse_refuses_restore() {
    let mut source = source("abc");
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::CreateFile {
                path: "new.ts".into(),
                text: "x".into(),
            }],
            op("alice", "create"),
        )
        .unwrap();
    let mut bob = source.working().fork(b"bob").unwrap();
    let basis = bob.revision();
    bob.splice("new.ts", 0, 1, "x").unwrap();
    source
        .apply_user_text_changes(
            &bob.changes_since(&basis).unwrap(),
            b"bob",
            op("bob", "same"),
        )
        .unwrap();
    assert!(
        source
            .apply_user_text_inverse(op("alice", "blocked"), false)
            .is_err()
    );
    undo(&mut source, "bob", "undo");
    undo(&mut source, "alice", "undo-create");
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::CreateFile {
                path: "new.ts".into(),
                text: "x".into(),
            }],
            op("bob", "recreate"),
        )
        .unwrap();
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::RemoveFile {
                path: "new.ts".into(),
            }],
            op("bob", "delete"),
        )
        .unwrap();
    assert!(
        source
            .apply_user_text_inverse(op("alice", "redo-blocked"), true)
            .is_err()
    );
}
#[test]
fn corrupt_history_event_provenance_and_inverse_heads_are_rejected_on_restore() {
    let mut source = source("abc");
    type_text(&mut source, "alice", "type", 1, 1, "B");
    undo(&mut source, "alice", "undo");
    let saved = source.to_json().unwrap();
    for kind in ["actor", "inverse", "order"] {
        let mut wire: serde_json::Value = serde_json::from_str(&saved).unwrap();
        match kind {
            "actor" => wire["textHistory"][0]["actor"] = serde_json::json!([9]),
            "inverse" => wire["textHistory"][1]["after"] = wire["textHistory"][0]["after"].clone(),
            _ => wire["textHistory"].as_array_mut().unwrap().reverse(),
        }
        assert!(
            SourceDocument::from_json(&wire.to_string(), b"server", SharedTextLimits::default())
                .is_err(),
            "{kind}"
        );
    }
    let checkpoint = source.working().save();
    let client =
        SharedTextDocument::load(&checkpoint, b"client", SharedTextLimits::default()).unwrap();
    assert_eq!(client.capture(), source.working().capture());
}

#[test]
fn explicit_same_path_rename_owns_the_property_until_its_undo() {
    let mut source = source("abc");
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::RenameFile {
                path: "main.ts".into(),
                new_path: "other.ts".into(),
            }],
            op("alice", "rename"),
        )
        .unwrap();
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::RenameFile {
                path: "other.ts".into(),
                new_path: "other.ts".into(),
            }],
            op("bob", "same-path"),
        )
        .unwrap();
    restart(&mut source);
    assert!(!source.user_text_history("alice").can_undo);
    assert!(
        source
            .apply_user_text_inverse(op("alice", "blocked"), false)
            .is_err()
    );
    undo(&mut source, "bob", "undo-same-path");
    restart(&mut source);
    undo(&mut source, "alice", "undo-rename");
    assert_eq!(text(&source), "abc");
}

#[test]
fn canvas_writeback_is_a_character_ownership_barrier_and_deleted_file_typing_stays_stale() {
    use geosolve_collaboration::{
        document::{AcceptedFilePatch, WorkingSourceChange},
        source_patch::{SourceEdit, SourcePatch, source_digest},
    };
    let mut source = source("width=12;");
    type_text(&mut source, "alice", "type", 6, 2, "14");
    let patch = |before: &str, after: &str, expected: &str, replacement: &str| SourcePatch {
        base_source_digest: source_digest(before),
        candidate_source_digest: source_digest(after),
        edits: vec![SourceEdit {
            start: 6,
            end: 8,
            expected: expected.into(),
            replacement: replacement.into(),
        }],
    };
    let prepared = source
        .prepare_canvas_update(&[AcceptedFilePatch {
            path: "main.ts".into(),
            patch: patch("width=12;", "width=16;", "12", "16"),
        }])
        .unwrap();
    source
        .publish_validated(
            &prepared,
            "model16".into(),
            &source.working().revision(),
            &[WorkingSourceChange::Reconciled {
                path: "main.ts".into(),
                patch: patch("width=14;", "width=16;", "14", "16"),
            }],
        )
        .unwrap();
    restart(&mut source);
    assert!(
        source
            .apply_user_text_inverse(op("alice", "blocked"), false)
            .is_err()
    );
    assert_eq!(text(&source), "width=16;");
    let mut stale = source.working().fork(b"bob").unwrap();
    let basis = stale.revision();
    stale.splice("main.ts", 6, 2, "18").unwrap();
    let bytes = stale.changes_since(&basis).unwrap();
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::RemoveFile {
                path: "main.ts".into(),
            }],
            op("carol", "delete"),
        )
        .unwrap();
    undo(&mut source, "carol", "restore");
    let before = source.to_json().unwrap();
    assert!(
        source
            .apply_user_text_changes(&bytes, b"bob", op("bob", "stale"))
            .is_err()
    );
    assert_eq!(source.to_json().unwrap(), before);
}

#[test]
fn atomic_file_batch_inverse_handles_path_swaps_and_delete_replacement() {
    let mut source = source("abc");
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::CreateFile {
                path: "second.ts".into(),
                text: "xyz".into(),
            }],
            op("bob", "second"),
        )
        .unwrap();
    let original = source.working().capture();
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[
                TextEdit::RenameFile {
                    path: "main.ts".into(),
                    new_path: "temporary.ts".into(),
                },
                TextEdit::RenameFile {
                    path: "second.ts".into(),
                    new_path: "main.ts".into(),
                },
                TextEdit::RenameFile {
                    path: "temporary.ts".into(),
                    new_path: "second.ts".into(),
                },
            ],
            op("alice", "swap"),
        )
        .unwrap();
    restart(&mut source);
    undo(&mut source, "alice", "undo-swap");
    assert_eq!(source.working().capture().files(), original.files());
    restart(&mut source);
    redo(&mut source, "alice", "redo-swap");
    assert_eq!(text(&source), "xyz");
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[
                TextEdit::RemoveFile {
                    path: "main.ts".into(),
                },
                TextEdit::CreateFile {
                    path: "main.ts".into(),
                    text: "replacement".into(),
                },
            ],
            op("alice", "replace"),
        )
        .unwrap();
    restart(&mut source);
    undo(&mut source, "alice", "undo-replace");
    assert_eq!(text(&source), "xyz");
    restart(&mut source);
    redo(&mut source, "alice", "redo-replace");
    assert_eq!(text(&source), "replacement");
}

#[test]
fn creation_undo_preserves_a_later_same_path_file_contribution() {
    let mut source = source("abc");
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::CreateFile {
                path: "new.ts".into(),
                text: "x".into(),
            }],
            op("alice", "create"),
        )
        .unwrap();
    source
        .apply_user_file_edits(
            &source.working().revision(),
            &[TextEdit::RenameFile {
                path: "new.ts".into(),
                new_path: "new.ts".into(),
            }],
            op("bob", "same-path"),
        )
        .unwrap();
    assert!(
        source
            .apply_user_text_inverse(op("alice", "blocked"), false)
            .is_err()
    );
    undo(&mut source, "bob", "undo-same");
    restart(&mut source);
    redo(&mut source, "bob", "redo-same");
    assert!(
        source
            .apply_user_text_inverse(op("alice", "blocked-again"), false)
            .is_err()
    );
    undo(&mut source, "bob", "undo-again");
    undo(&mut source, "alice", "undo-create");
}

#[test]
fn repeated_foreign_same_value_undo_redo_keeps_earlier_character_lineage_available() {
    let mut source = source("abc");
    type_text(&mut source, "alice", "first", 1, 1, "B");
    type_text(&mut source, "bob", "same", 1, 1, "B");
    for index in 0..3 {
        undo(&mut source, "bob", &format!("undo-{index}"));
        restart(&mut source);
        assert!(source.user_text_history("alice").can_undo);
        redo(&mut source, "bob", &format!("redo-{index}"));
        restart(&mut source);
        assert!(!source.user_text_history("alice").can_undo);
    }
    undo(&mut source, "bob", "last-undo");
    undo(&mut source, "alice", "undo-first");
    assert_eq!(text(&source), "abc");
}

#[test]
fn ordinary_typing_advances_a_bounded_durable_undo_horizon_instead_of_stopping() {
    let mut source = source("a0b0");
    for index in 0..530 {
        type_text(
            &mut source,
            "alice",
            &format!("key-{index}"),
            1,
            1,
            if index % 2 == 0 { "1" } else { "0" },
        );
    }
    let availability = source.user_text_history("alice");
    assert!(availability.horizon.generation > 0);
    assert!(availability.horizon.discarded_events >= 256);
    assert!(availability.undo_count > 200 && availability.undo_count <= 512);
    assert!(availability.can_undo);
    restart(&mut source);
    assert_eq!(
        source.user_text_history("alice").horizon,
        availability.horizon
    );
    type_text(&mut source, "bob", "same", 1, 1, "0");
    assert!(!source.user_text_history("alice").can_undo);
    undo(&mut source, "bob", "undo-same");
    undo(&mut source, "alice", "undo-latest");
    assert_eq!(text(&source), "a1b0");
    restart(&mut source);
    redo(&mut source, "alice", "redo-latest");
    assert_eq!(text(&source), "a0b0");
}

#[test]
fn oversized_undo_contribution_accepts_raw_typing_and_explicitly_closes_older_history() {
    let mut source = source("abc");
    type_text(&mut source, "alice", "first", 1, 1, "B");
    type_text(&mut source, "bob", "large", 0, 3, &"x".repeat(65_537));
    assert_eq!(text(&source).len(), 65_537);
    let availability = source.user_text_history("alice");
    assert!(!availability.can_undo);
    assert!(availability.horizon.generation > 0);
    restart(&mut source);
    assert_eq!(
        source.user_text_history("alice").horizon,
        availability.horizon
    );
    type_text(&mut source, "alice", "continued", 0, 1, "z");
    undo(&mut source, "alice", "undo-continued");
    assert!(text(&source).starts_with('x'));
    let mut wire: serde_json::Value = serde_json::from_str(&source.to_json().unwrap()).unwrap();
    wire["textHistoryHorizon"]["oldestRevision"] = serde_json::json!({"heads":["00".repeat(32)]});
    assert!(
        SourceDocument::from_json(&wire.to_string(), b"server", SharedTextLimits::default())
            .is_err()
    );
}

#[test]
fn mixed_host_working_batch_is_atomic_and_one_durable_personal_contribution() {
    let mut source = source("a😀b0");
    let accepted = source.accepted().clone();
    let original_id = source.working().file_id("main.ts").unwrap();
    let before = source.to_json().unwrap();
    let expected = source.working().revision();
    let edits = vec![
        TextEdit::RenameFile {
            path: "main.ts".into(),
            new_path: "renamed.ts".into(),
        },
        TextEdit::Splice {
            path: "renamed.ts".into(),
            start_utf16: 1,
            delete_utf16: 2,
            insert: "🦀(".into(),
        },
        TextEdit::CreateFile {
            path: "broken.ts".into(),
            text: "export const = (".into(),
        },
    ];
    assert!(
        source
            .apply_user_file_edits(&expected, &edits, op("alice", "strict"))
            .is_err()
    );
    let mut invalid = edits.clone();
    invalid.push(TextEdit::RemoveFile {
        path: "absent.ts".into(),
    });
    assert!(
        source
            .apply_user_working_edits(&expected, &invalid, op("alice", "invalid"))
            .is_err()
    );
    assert_eq!(source.to_json().unwrap(), before);
    source
        .apply_user_working_edits(&expected, &edits, op("alice", "mixed"))
        .unwrap();
    assert_eq!(source.working().file_id("renamed.ts").unwrap(), original_id);
    assert_eq!(
        source.working().capture().text("renamed.ts"),
        Some("a🦀(b0")
    );
    assert_eq!(source.user_text_history("alice").undo_count, 1);
    assert!(
        source
            .apply_user_working_edits(&expected, &edits, op("alice", "stale"))
            .is_err()
    );
    restart(&mut source);
    undo(&mut source, "alice", "undo-mixed");
    assert_eq!(source.working().capture().text("main.ts"), Some("a😀b0"));
    assert_eq!(source.working().capture().text("broken.ts"), None);
    restart(&mut source);
    source
        .apply_user_text_inverse(op("alice", "redo-mixed"), true)
        .unwrap();
    assert_eq!(
        source.working().capture().text("renamed.ts"),
        Some("a🦀(b0")
    );
    assert_eq!(source.accepted(), &accepted);
}

#[test]
fn mixed_host_batch_tracks_same_path_ownership_through_later_lifecycle() {
    let mut source = source("a0");
    let edits = vec![
        TextEdit::RenameFile {
            path: "main.ts".into(),
            new_path: "main.ts".into(),
        },
        TextEdit::RenameFile {
            path: "main.ts".into(),
            new_path: "moved.ts".into(),
        },
        TextEdit::Splice {
            path: "moved.ts".into(),
            start_utf16: 1,
            delete_utf16: 1,
            insert: "1".into(),
        },
        TextEdit::CreateFile {
            path: "temporary.ts".into(),
            text: "x".into(),
        },
        TextEdit::RenameFile {
            path: "temporary.ts".into(),
            new_path: "temporary.ts".into(),
        },
        TextEdit::RemoveFile {
            path: "temporary.ts".into(),
        },
        TextEdit::CreateFile {
            path: "new.ts".into(),
            text: "y".into(),
        },
        TextEdit::RenameFile {
            path: "new.ts".into(),
            new_path: "new.ts".into(),
        },
    ];
    source
        .apply_user_working_edits(
            &source.working().revision(),
            &edits,
            op("alice", "mixed-paths"),
        )
        .unwrap();
    restart(&mut source);
    undo(&mut source, "alice", "undo-paths");
    assert_eq!(source.working().capture().text("main.ts"), Some("a0"));
    assert_eq!(source.working().capture().text("new.ts"), None);
}
