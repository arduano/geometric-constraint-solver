// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::fmt::{self, Write as _};

use geosolve_constraint_editor::{EditorEffect, RetainedEditorCoordinator};
use geosolve_sketch::DesignPointId;

use super::persistence::WorkspaceSnapshot;
use super::scenario_fixtures::{
    ScenarioAction, ScenarioCandidate, ScenarioFixture, ScenarioObservation,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum VerificationPointId {
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15,
    P16,
    P17,
    P18,
    P19,
    P20,
    P21,
    P22,
    P23,
    P24,
    P25,
    P26,
    P27,
    P28,
}

impl VerificationPointId {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 28] = [
        Self::P1,
        Self::P2,
        Self::P3,
        Self::P4,
        Self::P5,
        Self::P6,
        Self::P7,
        Self::P8,
        Self::P9,
        Self::P10,
        Self::P11,
        Self::P12,
        Self::P13,
        Self::P14,
        Self::P15,
        Self::P16,
        Self::P17,
        Self::P18,
        Self::P19,
        Self::P20,
        Self::P21,
        Self::P22,
        Self::P23,
        Self::P24,
        Self::P25,
        Self::P26,
        Self::P27,
        Self::P28,
    ];

    pub(crate) const fn number(self) -> u8 {
        match self {
            Self::P1 => 1,
            Self::P2 => 2,
            Self::P3 => 3,
            Self::P4 => 4,
            Self::P5 => 5,
            Self::P6 => 6,
            Self::P7 => 7,
            Self::P8 => 8,
            Self::P9 => 9,
            Self::P10 => 10,
            Self::P11 => 11,
            Self::P12 => 12,
            Self::P13 => 13,
            Self::P14 => 14,
            Self::P15 => 15,
            Self::P16 => 16,
            Self::P17 => 17,
            Self::P18 => 18,
            Self::P19 => 19,
            Self::P20 => 20,
            Self::P21 => 21,
            Self::P22 => 22,
            Self::P23 => 23,
            Self::P24 => 24,
            Self::P25 => 25,
            Self::P26 => 26,
            Self::P27 => 27,
            Self::P28 => 28,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerificationPoint {
    id: VerificationPointId,
    objective: &'static str,
    human_judgment: &'static str,
}

impl VerificationPoint {
    pub(crate) const fn id(self) -> VerificationPointId {
        self.id
    }

    pub(crate) const fn objective(self) -> &'static str {
        self.objective
    }

    pub(crate) const fn human_judgment(self) -> &'static str {
        self.human_judgment
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ScenarioId {
    RoleProfileParticipation,
    ActivationDimensionMode,
    SharedParameterProposal,
    InvalidStaleParameterRecovery,
    ExternalLossExplicitRecovery,
    LifecycleEvidenceNaturalPass,
    AttributedCanvasError,
    GlobalCanvasError,
    AlphaParityCatalog,
    AlphaBranchRecovery,
    AdvancedAllFamilies,
    NurbsBranchTopology,
    AssociativeCompanionOperations,
    ProductionTopologyTrust,
    DraftingCompass,
    BezierBridge,
    TwinRollerCam,
    TangentOrbit,
    EllipticTrammel,
    ScotchYoke,
    RotatingSquare,
    ScissorJack,
    ScissorTower,
    PeaucellierLinkage,
}

impl ScenarioId {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 24] = [
        Self::RoleProfileParticipation,
        Self::ActivationDimensionMode,
        Self::SharedParameterProposal,
        Self::InvalidStaleParameterRecovery,
        Self::ExternalLossExplicitRecovery,
        Self::LifecycleEvidenceNaturalPass,
        Self::AttributedCanvasError,
        Self::GlobalCanvasError,
        Self::AlphaParityCatalog,
        Self::AlphaBranchRecovery,
        Self::AdvancedAllFamilies,
        Self::NurbsBranchTopology,
        Self::AssociativeCompanionOperations,
        Self::ProductionTopologyTrust,
        Self::DraftingCompass,
        Self::BezierBridge,
        Self::TwinRollerCam,
        Self::TangentOrbit,
        Self::EllipticTrammel,
        Self::ScotchYoke,
        Self::RotatingSquare,
        Self::ScissorJack,
        Self::ScissorTower,
        Self::PeaucellierLinkage,
    ];

    pub(crate) fn from_key(value: &str) -> Option<Self> {
        Some(match value {
            "role-profile-participation" => Self::RoleProfileParticipation,
            "activation-dimension-mode" => Self::ActivationDimensionMode,
            "shared-parameter-proposal" => Self::SharedParameterProposal,
            "invalid-stale-parameter-recovery" => Self::InvalidStaleParameterRecovery,
            "external-loss-explicit-recovery" => Self::ExternalLossExplicitRecovery,
            "lifecycle-evidence-natural-pass" => Self::LifecycleEvidenceNaturalPass,
            "attributed-canvas-error" => Self::AttributedCanvasError,
            "global-canvas-error" => Self::GlobalCanvasError,
            "alpha-parity-catalog" => Self::AlphaParityCatalog,
            "alpha-branch-recovery" => Self::AlphaBranchRecovery,
            "advanced-all-families" => Self::AdvancedAllFamilies,
            "nurbs-branch-topology" => Self::NurbsBranchTopology,
            "associative-companion-operations" => Self::AssociativeCompanionOperations,
            "production-topology-trust" => Self::ProductionTopologyTrust,
            "drafting-compass" => Self::DraftingCompass,
            "bezier-c1-bridge" => Self::BezierBridge,
            "twin-roller-bezier-cam" => Self::TwinRollerCam,
            "tangent-orbit" => Self::TangentOrbit,
            "elliptic-trammel" => Self::EllipticTrammel,
            "scotch-yoke" => Self::ScotchYoke,
            "rotating-square" => Self::RotatingSquare,
            "scissor-jack" => Self::ScissorJack,
            "five-stage-scissor-tower" => Self::ScissorTower,
            "peaucellier-linkage" => Self::PeaucellierLinkage,
            _ => return None,
        })
    }

    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::RoleProfileParticipation => "role-profile-participation",
            Self::ActivationDimensionMode => "activation-dimension-mode",
            Self::SharedParameterProposal => "shared-parameter-proposal",
            Self::InvalidStaleParameterRecovery => "invalid-stale-parameter-recovery",
            Self::ExternalLossExplicitRecovery => "external-loss-explicit-recovery",
            Self::LifecycleEvidenceNaturalPass => "lifecycle-evidence-natural-pass",
            Self::AttributedCanvasError => "attributed-canvas-error",
            Self::GlobalCanvasError => "global-canvas-error",
            Self::AlphaParityCatalog => "alpha-parity-catalog",
            Self::AlphaBranchRecovery => "alpha-branch-recovery",
            Self::AdvancedAllFamilies => "advanced-all-families",
            Self::NurbsBranchTopology => "nurbs-branch-topology",
            Self::AssociativeCompanionOperations => "associative-companion-operations",
            Self::ProductionTopologyTrust => "production-topology-trust",
            Self::DraftingCompass => "drafting-compass",
            Self::BezierBridge => "bezier-c1-bridge",
            Self::TwinRollerCam => "twin-roller-bezier-cam",
            Self::TangentOrbit => "tangent-orbit",
            Self::EllipticTrammel => "elliptic-trammel",
            Self::ScotchYoke => "scotch-yoke",
            Self::RotatingSquare => "rotating-square",
            Self::ScissorJack => "scissor-jack",
            Self::ScissorTower => "five-stage-scissor-tower",
            Self::PeaucellierLinkage => "peaucellier-linkage",
        }
    }

    pub(crate) fn definition(self) -> &'static ScenarioDefinition {
        SCENARIO_CATALOG
            .scenario(self)
            .expect("every typed scenario ID must have one catalog definition")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ScenarioGroupId {
    Root,
    M53HostSemantics,
    M55ActionParity,
    M61AdvancedTopology,
    GeometryIntent,
    HostOwnedInputs,
    TruthEvidence,
    ErrorAttribution,
    InteractiveMechanisms,
    CompactMechanisms,
    LinkageMechanisms,
    AdvancedCurvesTopology,
}

impl ScenarioGroupId {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Root => "uat-scenarios",
            Self::M53HostSemantics => "m53-host-semantics",
            Self::M55ActionParity => "m55-action-parity",
            Self::M61AdvancedTopology => "m61-advanced-topology",
            Self::GeometryIntent => "geometry-intent",
            Self::HostOwnedInputs => "host-owned-inputs",
            Self::TruthEvidence => "truth-evidence",
            Self::ErrorAttribution => "error-attribution",
            Self::InteractiveMechanisms => "interactive-mechanisms",
            Self::CompactMechanisms => "compact-mechanisms",
            Self::LinkageMechanisms => "linkage-mechanisms",
            Self::AdvancedCurvesTopology => "advanced-curves-topology",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScenarioStep {
    instruction: &'static str,
    action: Option<ScenarioAction>,
    expected: &'static str,
}

impl ScenarioStep {
    pub(crate) const fn instruction(self) -> &'static str {
        self.instruction
    }

    pub(crate) const fn action(self) -> Option<ScenarioAction> {
        self.action
    }

    pub(crate) const fn expected(self) -> &'static str {
        self.expected
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScenarioDefinition {
    id: ScenarioId,
    title: &'static str,
    description: &'static str,
    human_question: &'static str,
    fixture: ScenarioFixture,
    points: &'static [VerificationPointId],
    steps: &'static [ScenarioStep],
}

impl ScenarioDefinition {
    pub(crate) const fn id(self) -> ScenarioId {
        self.id
    }

    pub(crate) const fn title(self) -> &'static str {
        self.title
    }

    pub(crate) const fn description(self) -> &'static str {
        self.description
    }

    pub(crate) const fn human_question(self) -> &'static str {
        self.human_question
    }

    pub(crate) const fn fixture(self) -> ScenarioFixture {
        self.fixture
    }

    pub(crate) const fn points(self) -> &'static [VerificationPointId] {
        self.points
    }

    pub(crate) const fn steps(self) -> &'static [ScenarioStep] {
        self.steps
    }

    pub(crate) fn allows_action(self, action: ScenarioAction) -> bool {
        self.steps.iter().any(|step| step.action() == Some(action))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScenarioNode {
    Group(ScenarioGroup),
    Scenario(ScenarioId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScenarioGroup {
    id: ScenarioGroupId,
    title: &'static str,
    description: &'static str,
    children: &'static [ScenarioNode],
}

impl ScenarioGroup {
    pub(crate) const fn id(self) -> ScenarioGroupId {
        self.id
    }

    pub(crate) const fn title(self) -> &'static str {
        self.title
    }

    pub(crate) const fn description(self) -> &'static str {
        self.description
    }

    pub(crate) const fn children(self) -> &'static [ScenarioNode] {
        self.children
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScenarioCatalog {
    root: &'static ScenarioGroup,
    scenarios: &'static [ScenarioDefinition],
    verification_points: &'static [VerificationPoint],
}

impl ScenarioCatalog {
    pub(crate) const fn root(self) -> &'static ScenarioGroup {
        self.root
    }

    #[cfg(test)]
    pub(crate) const fn scenarios(self) -> &'static [ScenarioDefinition] {
        self.scenarios
    }

    #[cfg(test)]
    pub(crate) const fn verification_points(self) -> &'static [VerificationPoint] {
        self.verification_points
    }

    pub(crate) fn scenario(self, id: ScenarioId) -> Option<&'static ScenarioDefinition> {
        self.scenarios.iter().find(|scenario| scenario.id() == id)
    }

    pub(crate) fn verification_point(
        self,
        id: VerificationPointId,
    ) -> Option<&'static VerificationPoint> {
        self.verification_points
            .iter()
            .find(|point| point.id() == id)
    }
}

