// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use geosolve_sketch_intent::{intent_content_digest, intent_content_digest as digest};
use serde::{Deserialize, Serialize};

use crate::{
    CodeProject, CodeProjectFile, FeatureKind, GeneratedMemberAddress, PatchModuleArtifact,
    ProjectKey, parse_managed_source,
};

/// Stable bundled demonstration identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodeProjectDemoId {
    RoundedPolyline,
    TypedPanel,
    BracedFrame,
    MountingPlate,
}

impl CodeProjectDemoId {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::RoundedPolyline => "rounded-polyline",
            Self::TypedPanel => "typed-panel",
            Self::BracedFrame => "braced-frame",
            Self::MountingPlate => "mounting-plate",
        }
    }

    /// Concise user-facing explanation of the architectural behavior this
    /// demonstration exercises.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::RoundedPolyline => {
                "A keyed Polyline feeds one reusable patch that follows every current corner."
            }
            Self::TypedPanel => {
                "Named rectangle outputs feed a type-safe mapped record of selected Fillets."
            }
            Self::BracedFrame => {
                "Direct geometry feeds a reusable cross-brace and an ordinary native relation."
            }
            Self::MountingPlate => {
                "One pinned helper expands a rounded profile and stable keyed mounting holes."
            }
        }
    }
}

/// Complete offline fixture for one M84 structural-authoring demonstration.
#[derive(Clone, Debug)]
pub struct CodeProjectDemo {
    pub id: CodeProjectDemoId,
    pub title: &'static str,
    pub managed_source: &'static str,
    pub custom_files: BTreeMap<&'static str, &'static str>,
    pub artifacts: Vec<PatchModuleArtifact>,
    pub output_kinds: BTreeMap<&'static str, FeatureKind>,
    pub generated_members: Vec<GeneratedMemberAddress>,
}

