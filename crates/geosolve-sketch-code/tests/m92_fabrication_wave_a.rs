// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/catalog_contract.rs"]
mod catalog_contract;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, KeyedReconcileState, bundled_sample,
    materialize_code_project_cold, rehydrate_materialized_code_project, required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

const SAMPLES: [&str; 5] = [
    "pc-water-manifold",
    "cnc-dogbone-coupon",
    "vacuum-fixture-plate",
    "dust-shoe-clamp",
    "gridfinity-bin-section",
];

// Independent ordered names retained from the reviewed M92 sample contract.
// Display metadata is now authored in sketch.ts, not duplicated in manifests.
fn expected_groups(key: &str) -> &'static [&'static str] {
    match key {
        "pc-water-manifold" => &[
            "Manifold envelope and reservoir",
            "Upper channel circuit",
            "Middle channel circuit",
            "Lower channel circuit",
            "Point-to-point stair channel",
            "Shared circuit seal",
            "Fastener stack",
        ],
        "cnc-dogbone-coupon" => &[
            "Female coupon blank",
            "Relational station datums",
            "Press-fit station",
            "Nominal-fit station",
            "Loose-fit station",
        ],
        "vacuum-fixture-plate" => &[
            "Fixture envelope",
            "Gasket groove",
            "Vacuum distribution grid",
            "Workholding pattern",
        ],
        "dust-shoe-clamp" => &[
            "Spindle clamp ring",
            "Dust extraction port",
            "Symmetric clamp lug",
            "Split relief",
        ],
        "gridfinity-bin-section" => &[
            "3U material section",
            "Section standard dimensions",
            "Section projection datums",
            "1 x 1 plan study",
        ],
        _ => panic!("unknown fabrication sample {key}"),
    }
}

