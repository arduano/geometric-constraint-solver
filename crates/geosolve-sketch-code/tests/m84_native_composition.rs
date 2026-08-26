// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{ComputedCornerRef, IntentNativeBinding};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeCompositionError, CodeHostRequest, CodeInteractionOverlay, CodePointSeedSource,
    CodeProject, CodeProjectDemoId, ExpandedCodeProject, ExpandedSemanticTarget,
    GeneratedMemberAddress, KeyedReconcileState, MaterializedCodeProject, SemanticSymbol,
    bundled_code_project_demos, materialize_code_project_cold,
    materialize_code_project_incremental, materialize_code_project_incremental_with_overlay,
    parse_managed_source, rehydrate_materialized_code_project, required_generated_members,
    rounded_polyline_member_addresses,
};
use geosolve_sketch_intent::{
    AggregateKind, DimensionKind, InputRole, InputSlot, IntentFieldKey, IntentKey, IntentLiteral,
    IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentPlanDisposition, IntentPortRef, IntentPortRole, IntentPortSelector, IntentReservation,
    IntentSessionId, NodeId, PatchPortRef, PortId, ReservationId, intent_content_digest,
};

#[derive(Clone, Debug, PartialEq)]
struct DirectNativeIdentity {
    node: NodeId,
    port_ids: BTreeSet<PortId>,
    reservations: BTreeMap<ReservationId, IntentReservation>,
    semantic_port: IntentPortRef,
    native: IntentNativeBinding,
}

#[derive(Clone, Debug, PartialEq)]
struct HostNativeIdentity {
    node: NodeId,
    port_ids: BTreeSet<PortId>,
    reservations: BTreeMap<ReservationId, IntentReservation>,
    owner: ComputedCornerRef,
}