#[cfg(test)]
pub(crate) const ALL_SCENARIO_ACTIONS: [ScenarioAction; 32] = [
    ScenarioAction::RoleConstruction,
    ScenarioAction::RoleProfile,
    ScenarioAction::SuppressDimension,
    ScenarioAction::ReactivateDimension,
    ScenarioAction::ReferenceDimension,
    ScenarioAction::HostInactive,
    ScenarioAction::MissingDependency,
    ScenarioAction::ParameterValid,
    ScenarioAction::ParameterInvalidKind,
    ScenarioAction::ParameterStale,
    ScenarioAction::ParameterRecovery,
    ScenarioAction::ExternalMissing,
    ScenarioAction::ExternalStale,
    ScenarioAction::ExternalTopologyChange,
    ScenarioAction::ExternalExplicitRebind,
    ScenarioAction::ExternalFreshRecovery,
    ScenarioAction::LifecycleRejected,
    ScenarioAction::LifecycleRecovery,
    ScenarioAction::AttributedConflict,
    ScenarioAction::AttributedRecovery,
    ScenarioAction::AlphaFlipTangency,
    ScenarioAction::AlphaRejectedContact,
    ScenarioAction::AlphaRecovery,
    ScenarioAction::NurbsNextSpan,
    ScenarioAction::NurbsInsertKnot,
    ScenarioAction::OperationSplit,
    ScenarioAction::OperationMirror,
    ScenarioAction::OperationPattern,
    ScenarioAction::TopologyMakeIncomplete,
    ScenarioAction::TopologyRecover,
    ScenarioAction::TopologyCancel,
    ScenarioAction::CaptureEvidence,
];

const VERIFICATION_POINTS: [VerificationPoint; 28] = [
    VerificationPoint {
        id: VerificationPointId::P1,
        objective: "Role changes profile participation while geometry remains solver-active and accepted.",
        human_judgment: "Judge whether role, solver activity, and profile participation are clear.",
    },
    VerificationPoint {
        id: VerificationPointId::P2,
        objective: "Suppression/reactivation and driving/reference mode remain separate typed transitions.",
        human_judgment: "Judge whether the two concepts and their recovery are discoverable.",
    },
    VerificationPoint {
        id: VerificationPointId::P3,
        objective: "Host-inactive, unavailable-external, and dependency-loss reasons remain distinct without replacing accepted geometry.",
        human_judgment: "Judge whether each inactivity reason reads clearly and truthfully.",
    },
    VerificationPoint {
        id: VerificationPointId::P4,
        objective: "One shared typed input updates two bindings and accepted proposal provenance atomically.",
        human_judgment: "Judge whether shared host ownership and proposal provenance are clear.",
    },
    VerificationPoint {
        id: VerificationPointId::P5,
        objective: "Invalid-kind and stale input retain accepted evidence; complete typed recovery advances it.",
        human_judgment: "Judge whether retained intent, latest failure, and recovery are visually distinct.",
    },
    VerificationPoint {
        id: VerificationPointId::P6,
        objective: "Missing, stale, and topology-incompatible snapshots retain accepted external evidence without implicit repair.",
        human_judgment: "Judge whether stale and missing external-data ownership is trustworthy.",
    },
    VerificationPoint {
        id: VerificationPointId::P7,
        objective: "Declaration-only rebind retains accepted state; a fresh compatible snapshot advances it.",
        human_judgment: "Judge whether explicit ownership and the recovery sequence are clear.",
    },
    VerificationPoint {
        id: VerificationPointId::P8,
        objective: "Design, latest-attempt, and accepted identities remain separate while the scene stays accepted-only.",
        human_judgment: "Judge whether the three lifecycle views are visually distinguishable.",
    },
    VerificationPoint {
        id: VerificationPointId::P9,
        objective: "Fixed-provenance evidence preserves exact typed inputs and accepted/attempted evidence.",
        human_judgment: "Judge whether the captured evidence is useful; add a screenshot only for a visual finding.",
    },
    VerificationPoint {
        id: VerificationPointId::P10,
        objective: "The same typed state machine drives natural role, activity, parameter, and external-recovery transitions.",
        human_judgment: "Judge overall coherence and trust without relying on step-by-step instructions.",
    },
    VerificationPoint {
        id: VerificationPointId::P11,
        objective: "A rejected dimension conflict highlights its persistent owner and visible accepted operands without rendering attempted geometry.",
        human_judgment: "Judge whether the highlighted line, points, dimension, and focusable error markers make attribution immediately understandable.",
    },
    VerificationPoint {
        id: VerificationPointId::P12,
        objective: "An input failure with no defensible element attribution remains a global canvas error and clears only after valid recovery.",
        human_judgment: "Judge whether the global marker is noticeable and truthful without implying blame on an unrelated element.",
    },
    VerificationPoint {
        id: VerificationPointId::P13,
        objective: "The compact contextual intents preserve the alpha families and visibly include line/curve direction, equal-curvature and endpoint-continuity dispatch without a legacy application.",
        human_judgment: "Judge whether contextual labels, relation glyphs, explicit branch metadata, dimension annotations and persistent operands make the family catalog understandable.",
    },
    VerificationPoint {
        id: VerificationPointId::P14,
        objective: "Explicit tangent orientation changes and an impossible fixed contact retain typed branch state, accepted truth, and deterministic recovery.",
        human_judgment: "Judge whether branch choice, rejection attribution, and undo recovery are clear and trustworthy.",
    },
    VerificationPoint {
        id: VerificationPointId::P15,
        objective: "Every built-in advanced curve family is presented from one accepted public-domain scene with stable diagnostics and no browser-owned geometry.",
        human_judgment: "Judge whether the family variety, accepted state, and diagnostic provenance remain legible at a glance.",
    },
    VerificationPoint {
        id: VerificationPointId::P16,
        objective: "Periodic NURBS span/winding transitions and knot insertion occur only through explicit typed document edits and retain accepted geometry truth.",
        human_judgment: "Judge whether span, winding, knot topology, and the resulting accepted state are understandable and predictable.",
    },
    VerificationPoint {
        id: VerificationPointId::P17,
        objective: "Associative fillet/trim state and split, exact mirror, and bounded pattern proposals publish through the ordinary accepted transaction boundary.",
        human_judgment: "Judge whether operation ownership, retained source identity, and generated geometry read coherently.",
    },
    VerificationPoint {
        id: VerificationPointId::P18,
        objective: "Only independently complete topology is presented as consumable; open support and cancellation remain explicit non-profile outcomes and recovery is deterministic.",
        human_judgment: "Judge whether complete, incomplete, cancelled, and recovered topology states are immediately trustworthy.",
    },
    VerificationPoint {
        id: VerificationPointId::P19,
        objective: "The drafting compass reports one equality DOF and projected tip drag preserves its symmetric equal-arm construction.",
        human_judgment: "Judge whether the selected driver and symmetric motion make the reported DOF credible.",
    },
    VerificationPoint {
        id: VerificationPointId::P20,
        objective: "The two-Bezier C1 bridge exposes one bounded seam-sliding DOF without collapsing either endpoint jet.",
        human_judgment: "Judge whether the seam driver and associated curve motion are predictable.",
    },
    VerificationPoint {
        id: VerificationPointId::P21,
        objective: "The twin-roller cam reports two DOF and projects one selected roller along the fixed Bezier normal offset.",
        human_judgment: "Judge whether independent roller mobility and the selected driver are understandable.",
    },
    VerificationPoint {
        id: VerificationPointId::P22,
        objective: "The tangent satellite reports one orbital DOF and retains its explicit external-tangency branch around the full locus.",
        human_judgment: "Judge whether orbit motion remains smooth and branch-stable.",
    },
    VerificationPoint {
        id: VerificationPointId::P23,
        objective: "The trammel reports one DOF and produces its elliptic tracer path from ordinary bar and rail constraints.",
        human_judgment: "Judge whether dependent tracer motion convincingly reflects the underlying constraints.",
    },
    VerificationPoint {
        id: VerificationPointId::P24,
        objective: "The Scotch yoke reports one DOF and converts crank rotation into guided slider travel.",
        human_judgment: "Judge whether the crank-pin driver produces coherent mechanism motion.",
    },
    VerificationPoint {
        id: VerificationPointId::P25,
        objective: "The constraint-built square reports one rotational DOF around its fixed corner while retaining equal/perpendicular/parallel relations.",
        human_judgment: "Judge whether a non-privileged square behaves like a rigid rotating body.",
    },
    VerificationPoint {
        id: VerificationPointId::P26,
        objective: "The scissor jack reports one DOF and projected base-slider drag opens and closes its mirrored linkage.",
        human_judgment: "Judge whether the scissor motion and selected base driver are clear and stable.",
    },
    VerificationPoint {
        id: VerificationPointId::P27,
        objective: "The five-stage scissor tower reports one DOF and synchronously propagates base motion through all stages.",
        human_judgment: "Judge whether zoom/pan and projected drag make the large linked motion inspectable.",
    },
    VerificationPoint {
        id: VerificationPointId::P28,
        objective: "The Peaucellier linkage reports one DOF and maps circular input motion to straight output motion using ordinary bars.",
        human_judgment: "Judge whether the dependent output path makes the solver behavior trustworthy.",
    },
];

const ROLE_PROFILE_POINTS: [VerificationPointId; 1] = [VerificationPointId::P1];
const ACTIVATION_MODE_POINTS: [VerificationPointId; 2] =
    [VerificationPointId::P2, VerificationPointId::P3];
const SHARED_PARAMETER_POINTS: [VerificationPointId; 1] = [VerificationPointId::P4];
const PARAMETER_RECOVERY_POINTS: [VerificationPointId; 1] = [VerificationPointId::P5];
const EXTERNAL_RECOVERY_POINTS: [VerificationPointId; 2] =
    [VerificationPointId::P6, VerificationPointId::P7];
