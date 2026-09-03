// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::ComputedFeatureEvaluationState;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProjectDemoId, KeyedReconcileState, bundled_code_project_demos,
    materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;

const ONE_MIB: usize = 1024 * 1024;

// This is deliberately a native proxy with an independently explicit stack
// size. The browser/WASM contract is owned separately by the actual
// wasm-bindgen Gridfinity regression in geosolve-demo-web.
#[test]
fn gridfinity_cold_materialization_fits_one_mib_native_proxy_stack() {
    let child = std::thread::Builder::new()
        .name("gridfinity-one-mib-stack".into())
        .stack_size(ONE_MIB)
        .spawn(|| {
            let project = bundled_code_project_demos()
                .into_iter()
                .find(|demo| demo.id == CodeProjectDemoId::GridfinityBinSection)
                .expect("bundled Gridfinity demo")
                .project();
            let generated = KeyedReconcileState::empty()
                .plan(
                    required_generated_members(&project)
                        .expect("Gridfinity generated-member inventory"),
                    &BTreeSet::new(),
                )
                .expect("Gridfinity generated-member reconciliation")
                .into_staged();
            assert_eq!(generated.active().len(), 66);

            let materialized = materialize_code_project_cold(
                &project,
                &generated,
                IntentSessionId::from_raw(0x8800_0100),
                DocumentId(PersistentId::from_u128(0x8800_0100)),
                1.0,
            )
            .expect("Gridfinity cold materialization must fit the one-MiB native proxy stack");
            let accepted = materialized
                .editor
                .coordinator()
                .accepted_materialization()
                .expect("Gridfinity owns accepted native authority");
            assert!(accepted.validation.hard_residuals_validated);
            assert!(accepted.validation.all_active_features_current);
            assert!(
                accepted
                    .validation
                    .maximum_normalized_hard_residual
                    .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
            );
            assert!(
                accepted
                    .computed
                    .feature_evaluations()
                    .iter()
                    .all(|feature| {
                        matches!(
                            feature.state,
                            ComputedFeatureEvaluationState::Current { .. }
                        )
                    })
            );

            let accepted_state = accepted
                .session
                .accepted_state_for_current_input()
                .expect("success-like Gridfinity result owns its exact accepted input");
            let core_report = accepted_state.solve_result().unstable_core_report();
            assert!(core_report.hard_residuals_validated);
            assert!(
                core_report
                    .audit
                    .sources
                    .iter()
                    .flat_map(|source| &source.rows)
                    .all(|row| {
                        row.normalized_residual.is_finite()
                            && row.normalized_residual.abs() <= 1.0e-9
                    })
            );
            assert!(
                accepted_state
                    .document()
                    .points()
                    .iter()
                    .all(|point| { point.position.into_iter().all(f64::is_finite) })
            );
            assert!(
                accepted_state
                    .document()
                    .scalars()
                    .iter()
                    .all(|scalar| scalar.value.is_finite())
            );
            let rank = accepted_state
                .diagnostics()
                .rank
                .expect("Gridfinity numerical rank diagnostics");
            assert_eq!(rank.numerical_left_nullity, Some(0));
            assert_eq!(rank.numerical_right_nullity, Some(0));
            let mobility = accepted_state
                .diagnostics()
                .mobility
                .expect("Gridfinity mobility diagnostics");
            assert_eq!(mobility.equality_degrees_of_freedom, Some(0));
            assert_eq!(mobility.bidirectional_bounded_degrees_of_freedom, Some(0));
        })
        .expect("spawn explicit one-MiB Gridfinity stack");

    if let Err(panic) = child.join() {
        std::panic::resume_unwind(panic);
    }
}
