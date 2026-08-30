// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_code::{
    ArtifactValidationError, CodeProjectDemoId, FeatureKind, ManagedPathSegment,
    PatchModuleArtifact, TemplateBinding, bundled_code_project_demos,
};
use geosolve_sketch_intent::intent_content_digest;

struct Fixture {
    module: &'static str,
    source: &'static str,
    canonical_json: &'static str,
}

#[test]
fn new_bundled_managed_programs_are_exact_type_checked_typescript_fixtures() {
    let fixtures = [
        (
            CodeProjectDemoId::AdaptiveLanterns,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/adaptive-lanterns.managed.ts"
            ),
        ),
        (
            CodeProjectDemoId::SuspensionBridge,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/suspension-bridge.managed.ts"
            ),
        ),
        (
            CodeProjectDemoId::CompassRose,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/compass-rose.managed.ts"
            ),
        ),
        (
            CodeProjectDemoId::NeonManifold,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/neon-manifold.managed.ts"
            ),
        ),
        (
            CodeProjectDemoId::PcWaterManifold,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/pc-water-manifold.managed.ts"
            ),
        ),
        (
            CodeProjectDemoId::RoboticRoutingBoard,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/robotic-routing-board.managed.ts"
            ),
        ),
        (
            CodeProjectDemoId::CncJoineryFitCoupon,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/cnc-joinery-fit-coupon.managed.ts"
            ),
        ),
        (
            CodeProjectDemoId::GridfinityBinSection,
            include_str!(
                "../../../packages/geosolve-sketch-code/test/managed/gridfinity-1x1x3-section.managed.ts"
            ),
        ),
    ];
    let demos = bundled_code_project_demos();
    for (id, fixture) in fixtures {
        let demo = demos
            .iter()
            .find(|demo| demo.id == id)
            .unwrap_or_else(|| panic!("missing bundled demo `{}`", id.key()));
        assert_eq!(
            demo.managed_source.as_bytes(),
            fixture.as_bytes(),
            "{} must display the exact managed program checked by tsconfig.managed.json",
            id.key(),
        );
    }
}

fn fixtures() -> [Fixture; 10] {
    [
        Fixture {
            module: "./patches/round-every-corner.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/rounded-polyline.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/round-every-corner.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/fillet-record.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/typed-panel.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/fillet-record.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/cross-brace.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/braced-frame.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/cross-brace.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/mounting-plate.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/mounting-plate.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/mounting-plate.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/adaptive-lanterns.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/adaptive-lanterns.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/adaptive-lanterns.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/bridge-cables.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/bridge-cables.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/bridge-cables.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/compass-core.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/compass-core.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/compass-core.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/corner-reliefs.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/corner-reliefs.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/corner-reliefs.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/water-channel.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/water-channel.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/water-channel.artifact.json"
            ),
        },
        Fixture {
            module: "./patches/harness-route.patch.ts",
            source: include_str!(
                "../../../packages/geosolve-sketch-code/examples/harness-route.patch.ts"
            ),
            canonical_json: include_str!(
                "../../../packages/geosolve-sketch-code/test/fixtures/harness-route.artifact.json"
            ),
        },
    ]
}

#[test]
fn typescript_emitted_artifacts_are_byte_exact_rust_canonical_values() {
    let demos = bundled_code_project_demos();
    for fixture in fixtures() {
        let validated = PatchModuleArtifact::from_canonical_json(fixture.canonical_json)
            .unwrap_or_else(|error| panic!("{} was rejected: {error}", fixture.module));
        assert_eq!(validated.canonical_json(), fixture.canonical_json);
        assert_eq!(validated.artifact().module_specifier, fixture.module);
        assert_eq!(
            validated.artifact().source_digest,
            intent_content_digest(fixture.source.as_bytes()).to_string()
        );
        let interface = serde_json::json!({
            "export_name": validated.artifact().export_name,
            "inputs": validated.artifact().inputs,
            "module_specifier": validated.artifact().module_specifier,
            "outputs": validated.artifact().outputs,
            "sdk_abi": validated.artifact().sdk_abi,
        });
        assert_eq!(
            validated.artifact().interface_digest,
            intent_content_digest(serde_json::to_string(&interface).unwrap().as_bytes())
                .to_string(),
            "{} interface pin must authenticate the canonical TS compiler interface",
            fixture.module,
        );

        let bundled = demos
            .iter()
            .flat_map(|demo| &demo.artifacts)
            .find(|artifact| artifact.module_specifier == fixture.module)
            .unwrap_or_else(|| panic!("{} is absent from the bundled demos", fixture.module));
        assert_eq!(
            bundled.clone().validate().unwrap().canonical_json(),
            fixture.canonical_json
        );
    }
}

