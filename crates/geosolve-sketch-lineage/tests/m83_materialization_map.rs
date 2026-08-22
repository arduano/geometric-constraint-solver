// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_lineage::{
    LineageActionDefinition, LineageDeveloperKey, LineageDocument, LineageDocumentId,
    LineageEvaluationPolicy, LineageMaterializationMap, LineageMaterializationMapError,
    LineageMaterializedIdentity, LineageMaterializedLeaf, LineageMutation, LineageOpaqueId,
    LineageOutput, LineageOutputId, LineageOutputIdentity, LineageOutputIdentityFlow,
    LineageOutputKind, LineageOutputRef, LineagePatch, LineageReservation, LineageReservationId,
    LineageReservationKind, LineageSemanticKey, LineageStep, LineageStepEvaluationState,
    LineageStepId, LineageWritableLeaf, VersionedActionPayload,
};

fn key(value: &str) -> LineageSemanticKey {
    LineageSemanticKey::new(value).unwrap()
}

fn action(schema: &str) -> LineageActionDefinition {
    LineageActionDefinition::Operation {
        action: VersionedActionPayload::empty(key(schema), 1),
    }
}

fn created_step(step: u64, output: u64, reservation: u64, persistent: &str) -> LineageStep {
    LineageStep::new(
        LineageStepId::from_raw(step),
        LineageDeveloperKey::new(format!("step-{step}")).unwrap(),
        format!("Step {step}"),
        action("geosolve.test.v1.created"),
        vec![LineageOutput {
            id: LineageOutputId::from_raw(output),
            key: key("point"),
            kind: LineageOutputKind::Point,
            reservation: Some(LineageReservationId::from_raw(reservation)),
        }],
        vec![LineageReservation {
            id: LineageReservationId::from_raw(reservation),
            key: key("point"),
            kind: LineageReservationKind::Point,
            persistent_id: LineageOpaqueId::new(persistent).unwrap(),
        }],
    )
    .with_writable_leaves(vec![LineageWritableLeaf {
        output: LineageOutputId::from_raw(output),
        key: key("position"),
    }])
}

fn transitioned_step(
    document: LineageDocumentId,
    step: u64,
    output: u64,
    source_step: u64,
    source_output: u64,
    flow: fn(LineageOutputRef) -> LineageOutputIdentityFlow,
) -> LineageStep {
    let source = LineageOutputRef {
        document,
        step: LineageStepId::from_raw(source_step),
        output: LineageOutputId::from_raw(source_output),
        kind: LineageOutputKind::Point,
    };
    let identity_flow = flow(source);
    let mut step = LineageStep::new(
        LineageStepId::from_raw(step),
        LineageDeveloperKey::new(format!("step-{step}")).unwrap(),
        format!("Step {step}"),
        action("geosolve.test.v1.transition"),
        vec![LineageOutput {
            id: LineageOutputId::from_raw(output),
            key: key("point"),
            kind: LineageOutputKind::Point,
            reservation: None,
        }],
        vec![],
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(output),
        flow: identity_flow,
    }]);
    if matches!(identity_flow, LineageOutputIdentityFlow::Continued { .. }) {
        step = step.with_writable_leaves(vec![LineageWritableLeaf {
            output: LineageOutputId::from_raw(output),
            key: key("position"),
        }]);
    }
    step
}

