// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use geosolve_sketch_code::{SampleCategory, bundled_sample, bundled_sample_catalog};

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
