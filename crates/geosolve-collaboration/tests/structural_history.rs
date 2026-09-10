// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration::{
    history::{
        ContributionHistory, DependencyChange, HistoryError, ObjectDescription, PropertyAddress,
        PropertyChange, ReorderChange, StatementPosition, TransactionChanges,
    },
    protocol::OperationId,
    targets::{SemanticTarget, TargetLedger, TargetLimits},
};
use serde_json::json;
fn op(user: &str, id: &str) -> OperationId {
    OperationId {
        user_id: user.into(),
        client_id: format!("tab-{user}"),
        request_id: id.into(),
    }
}
fn ledger() -> TargetLedger {
    TargetLedger::new(TargetLimits::default()).unwrap()
}
fn description(target: &SemanticTarget, targets: &TargetLedger) -> ObjectDescription {
    ObjectDescription {
        target: target.clone(),
        dependencies: targets.dependencies(target).unwrap(),
        payload: json!({"declaration":target.object,"source":"circle({ radius: mm(12) })"}),
        position: StatementPosition::default(),
    }
}
fn property(target: &SemanticTarget, before: i32, after: i32) -> PropertyChange {
    PropertyChange {
        address: PropertyAddress {
            target: target.clone(),
            property: "radius".into(),
        },
        before: json!(before),
        after: json!(after),
    }
}
fn create(
    history: &mut ContributionHistory,
    targets: &mut TargetLedger,
    user: &str,
    id: &str,
    revision: u64,
    name: &str,
) -> SemanticTarget {
    let before = targets.clone();
    let target = targets.create(name).unwrap();
    history
        .record_transaction(
            op(user, id),
            revision,
            TransactionChanges {
                created: vec![description(&target, targets)],
                ..TransactionChanges::default()
            },
            &before,
            targets,
        )
        .unwrap();
    target
}
fn delete(
    history: &mut ContributionHistory,
    targets: &mut TargetLedger,
    user: &str,
    id: &str,
    revision: u64,
    roots: &[SemanticTarget],
) {
    let before = targets.clone();
    let plan = targets.plan_delete(roots).unwrap();
    let deleted = plan
        .closure
        .iter()
        .map(|target| description(target, targets))
        .collect();
    targets.delete(&plan).unwrap();
    history
        .record_transaction(
            op(user, id),
            revision,
            TransactionChanges {
                deleted,
                ..TransactionChanges::default()
            },
            &before,
            targets,
        )
        .unwrap();
}
fn undo(
    history: &mut ContributionHistory,
    targets: &mut TargetLedger,
    user: &str,
    id: &str,
    revision: u64,
) {
    let inverse = history.prepare_undo(user, targets).unwrap();
    history
        .commit_inverse_transaction(&inverse, op(user, id), revision, targets)
        .unwrap();
}
fn redo(
    history: &mut ContributionHistory,
    targets: &mut TargetLedger,
    user: &str,
    id: &str,
    revision: u64,
) {
    let inverse = history.prepare_redo(user, targets).unwrap();
    history
        .commit_inverse_transaction(&inverse, op(user, id), revision, targets)
        .unwrap();
}
fn restart(history: &mut ContributionHistory, targets: &mut TargetLedger) {
    *history = ContributionHistory::from_json(&history.to_json().unwrap()).unwrap();
    *targets =
        TargetLedger::from_json(&targets.to_json().unwrap(), TargetLimits::default()).unwrap();
}

#[test]
fn creation_undo_preserves_disjoint_bob_but_refuses_his_same_value_property() {
    let mut targets = ledger();
    let independent = targets.create("independent").unwrap();
    let mut history = ContributionHistory::default();
    let created = create(&mut history, &mut targets, "alice", "create", 1, "circle");
    history
        .record(
            op("bob", "independent"),
            2,
            vec![property(&independent, 12, 15)],
            &targets,
        )
        .unwrap();
    let prepared = history.prepare_undo("alice", &targets).unwrap();
    assert_eq!(
        prepared.structural().delete.as_ref().unwrap().closure,
        vec![created.clone()]
    );
    history
        .record(
            op("bob", "same"),
            3,
            vec![property(&created, 12, 12)],
            &targets,
        )
        .unwrap();
    let before = (history.to_json().unwrap(), targets.to_json().unwrap());
    assert_eq!(
        history.commit_inverse_transaction(&prepared, op("alice", "undo"), 4, &mut targets),
        Err(HistoryError::Overwritten)
    );
    assert_eq!(
        before,
        (history.to_json().unwrap(), targets.to_json().unwrap())
    );
    undo(&mut history, &mut targets, "bob", "undo-same", 4);
    undo(&mut history, &mut targets, "alice", "undo-create", 5);
    assert!(targets.current("circle").is_none());
    assert_eq!(
        history.property_owner(&property(&independent, 0, 0).address),
        Some(&op("bob", "independent"))
    );
}