#[test]
fn harness_route_sources_and_artifacts_are_byte_identical_across_hosts() {
    let package_source =
        include_str!("../../../packages/geosolve-sketch-code/examples/harness-route.patch.ts");
    assert_eq!(
        package_source,
        include_str!(
            "../../../packages/geosolve-sketch-code/test/managed/patches/harness-route.patch.ts"
        )
    );
    assert_eq!(
        package_source,
        include_str!("../assets/patches/harness-route.patch.ts")
    );

    let package_artifact = include_str!(
        "../../../packages/geosolve-sketch-code/test/fixtures/harness-route.artifact.json"
    );
    assert_eq!(
        package_artifact,
        include_str!("../assets/artifacts/harness-route.artifact.json")
    );
}

#[test]
fn corner_relief_sources_and_artifacts_are_byte_identical_across_hosts() {
    let package_source =
        include_str!("../../../packages/geosolve-sketch-code/examples/corner-reliefs.patch.ts");
    assert_eq!(
        package_source,
        include_str!(
            "../../../packages/geosolve-sketch-code/test/managed/patches/corner-reliefs.patch.ts"
        )
    );
    assert_eq!(
        package_source,
        include_str!("../assets/patches/corner-reliefs.patch.ts")
    );

    let package_artifact = include_str!(
        "../../../packages/geosolve-sketch-code/test/fixtures/corner-reliefs.artifact.json"
    );
    assert_eq!(
        package_artifact,
        include_str!("../assets/artifacts/corner-reliefs.artifact.json")
    );
}

#[test]
fn rust_rejects_a_well_formed_but_forged_typescript_interface_pin() {
    let mut artifact: PatchModuleArtifact =
        serde_json::from_str(fixtures()[0].canonical_json).unwrap();
    artifact.interface_digest = "0".repeat(64);
    let canonical = serde_json::to_string(&artifact).unwrap();
    assert!(matches!(
        PatchModuleArtifact::from_canonical_json(&canonical),
        Err(ArtifactValidationError::InterfaceDigestMismatch { .. })
    ));
}

#[test]
fn brace_paths_are_segmented_and_mounting_plate_has_profile_and_keyed_holes() {
    let cross = PatchModuleArtifact::from_canonical_json(fixtures()[2].canonical_json)
        .unwrap()
        .into_artifact();
    assert_eq!(cross.outputs["diagonals"], FeatureKind::Collection);
    assert!(
        cross
            .templates
            .iter()
            .all(|template| template.result_output.as_deref() == Some("span"))
    );
    let TemplateBinding::Input {
        path,
        expected_kind,
        ..
    } = &cross.templates[0].inputs["start"]
    else {
        panic!("brace start must bind a semantic input path");
    };
    assert_eq!(*expected_kind, FeatureKind::FeatureCorner);
    assert_eq!(
        path.0,
        vec![
            ManagedPathSegment::Field("corners".into()),
            ManagedPathSegment::Field("lowerLeft".into()),
        ]
    );
    assert!(path.0.iter().all(|segment| match segment {
        ManagedPathSegment::Field(field) => !field.contains('.'),
        ManagedPathSegment::Index(_) | ManagedPathSegment::Member { .. } => true,
    }));

    let mounting = PatchModuleArtifact::from_canonical_json(fixtures()[3].canonical_json)
        .unwrap()
        .into_artifact();
    assert_eq!(mounting.outputs["profile"], FeatureKind::Profile);
    assert_eq!(mounting.outputs["holes"], FeatureKind::Collection);
    assert!(!mounting.inputs.contains_key("holeKeys"));
    assert!(mounting.collections.is_empty());
    assert_eq!(mounting.templates.len(), 5);
    assert_eq!(mounting.templates[0].path, ["profile"]);
    assert_eq!(
        mounting.templates[0].result_output.as_deref(),
        Some("profile")
    );
    for (template, key) in mounting.templates[1..].iter().zip(["nw", "ne", "se", "sw"]) {
        assert_eq!(template.path, ["holes", key]);
        assert_eq!(template.result_output.as_deref(), Some("circle"));
        let TemplateBinding::TemplateOutput { output, .. } = &template.inputs["center"] else {
            panic!("mounting hole center must bind the rounded-profile template");
        };
        assert_eq!(output, key);
    }
}
