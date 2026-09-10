// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_collaboration::targets::{TargetError, TargetLedger, TargetLimits};

#[test]
fn recreation_rejects_old_targets_and_allocator_rejections_consume_nothing() {
    let mut state = TargetLedger::new(TargetLimits::default()).unwrap();
    let original = state.create("file-one/width").unwrap();
    let allocated = state.high_water();
    assert_eq!(state.create("file-one/width"), Err(TargetError::Exists));
    assert_eq!(state.high_water(), allocated);
    state
        .delete(&state.plan_delete(std::slice::from_ref(&original)).unwrap())
        .unwrap();
    assert_eq!(state.authenticate(&original), Err(TargetError::Stale));
    let replacement = state.create("file-one/width").unwrap();
    assert!(replacement.generation > original.generation);
    assert_eq!(state.authenticate(&original), Err(TargetError::Stale));
    assert!(state.authenticate(&replacement).is_ok());
}

#[test]
fn stale_deletion_never_swallows_another_editors_new_dependency() {
    let mut state = TargetLedger::new(TargetLimits::default()).unwrap();
    let width = state.create("file-one/width").unwrap();
    let first = state.create("file-one/channel").unwrap();
    state
        .set_dependencies(&first, std::slice::from_ref(&width))
        .unwrap();
    let before = state.plan_delete(std::slice::from_ref(&width)).unwrap();
    let second = state.create("file-two/channel").unwrap();
    state
        .set_dependencies(&second, std::slice::from_ref(&first))
        .unwrap();
    assert_eq!(state.delete(&before), Err(TargetError::DeletionChanged));
    for target in [&width, &first, &second] {
        assert!(state.authenticate(target).is_ok());
    }
    let reviewed = state.plan_delete(std::slice::from_ref(&width)).unwrap();
    assert_eq!(reviewed.closure.len(), 3);
    state.delete(&reviewed).unwrap();
    for target in [&width, &first, &second] {
        assert_eq!(state.authenticate(target), Err(TargetError::Stale));
    }
    assert_eq!(state.dependency_count(), 0);
}

#[test]
fn disjoint_creation_does_not_stale_an_exact_deletion() {
    let mut state = TargetLedger::new(TargetLimits::default()).unwrap();
    let point = state.create("file/point").unwrap();
    let plan = state.plan_delete(&[point]).unwrap();
    let unrelated = state.create("file/unrelated").unwrap();
    state.delete(&plan).unwrap();
    assert!(state.authenticate(&unrelated).is_ok());
}

#[test]
fn removed_and_recreated_dependent_invalidates_old_closure() {
    let mut state = TargetLedger::new(TargetLimits::default()).unwrap();
    let parent = state.create("file/parent").unwrap();
    let child = state.create("file/child").unwrap();
    state
        .set_dependencies(&child, std::slice::from_ref(&parent))
        .unwrap();
    let plan = state.plan_delete(std::slice::from_ref(&parent)).unwrap();
    state.delete(&state.plan_delete(&[child]).unwrap()).unwrap();
    let replacement = state.create("file/child").unwrap();
    state.set_dependencies(&replacement, &[parent]).unwrap();
    assert_eq!(
        state.authenticate_delete(&plan),
        Err(TargetError::DeletionChanged)
    );
}

#[test]
fn failed_dependency_replacement_preserves_adjacency_and_limits_include_tombstones() {
    let mut state = TargetLedger::new(TargetLimits {
        max_objects_including_tombstones: 3,
        max_dependencies_per_object: 1,
        max_total_dependencies: 1,
    })
    .unwrap();
    let a = state.create("file/a").unwrap();
    let b = state.create("file/b").unwrap();
    let c = state.create("file/c").unwrap();
    state
        .set_dependencies(&b, std::slice::from_ref(&a))
        .unwrap();
    let expected = state.plan_delete(std::slice::from_ref(&a)).unwrap();
    assert_eq!(
        state.set_dependencies(&b, &[a.clone(), c.clone()]),
        Err(TargetError::Limit)
    );
    assert_eq!(
        state.set_dependencies(&c, std::slice::from_ref(&a)),
        Err(TargetError::Limit)
    );
    assert_eq!(state.plan_delete(&[a]).unwrap(), expected);
    state.delete(&state.plan_delete(&[c]).unwrap()).unwrap();
    assert_eq!(state.create("file/d"), Err(TargetError::Limit));
}

#[test]
fn durable_roundtrip_preserves_tombstones_closures_and_generation_high_water() {
    let mut state = TargetLedger::new(TargetLimits::default()).unwrap();
    let parent = state.create("file/parent").unwrap();
    let child = state.create("file/child").unwrap();
    let removed = state.create("file/removed").unwrap();
    state
        .set_dependencies(&child, std::slice::from_ref(&parent))
        .unwrap();
    state
        .delete(&state.plan_delete(std::slice::from_ref(&removed)).unwrap())
        .unwrap();
    let json = state.to_json().unwrap();
    let mut restored = TargetLedger::from_json(&json, TargetLimits::default()).unwrap();
    assert_eq!(restored.to_json().unwrap(), json);
    assert_eq!(
        restored.plan_delete(std::slice::from_ref(&parent)).unwrap(),
        state.plan_delete(std::slice::from_ref(&parent)).unwrap()
    );
    assert_eq!(restored.authenticate(&removed), Err(TargetError::Stale));
    let replacement = restored.create("file/removed").unwrap();
    assert!(replacement.generation > removed.generation);
    let mut corrupt: serde_json::Value = serde_json::from_str(&json).unwrap();
    corrupt["objects"][0]["target"]["generation"] = serde_json::json!(0);
    assert!(TargetLedger::from_json(&corrupt.to_string(), TargetLimits::default()).is_err());
}