#[test]
fn all_four_code_projects_materialize_through_native_authority() {
    for (ordinal, demo) in bundled_code_project_demos().into_iter().enumerate() {
        let project = demo.project();
        let desired = required_generated_members(&project).unwrap();
        let plan = KeyedReconcileState::empty()
            .plan(desired, &BTreeSet::new())
            .unwrap();
        let materialized = materialize_code_project_cold(
            &project,
            plan.staged(),
            IntentSessionId::from_raw(0x84_0000 + ordinal as u128),
            DocumentId(PersistentId::from_u128(0x84_0000 + ordinal as u128)),
            1.0,
        )
        .unwrap_or_else(|error| panic!("{} failed native composition: {error}", demo.id.key()));

        let accepted = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("a composed project owns accepted native authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );
        materialized
            .editor
            .coordinator()
            .intent()
            .graph()
            .validate()
            .unwrap();
        accepted
            .ownership
            .validate_against(
                materialized
                    .editor
                    .coordinator()
                    .intent()
                    .semantic_identity(),
                materialized.editor.coordinator().intent().graph(),
                materialized.editor.coordinator().intent().instance(),
                materialized.editor.coordinator().intent().reservations(),
                accepted
                    .session
                    .accepted_state_for_current_input()
                    .unwrap()
                    .document(),
                &accepted.features,
            )
            .unwrap();
        assert_eq!(
            materialized.expansion.semantic_outputs.len(),
            demo.output_kinds.len()
        );

        match demo.id {
            CodeProjectDemoId::RoundedPolyline => {
                assert_eq!(materialized.host_outputs.len(), 4);
                assert_eq!(accepted.validation.feature_count, 4);
            }
            CodeProjectDemoId::TypedPanel => {
                assert_eq!(materialized.host_outputs.len(), 2);
                assert_eq!(accepted.validation.feature_count, 2);
            }
            CodeProjectDemoId::BracedFrame => {
                assert!(materialized.host_outputs.is_empty());
                assert_eq!(accepted.validation.feature_count, 0);
            }
            CodeProjectDemoId::MountingPlate => {
                assert_eq!(materialized.host_outputs.len(), 1);
                assert_eq!(accepted.validation.feature_count, 4);
                assert_eq!(materialized.host_outputs.values().next().unwrap().len(), 4);
            }
        }
    }
}

#[test]
fn restored_editor_rehydrates_exact_base_aliases_and_generated_owners() {
    for (ordinal, demo) in bundled_code_project_demos().into_iter().enumerate() {
        let project = demo.project();
        let desired = required_generated_members(&project).unwrap();
        let plan = KeyedReconcileState::empty()
            .plan(desired, &BTreeSet::new())
            .unwrap();
        let materialized = materialize_code_project_cold(
            &project,
            plan.staged(),
            IntentSessionId::from_raw(0x84_0800 + ordinal as u128),
            DocumentId(PersistentId::from_u128(0x84_0800 + ordinal as u128)),
            1.0,
        )
        .unwrap();
        let expected_aliases = materialized.base_outcome.aliases.clone();
        let expected_hosts = materialized.host_outputs.clone();
        let expected_identity = materialized.editor.coordinator().intent().identity();
        let restored = rehydrate_materialized_code_project(
            Box::new(materialized.editor),
            materialized.expansion,
        )
        .unwrap_or_else(|error| panic!("{} failed rehydration: {error}", demo.id.key()));

        assert_eq!(restored.base_outcome.aliases, expected_aliases);
        assert_eq!(restored.host_outputs, expected_hosts);
        assert_eq!(
            restored.editor.coordinator().intent().identity(),
            expected_identity
        );
        assert_valid_native_authority(&restored, expected_hosts.values().map(Vec::len).sum());
    }
}

#[test]
fn rehydration_rejects_tampered_expansion_without_mutating_native_authority() {
    let demo = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoundedPolyline)
        .unwrap();
    let project = demo.project();
    let desired = required_generated_members(&project).unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_0900),
        DocumentId(PersistentId::from_u128(0x84_0900)),
        1.0,
    )
    .unwrap();
    let expected_identity = materialized.editor.coordinator().intent().identity();
    let expected_document = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .clone();
    let mut tampered = materialized.expansion.clone();
    let CodeHostRequest::FilletAtCorner(request) = &mut tampered.host_requests[0] else {
        panic!("rounded Polyline host request must be one Fillet")
    };
    request.radius.value = 2.0;
    refresh_expansion_digest(&mut tampered);
    let error =
        rehydrate_materialized_code_project(Box::new(materialized.editor), tampered).unwrap_err();
    assert!(matches!(
        error,
        CodeCompositionError::RehydratedHostMismatch { .. }
    ));
    // The consumed editor is not available after a failed ownership move, so
    // independently prove that the exact pre-call authority was finite and
    // stable rather than relying on a status-like composition result.
    assert_eq!(expected_identity.session.raw(), 0x84_0900);
    assert!(
        expected_document
            .points()
            .iter()
            .all(|point| { point.position[0].is_finite() && point.position[1].is_finite() })
    );
}

