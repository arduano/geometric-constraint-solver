// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    dead_code,
    reason = "shared fixture adapter is compiled by distinct integration targets"
)]

use geosolve_sketch_code::{
    BundledSampleSpec, CodeProject, CompiledManagedSource, ProjectKey, bundled_sample,
};

const RETAINED_KEY: &str = "bondtech-indx-link";
const SOURCE: &str = include_str!("../fixtures/retained-bondtech-indx-link/sketch.ts");
const COMPILED: &str = include_str!("../fixtures/retained-bondtech-indx-link/sketch.compiled.json");
const MANIFEST: &str = include_str!("../fixtures/retained-bondtech-indx-link/manifest.json");
const WITNESSES: &str = include_str!("../fixtures/retained-bondtech-indx-link/witnesses.json");
const NOTICE: &str = include_str!("../fixtures/retained-bondtech-indx-link/NOTICE.md");
const EDITS: &str = include_str!("../fixtures/retained-bondtech-indx-link/audit-edits.json");

#[derive(Debug)]
pub enum TestSample {
    Catalog(&'static BundledSampleSpec),
    RetainedBondtech,
}

/// A missing live key is never an implicit skipped test. Exactly one archived
/// input may replace its catalog route, and only after reviewed retirement.
pub fn resolve(key: &str) -> TestSample {
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("../../assets/bundled-sample-catalog.json"))
            .expect("reviewed sample catalog contract");
    assert_eq!(contract["schema"], 1);
    let expected = contract["samples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|sample| sample["key"] == key);
    if expected {
        return TestSample::Catalog(
            bundled_sample(key).expect("reviewed live sample must resolve"),
        );
    }
    assert_eq!(
        key, RETAINED_KEY,
        "no archived fixture exists for this dedicated sample"
    );
    assert!(
        contract["retired_keys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|retired| retired == key),
        "dedicated fixture requires explicit reviewed retirement"
    );
    assert!(
        bundled_sample(key).is_none(),
        "retired key must not resolve in production"
    );
    TestSample::RetainedBondtech
}

pub fn original_bondtech_project() -> CodeProject {
    let compiled = CompiledManagedSource::from_json(COMPILED)
        .expect("retained compiler envelope authenticates");
    compiled
        .validate_input_source(SOURCE)
        .expect("retained original source authenticates");
    assert_eq!(compiled.normalized_source, SOURCE);
    CodeProject::managed(
        ProjectKey(format!("geosolve-sample-{RETAINED_KEY}")),
        compiled,
    )
    .expect("retained original project uses the ordinary validating decoder")
}

impl TestSample {
    pub fn manifest(&self) -> serde_json::Value {
        serde_json::from_str(match self {
            Self::Catalog(sample) => sample.manifest_json(),
            Self::RetainedBondtech => MANIFEST,
        })
        .expect("sample manifest")
    }

    pub fn source(&self) -> &str {
        match self {
            Self::Catalog(sample) => sample.managed_source(),
            Self::RetainedBondtech => SOURCE,
        }
    }

    pub fn witnesses(&self) -> &str {
        match self {
            Self::Catalog(sample) => sample.witnesses_json(),
            Self::RetainedBondtech => WITNESSES,
        }
    }

    pub fn notice(&self) -> Option<&str> {
        match self {
            Self::Catalog(sample) => sample.notice(),
            Self::RetainedBondtech => Some(NOTICE),
        }
    }

    pub fn catalog_sample(&self) -> Option<&'static BundledSampleSpec> {
        match self {
            Self::Catalog(sample) => Some(sample),
            Self::RetainedBondtech => None,
        }
    }

    pub fn project(&self) -> CodeProject {
        match self {
            Self::Catalog(sample) => sample.project(),
            Self::RetainedBondtech => original_bondtech_project(),
        }
    }

    pub fn edits(&self) -> serde_json::Value {
        match self {
            Self::Catalog(sample) => {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../geosolve-sketch-code/assets/bundled-samples")
                    .join(sample.key)
                    .join("audit-edits.json");
                serde_json::from_str(&std::fs::read_to_string(path).expect("live audit edits"))
                    .expect("live audit edit JSON")
            }
            Self::RetainedBondtech => {
                serde_json::from_str(EDITS).expect("retained audit edit JSON")
            }
        }
    }
}