impl CodeProjectDemo {
    /// Concise user-facing explanation shared by every catalog surface.
    #[must_use]
    pub const fn summary(&self) -> &'static str {
        self.id.summary()
    }

    /// Builds the bounded offline code project. Custom patch source is copied
    /// byte-for-byte; only its precompiled artifact is interpreted by Rust.
    ///
    /// # Panics
    ///
    /// Panics only when a source-controlled bundled fixture violates the same
    /// public parser or artifact contract tested by this crate.
    pub fn project(&self) -> CodeProject {
        let managed = parse_managed_source(self.managed_source)
            .expect("bundled managed demonstration source is valid");
        let custom_files = self
            .custom_files
            .iter()
            .map(|(path, contents)| {
                (
                    (*path).to_owned(),
                    CodeProjectFile {
                        path: (*path).to_owned(),
                        source_digest: intent_content_digest(contents.as_bytes()).to_string(),
                        contents: (*contents).to_owned(),
                        managed: false,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut artifacts = BTreeMap::new();
        let mut pins = BTreeMap::new();
        for artifact in &self.artifacts {
            let validated = artifact
                .clone()
                .validate()
                .expect("bundled patch artifact is valid");
            let value = serde_json::from_str(validated.canonical_json())
                .expect("canonical patch artifact is valid JSON");
            pins.insert(
                artifact.module_specifier.clone(),
                serde_json::json!({
                    "artifact": validated.digest(),
                    "source": artifact.source_digest,
                    "interface": artifact.interface_digest,
                    "sdk_abi": artifact.sdk_abi,
                }),
            );
            artifacts.insert(validated.digest().to_owned(), value);
        }
        CodeProject {
            project: ProjectKey(format!("m84-demo-{}", self.id.key())),
            managed,
            custom_files,
            artifacts,
            lock: serde_json::json!({
                "format": "geosolve-lock-v1",
                "modules": pins,
            }),
        }
    }

    /// Deterministic reviewed ledger row, independent of native wire IDs.
    #[must_use]
    pub fn ledger_row(&self) -> String {
        let project = self.project();
        let artifact_digests = project
            .artifacts
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let custom_digest = digest(
            &self
                .custom_files
                .values()
                .flat_map(|source| source.as_bytes())
                .copied()
                .collect::<Vec<_>>(),
        )
        .to_string();
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.id.key(),
            project.managed.source_digest,
            artifact_digests,
            custom_digest,
            project.managed.program.declarations.len(),
            self.generated_members.len(),
            self.output_kinds.len(),
        )
    }
}

/// Four curated projects which exercise adaptive generation, mapped outputs,
/// GUI/code/GUI references and reusable AI-authored patch source.
#[must_use]
pub fn bundled_code_project_demos() -> Vec<CodeProjectDemo> {
    vec![
        rounded_polyline_demo(),
        typed_panel_demo(),
        braced_frame_demo(),
        mounting_plate_demo(),
    ]
}

/// Semantic generated identities for one keyed Polyline. A directed span is
/// owned by its starting vertex key; an open Polyline Fillet is owned by each
/// interior vertex key.
#[must_use]
pub fn rounded_polyline_member_addresses(
    polyline: &str,
    fillet_invocation: &str,
    vertex_keys: &[&str],
    closed: bool,
) -> Vec<GeneratedMemberAddress> {
    let mut addresses = Vec::new();
    if vertex_keys.len() < 2 {
        return addresses;
    }
    let segment_count = if closed {
        vertex_keys.len()
    } else {
        vertex_keys.len() - 1
    };
    addresses.extend(vertex_keys.iter().map(|key| {
        GeneratedMemberAddress::new(polyline, ["polyline", "vertex"], [*key], ["point"])
    }));
    addresses.extend(vertex_keys[..segment_count].iter().map(|key| {
        GeneratedMemberAddress::new(polyline, ["polyline", "segment"], [*key], ["span"])
    }));
    let corners: &[&str] = if closed {
        vertex_keys
    } else {
        &vertex_keys[1..vertex_keys.len() - 1]
    };
    addresses.extend(
        corners
            .iter()
            .map(|key| GeneratedMemberAddress::new(fillet_invocation, ["fillet"], [*key], ["arc"])),
    );
    addresses
}

fn rounded_polyline_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/rounded-polyline.patch.ts");
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { roundEveryCorner } from "./patches/round-every-corner.patch.ts";

export default sketch(($) => {
  const path = $.geometry.polyline("path", {
    vertices: [
      { key: "start", position: [0, 0] },
      { key: "rise", position: [20, 0] },
      { key: "shoulder", position: [24, 12] },
      { key: "ridge", position: [40, 18] },
      { key: "fall", position: [55, 10] },
      { key: "end", position: [65, 10] },
    ],
    closed: false,
  });
  const rounded = $.use("rounded", roundEveryCorner, {
    corners: path.filletableCorners,
    radius: mm(0.4),
  });
  $.organize("Adaptive profile", [path, rounded]);
  return $.outputs({ path, rounded });
});
"#;
    let artifact = round_every_corner_artifact(PATCH);
    CodeProjectDemo {
        id: CodeProjectDemoId::RoundedPolyline,
        title: "Rounded polyline · dynamic corners",
        managed_source: SOURCE,
        custom_files: BTreeMap::from([("patches/round-every-corner.patch.ts", PATCH)]),
        artifacts: vec![artifact],
        output_kinds: BTreeMap::from([
            ("path", FeatureKind::Feature),
            ("rounded", FeatureKind::Collection),
        ]),
        generated_members: rounded_polyline_member_addresses(
            "path",
            "rounded",
            &["start", "rise", "shoulder", "ridge", "fall", "end"],
            false,
        ),
    }
}

fn typed_panel_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/typed-panel.patch.ts");
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { fillets } from "./patches/fillet-record.patch.ts";

export default sketch(($) => {
  const panel = $.geometry.rectangle("panel", {
    lowerLeft: [0, 0],
    upperRight: [80, 40],
  });
  const corners = $.use("cornerFillets", fillets, {
    corners: {
      lowerLeft: panel.corners.lowerLeft,
      upperRight: panel.corners.upperRight,
    },
    radius: mm(4),
  });
  return $.outputs({ panel, lowerLeft: corners.fillets.lowerLeft, upperRight: corners.fillets.upperRight });
});
"#;
    let artifact = fillet_record_artifact(PATCH);
    let generated_members = ["lowerLeft", "upperRight"]
        .into_iter()
        .map(|key| GeneratedMemberAddress::new("cornerFillets", ["fillet"], [key], ["arc"]))
        .collect();
    CodeProjectDemo {
        id: CodeProjectDemoId::TypedPanel,
        title: "Typed panel · keyed Fillets",
        managed_source: SOURCE,
        custom_files: BTreeMap::from([("patches/fillet-record.patch.ts", PATCH)]),
        artifacts: vec![artifact],
        output_kinds: BTreeMap::from([
            ("panel", FeatureKind::Feature),
            ("lowerLeft", FeatureKind::CurveSpan),
            ("upperRight", FeatureKind::CurveSpan),
        ]),
        generated_members,
    }
}