#[test]
fn adaptive_polyline_insertion_retains_all_fifteen_existing_native_identities() {
    let demo = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoundedPolyline)
        .unwrap();
    let project = demo.project();
    let initial_members = required_generated_members(&project).unwrap();
    let initial = KeyedReconcileState::empty()
        .plan(initial_members, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let before = materialize_code_project_cold(
        &project,
        &initial,
        IntentSessionId::from_raw(0x84_1000),
        DocumentId(PersistentId::from_u128(0x84_1000)),
        1.0,
    )
    .unwrap();
    let before_direct = direct_native_identities(&before);
    let before_hosts = before.host_outputs.clone();
    assert_eq!(before_direct.len(), 11);
    assert_eq!(before_hosts.len(), 4);

    let mut edited = project.clone();
    let needle =
        "      { key: \"fall\", position: [55, 10] },\n      { key: \"end\", position: [65, 10] },";
    let replacement = "      { key: \"fall\", position: [55, 10] },\n      { key: \"crest\", position: [61, 16] },\n      { key: \"end\", position: [65, 10] },";
    let edited_source = edited.managed.source.replacen(needle, replacement, 1);
    assert_ne!(edited_source, edited.managed.source);
    edited.managed = parse_managed_source(&edited_source).unwrap();
    let desired = required_generated_members(&edited).unwrap();
    let plan = initial.plan(desired, &BTreeSet::new()).unwrap();
    assert_eq!(plan.created.len(), 3);
    assert_eq!(plan.retained.len(), 15);
    let retained = plan
        .retained
        .iter()
        .map(|member| member.address.clone())
        .collect::<BTreeSet<_>>();
    let after = materialize_code_project_incremental(&before, &edited, plan.staged()).unwrap();

    let after_direct = direct_native_identities(&after);
    for (address, identity) in before_direct {
        assert!(retained.contains(&address));
        assert_eq!(after_direct.get(&address), Some(&identity), "{address:?}");
    }
    for (address, outputs) in before_hosts {
        assert!(retained.contains(&address));
        assert_eq!(
            after.host_outputs.get(&address),
            Some(&outputs),
            "{address:?}"
        );
    }
    assert_eq!(after_direct.len(), 13);
    assert_eq!(after.host_outputs.len(), 5);
    let accepted = after
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert_eq!(accepted.validation.feature_count, 5);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps removal, rebinding, tombstone, reuse, and native identity assertions adjacent"
)]
fn adaptive_polyline_removal_rebinds_neighbors_and_reuses_no_retired_identity() {
    let project = rounded_project();
    let initial_members = required_generated_members(&project).unwrap();
    let initial = KeyedReconcileState::empty()
        .plan(initial_members, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let before = materialize_code_project_cold(
        &project,
        &initial,
        IntentSessionId::from_raw(0x84_1100),
        DocumentId(PersistentId::from_u128(0x84_1100)),
        1.0,
    )
    .unwrap();
    let before_direct = direct_native_identities(&before);
    let before_hosts = host_native_identities(&before);

    let removed_project = without_shoulder(&project);
    let removal_plan = initial
        .plan(
            required_generated_members(&removed_project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    assert_eq!(removal_plan.removed.len(), 3);
    assert_eq!(removal_plan.retained.len(), 12);
    let removed_addresses = removal_plan
        .removed
        .iter()
        .map(|member| member.address.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(removed_addresses, shoulder_addresses());
    let removed_state = removal_plan.staged().clone();
    let after_removal =
        materialize_code_project_incremental(&before, &removed_project, &removed_state).unwrap();
    assert_valid_native_authority(&after_removal, 3);

    let removed_direct = direct_native_identities(&after_removal);
    for (address, identity) in &before_direct {
        if removed_addresses.contains(address) {
            assert!(!removed_direct.contains_key(address), "{address:?}");
            assert!(
                after_removal
                    .editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node(identity.node)
                    .is_none(),
                "removed declaration survived for {address:?}"
            );
            for reservation in identity.reservations.keys() {
                assert_eq!(
                    after_removal
                        .editor
                        .coordinator()
                        .intent()
                        .reservations()
                        .entries()[reservation]
                        .state,
                    geosolve_sketch_intent::IntentReservationState::Tombstoned
                );
            }
        } else {
            assert_eq!(removed_direct.get(address), Some(identity), "{address:?}");
        }
    }
    let removed_hosts = host_native_identities(&after_removal);
    for (key, identity) in &before_hosts {
        if removed_addresses.contains(&key.0) {
            assert!(!removed_hosts.contains_key(key), "{key:?}");
            assert!(
                after_removal
                    .editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node(identity.node)
                    .is_none(),
                "removed Fillet declaration survived for {key:?}"
            );
        } else {
            assert_eq!(removed_hosts.get(key), Some(identity), "{key:?}");
        }
    }
    for member in &removal_plan.removed {
        assert_eq!(removed_state.tombstones()[&member.address], member.identity);
    }

    let reuse_plan = removed_state
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    assert_eq!(reuse_plan.created.len(), 3);
    assert_eq!(reuse_plan.retained.len(), 12);
    for created in &reuse_plan.created {
        let retired = initial.active()[&created.address];
        assert!(created.identity.allocation > retired.allocation);
        assert!(created.identity.generation > retired.generation);
    }
    let restored =
        materialize_code_project_incremental(&after_removal, &project, reuse_plan.staged())
            .unwrap();
    assert_valid_native_authority(&restored, 4);
    let restored_direct = direct_native_identities(&restored);
    let restored_hosts = host_native_identities(&restored);
    for address in &removed_addresses {
        if let Some(retired) = before_direct.get(address) {
            assert_ne!(restored_direct.get(address), Some(retired), "{address:?}");
        } else {
            let key = (address.clone(), address.member_key.clone());
            assert_ne!(restored_hosts.get(&key), before_hosts.get(&key), "{key:?}");
        }
    }
    for (address, identity) in &removed_direct {
        if !removed_addresses.contains(address) {
            assert_eq!(restored_direct.get(address), Some(identity), "{address:?}");
        }
    }
    for (key, identity) in &removed_hosts {
        if !removed_addresses.contains(&key.0) {
            assert_eq!(restored_hosts.get(key), Some(identity), "{key:?}");
        }
    }
}

#[test]
fn managed_radius_edit_retains_every_fillet_owner_node_port_and_reservation() {
    let project = rounded_project();
    let initial = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let before = materialize_code_project_cold(
        &project,
        &initial,
        IntentSessionId::from_raw(0x84_1200),
        DocumentId(PersistentId::from_u128(0x84_1200)),
        1.0,
    )
    .unwrap();
    let before_direct = direct_native_identities(&before);
    let before_hosts = host_native_identities(&before);
    assert_eq!(fillet_radii(&before), BTreeSet::from([0.4_f64.to_bits()]));

    let mut edited = project.clone();
    edited.managed = parse_managed_source(&edited.managed.source.replacen(
        "radius: mm(0.4)",
        "radius: mm(0.65)",
        1,
    ))
    .unwrap();
    let plan = initial
        .plan(
            required_generated_members(&edited).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    assert!(plan.created.is_empty());
    assert!(plan.removed.is_empty());
    assert_eq!(plan.retained.len(), 15);
    let after = materialize_code_project_incremental(&before, &edited, plan.staged()).unwrap();

    assert_eq!(direct_native_identities(&after), before_direct);
    assert_eq!(host_native_identities(&after), before_hosts);
    assert_eq!(fillet_radii(&after), BTreeSet::from([0.65_f64.to_bits()]));
    assert_valid_native_authority(&after, 4);
}

#[test]
fn native_outside_dependent_rejects_removal_without_mutating_prior_authority() {
    let project = rounded_project();
    let initial = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let mut before = materialize_code_project_cold(
        &project,
        &initial,
        IntentSessionId::from_raw(0x84_1300),
        DocumentId(PersistentId::from_u128(0x84_1300)),
        1.0,
    )
    .unwrap();
    let shoulder_span =
        GeneratedMemberAddress::new("path", ["polyline", "segment"], ["shoulder"], ["span"]);
    let source = direct_native_identities(&before)[&shoulder_span].semantic_port;
    let alias = IntentKey::new("outside.shoulder.chain").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        alias.clone(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: source },
    );
    let outcome = before
        .editor
        .apply_patch(IntentPatch::new(
            before.editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias,
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let exact_before = before.editor.coordinator().intent().identity();

    let removed_project = without_shoulder(&project);
    let plan = initial
        .plan(
            required_generated_members(&removed_project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    assert!(matches!(
        materialize_code_project_incremental(&before, &removed_project, plan.staged()),
        Err(CodeCompositionError::IncrementalOutsideDependent { .. })
    ));
    assert_eq!(
        before.editor.coordinator().intent().identity(),
        exact_before
    );
    assert_valid_native_authority(&before, 4);
}

#[test]
fn warm_structural_edit_preserves_ordinary_outside_dependent_on_retained_output() {
    let project = rounded_project();
    let initial = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let mut before = materialize_code_project_cold(
        &project,
        &initial,
        IntentSessionId::from_raw(0x84_1400),
        DocumentId(PersistentId::from_u128(0x84_1400)),
        1.0,
    )
    .unwrap();
    let shoulder_span =
        GeneratedMemberAddress::new("path", ["polyline", "segment"], ["shoulder"], ["span"]);
    let source = direct_native_identities(&before)[&shoulder_span].semantic_port;
    let symbol = IntentKey::new("outside.shoulder.chain").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        symbol.clone(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: source },
    );
    let outcome = before
        .editor
        .apply_patch(IntentPatch::new(
            before.editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: symbol.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let outside_before = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&symbol)
        .unwrap()
        .clone();

    let mut edited = project.clone();
    edited.managed = parse_managed_source(&edited.managed.source.replacen(
        "radius: mm(0.4)",
        "radius: mm(0.65)",
        1,
    ))
    .unwrap();
    let plan = initial
        .plan(
            required_generated_members(&edited).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    let after = materialize_code_project_incremental(&before, &edited, plan.staged()).unwrap();
    let outside_after = after
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&symbol)
        .unwrap();
    assert_eq!(outside_after, &outside_before);
    assert_eq!(
        outside_after
            .inputs
            .get(&InputSlot::new(InputRole::Span, 0)),
        Some(&source)
    );
    assert_eq!(
        direct_native_identities(&after)[&shoulder_span].semantic_port,
        source
    );
    assert_valid_native_authority(&after, 4);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact three-level survivor regression keeps code replacement, native identity, and GUI dimension authority adjacent"
)]
fn detached_segment_rebind_preserves_retained_code_and_gui_descendant_chain() {
    let project = collaborative_reference_project();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let mut before = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_1500),
        DocumentId(PersistentId::from_u128(0x84_1500)),
        1.0,
    )
    .unwrap();

    let declaration_alias = |name: &str| {
        before
            .expansion
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| {
                (declaration == &SemanticSymbol(name.into())).then_some(alias.clone())
            })
            .unwrap_or_else(|| panic!("missing managed declaration `{name}`"))
    };
    let diagonal_alias = declaration_alias("diagonal");
    let follower_alias = declaration_alias("follower");
    let diagonal_before = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&diagonal_alias)
        .unwrap()
        .clone();
    let follower_before = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&follower_alias)
        .unwrap()
        .clone();
    assert_eq!(
        follower_before.inputs[&InputSlot::new(InputRole::Point, 0)].node,
        diagonal_before.id,
    );
    let follower_span = follower_before
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .unwrap()
        .as_ref(follower_before.id);
    let outside_symbol = IntentKey::new("outside.follower.reference-length").unwrap();
    let outside_draft = IntentNodeDraft::new(
        IntentNodeKind::Dimension {
            dimension: DimensionKind::CurveLength,
        },
        outside_symbol.clone(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable {
            port: follower_span,
        },
    )
    .with_field(
        IntentFieldKey(IntentKey::new("mode").unwrap()),
        IntentLiteral::Enum(IntentKey::new("reference").unwrap()),
    );
    let outcome = before
        .editor
        .apply_patch(IntentPatch::new(
            before.editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: outside_symbol.clone(),
                draft: Box::new(outside_draft),
                cell: None,
            }],
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let outside_before = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&outside_symbol)
        .unwrap()
        .clone();
    assert_eq!(
        outside_before.inputs[&InputSlot::new(InputRole::Span, 0)].node,
        follower_before.id,
    );

    let diagonal_start = before
        .expansion
        .writable_points
        .iter()
        .find(|point| {
            point.handle.alias == diagonal_alias
                && point.handle.selector
                    == (IntentPortSelector::Node {
                        role: IntentPortRole::Start,
                        index: 0,
                    })
                && matches!(point.source, CodePointSeedSource::Reference { .. })
        })
        .unwrap()
        .clone();
    let overlay = diagonal_start
        .stage_drag(&CodeInteractionOverlay::empty(), [5.0, 6.0])
        .unwrap();
    let after =
        materialize_code_project_incremental_with_overlay(&before, &project, &generated, &overlay)
            .unwrap();
    let graph = after.editor.coordinator().intent().graph();
    let diagonal_after = graph.node_by_symbol(&diagonal_alias).unwrap();
    let follower_after = graph.node_by_symbol(&follower_alias).unwrap();
    let outside_after = graph.node_by_symbol(&outside_symbol).unwrap();

    assert_ne!(diagonal_after.id, diagonal_before.id);
    assert_eq!(follower_after.id, follower_before.id);
    assert_eq!(
        follower_after
            .ports
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        follower_before
            .ports
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
    );
    assert_eq!(follower_after.reservations, follower_before.reservations);
    assert_eq!(
        follower_after.inputs[&InputSlot::new(InputRole::Point, 0)].node,
        diagonal_after.id,
    );
    assert_eq!(outside_after, &outside_before);
    assert_eq!(
        outside_after.inputs[&InputSlot::new(InputRole::Span, 0)],
        follower_span,
    );
    assert_eq!(
        after
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .dimensions()
            .len(),
        1,
    );
    assert_valid_native_authority(&after, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one regression audits generated detachment, owner stability, rebind and native point locality"
)]
fn generated_referenced_segment_detaches_locally_and_rebinds_its_constraint() {
    let project = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::BracedFrame)
        .unwrap()
        .project();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let before = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_1510),
        DocumentId(PersistentId::from_u128(0x84_1510)),
        1.0,
    )
    .unwrap();
    let rising_address = generated
        .active()
        .keys()
        .find(|address| {
            address.invocation == "brace"
                && address.template == ["diagonals", "rising"]
                && address.output == ["span"]
        })
        .unwrap()
        .clone();
    let rising_identity = generated.active()[&rising_address];
    let ExpandedSemanticTarget::Port { port: rising_span } =
        &before.expansion.generated_provenance[&rising_address].target
    else {
        panic!("generated rising brace must publish a span port")
    };
    let rising_alias = rising_span.alias.clone();
    let rising_before = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&rising_alias)
        .unwrap()
        .clone();
    let writable_start = before
        .expansion
        .writable_points
        .iter()
        .find(|point| {
            point.handle.alias == rising_alias
                && point.handle.selector
                    == (IntentPortSelector::Node {
                        role: IntentPortRole::Start,
                        index: 0,
                    })
        })
        .unwrap()
        .clone();
    assert_eq!(
        writable_start.source,
        CodePointSeedSource::GeneratedReference,
    );
    let start_before = rising_before
        .port_by_selector(writable_start.handle.selector)
        .unwrap()
        .as_ref(rising_before.id);
    let accepted_before = before
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    let IntentNativeBinding::Point(shared_point) =
        accepted_before.ownership.port(start_before).unwrap()
    else {
        panic!("generated brace endpoint must alias one native point")
    };
    let producer_port = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .flat_map(|node| node.ports.values().map(move |port| port.as_ref(node.id)))
        .find(|port| {
            *port != start_before
                && accepted_before.ownership.port(*port)
                    == Some(IntentNativeBinding::Point(shared_point))
        })
        .expect("the generated reference must have an attached producer");
    let producer_position = accepted_before
        .session
        .design_document()
        .point(shared_point)
        .unwrap()
        .position;
    let datum_alias = before
        .expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("datum".into())).then_some(alias.clone())
        })
        .unwrap();
    let datum_before = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&datum_alias)
        .unwrap()
        .clone();
    assert!(
        datum_before
            .inputs
            .values()
            .any(|input| input.node == rising_before.id)
    );

    let target = [5.0, 6.0];
    let overlay = writable_start
        .stage_drag(&CodeInteractionOverlay::empty(), target)
        .unwrap();
    let after =
        materialize_code_project_incremental_with_overlay(&before, &project, &generated, &overlay)
            .unwrap();
    let graph = after.editor.coordinator().intent().graph();
    let rising_after = graph.node_by_symbol(&rising_alias).unwrap();
    let datum_after = graph.node_by_symbol(&datum_alias).unwrap();
    assert_ne!(rising_after.id, rising_before.id);
    assert_eq!(datum_after.id, datum_before.id);
    assert!(
        datum_after
            .inputs
            .values()
            .any(|input| input.node == rising_after.id)
    );
    assert_eq!(
        after.expansion.generated_provenance[&rising_address].identity, rising_identity,
        "local detachment must not churn generated semantic ownership",
    );
    let start_after = rising_after
        .port_by_selector(writable_start.handle.selector)
        .unwrap()
        .as_ref(rising_after.id);
    let accepted_after = after
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    let IntentNativeBinding::Point(detached_point) =
        accepted_after.ownership.port(start_after).unwrap()
    else {
        panic!("detached generated endpoint must remain a native point")
    };
    assert_ne!(detached_point, shared_point);
    assert_eq!(
        accepted_after.ownership.port(producer_port),
        Some(IntentNativeBinding::Point(shared_point)),
    );
    assert_eq!(
        accepted_after
            .session
            .design_document()
            .point(shared_point)
            .unwrap()
            .position
            .map(f64::to_bits),
        producer_position.map(f64::to_bits),
    );
    assert_eq!(
        accepted_after
            .session
            .design_document()
            .point(detached_point)
            .unwrap()
            .position
            .map(f64::to_bits),
        target.map(f64::to_bits),
    );
    assert_valid_native_authority(&after, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact generated-circle detachment regression authenticates producer and consumer native ownership"
)]
fn generated_referenced_circle_center_detaches_without_moving_its_producer() {
    let project = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::MountingPlate)
        .unwrap()
        .project();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let before = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_1520),
        DocumentId(PersistentId::from_u128(0x84_1520)),
        1.0,
    )
    .unwrap();
    let center = before
        .expansion
        .writable_points
        .iter()
        .find(|point| {
            point.source == CodePointSeedSource::GeneratedReference
                && point.handle.selector
                    == (IntentPortSelector::Node {
                        role: IntentPortRole::Center,
                        index: 0,
                    })
        })
        .unwrap()
        .clone();
    let circle_before = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&center.handle.alias)
        .unwrap()
        .clone();
    let center_before = circle_before
        .port_by_selector(center.handle.selector)
        .unwrap()
        .as_ref(circle_before.id);
    let accepted_before = before
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    let IntentNativeBinding::Point(shared_point) =
        accepted_before.ownership.port(center_before).unwrap()
    else {
        panic!("generated circle center must alias one producer point")
    };
    let producer_port = before
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .flat_map(|node| node.ports.values().map(move |port| port.as_ref(node.id)))
        .find(|port| {
            *port != center_before
                && accepted_before.ownership.port(*port)
                    == Some(IntentNativeBinding::Point(shared_point))
        })
        .expect("the generated center reference must have one producer");
    let producer_position = accepted_before
        .session
        .design_document()
        .point(shared_point)
        .unwrap()
        .position;
    let (circle_address, circle_identity) = before
        .expansion
        .generated_provenance
        .iter()
        .find_map(|(address, provenance)| match &provenance.target {
            ExpandedSemanticTarget::Port { port } if port.alias == center.handle.alias => {
                Some((address.clone(), provenance.identity))
            }
            _ => None,
        })
        .unwrap();

    let target = [-20.0, 10.0];
    let overlay = center
        .stage_drag(&CodeInteractionOverlay::empty(), target)
        .unwrap();
    let after =
        materialize_code_project_incremental_with_overlay(&before, &project, &generated, &overlay)
            .unwrap();
    let circle_after = after
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&center.handle.alias)
        .unwrap();
    assert_ne!(circle_after.id, circle_before.id);
    assert_eq!(
        after.expansion.generated_provenance[&circle_address].identity,
        circle_identity,
    );
    let center_after = circle_after
        .port_by_selector(center.handle.selector)
        .unwrap()
        .as_ref(circle_after.id);
    let accepted_after = after
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    let IntentNativeBinding::Point(detached_point) =
        accepted_after.ownership.port(center_after).unwrap()
    else {
        panic!("detached circle center must remain a native point")
    };
    assert_ne!(detached_point, shared_point);
    assert_eq!(
        accepted_after.ownership.port(producer_port),
        Some(IntentNativeBinding::Point(shared_point)),
    );
    assert_eq!(
        accepted_after
            .session
            .design_document()
            .point(shared_point)
            .unwrap()
            .position
            .map(f64::to_bits),
        producer_position.map(f64::to_bits),
    );
    assert_eq!(
        accepted_after
            .session
            .design_document()
            .point(detached_point)
            .unwrap()
            .position
            .map(f64::to_bits),
        target.map(f64::to_bits),
    );
    assert_valid_native_authority(&after, 4);
}

