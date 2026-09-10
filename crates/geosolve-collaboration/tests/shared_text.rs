// SPDX-License-Identifier: GPL-3.0-or-later
use automerge::transaction::Transactable;
use geosolve_collaboration::{
    CursorDeletionBias, SharedTextDocument, SharedTextError, SharedTextLimits, TextEdit,
};

fn seeded(text: &str) -> (SharedTextDocument, SharedTextDocument) {
    let mut first = SharedTextDocument::new(b"alice", SharedTextLimits::default()).unwrap();
    first.create_file("main.ts", text).unwrap();
    let second = first.fork(b"bob").unwrap();
    (first, second)
}
fn edit(document: &mut SharedTextDocument, start: usize, delete: usize, insert: &str) {
    let revision = document.capture().revision().clone();
    document
        .apply_edits(
            &revision,
            &[TextEdit::Splice {
                path: "main.ts".into(),
                start_utf16: start,
                delete_utf16: delete,
                insert: insert.into(),
            }],
        )
        .unwrap();
}
fn sync(a: &mut SharedTextDocument, b: &mut SharedTextDocument) {
    for _ in 0..20 {
        let a_message = a.generate_sync_message("bob").unwrap();
        let b_message = b.generate_sync_message("alice").unwrap();
        if a_message.is_none() && b_message.is_none() {
            assert_eq!(a.capture(), b.capture());
            return;
        }
        if let Some(message) = a_message {
            b.receive_sync_message("alice", &message).unwrap();
        }
        if let Some(message) = b_message {
            a.receive_sync_message("bob", &message).unwrap();
        }
    }
    panic!("bounded sync did not settle");
}

#[test]
fn concurrent_same_file_splices_converge_without_losing_either_actor() {
    let (mut alice, mut bob) = seeded("const width = 12;\nconst height = 8;\n");
    let immutable = alice.capture();
    edit(&mut alice, 14, 2, "16");
    edit(&mut bob, 33, 1, "9");
    sync(&mut alice, &mut bob);
    assert_eq!(
        alice.capture().files()["main.ts"],
        "const width = 16;\nconst height = 9;\n"
    );
    assert_eq!(
        immutable.files()["main.ts"],
        "const width = 12;\nconst height = 8;\n"
    );
    assert_ne!(immutable.revision(), alice.capture().revision());
    let (mut alice, mut bob) = seeded("ab");
    edit(&mut alice, 1, 0, "A");
    edit(&mut bob, 1, 0, "B");
    sync(&mut alice, &mut bob);
    let text = alice.capture().files()["main.ts"].clone();
    assert!(text == "aABb" || text == "aBAb");
}

#[test]
fn utf16_unicode_splices_and_stable_cursors_survive_merge_and_restart() {
    let (mut alice, mut bob) = seeded("a😀βe\u{301}z");
    let cursor = alice
        .cursor("main.ts", 3, CursorDeletionBias::After)
        .unwrap();
    let end = alice
        .cursor("main.ts", 7, CursorDeletionBias::Before)
        .unwrap();
    assert_eq!(alice.resolve_cursor(&cursor).unwrap(), 3);
    edit(&mut alice, 1, 2, "🐟");
    edit(&mut bob, 0, 0, "𐐀");
    sync(&mut alice, &mut bob);
    assert_eq!(alice.capture().files()["main.ts"], "𐐀a🐟βe\u{301}z");
    assert_eq!(alice.resolve_cursor(&cursor).unwrap(), 5);
    assert_eq!(alice.resolve_cursor(&end).unwrap(), 9);
    let mut restored =
        SharedTextDocument::load(&alice.save(), b"carol", SharedTextLimits::default()).unwrap();
    assert_eq!(restored.capture(), alice.capture());
    assert_eq!(restored.resolve_cursor(&cursor).unwrap(), 5);
    let before = restored.capture();
    assert_eq!(
        restored.cursor("main.ts", 1, CursorDeletionBias::After),
        Err(SharedTextError::InvalidPosition)
    );
    assert_eq!(
        restored.apply_edits(
            before.revision(),
            &[TextEdit::Splice {
                path: "main.ts".into(),
                start_utf16: 1,
                delete_utf16: 0,
                insert: "x".into()
            }]
        ),
        Err(SharedTextError::InvalidPosition)
    );
    assert_eq!(restored.capture(), before);
}

