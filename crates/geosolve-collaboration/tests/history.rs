// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration::{
    history::{ContributionHistory, HistoryError, PropertyAddress, PropertyChange},
    protocol::OperationId,
    targets::{TargetLedger, TargetLimits},
};
use serde_json::json;
fn operation(user: &str, id: &str) -> OperationId {
    OperationId {
        user_id: user.into(),
        client_id: format!("tab-{user}"),
        request_id: id.into(),
    }
}
fn setup() -> (TargetLedger, PropertyAddress, PropertyAddress) {
    let mut targets = TargetLedger::new(TargetLimits::default()).unwrap();
    let width = PropertyAddress {
        target: targets.create("main/width").unwrap(),
        property: "value".into(),
    };
    let height = PropertyAddress {
        target: targets.create("main/height").unwrap(),
        property: "value".into(),
    };
    (targets, width, height)
}
fn change(address: &PropertyAddress, before: i32, after: i32) -> PropertyChange {
    PropertyChange {
        address: address.clone(),
        before: json!(before),
        after: json!(after),
    }
}

#[test]
fn personal_undo_preserves_other_users_later_disjoint_change_and_repeated_own_undo() {
    let (targets, width, height) = setup();
    let mut history = ContributionHistory::default();
    history
        .record(
            operation("alice", "first"),
            1,
            vec![change(&width, 12, 14)],
            &targets,
        )
        .unwrap();
    history
        .record(
            operation("alice", "second"),
            2,
            vec![change(&width, 14, 16)],
            &targets,
        )
        .unwrap();
    history
        .record(
            operation("bob", "first"),
            3,
            vec![change(&height, 8, 9)],
            &targets,
        )
        .unwrap();
    let inverse = history.prepare_undo("alice", &targets).unwrap();
    assert_eq!(inverse.changes(), &[change(&width, 16, 14)]);
    history
        .commit_inverse(&inverse, operation("alice", "undo-1"), 4, &targets)
        .unwrap();
    assert_eq!(
        history.property_owner(&height),
        Some(&operation("bob", "first"))
    );
    let inverse = history.prepare_undo("alice", &targets).unwrap();
    assert_eq!(inverse.changes(), &[change(&width, 14, 12)]);
    history
        .commit_inverse(&inverse, operation("alice", "undo-2"), 5, &targets)
        .unwrap();
    let redo = history.prepare_redo("alice", &targets).unwrap();
    assert_eq!(redo.changes(), &[change(&width, 12, 14)]);
    history
        .commit_inverse(&redo, operation("alice", "redo-1"), 6, &targets)
        .unwrap();
    let redo = history.prepare_redo("alice", &targets).unwrap();
    assert_eq!(redo.changes(), &[change(&width, 14, 16)]);
    assert_eq!(history.revision(), 6);
}

#[test]
fn same_value_later_write_blocks_another_users_undo_in_both_orders() {
    for (first, second) in [("alice", "bob"), ("bob", "alice")] {
        let (targets, width, _) = setup();
        let mut history = ContributionHistory::default();
        history
            .record(
                operation(first, "first"),
                1,
                vec![change(&width, 12, 14)],
                &targets,
            )
            .unwrap();
        history
            .record(
                operation(second, "first"),
                2,
                vec![change(&width, 14, 14)],
                &targets,
            )
            .unwrap();
        assert!(matches!(
            history.prepare_undo(first, &targets),
            Err(HistoryError::Overwritten)
        ));
        let inverse = history.prepare_undo(second, &targets).unwrap();
        history
            .commit_inverse(&inverse, operation(second, "undo"), 3, &targets)
            .unwrap();
        assert!(history.prepare_undo(first, &targets).is_ok());
    }
}

#[test]
fn competing_change_during_validation_rejects_entire_inverse_without_partial_undo() {
    let (targets, width, height) = setup();
    let mut history = ContributionHistory::default();
    history
        .record(
            operation("alice", "first"),
            1,
            vec![change(&width, 12, 14), change(&height, 8, 9)],
            &targets,
        )
        .unwrap();
    let prepared = history.prepare_undo("alice", &targets).unwrap();
    history
        .record(
            operation("bob", "first"),
            2,
            vec![change(&height, 9, 9)],
            &targets,
        )
        .unwrap();
    let before = history.to_json().unwrap();
    assert_eq!(
        history.commit_inverse(&prepared, operation("alice", "undo"), 3, &targets),
        Err(HistoryError::Overwritten)
    );
    assert_eq!(history.to_json().unwrap(), before);
    assert_eq!(
        history.property_owner(&width),
        Some(&operation("alice", "first"))
    );
}