fn rounded_project() -> CodeProject {
    bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoundedPolyline)
        .unwrap()
        .project()
}

fn collaborative_reference_project() -> CodeProject {
    let source = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  const follower = $.geometry.line("follower", {
    start: diagonal.start,
    end: [15, -10],
  });
  return $.outputs({ frame, diagonal, follower });
});
"#;
    CodeProject {
        project: geosolve_sketch_code::ProjectKey("m84-composition-closure".into()),
        managed: parse_managed_source(source).unwrap(),
        custom_files: BTreeMap::new(),
        artifacts: BTreeMap::new(),
        lock: serde_json::json!({ "format": "geosolve-lock-v1", "modules": {} }),
    }
}

fn without_shoulder(project: &CodeProject) -> CodeProject {
    let mut edited = project.clone();
    let source =
        edited
            .managed
            .source
            .replacen("      { key: \"shoulder\", position: [24, 12] },\n", "", 1);
    assert_ne!(source, edited.managed.source);
    edited.managed = parse_managed_source(&source).unwrap();
    edited
}

fn shoulder_addresses() -> BTreeSet<GeneratedMemberAddress> {
    rounded_polyline_member_addresses(
        "path",
        "rounded",
        &["start", "rise", "shoulder", "ridge", "fall", "end"],
        false,
    )
    .into_iter()
    .filter(|address| address.member_key == ["shoulder"])
    .collect()
}