const LIFECYCLE_EVIDENCE_POINTS: [VerificationPointId; 3] = [
    VerificationPointId::P8,
    VerificationPointId::P9,
    VerificationPointId::P10,
];
const ATTRIBUTED_ERROR_POINTS: [VerificationPointId; 1] = [VerificationPointId::P11];
const GLOBAL_ERROR_POINTS: [VerificationPointId; 1] = [VerificationPointId::P12];
const ALPHA_PARITY_POINTS: [VerificationPointId; 1] = [VerificationPointId::P13];
const ALPHA_BRANCH_POINTS: [VerificationPointId; 1] = [VerificationPointId::P14];
const ADVANCED_ALL_FAMILIES_POINTS: [VerificationPointId; 1] = [VerificationPointId::P15];
const NURBS_BRANCH_POINTS: [VerificationPointId; 1] = [VerificationPointId::P16];
const OPERATIONS_POINTS: [VerificationPointId; 1] = [VerificationPointId::P17];
const PRODUCTION_TOPOLOGY_POINTS: [VerificationPointId; 1] = [VerificationPointId::P18];
const COMPASS_POINTS: [VerificationPointId; 1] = [VerificationPointId::P19];
const BRIDGE_POINTS: [VerificationPointId; 1] = [VerificationPointId::P20];
const CAM_POINTS: [VerificationPointId; 1] = [VerificationPointId::P21];
const ORBIT_POINTS: [VerificationPointId; 1] = [VerificationPointId::P22];
const TRAMMEL_POINTS: [VerificationPointId; 1] = [VerificationPointId::P23];
const SCOTCH_YOKE_POINTS: [VerificationPointId; 1] = [VerificationPointId::P24];
const ROTATING_SQUARE_POINTS: [VerificationPointId; 1] = [VerificationPointId::P25];
const SCISSOR_POINTS: [VerificationPointId; 1] = [VerificationPointId::P26];
const SCISSOR_TOWER_POINTS: [VerificationPointId; 1] = [VerificationPointId::P27];
const PEAUCELLIER_POINTS: [VerificationPointId; 1] = [VerificationPointId::P28];

const ROLE_PROFILE_STEPS: [ScenarioStep; 3] = [
    ScenarioStep {
        instruction: "Change the accepted role to Construction.",
        action: Some(ScenarioAction::RoleConstruction),
        expected: "Construction styling and profile exclusion change while the curve remains solver-active and accepted geometry does not move.",
    },
    ScenarioStep {
        instruction: "Inspect the accepted canvas, tree, effective activity, and accepted-profile evidence.",
        action: None,
        expected: "Every surface reports the same role/activity distinction without presenting construction as inactive.",
    },
    ScenarioStep {
        instruction: "Restore the accepted role to Profile.",
        action: Some(ScenarioAction::RoleProfile),
        expected: "Profile participation returns without an unexpected geometry or identity discontinuity.",
    },
];

const ACTIVATION_MODE_STEPS: [ScenarioStep; 7] = [
    ScenarioStep {
        instruction: "Suppress the selected driving dimension.",
        action: Some(ScenarioAction::SuppressDimension),
        expected: "The dimension reports user suppression independently of its driving/reference mode.",
    },
    ScenarioStep {
        instruction: "Reactivate the selected dimension.",
        action: Some(ScenarioAction::ReactivateDimension),
        expected: "The same dimension becomes active again without changing its mode.",
    },
    ScenarioStep {
        instruction: "Change the dimension to Reference.",
        action: Some(ScenarioAction::ReferenceDimension),
        expected: "Reference mode is presented separately from suppression and activation.",
    },
    ScenarioStep {
        instruction: "Apply a direct host-inactive override.",
        action: Some(ScenarioAction::HostInactive),
        expected: "The directly inactive dimension retains accepted geometry and names the host reason.",
    },
    ScenarioStep {
        instruction: "Make the supporting dependency unavailable.",
        action: Some(ScenarioAction::MissingDependency),
        expected: "The source curve reports unavailable external input and the dependent dimension reports derived dependency loss.",
    },
    ScenarioStep {
        instruction: "Inspect lifecycle, Problems, tree, and accepted canvas.",
        action: None,
        expected: "Direct inactivity and derived dependency loss remain distinguishable on every relevant surface.",
    },
    ScenarioStep {
        instruction: "Use Reset scenario before repeating this sequence.",
        action: None,
        expected: "Reset reconstructs the fixed initial role/activity fixture.",
    },
];

const SHARED_PARAMETER_STEPS: [ScenarioStep; 2] = [
    ScenarioStep {
        instruction: "Submit one valid shared length parameter.",
        action: Some(ScenarioAction::ParameterValid),
        expected: "Both driving bindings and the accepted output proposal advance atomically to the same input revision.",
    },
    ScenarioStep {
        instruction: "Inspect both bound dimensions, the accepted parameter stamp, and proposal provenance.",
        action: None,
        expected: "No intermediate partial update or ambiguous proposal owner is presented.",
    },
];

const PARAMETER_RECOVERY_STEPS: [ScenarioStep; 6] = [
    ScenarioStep {
        instruction: "Establish a valid accepted parameter update.",
        action: Some(ScenarioAction::ParameterValid),
        expected: "The accepted input and both bindings advance together.",
    },
    ScenarioStep {
        instruction: "Submit a value with the wrong parameter kind.",
        action: Some(ScenarioAction::ParameterInvalidKind),
        expected: "The latest attempt is rejected while accepted geometry, input stamps, and proposal evidence are retained.",
    },
    ScenarioStep {
        instruction: "Submit a stale parameter revision.",
        action: Some(ScenarioAction::ParameterStale),
        expected: "The stale attempt cannot replace the accepted input or accepted-only scene.",
    },
    ScenarioStep {
        instruction: "Compare design, latest-attempt, and accepted evidence.",
        action: None,
        expected: "Retained intent, exact failure, and accepted truth remain visibly separate.",
    },
    ScenarioStep {
        instruction: "Submit the complete valid recovery.",
        action: Some(ScenarioAction::ParameterRecovery),
        expected: "Only the recovery advances accepted identity and proposal evidence.",
    },
    ScenarioStep {
        instruction: "Inspect the recovered canvas and host cards.",
        action: None,
        expected: "No rejected or stale intermediate geometry appears as accepted.",
    },
];

const EXTERNAL_RECOVERY_STEPS: [ScenarioStep; 7] = [
    ScenarioStep {
        instruction: "Remove the required external snapshot.",
        action: Some(ScenarioAction::ExternalMissing),
        expected: "Missing input retains accepted external evidence and accepted geometry.",
    },
    ScenarioStep {
        instruction: "Submit a stale external snapshot.",
        action: Some(ScenarioAction::ExternalStale),
        expected: "The stale revision is rejected without replacing accepted evidence.",
    },
    ScenarioStep {
        instruction: "Submit a snapshot with changed topology.",
        action: Some(ScenarioAction::ExternalTopologyChange),
        expected: "Topology incompatibility is explicit and does not trigger an implicit topology repair.",
    },
    ScenarioStep {
        instruction: "Inspect attempted, retained, and accepted external stamps.",
        action: None,
        expected: "Missing, stale, and topology-incompatible states remain distinct and accepted truth stays stable.",
    },
    ScenarioStep {
        instruction: "Declare the explicit topology rebind.",
        action: Some(ScenarioAction::ExternalExplicitRebind),
        expected: "Declaration evidence changes while accepted geometry and accepted input remain retained.",
    },
    ScenarioStep {
        instruction: "Submit a fresh compatible snapshot.",
        action: Some(ScenarioAction::ExternalFreshRecovery),
        expected: "Only the fresh post-rebind snapshot advances accepted state.",
    },
    ScenarioStep {
        instruction: "Inspect the recovered external evidence and scene.",
        action: None,
        expected: "The complete recovery reads as explicit host ownership rather than automatic repair.",
    },
];

const LIFECYCLE_EVIDENCE_STEPS: [ScenarioStep; 6] = [
    ScenarioStep {
        instruction: "Submit a typed input that creates a rejected latest attempt.",
        action: Some(ScenarioAction::LifecycleRejected),
        expected: "The attempt advances independently of the retained design while accepted identity and accepted-only geometry remain retained.",
    },
    ScenarioStep {
        instruction: "Compare lifecycle, tree, Problems, host cards, and canvas.",
        action: None,
        expected: "The latest failure is visible without leaking rejected geometry into the accepted scene.",
    },
    ScenarioStep {
        instruction: "Submit the valid lifecycle recovery.",
        action: Some(ScenarioAction::LifecycleRecovery),
        expected: "The recovery alone advances the accepted identity and scene.",
    },
    ScenarioStep {
        instruction: "Capture fixed-provenance typed evidence.",
        action: Some(ScenarioAction::CaptureEvidence),
        expected: "The copyable evidence includes typed inputs and accepted/attempted evidence without canonical fixture documents.",
    },
    ScenarioStep {
        instruction: "Reset, then revisit one Geometry intent scenario and one Host-owned inputs recovery scenario.",
        action: None,
        expected: "The selector exposes the same deterministic scenarios without relying on this guide step by step.",
    },
    ScenarioStep {
        instruction: "Complete a natural-use pass and judge the whole state story.",
        action: None,
        expected: "Labels, accepted-only geometry, recovery, and evidence remain coherent without stale display or unexpected movement.",
    },
];

const ATTRIBUTED_ERROR_STEPS: [ScenarioStep; 4] = [
    ScenarioStep {
        instruction: "Change the accepted reference line length into an incompatible driving dimension.",
        action: Some(ScenarioAction::AttributedConflict),
        expected: "The attempt rejects while the accepted line stays visible; the dimension owner, line, and endpoint operands receive error highlights and focusable markers.",
    },
    ScenarioStep {
        instruction: "Hover each error icon, then reach the same icons by keyboard focus.",
        action: None,
        expected: "Every marker presents the same current problem without changing selection, history, or accepted geometry.",
    },
    ScenarioStep {
        instruction: "Compare the canvas attribution with the canonical Problems panel.",
        action: None,
        expected: "Both surfaces describe the same latest attempt while the canvas remains explicitly accepted-only.",
    },
    ScenarioStep {
        instruction: "Recover by returning the dimension to reference mode.",
        action: Some(ScenarioAction::AttributedRecovery),
        expected: "The recovery accepts and all current-error highlights and markers clear.",
    },
];

