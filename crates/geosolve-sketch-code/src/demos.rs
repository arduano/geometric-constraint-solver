// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use geosolve_sketch_intent::{intent_content_digest, intent_content_digest as digest};
use serde::{Deserialize, Serialize};

use crate::{
    CodeProject, CodeProjectFile, CompiledManagedSource, FeatureKind, GeneratedMemberAddress,
    PatchModuleArtifact, ProjectKey,
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
    PcWaterManifold,
    RoboticRoutingBoard,
    CncJoineryFitCoupon,
    GridfinityBinSection,
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
            Self::PcWaterManifold => "pc-water-manifold",
            Self::RoboticRoutingBoard => "robotic-routing-board",
            Self::CncJoineryFitCoupon => "cnc-joinery-fit-coupon",
            Self::GridfinityBinSection => "gridfinity-1x1x3-section",
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
            Self::PcWaterManifold => {
                "A fully constrained acrylic distro plate combines managed mechanical dimensions with adaptive AI-authored water-channel patches."
            }
            Self::RoboticRoutingBoard => {
                "Eight keyed cable routes dogfood shared/local source controls, adaptive clips and Fillets, dense rendering, and one movable service loop."
            }
            Self::CncJoineryFitCoupon => {
                "Loose, nominal and press-fit router coupons compose constrained mortises, shared cutter reliefs and selected handling Fillets."
            }
            Self::GridfinityBinSection => {
                "One keyed closed material contour captures a standards-informed 1×1×3U base, cavity, walls and stacking lips."
            }
        }
    }
}

/// Complete offline fixture for one bundled structural-authoring demonstration.
#[derive(Clone, Debug)]
pub struct CodeProjectDemo {
    pub id: CodeProjectDemoId,
    pub title: &'static str,
    pub managed_source: &'static str,
    /// Checked-in V3 IR/execution authority for `managed_source`.
    pub compiled_source: &'static str,
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
        let compiled =
            CompiledManagedSource::from_json(self.compiled_source).unwrap_or_else(|error| {
                panic!(
                    "bundled managed demonstration `{}` compiler authority is valid: {error:?}",
                    self.id.key()
                )
            });
        assert_eq!(
            compiled.normalized_source, self.managed_source,
            "bundled source must be the exact normalized source authenticated by its compiler envelope"
        );
        let managed = compiled
            .into_managed_document()
            .expect("bundled managed demonstration projects to equation-free Intent authority");
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
            project: ProjectKey(format!("geosolve-demo-{}", self.id.key())),
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
        pc_water_manifold_demo(),
        robotic_routing_board_demo(),
        cnc_joinery_fit_coupon_demo(),
        gridfinity_bin_section_demo(),
    ]
}

/// One user-visible, source-authoritative bundled sketch. The original twelve
/// curated projects may own reusable patch modules; the converted reference
/// corpus deliberately owns only its typed managed source and executed V3
/// envelope.
#[derive(Clone, Debug)]
pub struct BundledCodeProject {
    key: &'static str,
    title: &'static str,
    summary: &'static str,
    source: BundledCodeProjectSource,
}

#[derive(Clone, Debug)]
enum BundledCodeProjectSource {
    Curated(CodeProjectDemo),
    Managed {
        source: &'static str,
        compiled: &'static str,
    },
}

impl BundledCodeProject {
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.key
    }

    #[must_use]
    pub const fn title(&self) -> &'static str {
        self.title
    }

    #[must_use]
    pub const fn summary(&self) -> &'static str {
        self.summary
    }

    /// Builds the exact offline project authenticated by the checked-in
    /// TypeScript compiler envelope.
    ///
    /// # Panics
    ///
    /// Panics only when a source-controlled bundled envelope violates the
    /// same public compiler/project contract covered by the catalog tests.
    #[must_use]
    pub fn project(&self) -> CodeProject {
        match &self.source {
            BundledCodeProjectSource::Curated(demo) => demo.project(),
            BundledCodeProjectSource::Managed { source, compiled } => {
                let compiled = CompiledManagedSource::from_json(compiled).unwrap_or_else(|error| {
                    panic!("bundled managed sketch `{}` is valid: {error:?}", self.key)
                });
                assert_eq!(
                    compiled.normalized_source, *source,
                    "bundled managed source must match its compiler envelope"
                );
                CodeProject::managed(
                    ProjectKey(format!("geosolve-sample-{}", self.key)),
                    compiled,
                )
                .expect("bundled managed sketch forms a valid code project")
            }
        }
    }
}

