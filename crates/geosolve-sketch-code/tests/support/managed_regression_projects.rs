// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact pre-M92 projects retained only by focused behavior regressions.
//!
//! These fixtures are deliberately outside `src`, the packaged asset allowlist,
//! and the canonical bundled-sample registry. They are not a catalog and expose
//! no list or fallback API; tests must opt into one exact historical specimen.

use std::collections::BTreeMap;

use geosolve_sketch_code::{
    CodeProject, CodeProjectFile, CompiledManagedSource, PatchModuleArtifact, ProjectKey,
};
use geosolve_sketch_intent::intent_content_digest;

#[derive(Clone, Copy)]
struct PatchFixture {
    path: &'static str,
    source: &'static str,
    artifact: &'static str,
}

const ROUND_EVERY_CORNER: PatchFixture = PatchFixture {
    path: "patches/round-every-corner.patch.ts",
    source: include_str!(
        "../../../../packages/geosolve-sketch-code/examples/rounded-polyline.patch.ts"
    ),
    artifact: include_str!(
        "../../../../packages/geosolve-sketch-code/test/fixtures/round-every-corner.artifact.json"
    ),
};
const FILLET_RECORD: PatchFixture = PatchFixture {
    path: "patches/fillet-record.patch.ts",
    source: include_str!("../../../../packages/geosolve-sketch-code/examples/typed-panel.patch.ts"),
    artifact: include_str!(
        "../../../../packages/geosolve-sketch-code/test/fixtures/fillet-record.artifact.json"
    ),
};
const CROSS_BRACE: PatchFixture = PatchFixture {
    path: "patches/cross-brace.patch.ts",
    source: include_str!(
        "../../../../packages/geosolve-sketch-code/examples/braced-frame.patch.ts"
    ),
    artifact: include_str!(
        "../../../../packages/geosolve-sketch-code/test/fixtures/cross-brace.artifact.json"
    ),
};
const MOUNTING_PLATE: PatchFixture = PatchFixture {
    path: "patches/mounting-plate.patch.ts",
    source: include_str!(
        "../../../../packages/geosolve-sketch-code/examples/mounting-plate.patch.ts"
    ),
    artifact: include_str!(
        "../../../../packages/geosolve-sketch-code/test/fixtures/mounting-plate.artifact.json"
    ),
};
const COMPASS_CORE: PatchFixture = PatchFixture {
    path: "patches/compass-core.patch.ts",
    source: include_str!(
        "../../../../packages/geosolve-sketch-code/examples/compass-core.patch.ts"
    ),
    artifact: include_str!(
        "../../../../packages/geosolve-sketch-code/test/fixtures/compass-core.artifact.json"
    ),
};
const HARNESS_ROUTE: PatchFixture = PatchFixture {
    path: "patches/harness-route.patch.ts",
    source: include_str!(
        "../../../../packages/geosolve-sketch-code/examples/harness-route.patch.ts"
    ),
    artifact: include_str!(
        "../../../../packages/geosolve-sketch-code/test/fixtures/harness-route.artifact.json"
    ),
};

/// Loads one exact historical project for an explicitly named regression.
///
/// This deliberately returns `None` for every specimen not still named by a
/// focused test. It must never be called by production sample resolution.
pub fn managed_regression_project(key: &str) -> Option<CodeProject> {
    let (compiled, patches): (&str, &[PatchFixture]) = match key {
        "rounded-polyline" => (
            include_str!("../fixtures/managed-regression-projects/rounded-polyline.compiled.json"),
            &[ROUND_EVERY_CORNER],
        ),
        "typed-panel" => (
            include_str!("../fixtures/managed-regression-projects/typed-panel.compiled.json"),
            &[FILLET_RECORD],
        ),
        "braced-frame" => (
            include_str!("../fixtures/managed-regression-projects/braced-frame.compiled.json"),
            &[CROSS_BRACE],
        ),
        "mounting-plate" => (
            include_str!("../fixtures/managed-regression-projects/mounting-plate.compiled.json"),
            &[MOUNTING_PLATE],
        ),
        "compass-rose" => (
            include_str!("../fixtures/managed-regression-projects/compass-rose.compiled.json"),
            &[COMPASS_CORE],
        ),
        "robotic-routing-board" => (
            include_str!(
                "../fixtures/managed-regression-projects/robotic-routing-board.compiled.json"
            ),
            &[HARNESS_ROUTE],
        ),
        "gridfinity-1x1x3-section" => (
            include_str!(
                "../fixtures/managed-regression-projects/gridfinity-1x1x3-section.compiled.json"
            ),
            &[FILLET_RECORD],
        ),
        _ => return None,
    };

    let compiled = CompiledManagedSource::from_json(compiled)
        .unwrap_or_else(|error| panic!("managed regression project `{key}` is valid: {error}"));
    if patches.is_empty() {
        return Some(
            CodeProject::managed(ProjectKey(format!("geosolve-demo-{key}")), compiled)
                .unwrap_or_else(|error| panic!("managed regression project `{key}`: {error}")),
        );
    }

    let managed = compiled.into_managed_document().unwrap_or_else(|error| {
        panic!("managed regression project `{key}` projects to managed authority: {error}")
    });
    let mut custom_files = BTreeMap::new();
    let mut artifacts = BTreeMap::new();
    let mut pins = BTreeMap::new();
    for patch in patches {
        let source_digest = intent_content_digest(patch.source.as_bytes()).to_string();
        custom_files.insert(
            patch.path.to_owned(),
            CodeProjectFile {
                path: patch.path.to_owned(),
                source_digest,
                contents: patch.source.to_owned(),
                managed: false,
            },
        );
        let definition: PatchModuleArtifact = serde_json::from_str(patch.artifact)
            .unwrap_or_else(|error| panic!("managed regression patch is valid JSON: {error}"));
        let validated = definition
            .clone()
            .validate()
            .unwrap_or_else(|error| panic!("managed regression patch is valid: {error}"));
        let value = serde_json::from_str(validated.canonical_json())
            .expect("validated managed regression patch is canonical JSON");
        pins.insert(
            definition.module_specifier.clone(),
            serde_json::json!({
                "artifact": validated.digest(),
                "source": definition.source_digest,
                "interface": definition.interface_digest,
                "sdk_abi": definition.sdk_abi,
            }),
        );
        artifacts.insert(validated.digest().to_owned(), value);
    }
    let project = CodeProject {
        project: ProjectKey(format!("geosolve-demo-{key}")),
        managed,
        custom_files,
        artifacts,
        lock: serde_json::json!({
            "format": "geosolve-lock-v1",
            "modules": pins,
        }),
    };
    project
        .validate()
        .unwrap_or_else(|error| panic!("managed regression project `{key}` is valid: {error}"));
    Some(project)
}