const GLOBAL_ERROR_STEPS: [ScenarioStep; 4] = [
    ScenarioStep {
        instruction: "Submit an angle value to the length parameter.",
        action: Some(ScenarioAction::ParameterInvalidKind),
        expected: "The failed input retains accepted geometry and produces one global top-right error marker without highlighting unrelated elements.",
    },
    ScenarioStep {
        instruction: "Hover and keyboard-focus the global marker, then compare it with Problems.",
        action: None,
        expected: "The global tooltip and Problems panel expose the same actionable input failure without claiming element attribution.",
    },
    ScenarioStep {
        instruction: "Confirm the accepted canvas and accepted parameter evidence did not advance.",
        action: None,
        expected: "Only latest-attempt metadata changes; no attempted geometry becomes authoritative.",
    },
    ScenarioStep {
        instruction: "Submit the valid length recovery.",
        action: Some(ScenarioAction::ParameterRecovery),
        expected: "The accepted state advances and the global marker clears.",
    },
];

const ALPHA_PARITY_STEPS: [ScenarioStep; 3] = [
    ScenarioStep {
        instruction: "Inspect the accepted contextual-constraint corpus on the canvas and in the sketch tree.",
        action: None,
        expected: "Coincidence/contact, equal length/radius/curvature, line/curve direction, midpoint/symmetry, tangency and endpoint continuity retain persistent selectable identities.",
    },
    ScenarioStep {
        instruction: "Compare each contextual intent with the selection-specific label, relation glyph kind and driving/reference dimension annotations.",
        action: None,
        expected: "The workbench presents Lock/Coincident/Equal/Tangent/Continuity intent while rendering the domain-owned underlying definitions and explicit branches.",
    },
    ScenarioStep {
        instruction: "Open diagnostics and capture typed evidence if useful.",
        action: None,
        expected: "Accepted provenance, rank, mobility and source identities agree with the visible accepted catalog.",
    },
];

const ALPHA_BRANCH_STEPS: [ScenarioStep; 4] = [
    ScenarioStep {
        instruction: "Flip the A3 tangent orientation through the typed contact branch action.",
        action: Some(ScenarioAction::AlphaFlipTangency),
        expected: "Both source contacts change one explicit orientation together; no coordinate heuristic chooses the branch.",
    },
    ScenarioStep {
        instruction: "Submit Coincident between two distinct fixed parallel spans.",
        action: Some(ScenarioAction::AlphaRejectedContact),
        expected: "The retained design records the impossible contact while the prior accepted scene and branch evidence remain authoritative.",
    },
    ScenarioStep {
        instruction: "Inspect the canvas markers, Problems, lifecycle identities, and stable diagnostics.",
        action: None,
        expected: "The current rejected source is attributed without presenting attempted geometry as accepted.",
    },
    ScenarioStep {
        instruction: "Undo the rejected contact and any incompatible branch candidate.",
        action: Some(ScenarioAction::AlphaRecovery),
        expected: "Rejected sources are removed, accepted state advances from fresh recovery attempts, and the current problem clears.",
    },
];

const ADVANCED_ALL_FAMILIES_STEPS: [ScenarioStep; 3] = [
    ScenarioStep {
        instruction: "Inspect the accepted all-family scene, sketch tree, and stable diagnostics.",
        action: None,
        expected: "Lines, circular and conic curves, Beziers, B-splines, and NURBS all come from one accepted public-domain fixture.",
    },
    ScenarioStep {
        instruction: "Compare curve identity, branch annotations, rank/mobility, and the production-topology card.",
        action: None,
        expected: "The workbench presents domain-owned geometry and diagnostics without inferring an equation, branch, or profile claim.",
    },
    ScenarioStep {
        instruction: "Reset the scenario and confirm the same accepted scene and evidence return.",
        action: None,
        expected: "The advanced gallery is deterministic and ordinary workspace state remains isolated.",
    },
];

const NURBS_BRANCH_STEPS: [ScenarioStep; 5] = [
    ScenarioStep {
        instruction: "Inspect the periodic NURBS contact's current semantic span, winding, side, and neighborhood.",
        action: None,
        expected: "The selected branch is explicit persistent state rather than a coordinate-derived choice.",
    },
    ScenarioStep {
        instruction: "Advance the contact to the next periodic span.",
        action: Some(ScenarioAction::NurbsNextSpan),
        expected: "The typed transition preserves the seam position while changing the explicit semantic span and winding as required.",
    },
    ScenarioStep {
        instruction: "Insert a knot into the same NURBS definition.",
        action: Some(ScenarioAction::NurbsInsertKnot),
        expected: "One accepted topology edit adds a semantic span while preserving finite rational geometry and existing identities.",
    },
    ScenarioStep {
        instruction: "Inspect accepted diagnostics and branch controls after both edits.",
        action: None,
        expected: "The accepted scene, branch metadata, audit, and rank describe the same current document.",
    },
    ScenarioStep {
        instruction: "Reset and repeat the span transition.",
        action: None,
        expected: "The explicit branch workflow starts from the same deterministic periodic state.",
    },
];

const OPERATIONS_STEPS: [ScenarioStep; 6] = [
    ScenarioStep {
        instruction: "Inspect the accepted generic fillet and parent trim views beside the independent exact line source.",
        action: None,
        expected: "Associative fillet state remains ordinary accepted sketch state while companion operations own no solver equation.",
    },
    ScenarioStep {
        instruction: "Split the independent line at its exact midpoint.",
        action: Some(ScenarioAction::OperationSplit),
        expected: "The immutable support identity is retained with deterministic adjacent visible intervals.",
    },
    ScenarioStep {
        instruction: "Mirror the exact source across the declared line axis.",
        action: Some(ScenarioAction::OperationMirror),
        expected: "A deterministic proposal publishes ordinary exact mirrored geometry through the retained session transaction boundary.",
    },
    ScenarioStep {
        instruction: "Create a bounded three-instance linear pattern.",
        action: Some(ScenarioAction::OperationPattern),
        expected: "The pattern publishes ordinary geometry and explicit identity disposition without a browser equation or B-rep entity.",
    },
    ScenarioStep {
        instruction: "Compare the tree, canvas, stable diagnostics, and production-topology card.",
        action: None,
        expected: "Every surface reflects the same newly accepted document and no proposal is shown as accepted before publication.",
    },
    ScenarioStep {
        instruction: "Reset before trying a different operation order.",
        action: None,
        expected: "Reset reconstructs the deterministic associative/companion fixture.",
    },
];

const PRODUCTION_TOPOLOGY_STEPS: [ScenarioStep; 6] = [
    ScenarioStep {
        instruction: "Inspect the initial production-topology card and its accepted-revision evidence.",
        action: None,
        expected: "Only complete independently checked wires and regions are labelled consumable.",
    },
    ScenarioStep {
        instruction: "Add one open eligible support to the accepted design.",
        action: Some(ScenarioAction::TopologyMakeIncomplete),
        expected: "Topology becomes skipped or truncated with typed issue evidence and no consumable production profile.",
    },
    ScenarioStep {
        instruction: "Cancel a fresh topology query before its first controlled checkpoint.",
        action: Some(ScenarioAction::TopologyCancel),
        expected: "Cancellation is recorded separately from topology incompleteness and changes no accepted sketch input or geometry.",
    },
    ScenarioStep {
        instruction: "Recover the deterministic complete topology fixture.",
        action: Some(ScenarioAction::TopologyRecover),
        expected: "A fresh complete profile returns with exact accepted provenance; no stale result is reused.",
    },
    ScenarioStep {
        instruction: "Compare complete, incomplete, cancellation, and recovered transcript entries.",
        action: None,
        expected: "The state story distinguishes solver acceptance, query control, and consumable topology.",
    },
    ScenarioStep {
        instruction: "Reset and verify the initial complete output is reproduced.",
        action: None,
        expected: "The production-topology evidence is deterministic and ordinary workspace persistence remains untouched.",
    },
];

const INTERACTIVE_MOTION_STEPS: [ScenarioStep; 4] = [
    ScenarioStep {
        instruction: "Confirm the selected point is the documented primary driver and inspect the nonzero mobility in accepted diagnostics.",
        action: None,
        expected: "The accepted scene starts with a selected persistent point and reports the scenario's documented equality and bounded mobility.",
    },
    ScenarioStep {
        instruction: "Drag the selected driver through several nearby targets on the canvas.",
        action: None,
        expected: "Projected motion follows only solver-permitted directions while connected geometry moves coherently and every hard residual remains valid.",
    },
    ScenarioStep {
        instruction: "Use wheel zoom, middle-drag pan, and Fit while inspecting the mechanism at different configurations.",
        action: None,
        expected: "Camera changes do not alter geometry, branch state, history, diagnostics, or the ordinary saved workspace.",
    },
    ScenarioStep {
        instruction: "Reset and repeat the first drag.",
        action: None,
        expected: "Reset restores the deterministic starting geometry, selected driver, accepted identity shape, and nonzero DOF.",
    },
];