macro_rules! managed_sample {
    ($key:literal, $title:literal, $summary:literal) => {
        BundledCodeProject {
            key: $key,
            title: $title,
            summary: $summary,
            source: BundledCodeProjectSource::Managed {
                source: include_str!(concat!("../assets/samples/", $key, ".sketch.ts")),
                compiled: include_str!(concat!("../assets/samples/", $key, ".compiled.json")),
            },
        }
    };
}

/// Complete source-authoritative sample inventory used by the workbench.
/// Direct-native constructors remain only as independently comparable
/// reference/oracle fixtures.
#[must_use]
pub fn bundled_code_projects() -> Vec<BundledCodeProject> {
    let mut projects = vec![
        managed_sample!(
            "drafting-compass",
            "Drafting compass · 1 DOF",
            "A constrained one-DOF drafting compass with editable typed source."
        ),
        managed_sample!(
            "bezier-continuity-bridge",
            "Bezier continuity bridge · 1 DOF",
            "A movable continuity mechanism backed by explicit curve/contact state."
        ),
        managed_sample!(
            "twin-roller-cam",
            "Twin-roller cam · 2 DOF",
            "Two independently mobile roller contacts around one driven cam."
        ),
        managed_sample!(
            "tangent-orbit",
            "Tangent orbit · 1 DOF",
            "A branch-explicit one-DOF tangent orbit."
        ),
        managed_sample!(
            "elliptic-trammel",
            "Elliptic trammel · 1 DOF",
            "A one-DOF trammel expressed with ordinary points, spans and dimensions."
        ),
        managed_sample!(
            "scotch-yoke",
            "Scotch yoke · 1 DOF",
            "A one-DOF crank and guided yoke with shared source-defined topology."
        ),
        managed_sample!(
            "rotating-constraint-square",
            "Rotating constraint square · 1 DOF",
            "A constrained square retaining one intended rotational degree of freedom."
        ),
        managed_sample!(
            "scissor-jack",
            "Scissor jack · 1 DOF",
            "A one-stage one-DOF scissor mechanism with deterministic source controls."
        ),
        managed_sample!(
            "five-stage-scissor-tower",
            "Five-stage scissor tower · 1 DOF",
            "A coupled five-stage one-DOF scissor tower."
        ),
        managed_sample!(
            "peaucellier-inversor",
            "Peaucellier inversor · 1 DOF",
            "An ordinary constrained Peaucellier linkage retaining its intended motion."
        ),
        managed_sample!(
            "four-bar-coupler",
            "Four-bar coupler · 1 DOF",
            "A one-DOF four-bar coupler with explicit length constraints."
        ),
        managed_sample!(
            "pantograph-linkage",
            "Pantograph linkage · 2 DOF",
            "A two-DOF pantograph expressed entirely through typed declarations."
        ),
        managed_sample!(
            "three-link-drawing-arm",
            "Three-link drawing arm · 3 DOF",
            "A three-link, three-DOF drawing arm with ordinary source-backed dimensions."
        ),
        managed_sample!(
            "constraint-dimension-sampler",
            "Constraint and dimension sampler",
            "A readable catalog of ordinary constraint and dimension declarations."
        ),
        managed_sample!(
            "auto-constraint-drafting",
            "Auto-constraint drafting playground",
            "A source-defined playground for retained drafting inference."
        ),
        managed_sample!(
            "retained-drafting-relations",
            "Retained drafting relations",
            "Examples of remembered drafting relations as ordinary source declarations."
        ),
        managed_sample!(
            "tangent-radial-normal",
            "Tangent and radial-normal construction",
            "Tangent and radial-normal relations with explicit contact state."
        ),
        managed_sample!(
            "contact-branch-specimen",
            "Contact branch specimen",
            "A branch-explicit line/circle contact specimen."
        ),
        managed_sample!(
            "angle-dimension-annotations",
            "Angle and dimension annotations",
            "Driving and reference annotations backed by typed dimensions."
        ),
        managed_sample!(
            "contextual-constraint-annotations",
            "Contextual constraint annotations",
            "The complete contextual constraint-glyph specimen in editable source."
        ),
        managed_sample!(
            "dense-constraint-junction",
            "Dense constraint junction",
            "A dense overlapping junction for picking and annotation review."
        ),
        managed_sample!(
            "construction-reference-geometry",
            "Construction and reference geometry",
            "Profile, construction and reference geometry sharing ordinary topology."
        ),
        managed_sample!(
            "curve-family-gallery",
            "Curve family gallery",
            "The full supported curve-family gallery in typed source."
        ),
        managed_sample!(
            "periodic-nurbs-specimen",
            "Periodic NURBS specimen",
            "A keyed periodic NURBS with explicit projective gauge."
        ),
        managed_sample!(
            "fillet-workshop",
            "2D Fillet playground",
            "A source-defined collection of ordinary Fillet input constructions."
        ),
    ];
    projects.extend(
        bundled_code_project_demos()
            .into_iter()
            .map(|demo| BundledCodeProject {
                key: demo.id.key(),
                title: demo.title,
                summary: demo.summary(),
                source: BundledCodeProjectSource::Curated(demo),
            }),
    );
    projects
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
    const SOURCE: &str = include_str!("../assets/demos/rounded-polyline.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/rounded-polyline.compiled.json");
    let artifact = round_every_corner_artifact(PATCH);
    CodeProjectDemo {
        id: CodeProjectDemoId::RoundedPolyline,
        title: "Rounded polyline · dynamic corners",
        managed_source: SOURCE,
        compiled_source: COMPILED,
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
    const SOURCE: &str = include_str!("../assets/demos/adaptive-lanterns.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/adaptive-lanterns.compiled.json");
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
        compiled_source: COMPILED,
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
    const SOURCE: &str = include_str!("../assets/demos/suspension-bridge.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/suspension-bridge.compiled.json");
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
        compiled_source: COMPILED,
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
    const SOURCE: &str = include_str!("../assets/demos/compass-rose.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/compass-rose.compiled.json");
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
        compiled_source: COMPILED,
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
    const SOURCE: &str = include_str!("../assets/demos/neon-manifold.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/neon-manifold.compiled.json");
    CodeProjectDemo {
        id: CodeProjectDemoId::NeonManifold,
        title: "Neon manifold · explicit native bends",
        managed_source: SOURCE,
        compiled_source: COMPILED,
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

fn pc_water_manifold_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/water-channel.patch.ts");
    const SOURCE: &str = include_str!("../assets/demos/pc-water-manifold.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/pc-water-manifold.compiled.json");
    let mut demo = CodeProjectDemo {
        id: CodeProjectDemoId::PcWaterManifold,
        title: "PC water manifold · fully constrained dogfood",
        managed_source: SOURCE,
        compiled_source: COMPILED,
        custom_files: BTreeMap::from([("patches/water-channel.patch.ts", PATCH)]),
        artifacts: vec![water_channel_artifact(PATCH)],
        output_kinds: BTreeMap::from([
            ("plate", FeatureKind::Feature),
            ("reservoir", FeatureKind::Feature),
            ("upperCenterline", FeatureKind::Feature),
            ("upperSeal", FeatureKind::Feature),
            ("upperChannelBends", FeatureKind::Collection),
            ("middleCenterline", FeatureKind::Feature),
            ("middleSeal", FeatureKind::Feature),
            ("middleChannelBends", FeatureKind::Collection),
            ("lowerCenterline", FeatureKind::Feature),
            ("lowerSeal", FeatureKind::Feature),
            ("lowerChannelBends", FeatureKind::Collection),
            ("screwNwOuter", FeatureKind::Feature),
            ("screwNwInner", FeatureKind::Feature),
            ("screwNeInner", FeatureKind::Feature),
            ("screwNeOuter", FeatureKind::Feature),
            ("screwSwOuter", FeatureKind::Feature),
            ("screwSwInner", FeatureKind::Feature),
            ("screwSeInner", FeatureKind::Feature),
            ("screwSeOuter", FeatureKind::Feature),
            ("plateAnchor", FeatureKind::Constraint),
        ]),
        generated_members: Vec::new(),
    };
    demo.generated_members = crate::required_generated_members(&demo.project())
        .expect("bundled manifold generated-member inventory is valid");
    demo
}

fn robotic_routing_board_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/harness-route.patch.ts");
    const SOURCE: &str = include_str!("../assets/demos/robotic-routing-board.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/robotic-routing-board.compiled.json");
    let mut demo = CodeProjectDemo {
        id: CodeProjectDemoId::RoboticRoutingBoard,
        title: "Robotic cable-harness routing board · adaptive dogfood",
        managed_source: SOURCE,
        compiled_source: COMPILED,
        custom_files: BTreeMap::from([("patches/harness-route.patch.ts", PATCH)]),
        artifacts: vec![harness_route_artifact(PATCH)],
        output_kinds: BTreeMap::from([
            ("board", FeatureKind::Feature),
            ("powerRoute", FeatureKind::Feature),
            ("powerClips", FeatureKind::Collection),
            ("powerFillets", FeatureKind::Collection),
            ("servoARoute", FeatureKind::Feature),
            ("servoAClips", FeatureKind::Collection),
            ("servoAFillets", FeatureKind::Collection),
            ("servoBRoute", FeatureKind::Feature),
            ("servoBClips", FeatureKind::Collection),
            ("servoBFillets", FeatureKind::Collection),
            ("sensorARoute", FeatureKind::Feature),
            ("sensorAClips", FeatureKind::Collection),
            ("sensorAFillets", FeatureKind::Collection),
            ("sensorBRoute", FeatureKind::Feature),
            ("sensorBClips", FeatureKind::Collection),
            ("sensorBFillets", FeatureKind::Collection),
            ("gripperRoute", FeatureKind::Feature),
            ("gripperClips", FeatureKind::Collection),
            ("gripperFillets", FeatureKind::Collection),
            ("visionRoute", FeatureKind::Feature),
            ("visionClips", FeatureKind::Collection),
            ("visionFillets", FeatureKind::Collection),
            ("serviceRoute", FeatureKind::Feature),
            ("serviceClips", FeatureKind::Collection),
            ("serviceFillets", FeatureKind::Collection),
        ]),
        generated_members: Vec::new(),
    };
    demo.generated_members = crate::required_generated_members(&demo.project())
        .expect("bundled routing-board generated-member inventory is valid");
    demo
}

fn cnc_joinery_fit_coupon_demo() -> CodeProjectDemo {
    const RELIEF_PATCH: &str = include_str!("../assets/patches/corner-reliefs.patch.ts");
    const FILLET_PATCH: &str = include_str!("../assets/patches/typed-panel.patch.ts");
    const SOURCE: &str = include_str!("../assets/demos/cnc-joinery-fit-coupon.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/cnc-joinery-fit-coupon.compiled.json");
    let mut demo = CodeProjectDemo {
        id: CodeProjectDemoId::CncJoineryFitCoupon,
        title: "CNC joinery fit coupon · keyed corner reliefs",
        managed_source: SOURCE,
        compiled_source: COMPILED,
        custom_files: BTreeMap::from([
            ("patches/corner-reliefs.patch.ts", RELIEF_PATCH),
            ("patches/fillet-record.patch.ts", FILLET_PATCH),
        ]),
        artifacts: vec![
            corner_reliefs_artifact(RELIEF_PATCH),
            fillet_record_artifact(FILLET_PATCH),
        ],
        output_kinds: BTreeMap::from([
            ("femaleBlank", FeatureKind::Feature),
            ("looseMortise", FeatureKind::Feature),
            ("nominalMortise", FeatureKind::Feature),
            ("pressMortise", FeatureKind::Feature),
            ("looseReliefs", FeatureKind::Collection),
            ("nominalReliefs", FeatureKind::Collection),
            ("pressReliefs", FeatureKind::Collection),
            ("looseTab", FeatureKind::Feature),
            ("nominalTab", FeatureKind::Feature),
            ("pressTab", FeatureKind::Feature),
            ("blankHandling", FeatureKind::Collection),
            ("looseTabHandling", FeatureKind::Collection),
            ("nominalTabHandling", FeatureKind::Collection),
            ("pressTabHandling", FeatureKind::Collection),
        ]),
        generated_members: Vec::new(),
    };
    demo.generated_members = crate::required_generated_members(&demo.project())
        .expect("bundled CNC joinery coupon generated-member inventory is valid");
    demo
}

fn gridfinity_bin_section_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/typed-panel.patch.ts");
    const SOURCE: &str = include_str!("../assets/demos/gridfinity-1x1x3-section.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/gridfinity-1x1x3-section.compiled.json");
    let mut demo = CodeProjectDemo {
        id: CodeProjectDemoId::GridfinityBinSection,
        title: "Gridfinity 1×1×3U section · keyed standard profile",
        managed_source: SOURCE,
        compiled_source: COMPILED,
        custom_files: BTreeMap::from([("patches/fillet-record.patch.ts", PATCH)]),
        artifacts: vec![fillet_record_artifact(PATCH)],
        output_kinds: BTreeMap::from([
            ("section", FeatureKind::Feature),
            ("floorFillets", FeatureKind::Collection),
            ("lipFillets", FeatureKind::Collection),
        ]),
        generated_members: Vec::new(),
    };
    demo.generated_members = crate::required_generated_members(&demo.project())
        .expect("bundled Gridfinity section generated-member inventory is valid");
    demo
}

