// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_code::{
    CodeProject, CodeProjectFile, ExpandedSemanticTarget, FeatureKind, KeyedReconcileState,
    PatchModuleArtifact, PatchTemplateNode, ProjectKey, SKETCH_CODE_SDK_ABI, SemanticOutputPath,
    TemplateBinding, expand_code_project, parse_managed_source, required_generated_members,
};
use geosolve_sketch_intent::{
    IntentPortKind, IntentSession, IntentSessionId, intent_content_digest,
};

const MODULE: &str = "./patches/aliased-profile.patch.ts";
const PATCH_SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later
import { definePatch, t } from "@geosolve/sketch-code";

export const aliasedProfile = definePatch(
  { width: t.length(), height: t.length(), cornerRadius: t.length() },
  (p, input) => {
    const rounded = p.roundedRectangle(input.width, input.height, input.cornerRadius);
    return { nested: { shape: rounded.profile } };
  },
);
"#;
const MANAGED_SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { aliasedProfile } from "./patches/aliased-profile.patch.ts";

export default sketch(($) => {
  const aliased = $.use("aliased", aliasedProfile, {
    width: mm(40),
    height: mm(20),
    cornerRadius: mm(3),
  });
  return $.outputs({ root: aliased, profile: aliased.nested.shape });
});
"#;

fn project() -> CodeProject {
    let mut artifact = PatchModuleArtifact {
        format: geosolve_sketch_code::PATCH_ARTIFACT_FORMAT.into(),
        sdk_abi: SKETCH_CODE_SDK_ABI.into(),
        module_specifier: MODULE.into(),
        export_name: "aliasedProfile".into(),
        source_digest: intent_content_digest(PATCH_SOURCE.as_bytes()).to_string(),
        interface_digest: String::new(),
        inputs: BTreeMap::from([
            ("cornerRadius".into(), FeatureKind::Scalar),
            ("height".into(), FeatureKind::Scalar),
            ("width".into(), FeatureKind::Scalar),
        ]),
        outputs: BTreeMap::from([("nested".into(), FeatureKind::Collection)]),
        templates: vec![PatchTemplateNode {
            path: vec!["nested".into(), "shape".into()],
            result_output: Some("profile".into()),
            declaration_family: "geometry.rectangle".into(),
            inputs: ["cornerRadius", "height", "width"]
                .into_iter()
                .map(|name| {
                    (
                        name.into(),
                        TemplateBinding::Input {
                            name: name.into(),
                            path: SemanticOutputPath::default(),
                            expected_kind: FeatureKind::Scalar,
                        },
                    )
                })
                .collect(),
            fields: BTreeMap::new(),
            outputs: BTreeMap::from([
                ("ne".into(), FeatureKind::Point),
                ("nw".into(), FeatureKind::Point),
                ("profile".into(), FeatureKind::Profile),
                ("se".into(), FeatureKind::Point),
                ("sw".into(), FeatureKind::Point),
            ]),
        }],
        collections: Vec::new(),
        edit_lenses: Vec::new(),
    };
    let interface = serde_json::json!({
        "export_name": artifact.export_name,
        "inputs": artifact.inputs,
        "module_specifier": artifact.module_specifier,
        "outputs": artifact.outputs,
        "sdk_abi": artifact.sdk_abi,
    });
    artifact.interface_digest =
        intent_content_digest(serde_json::to_string(&interface).unwrap().as_bytes()).to_string();
    let validated = artifact.clone().validate().unwrap();
    CodeProject {
        project: ProjectKey("multi-output-result-routing".into()),
        managed: parse_managed_source(MANAGED_SOURCE).unwrap(),
        custom_files: BTreeMap::from([(
            "patches/aliased-profile.patch.ts".into(),
            CodeProjectFile {
                path: "patches/aliased-profile.patch.ts".into(),
                source_digest: artifact.source_digest.clone(),
                contents: PATCH_SOURCE.into(),
                managed: false,
            },
        )]),
        artifacts: BTreeMap::from([(
            validated.digest().into(),
            serde_json::from_str(validated.canonical_json()).unwrap(),
        )]),
        lock: serde_json::json!({
            "format": "geosolve-lock-v1",
            "modules": {
                MODULE: {
                    "artifact": validated.digest(),
                    "source": artifact.source_digest,
                    "interface": artifact.interface_digest,
                    "sdk_abi": artifact.sdk_abi,
                },
            },
        }),
    }
}

#[test]
fn renamed_nested_multi_output_result_uses_compiler_selected_output_not_map_order() {
    let project = project();
    project.validate().unwrap();
    let desired = required_generated_members(&project).unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_3900)).unwrap();
    let expanded = expand_code_project(&project, &generated, intent.identity()).unwrap();

    let output = &expanded.semantic_outputs["profile"];
    assert_eq!(output.reference.expected_kind, FeatureKind::Profile);
    let ExpandedSemanticTarget::Port { port } = &output.target else {
        panic!("renamed nested result must retain the selected native Profile port")
    };
    assert_eq!(port.kind, IntentPortKind::Profile);

    let ExpandedSemanticTarget::Collection { members } = &expanded.semantic_outputs["root"].target
    else {
        panic!("single-Collection patch root must retain its nested result shape")
    };
    let ExpandedSemanticTarget::Collection { members } = members["nested"].as_ref() else {
        panic!("nested result namespace must remain a collection")
    };
    let ExpandedSemanticTarget::Port { port } = members["shape"].as_ref() else {
        panic!("nested selected result must remain its exact port")
    };
    assert_eq!(port.kind, IntentPortKind::Profile);
}