#[test]
fn materialization_map_tracks_alias_transfer_and_retirement_without_proximity() {
    let id = LineageDocumentId::from_raw(0x83);
    let mut document = LineageDocument::with_id(id);
    let created = created_step(1, 1, 1, "sketch:point:17");
    let alias = transitioned_step(id, 2, 2, 1, 1, |source| {
        LineageOutputIdentityFlow::Aliased { source }
    });
    let continued = transitioned_step(id, 3, 3, 1, 1, |source| {
        LineageOutputIdentityFlow::Continued { source }
    });
    let retired = transitioned_step(id, 4, 4, 3, 3, |source| {
        LineageOutputIdentityFlow::Retired { source }
    });
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(created),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(alias),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(continued),
                },
            ],
        ))
        .unwrap();

    let map = LineageMaterializationMap::derive(&document).unwrap();
    let identity = LineageMaterializedIdentity {
        kind: LineageReservationKind::Point,
        persistent_id: LineageOpaqueId::new("sketch:point:17").unwrap(),
    };
    assert_eq!(map.lineage(), document.identity());
    assert_eq!(
        map.owner_for(&identity).unwrap().step,
        LineageStepId::from_raw(3)
    );
    assert_eq!(map.owned_outputs(LineageStepId::from_raw(1)).len(), 1);
    assert!(map.owned_outputs(LineageStepId::from_raw(2)).is_empty());
    assert_eq!(map.owned_outputs(LineageStepId::from_raw(3)).len(), 1);
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(1))
            .is_empty()
    );
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(2))
            .is_empty()
    );
    assert_eq!(map.live_owned_outputs(LineageStepId::from_raw(3)).len(), 1);
    for logical_step in [1, 2, 3] {
        assert_eq!(
            map.bindings()
                .iter()
                .find(|binding| binding.logical.step == LineageStepId::from_raw(logical_step))
                .unwrap()
                .owner
                .as_ref()
                .unwrap()
                .step,
            LineageStepId::from_raw(3)
        );
    }
    assert_eq!(
        document
            .dirty_dependency_closure([LineageStepId::from_raw(1)])
            .unwrap(),
        vec![
            LineageStepId::from_raw(1),
            LineageStepId::from_raw(2),
            LineageStepId::from_raw(3),
        ]
    );

    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(retired),
            }],
        ))
        .unwrap();
    let retired_map = LineageMaterializationMap::derive(&document).unwrap();
    assert!(retired_map.owner_for(&identity).is_none());
    assert!(
        retired_map
            .bindings()
            .iter()
            .find(|binding| binding.logical.step == LineageStepId::from_raw(4))
            .unwrap()
            .leaf
            .is_none()
    );
}

#[test]
fn suppression_keeps_the_reservation_but_removes_live_reverse_authority() {
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x84));
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(created_step(1, 1, 1, "sketch:point:18")),
            }],
        ))
        .unwrap();
    let identity = LineageMaterializedIdentity {
        kind: LineageReservationKind::Point,
        persistent_id: LineageOpaqueId::new("sketch:point:18").unwrap(),
    };
    assert!(
        LineageMaterializationMap::derive(&document)
            .unwrap()
            .owner_for(&identity)
            .is_some()
    );

    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::SetSuppressed {
                step: LineageStepId::from_raw(1),
                suppressed: true,
            }],
        ))
        .unwrap();
    let map = LineageMaterializationMap::derive(&document).unwrap();
    assert!(map.owner_for(&identity).is_none());
    assert_eq!(map.owned_outputs(LineageStepId::from_raw(1)).len(), 1);
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(1))
            .is_empty()
    );
    assert_eq!(
        document.steps()[0].reservations[0].persistent_id,
        identity.persistent_id
    );
}

#[test]
fn blocked_continuation_has_no_live_reverse_authority() {
    let id = LineageDocumentId::from_raw(0x84_001);
    let mut document = LineageDocument::with_id(id);
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(created_step(1, 1, 1, "sketch:point:blocked")),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(transitioned_step(id, 2, 2, 1, 1, |source| {
                        LineageOutputIdentityFlow::Continued { source }
                    })),
                },
                LineageMutation::SetSuppressed {
                    step: LineageStepId::from_raw(2),
                    suppressed: true,
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(transitioned_step(id, 3, 3, 2, 2, |source| {
                        LineageOutputIdentityFlow::Continued { source }
                    })),
                },
            ],
        ))
        .unwrap();

    let plan = document
        .dependency_plan_for(LineageEvaluationPolicy::StrictChronological, [])
        .unwrap();
    assert_eq!(
        plan[2].state,
        LineageStepEvaluationState::Blocked {
            dependencies: vec![LineageStepId::from_raw(2)],
        }
    );

    let map = LineageMaterializationMap::derive(&document).unwrap();
    let identity = LineageMaterializedIdentity {
        kind: LineageReservationKind::Point,
        persistent_id: LineageOpaqueId::new("sketch:point:blocked").unwrap(),
    };
    assert_eq!(
        map.owner_for(&identity).unwrap().step,
        LineageStepId::from_raw(1)
    );
    assert_eq!(map.reverse_bindings().len(), 1);
    assert_eq!(
        map.declared_writable_owners(LineageStepId::from_raw(3))
            .len(),
        1
    );
    assert!(
        map.live_writable_owners(LineageStepId::from_raw(3))
            .is_empty()
    );
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(3))
            .is_empty()
    );
}

