// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/catalog_contract.rs"]
mod catalog_contract;
#[path = "support/retained_sample.rs"]
mod retained_sample;
use retained_sample::TestSample;

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, KeyedReconcileState, ManagedControlEdit, ManagedControlEditBatch,
    ManagedPathSegment, ManagedSketchMutation, ManagedValue, SampleCategory, SemanticOutputPath,
    UnitLiteral, managed_control_manifest, materialize_code_project_cold,
    prepare_managed_control_mutation, required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;

const SAMPLES: [&str; 3] = ["voron-panel", "micron-carriage", "bondtech-indx-link"];

fn generated(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated member reconciliation")
        .into_staged()
}

fn witness_value(value: &serde_json::Value) -> ManagedValue {
    if let Some(number) = value.as_f64() {
        return ManagedValue::Number(number);
    }
    if let Some(pair) = value.as_array() {
        return ManagedValue::Array(pair.iter().map(witness_value).collect());
    }
    if let Some(object) = value.as_object()
        && let (Some(unit), Some(number)) = (
            object.get("unit").and_then(serde_json::Value::as_str),
            object.get("value").and_then(serde_json::Value::as_f64),
        )
    {
        return ManagedValue::Unit(UnitLiteral {
            unit: unit.to_owned(),
            value: number,
        });
    }
    panic!("unsupported fabrication witness value: {value}");
}

fn assert_source_authority(key: &str, sample: &TestSample, project: &CodeProject) {
    // Source-level anchors remain the authored contract. Rectangle and Slot
    // operations own their internal construction locks and are audited as
    // single source operations rather than expanded user-authored Fix rows.
    assert!(
        sample.source().matches("$.constraint.fixedPoint(").count() <= 2,
        "{key} uses only the minimal explicit datum anchors"
    );
    let canonical_project = project.to_canonical_json().expect("canonical project");
    assert_eq!(
        CodeProject::from_json(&canonical_project).expect("restored project"),
        *project,
        "{key} project round-trip"
    );

    let compiled = project
        .managed
        .compiled
        .as_deref()
        .expect("bundled sample compiler authority");
    compiled
        .validate_input_source(sample.source())
        .unwrap_or_else(|error| panic!("{key} source authority: {error}"));
    assert_eq!(compiled.normalized_source, sample.source(), "{key}");
    assert_eq!(
        serde_json::to_string(&compiled.ir).expect("canonical IR encoding"),
        compiled.canonical_ir_json,
        "{key} canonical IR round-trip"
    );
}