fn typed_panel_demo() -> CodeProjectDemo {
    const PATCH: &str = include_str!("../assets/patches/typed-panel.patch.ts");
    const SOURCE: &str = include_str!("../assets/demos/typed-panel.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/typed-panel.compiled.json");
    let artifact = fillet_record_artifact(PATCH);
    let generated_members = ["lowerLeft", "upperRight"]
        .into_iter()
        .map(|key| GeneratedMemberAddress::new("cornerFillets", ["fillet"], [key], ["arc"]))
        .collect();
    CodeProjectDemo {
        id: CodeProjectDemoId::TypedPanel,
        title: "Typed panel · keyed Fillets",
        managed_source: SOURCE,
        compiled_source: COMPILED,
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
    const SOURCE: &str = include_str!("../assets/demos/braced-frame.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/braced-frame.compiled.json");
    CodeProjectDemo {
        id: CodeProjectDemoId::BracedFrame,
        title: "Braced frame · GUI → code → GUI",
        managed_source: SOURCE,
        compiled_source: COMPILED,
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
    const SOURCE: &str = include_str!("../assets/demos/mounting-plate.sketch.ts");
    const COMPILED: &str = include_str!("../assets/demos/mounting-plate.compiled.json");
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
        compiled_source: COMPILED,
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

fn water_channel_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/water-channel.artifact.json"),
    )
}

fn harness_route_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/harness-route.artifact.json"),
    )
}

fn corner_reliefs_artifact(source: &str) -> PatchModuleArtifact {
    compiled_typescript_artifact(
        source,
        include_str!("../assets/artifacts/corner-reliefs.artifact.json"),
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
        assert_eq!(demos.len(), 12);
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
                ManagedPathSegment::Member {
                    member: "lowerLeft".into(),
                },
            ]
        );
        assert_eq!(demo.output_kinds["lowerLeft"], FeatureKind::CurveSpan);
    }

    #[test]
    fn custom_patch_bytes_survive_compiled_project_round_trip_unchanged() {
        let demo = mounting_plate_demo();
        let project = demo.project();
        let before = project.custom_files.clone();
        let json = project.to_canonical_json().unwrap();
        let after = CodeProject::from_json(&json).unwrap();
        assert_eq!(after.custom_files, before);
    }
}
