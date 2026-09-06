// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/catalog_contract.rs"]
mod catalog_contract;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CodeProjectFile, CompiledManagedSource, KeyedReconcileState, PatchModuleArtifact,
    ProjectKey, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;

const SAMPLES: [&str; 5] = [
    "pc-water-manifold",
    "cnc-dogbone-coupon",
    "vacuum-fixture-plate",
    "dust-shoe-clamp",
    "gridfinity-bin-section",
];

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
    let compiled = compiled(key);
    if key == "pc-water-manifold" {
        let custom_source = asset(key, "patches/water-channel.patch.ts");
        let artifact = asset(key, "patches/water-channel.artifact.json");
        let patch: PatchModuleArtifact =
            serde_json::from_str(&artifact).expect("patch artifact shape");
        let validated = patch.clone().validate().expect("valid patch artifact");
        let managed = compiled
            .into_managed_document()
            .expect("valid manifold managed document");
        let mut custom_files = BTreeMap::new();
        custom_files.insert(
            "patches/water-channel.patch.ts".to_owned(),
            CodeProjectFile {
                path: "patches/water-channel.patch.ts".to_owned(),
                source_digest: geosolve_sketch_intent::intent_content_digest(
                    custom_source.as_bytes(),
                )
                .to_string(),
                contents: custom_source,
                managed: false,
            },
        );
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            validated.digest().to_owned(),
            serde_json::from_str(validated.canonical_json()).expect("canonical patch json"),
        );
        let project = CodeProject {
            project: ProjectKey(format!("geosolve-sample-{key}")),
            managed,
            custom_files,
            artifacts,
            lock: serde_json::json!({
                "format": "geosolve-lock-v1",
                "modules": {
                    patch.module_specifier.clone(): {
                        "artifact": validated.digest(),
                        "source": patch.source_digest,
                        "interface": patch.interface_digest,
                        "sdk_abi": patch.sdk_abi
                    }
                }
            }),
        };
        project.validate().expect("valid manifold project");
        project
    } else {
        CodeProject::managed(ProjectKey(format!("geosolve-sample-{key}")), compiled)
            .expect("valid fabrication project")
    }
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
        let expected_groups = manifest["groups"]
            .as_array()
            .expect("manifest groups")
            .iter()
            .map(|name| name.as_str().expect("group name"))
            .collect::<Vec<_>>();
        assert_eq!(
            compiled
                .artifact
                .groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            expected_groups,
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