#[test]
fn creation_undo_refuses_new_dependent_and_same_value_dependency_ownership() {
    for same_value in [false, true] {
        let mut targets = ledger();
        let anchor = targets.create("anchor").unwrap();
        let mut history = ContributionHistory::default();
        let created = create(&mut history, &mut targets, "alice", "create", 1, "circle");
        let before = targets.clone();
        if same_value {
            history
                .record_transaction(
                    op("bob", "deps"),
                    2,
                    TransactionChanges {
                        dependencies: vec![DependencyChange {
                            target: created.clone(),
                            before: vec![],
                            after: vec![],
                        }],
                        ..TransactionChanges::default()
                    },
                    &before,
                    &targets,
                )
                .unwrap();
        } else {
            targets
                .set_dependencies(&anchor, std::slice::from_ref(&created))
                .unwrap();
            history
                .record_transaction(
                    op("bob", "deps"),
                    2,
                    TransactionChanges {
                        dependencies: vec![DependencyChange {
                            target: anchor,
                            before: vec![],
                            after: vec![created],
                        }],
                        ..TransactionChanges::default()
                    },
                    &before,
                    &targets,
                )
                .unwrap();
        }
        assert!(matches!(
            history.prepare_undo("alice", &targets),
            Err(HistoryError::Overwritten)
        ));
        undo(&mut history, &mut targets, "bob", "undo-deps", 3);
        undo(&mut history, &mut targets, "alice", "undo-create", 4);
    }
}

#[test]
fn deletion_undo_restores_exact_closure_with_fresh_generations_and_independent_bob() {
    let mut targets = ledger();
    let parent = targets.create("parent").unwrap();
    let dependent = targets.create("dependent").unwrap();
    targets
        .set_dependencies(&dependent, std::slice::from_ref(&parent))
        .unwrap();
    let bob = targets.create("bob").unwrap();
    let mut history = ContributionHistory::default();
    history
        .record(
            op("alice", "edit"),
            1,
            vec![property(&parent, 12, 14)],
            &targets,
        )
        .unwrap();
    delete(
        &mut history,
        &mut targets,
        "alice",
        "delete",
        2,
        std::slice::from_ref(&parent),
    );
    history
        .record(op("bob", "edit"), 3, vec![property(&bob, 12, 15)], &targets)
        .unwrap();
    let high = targets.high_water();
    let prepared = history.prepare_undo("alice", &targets).unwrap();
    assert_eq!(targets.high_water(), high); // Independent model rejection consumes no allocation.
    assert_eq!(prepared.structural().create.len(), 2);
    history
        .commit_inverse_transaction(&prepared, op("alice", "undo-delete"), 4, &mut targets)
        .unwrap();
    let fresh = targets.current("parent").unwrap();
    let fresh_dependent = targets.current("dependent").unwrap();
    assert!(fresh.generation > high && fresh_dependent.generation > high);
    assert!(targets.authenticate(&parent).is_err() && targets.authenticate(&dependent).is_err());
    assert_eq!(
        targets.dependencies(&fresh_dependent).unwrap(),
        vec![fresh.clone()]
    );
    assert_eq!(
        history.prepare_undo("alice", &targets).unwrap().changes(),
        vec![property(&fresh, 14, 12)]
    );
    assert_eq!(
        history.property_owner(&property(&bob, 0, 0).address),
        Some(&op("bob", "edit"))
    );
    restart(&mut history, &mut targets);
    undo(&mut history, &mut targets, "alice", "undo-edit", 5);
    redo(&mut history, &mut targets, "alice", "redo-edit", 6);
    redo(&mut history, &mut targets, "alice", "redo-delete", 7);
    assert!(targets.current("parent").is_none() && targets.current("dependent").is_none());
    restart(&mut history, &mut targets);
    undo(&mut history, &mut targets, "alice", "restore-again", 8);
    assert!(targets.current("parent").unwrap().generation > fresh.generation);
}