#[test]
fn invalid_source_and_multifile_edits_are_raw_text_with_immutable_apply_capture() {
    let (mut alice, mut bob) = seeded("export const = (\n");
    let revision = alice.capture().revision().clone();
    alice
        .set_file(&revision, "patches/channel.ts", "/* 😀 unterminated")
        .unwrap();
    edit(&mut bob, 0, 0, "<<<<<<< pending\n");
    sync(&mut alice, &mut bob);
    let captured = alice.capture();
    assert_eq!(captured.files().len(), 2);
    assert!(captured.files()["main.ts"].contains("<<<<<<<"));
    edit(&mut alice, 0, 0, "later\n");
    assert!(!captured.files()["main.ts"].starts_with("later"));
    assert_eq!(
        alice.set_file(captured.revision(), "main.ts", "stale"),
        Err(SharedTextError::StaleRevision)
    );
}

#[test]
fn malformed_tail_and_resource_failure_retain_every_prior_byte_and_revision() {
    let (mut alice, mut bob) = seeded("a😀b");
    let before = alice.capture();
    let edits = [
        TextEdit::Splice {
            path: "main.ts".into(),
            start_utf16: 0,
            delete_utf16: 0,
            insert: "first".into(),
        },
        TextEdit::Splice {
            path: "main.ts".into(),
            start_utf16: 7,
            delete_utf16: 0,
            insert: "invalid surrogate split".into(),
        },
    ];
    assert_eq!(
        alice.apply_edits(before.revision(), &edits),
        Err(SharedTextError::InvalidPosition)
    );
    assert_eq!(alice.capture(), before);
    assert!(alice.receive_sync_message("bob", b"not automerge").is_err());
    let mut message = bob.generate_sync_message("alice").unwrap().unwrap();
    message.extend([0, 0, 0]);
    assert!(alice.receive_sync_message("bob", &message).is_err());
    assert_eq!(alice.capture(), before);
    let limits = SharedTextLimits {
        max_file_bytes: 8,
        max_total_bytes: 16,
        max_operations: 32,
        ..SharedTextLimits::default()
    };
    let mut small = SharedTextDocument::new(b"alice", limits).unwrap();
    small.create_file("main.ts", "1234").unwrap();
    let snapshot = small.capture();
    assert!(
        small
            .set_file(snapshot.revision(), "main.ts", "123456789")
            .is_err()
    );
    assert_eq!(small.capture(), snapshot);
    assert!(
        small
            .set_file(snapshot.revision(), "../escape.ts", "x")
            .is_err()
    );
    let mut saved = small.save();
    saved.truncate(saved.len() - 2);
    assert!(SharedTextDocument::load(&saved, b"bob", limits).is_err());
}

#[test]
fn restart_reconnect_and_duplicate_messages_converge_idempotently() {
    let (mut alice, mut bob) = seeded("abcdef");
    let message = alice.generate_sync_message("bob").unwrap().unwrap();
    bob.receive_sync_message("alice", &message).unwrap();
    let once = bob.capture();
    bob.receive_sync_message("alice", &message).unwrap();
    assert_eq!(bob.capture(), once);
    alice.forget_peer("bob");
    bob.forget_peer("alice");
    edit(&mut alice, 2, 2, "X");
    let mut restarted =
        SharedTextDocument::load(&bob.save(), b"bob", SharedTextLimits::default()).unwrap();
    edit(&mut restarted, 6, 0, "Y");
    sync(&mut alice, &mut restarted);
    assert_eq!(alice.capture().files()["main.ts"], "abXefY");
}

