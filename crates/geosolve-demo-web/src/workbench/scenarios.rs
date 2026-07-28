// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::fmt::{self, Write as _};

use geosolve_constraint_editor::RetainedEditorCoordinator;

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
}

impl VerificationPointId {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 12] = [
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
}

impl ScenarioId {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 8] = [
        Self::RoleProfileParticipation,
        Self::ActivationDimensionMode,
        Self::SharedParameterProposal,
        Self::InvalidStaleParameterRecovery,
        Self::ExternalLossExplicitRecovery,
        Self::LifecycleEvidenceNaturalPass,
        Self::AttributedCanvasError,
        Self::GlobalCanvasError,
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
    M53HostSemantics,
    GeometryIntent,
    HostOwnedInputs,
    TruthEvidence,
    ErrorAttribution,
}

impl ScenarioGroupId {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::M53HostSemantics => "m53-host-semantics",
            Self::GeometryIntent => "geometry-intent",
            Self::HostOwnedInputs => "host-owned-inputs",
            Self::TruthEvidence => "truth-evidence",
            Self::ErrorAttribution => "error-attribution",
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
pub(crate) const ALL_SCENARIO_ACTIONS: [ScenarioAction; 21] = [
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
    ScenarioAction::CaptureEvidence,
];

const VERIFICATION_POINTS: [VerificationPoint; 12] = [
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

const SCENARIOS: [ScenarioDefinition; 8] = [
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

pub(crate) const SCENARIO_CATALOG: ScenarioCatalog = ScenarioCatalog {
    root: &M53_HOST_SEMANTICS_GROUP,
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

    pub(crate) fn ordinary_action_allowed(&self, action: &str) -> bool {
        !self.is_active() || action == "problems"
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
        "<nav class=\"wb-scenario-catalog\" aria-label=\"M53 scenario selector\"><header class=\"wb-scenario-catalog-header\"><strong>",
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

        assert_eq!(SCENARIO_CATALOG.root().title(), "M53 Host semantics");
        assert_eq!(groups.len(), 5);
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

        assert_eq!(markup.matches("data-scenario-group-trigger=").count(), 4);
        assert_eq!(markup.matches("class=\"wb-scenario-flyout\"").count(), 4);
        assert_eq!(markup.matches("data-scenario-id=").count(), 8);
        assert!(markup.contains("class=\"wb-scenario-catalog-header\""));
        assert!(markup.contains("aria-expanded=\"false\""));
        assert!(markup.contains("aria-controls=\"wb-scenario-flyout-host-owned-inputs\""));
        assert!(!markup.contains("<details"));
        assert!(!markup.contains("<summary"));
        assert!(markup.contains(
            "data-scenario-id=\"external-loss-explicit-recovery\" aria-current=\"true\""
        ));
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
}
