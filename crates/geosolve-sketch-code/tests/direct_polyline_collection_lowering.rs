// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_code::{
    CodeProject, CodeProjectFile, CollectionRule, ExpandedSemanticTarget, FeatureKind,
    GeneratedMemberAddress, KeyedReconcileState, ManagedPathSegment, PatchModuleArtifact,
    PatchTemplateNode, ProjectKey, SKETCH_CODE_SDK_ABI, SemanticOutputPath, TemplateBinding,
    expand_code_project, parse_managed_source, required_generated_members,
};
use geosolve_sketch_intent::{
    IntentPortKind, IntentSession, IntentSessionId, intent_content_digest,
};

const MODULE: &str = "./patches/mark-polyline-vertices.patch.ts";
const PATCH_SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later
import { definePatch, t } from "@geosolve/sketch-code";

export const markPolylineVertices = definePatch(
  { vertices: t.keyed(t.point()), radius: t.length() },
  (p, { vertices, radius }) => ({
    markers: p.each(
      vertices,
      (vertex) => p.circle(vertex, radius),
      { key: (vertex) => vertex.key },
    ),
  }),
);
"#;
const MANAGED_SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { markPolylineVertices } from "./patches/mark-polyline-vertices.patch.ts";

export default sketch(($) => {
  const wire = $.geometry.polyline("wire", {
    vertices: [
      { key: "start", position: [0, 0] },
      { key: "middle", position: [10, 0] },
      { key: "end", position: [20, 0] },
    ],
    closed: false,
  });
  const marked = $.use("marked", markPolylineVertices, {
    vertices: wire.vertices,
    radius: mm(1),
  });
  return $.outputs({
    vertices: wire.vertices,
    segments: wire.segments,
    markers: marked.markers,
  });
});
"#;

fn marker_artifact() -> PatchModuleArtifact {
    let mut artifact = PatchModuleArtifact {
        format: geosolve_sketch_code::PATCH_ARTIFACT_FORMAT.into(),
        sdk_abi: SKETCH_CODE_SDK_ABI.into(),
        module_specifier: MODULE.into(),
        export_name: "markPolylineVertices".into(),
        source_digest: intent_content_digest(PATCH_SOURCE.as_bytes()).to_string(),
        interface_digest: String::new(),
        inputs: BTreeMap::from([
            ("radius".into(), FeatureKind::Scalar),
            ("vertices".into(), FeatureKind::Collection),
        ]),
        outputs: BTreeMap::from([("markers".into(), FeatureKind::Collection)]),
        templates: vec![PatchTemplateNode {
            path: vec!["circle".into()],
            result_output: Some("circle".into()),
            declaration_family: "geometry.circle".into(),
            inputs: BTreeMap::from([
                (
                    "center".into(),
                    TemplateBinding::CollectionMember {
                        input: "vertices".into(),
                        path: SemanticOutputPath::default(),
                        expected_kind: FeatureKind::Point,
                    },
                ),
                (
                    "radius".into(),
                    TemplateBinding::Input {
                        name: "radius".into(),
                        path: SemanticOutputPath::default(),
                        expected_kind: FeatureKind::Scalar,
                    },
                ),
            ]),
            fields: BTreeMap::new(),
            outputs: BTreeMap::from([("circle".into(), FeatureKind::Curve)]),
        }],
        collections: vec![CollectionRule::Each {
            path: vec!["markers".into()],
            input: "vertices".into(),
            member_key_field: "key".into(),
            templates: vec![vec!["circle".into()]],
        }],
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
    artifact
}

fn project() -> CodeProject {
    let artifact = marker_artifact();
    let validated = artifact.clone().validate().unwrap();
    CodeProject {
        project: ProjectKey("direct-polyline-collection-regression".into()),
        managed: parse_managed_source(MANAGED_SOURCE).unwrap(),
        custom_files: BTreeMap::from([(
            "patches/mark-polyline-vertices.patch.ts".into(),
            CodeProjectFile {
                path: "patches/mark-polyline-vertices.patch.ts".into(),
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
fn direct_polyline_collection_roots_preserve_keys_types_and_drive_artifact_each() {
    let project = project();
    project.validate().unwrap();
    let desired = required_generated_members(&project).unwrap();
    for key in ["start", "middle", "end"] {
        assert!(desired.contains(&GeneratedMemberAddress::new(
            "marked",
            ["circle"],
            [key],
            ["circle"],
        )));
    }
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_3400)).unwrap();
    let expanded = expand_code_project(&project, &generated, intent.identity()).unwrap();

    assert_collection_ports(
        &expanded.semantic_outputs["vertices"].target,
        &["end", "middle", "start"],
        IntentPortKind::Point,
    );
    assert_collection_ports(
        &expanded.semantic_outputs["segments"].target,
        &["middle", "start"],
        IntentPortKind::CurveSpan,
    );
    assert_collection_ports(
        &expanded.semantic_outputs["markers"].target,
        &["end", "middle", "start"],
        IntentPortKind::Curve,
    );
    assert!(expanded.semantic_outputs.values().all(|output| {
        output.reference.expected_kind == FeatureKind::Collection
            && output.reference.output.0.iter().all(|segment| {
                matches!(
                    segment,
                    ManagedPathSegment::Field(_) | ManagedPathSegment::Member { .. }
                )
            })
    }));
}

fn assert_collection_ports(
    target: &ExpandedSemanticTarget,
    expected_keys: &[&str],
    expected_kind: IntentPortKind,
) {
    let ExpandedSemanticTarget::Collection { members } = target else {
        panic!("semantic root must remain a keyed collection")
    };
    assert_eq!(
        members.keys().map(String::as_str).collect::<Vec<_>>(),
        expected_keys
    );
    for member in members.values() {
        let ExpandedSemanticTarget::Port { port } = member.as_ref() else {
            panic!("collection member must preserve its exact native port type")
        };
        assert_eq!(port.kind, expected_kind);
    }
}