const SCENARIOS: [ScenarioDefinition; 24] = [
    ScenarioDefinition {
        id: ScenarioId::RoleProfileParticipation,
        title: "Role & profile participation",
        description: "Compare construction role, solver activity, and default-profile participation on one accepted curve.",
        human_question: "Can a host user understand why construction geometry remains active but leaves the default profile?",
        fixture: ScenarioFixture::RoleActivity,
        points: &ROLE_PROFILE_POINTS,
        steps: &ROLE_PROFILE_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::ActivationDimensionMode,
        title: "Activation & dimension mode",
        description: "Exercise suppression, reference mode, direct host inactivity, and derived dependency loss without replacing accepted geometry.",
        human_question: "Are mode, direct inactivity, dependency loss, and their recovery discoverably different?",
        fixture: ScenarioFixture::RoleActivity,
        points: &ACTIVATION_MODE_POINTS,
        steps: &ACTIVATION_MODE_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::SharedParameterProposal,
        title: "Shared parameter & proposal",
        description: "Publish one shared host parameter to two driving bindings and one accepted output proposal.",
        human_question: "Is atomic shared ownership and accepted proposal provenance clear?",
        fixture: ScenarioFixture::ParameterProposal,
        points: &SHARED_PARAMETER_POINTS,
        steps: &SHARED_PARAMETER_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::InvalidStaleParameterRecovery,
        title: "Invalid/stale parameter recovery",
        description: "Compare a valid update, wrong-kind attempt, stale revision, retained accepted state, and complete recovery.",
        human_question: "Can a host user distinguish retained intent, the latest rejected input, accepted truth, and recovery?",
        fixture: ScenarioFixture::ParameterProposal,
        points: &PARAMETER_RECOVERY_POINTS,
        steps: &PARAMETER_RECOVERY_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::ExternalLossExplicitRecovery,
        title: "External loss & explicit recovery",
        description: "Exercise missing, stale, and topology-incompatible external data before explicit rebind and fresh recovery.",
        human_question: "Does the workflow communicate stale-data ownership and prohibit the impression of implicit repair?",
        fixture: ScenarioFixture::ExternalRebind,
        points: &EXTERNAL_RECOVERY_POINTS,
        steps: &EXTERNAL_RECOVERY_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::LifecycleEvidenceNaturalPass,
        title: "Lifecycle, evidence & natural pass",
        description: "Compare rejected and accepted identities, capture typed evidence, then repeat representative flows naturally.",
        human_question: "Do the lifecycle surfaces, accepted-only scene, evidence, and natural recovery tell one trustworthy story?",
        fixture: ScenarioFixture::LifecycleEvidence,
        points: &LIFECYCLE_EVIDENCE_POINTS,
        steps: &LIFECYCLE_EVIDENCE_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::AttributedCanvasError,
        title: "Attributed canvas error",
        description: "Reject an incompatible line-length dimension and inspect owner-and-operand highlights over retained accepted geometry.",
        human_question: "Can a host user identify what is involved in the failure without mistaking attempted geometry for accepted truth?",
        fixture: ScenarioFixture::ErrorAttribution,
        points: &ATTRIBUTED_ERROR_POINTS,
        steps: &ATTRIBUTED_ERROR_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::GlobalCanvasError,
        title: "Global canvas error",
        description: "Submit a wrong-kind host parameter that cannot be defensibly attributed to an individual canvas element.",
        human_question: "Is the global fallback clear and noticeable without falsely highlighting unrelated geometry?",
        fixture: ScenarioFixture::ParameterProposal,
        points: &GLOBAL_ERROR_POINTS,
        steps: &GLOBAL_ERROR_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::AlphaParityCatalog,
        title: "Contextual relation & dimension catalog",
        description: "Inspect contextual contact/equality/direction/tangency/continuity dispatch, dimensions and explicit branches through the sole workbench.",
        human_question: "Do the compact intents, selection-specific labels, selectable glyphs, operands and branch diagnostics form a coherent reusable authoring catalog?",
        fixture: ScenarioFixture::AlphaParity,
        points: &ALPHA_PARITY_POINTS,
        steps: &ALPHA_PARITY_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::AlphaBranchRecovery,
        title: "Contact branch & rejection recovery",
        description: "Edit explicit tangent orientation, submit an impossible fixed contact, inspect retained accepted truth, and recover deterministically.",
        human_question: "Is explicit branch ownership clear before, during, and after a rejected contact action?",
        fixture: ScenarioFixture::AlphaBranchRecovery,
        points: &ALPHA_BRANCH_POINTS,
        steps: &ALPHA_BRANCH_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::AdvancedAllFamilies,
        title: "Advanced all-family gallery",
        description: "Inspect accepted analytic, polynomial, spline, and rational curve families through public scene and stable diagnostic APIs.",
        human_question: "Can a CAD user distinguish the advanced families and trust that the visible geometry and diagnostics share one accepted source?",
        fixture: ScenarioFixture::AdvancedGallery,
        points: &ADVANCED_ALL_FAMILIES_POINTS,
        steps: &ADVANCED_ALL_FAMILIES_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::NurbsBranchTopology,
        title: "NURBS branch & knot topology",
        description: "Exercise a periodic semantic-span transition, explicit winding, and geometry-preserving knot insertion.",
        human_question: "Are periodic branch state and topology-changing refinement explicit, predictable, and recoverable?",
        fixture: ScenarioFixture::NurbsBranches,
        points: &NURBS_BRANCH_POINTS,
        steps: &NURBS_BRANCH_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::AssociativeCompanionOperations,
        title: "Associative & companion operations",
        description: "Inspect a public associative fillet, then publish split, exact mirror, and bounded pattern proposals through ordinary transactions.",
        human_question: "Does operation ownership remain clear while associated and generated geometry move into the accepted sketch?",
        fixture: ScenarioFixture::Operations,
        points: &OPERATIONS_POINTS,
        steps: &OPERATIONS_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::ProductionTopologyTrust,
        title: "Production topology trust",
        description: "Compare complete production output with open-support incompleteness, cooperative cancellation, and deterministic recovery.",
        human_question: "Can a host user immediately tell which topology is consumable and why incomplete or cancelled work is not?",
        fixture: ScenarioFixture::ProductionTopology,
        points: &PRODUCTION_TOPOLOGY_POINTS,
        steps: &PRODUCTION_TOPOLOGY_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::DraftingCompass,
        title: "Drafting compass · 1 DOF",
        description: "Drag the preselected first tip; equal-length arms remain symmetric about a fixed 30-degree bisector.",
        human_question: "Does projected tip motion make the compass's single rotational freedom and symmetry obvious?",
        fixture: ScenarioFixture::StressCompass,
        points: &COMPASS_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::BezierBridge,
        title: "Bezier C1 bridge · 1 bounded DOF",
        description: "Drag the preselected left seam of two tangent cubic Beziers while the suppressed equal-handle row leaves one bounded seam freedom.",
        human_question: "Is the one-DOF seam motion smooth, regular, and visibly associated across both curves?",
        fixture: ScenarioFixture::StressBridge,
        points: &BRIDGE_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::TwinRollerCam,
        title: "Twin-roller Bezier cam · 2 DOF",
        description: "Drag the preselected left roller around its normal-offset path; the second roller retains its independent freedom.",
        human_question: "Can you distinguish the two independent roller freedoms and trust each projected contact?",
        fixture: ScenarioFixture::MotionCam,
        points: &CAM_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::TangentOrbit,
        title: "Tangent orbit · 1 DOF",
        description: "Drag the preselected satellite center around the complete external-tangency locus with explicit periodic branch state.",
        human_question: "Does the satellite traverse the full orbit without a branch flip or discontinuity?",
        fixture: ScenarioFixture::MotionOrbit,
        points: &ORBIT_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::EllipticTrammel,
        title: "Elliptic trammel · 1 DOF",
        description: "Drag the preselected horizontal slider; the perpendicular rail sliders drive a quarter-point tracer along an exact ellipse.",
        human_question: "Does the emergent tracer path make the ordinary rail and bar constraints believable?",
        fixture: ScenarioFixture::MotionTrammel,
        points: &TRAMMEL_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::ScotchYoke,
        title: "Scotch yoke · 1 DOF",
        description: "Drag the preselected crank pin; the fixed crank and vertical slot produce sinusoidal horizontal slider travel.",
        human_question: "Is rotational-to-linear motion coherent throughout the usable crank range?",
        fixture: ScenarioFixture::MotionScotchYoke,
        points: &SCOTCH_YOKE_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::RotatingSquare,
        title: "Constraint-built rotating square · 1 DOF",
        description: "Drag the preselected corner; four ordinary lines remain a rigid square rotating about their fixed corner.",
        human_question: "Does the relation-built square preserve rigidity without relying on a privileged rectangle primitive?",
        fixture: ScenarioFixture::MotionRotatingSquare,
        points: &ROTATING_SQUARE_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::ScissorJack,
        title: "Scissor jack · 1 DOF",
        description: "Drag the preselected horizontal base slider to open and close a mirrored equal-arm scissor mechanism.",
        human_question: "Is the base-driven mirrored motion stable, predictable, and easy to inspect?",
        fixture: ScenarioFixture::MotionScissor,
        points: &SCISSOR_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::ScissorTower,
        title: "Five-stage scissor tower · 1 DOF",
        description: "Drag the preselected right base pivot; one base freedom synchronously raises or lowers all five X stages.",
        human_question: "Can you zoom, pan, and drag the large tower while seeing coherent motion propagate through every stage?",
        fixture: ScenarioFixture::MotionScissorTower,
        points: &SCISSOR_TOWER_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
    ScenarioDefinition {
        id: ScenarioId::PeaucellierLinkage,
        title: "Peaucellier straight-line linkage · 1 DOF",
        description: "Drag the preselected input crank; the seven-bar inversor maps circular input motion to an exact straight output path.",
        human_question: "Does the dependent output motion convincingly demonstrate the one-DOF bar system?",
        fixture: ScenarioFixture::MotionPeaucellier,
        points: &PEAUCELLIER_POINTS,
        steps: &INTERACTIVE_MOTION_STEPS,
    },
];

const GEOMETRY_INTENT_CHILDREN: [ScenarioNode; 2] = [
    ScenarioNode::Scenario(ScenarioId::RoleProfileParticipation),
    ScenarioNode::Scenario(ScenarioId::ActivationDimensionMode),
];

const HOST_OWNED_INPUTS_CHILDREN: [ScenarioNode; 3] = [
    ScenarioNode::Scenario(ScenarioId::SharedParameterProposal),
    ScenarioNode::Scenario(ScenarioId::InvalidStaleParameterRecovery),
    ScenarioNode::Scenario(ScenarioId::ExternalLossExplicitRecovery),
];

const TRUTH_EVIDENCE_CHILDREN: [ScenarioNode; 1] = [ScenarioNode::Scenario(
    ScenarioId::LifecycleEvidenceNaturalPass,
)];

const ERROR_ATTRIBUTION_CHILDREN: [ScenarioNode; 2] = [
    ScenarioNode::Scenario(ScenarioId::AttributedCanvasError),
    ScenarioNode::Scenario(ScenarioId::GlobalCanvasError),
];

const GEOMETRY_INTENT_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::GeometryIntent,
    title: "Geometry intent",
    description: "Role, participation, activation, and dimension-mode distinctions.",
    children: &GEOMETRY_INTENT_CHILDREN,
};

const HOST_OWNED_INPUTS_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::HostOwnedInputs,
    title: "Host-owned inputs",
    description: "Atomic parameter ownership and explicit external-data recovery.",
    children: &HOST_OWNED_INPUTS_CHILDREN,
};

const TRUTH_EVIDENCE_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::TruthEvidence,
    title: "Truth & evidence",
    description: "Accepted-only lifecycle truth, finding evidence, and natural-use trust.",
    children: &TRUTH_EVIDENCE_CHILDREN,
};

const ERROR_ATTRIBUTION_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::ErrorAttribution,
    title: "Error attribution",
    description: "Element-targeted and honest global current-error presentation.",
    children: &ERROR_ATTRIBUTION_CHILDREN,
};

const M53_HOST_SEMANTICS_CHILDREN: [ScenarioNode; 4] = [
    ScenarioNode::Group(GEOMETRY_INTENT_GROUP),
    ScenarioNode::Group(HOST_OWNED_INPUTS_GROUP),
    ScenarioNode::Group(TRUTH_EVIDENCE_GROUP),
    ScenarioNode::Group(ERROR_ATTRIBUTION_GROUP),
];

const M53_HOST_SEMANTICS_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::M53HostSemantics,
    title: "M53 Host semantics",
    description: "Disposable human-review scenarios over fixed typed host-state fixtures; no scenario is persisted.",
    children: &M53_HOST_SEMANTICS_CHILDREN,
};

const M55_ACTION_PARITY_CHILDREN: [ScenarioNode; 2] = [
    ScenarioNode::Scenario(ScenarioId::AlphaParityCatalog),
    ScenarioNode::Scenario(ScenarioId::AlphaBranchRecovery),
];

