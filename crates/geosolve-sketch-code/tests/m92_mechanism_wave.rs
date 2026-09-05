// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::BTreeSet, fs};

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, KeyedReconcileState, ProjectKey,
    materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;

fn project(key: &str) -> CodeProject {
    let path = format!(
        "{}/assets/bundled-samples/{key}/sketch.compiled.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let compiled = CompiledManagedSource::from_json(
        &fs::read_to_string(path).expect("read mechanism compiler envelope"),
    )
    .expect("valid mechanism compiler envelope");
    CodeProject::managed(ProjectKey(format!("geosolve-sample-{key}")), compiled)
        .expect("valid mechanism project")
}

fn validate(key: &str, expected_raw_dof: usize, expected_effective_dof: usize, seed: u128) {
    let project = project(key);
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated member reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(seed),
        DocumentId(PersistentId::from_u128(seed)),
        1.0,
    )
    .unwrap_or_else(|error| panic!("{key} cold materialization failed: {error}"));
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted mechanism authority");
    assert!(accepted.validation.hard_residuals_validated, "{key}");
    assert!(accepted.validation.all_active_features_current, "{key}");
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        "{key}"
    );
    let state = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted solve authority");
    assert!(
        state
            .document()
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite),
        "{key}"
    );
    let rank = state.diagnostics().rank.expect("rank diagnostics");
    assert_eq!(
        rank.numerical_right_nullity,
        Some(expected_raw_dof),
        "{key} raw DOF"
    );
    let mobility = state.diagnostics().mobility.expect("mobility diagnostics");
    assert_eq!(
        mobility.equality_degrees_of_freedom,
        Some(expected_raw_dof),
        "{key} equality DOF"
    );
    assert_eq!(
        mobility.bidirectional_bounded_degrees_of_freedom,
        Some(expected_effective_dof),
        "{key} effective DOF"
    );
}

#[test]
fn advanced_mechanism_wave_cold_materializes_with_intended_mobility() {
    for (ordinal, (key, raw, effective)) in [
        ("theo-jansen-leg", 1, 1),
        ("whitworth-quick-return", 1, 1),
        ("peaucellier-linkage", 1, 1),
        ("five-stage-scissor-lift", 1, 1),
    ]
    .into_iter()
    .enumerate()
    {
        validate(key, raw, effective, 0x92_10 + ordinal as u128);
    }
}