fn braced_frame_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/braced-frame.patch.ts");
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";
import { crossBrace } from "./patches/cross-brace.patch.ts";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const brace = $.use("brace", crossBrace, { frame: frame });
  // A diagonal of an axis-aligned frame cannot itself be horizontal. Keep the
  // downstream ordinary relation as an explicit, editable suppressed example
  // rather than publishing an invalid demonstration scene.
  const datum = $.constraint.horizontal("datum", { curve: brace.diagonals.rising, suppressed: true });
  $.organize("Frame", [frame, brace, datum]);
  return $.outputs({ frame, brace, rising: brace.diagonals.rising, datum });
});
"#;
    CodeProjectDemo {
        id: CodeProjectDemoId::BracedFrame,
        title: "Braced frame · GUI → code → GUI",
        managed_source: SOURCE,
        custom_files: BTreeMap::from([("patches/cross-brace.patch.ts", PATCH)]),
        artifacts: vec![cross_brace_artifact(PATCH)],
        output_kinds: BTreeMap::from([
            ("frame", FeatureKind::Feature),
            ("brace", FeatureKind::Collection),
            ("rising", FeatureKind::CurveSpan),
            ("datum", FeatureKind::Constraint),
        ]),
        generated_members: ["rising", "falling"]
            .into_iter()
            .map(|key| GeneratedMemberAddress::new("brace", ["diagonals", key], ["self"], ["span"]))
            .collect(),
    }
}

fn mounting_plate_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/mounting-plate.patch.ts");
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { mountingPlate } from "./patches/mounting-plate.patch.ts";

export default sketch(($) => {
  const plate = $.use("plate", mountingPlate, {
    width: mm(90),
    height: mm(55),
    cornerRadius: mm(7),
    holeRadius: mm(2.5),
  });
  $.organize("Mounting plate", [plate]);
  return $.outputs({ plate, profile: plate.profile, nw: plate.holes.nw, ne: plate.holes.ne, se: plate.holes.se, sw: plate.holes.sw });
});
"#;
    let generated_members =
        ["profile", "nw", "ne", "se", "sw"]
            .into_iter()
            .map(|output| GeneratedMemberAddress::new("plate", ["profile"], ["self"], [output]))
            .chain(["nw", "ne", "se", "sw"].into_iter().map(|key| {
                GeneratedMemberAddress::new("plate", ["holes", key], ["self"], ["circle"])
            }))
            .collect();
    CodeProjectDemo {
        id: CodeProjectDemoId::MountingPlate,
        title: "Mounting plate · reusable AI-authored module",
        managed_source: SOURCE,
        custom_files: BTreeMap::from([("patches/mounting-plate.patch.ts", PATCH)]),
        artifacts: vec![mounting_plate_artifact(PATCH)],
        output_kinds: BTreeMap::from([
            ("plate", FeatureKind::Feature),
            ("profile", FeatureKind::Profile),
            ("nw", FeatureKind::Curve),
            ("ne", FeatureKind::Curve),
            ("se", FeatureKind::Curve),
            ("sw", FeatureKind::Curve),
        ]),
        generated_members,
    }
}