const M55_ACTION_PARITY_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::M55ActionParity,
    title: "M55 Contextual constraints",
    description: "Reusable contextual dispatch, dimension, explicit-branch and rejection-recovery scenarios.",
    children: &M55_ACTION_PARITY_CHILDREN,
};

const COMPACT_MECHANISM_CHILDREN: [ScenarioNode; 7] = [
    ScenarioNode::Scenario(ScenarioId::DraftingCompass),
    ScenarioNode::Scenario(ScenarioId::BezierBridge),
    ScenarioNode::Scenario(ScenarioId::TwinRollerCam),
    ScenarioNode::Scenario(ScenarioId::TangentOrbit),
    ScenarioNode::Scenario(ScenarioId::EllipticTrammel),
    ScenarioNode::Scenario(ScenarioId::ScotchYoke),
    ScenarioNode::Scenario(ScenarioId::RotatingSquare),
];

const COMPACT_MECHANISMS_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::CompactMechanisms,
    title: "Compact mechanisms",
    description: "One- and two-DOF curve/contact and ordinary-constraint mechanisms.",
    children: &COMPACT_MECHANISM_CHILDREN,
};

const LINKAGE_MECHANISM_CHILDREN: [ScenarioNode; 3] = [
    ScenarioNode::Scenario(ScenarioId::ScissorJack),
    ScenarioNode::Scenario(ScenarioId::ScissorTower),
    ScenarioNode::Scenario(ScenarioId::PeaucellierLinkage),
];

const LINKAGE_MECHANISMS_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::LinkageMechanisms,
    title: "Linkage mechanisms",
    description: "Representative one-DOF scissor and exact straight-line bar linkages.",
    children: &LINKAGE_MECHANISM_CHILDREN,
};

const INTERACTIVE_MECHANISM_CHILDREN: [ScenarioNode; 2] = [
    ScenarioNode::Group(COMPACT_MECHANISMS_GROUP),
    ScenarioNode::Group(LINKAGE_MECHANISMS_GROUP),
];

const INTERACTIVE_MECHANISMS_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::InteractiveMechanisms,
    title: "Interactive mechanisms",
    description: "Movable accepted scenarios with a preselected primary driver and documented nonzero DOF.",
    children: &INTERACTIVE_MECHANISM_CHILDREN,
};

const ADVANCED_CURVES_TOPOLOGY_CHILDREN: [ScenarioNode; 4] = [
    ScenarioNode::Scenario(ScenarioId::AdvancedAllFamilies),
    ScenarioNode::Scenario(ScenarioId::NurbsBranchTopology),
    ScenarioNode::Scenario(ScenarioId::AssociativeCompanionOperations),
    ScenarioNode::Scenario(ScenarioId::ProductionTopologyTrust),
];

const ADVANCED_CURVES_TOPOLOGY_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::AdvancedCurvesTopology,
    title: "Advanced curves & topology",
    description: "Accepted curve families, explicit NURBS branches, operations, and production topology.",
    children: &ADVANCED_CURVES_TOPOLOGY_CHILDREN,
};

const M61_ADVANCED_TOPOLOGY_CHILDREN: [ScenarioNode; 2] = [
    ScenarioNode::Group(INTERACTIVE_MECHANISMS_GROUP),
    ScenarioNode::Group(ADVANCED_CURVES_TOPOLOGY_GROUP),
];

const M61_ADVANCED_TOPOLOGY_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::M61AdvancedTopology,
    title: "M61 Advanced geometry & topology",
    description: "Prepared advanced-curve, branch, companion-operation and production-topology review scenarios.",
    children: &M61_ADVANCED_TOPOLOGY_CHILDREN,
};

const ROOT_CHILDREN: [ScenarioNode; 3] = [
    ScenarioNode::Group(M53_HOST_SEMANTICS_GROUP),
    ScenarioNode::Group(M55_ACTION_PARITY_GROUP),
    ScenarioNode::Group(M61_ADVANCED_TOPOLOGY_GROUP),
];

const ROOT_GROUP: ScenarioGroup = ScenarioGroup {
    id: ScenarioGroupId::Root,
    title: "GeoSolve scenarios",
    description: "Versioned reusable review scenarios over public domain and editor APIs.",
    children: &ROOT_CHILDREN,
};

pub(crate) const SCENARIO_CATALOG: ScenarioCatalog = ScenarioCatalog {
    root: &ROOT_GROUP,
    scenarios: &SCENARIOS,
    verification_points: &VERIFICATION_POINTS,
};

struct ScenarioRunner {
    selected: ScenarioId,
    candidate: ScenarioCandidate,
}

pub(crate) struct ScenarioWorkbenchState {
    runner: Option<ScenarioRunner>,
}

impl fmt::Debug for ScenarioWorkbenchState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScenarioWorkbenchState")
            .field("selected", &self.selected_id())
            .finish_non_exhaustive()
    }
}

impl ScenarioWorkbenchState {
    pub(crate) const fn new() -> Self {
        Self { runner: None }
    }

    pub(crate) fn select(&mut self, id: ScenarioId) -> Result<(), String> {
        let candidate = ScenarioCandidate::new(id.definition().fixture())?;
        self.runner = Some(ScenarioRunner {
            selected: id,
            candidate,
        });
        Ok(())
    }

    pub(crate) fn select_key(&mut self, key: &str) -> Result<(), String> {
        let id = ScenarioId::from_key(key)
            .ok_or_else(|| format!("Unknown workbench scenario key: {key}"))?;
        self.select(id)
    }

    pub(crate) fn reset(&mut self) -> Result<(), String> {
        let selected = self
            .selected_id()
            .ok_or_else(|| "Select a workbench scenario before resetting it".to_owned())?;
        self.select(selected)
    }

    pub(crate) fn exit(&mut self) {
        self.runner = None;
    }

    pub(crate) const fn is_active(&self) -> bool {
        self.runner.is_some()
    }

    pub(crate) fn selected_id(&self) -> Option<ScenarioId> {
        self.runner.as_ref().map(|runner| runner.selected)
    }

    pub(crate) fn selected_key(&self) -> Option<&'static str> {
        self.selected_id().map(ScenarioId::key)
    }

    pub(crate) fn selected_title(&self) -> Option<&'static str> {
        self.selected_id()
            .map(|selected| selected.definition().title())
    }

    pub(crate) fn coordinator_for_render<'a>(
        &'a self,
        ordinary: &'a RetainedEditorCoordinator,
    ) -> &'a RetainedEditorCoordinator {
        self.runner
            .as_ref()
            .map_or(ordinary, |runner| runner.candidate.active_coordinator())
    }

    pub(crate) fn coordinator_for_interaction_mut<'a>(
        &'a mut self,
        ordinary: &'a mut RetainedEditorCoordinator,
    ) -> &'a mut RetainedEditorCoordinator {
        self.runner
            .as_mut()
            .map_or(ordinary, |runner| runner.candidate.active_coordinator_mut())
    }

    pub(crate) fn resolve_projected_point_move(
        &mut self,
        ordinary: &mut RetainedEditorCoordinator,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        model_position: [f64; 2],
    ) -> Vec<EditorEffect> {
        self.runner.as_mut().map_or_else(
            || ordinary.resolve_projected_point_move(pointer_id, request_id, point, model_position),
            |runner| {
                runner.candidate.resolve_projected_point_move(
                    pointer_id,
                    request_id,
                    point,
                    model_position,
                )
            },
        )
    }

    pub(crate) fn ordinary_action_allowed(&self, action: &str) -> bool {
        !self.is_active() || matches!(action, "problems" | "zoom-in" | "zoom-out" | "zoom-fit")
    }

    pub(crate) fn persistence_snapshot(
        &self,
        ordinary: &RetainedEditorCoordinator,
    ) -> Option<WorkspaceSnapshot> {
        (!self.is_active()).then(|| WorkspaceSnapshot::from_checkpoint(ordinary.checkpoint()))
    }

    pub(crate) fn perform(
        &mut self,
        action: ScenarioAction,
    ) -> Result<ScenarioObservation, String> {
        let runner = self
            .runner
            .as_mut()
            .ok_or_else(|| "Select a workbench scenario before using its actions".to_owned())?;
        let definition = runner.selected.definition();
        if action.fixture().is_some() && !definition.allows_action(action) {
            return Err(format!(
                "{} is not an action in {}",
                action.label(),
                definition.title()
            ));
        }
        runner.candidate.perform(action)
    }

    pub(crate) fn perform_key(&mut self, action_key: &str) -> Result<ScenarioObservation, String> {
        let action = ScenarioAction::from_key(action_key)
            .ok_or_else(|| format!("Unknown workbench scenario action: {action_key}"))?;
        if action == ScenarioAction::CaptureEvidence {
            self.capture()
        } else {
            self.perform(action)
        }
    }

    pub(crate) fn capture(&mut self) -> Result<ScenarioObservation, String> {
        self.perform(ScenarioAction::CaptureEvidence)
    }

    pub(crate) fn transcript(&self) -> &[ScenarioObservation] {
        self.runner
            .as_ref()
            .map_or(&[], |runner| runner.candidate.transcript())
    }

    pub(crate) fn evidence_text(&self) -> Option<&str> {
        self.runner
            .as_ref()
            .map(|runner| runner.candidate.evidence_text())
    }

    pub(crate) fn menu_markup(&self) -> String {
        selector_markup(self.selected_id())
    }

    pub(crate) fn guide_markup(&self) -> Option<String> {
        self.selected_id().map(scenario_guide_markup)
    }

    pub(crate) fn transcript_markup(&self) -> String {
        let mut markup =
            String::from("<section class=\"wb-scenario-transcript\"><h3>Scenario transcript</h3>");
        if self.transcript().is_empty() {
            markup.push_str("<p>No scenario action has been performed.</p>");
        } else {
            markup.push_str("<ol>");
            for observation in self.transcript() {
                markup.push_str("<li>");
                push_escaped_text(&mut markup, &observation.summary());
                markup.push_str("</li>");
            }
            markup.push_str("</ol>");
        }
        markup.push_str("</section>");
        markup
    }

    pub(crate) fn evidence_markup(&self) -> String {
        let mut markup =
            String::from("<section class=\"wb-scenario-evidence\"><h3>Typed evidence</h3><pre>");
        push_escaped_text(
            &mut markup,
            self.evidence_text()
                .unwrap_or("Select a scenario before capturing typed evidence."),
        );
        markup.push_str("</pre></section>");
        markup
    }
}

fn selector_markup(selected: Option<ScenarioId>) -> String {
    let root = SCENARIO_CATALOG.root();
    let mut markup = String::from(
        "<nav class=\"wb-scenario-catalog\" aria-label=\"GeoSolve scenario selector\"><header class=\"wb-scenario-catalog-header\"><strong>",
    );
    push_escaped_text(&mut markup, root.title());
    markup.push_str("</strong><span>");
    push_escaped_text(&mut markup, root.description());
    markup.push_str("</span></header>");
    render_level(&mut markup, root.children(), selected);
    markup.push_str("</nav>");
    markup
}

