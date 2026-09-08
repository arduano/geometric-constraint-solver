// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_code::{
    KeyedReconcileState, ManagedPathSegment, ManagedStatement, ManagedValue, SampleCategory,
    bundled_sample, bundled_sample_catalog, expand_code_project, managed_control_manifest,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogContract {
    schema: u32,
    samples: Vec<CatalogEntry>,
    retired_keys: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogEntry {
    key: String,
    title: String,
    category: SampleCategory,
}

fn reviewed_catalog() -> CatalogContract {
    let reviewed: CatalogContract =
        serde_json::from_str(include_str!("../assets/bundled-sample-catalog.json"))
            .expect("reviewed sample catalog contract");
    assert_eq!(reviewed.schema, 1);
    assert!(!reviewed.samples.is_empty());
    reviewed
}

#[test]
fn canonical_registry_has_frozen_order_and_distribution() {
    let catalog = bundled_sample_catalog();
    let reviewed = reviewed_catalog();
    assert_eq!(catalog.len(), reviewed.samples.len());
    assert_eq!(
        catalog.iter().map(|sample| sample.key).collect::<Vec<_>>(),
        reviewed
            .samples
            .iter()
            .map(|sample| sample.key.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        catalog
            .iter()
            .map(|sample| sample.ordinal)
            .collect::<Vec<_>>(),
        (1..=reviewed.samples.len()).collect::<Vec<_>>()
    );

    let mut categories = BTreeMap::new();
    for (sample, expected) in catalog.iter().zip(&reviewed.samples) {
        assert_eq!(
            sample.title, expected.title,
            "{} reviewed title",
            sample.key
        );
        assert_eq!(
            sample.category, expected.category,
            "{} reviewed category",
            sample.key
        );
        *categories.entry(sample.category).or_insert(0) += 1;
        assert_eq!(
            bundled_sample(sample.key).map(|found| found.ordinal),
            Some(sample.ordinal)
        );
        assert_eq!(
            sample.expected.numerical_right_nullity(),
            sample.expected.equality_degrees_of_freedom()
        );
        assert!(
            sample.expected.bidirectional_bounded_degrees_of_freedom()
                <= sample.expected.equality_degrees_of_freedom()
        );
        assert!(!sample.manifest_json().is_empty());
        assert!(!sample.managed_source().is_empty());
        assert!(!sample.witnesses_json().is_empty());
    }
    let mut expected_categories = BTreeMap::new();
    for sample in &reviewed.samples {
        *expected_categories.entry(sample.category).or_insert(0) += 1;
    }
    assert_eq!(categories, expected_categories);
    assert!(bundled_sample("unknown-sample").is_none());
    for retired in &reviewed.retired_keys {
        assert!(
            bundled_sample(retired).is_none(),
            "retired catalog key `{retired}` must not resolve"
        );
    }
}

#[test]
fn every_compiler_envelope_reconstructs_and_authenticates_its_project() {
    for sample in bundled_sample_catalog() {
        assert_eq!(
            sample.project().managed.source,
            sample.managed_source(),
            "{}",
            sample.key
        );
    }
}

#[test]
fn dimension_presets_cover_gridfinity_standards_and_curated_manifold_intent() {
    let gridfinity = bundled_sample("gridfinity-bin-section").unwrap();
    assert!(gridfinity.dimension_presentation.all_authored);
    let project = gridfinity.project();
    let compiled = project.managed.compiled.as_deref().unwrap();
    let authored: Vec<_> = compiled
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration {
                symbol,
                builder_path,
                ..
            } if builder_path
                .first()
                .is_some_and(|family| family == "dimension") =>
            {
                Some(symbol)
            }
            _ => None,
        })
        .collect();
    assert_eq!(authored.len(), 20);
    assert!(authored.iter().any(|symbol| symbol.as_str() == "planWidth"));
    assert!(
        authored
            .iter()
            .any(|symbol| symbol.as_str() == "lowerChamferXLength")
    );

    let manifold = bundled_sample("pc-water-manifold").unwrap();
    assert!(!manifold.dimension_presentation.all_authored);
    assert_eq!(
        manifold.dimension_presentation.dimensions,
        [
            "plateWidth",
            "plateHeight",
            "reservoirWidth",
            "reservoirHeight",
            "upperOutletRadius",
            "screwNwOuterDiameter",
        ]
    );
}

#[test]
fn overview_parameter_selectors_resolve_authenticated_public_source_controls() {
    for sample in bundled_sample_catalog()
        .iter()
        .filter(|sample| !sample.dimension_presentation.parameters.is_empty())
    {
        let project = sample.project();
        let generated = KeyedReconcileState::empty()
            .plan(
                required_generated_members(&project).unwrap(),
                &BTreeSet::new(),
            )
            .unwrap()
            .into_staged();
        let intent = IntentSession::with_id(IntentSessionId::from_raw(0x97_01)).unwrap();
        let expansion = expand_code_project(&project, &generated, intent.identity()).unwrap();
        let controls = managed_control_manifest(&project, &expansion).unwrap();
        let mut values = Vec::new();
        for selector in sample.dimension_presentation.parameters {
            let matches: Vec<_> = controls
                .controls
                .iter()
                .filter(|control| {
                    control.source.declaration.0 == selector.declaration
                        && control.source.path.0.len() == selector.path.len()
                        && control.source.path.0.iter().zip(selector.path).all(|(actual, expected)| {
                                matches!(actual, ManagedPathSegment::Field(field) if field.as_str() == *expected)
                        })
                })
                .collect();
            assert_eq!(matches.len(), 1, "{}: {selector:?}", sample.key);
            let ManagedValue::Unit(value) = &matches[0].value else {
                panic!("overview parameter must retain its dimensional source value");
            };
            assert_eq!(value.unit, "mm");
            assert!(matches[0].token().is_some());
            values.push(value.value);
        }
        if sample.key == "pc-water-manifold" {
            assert_eq!(
                values,
                [12.0, 2.4],
                "full widths, never generated half offsets"
            );
        }
    }
}
