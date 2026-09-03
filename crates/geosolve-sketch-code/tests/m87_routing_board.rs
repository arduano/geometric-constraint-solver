// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProjectDemoId, KeyedReconcileState, ManagedControlConsumerTarget,
    bundled_code_project_demos, expand_code_project, managed_control_manifest,
    materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

#[test]
fn normalized_routing_board_retains_local_and_shared_runtime_control_fanout() {
    let project = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoboticRoutingBoard)
        .expect("routing-board sample")
        .project();
    let desired = required_generated_members(&project).expect("routing generated inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("routing generated reconciliation")
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x90_60)).unwrap();
    let expansion = expand_code_project(&project, &generated, intent.identity()).unwrap();
    let manifest = managed_control_manifest(&project, &expansion).unwrap();

    let generated_fanouts = manifest
        .editable()
        .filter(|control| {
            matches!(
                control.source.declaration.0.as_str(),
                "sharedClipRadius" | "sharedBendRadius"
            )
        })
        .map(|control| {
            control
                .consumers
                .iter()
                .filter(|consumer| {
                    matches!(
                        consumer.target,
                        ManagedControlConsumerTarget::Generated { .. }
                    )
                })
                .count()
        })
        .collect::<Vec<_>>();
    assert_eq!(generated_fanouts.len(), 2);
    assert!(generated_fanouts.iter().all(|count| *count > 1));
    assert!(manifest.editable().any(|control| {
        control
            .consumers
            .iter()
            .filter(|consumer| {
                matches!(
                    consumer.target,
                    ManagedControlConsumerTarget::Declaration { .. }
                )
            })
            .count()
            == 1
    }));

    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x90_60),
        DocumentId(PersistentId::from_u128(0x90_60)),
        1.0,
    )
    .expect("routing board cold materialization");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted routing authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    assert!(!materialized.expansion.generated_children.is_empty());
    assert!(!materialized.expansion.host_requests.is_empty());
}