fn render_level(markup: &mut String, nodes: &[ScenarioNode], selected: Option<ScenarioId>) {
    markup.push_str("<ul class=\"wb-scenario-level\">");
    for node in nodes {
        match node {
            ScenarioNode::Group(group) => render_group_branch(markup, *group, selected),
            ScenarioNode::Scenario(id) => {
                markup.push_str("<li class=\"wb-scenario-leaf\">");
                let definition = id.definition();
                let _ = write!(
                    markup,
                    "<button type=\"button\" data-scenario-id=\"{}\"{}>",
                    id.key(),
                    if Some(*id) == selected {
                        " aria-current=\"true\""
                    } else {
                        ""
                    }
                );
                markup.push_str("<strong>");
                push_escaped_text(markup, definition.title());
                markup.push_str("</strong><span>");
                push_escaped_text(markup, definition.description());
                markup.push_str("</span></button></li>");
            }
        }
    }
    markup.push_str("</ul>");
}

fn render_group_branch(markup: &mut String, group: ScenarioGroup, selected: Option<ScenarioId>) {
    let key = group.id().key();
    let _ = write!(
        markup,
        concat!(
            "<li class=\"wb-scenario-branch\" data-scenario-group=\"{key}\">",
            "<button id=\"wb-scenario-group-{key}\" type=\"button\" ",
            "class=\"wb-scenario-branch-trigger\" data-scenario-group-trigger=\"{key}\" ",
            "aria-expanded=\"false\" aria-controls=\"wb-scenario-flyout-{key}\"><strong>"
        ),
        key = key,
    );
    push_escaped_text(markup, group.title());
    markup.push_str("</strong><span>");
    push_escaped_text(markup, group.description());
    let _ = write!(
        markup,
        "</span></button><div id=\"wb-scenario-flyout-{key}\" class=\"wb-scenario-flyout\" aria-labelledby=\"wb-scenario-group-{key}\">",
    );
    render_level(markup, group.children(), selected);
    markup.push_str("</div></li>");
}

fn scenario_guide_markup(id: ScenarioId) -> String {
    let definition = id.definition();
    let mut markup = format!(
        "<section class=\"wb-scenario-guide-content\" data-selected-scenario=\"{}\"><header><h2 id=\"wb-scenario-title\" tabindex=\"-1\">",
        id.key()
    );
    push_escaped_text(&mut markup, definition.title());
    markup.push_str("</h2><p>");
    push_escaped_text(&mut markup, definition.description());
    markup.push_str(
        "</p></header><p class=\"wb-scenario-human-question\"><strong>Human question:</strong> ",
    );
    push_escaped_text(&mut markup, definition.human_question());
    markup.push_str("</p><section class=\"wb-scenario-points\"><h3>Verification points</h3><ol>");
    for point_id in definition.points() {
        let point = SCENARIO_CATALOG
            .verification_point(*point_id)
            .expect("scenario verification point must exist in the catalog");
        let _ = write!(
            markup,
            "<li value=\"{}\"><p><strong>Objective:</strong> ",
            point.id().number()
        );
        push_escaped_text(&mut markup, point.objective());
        markup.push_str("</p><p><strong>Human judgment:</strong> ");
        push_escaped_text(&mut markup, point.human_judgment());
        markup.push_str("</p></li>");
    }
    markup
        .push_str("</ol></section><section class=\"wb-scenario-steps\"><h3>Guided steps</h3><ol>");
    for step in definition.steps() {
        markup.push_str("<li><p>");
        push_escaped_text(&mut markup, step.instruction());
        markup.push_str("</p>");
        if let Some(action) = step.action() {
            let _ = write!(
                markup,
                "<button type=\"button\" data-scenario-action=\"{}\">",
                action.key()
            );
            push_escaped_text(&mut markup, action.label());
            markup.push_str("</button>");
        }
        markup.push_str("<p class=\"wb-scenario-expected\"><strong>Expected:</strong> ");
        push_escaped_text(&mut markup, step.expected());
        markup.push_str("</p></li>");
    }
    markup.push_str(concat!(
        "</ol></section><div class=\"wb-scenario-global-controls\">",
        "<button type=\"button\" data-scenario-action=\"capture\">Capture typed evidence</button>",
        "<button type=\"button\" data-scenario-control=\"reset\">Reset scenario</button>",
        "<button type=\"button\" data-scenario-control=\"exit\">Exit scenario</button>",
        "</div></section>"
    ));
    markup
}

