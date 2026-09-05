// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use geosolve_sketch_code::{SampleCategory, bundled_sample, bundled_sample_catalog};

const KEYS: [&str; 16] = [
    "theo-jansen-leg",
    "whitworth-quick-return",
    "peaucellier-linkage",
    "five-stage-scissor-lift",
    "pc-water-manifold",
    "cnc-dogbone-coupon",
    "vacuum-fixture-plate",
    "dust-shoe-clamp",
    "gridfinity-bin-section",
    "voron-panel",
    "micron-carriage",
    "bondtech-indx-link",
    "curves-contact-continuity-atlas",
    "fabrication-operations-atlas",
    "perforated-fixture-field",
    "robotic-harness-backplane",
];

#[test]
fn canonical_registry_has_frozen_order_and_distribution() {
    let catalog = bundled_sample_catalog();
    assert_eq!(catalog.len(), 16);
    assert_eq!(
        catalog.iter().map(|sample| sample.key).collect::<Vec<_>>(),
        KEYS
    );
    assert_eq!(
        catalog
            .iter()
            .map(|sample| sample.ordinal)
            .collect::<Vec<_>>(),
        (1..=16).collect::<Vec<_>>()
    );

    let mut categories = BTreeMap::new();
    for sample in catalog {
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
    assert_eq!(categories.get(&SampleCategory::Mechanism), Some(&4));
    assert_eq!(
        categories.get(&SampleCategory::ProductFabrication),
        Some(&8)
    );
    assert_eq!(categories.get(&SampleCategory::ReferenceLab), Some(&2));
    assert_eq!(categories.get(&SampleCategory::ScaleStudy), Some(&2));
    assert!(bundled_sample("unknown-sample").is_none());
    for retired in [
        "prusa-mini-interface",
        "nema-17-motor-interface",
        "hevort-datum-study",
        "twin-roller-bezier-cam",
        "rounded-polyline",
        "typed-panel",
        "braced-frame",
        "mounting-plate",
        "compass-rose",
        "drafting-compass",
        "scotch-yoke",
        "scissor-jack",
        "five-stage-scissor-tower",
    ] {
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