#[test]
fn cached_map_is_bounded_and_must_exactly_match_authoritative_lineage() {
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x85));
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(created_step(1, 1, 1, "sketch:point:19")),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(created_step(2, 2, 2, "sketch:point:20")),
                },
            ],
        ))
        .unwrap();
    let map = LineageMaterializationMap::derive(&document).unwrap();
    let json = map.to_canonical_json().unwrap();
    assert_eq!(
        LineageMaterializationMap::from_json_for_document(&json, &document).unwrap(),
        map
    );

    let mut wrong_version = serde_json::from_str::<serde_json::Value>(&json).unwrap();
    wrong_version["version"] = serde_json::json!(1);
    assert!(matches!(
        LineageMaterializationMap::from_json_for_document(
            &serde_json::to_string(&wrong_version).unwrap(),
            &document,
        ),
        Err(LineageMaterializationMapError::UnsupportedVersion {
            actual: 1,
            expected: 2,
        })
    ));

    let mut unsorted = serde_json::from_str::<serde_json::Value>(&json).unwrap();
    unsorted["bindings"].as_array_mut().unwrap().reverse();
    assert!(matches!(
        LineageMaterializationMap::from_json_for_document(
            &serde_json::to_string(&unsorted).unwrap(),
            &document,
        ),
        Err(LineageMaterializationMapError::CacheMismatch)
    ));

    let mut duplicate = serde_json::from_str::<serde_json::Value>(&json).unwrap();
    let reverse = duplicate["reverse"].as_array_mut().unwrap();
    reverse.push(reverse[0].clone());
    assert!(matches!(
        LineageMaterializationMap::from_json_for_document(
            &serde_json::to_string(&duplicate).unwrap(),
            &document,
        ),
        Err(LineageMaterializationMapError::CacheMismatch)
    ));

    assert!(matches!(
        LineageMaterializationMap::from_json_for_document(
            &" ".repeat(geosolve_sketch_lineage::MAX_LINEAGE_MATERIALIZATION_MAP_JSON_BYTES + 1),
            &document,
        ),
        Err(LineageMaterializationMapError::JsonResourceLimit { .. })
    ));

    let mut corrupt = serde_json::from_str::<serde_json::Value>(&json).unwrap();
    corrupt["reverse"] = serde_json::json!([]);
    assert!(matches!(
        LineageMaterializationMap::from_json_for_document(
            &serde_json::to_string(&corrupt).unwrap(),
            &document,
        ),
        Err(LineageMaterializationMapError::CacheMismatch)
    ));

    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::SetSuppressed {
                step: LineageStepId::from_raw(1),
                suppressed: true,
            }],
        ))
        .unwrap();
    assert!(matches!(
        LineageMaterializationMap::from_json_for_document(&json, &document),
        Err(LineageMaterializationMapError::CacheMismatch)
    ));
}

#[test]
fn cascade_tombstone_of_continuation_chain_derives_an_empty_live_map() {
    let id = LineageDocumentId::from_raw(0x86);
    let mut document = LineageDocument::with_id(id);
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(created_step(1, 1, 1, "sketch:point:20")),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(transitioned_step(id, 2, 2, 1, 1, |source| {
                        LineageOutputIdentityFlow::Continued { source }
                    })),
                },
            ],
        ))
        .unwrap();
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::DeleteSubtree {
                root: LineageStepId::from_raw(1),
            }],
        ))
        .unwrap();
    let map = LineageMaterializationMap::derive(&document).unwrap();
    assert!(map.reverse_bindings().is_empty());
    assert_eq!(map.owned_outputs(LineageStepId::from_raw(1)).len(), 1);
    assert_eq!(map.owned_outputs(LineageStepId::from_raw(2)).len(), 1);
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(1))
            .is_empty()
    );
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(2))
            .is_empty()
    );
}

