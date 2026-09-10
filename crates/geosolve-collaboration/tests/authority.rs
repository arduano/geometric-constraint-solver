// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_collaboration::{
    authority::{AuthorityError, Completion, DocumentAuthority},
    protocol::{
        Command, CommandKind, Connection, JournalRecord, Limits, Outcome, Principal, Role, Submit,
    },
};
use serde_json::json;

fn authority(limits: Limits) -> DocumentAuthority {
    DocumentAuthority::new(
        "project".into(),
        "document-epoch".into(),
        "server-one".into(),
        "initial-input".into(),
        limits,
    )
    .unwrap()
}
fn connect(
    state: &mut DocumentAuthority,
    user: &str,
    client: &str,
    session: &str,
    role: Role,
) -> Connection {
    state
        .connect(
            Principal {
                user_id: user.into(),
                role,
            },
            client.into(),
            session.into(),
        )
        .unwrap()
}
fn request(connection: &Connection, id: &str, value: i32) -> Submit {
    Submit {
        connection: connection.clone(),
        request_id: id.into(),
        command: Command {
            kind: CommandKind::Semantic,
            basis_revision: 0,
            payload: json!({ "target": { "declaration": "width", "generation": 1 }, "value": value }),
        },
    }
}
fn admit(state: &mut DocumentAuthority, request: &Submit, log: &mut Vec<JournalRecord>) {
    state
        .admit(request, |record| {
            log.push(record.clone());
            Ok(())
        })
        .unwrap();
}
fn accept(state: &mut DocumentAuthority, input: &str, log: &mut Vec<JournalRecord>) {
    let ticket = state.begin_next().unwrap().unwrap();
    state
        .complete(
            &ticket,
            Completion::Accepted {
                accepted_input: input.into(),
                summary: "Dimension changed".into(),
            },
            |record| {
                log.push(record.clone());
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn server_order_prepares_each_command_against_latest_input() {
    for first in [12, 14] {
        let mut state = authority(Limits::default());
        let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
        let bob = connect(&mut state, "bob", "tab-b", "session-b", Role::Editor);
        let mut log = vec![];
        admit(&mut state, &request(&alice, "one", first), &mut log);
        admit(&mut state, &request(&bob, "one", 26 - first), &mut log);
        let ticket = state.begin_next().unwrap().unwrap();
        assert_eq!(ticket.command().payload["value"], first);
        assert_eq!(ticket.accepted_revision(), 0);
        assert!(state.begin_next().unwrap().is_none());
        state
            .complete(
                &ticket,
                Completion::Accepted {
                    accepted_input: "after-first".into(),
                    summary: String::new(),
                },
                |record| {
                    log.push(record.clone());
                    Ok(())
                },
            )
            .unwrap();
        let second = state.begin_next().unwrap().unwrap();
        assert_eq!(second.command().basis_revision, 0);
        assert_eq!(second.accepted_revision(), 1);
        assert_eq!(second.accepted_input(), "after-first");
        assert_eq!(second.command().payload["value"], 26 - first);
        assert_eq!(
            state.complete(
                &ticket,
                Completion::Rejected {
                    code: "stale".into(),
                    message: String::new()
                },
                |_| Ok(())
            ),
            Err(AuthorityError::StalePrepared)
        );
    }
}

#[test]
fn duplicate_lost_ack_and_restart_preserve_original_outcome() {
    let mut state = authority(Limits::default());
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let original = request(&alice, "one", 12);
    let mut log = vec![];
    admit(&mut state, &original, &mut log);
    let first = state
        .admit(&original, |_| panic!("duplicate must not persist twice"))
        .unwrap();
    assert!(first.outcome.is_none());
    accept(&mut state, "accepted-one", &mut log);
    let final_receipt = state
        .admit(&original, |_| panic!("lost ACK retry must not execute"))
        .unwrap();
    assert_eq!(final_receipt.admission, 1);
    assert!(matches!(
        final_receipt.outcome,
        Some(Outcome::Accepted { revision: 1, .. })
    ));
    let mut restored = DocumentAuthority::restore(
        "project".into(),
        "document-epoch".into(),
        "server-two".into(),
        "initial-input".into(),
        Limits::default(),
        &log,
    )
    .unwrap();
    assert_eq!(
        restored.admit(&original, |_| Ok(())),
        Err(AuthorityError::Connection)
    );
    let rejoined = connect(
        &mut restored,
        "alice",
        "tab-a",
        "session-rejoin",
        Role::Editor,
    );
    let mut retry = request(&rejoined, "one", 12);
    assert_eq!(
        restored.admit(&retry, |_| panic!("persisted duplicate")),
        Ok(final_receipt)
    );
    retry.command.payload["value"] = json!(18);
    assert_eq!(
        restored.admit(&retry, |_| Ok(())),
        Err(AuthorityError::DuplicateMismatch)
    );
    assert_eq!(restored.accepted_revision(), 1);
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn uncertain_durability_poisoning_requires_replay_and_cannot_ack_twice() {
    let mut state = authority(Limits::default());
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let mut log = vec![];
    admit(&mut state, &request(&alice, "one", 12), &mut log);
    let ticket = state.begin_next().unwrap().unwrap();
    let result = state.complete(
        &ticket,
        Completion::Accepted {
            accepted_input: "accepted-one".into(),
            summary: String::new(),
        },
        |record| {
            // Simulate bytes reaching durable storage, then the acknowledgement failing.
            log.push(record.clone());
            Err("fsync outcome unavailable".into())
        },
    );
    assert!(matches!(result, Err(AuthorityError::Durability(_))));
    assert_eq!(state.accepted_revision(), 0);
    assert_eq!(state.accepted_input(), "initial-input");
    assert!(state.needs_recovery());
    assert!(matches!(
        state.admit(&request(&alice, "two", 14), |_| Ok(())),
        Err(AuthorityError::Durability(_))
    ));
    let restored = DocumentAuthority::restore(
        "project".into(),
        "document-epoch".into(),
        "server-two".into(),
        "initial-input".into(),
        Limits::default(),
        &log,
    )
    .unwrap();
    assert_eq!(restored.accepted_revision(), 1);
    assert_eq!(restored.accepted_input(), "accepted-one");
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn failed_admission_retains_visible_state_and_pending_admission_replays() {
    let mut state = authority(Limits::default());
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let mut log = vec![];
    assert!(
        state
            .admit(&request(&alice, "one", 12), |record| {
                log.push(record.clone());
                Err("lost durability reply".into())
            })
            .is_err()
    );
    assert_eq!(state.pending_count(), 0);
    let mut restored = DocumentAuthority::restore(
        "project".into(),
        "document-epoch".into(),
        "server-two".into(),
        "initial-input".into(),
        Limits::default(),
        &log,
    )
    .unwrap();
    assert_eq!(restored.pending_count(), 1);
    let ticket = restored.begin_next().unwrap().unwrap();
    assert_eq!(ticket.operation().request_id, "one");
    assert_eq!(ticket.accepted_input(), "initial-input");
}

#[test]
fn rejected_model_retains_revision_but_releases_following_work() {
    let mut state = authority(Limits::default());
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let mut log = vec![];
    admit(&mut state, &request(&alice, "one", -12), &mut log);
    admit(&mut state, &request(&alice, "two", 14), &mut log);
    let ticket = state.begin_next().unwrap().unwrap();
    let receipt = state
        .complete(
            &ticket,
            Completion::Rejected {
                code: "invalid-geometry".into(),
                message: "Width must remain positive".into(),
            },
            |record| {
                log.push(record.clone());
                Ok(())
            },
        )
        .unwrap();
    assert!(matches!(receipt.outcome, Some(Outcome::Rejected { .. })));
    assert_eq!(state.accepted_revision(), 0);
    assert_eq!(state.accepted_input(), "initial-input");
    accept(&mut state, "valid-second", &mut log);
    assert_eq!(state.accepted_revision(), 1);
}

#[test]
fn forged_sessions_viewers_and_future_bases_never_enter_journal() {
    let mut state = authority(Limits::default());
    let viewer = connect(&mut state, "viewer", "tab-v", "session-v", Role::Viewer);
    assert_eq!(
        state.admit(&request(&viewer, "one", 12), |_| panic!()),
        Err(AuthorityError::ReadOnly)
    );
    let mut forged = viewer.clone();
    forged.role = Role::Editor;
    assert_eq!(
        state.admit(&request(&forged, "one", 12), |_| panic!()),
        Err(AuthorityError::Connection)
    );
    let editor = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let mut future = request(&editor, "one", 12);
    future.command.basis_revision = 1;
    assert!(matches!(
        state.admit(&future, |_| panic!()),
        Err(AuthorityError::Invalid(_))
    ));
    let replacement = connect(&mut state, "alice", "tab-a", "session-new", Role::Editor);
    assert_eq!(
        state.admit(&request(&editor, "one", 12), |_| panic!()),
        Err(AuthorityError::Connection)
    );
    assert!(state.receipt(&replacement, "one").unwrap().is_none());
    assert_eq!(state.latest_sequence(), 0);
}

#[test]
fn pending_limits_allow_duplicate_resolution_and_independent_clients() {
    let limits = Limits {
        max_pending_per_client: 1,
        max_pending: 2,
        ..Limits::default()
    };
    let mut state = authority(limits);
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let bob = connect(&mut state, "bob", "tab-b", "session-b", Role::Editor);
    let mut log = vec![];
    admit(&mut state, &request(&alice, "one", 12), &mut log);
    assert!(matches!(
        state.admit(&request(&alice, "two", 14), |_| panic!()),
        Err(AuthorityError::Backpressure(_))
    ));
    admit(&mut state, &request(&bob, "one", 14), &mut log);
    assert!(
        state
            .admit(&request(&alice, "one", 12), |_| panic!())
            .is_ok()
    );
    assert!(matches!(
        state.admit(&request(&bob, "two", 16), |_| panic!()),
        Err(AuthorityError::Backpressure(_))
    ));
    accept(&mut state, "first-input", &mut log);
    admit(&mut state, &request(&alice, "two", 16), &mut log);
    assert_eq!(state.pending_count(), 2);
}

#[test]
fn replay_detects_reorder_tampering_foreign_identity_and_missing_records() {
    let mut state = authority(Limits::default());
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let mut log = vec![];
    admit(&mut state, &request(&alice, "one", 12), &mut log);
    accept(&mut state, "accepted-input", &mut log);
    let restore = |records: &[JournalRecord]| {
        DocumentAuthority::restore(
            "project".into(),
            "document-epoch".into(),
            "server-two".into(),
            "initial-input".into(),
            Limits::default(),
            records,
        )
    };
    let mut tampered = log.clone();
    tampered[0].document_id = "foreign".into();
    assert!(matches!(
        restore(&tampered),
        Err(AuthorityError::Corrupt(_))
    ));
    let mut reordered = log.clone();
    reordered.reverse();
    assert!(restore(&reordered).is_err());
    assert!(restore(&log[1..]).is_err());
    let mut tampered = log.clone();
    tampered[0].digest = "forged".into();
    assert!(restore(&tampered).is_err());
}

#[test]
fn ticket_from_another_document_cannot_complete_matching_operation() {
    let mut first = authority(Limits::default());
    let mut second = DocumentAuthority::new(
        "other-project".into(),
        "document-epoch".into(),
        "server-one".into(),
        "initial-input".into(),
        Limits::default(),
    )
    .unwrap();
    let a = connect(&mut first, "alice", "tab", "session", Role::Editor);
    let b = connect(&mut second, "alice", "tab", "session", Role::Editor);
    admit(&mut first, &request(&a, "one", 12), &mut vec![]);
    admit(&mut second, &request(&b, "one", 12), &mut vec![]);
    let foreign = first.begin_next().unwrap().unwrap();
    let local = second.begin_next().unwrap().unwrap();
    assert_eq!(
        second.complete(
            &foreign,
            Completion::Accepted {
                accepted_input: "forged-input".into(),
                summary: String::new()
            },
            |_| panic!("foreign ticket cannot persist")
        ),
        Err(AuthorityError::StalePrepared)
    );
    assert_eq!(second.accepted_revision(), 0);
    second
        .complete(
            &local,
            Completion::Rejected {
                code: "valid-local-rejection".into(),
                message: String::new(),
            },
            |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn resumable_event_pages_and_ingress_remain_available_during_a_solve() {
    use geosolve_collaboration::protocol::Resume;
    let mut state = authority(Limits {
        max_resume_records: 1,
        ..Limits::default()
    });
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let bob = connect(&mut state, "bob", "tab-b", "session-b", Role::Editor);
    let viewer = connect(&mut state, "viewer", "tab-v", "session-v", Role::Viewer);
    let mut log = vec![];
    admit(&mut state, &request(&alice, "one", 12), &mut log);
    let held = state.begin_next().unwrap().unwrap();
    admit(&mut state, &request(&bob, "one", 14), &mut log);
    assert_eq!(state.pending_count(), 2);
    let Resume::Events { records } = state.resume(&viewer, 0).unwrap() else {
        panic!("initial page")
    };
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].sequence, 1);
    let Resume::Events { records } = state.resume(&viewer, 1).unwrap() else {
        panic!("next page")
    };
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].sequence, 2);
    assert!(state.resume(&viewer, 3).is_err());
    assert!(
        state
            .receipt(&bob, "one")
            .unwrap()
            .unwrap()
            .outcome
            .is_none()
    );
    state
        .complete(
            &held,
            Completion::Accepted {
                accepted_input: "held-result".into(),
                summary: String::new(),
            },
            |record| {
                log.push(record.clone());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(state.latest_sequence(), 3);
}

#[test]
fn ledger_reserves_durable_completion_space_before_admission() {
    let mut state = authority(Limits {
        max_ledger_bytes: 9500,
        ..Limits::default()
    });
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    admit(&mut state, &request(&alice, "one", 12), &mut vec![]);
    assert!(matches!(
        state.admit(&request(&alice, "two", 14), |_| panic!()),
        Err(AuthorityError::Backpressure(_))
    ));
    let ticket = state.begin_next().unwrap().unwrap();
    state
        .complete(
            &ticket,
            Completion::Rejected {
                code: "bounded-error".into(),
                message: "\u{0001}".repeat(1024),
            },
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(state.pending_count(), 0);
    assert!(state.ledger_bytes() < 9500);
}

#[test]
fn replay_binds_initial_accepted_input_even_before_first_model_commit() {
    let mut state = authority(Limits::default());
    let alice = connect(&mut state, "alice", "tab-a", "session-a", Role::Editor);
    let mut log = vec![];
    admit(&mut state, &request(&alice, "one", 12), &mut log);
    let restored = DocumentAuthority::restore(
        "project".into(),
        "document-epoch".into(),
        "server-two".into(),
        "different-initial-input".into(),
        Limits::default(),
        &log,
    );
    assert!(matches!(restored, Err(AuthorityError::Corrupt(_))));
}