#[test]
fn deleted_and_recreated_file_does_not_retarget_old_cursor() {
    let (mut alice, _) = seeded("abc");
    let cursor = alice
        .cursor("main.ts", 1, CursorDeletionBias::After)
        .unwrap();
    let removed = alice.remove_file("main.ts").unwrap();
    alice.set_file(&removed, "main.ts", "abc").unwrap();
    assert_eq!(
        alice.resolve_cursor(&cursor),
        Err(SharedTextError::InvalidCursor)
    );
}

#[test]
fn file_rename_preserves_identity_anchors_and_concurrent_edit() {
    let (mut alice, mut bob) = seeded("const width = 12;");
    let identity = alice.file_id("main.ts").unwrap();
    let cursor = alice
        .cursor("main.ts", 14, CursorDeletionBias::After)
        .unwrap();
    alice.rename_file("main.ts", "src/design.ts").unwrap();
    edit(&mut bob, 14, 2, "16");
    sync(&mut alice, &mut bob);
    assert_eq!(alice.file_id("src/design.ts").unwrap(), identity);
    assert_eq!(
        alice.capture().text("src/design.ts"),
        Some("const width = 16;")
    );
    assert_eq!(
        alice.resolve_cursor_location(&cursor).unwrap().path,
        "src/design.ts"
    );
    assert!(alice.capture().text("main.ts").is_none());
    let restored =
        SharedTextDocument::load(&alice.save(), b"carol", SharedTextLimits::default()).unwrap();
    assert_eq!(restored.file_id("src/design.ts").unwrap(), identity);
}

#[test]
fn source_range_tracks_unrelated_invalid_draft_and_refuses_overlap() {
    let (mut alice, mut bob) = seeded("const width = 12;\nconst height = 8;\n");
    let anchor = alice
        .anchor_range(&alice.revision(), "main.ts", 14, 16)
        .unwrap();
    edit(&mut bob, 0, 0, "import { incomplete\n");
    alice.merge(&bob).unwrap();
    alice.replace_range(&anchor, "16").unwrap();
    assert_eq!(
        alice.capture().text("main.ts"),
        Some("import { incomplete\nconst width = 16;\nconst height = 8;\n")
    );
    let before = alice.capture();
    assert!(alice.replace_range(&anchor, "20").is_err());
    assert_eq!(alice.capture(), before);
}

#[test]
fn checked_undo_preserves_remote_contributions_and_rejects_ownership_loss() {
    let (mut alice, mut bob) = seeded("width=12; height=8");
    let (_, undo) = alice
        .edit_undoable(
            &alice.revision(),
            &[TextEdit::Splice {
                path: "main.ts".into(),
                start_utf16: 6,
                delete_utf16: 2,
                insert: "16".into(),
            }],
        )
        .unwrap();
    edit(&mut bob, 17, 1, "9");
    alice.merge(&bob).unwrap();
    let redo = alice.apply_inverse(&undo).unwrap();
    assert_eq!(alice.capture().text("main.ts"), Some("width=12; height=9"));
    let undo = alice.apply_inverse(&redo).unwrap();
    assert_eq!(alice.capture().text("main.ts"), Some("width=16; height=9"));
    let mut bob = alice.fork(b"bob").unwrap();
    edit(&mut bob, 7, 1, "7");
    alice.merge(&bob).unwrap();
    let before = alice.capture();
    assert!(alice.apply_inverse(&undo).is_err());
    assert_eq!(alice.capture(), before);
    assert!(bob.apply_inverse(&undo).is_err());
}

#[test]
fn novel_changes_require_authenticated_actor_and_dependencies() {
    let (mut alice, mut bob) = seeded("abc");
    let basis = alice.revision();
    edit(&mut bob, 1, 0, "B");
    let first = bob.changes_since(&basis).unwrap();
    edit(&mut bob, 0, 0, "X");
    let all = bob.changes_since(&basis).unwrap();
    let before = alice.capture();
    assert_eq!(
        alice.apply_changes_from(&all, b"forged"),
        Err(SharedTextError::ActorMismatch)
    );
    assert_eq!(alice.capture(), before);
    assert_eq!(
        alice.apply_changes(&all[1..]),
        Err(SharedTextError::MissingDependencies)
    );
    assert_eq!(alice.capture(), before);
    alice.apply_changes_from(&first, b"bob").unwrap();
    alice.apply_changes_from(&all, b"bob").unwrap();
    assert_eq!(alice.capture(), bob.capture());
    alice.apply_changes_from(&all, b"other-relay").unwrap();
    assert_eq!(alice.capture(), bob.capture());
}