#[test]
fn two_writable_leaves_of_one_identity_have_distinct_exact_owners() {
    let id = LineageDocumentId::from_raw(0x87);
    let mut document = LineageDocument::with_id(id);
    let reservation = LineageReservationId::from_raw(1);
    let step = LineageStep::new(
        LineageStepId::from_raw(1),
        LineageDeveloperKey::new("point-components").unwrap(),
        "Point components",
        action("geosolve.test.v1.point-components"),
        vec![LineageOutput {
            id: LineageOutputId::from_raw(1),
            key: key("point"),
            kind: LineageOutputKind::Point,
            reservation: Some(reservation),
        }],
        vec![LineageReservation {
            id: reservation,
            key: key("point"),
            kind: LineageReservationKind::Point,
            persistent_id: LineageOpaqueId::new("sketch:point:21").unwrap(),
        }],
    )
    .with_writable_leaves(vec![
        LineageWritableLeaf {
            output: LineageOutputId::from_raw(1),
            key: key("position.x"),
        },
        LineageWritableLeaf {
            output: LineageOutputId::from_raw(1),
            key: key("position.y"),
        },
    ]);
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(step),
            }],
        ))
        .unwrap();

    let map = LineageMaterializationMap::derive(&document).unwrap();
    let materialized = LineageMaterializedIdentity {
        kind: LineageReservationKind::Point,
        persistent_id: LineageOpaqueId::new("sketch:point:21").unwrap(),
    };
    let x = LineageMaterializedLeaf {
        materialized: materialized.clone(),
        key: key("position.x"),
    };
    let y = LineageMaterializedLeaf {
        materialized: materialized.clone(),
        key: key("position.y"),
    };
    assert_eq!(
        map.owner_for_leaf(&x).unwrap().output.output,
        LineageOutputId::from_raw(1)
    );
    assert_eq!(
        map.owner_for_leaf(&y).unwrap().output.output,
        LineageOutputId::from_raw(1)
    );
    assert_ne!(map.owner_for_leaf(&x), map.owner_for_leaf(&y));
    assert!(map.owner_for(&materialized).is_none());
    assert_eq!(map.reverse_bindings().len(), 2);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the continuation regression keeps both exact leaves and their predecessor/transfer ownership assertions together"
)]
fn continuation_transfers_only_declared_overlapping_leaves() {
    let id = LineageDocumentId::from_raw(0x89);
    let mut document = LineageDocument::with_id(id);
    let reservation = LineageReservationId::from_raw(1);
    let created_output = LineageOutputId::from_raw(1);
    let created = LineageStep::new(
        LineageStepId::from_raw(1),
        LineageDeveloperKey::new("split-owner-point").unwrap(),
        "Split-owner point",
        action("geosolve.test.v1.split-owner-point"),
        vec![LineageOutput {
            id: created_output,
            key: key("point"),
            kind: LineageOutputKind::Point,
            reservation: Some(reservation),
        }],
        vec![LineageReservation {
            id: reservation,
            key: key("point"),
            kind: LineageReservationKind::Point,
            persistent_id: LineageOpaqueId::new("sketch:point:23").unwrap(),
        }],
    )
    .with_writable_leaves(vec![
        LineageWritableLeaf {
            output: created_output,
            key: key("position.x"),
        },
        LineageWritableLeaf {
            output: created_output,
            key: key("position.y"),
        },
    ]);
    let continued_output = LineageOutputId::from_raw(2);
    let source = LineageOutputRef {
        document: id,
        step: LineageStepId::from_raw(1),
        output: created_output,
        kind: LineageOutputKind::Point,
    };
    let continued = LineageStep::new(
        LineageStepId::from_raw(2),
        LineageDeveloperKey::new("x-only-continuation").unwrap(),
        "X-only continuation",
        action("geosolve.test.v1.x-only-continuation"),
        vec![LineageOutput {
            id: continued_output,
            key: key("continued-point"),
            kind: LineageOutputKind::Point,
            reservation: None,
        }],
        vec![],
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: continued_output,
        flow: LineageOutputIdentityFlow::Continued { source },
    }])
    .with_writable_leaves(vec![LineageWritableLeaf {
        output: continued_output,
        key: key("position.x"),
    }]);
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(created),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(continued),
                },
            ],
        ))
        .unwrap();

    let materialized = LineageMaterializedIdentity {
        kind: LineageReservationKind::Point,
        persistent_id: LineageOpaqueId::new("sketch:point:23").unwrap(),
    };
    let x = LineageMaterializedLeaf {
        materialized: materialized.clone(),
        key: key("position.x"),
    };
    let y = LineageMaterializedLeaf {
        materialized,
        key: key("position.y"),
    };
    let map = LineageMaterializationMap::derive(&document).unwrap();
    assert_eq!(
        map.owner_for_leaf(&x).unwrap().step,
        LineageStepId::from_raw(2)
    );
    assert_eq!(
        map.owner_for_leaf(&y).unwrap().step,
        LineageStepId::from_raw(1)
    );
    assert_eq!(
        map.live_writable_owners(LineageStepId::from_raw(1)),
        &[geosolve_sketch_lineage::LineageMaterializationOwner {
            step: LineageStepId::from_raw(1),
            output: source,
            field: key("position.y"),
        }]
    );
    assert_eq!(
        map.live_writable_owners(LineageStepId::from_raw(2))[0].field,
        key("position.x")
    );
    assert_eq!(
        map.live_owned_outputs(LineageStepId::from_raw(1)),
        &[source]
    );
    assert_eq!(
        map.live_owned_outputs(LineageStepId::from_raw(2)),
        &[LineageOutputRef {
            document: id,
            step: LineageStepId::from_raw(2),
            output: continued_output,
            kind: LineageOutputKind::Point,
        }]
    );

    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::SetSuppressed {
                step: LineageStepId::from_raw(2),
                suppressed: true,
            }],
        ))
        .unwrap();
    let suppressed = LineageMaterializationMap::derive(&document).unwrap();
    assert_eq!(
        suppressed.owner_for_leaf(&x).unwrap().step,
        LineageStepId::from_raw(1)
    );
    assert_eq!(
        suppressed.owner_for_leaf(&y).unwrap().step,
        LineageStepId::from_raw(1)
    );
}

