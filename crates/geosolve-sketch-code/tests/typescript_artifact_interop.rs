// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_code::{
    ArtifactValidationError, CompiledManagedSource, FeatureKind, ManagedPathSegment,
    PatchModuleArtifact, PatchTemplateNode, SemanticOutputPath, TemplateArgument, TemplateBinding,
    bundled_sample, bundled_sample_catalog,
};
use geosolve_sketch_intent::intent_content_digest;

struct Fixture {
    module: &'static str,
    source: &'static str,
    canonical_json: &'static str,
}

#[test]
fn bundled_managed_programs_authenticate_input_and_publish_normalized_source() {
    for sample in bundled_sample_catalog() {
        let compiled =
            CompiledManagedSource::from_json(sample.compiled_source()).unwrap_or_else(|error| {
                panic!("{} compiler envelope was rejected: {error}", sample.key)
            });
        assert_eq!(
            compiled.input_source_digest,
            intent_content_digest(sample.managed_source().as_bytes()).to_string(),
            "{} must authenticate its exact canonical sample source",
            sample.key,
        );
        assert_eq!(
            compiled.normalized_source,
            sample.managed_source(),
            "{} must display and replay the compiler's canonical normalized source",
            sample.key,
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
    }
}

#[test]
fn canonical_water_patch_is_byte_identical_to_the_typescript_compiler_fixture() {
    assert!(bundled_sample("pc-water-manifold").is_some());
    let expected = include_str!(
        "../../../packages/geosolve-sketch-code/test/fixtures/water-channel.artifact.json"
    );
    let bundled = include_str!(
        "../assets/bundled-samples/pc-water-manifold/patches/water-channel.artifact.json"
    );
    assert_eq!(bundled, expected);
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

fn field_path(fields: &[&str]) -> SemanticOutputPath {
    SemanticOutputPath(
        fields
            .iter()
            .map(|field| ManagedPathSegment::Field((*field).into()))
            .collect(),
    )
}

fn argument<'a>(template: &'a PatchTemplateNode, name: &str) -> &'a TemplateArgument {
    let TemplateArgument::Object(arguments) = &template.arguments else {
        panic!("patch declaration arguments must retain their named object shape")
    };
    arguments
        .get(name)
        .unwrap_or_else(|| panic!("missing patch declaration argument `{name}`"))
}

#[test]
fn brace_paths_are_segmented_and_mounting_plate_has_profile_and_keyed_holes() {
    let cross = PatchModuleArtifact::from_canonical_json(fixtures()[2].canonical_json)
        .unwrap()
        .into_artifact();
    assert_eq!(cross.outputs["diagonals"], FeatureKind::Collection);
    assert_eq!(cross.templates.len(), 2);
    for (template, key) in cross.templates.iter().zip(["rising", "falling"]) {
        assert_eq!(
            template.result_path,
            Some(vec!["diagonals".into(), key.into()])
        );
        assert_eq!(template.result_output, None);
    }
    let TemplateArgument::Binding(TemplateBinding::Input {
        path,
        expected_kind,
        ..
    }) = argument(&cross.templates[0], "start")
    else {
        panic!("brace start must bind a semantic input path");
    };
    assert_eq!(*expected_kind, FeatureKind::Point);
    assert_eq!(
        path.0,
        vec![
            ManagedPathSegment::Field("corners".into()),
            ManagedPathSegment::Index(0),
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
        mounting.templates[0].result_output.as_ref(),
        Some(&field_path(&["profile"]))
    );
    for (template, key) in mounting.templates[1..].iter().zip(["nw", "ne", "se", "sw"]) {
        assert_eq!(template.path, [format!("hole-{key}")]);
        assert_eq!(template.result_path, Some(vec!["holes".into(), key.into()]));
        assert_eq!(
            template.result_output.as_ref(),
            Some(&field_path(&["curve"]))
        );
        let TemplateArgument::Binding(TemplateBinding::TemplateOutput {
            template: source_template,
            path,
            expected_kind,
        }) = argument(template, "center")
        else {
            panic!("mounting hole center must bind the rounded-profile template");
        };
        assert_eq!(source_template, &["profile"]);
        assert_eq!(path, &field_path(&["mounts", key]));
        assert_eq!(*expected_kind, FeatureKind::Point);
    }
}