fn round_every_corner_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/round-every-corner.artifact.json"),
    )
}

fn fillet_record_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/fillet-record.artifact.json"),
    )
}

fn cross_brace_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/cross-brace.artifact.json"),
    )
}

fn mounting_plate_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/mounting-plate.artifact.json"),
    )
}

fn compiled_typescript_artifact(source: &str, canonical_json: &str) -> PatchModuleArtifact {
    let canonical_json = canonical_json.trim_end_matches(['\r', '\n']);
    let validated = PatchModuleArtifact::from_canonical_json(canonical_json)
        .expect("checked-in TypeScript patch artifact is canonical and Rust-admissible");
    assert_eq!(
        validated.artifact().source_digest,
        digest(source.as_bytes()).to_string(),
        "checked-in TypeScript artifact source pin must match the bundled helper bytes"
    );
    validated.into_artifact()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{KeyedReconcileState, ManagedPathSegment};

    #[test]
    fn all_four_projects_are_offline_parseable_and_artifact_valid() {
        let demos = bundled_code_project_demos();
        assert_eq!(demos.len(), 4);
        for demo in demos {
            let project = demo.project();
            assert_eq!(project.managed.source, demo.managed_source);
            for (path, source) in &demo.custom_files {
                assert_eq!(project.custom_files[*path].contents, *source);
            }
            assert!(!project.artifacts.is_empty());
        }
    }

    #[test]
    fn adaptive_polyline_adds_only_crest_owned_members() {
        let six = ["start", "rise", "shoulder", "ridge", "fall", "end"];
        let seven = ["start", "rise", "shoulder", "ridge", "fall", "crest", "end"];
        let before = rounded_polyline_member_addresses("path", "rounded", &six, false);
        let after = rounded_polyline_member_addresses("path", "rounded", &seven, false);
        assert_eq!(before.len(), 15); // six vertices, five spans and four Fillets
        assert_eq!(after.len(), 18); // seven vertices, six spans and five Fillets

        let state = KeyedReconcileState::empty();
        let state = state.plan(before, &BTreeSet::new()).unwrap().into_staged();
        let before_identities = state.active().clone();
        let plan = state.plan(after, &BTreeSet::new()).unwrap();
        assert_eq!(plan.created.len(), 3);
        assert_eq!(plan.retained.len(), 15);
        for retained in plan.retained {
            assert_eq!(retained.identity, before_identities[&retained.address]);
        }
    }

    #[test]
    fn typed_demo_outputs_are_named_semantic_paths_not_wire_ids() {
        let demo = typed_panel_demo();
        let project = demo.project();
        let lower_left = project
            .managed
            .program
            .outputs
            .iter()
            .find(|output| output.name == "lowerLeft")
            .unwrap();
        assert_eq!(lower_left.declaration.0, "cornerFillets");
        assert_eq!(
            lower_left.path.0,
            vec![
                ManagedPathSegment::Field("fillets".into()),
                ManagedPathSegment::Field("lowerLeft".into()),
            ]
        );
        assert_eq!(demo.output_kinds["lowerLeft"], FeatureKind::CurveSpan);
    }

    #[test]
    fn custom_patch_bytes_survive_managed_reparse_unchanged() {
        let demo = mounting_plate_demo();
        let project = demo.project();
        let before = project.custom_files.clone();
        let managed =
            parse_managed_source(&project.managed.source.replacen("mm(90)", "mm(100)", 1)).unwrap();
        let mut after = project;
        after.managed = managed;
        assert_eq!(after.custom_files, before);
    }
}