#[test]
fn suppressed_continuation_chain_keeps_reservations_without_reverse_owners() {
    let id = LineageDocumentId::from_raw(0x88);
    let mut document = LineageDocument::with_id(id);
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(created_step(1, 1, 1, "sketch:point:22")),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(transitioned_step(id, 2, 2, 1, 1, |source| {
                        LineageOutputIdentityFlow::Continued { source }
                    })),
                },
                LineageMutation::SetSuppressed {
                    step: LineageStepId::from_raw(1),
                    suppressed: true,
                },
                LineageMutation::SetSuppressed {
                    step: LineageStepId::from_raw(2),
                    suppressed: true,
                },
            ],
        ))
        .unwrap();

    let map = LineageMaterializationMap::derive(&document).unwrap();
    assert!(map.reverse_bindings().is_empty());
    assert_eq!(map.owned_outputs(LineageStepId::from_raw(1)).len(), 1);
    assert_eq!(map.owned_outputs(LineageStepId::from_raw(2)).len(), 1);
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(1))
            .is_empty()
    );
    assert!(
        map.live_owned_outputs(LineageStepId::from_raw(2))
            .is_empty()
    );
    assert_eq!(document.steps()[0].reservations.len(), 1);
    assert_eq!(document.allocator_high_water().next_reservation_id.raw(), 2);
}