#[test]
fn lifecycle_conflict_operation_peer_and_schema_limits_fail_atomically() {
    let (mut alice, mut bob) = seeded("abc");
    alice.create_file("new.ts", "alice").unwrap();
    bob.create_file("new.ts", "bob").unwrap();
    let before = alice.capture();
    assert!(matches!(
        alice.merge(&bob),
        Err(SharedTextError::FileConflict(_))
    ));
    assert_eq!(alice.capture(), before);
    let limits = SharedTextLimits {
        max_operations: 10,
        max_peers: 1,
        ..SharedTextLimits::default()
    };
    let mut small = SharedTextDocument::new(b"a", limits).unwrap();
    let before = small.capture();
    assert_eq!(
        small.create_file("main.ts", "0123456789"),
        Err(SharedTextError::ResourceLimit("operation count"))
    );
    assert_eq!(small.capture(), before);
    small.generate_sync_message("one").unwrap();
    assert_eq!(
        small.generate_sync_message("two"),
        Err(SharedTextError::ResourceLimit("peer count"))
    );
    small.forget_peer("one");
    assert!(small.generate_sync_message("two").is_ok());
    let mut raw = automerge::AutoCommit::new();
    raw.put(automerge::ROOT, "intruder", "not source").unwrap();
    assert!(SharedTextDocument::load(&raw.save(), b"a", SharedTextLimits::default()).is_err());
}

#[test]
fn batch_undo_survives_adjacent_local_and_remote_replacements() {
    let (mut alice, mut bob) = seeded("abcd");
    let (_, token) = alice
        .edit_undoable(
            &alice.revision(),
            &[
                TextEdit::Splice {
                    path: "main.ts".into(),
                    start_utf16: 0,
                    delete_utf16: 1,
                    insert: "A".into(),
                },
                TextEdit::Splice {
                    path: "main.ts".into(),
                    start_utf16: 1,
                    delete_utf16: 1,
                    insert: "B".into(),
                },
            ],
        )
        .unwrap();
    edit(&mut bob, 2, 1, "C");
    alice.merge(&bob).unwrap();
    alice.apply_inverse(&token).unwrap();
    assert_eq!(alice.capture().text("main.ts"), Some("abCd"));
}

#[test]
fn overlapping_batch_undo_and_identical_remote_character_replacement() {
    let (mut alice, _) = seeded("abc");
    let (_, token) = alice
        .edit_undoable(
            &alice.revision(),
            &[
                TextEdit::Splice {
                    path: "main.ts".into(),
                    start_utf16: 0,
                    delete_utf16: 1,
                    insert: "A".into(),
                },
                TextEdit::Splice {
                    path: "main.ts".into(),
                    start_utf16: 0,
                    delete_utf16: 2,
                    insert: "BC".into(),
                },
            ],
        )
        .unwrap();
    let redo = alice.apply_inverse(&token).unwrap();
    assert_eq!(alice.capture().text("main.ts"), Some("abc"));
    let undo = alice.apply_inverse(&redo).unwrap();
    let mut bob = alice.fork(b"bob").unwrap();
    edit(&mut bob, 1, 1, "");
    edit(&mut bob, 1, 0, "C");
    alice.merge(&bob).unwrap();
    let before = alice.capture();
    assert_eq!(before.text("main.ts"), Some("BCc"));
    assert_eq!(
        alice.apply_inverse(&undo).unwrap_err(),
        SharedTextError::RangeConflict
    );
    assert_eq!(alice.capture(), before);
}