fn asset(key: &str, file: &str) -> String {
    fs::read_to_string(format!(
        "{}/assets/bundled-samples/{key}/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|error| panic!("read {key}/{file}: {error}"))
}

fn compiled(key: &str) -> CompiledManagedSource {
    let source = asset(key, "sketch.ts");
    let compiled = CompiledManagedSource::from_json(&asset(key, "sketch.compiled.json"))
        .unwrap_or_else(|error| panic!("{key} compiler envelope: {error}"));
    compiled
        .validate_input_source(&source)
        .unwrap_or_else(|error| panic!("{key} source authority: {error}"));
    assert_eq!(
        compiled.normalized_source, source,
        "{key} normalized source"
    );
    compiled
}

fn project(key: &str) -> CodeProject {
    compiled(key);
    bundled_sample(key)
        .expect("registered fabrication sample")
        .project()
}

fn validate(key: &str, seed: u128) {
    let project = project(key);
    let canonical_project = project.to_canonical_json().expect("canonical project");
    assert_eq!(
        CodeProject::from_json(&canonical_project).expect("restored project"),
        project,
        "{key} project round-trip"
    );
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
        .expect("accepted fabrication authority");
    assert!(accepted.validation.hard_residuals_validated, "{key}");
    assert!(accepted.validation.all_active_features_current, "{key}");
    let maximum_residual = accepted
        .validation
        .maximum_normalized_hard_residual
        .unwrap_or(0.0);
    assert!(
        maximum_residual.is_finite() && maximum_residual <= 1.0e-9,
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
    assert!(
        state
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite()),
        "{key}"
    );
    let raw = state
        .diagnostics()
        .rank
        .expect("rank diagnostics")
        .numerical_right_nullity
        .expect("numerical right nullity");
    let mobility = state.diagnostics().mobility.expect("mobility diagnostics");
    let effective = mobility
        .bidirectional_bounded_degrees_of_freedom
        .expect("bidirectional bounded DOF");
    assert_eq!(raw, 0, "{key} numerical right nullity");
    assert_eq!(
        mobility.equality_degrees_of_freedom,
        Some(0),
        "{key} equality DOF"
    );
    assert_eq!(effective, 0, "{key} bidirectional bounded DOF");
    if key == "pc-water-manifold" {
        validate_manifold_restore(&materialized, seed);
    }
}

fn validate_manifold_restore(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    seed: u128,
) {
    let state = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let document_before = state.document().clone();
    let intent_wire = materialized
        .editor
        .coordinator()
        .intent()
        .to_canonical_json()
        .unwrap();
    let expansion_wire = serde_json::to_string(&materialized.expansion).unwrap();
    let restored_editor = ProjectionalEditorSession::restore(
        IntentSession::from_json(&intent_wire).unwrap(),
        DocumentId(PersistentId::from_u128(seed)),
        1.0,
    )
    .expect("channel native checkpoint independently restores");
    let restored = rehydrate_materialized_code_project(
        Box::new(restored_editor),
        serde_json::from_str(&expansion_wire).unwrap(),
    )
    .expect("channel host members and boundary checks reauthenticate");
    assert_eq!(
        restored
            .editor
            .coordinator()
            .intent()
            .to_canonical_json()
            .unwrap(),
        intent_wire,
        "manifold serialized native checkpoint is reproduced exactly",
    );
    assert_eq!(restored.host_outputs, materialized.host_outputs);
    assert_eq!(
        serde_json::to_string(&restored.expansion).unwrap(),
        expansion_wire
    );
    let accepted = restored
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert_eq!(
        accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &document_before
    );
}

#[test]
fn fabrication_wave_a_is_source_authoritative_grouped_and_fully_constrained() {
    for (ordinal, key) in SAMPLES.into_iter().enumerate() {
        let manifest: serde_json::Value =
            serde_json::from_str(&asset(key, "manifest.json")).expect("manifest JSON");
        assert_eq!(
            manifest["ordinal"],
            catalog_contract::ordinal(key),
            "{key} ordinal"
        );
        assert_eq!(manifest["key"], key, "{key} manifest key");
        assert_eq!(manifest["expected"]["raw_dof"], 0, "{key}");
        assert_eq!(manifest["expected"]["effective_dof"], 0, "{key}");

        let compiled = compiled(key);
        assert_eq!(
            compiled
                .artifact
                .groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            expected_groups(key),
            "{key} ordered functional groups"
        );
        let declarations = compiled
            .artifact
            .declarations
            .iter()
            .map(|declaration| declaration.declaration.as_str())
            .collect::<BTreeSet<_>>();
        let fixed_points = compiled
            .artifact
            .declarations
            .iter()
            .filter(|entry| entry.family == "constraint.fixedPoint")
            .count();
        assert!(fixed_points <= 1, "{key} uses {fixed_points} fixed points");
        if key == "gridfinity-bin-section" {
            assert_eq!(
                fixed_points, 0,
                "Gridfinity uses symmetry and two scalar Y datums"
            );
            assert_eq!(
                compiled
                    .artifact
                    .declarations
                    .iter()
                    .filter(|entry| entry.family == "constraint.fixedCoordinate")
                    .count(),
                2,
                "Gridfinity section and plan each own one scalar Y datum"
            );
        }
        let mut ownership = BTreeMap::<&str, usize>::new();
        for group in &compiled.artifact.groups {
            assert!(!group.declarations.is_empty(), "{key} group {}", group.name);
            for member in &group.declarations {
                assert!(
                    member.path.is_empty(),
                    "{key} group {} must own flat roots",
                    group.name
                );
                assert!(
                    declarations.contains(member.declaration.as_str()),
                    "{key} unknown group member"
                );
                *ownership.entry(member.declaration.as_str()).or_default() += 1;
            }
        }
        for declaration in declarations {
            assert_eq!(
                ownership.get(declaration),
                Some(&1),
                "{key} declaration {declaration}"
            );
        }

        let witnesses: serde_json::Value =
            serde_json::from_str(&asset(key, "witnesses.json")).expect("witnesses JSON");
        assert_eq!(witnesses["format"], "geosolve-sample-witnesses-v1", "{key}");
        let edited = witnesses["representative_edit"]["declaration"]
            .as_str()
            .expect("representative edit declaration");
        assert!(
            compiled
                .artifact
                .declarations
                .iter()
                .any(|entry| entry.declaration == edited),
            "{key} representative edit must address a declaration"
        );
        assert_eq!(witnesses["drags"], serde_json::json!([]), "{key}");

        validate(key, 0x92_60 + ordinal as u128);
    }
}