#[test]
fn redoing_cannot_replace_a_newer_same_value_contribution() {
    let (targets, width, _) = setup();
    let mut history = ContributionHistory::default();
    history
        .record(
            operation("alice", "first"),
            1,
            vec![change(&width, 12, 14)],
            &targets,
        )
        .unwrap();
    let inverse = history.prepare_undo("alice", &targets).unwrap();
    history
        .commit_inverse(&inverse, operation("alice", "undo"), 2, &targets)
        .unwrap();
    history
        .record(
            operation("bob", "first"),
            3,
            vec![change(&width, 12, 12)],
            &targets,
        )
        .unwrap();
    assert!(matches!(
        history.prepare_redo("alice", &targets),
        Err(HistoryError::Overwritten)
    ));
}

#[test]
fn deletion_recreation_refuses_old_inverse_even_with_same_name_and_value() {
    let (mut targets, width, _) = setup();
    let mut history = ContributionHistory::default();
    history
        .record(
            operation("alice", "first"),
            1,
            vec![change(&width, 12, 14)],
            &targets,
        )
        .unwrap();
    targets
        .delete(
            &targets
                .plan_delete(std::slice::from_ref(&width.target))
                .unwrap(),
        )
        .unwrap();
    targets.create("main/width").unwrap();
    assert!(matches!(
        history.prepare_undo("alice", &targets),
        Err(HistoryError::Lifetime)
    ));
}

#[test]
fn durable_history_preserves_effective_ownership_and_checked_redo_after_restart() {
    let (targets, width, height) = setup();
    let mut history = ContributionHistory::default();
    history
        .record(
            operation("alice", "first"),
            1,
            vec![change(&width, 12, 14)],
            &targets,
        )
        .unwrap();
    history
        .record(
            operation("bob", "first"),
            2,
            vec![change(&height, 8, 9)],
            &targets,
        )
        .unwrap();
    let inverse = history.prepare_undo("alice", &targets).unwrap();
    history
        .commit_inverse(&inverse, operation("alice", "undo"), 3, &targets)
        .unwrap();
    let json = history.to_json().unwrap();
    let mut restored = ContributionHistory::from_json(&json).unwrap();
    assert_eq!(restored.to_json().unwrap(), json);
    let redo = restored.prepare_redo("alice", &targets).unwrap();
    restored
        .commit_inverse(&redo, operation("alice", "redo"), 4, &targets)
        .unwrap();
    assert_eq!(
        restored.property_owner(&height),
        Some(&operation("bob", "first"))
    );
    assert_eq!(
        restored.property_owner(&width),
        Some(&operation("alice", "first"))
    );
    let mut corrupt: serde_json::Value = serde_json::from_str(&json).unwrap();
    corrupt["contributions"][0]["operation"]["requestId"] = json!("forged");
    assert!(ContributionHistory::from_json(&corrupt.to_string()).is_err());
}

#[test]
fn restoration_refuses_missing_or_unowned_property_state_and_impossible_order() {
    let (targets, width, _) = setup();
    let mut history = ContributionHistory::default();
    history
        .record(
            operation("alice", "one"),
            1,
            vec![change(&width, 12, 14)],
            &targets,
        )
        .unwrap();
    history
        .record(
            operation("bob", "one"),
            2,
            vec![change(&width, 14, 14)],
            &targets,
        )
        .unwrap();
    let saved = history.to_json().unwrap();
    for corruption in ["missing", "ownerless", "old-owner", "revision", "extra"] {
        let mut wire: serde_json::Value = serde_json::from_str(&saved).unwrap();
        match corruption {
            "missing" => wire["properties"] = json!([]),
            "ownerless" => wire["properties"][0]["owner"] = serde_json::Value::Null,
            "old-owner" => {
                wire["properties"][0]["owner"] =
                    serde_json::to_value(operation("alice", "one")).unwrap();
            }
            "revision" => wire["revision"] = json!(1),
            _ => {
                let mut added = wire["properties"][0].clone();
                added["address"]["property"] = json!("unrecorded-property");
                added["owner"] = serde_json::Value::Null;
                wire["properties"].as_array_mut().unwrap().push(added);
            }
        }
        assert!(
            ContributionHistory::from_json(&wire.to_string()).is_err(),
            "{corruption}"
        );
    }
    assert!(ContributionHistory::from_json(&saved).is_ok());
}