fn assert_group_ownership(key: &str, sample: &TestSample, project: &CodeProject) {
    let compiled = project
        .managed
        .compiled
        .as_deref()
        .expect("bundled sample compiler authority");
    let declarations = compiled
        .artifact
        .declarations
        .iter()
        .map(|declaration| declaration.declaration.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        compiled
            .artifact
            .groups
            .iter()
            .map(|group| group.name.as_str())
            .collect::<Vec<_>>(),
        sample.manifest()["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| group.as_str().unwrap())
            .collect::<Vec<_>>(),
        "{key} ordered functional groups"
    );
    if let Some(live) = sample.catalog_sample() {
        assert_eq!(
            serde_json::json!(live.functional_groups),
            sample.manifest()["groups"],
            "{key} generated registry preserves the declared groups"
        );
    }
    let mut ownership = BTreeMap::<&str, usize>::new();
    for group in &compiled.artifact.groups {
        assert!(!group.declarations.is_empty(), "{key} group {}", group.name);
        for member in &group.declarations {
            assert!(member.path.is_empty(), "{key} group {} is flat", group.name);
            assert!(declarations.contains(member.declaration.as_str()), "{key}");
            *ownership.entry(member.declaration.as_str()).or_default() += 1;
        }
    }
    for declaration in declarations {
        assert_eq!(ownership.get(declaration), Some(&1), "{key} {declaration}");
    }
}

fn assert_external_provenance(key: &str, sample: &TestSample) {
    let manifest = sample.manifest();
    let provenance = manifest["provenance"]
        .as_array()
        .expect("sample provenance");
    if let Some(live) = sample.catalog_sample() {
        let projected = live
            .provenance
            .iter()
            .map(|record| {
                serde_json::json!({
                    "relationship": record.relationship,
                    "name": record.name,
                    "url": record.url,
                    "revision": record.revision,
                    "path": record.path,
                    "licence": record.licence,
                    "scope": record.scope,
                    "notice_required": record.notice_required,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            &projected, provenance,
            "{key} generated registry preserves every provenance field"
        );
    }
    assert!(!provenance.is_empty(), "{key} external provenance");
    let notice = sample.notice().expect("external sample notice");
    assert!(notice.contains("SPDX-License-Identifier: GPL-3.0-or-later"));
    assert!(notice.contains("manufacturing guarantee"), "{key}");
    for provenance in provenance {
        let revision = provenance["revision"]
            .as_str()
            .expect("immutable external revision");
        let path = provenance["path"].as_str().expect("exact external path");
        assert_eq!(revision.len(), 40, "{key}");
        assert!(
            revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{key}"
        );
        assert_eq!(provenance["licence"], "GPL-3.0-only", "{key}");
        assert_eq!(provenance["notice_required"], true, "{key}");
        assert!(notice.contains(revision), "{key}");
        assert!(notice.contains(path), "{key}");
    }
}

fn assert_editable_and_solved(key: &str, sample: &TestSample, project: &CodeProject, seed: u128) {
    let witnesses: serde_json::Value =
        serde_json::from_str(sample.witnesses()).expect("witness JSON");
    let edit = &witnesses["representative_edit"];
    let declaration = edit["declaration"].as_str().expect("witness declaration");
    let path = SemanticOutputPath(
        edit["path"]
            .as_array()
            .expect("witness path")
            .iter()
            .map(|field| ManagedPathSegment::Field(field.as_str().expect("field path").to_owned()))
            .collect(),
    );
    let generated = generated(project);
    let materialized = materialize_code_project_cold(
        project,
        &generated,
        IntentSessionId::from_raw(seed),
        DocumentId(PersistentId::from_u128(seed)),
        1.0,
    )
    .unwrap_or_else(|error| panic!("{key} cold materialization failed: {error}"));
    let controls =
        managed_control_manifest(project, &materialized.expansion).expect("managed controls");
    let control = controls
        .editable()
        .find(|control| control.source.declaration.0 == declaration && control.source.path == path)
        .expect("representative edit addresses one editable source leaf");
    let mutation = prepare_managed_control_mutation(
        project,
        &materialized.expansion,
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: control.token().expect("editable token").clone(),
            value: witness_value(&edit["replacement"]),
        }]),
    )
    .expect("representative managed mutation");
    assert!(
        matches!(mutation, ManagedSketchMutation::SetValues { ref values } if values.len() == 1)
    );

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted fabrication authority");
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
    assert!(
        state
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite()),
        "{key}"
    );
    let rank = state.diagnostics().rank.expect("rank diagnostics");
    let mobility = state.diagnostics().mobility.expect("mobility diagnostics");
    assert_eq!(rank.numerical_right_nullity, Some(0), "{key} raw DOF");
    assert_eq!(mobility.equality_degrees_of_freedom, Some(0), "{key}");
    assert_eq!(
        mobility.bidirectional_bounded_degrees_of_freedom,
        Some(0),
        "{key}"
    );
}

fn validate(key: &str, seed: u128) {
    let sample = retained_sample::resolve(key);
    if let Some(live) = sample.catalog_sample() {
        assert_eq!(live.category, SampleCategory::ProductFabrication);
    }
    assert_eq!(
        serde_json::from_value::<SampleCategory>(sample.manifest()["category"].clone()).unwrap(),
        SampleCategory::ProductFabrication
    );
    let project = sample.project();
    assert_source_authority(key, &sample, &project);
    assert_group_ownership(key, &sample, &project);
    assert_external_provenance(key, &sample);
    assert_editable_and_solved(key, &sample, &project, seed);
}

#[test]
fn fabrication_wave_b_is_provenanced_grouped_editable_and_fully_constrained() {
    for (ordinal, key) in SAMPLES.into_iter().enumerate() {
        let sample = retained_sample::resolve(key);
        if let Some(live) = sample.catalog_sample() {
            assert_eq!(live.ordinal, catalog_contract::ordinal(key), "{key}");
        }
        // Retired Bondtech still executes every original assertion below using
        // its explicit archive. The loop index remains the original seed owner.
        validate(key, 0x92_70 + ordinal as u128);
    }
}
