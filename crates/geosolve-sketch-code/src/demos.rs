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
    AdaptiveLanterns,
    SuspensionBridge,
    CompassRose,
    NeonManifold,
}

impl CodeProjectDemoId {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::RoundedPolyline => "rounded-polyline",
            Self::TypedPanel => "typed-panel",
            Self::BracedFrame => "braced-frame",
            Self::MountingPlate => "mounting-plate",
            Self::AdaptiveLanterns => "adaptive-lanterns",
            Self::SuspensionBridge => "suspension-bridge",
            Self::CompassRose => "compass-rose",
            Self::NeonManifold => "neon-manifold",
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
            Self::AdaptiveLanterns => {
                "One changing Polyline drives keyed bulbs and corner Fillets through two derived collections."
            }
            Self::SuspensionBridge => {
                "A constrained native deck and towers feed a reusable typed cable-and-stay module."
            }
            Self::CompassRose => {
                "Four editable native spokes drive a generated diamond ring, keyed markers and ordinary axis relations."
            }
            Self::NeonManifold => {
                "Lexical native spans, ordinary axis relations and explicit multi-corner Fillet branches coexist in managed code."
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

/// Curated projects which exercise adaptive generation, mapped outputs,
/// lexical dependencies, patch composition, native relations and reusable
/// AI-authored patch source.
#[must_use]
pub fn bundled_code_project_demos() -> Vec<CodeProjectDemo> {
    vec![
        rounded_polyline_demo(),
        typed_panel_demo(),
        braced_frame_demo(),
        mounting_plate_demo(),
        adaptive_lanterns_demo(),
        suspension_bridge_demo(),
        compass_rose_demo(),
        neon_manifold_demo(),
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
    radius: mm(4),
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

fn adaptive_lanterns_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/adaptive-lanterns.patch.ts");
    const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { adaptiveLanterns } from "./patches/adaptive-lanterns.patch.ts";

export default sketch(($) => {
  const wire = $.geometry.polyline("wire", {
    vertices: [
      { key: "plug", position: [-55, 4] },
      { key: "amber", position: [-38, 16] },
      { key: "coral", position: [-20, 7] },
      { key: "gold", position: [0, 18] },
      { key: "mint", position: [21, 8] },
      { key: "violet", position: [39, 16] },
      { key: "tail", position: [56, 4] },
    ],
    closed: false,
  });
  const decorations = $.use("decorations", adaptiveLanterns, {
    vertices: wire.vertices,
    corners: wire.filletableCorners,
    bulbRadius: mm(2.6),
    bendRadius: mm(3.2),
  });
  $.organize("Adaptive lantern garland", [wire, decorations]);
  return $.outputs({ wire, bulbs: decorations.bulbs, fillets: decorations.fillets });
});
"#;
    let keys = ["plug", "amber", "coral", "gold", "mint", "violet", "tail"];
    let mut generated_members = rounded_polyline_member_addresses("wire", "unused", &keys, false);
    generated_members.retain(|address| address.invocation != "unused");
    generated_members.extend(
        keys.into_iter()
            .map(|key| GeneratedMemberAddress::new("decorations", ["circle"], [key], ["circle"])),
    );
    generated_members.extend(
        keys[1..keys.len() - 1]
            .iter()
            .map(|key| GeneratedMemberAddress::new("decorations", ["fillet"], [*key], ["arc"])),
    );
    CodeProjectDemo {
        id: CodeProjectDemoId::AdaptiveLanterns,
        title: "Lantern garland · adaptive decorations",
        managed_source: SOURCE,
        custom_files: BTreeMap::from([("patches/adaptive-lanterns.patch.ts", PATCH)]),
        artifacts: vec![adaptive_lanterns_artifact(PATCH)],
        output_kinds: BTreeMap::from([
            ("wire", FeatureKind::Feature),
            ("bulbs", FeatureKind::Collection),
            ("fillets", FeatureKind::Collection),
        ]),
        generated_members,
    }
}

fn suspension_bridge_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/bridge-cables.patch.ts");
    const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";
import { bridgeCables } from "./patches/bridge-cables.patch.ts";

export default sketch(($) => {
  const deckLeft = $.geometry.line("deckLeft", { start: [-65, 0], end: [-28, 0] });
  const deckCenter = $.geometry.line("deckCenter", { start: deckLeft.end, end: [28, 0] });
  const deckRight = $.geometry.line("deckRight", { start: deckCenter.end, end: [65, 0] });
  const leftTower = $.geometry.line("leftTower", { start: deckLeft.end, end: [-28, 38] });
  const rightTower = $.geometry.line("rightTower", { start: deckCenter.end, end: [28, 38] });
  const cables = $.use("cables", bridgeCables, {
    leftAbutment: deckLeft.start,
    leftBase: leftTower.start,
    leftPeak: leftTower.end,
    rightBase: rightTower.start,
    rightPeak: rightTower.end,
    rightAbutment: deckRight.end,
  });
  const deckLeftAxis = $.constraint.horizontal("deckLeftAxis", { curve: deckLeft });
  const deckCenterAxis = $.constraint.horizontal("deckCenterAxis", { curve: deckCenter });
  const deckRightAxis = $.constraint.horizontal("deckRightAxis", { curve: deckRight });
  const leftTowerAxis = $.constraint.vertical("leftTowerAxis", { curve: leftTower });
  const rightTowerAxis = $.constraint.vertical("rightTowerAxis", { curve: rightTower });
  $.organize("Suspension bridge", [deckLeft, deckCenter, deckRight, leftTower, rightTower, cables, deckLeftAxis, deckCenterAxis, deckRightAxis, leftTowerAxis, rightTowerAxis]);
  return $.outputs({ deckLeft, deckCenter, deckRight, leftTower, rightTower, cables, crown: cables.mainCable.crown, fallingStay: cables.stays.falling });
});
"#;
    let generated_members = [
        ["mainCable", "left"],
        ["mainCable", "crown"],
        ["mainCable", "right"],
        ["stays", "falling"],
        ["stays", "rising"],
    ]
    .into_iter()
    .map(|path| GeneratedMemberAddress::new("cables", path, ["self"], ["span"]))
    .collect();
    CodeProjectDemo {
        id: CodeProjectDemoId::SuspensionBridge,
        title: "Suspension bridge · typed structural graph",
        managed_source: SOURCE,
        custom_files: BTreeMap::from([("patches/bridge-cables.patch.ts", PATCH)]),
        artifacts: vec![bridge_cables_artifact(PATCH)],
        output_kinds: BTreeMap::from([
            ("deckLeft", FeatureKind::Feature),
            ("deckCenter", FeatureKind::Feature),
            ("deckRight", FeatureKind::Feature),
            ("leftTower", FeatureKind::Feature),
            ("rightTower", FeatureKind::Feature),
            ("cables", FeatureKind::Feature),
            ("crown", FeatureKind::CurveSpan),
            ("fallingStay", FeatureKind::CurveSpan),
        ]),
        generated_members,
    }
}

fn compass_rose_demo() -> CodeProjectDemo {
    const CORE_PATCH: &str = include_str!("../assets/patches/compass-core.patch.ts");
    const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { compassCore } from "./patches/compass-core.patch.ts";

export default sketch(($) => {
  const north = $.geometry.line("north", { start: [0, 0], end: [0, 34] });
  const east = $.geometry.line("east", { start: north.start, end: [34, 0] });
  const south = $.geometry.line("south", { start: north.start, end: [0, -34] });
  const west = $.geometry.line("west", { start: north.start, end: [-34, 0] });
  const core = $.use("core", compassCore, {
    north: north.end,
    east: east.end,
    south: south.end,
    west: west.end,
    markerRadius: mm(3),
  });
  const northAxis = $.constraint.vertical("northAxis", { curve: north.span });
  const eastAxis = $.constraint.horizontal("eastAxis", { curve: east.span });
  const southAxis = $.constraint.vertical("southAxis", { curve: south.span });
  const westAxis = $.constraint.horizontal("westAxis", { curve: west.span });
  $.organize("Composable compass rose", [north, east, south, west, core, northAxis, eastAxis, southAxis, westAxis]);
  return $.outputs({
    north,
    east,
    south,
    west,
    core,
    northMarker: core.markers.north.circle,
    eastMarker: core.markers.east.circle,
    southMarker: core.markers.south.circle,
    westMarker: core.markers.west.circle,
    northEast: core.ring.northEast.span,
    northAxis,
    eastAxis,
    southAxis,
    westAxis,
  });
});
"#;
    let mut generated_members = ["northEast", "southEast", "southWest", "northWest"]
        .into_iter()
        .map(|name| GeneratedMemberAddress::new("core", ["ring", name], ["self"], ["span"]))
        .collect::<Vec<_>>();
    generated_members.extend(
        ["north", "east", "south", "west"]
            .into_iter()
            .map(|key| GeneratedMemberAddress::new("core", ["markers", key], ["self"], ["circle"])),
    );
    CodeProjectDemo {
        id: CodeProjectDemoId::CompassRose,
        title: "Compass rose · generated semantic lattice",
        managed_source: SOURCE,
        custom_files: BTreeMap::from([("patches/compass-core.patch.ts", CORE_PATCH)]),
        artifacts: vec![compass_core_artifact(CORE_PATCH)],
        output_kinds: BTreeMap::from([
            ("core", FeatureKind::Feature),
            ("north", FeatureKind::Feature),
            ("east", FeatureKind::Feature),
            ("south", FeatureKind::Feature),
            ("west", FeatureKind::Feature),
            ("northMarker", FeatureKind::Curve),
            ("eastMarker", FeatureKind::Curve),
            ("southMarker", FeatureKind::Curve),
            ("westMarker", FeatureKind::Curve),
            ("northEast", FeatureKind::CurveSpan),
            ("northAxis", FeatureKind::Constraint),
            ("eastAxis", FeatureKind::Constraint),
            ("southAxis", FeatureKind::Constraint),
            ("westAxis", FeatureKind::Constraint),
        ]),
        generated_members,
    }
}

fn neon_manifold_demo() -> CodeProjectDemo {
    const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const feed = $.geometry.line("feed", {
    start: [-54, -20],
    end: [-20, -20],
  });
  const rise = $.geometry.line("rise", {
    start: feed.end,
    end: [-20, 12],
  });
  const bridge = $.geometry.line("bridge", {
    start: rise.end,
    end: [20, 12],
  });
  const stack = $.geometry.line("stack", {
    start: bridge.end,
    end: [20, 44],
  });
  const feedAxis = $.constraint.horizontal("feedAxis", { curve: feed.span });
  const riseAxis = $.constraint.vertical("riseAxis", { curve: rise.span });
  const bridgeAxis = $.constraint.horizontal("bridgeAxis", { curve: bridge.span });
  const stackAxis = $.constraint.vertical("stackAxis", { curve: stack.span });
  const bends = $.computed.filletSet("bends", {
    radius: mm(5),
    corners: [{
      parents: [{
        span: feed.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "end",
        periodicAnchor: null,
      }, {
        span: rise.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }, {
      parents: [{
        span: bridge.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "end",
        periodicAnchor: null,
      }, {
        span: stack.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
    suppressed: false,
  });
  $.organize("Neon manifold", [feed, rise, bridge, stack, bends, feedAxis, riseAxis, bridgeAxis, stackAxis]);
  return $.outputs({ feed, rise, bridge, stack, bends, feedAxis, riseAxis, bridgeAxis, stackAxis });
});
"#;
    CodeProjectDemo {
        id: CodeProjectDemoId::NeonManifold,
        title: "Neon manifold · explicit native bends",
        managed_source: SOURCE,
        custom_files: BTreeMap::new(),
        artifacts: Vec::new(),
        output_kinds: BTreeMap::from([
            ("feed", FeatureKind::Feature),
            ("rise", FeatureKind::Feature),
            ("bridge", FeatureKind::Feature),
            ("stack", FeatureKind::Feature),
            ("bends", FeatureKind::Feature),
            ("feedAxis", FeatureKind::Constraint),
            ("riseAxis", FeatureKind::Constraint),
            ("bridgeAxis", FeatureKind::Constraint),
            ("stackAxis", FeatureKind::Constraint),
        ]),
        generated_members: Vec::new(),
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

fn adaptive_lanterns_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/adaptive-lanterns.artifact.json"),
    )
}

fn bridge_cables_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/bridge-cables.artifact.json"),
    )
}

fn compass_core_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/compass-core.artifact.json"),
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
    fn all_bundled_projects_are_offline_parseable_and_artifact_valid() {
        let demos = bundled_code_project_demos();
        assert_eq!(demos.len(), 8);
        for demo in demos {
            let project = demo.project();
            assert_eq!(project.managed.source, demo.managed_source);
            for (path, source) in &demo.custom_files {
                assert_eq!(project.custom_files[*path].contents, *source);
            }
            if demo.id == CodeProjectDemoId::NeonManifold {
                assert!(project.artifacts.is_empty());
            } else {
                assert!(!project.artifacts.is_empty());
            }
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