fn push_escaped_text(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            character => output.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use geosolve_constraint_editor::{EditorEffect, SelectionItem};
    use geosolve_sketch::{AlphaScenarioIds, AlphaScenarioKind, alpha_scenario};

    use super::{
        ALL_SCENARIO_ACTIONS, SCENARIO_CATALOG, ScenarioAction, ScenarioCandidate, ScenarioFixture,
        ScenarioGroup, ScenarioId, ScenarioNode, ScenarioWorkbenchState, VerificationPointId,
    };

    fn collect_catalog(
        group: ScenarioGroup,
        groups: &mut Vec<&'static str>,
        scenarios: &mut Vec<ScenarioId>,
    ) {
        groups.push(group.id().key());
        for node in group.children() {
            match node {
                ScenarioNode::Group(child) => collect_catalog(*child, groups, scenarios),
                ScenarioNode::Scenario(id) => scenarios.push(*id),
            }
        }
    }

    #[test]
    fn typed_catalog_is_nested_complete_and_round_trips_stable_keys() {
        let mut groups = Vec::new();
        let mut scenarios = Vec::new();
        collect_catalog(*SCENARIO_CATALOG.root(), &mut groups, &mut scenarios);

        assert_eq!(SCENARIO_CATALOG.root().title(), "GeoSolve scenarios");
        assert_eq!(groups.len(), 12);
        assert_eq!(scenarios.len(), ScenarioId::ALL.len());
        let unique_groups: HashSet<_> = groups.iter().copied().collect();
        let unique_scenarios: HashSet<_> = scenarios.iter().copied().collect();
        assert_eq!(unique_groups.len(), groups.len());
        assert_eq!(unique_scenarios.len(), scenarios.len());

        for id in ScenarioId::ALL {
            assert!(unique_scenarios.contains(&id));
            assert_eq!(ScenarioId::from_key(id.key()), Some(id));
            let definition = id.definition();
            assert_eq!(definition.id(), id);
            assert!(!definition.title().is_empty());
            assert!(!definition.description().is_empty());
            assert!(!definition.human_question().is_empty());
            assert!(!definition.steps().is_empty());
        }
        assert_eq!(ScenarioId::from_key("not-a-scenario"), None);

        let mut action_keys = HashSet::new();
        for action in ALL_SCENARIO_ACTIONS {
            assert!(action_keys.insert(action.key()));
            assert_eq!(ScenarioAction::from_key(action.key()), Some(action));
            assert!(!action.label().is_empty());
        }
        assert_eq!(ScenarioAction::from_key("not-an-action"), None);
    }

    #[test]
    fn m53_and_m55_stable_scenario_identities_remain_unchanged() {
        let preserved = [
            "role-profile-participation",
            "activation-dimension-mode",
            "shared-parameter-proposal",
            "invalid-stale-parameter-recovery",
            "external-loss-explicit-recovery",
            "lifecycle-evidence-natural-pass",
            "attributed-canvas-error",
            "global-canvas-error",
            "alpha-parity-catalog",
            "alpha-branch-recovery",
        ];
        assert_eq!(
            ScenarioId::ALL[..preserved.len()]
                .iter()
                .map(|id| id.key())
                .collect::<Vec<_>>(),
            preserved
        );
        for key in preserved {
            assert_eq!(ScenarioId::from_key(key).unwrap().key(), key);
        }
    }

    #[test]
    fn every_verification_point_is_owned_once_and_every_action_is_reachable() {
        let mut point_counts = HashMap::new();
        let mut represented_actions = HashSet::new();
        for scenario in SCENARIO_CATALOG.scenarios() {
            for point in scenario.points() {
                *point_counts.entry(*point).or_insert(0_usize) += 1;
            }
            for step in scenario.steps() {
                assert!(!step.instruction().is_empty());
                assert!(!step.expected().is_empty());
                if let Some(action) = step.action() {
                    represented_actions.insert(action);
                    if let Some(fixture) = action.fixture() {
                        assert_eq!(fixture, scenario.fixture());
                    }
                }
            }
        }

        for point in VerificationPointId::ALL {
            assert_eq!(point_counts.get(&point), Some(&1));
            let definition = SCENARIO_CATALOG.verification_point(point).unwrap();
            assert!(!definition.objective().is_empty());
            assert!(!definition.human_judgment().is_empty());
        }
        assert_eq!(
            SCENARIO_CATALOG.verification_points().len(),
            VerificationPointId::ALL.len()
        );
        assert_eq!(
            represented_actions,
            ALL_SCENARIO_ACTIONS.into_iter().collect()
        );
    }

    #[test]
    fn selector_uses_recursive_hover_focus_flyouts_and_plain_list_buttons() {
        let mut state = ScenarioWorkbenchState::new();
        state
            .select(ScenarioId::ExternalLossExplicitRecovery)
            .unwrap();
        let markup = state.menu_markup();

        assert_eq!(markup.matches("data-scenario-group-trigger=").count(), 11);
        assert_eq!(markup.matches("class=\"wb-scenario-flyout\"").count(), 11);
        assert_eq!(markup.matches("data-scenario-id=").count(), 24);
        assert!(markup.contains("class=\"wb-scenario-catalog-header\""));
        assert!(markup.contains("aria-expanded=\"false\""));
        assert!(markup.contains("aria-controls=\"wb-scenario-flyout-host-owned-inputs\""));
        assert!(!markup.contains("<details"));
        assert!(!markup.contains("<summary"));
        assert!(markup.contains(
            "data-scenario-id=\"external-loss-explicit-recovery\" aria-current=\"true\""
        ));
        assert!(markup.contains("data-scenario-group=\"m61-advanced-topology\""));
        assert!(markup.contains("data-scenario-group=\"interactive-mechanisms\""));
        assert!(markup.contains("data-scenario-group=\"compact-mechanisms\""));
        assert!(markup.contains("data-scenario-group=\"linkage-mechanisms\""));
        assert!(markup.contains("data-scenario-id=\"five-stage-scissor-tower\""));
        assert!(markup.contains("data-scenario-id=\"production-topology-trust\""));
        assert!(!markup.contains("role=\"tree"));
        assert!(!markup.contains("role=\"menu"));
        assert!(!markup.contains("role=\"listbox"));
    }

    #[test]
    fn selected_guide_contains_detail_expected_results_and_only_its_typed_actions() {
        let mut state = ScenarioWorkbenchState::new();
        state
            .select(ScenarioId::InvalidStaleParameterRecovery)
            .unwrap();
        let guide = state.guide_markup().unwrap();

        assert!(guide.contains("Invalid/stale parameter recovery"));
        assert!(guide.contains("<h2 id=\"wb-scenario-title\" tabindex=\"-1\">"));
        assert!(guide.contains("Human question:"));
        assert!(guide.contains("data-scenario-action=\"parameter-valid\""));
        assert!(guide.contains("data-scenario-action=\"parameter-invalid\""));
        assert!(guide.contains("data-scenario-action=\"parameter-stale\""));
        assert!(guide.contains("data-scenario-action=\"parameter-recovery\""));
        assert!(!guide.contains("data-scenario-action=\"external-missing\""));
        assert!(guide.contains("class=\"wb-scenario-expected\""));
        assert!(guide.contains("data-scenario-action=\"capture\""));
        assert!(guide.contains("data-scenario-control=\"reset\""));
        assert!(guide.contains("data-scenario-control=\"exit\""));
    }

    #[test]
    fn selected_scenario_filters_actions_and_reset_is_deterministic() {
        let ordinary_candidate = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let ordinary = ordinary_candidate.active_coordinator();
        let mut state = ScenarioWorkbenchState::new();
        state.select(ScenarioId::SharedParameterProposal).unwrap();

        let initial_identity = state
            .coordinator_for_render(ordinary)
            .session()
            .design_identity();
        let initial_accepted_identity = state
            .coordinator_for_render(ordinary)
            .session()
            .accepted_state()
            .unwrap()
            .identity();
        let initial_evidence = state.evidence_text().unwrap().to_owned();
        assert_eq!(state.selected_key(), Some("shared-parameter-proposal"));
        assert_eq!(state.selected_title(), Some("Shared parameter & proposal"));

        let rejected = state.perform_key("external-missing").unwrap_err();
        assert!(rejected.contains("not an action"));
        assert!(state.transcript().is_empty());

        state.perform_key("parameter-valid").unwrap();
        assert_eq!(state.transcript().len(), 1);
        state.capture().unwrap();
        assert_eq!(state.transcript().len(), 2);

        state.reset().unwrap();
        assert_eq!(
            state.selected_id(),
            Some(ScenarioId::SharedParameterProposal)
        );
        assert!(state.transcript().is_empty());
        assert_eq!(state.evidence_text(), Some(initial_evidence.as_str()));
        assert_eq!(
            state
                .coordinator_for_render(ordinary)
                .session()
                .design_identity(),
            initial_identity
        );

        state.perform_key("parameter-valid").unwrap();
        assert_ne!(
            state
                .coordinator_for_render(ordinary)
                .session()
                .accepted_state()
                .unwrap()
                .identity(),
            initial_accepted_identity
        );
        state.select(ScenarioId::RoleProfileParticipation).unwrap();
        state.select(ScenarioId::SharedParameterProposal).unwrap();
        assert!(state.transcript().is_empty());
        assert_eq!(state.evidence_text(), Some(initial_evidence.as_str()));
        assert_eq!(
            state
                .coordinator_for_render(ordinary)
                .session()
                .accepted_state()
                .unwrap()
                .identity(),
            initial_accepted_identity
        );
    }

    #[test]
    fn active_scenarios_suppress_persistence_and_exit_restores_the_ordinary_snapshot() {
        let ordinary_candidate = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let ordinary = ordinary_candidate.active_coordinator();
        let mut state = ScenarioWorkbenchState::new();
        let before = state.persistence_snapshot(ordinary).unwrap();

        state
            .select_key(ScenarioId::RoleProfileParticipation.key())
            .unwrap();
        assert!(state.is_active());
        assert!(state.persistence_snapshot(ordinary).is_none());
        for action in [
            "new",
            "undo",
            "redo",
            "finish",
            "cancel",
            "clear-selection",
            "delete",
            "constraint",
            "dimension",
        ] {
            assert!(!state.ordinary_action_allowed(action));
        }
        assert!(state.ordinary_action_allowed("problems"));
        assert!(state.select_key("not-a-scenario").is_err());
        assert_eq!(
            state.selected_id(),
            Some(ScenarioId::RoleProfileParticipation)
        );

        state.exit();
        assert!(!state.is_active());
        assert_eq!(state.selected_id(), None);
        assert_eq!(state.persistence_snapshot(ordinary).unwrap(), before);
        assert!(state.ordinary_action_allowed("new"));
    }

    #[test]
    fn interactive_mechanisms_publish_nonzero_mobility_and_preselect_their_driver() {
        let ordinary_candidate = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let ordinary = ordinary_candidate.active_coordinator();
        for (id, equality_dof, bounded_dof) in [
            (ScenarioId::DraftingCompass, 1, 1),
            (ScenarioId::BezierBridge, 3, 1),
            (ScenarioId::TwinRollerCam, 2, 2),
            (ScenarioId::TangentOrbit, 1, 1),
            (ScenarioId::EllipticTrammel, 1, 1),
            (ScenarioId::ScotchYoke, 1, 1),
            (ScenarioId::RotatingSquare, 1, 1),
            (ScenarioId::ScissorJack, 1, 1),
            (ScenarioId::ScissorTower, 1, 1),
            (ScenarioId::PeaucellierLinkage, 1, 1),
        ] {
            let mut state = ScenarioWorkbenchState::new();
            state.select(id).unwrap();
            let coordinator = state.coordinator_for_render(ordinary);
            assert!(matches!(
                coordinator.editor().selection(),
                [SelectionItem::Point(driver)]
                    if coordinator.session().design_document().point(*driver).is_some()
            ));
            let mobility = coordinator
                .session()
                .accepted_diagnostics()
                .unwrap()
                .mobility
                .unwrap();
            assert_eq!(
                mobility.equality_degrees_of_freedom,
                Some(equality_dof),
                "{}",
                id.key()
            );
            assert_eq!(
                mobility.bidirectional_bounded_degrees_of_freedom,
                Some(bounded_dof),
                "{}",
                id.key()
            );
        }
    }

    #[test]
    fn twin_roller_drag_keeps_the_passive_circle_at_its_accepted_position() {
        let ordinary_candidate = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let mut ordinary = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let expected = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).unwrap();
        let AlphaScenarioIds::MotionCam(ids) = expected.ids else {
            panic!("motion-cam IDs");
        };
        let mut state = ScenarioWorkbenchState::new();
        state.select(ScenarioId::TwinRollerCam).unwrap();
        let initial = state
            .coordinator_for_render(ordinary_candidate.active_coordinator())
            .session()
            .accepted_state()
            .unwrap()
            .document()
            .clone();

        for (driver, passive, parameters) in [
            (
                ids.left_center,
                ids.right_center,
                [0.26_f64, 0.28, 0.30, 0.32, 0.34, 0.32, 0.29, 0.27],
            ),
            (
                ids.right_center,
                ids.left_center,
                [0.74_f64, 0.72, 0.70, 0.68, 0.66, 0.68, 0.71, 0.73],
            ),
        ] {
            let passive_before = initial.point(passive).unwrap().position;
            for (request_id, parameter) in (1_u64..=8).zip(parameters) {
                let tangent = [8.0, 8.0 - 16.0 * parameter];
                let tangent_norm = f64::hypot(tangent[0], tangent[1]);
                let target = [
                    -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
                    8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
                ];
                let _effects = state.resolve_projected_point_move(
                    ordinary.active_coordinator_mut(),
                    7,
                    request_id,
                    driver,
                    target,
                );
                let preview = state
                    .coordinator_for_render(ordinary_candidate.active_coordinator())
                    .solved_preview_session()
                    .expect("accepted projected preview");
                let request = preview.last_attempt().input().candidate_request();
                assert_eq!(
                    request.stability_target.map(|target| target.point),
                    Some(passive)
                );
                let passive_after = preview
                    .accepted_state()
                    .unwrap()
                    .document()
                    .point(passive)
                    .unwrap()
                    .position;
                assert!(
                    f64::hypot(
                        passive_after[0] - passive_before[0],
                        passive_after[1] - passive_before[1]
                    ) <= 1.0e-9,
                    "passive roller moved from {passive_before:?} to {passive_after:?}"
                );
            }
        }
    }

    #[test]
    fn scissor_projected_drag_moves_dependents_and_reset_restores_exact_start() {
        let ordinary_candidate = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let mut ordinary = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let ordinary_before = ordinary
            .active_coordinator()
            .session()
            .design_document()
            .to_canonical_json()
            .unwrap();
        let mut state = ScenarioWorkbenchState::new();
        state.select(ScenarioId::ScissorJack).unwrap();
        let initial = state
            .coordinator_for_render(ordinary_candidate.active_coordinator())
            .session()
            .accepted_state()
            .unwrap()
            .document()
            .clone();
        let driver = match state
            .coordinator_for_render(ordinary_candidate.active_coordinator())
            .editor()
            .selection()
        {
            [SelectionItem::Point(driver)] => *driver,
            _ => panic!("one selected scissor driver"),
        };

        let coordinator = state.coordinator_for_interaction_mut(ordinary.active_coordinator_mut());
        let mut preview = coordinator.session().clone();
        let request = preview
            .last_attempt()
            .input()
            .candidate_request()
            .without_previous_state_preferences()
            .with_drag(driver, [3.5, 0.0]);
        let attempt = preview
            .reattempt(preview.design_identity(), request)
            .expect("projected scissor preview");
        assert!(attempt.accepted_state_identity().is_some());
        let preview_document = preview
            .accepted_state()
            .expect("preview accepted")
            .document()
            .clone();
        let accepted_driver = preview_document.point(driver).unwrap().position;
        coordinator
            .mark_solved_preview(&preview)
            .expect("publish solved preview");
        coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: coordinator.session().design_identity(),
                point: driver,
                model_position: accepted_driver,
            })
            .expect("commit projected move")
            .expect("accepted mutation");
        let moved = coordinator.session().accepted_state().unwrap().document();
        assert!(moved.points().iter().any(|point| {
            point.id != driver
                && initial.point(point.id).is_some_and(|before| {
                    (before.position[0] - point.position[0])
                        .hypot(before.position[1] - point.position[1])
                        > 1.0e-6
                })
        }));

        state.reset().unwrap();
        assert_eq!(
            state
                .coordinator_for_render(ordinary_candidate.active_coordinator())
                .session()
                .accepted_state()
                .unwrap()
                .document(),
            &initial
        );
        assert_eq!(
            ordinary
                .active_coordinator()
                .session()
                .design_document()
                .to_canonical_json()
                .unwrap(),
            ordinary_before
        );
    }
}