#[test]
fn a_reused_name_even_deleted_again_cannot_restore_an_old_tombstone() {
    for delete_again in [false, true] {
        let mut targets = ledger();
        let old = targets.create("circle").unwrap();
        let mut history = ContributionHistory::default();
        delete(
            &mut history,
            &mut targets,
            "alice",
            "delete",
            1,
            std::slice::from_ref(&old),
        );
        let replacement = create(&mut history, &mut targets, "bob", "create", 2, "circle");
        if delete_again {
            delete(
                &mut history,
                &mut targets,
                "bob",
                "delete",
                3,
                std::slice::from_ref(&replacement),
            );
        }
        assert!(matches!(
            history.prepare_undo("alice", &targets),
            Err(HistoryError::Lifetime)
        ));
    }
}

#[test]
fn deletion_redo_refuses_new_dependents_and_later_same_value_owner() {
    for dependency in [false, true] {
        let mut targets = ledger();
        let old = targets.create("circle").unwrap();
        let independent = targets.create("independent").unwrap();
        let mut history = ContributionHistory::default();
        history
            .record(
                op("bob", "original"),
                1,
                vec![property(&old, 12, 14)],
                &targets,
            )
            .unwrap();
        delete(
            &mut history,
            &mut targets,
            "alice",
            "delete",
            2,
            std::slice::from_ref(&old),
        );
        undo(&mut history, &mut targets, "alice", "restore", 3);
        let fresh = targets.current("circle").unwrap();
        if dependency {
            targets
                .set_dependencies(&independent, std::slice::from_ref(&fresh))
                .unwrap();
        } else {
            history
                .record(
                    op("bob", "same"),
                    4,
                    vec![property(&fresh, 14, 14)],
                    &targets,
                )
                .unwrap();
        }
        let before = (history.to_json().unwrap(), targets.to_json().unwrap());
        assert!(matches!(
            history.prepare_redo("alice", &targets),
            Err(HistoryError::Overwritten)
        ));
        assert_eq!(
            before,
            (history.to_json().unwrap(), targets.to_json().unwrap())
        );
    }
}

#[test]
fn mixed_create_property_reorder_delete_timeline_survives_repeated_restart() {
    let mut targets = ledger();
    let left = targets.create("left").unwrap();
    let right = targets.create("right").unwrap();
    let mut history = ContributionHistory::default();
    let original = create(&mut history, &mut targets, "alice", "create", 1, "circle");
    history
        .record(
            op("alice", "edit"),
            2,
            vec![property(&original, 12, 14)],
            &targets,
        )
        .unwrap();
    let positioned = StatementPosition {
        previous: Some(left),
        next: Some(right),
    };
    history
        .record_transaction(
            op("alice", "reorder"),
            3,
            TransactionChanges {
                reorders: vec![ReorderChange {
                    target: original.clone(),
                    before: StatementPosition::default(),
                    after: positioned.clone(),
                }],
                ..TransactionChanges::default()
            },
            &targets,
            &targets,
        )
        .unwrap();
    // Captured deletion descriptor uses the current statement position.
    let before = targets.clone();
    let mut deleted = description(&original, &targets);
    deleted.position = positioned;
    targets
        .delete(
            &targets
                .plan_delete(std::slice::from_ref(&original))
                .unwrap(),
        )
        .unwrap();
    history
        .record_transaction(
            op("alice", "delete"),
            4,
            TransactionChanges {
                deleted: vec![deleted],
                ..TransactionChanges::default()
            },
            &before,
            &targets,
        )
        .unwrap();
    let mut revision = 5;
    for step in 0..4 {
        restart(&mut history, &mut targets);
        let inverse = history.prepare_undo("alice", &targets).unwrap();
        assert_eq!(
            inverse.contribution().request_id,
            ["delete", "reorder", "edit", "create"][step]
        );
        undo(
            &mut history,
            &mut targets,
            "alice",
            &format!("undo-{step}"),
            revision,
        );
        revision += 1;
    }
    assert!(targets.current("circle").is_none());
    for step in 0..4 {
        restart(&mut history, &mut targets);
        let inverse = history.prepare_redo("alice", &targets).unwrap();
        assert_eq!(
            inverse.contribution().request_id,
            ["create", "edit", "reorder", "delete"][step]
        );
        redo(
            &mut history,
            &mut targets,
            "alice",
            &format!("redo-{step}"),
            revision,
        );
        revision += 1;
    }
    restart(&mut history, &mut targets);
    undo(
        &mut history,
        &mut targets,
        "alice",
        "final-restore",
        revision,
    );
    assert!(targets.current("circle").unwrap().generation > original.generation);
}