#[test]
fn authenticated_typing_cannot_smuggle_unordered_file_lifecycle() {
    let (mut alice, mut bob) = seeded("abc");
    let basis = alice.revision();
    bob.rename_file("main.ts", "renamed.ts").unwrap();
    let changes = bob.changes_since(&basis).unwrap();
    let before = alice.capture();
    assert_eq!(
        alice.apply_changes_from(&changes, b"bob"),
        Err(SharedTextError::FileLifecycleRequiresOrder)
    );
    assert_eq!(alice.capture(), before);
    alice.apply_changes(&changes).unwrap();
    assert_eq!(alice.capture(), bob.capture());
}

#[test]
fn stale_typing_into_deleted_file_is_refused_instead_of_acknowledged_invisibly() {
    let (mut server, mut client) = seeded("abc");
    let basis = server.revision();
    server.remove_file("main.ts").unwrap();
    edit(&mut client, 1, 0, "pending");
    let changes = client.changes_since(&basis).unwrap();
    let before = server.capture();
    assert!(server.apply_changes_from(&changes, b"bob").is_err());
    assert_eq!(server.capture(), before);
}

#[test]
fn explicit_unicode_compiler_spans_and_cursor_deletion_bias() {
    use geosolve_collaboration::{utf8_span_to_utf16, utf8_to_utf16, utf16_to_utf8};
    let text = "a😀βe\u{301}";
    assert_eq!(utf16_to_utf8(text, 3).unwrap(), 5);
    assert_eq!(utf8_to_utf16(text, 7).unwrap(), 4);
    assert_eq!(utf8_span_to_utf16(text, 1, 7).unwrap(), (1, 4));
    assert!(utf16_to_utf8(text, 2).is_err());
    assert!(utf8_to_utf16(text, 3).is_err());
    assert!(utf8_span_to_utf16(text, 7, 1).is_err());
    let (mut document, _) = seeded("abcde");
    let before = document
        .cursor("main.ts", 2, CursorDeletionBias::Before)
        .unwrap();
    let after = document
        .cursor("main.ts", 2, CursorDeletionBias::After)
        .unwrap();
    edit(&mut document, 1, 3, "");
    assert_eq!(document.resolve_cursor(&before).unwrap(), 0);
    assert_eq!(document.resolve_cursor(&after).unwrap(), 1);
}

#[test]
fn historical_captures_authenticate_exact_heads_source_and_file_identity() {
    let (mut alice, mut bob) = seeded("const width = 12;");
    let original = alice.capture();
    let original_id = alice.file_id("main.ts").unwrap();
    alice.rename_file("main.ts", "renamed.ts").unwrap();
    let renamed = alice.capture();
    edit(&mut bob, 14, 2, "16");
    alice.merge(&bob).unwrap();
    alice.remove_file("renamed.ts").unwrap();
    alice
        .create_file("renamed.ts", "later invalid = (")
        .unwrap();
    let latest = alice.capture();
    assert_eq!(alice.snapshot_at(original.revision()).unwrap(), original);
    assert_eq!(
        alice.file_ids_at(original.revision()).unwrap()["main.ts"],
        original_id
    );
    assert_eq!(alice.snapshot_at(renamed.revision()).unwrap(), renamed);
    assert_eq!(
        alice.file_ids_at(renamed.revision()).unwrap()["renamed.ts"],
        original_id
    );
    assert_ne!(
        alice.file_ids_at(latest.revision()).unwrap()["renamed.ts"],
        original_id
    );
    let foreign = SharedTextDocument::new(b"other-genesis", SharedTextLimits::default()).unwrap();
    assert!(alice.snapshot_at(&foreign.revision()).is_err());
    assert!(alice.file_ids_at(&foreign.revision()).is_err());
    assert_eq!(alice.capture(), latest);
    let restored =
        SharedTextDocument::load(&alice.save(), b"restored", SharedTextLimits::default()).unwrap();
    assert_eq!(restored.snapshot_at(original.revision()).unwrap(), original);
}