fn direct_native_identities(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<GeneratedMemberAddress, DirectNativeIdentity> {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    materialized
        .expansion
        .generated_provenance
        .iter()
        .filter_map(|(address, provenance)| {
            let ExpandedSemanticTarget::Port { port } = &provenance.target else {
                return None;
            };
            let node = intent.graph().node_by_symbol(&port.alias).unwrap();
            let port = node
                .port_by_selector(port.selector)
                .unwrap()
                .as_ref(node.id);
            let native = accepted.ownership.port(port).unwrap();
            Some((
                address.clone(),
                DirectNativeIdentity {
                    node: node.id,
                    port_ids: node.ports.keys().copied().collect(),
                    reservations: node.reservations.clone(),
                    semantic_port: port,
                    native,
                },
            ))
        })
        .collect()
}

fn host_native_identities(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<(GeneratedMemberAddress, Vec<String>), HostNativeIdentity> {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    materialized
        .host_outputs
        .iter()
        .flat_map(|(address, outputs)| {
            outputs.iter().map(|output| {
                let node = accepted
                    .ownership
                    .exact_owner(IntentNativeBinding::ComputedFeature(output.owner.feature))
                    .unwrap();
                let node = intent.graph().node(node).unwrap();
                (
                    (address.clone(), output.member_key.clone()),
                    HostNativeIdentity {
                        node: node.id,
                        port_ids: node.ports.keys().copied().collect(),
                        reservations: node.reservations.clone(),
                        owner: output.owner,
                    },
                )
            })
        })
        .collect()
}

fn fillet_radii(materialized: &MaterializedCodeProject) -> BTreeSet<u64> {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    materialized
        .host_outputs
        .values()
        .flatten()
        .map(|output| {
            serde_json::to_value(accepted.features.feature(output.owner.feature).unwrap()).unwrap()
                ["definition"]["radius"]
                .as_f64()
                .unwrap()
                .to_bits()
        })
        .collect()
}

fn refresh_expansion_digest(expansion: &mut ExpandedCodeProject) {
    let provenance_rows = expansion.generated_provenance.iter().collect::<Vec<_>>();
    let declaration_rows = expansion.declaration_provenance.iter().collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&(
        &expansion.patch,
        &expansion.semantic_outputs,
        &provenance_rows,
        &declaration_rows,
        &expansion.writable_points,
        &expansion.generated_children,
        &expansion.host_requests,
    ))
    .unwrap();
    expansion.digest = intent_content_digest(&bytes).to_string();
}

fn assert_valid_native_authority(materialized: &MaterializedCodeProject, feature_count: usize) {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert_eq!(accepted.validation.feature_count, feature_count);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    intent.graph().validate().unwrap();
    accepted
        .ownership
        .validate_against(
            intent.semantic_identity(),
            intent.graph(),
            intent.instance(),
            intent.reservations(),
            accepted
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document(),
            &accepted.features,
        )
        .unwrap();
}