#[test]
fn reorder_same_value_ownership_and_atomic_unrecorded_inventory_rejection() {
    let mut targets = ledger();
    let target = targets.create("circle").unwrap();
    let left = targets.create("left").unwrap();
    let mut history = ContributionHistory::default();
    let position = StatementPosition {
        previous: Some(left),
        next: None,
    };
    let transaction = |before, after| TransactionChanges {
        reorders: vec![ReorderChange {
            target: target.clone(),
            before,
            after,
        }],
        ..TransactionChanges::default()
    };
    history
        .record_transaction(
            op("alice", "reorder"),
            1,
            transaction(StatementPosition::default(), position.clone()),
            &targets,
            &targets,
        )
        .unwrap();
    history
        .record_transaction(
            op("bob", "same"),
            2,
            transaction(position.clone(), position),
            &targets,
            &targets,
        )
        .unwrap();
    assert!(matches!(
        history.prepare_undo("alice", &targets),
        Err(HistoryError::Overwritten)
    ));
    let saved = history.to_json().unwrap();
    let mut after = targets.clone();
    after.create("unrecorded").unwrap();
    assert_eq!(
        history.record_transaction(
            op("alice", "bad"),
            3,
            TransactionChanges {
                properties: vec![property(&target, 12, 13)],
                ..TransactionChanges::default()
            },
            &targets,
            &after
        ),
        Err(HistoryError::Stale)
    );
    assert_eq!(history.to_json().unwrap(), saved);
}

#[test]
fn deletion_redo_recovers_after_a_later_first_property_observation_is_undone() {
    let mut targets = ledger();
    let target = targets.create("circle").unwrap();
    let mut history = ContributionHistory::default();
    delete(
        &mut history,
        &mut targets,
        "alice",
        "delete",
        1,
        std::slice::from_ref(&target),
    );
    undo(&mut history, &mut targets, "alice", "restore", 2);
    let fresh = targets.current("circle").unwrap();
    history
        .record(
            op("bob", "first"),
            3,
            vec![property(&fresh, 12, 12)],
            &targets,
        )
        .unwrap();
    assert!(matches!(
        history.prepare_redo("alice", &targets),
        Err(HistoryError::Overwritten)
    ));
    undo(&mut history, &mut targets, "bob", "undo-first", 4);
    redo(&mut history, &mut targets, "alice", "redo-delete", 5);
    assert!(targets.current("circle").is_none());
}

#[test]
fn restoration_rejects_forged_structural_owner_links_and_untyped_dependency_values() {
    let mut targets = ledger();
    let mut history = ContributionHistory::default();
    let target = create(&mut history, &mut targets, "alice", "create", 1, "circle");
    history
        .record(
            op("bob", "value"),
            2,
            vec![property(&target, 12, 14)],
            &targets,
        )
        .unwrap();
    delete(
        &mut history,
        &mut targets,
        "alice",
        "delete",
        3,
        std::slice::from_ref(&target),
    );
    let saved = history.to_json().unwrap();
    for corruption in [
        "created-payload-lifetime",
        "missing-created-owner",
        "deleted-owner",
        "malformed-dependency",
    ] {
        let mut wire: serde_json::Value = serde_json::from_str(&saved).unwrap();
        match corruption {
            "created-payload-lifetime" => {
                wire["contributions"][0]["structural"]["created"][0]["target"]["generation"] =
                    json!(77);
            }
            "missing-created-owner" => wire["contributions"][0]["changes"] = json!([]),
            "deleted-owner" => {
                wire["contributions"][2]["structural"]["deleted"][0]["properties"][0]["owner"] =
                    json!({"userId":"forged","clientId":"tab-forged","requestId":"none"});
            }
            _ => {
                wire["contributions"][0]["changes"][0]["change"]["after"] =
                    json!({"not":"typed targets"});
            }
        }
        assert!(
            ContributionHistory::from_json(&wire.to_string()).is_err(),
            "{corruption}"
        );
    }
    assert_eq!(
        ContributionHistory::from_json(&saved)
            .unwrap()
            .to_json()
            .unwrap(),
        saved
    );
}
