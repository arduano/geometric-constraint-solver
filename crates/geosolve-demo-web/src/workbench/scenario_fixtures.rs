// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use geosolve_constraint_editor::{
    BranchAction, ConstraintActionRequest, ConstraintIntent, ContactActionChoice, CoordinatorError,
    RetainedEditorCoordinator, SelectionItem,
};
use geosolve_core::SolverConfig;
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactDomain, ContactNeighborhood, CurveDefinition,
    CurveSpan, DesignPointId, DocumentBSplineSpanDirection, DocumentConstraintDefinition,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
    DocumentCurveSpanRef, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentDirectionSense, DocumentEdit, DocumentElementId, DocumentError,
    DocumentExternalLineSupportRef, DocumentId, DocumentLineSupportRef, DocumentParameterKind,
    DocumentParameterTarget, DocumentSessionError, DocumentSolveRequest, ExternalFeatureKindV1,
    ExternalLineOrientationV1, ExternalSnapshotDigest, ExternalSnapshotEntry,
    ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1, ExternalSnapshotSet,
    ExternalTopologyDigest, GeometryRole, HostActivationOverride, HostConfigurationActivation,
    OperationControl, OperationOutcome, ParameterBatch, ParameterBatchEntry, ParameterValue,
    PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    TangentOrientation, alpha_scenario, cancellation_pair,
};
use geosolve_sketch_ops::{
    SketchOperationRequest, SketchOperationResult, SketchOperationSnapshot, SplitRetainedPiece,
};
use geosolve_sketch_topology::{TopologyRequest, TopologySnapshot};

use super::evidence::{parameter_batch_json, serialize_scenario_typed_host_evidence};
use super::panels::{host_state_markup, production_topology_markup};

const TOPOLOGY_A: ExternalTopologyDigest = ExternalTopologyDigest::from_bytes([0x41; 32]);
const TOPOLOGY_B: ExternalTopologyDigest = ExternalTopologyDigest::from_bytes([0x42; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ScenarioFixture {
    RoleActivity,
    ParameterProposal,
    ExternalRebind,
    LifecycleEvidence,
    ErrorAttribution,
    AlphaParity,
    AlphaBranchRecovery,
    AdvancedGallery,
    NurbsBranches,
    Operations,
    ProductionTopology,
    StressCompass,
    StressBridge,
    MotionCam,
    MotionOrbit,
    MotionTrammel,
    MotionScotchYoke,
    MotionRotatingSquare,
    MotionScissor,
    MotionScissorTower,
    MotionPeaucellier,
}

impl ScenarioFixture {
    const fn label(self) -> &'static str {
        match self {
            Self::RoleActivity => "Role and activity",
            Self::ParameterProposal => "Parameter and proposal",
            Self::ExternalRebind => "External reference and rebind",
            Self::LifecycleEvidence => "Lifecycle and evidence",
            Self::ErrorAttribution => "Canvas error attribution",
            Self::AlphaParity => "Alpha relation and dimension parity",
            Self::AlphaBranchRecovery => "Explicit contact branches and recovery",
            Self::AdvancedGallery => "Advanced curve gallery",
            Self::NurbsBranches => "NURBS topology and branch state",
            Self::Operations => "Associative and companion operations",
            Self::ProductionTopology => "Production topology and cancellation",
            Self::StressCompass => "Drafting compass",
            Self::StressBridge => "Bezier C1 bridge",
            Self::MotionCam => "Twin-roller Bezier cam",
            Self::MotionOrbit => "Tangent orbit",
            Self::MotionTrammel => "Elliptic trammel",
            Self::MotionScotchYoke => "Scotch yoke",
            Self::MotionRotatingSquare => "Rotating square",
            Self::MotionScissor => "Scissor jack",
            Self::MotionScissorTower => "Five-stage scissor tower",
            Self::MotionPeaucellier => "Peaucellier straight-line linkage",
        }
    }

    const fn motion_kind(self) -> Option<AlphaScenarioKind> {
        Some(match self {
            Self::StressCompass => AlphaScenarioKind::StressCompass,
            Self::StressBridge => AlphaScenarioKind::StressBridge,
            Self::MotionCam => AlphaScenarioKind::MotionCam,
            Self::MotionOrbit => AlphaScenarioKind::MotionOrbit,
            Self::MotionTrammel => AlphaScenarioKind::MotionTrammel,
            Self::MotionScotchYoke => AlphaScenarioKind::MotionScotchYoke,
            Self::MotionRotatingSquare => AlphaScenarioKind::MotionRotatingSquare,
            Self::MotionScissor => AlphaScenarioKind::MotionScissor,
            Self::MotionScissorTower => AlphaScenarioKind::MotionScissorTower,
            Self::MotionPeaucellier => AlphaScenarioKind::MotionPeaucellier,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ScenarioAction {
    RoleConstruction,
    RoleProfile,
    SuppressDimension,
    ReactivateDimension,
    ReferenceDimension,
    HostInactive,
    MissingDependency,
    ParameterValid,
    ParameterInvalidKind,
    ParameterStale,
    ParameterRecovery,
    ExternalMissing,
    ExternalStale,
    ExternalTopologyChange,
    ExternalExplicitRebind,
    ExternalFreshRecovery,
    LifecycleRejected,
    LifecycleRecovery,
    AttributedConflict,
    AttributedRecovery,
    AlphaFlipTangency,
    AlphaRejectedContact,
    AlphaRecovery,
    NurbsNextSpan,
    NurbsInsertKnot,
    OperationSplit,
    OperationMirror,
    OperationPattern,
    TopologyMakeIncomplete,
    TopologyRecover,
    TopologyCancel,
    CaptureEvidence,
}

impl ScenarioAction {
    pub(crate) fn from_key(value: &str) -> Option<Self> {
        Some(match value {
            "role-construction" => Self::RoleConstruction,
            "role-profile" => Self::RoleProfile,
            "suppress" => Self::SuppressDimension,
            "reactivate" => Self::ReactivateDimension,
            "reference" => Self::ReferenceDimension,
            "host-inactive" => Self::HostInactive,
            "missing-dependency" => Self::MissingDependency,
            "parameter-valid" => Self::ParameterValid,
            "parameter-invalid" => Self::ParameterInvalidKind,
            "parameter-stale" => Self::ParameterStale,
            "parameter-recovery" => Self::ParameterRecovery,
            "external-missing" => Self::ExternalMissing,
            "external-stale" => Self::ExternalStale,
            "external-topology" => Self::ExternalTopologyChange,
            "external-rebind" => Self::ExternalExplicitRebind,
            "external-fresh" => Self::ExternalFreshRecovery,
            "lifecycle-rejected" => Self::LifecycleRejected,
            "lifecycle-recovery" => Self::LifecycleRecovery,
            "attributed-conflict" => Self::AttributedConflict,
            "attributed-recovery" => Self::AttributedRecovery,
            "alpha-flip-tangency" => Self::AlphaFlipTangency,
            "alpha-rejected-contact" => Self::AlphaRejectedContact,
            "alpha-recovery" => Self::AlphaRecovery,
            "nurbs-next-span" => Self::NurbsNextSpan,
            "nurbs-insert-knot" => Self::NurbsInsertKnot,
            "operation-split" => Self::OperationSplit,
            "operation-mirror" => Self::OperationMirror,
            "operation-pattern" => Self::OperationPattern,
            "topology-incomplete" => Self::TopologyMakeIncomplete,
            "topology-recover" => Self::TopologyRecover,
            "topology-cancel" => Self::TopologyCancel,
            "capture" => Self::CaptureEvidence,
            _ => return None,
        })
    }

    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::RoleConstruction => "role-construction",
            Self::RoleProfile => "role-profile",
            Self::SuppressDimension => "suppress",
            Self::ReactivateDimension => "reactivate",
            Self::ReferenceDimension => "reference",
            Self::HostInactive => "host-inactive",
            Self::MissingDependency => "missing-dependency",
            Self::ParameterValid => "parameter-valid",
            Self::ParameterInvalidKind => "parameter-invalid",
            Self::ParameterStale => "parameter-stale",
            Self::ParameterRecovery => "parameter-recovery",
            Self::ExternalMissing => "external-missing",
            Self::ExternalStale => "external-stale",
            Self::ExternalTopologyChange => "external-topology",
            Self::ExternalExplicitRebind => "external-rebind",
            Self::ExternalFreshRecovery => "external-fresh",
            Self::LifecycleRejected => "lifecycle-rejected",
            Self::LifecycleRecovery => "lifecycle-recovery",
            Self::AttributedConflict => "attributed-conflict",
            Self::AttributedRecovery => "attributed-recovery",
            Self::AlphaFlipTangency => "alpha-flip-tangency",
            Self::AlphaRejectedContact => "alpha-rejected-contact",
            Self::AlphaRecovery => "alpha-recovery",
            Self::NurbsNextSpan => "nurbs-next-span",
            Self::NurbsInsertKnot => "nurbs-insert-knot",
            Self::OperationSplit => "operation-split",
            Self::OperationMirror => "operation-mirror",
            Self::OperationPattern => "operation-pattern",
            Self::TopologyMakeIncomplete => "topology-incomplete",
            Self::TopologyRecover => "topology-recover",
            Self::TopologyCancel => "topology-cancel",
            Self::CaptureEvidence => "capture",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::RoleConstruction => "Use construction role",
            Self::RoleProfile => "Restore profile role",
            Self::SuppressDimension => "Suppress dimension",
            Self::ReactivateDimension => "Reactivate dimension",
            Self::ReferenceDimension => "Make dimension reference",
            Self::HostInactive => "Set host inactive",
            Self::MissingDependency => "Remove dependency",
            Self::ParameterValid => "Submit valid parameter",
            Self::ParameterInvalidKind => "Submit invalid kind",
            Self::ParameterStale => "Submit stale parameter",
            Self::ParameterRecovery => "Submit recovery parameter",
            Self::ExternalMissing => "Remove external snapshot",
            Self::ExternalStale => "Submit stale snapshot",
            Self::ExternalTopologyChange => "Change topology",
            Self::ExternalExplicitRebind => "Declare explicit rebind",
            Self::ExternalFreshRecovery => "Submit fresh snapshot",
            Self::LifecycleRejected => "Submit rejected attempt",
            Self::LifecycleRecovery => "Submit valid recovery",
            Self::AttributedConflict => "Create attributed conflict",
            Self::AttributedRecovery => "Recover as reference",
            Self::AlphaFlipTangency => "Flip tangent orientation",
            Self::AlphaRejectedContact => "Submit impossible contact",
            Self::AlphaRecovery => "Undo rejected contact",
            Self::NurbsNextSpan => "Advance periodic span",
            Self::NurbsInsertKnot => "Insert NURBS knot",
            Self::OperationSplit => "Split visible support",
            Self::OperationMirror => "Mirror exact source",
            Self::OperationPattern => "Create linear pattern",
            Self::TopologyMakeIncomplete => "Add open eligible support",
            Self::TopologyRecover => "Recover complete topology",
            Self::TopologyCancel => "Cancel topology query",
            Self::CaptureEvidence => "Capture typed evidence",
        }
    }

    pub(crate) const fn fixture(self) -> Option<ScenarioFixture> {
        Some(match self {
            Self::RoleConstruction
            | Self::RoleProfile
            | Self::SuppressDimension
            | Self::ReactivateDimension
            | Self::ReferenceDimension
            | Self::HostInactive
            | Self::MissingDependency => ScenarioFixture::RoleActivity,
            Self::ParameterValid
            | Self::ParameterInvalidKind
            | Self::ParameterStale
            | Self::ParameterRecovery => ScenarioFixture::ParameterProposal,
            Self::ExternalMissing
            | Self::ExternalStale
            | Self::ExternalTopologyChange
            | Self::ExternalExplicitRebind
            | Self::ExternalFreshRecovery => ScenarioFixture::ExternalRebind,
            Self::LifecycleRejected | Self::LifecycleRecovery => ScenarioFixture::LifecycleEvidence,
            Self::AttributedConflict | Self::AttributedRecovery => {
                ScenarioFixture::ErrorAttribution
            }
            Self::AlphaFlipTangency | Self::AlphaRejectedContact | Self::AlphaRecovery => {
                ScenarioFixture::AlphaBranchRecovery
            }
            Self::NurbsNextSpan | Self::NurbsInsertKnot => ScenarioFixture::NurbsBranches,
            Self::OperationSplit | Self::OperationMirror | Self::OperationPattern => {
                ScenarioFixture::Operations
            }
            Self::TopologyMakeIncomplete | Self::TopologyRecover | Self::TopologyCancel => {
                ScenarioFixture::ProductionTopology
            }
            Self::CaptureEvidence => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScenarioBoundary {
    RetainedAccepted,
    AdvancedAccepted,
    ExplicitDeclarationOnly,
    EvidenceCapture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScenarioRejection {
    StaleParameter { submitted: u64, retained: u64 },
    StaleExternal { submitted: u64, retained: u64 },
}

impl ScenarioRejection {
    fn summary(self) -> String {
        match self {
            Self::StaleParameter {
                submitted,
                retained,
            } => format!(
                "typed stale-parameter rejection: submitted revision {submitted}, retained revision {retained}"
            ),
            Self::StaleExternal {
                submitted,
                retained,
            } => format!(
                "typed stale-external rejection: submitted revision {submitted}, retained revision {retained}"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScenarioObservation {
    pub action: ScenarioAction,
    pub fixture: ScenarioFixture,
    pub boundary: ScenarioBoundary,
    pub accepted_before: String,
    pub accepted_after: String,
    pub accepted_evidence_before: String,
    pub accepted_evidence_after: String,
    pub rejection: Option<ScenarioRejection>,
}

impl ScenarioObservation {
    pub(crate) fn summary(&self) -> String {
        let boundary = format!(
            "{} · {} ({})",
            self.action.label(),
            self.fixture.label(),
            match self.boundary {
                ScenarioBoundary::RetainedAccepted => "accepted identity/evidence retained",
                ScenarioBoundary::AdvancedAccepted => "accepted state advanced",
                ScenarioBoundary::ExplicitDeclarationOnly => {
                    "explicit declaration changed; accepted state retained"
                }
                ScenarioBoundary::EvidenceCapture => "deterministic evidence refreshed",
            }
        );
        self.rejection
            .as_ref()
            .map_or(boundary.clone(), |rejection| {
                format!("{boundary}: {}", rejection.summary())
            })
    }
}

struct RoleFixture {
    coordinator: RetainedEditorCoordinator,
    role_curve: geosolve_sketch::CurveId,
    activity_dependency: geosolve_sketch::CurveId,
    mode_dimension: geosolve_sketch::DocumentDimensionId,
    activation_revision: u64,
}

struct ParameterFixture {
    coordinator: RetainedEditorCoordinator,
    parameter: geosolve_sketch::DocumentParameterId,
    last_submitted: ParameterBatch,
}

struct ExternalFixture {
    coordinator: RetainedEditorCoordinator,
    binding: geosolve_sketch::DocumentExternalBindingId,
    spare: geosolve_sketch::DocumentExternalBindingId,
    last_submitted: ExternalSnapshotSet,
}

struct LifecycleFixture {
    coordinator: RetainedEditorCoordinator,
    parameter: geosolve_sketch::DocumentParameterId,
}

struct ErrorAttributionFixture {
    coordinator: RetainedEditorCoordinator,
    dimension: geosolve_sketch::DocumentDimensionId,
}

struct AlphaBranchFixture {
    coordinator: RetainedEditorCoordinator,
    tangency: geosolve_sketch::DocumentConstraintId,
    impossible_lines: [CurveSpan; 2],
}

struct NurbsBranchFixture {
    coordinator: RetainedEditorCoordinator,
    curve: geosolve_sketch::CurveId,
    contact: geosolve_sketch::ContactId,
}

struct OperationFixture {
    coordinator: RetainedEditorCoordinator,
    source: geosolve_sketch::CurveId,
    axis: CurveSpan,
}

struct ProductionTopologyFixture {
    coordinator: RetainedEditorCoordinator,
}

#[derive(Clone, Copy)]
struct DragStability {
    driver: DesignPointId,
    passive: DesignPointId,
}

struct MotionFixture {
    coordinator: RetainedEditorCoordinator,
    drag_stability: Vec<DragStability>,
}

#[derive(Default)]
struct ScenarioTransition {
    explicit_declaration: bool,
    rejection: Option<ScenarioRejection>,
}

pub(crate) struct ScenarioCandidate {
    active: ScenarioFixture,
    role: Box<RoleFixture>,
    parameter: Box<ParameterFixture>,
    external: Box<ExternalFixture>,
    lifecycle: Box<LifecycleFixture>,
    error_attribution: Box<ErrorAttributionFixture>,
    alpha_parity: Box<RetainedEditorCoordinator>,
    alpha_branch: Box<AlphaBranchFixture>,
    advanced_gallery: Box<RetainedEditorCoordinator>,
    nurbs_branches: Box<NurbsBranchFixture>,
    operations: Box<OperationFixture>,
    production_topology: Box<ProductionTopologyFixture>,
    motion: Option<Box<MotionFixture>>,
    transcript: Vec<ScenarioObservation>,
    evidence_text: String,
}

impl ScenarioCandidate {
    pub(crate) fn new(active: ScenarioFixture) -> Result<Self, String> {
        let motion = active
            .motion_kind()
            .map(motion_fixture)
            .transpose()?
            .map(Box::new);
        Ok(Self {
            active,
            role: Box::new(role_fixture()?),
            parameter: Box::new(parameter_fixture("Scenario shared parameter", 2)?),
            external: Box::new(external_fixture()?),
            lifecycle: Box::new(lifecycle_fixture()?),
            error_attribution: Box::new(error_attribution_fixture()?),
            alpha_parity: Box::new(alpha_parity_fixture()?),
            alpha_branch: Box::new(alpha_branch_fixture()?),
            advanced_gallery: Box::new(advanced_gallery_fixture()?),
            nurbs_branches: Box::new(nurbs_branch_fixture()?),
            operations: Box::new(operation_fixture()?),
            production_topology: Box::new(production_topology_fixture()?),
            motion,
            transcript: Vec::new(),
            evidence_text: "Capture has not been requested.".into(),
        })
    }

    pub(crate) fn active_coordinator(&self) -> &RetainedEditorCoordinator {
        self.coordinator(self.active)
    }

    pub(crate) fn active_coordinator_mut(&mut self) -> &mut RetainedEditorCoordinator {
        self.coordinator_mut(self.active)
    }

    pub(crate) fn transcript(&self) -> &[ScenarioObservation] {
        &self.transcript
    }

    pub(crate) fn evidence_text(&self) -> &str {
        &self.evidence_text
    }

    fn coordinator(&self, fixture: ScenarioFixture) -> &RetainedEditorCoordinator {
        match fixture {
            ScenarioFixture::RoleActivity => &self.role.coordinator,
            ScenarioFixture::ParameterProposal => &self.parameter.coordinator,
            ScenarioFixture::ExternalRebind => &self.external.coordinator,
            ScenarioFixture::LifecycleEvidence => &self.lifecycle.coordinator,
            ScenarioFixture::ErrorAttribution => &self.error_attribution.coordinator,
            ScenarioFixture::AlphaParity => &self.alpha_parity,
            ScenarioFixture::AlphaBranchRecovery => &self.alpha_branch.coordinator,
            ScenarioFixture::AdvancedGallery => &self.advanced_gallery,
            ScenarioFixture::NurbsBranches => &self.nurbs_branches.coordinator,
            ScenarioFixture::Operations => &self.operations.coordinator,
            ScenarioFixture::ProductionTopology => &self.production_topology.coordinator,
            fixture if fixture.motion_kind().is_some() => self
                .motion
                .as_deref()
                .map(|fixture| &fixture.coordinator)
                .expect("active motion fixture exists"),
            _ => unreachable!("all scenario fixtures are matched"),
        }
    }

    fn coordinator_mut(&mut self, fixture: ScenarioFixture) -> &mut RetainedEditorCoordinator {
        match fixture {
            ScenarioFixture::RoleActivity => &mut self.role.coordinator,
            ScenarioFixture::ParameterProposal => &mut self.parameter.coordinator,
            ScenarioFixture::ExternalRebind => &mut self.external.coordinator,
            ScenarioFixture::LifecycleEvidence => &mut self.lifecycle.coordinator,
            ScenarioFixture::ErrorAttribution => &mut self.error_attribution.coordinator,
            ScenarioFixture::AlphaParity => &mut self.alpha_parity,
            ScenarioFixture::AlphaBranchRecovery => &mut self.alpha_branch.coordinator,
            ScenarioFixture::AdvancedGallery => &mut self.advanced_gallery,
            ScenarioFixture::NurbsBranches => &mut self.nurbs_branches.coordinator,
            ScenarioFixture::Operations => &mut self.operations.coordinator,
            ScenarioFixture::ProductionTopology => &mut self.production_topology.coordinator,
            fixture if fixture.motion_kind().is_some() => self
                .motion
                .as_deref_mut()
                .map(|fixture| &mut fixture.coordinator)
                .expect("active motion fixture exists"),
            _ => unreachable!("all scenario fixtures are matched"),
        }
    }

    pub(crate) fn resolve_projected_point_move(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        model_position: [f64; 2],
    ) -> Vec<geosolve_constraint_editor::EditorEffect> {
        let stability = self.motion.as_deref().and_then(|fixture| {
            fixture
                .drag_stability
                .iter()
                .find(|stability| stability.driver == point)
                .map(|stability| stability.passive)
        });
        let coordinator = self.active_coordinator_mut();
        match stability {
            Some(stability) => coordinator.resolve_projected_point_move_stabilizing(
                pointer_id,
                request_id,
                point,
                model_position,
                stability,
            ),
            None => coordinator.resolve_projected_point_move(
                pointer_id,
                request_id,
                point,
                model_position,
            ),
        }
    }

    pub(crate) fn perform(
        &mut self,
        action: ScenarioAction,
    ) -> Result<ScenarioObservation, String> {
        let fixture = action.fixture().unwrap_or(self.active);
        self.active = fixture;
        let before = accepted_stamp(self.coordinator(fixture));
        let evidence_before = accepted_evidence(self.coordinator(fixture));
        let transition = self.apply(action)?;
        if action == ScenarioAction::CaptureEvidence {
            self.evidence_text = self.capture_text()?;
        }
        let after = accepted_stamp(self.coordinator(fixture));
        let evidence_after = accepted_evidence(self.coordinator(fixture));
        let boundary = if action == ScenarioAction::CaptureEvidence {
            ScenarioBoundary::EvidenceCapture
        } else if before != after {
            ScenarioBoundary::AdvancedAccepted
        } else if transition.explicit_declaration {
            ScenarioBoundary::ExplicitDeclarationOnly
        } else {
            ScenarioBoundary::RetainedAccepted
        };
        let observation = ScenarioObservation {
            action,
            fixture,
            boundary,
            accepted_before: before,
            accepted_after: after,
            accepted_evidence_before: evidence_before,
            accepted_evidence_after: evidence_after,
            rejection: transition.rejection,
        };
        self.transcript.push(observation.clone());
        Ok(observation)
    }

    #[allow(clippy::too_many_lines)]
    fn apply(&mut self, action: ScenarioAction) -> Result<ScenarioTransition, String> {
        let mut transition = ScenarioTransition::default();
        match action {
            ScenarioAction::RoleConstruction | ScenarioAction::RoleProfile => {
                let expected = self.role.coordinator.session().design_identity();
                self.role
                    .coordinator
                    .set_geometry_role(
                        expected,
                        self.role.role_curve,
                        if action == ScenarioAction::RoleConstruction {
                            GeometryRole::Construction
                        } else {
                            GeometryRole::Profile
                        },
                    )
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::SuppressDimension | ScenarioAction::ReactivateDimension => {
                self.role
                    .coordinator
                    .editor_mut()
                    .set_selection([SelectionItem::Dimension(self.role.mode_dimension)]);
                let expected = self.role.coordinator.session().design_identity();
                self.role
                    .coordinator
                    .set_selected_suppressed(expected, action == ScenarioAction::SuppressDimension)
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::ReferenceDimension => {
                let expected = self.role.coordinator.session().design_identity();
                self.role
                    .coordinator
                    .set_dimension_mode(
                        expected,
                        self.role.mode_dimension,
                        DocumentDimensionMode::Reference,
                    )
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::HostInactive | ScenarioAction::MissingDependency => {
                self.role.activation_revision += 1;
                let override_value = if action == ScenarioAction::HostInactive {
                    HostActivationOverride::Inactive(DocumentElementId::Dimension(
                        self.role.mode_dimension,
                    ))
                } else {
                    HostActivationOverride::UnavailableExternalReference(DocumentElementId::Curve(
                        self.role.activity_dependency,
                    ))
                };
                let activation = HostConfigurationActivation::new(
                    self.role.activation_revision,
                    vec![override_value],
                )
                .map_err(|error| error.to_string())?;
                let expected = self.role.coordinator.session().design_identity();
                self.role
                    .coordinator
                    .apply_edit(
                        expected,
                        DocumentEdit::SetHostConfigurationActivation { activation },
                    )
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::ParameterValid
            | ScenarioAction::ParameterInvalidKind
            | ScenarioAction::ParameterStale
            | ScenarioAction::ParameterRecovery => {
                let (revision, value) = match action {
                    ScenarioAction::ParameterValid => (11, ParameterValue::Length(5.0)),
                    ScenarioAction::ParameterInvalidKind => (12, ParameterValue::Angle(5.0)),
                    ScenarioAction::ParameterStale => (1, ParameterValue::Length(5.0)),
                    ScenarioAction::ParameterRecovery => (13, ParameterValue::Length(6.0)),
                    _ => unreachable!(),
                };
                let batch = parameter_batch(self.parameter.parameter, revision, value)?;
                let expected = self.parameter.coordinator.session().design_identity();
                self.parameter.last_submitted = batch.clone();
                let result = self.parameter.coordinator.replace_parameter_batch(
                    expected,
                    batch,
                    DocumentSolveRequest::default(),
                );
                if action == ScenarioAction::ParameterStale {
                    match result {
                        Err(CoordinatorError::Session(
                            DocumentSessionError::StaleParameterRevision { actual, retained },
                        )) => {
                            transition.rejection = Some(ScenarioRejection::StaleParameter {
                                submitted: actual,
                                retained,
                            });
                        }
                        Err(error) => {
                            return Err(format!(
                                "expected typed stale-parameter rejection, received: {error}"
                            ));
                        }
                        Ok(_) => {
                            return Err("stale parameter candidate unexpectedly succeeded".into());
                        }
                    }
                } else {
                    result.map_err(|error| error.to_string())?;
                }
            }
            ScenarioAction::ExternalMissing
            | ScenarioAction::ExternalStale
            | ScenarioAction::ExternalTopologyChange
            | ScenarioAction::ExternalFreshRecovery => {
                let snapshots = match action {
                    ScenarioAction::ExternalMissing => {
                        line_snapshot(11, self.external.spare, TOPOLOGY_A)?
                    }
                    ScenarioAction::ExternalStale => {
                        line_snapshot(1, self.external.binding, TOPOLOGY_A)?
                    }
                    ScenarioAction::ExternalTopologyChange => {
                        line_snapshot(12, self.external.binding, TOPOLOGY_B)?
                    }
                    ScenarioAction::ExternalFreshRecovery => {
                        line_snapshot(13, self.external.binding, TOPOLOGY_B)?
                    }
                    _ => unreachable!(),
                };
                let expected = self.external.coordinator.session().design_identity();
                self.external.last_submitted = snapshots.clone();
                let result = self.external.coordinator.replace_external_snapshot_set(
                    expected,
                    snapshots,
                    DocumentSolveRequest::default(),
                );
                if action == ScenarioAction::ExternalStale {
                    match result {
                        Err(CoordinatorError::Session(
                            DocumentSessionError::StaleExternalSnapshotRevision {
                                actual,
                                retained,
                            },
                        )) => {
                            transition.rejection = Some(ScenarioRejection::StaleExternal {
                                submitted: actual,
                                retained,
                            });
                        }
                        Err(error) => {
                            return Err(format!(
                                "expected typed stale-external rejection, received: {error}"
                            ));
                        }
                        Ok(_) => {
                            return Err("stale external candidate unexpectedly succeeded".into());
                        }
                    }
                } else {
                    result.map_err(|error| error.to_string())?;
                }
            }
            ScenarioAction::ExternalExplicitRebind => {
                let expected = self.external.coordinator.session().design_identity();
                self.external
                    .coordinator
                    .rebind_external_binding(
                        expected,
                        self.external.binding,
                        ExternalFeatureKindV1::LineSegment,
                        Some(TOPOLOGY_B),
                    )
                    .map_err(|error| error.to_string())?;
                transition.explicit_declaration = true;
            }
            ScenarioAction::LifecycleRejected | ScenarioAction::LifecycleRecovery => {
                let (revision, value) = if action == ScenarioAction::LifecycleRejected {
                    (22, ParameterValue::Angle(4.0))
                } else {
                    (23, ParameterValue::Length(5.0))
                };
                let batch = parameter_batch(self.lifecycle.parameter, revision, value)?;
                let expected = self.lifecycle.coordinator.session().design_identity();
                self.lifecycle
                    .coordinator
                    .replace_parameter_batch(expected, batch, DocumentSolveRequest::default())
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::AttributedConflict | ScenarioAction::AttributedRecovery => {
                let expected = self
                    .error_attribution
                    .coordinator
                    .session()
                    .design_identity();
                self.error_attribution
                    .coordinator
                    .set_dimension_mode(
                        expected,
                        self.error_attribution.dimension,
                        if action == ScenarioAction::AttributedConflict {
                            DocumentDimensionMode::Driving
                        } else {
                            DocumentDimensionMode::Reference
                        },
                    )
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::AlphaFlipTangency => {
                self.alpha_branch
                    .coordinator
                    .editor_mut()
                    .set_selection([SelectionItem::Constraint(self.alpha_branch.tangency)]);
                let edits = self
                    .alpha_branch
                    .coordinator
                    .branch_actions()
                    .into_iter()
                    .map(|branch| {
                        let BranchAction::Contact(branch) = branch else {
                            return Err("tangency contact branch metadata is missing".to_owned());
                        };
                        let orientation = match branch.current.tangent_orientation {
                            Some(TangentOrientation::Aligned) => TangentOrientation::Opposed,
                            Some(TangentOrientation::Opposed) => TangentOrientation::Aligned,
                            None => {
                                return Err(
                                    "tangency contact orientation is unexpectedly absent".into()
                                );
                            }
                        };
                        let mut edit = geosolve_sketch::ContactBranchEdit {
                            tangent_orientation: Some(orientation),
                            ..branch.current
                        };
                        if let ContactDomain::Periodic { period } = edit.domain {
                            edit.value = (edit.value + period * 0.5).rem_euclid(period);
                        }
                        Ok(edit)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let expected = self.alpha_branch.coordinator.session().design_identity();
                self.alpha_branch
                    .coordinator
                    .set_contact_branches(expected, edits)
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::AlphaRejectedContact => {
                self.alpha_branch
                    .coordinator
                    .editor_mut()
                    .set_selection(self.alpha_branch.impossible_lines.map(SelectionItem::Curve));
                let expected = self.alpha_branch.coordinator.session().design_identity();
                let outcome = self
                    .alpha_branch
                    .coordinator
                    .apply_constraint_action(
                        expected,
                        ConstraintActionRequest {
                            intent: ConstraintIntent::Coincident,
                            label: "Scenario impossible contact".into(),
                            contacts: self
                                .alpha_branch
                                .impossible_lines
                                .into_iter()
                                .map(|span| ContactActionChoice {
                                    support: DocumentCurveSpanRef { span, winding: 0 },
                                    domain: ContactDomain::Bounded {
                                        lower: 0.0,
                                        upper: 1.0,
                                    },
                                    parameter: 0.5,
                                    neighborhood: ContactNeighborhood::Interior,
                                    tangent_orientation: None,
                                })
                                .collect(),
                            relation: None,
                        },
                    )
                    .map_err(|error| error.to_string())?;
                if outcome.published_accepted.is_some() {
                    return Err("impossible fixed contact unexpectedly accepted".into());
                }
            }
            ScenarioAction::AlphaRecovery => {
                for _ in 0..8 {
                    if self
                        .alpha_branch
                        .coordinator
                        .current_problem_metadata()
                        .is_none()
                    {
                        break;
                    }
                    if self.alpha_branch.coordinator.can_undo() {
                        self.alpha_branch
                            .coordinator
                            .undo()
                            .map_err(|error| error.to_string())?;
                    } else {
                        let expected = self.alpha_branch.coordinator.session().design_identity();
                        self.alpha_branch
                            .coordinator
                            .reattempt(expected)
                            .map_err(|error| error.to_string())?;
                        break;
                    }
                }
            }
            ScenarioAction::NurbsNextSpan => {
                let expected = self.nurbs_branches.coordinator.session().design_identity();
                self.nurbs_branches
                    .coordinator
                    .apply_edit(
                        expected,
                        DocumentEdit::TransitionNurbsContact {
                            contact: self.nurbs_branches.contact,
                            direction: DocumentBSplineSpanDirection::Next,
                        },
                    )
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::NurbsInsertKnot => {
                let expected = self.nurbs_branches.coordinator.session().design_identity();
                self.nurbs_branches
                    .coordinator
                    .apply_edit(
                        expected,
                        DocumentEdit::InsertNurbsKnot {
                            curve: self.nurbs_branches.curve,
                            parameter: 0.5,
                        },
                    )
                    .map_err(|error| error.to_string())?;
            }
            ScenarioAction::OperationSplit => {
                apply_scenario_operation(
                    &mut self.operations.coordinator,
                    SketchOperationRequest::Split {
                        support: CurveSpan::line(self.operations.source),
                        parameter: 0.5,
                        retained: SplitRetainedPiece::Before,
                    },
                )?;
            }
            ScenarioAction::OperationMirror => {
                apply_scenario_operation(
                    &mut self.operations.coordinator,
                    SketchOperationRequest::Mirror {
                        label: "UAT mirrored source".into(),
                        source: self.operations.source,
                        axis: self.operations.axis,
                    },
                )?;
            }
            ScenarioAction::OperationPattern => {
                apply_scenario_operation(
                    &mut self.operations.coordinator,
                    SketchOperationRequest::LinearPattern {
                        label: "UAT source pattern".into(),
                        sources: vec![self.operations.source],
                        instances: 3,
                        step: [0.0, 1.5],
                    },
                )?;
            }
            ScenarioAction::TopologyMakeIncomplete => {
                let mut session = self.production_topology.coordinator.session().clone();
                let expected = session.design_identity();
                session
                    .transact(expected, |document| {
                        let points = [
                            document.add_point("UAT open start", [18.0, -2.0])?,
                            document.add_point("UAT open end", [22.0, 1.0])?,
                        ];
                        document.add_curve(
                            "UAT open eligible support",
                            CurveDefinition::Line {
                                start: points[0],
                                end: points[1],
                                branch_direction: [0.8, 0.6],
                            },
                        )?;
                        Ok(())
                    })
                    .map_err(|error| error.to_string())?;
                self.production_topology.coordinator =
                    RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?;
            }
            ScenarioAction::TopologyRecover => {
                *self.production_topology = production_topology_fixture()?;
            }
            ScenarioAction::TopologyCancel => {
                let snapshot =
                    TopologySnapshot::capture(self.production_topology.coordinator.session())
                        .map_err(|error| error.to_string())?;
                let (handle, token) = cancellation_pair();
                handle.cancel();
                let outcome = snapshot
                    .prepare(TopologyRequest::default())
                    .execute(OperationControl::new(
                        token,
                        geosolve_sketch::OperationLimits::unlimited(),
                    ))
                    .map_err(|error| error.to_string())?;
                if !matches!(outcome, OperationOutcome::Cancelled { .. }) {
                    return Err("pre-cancelled topology query unexpectedly completed".into());
                }
            }
            ScenarioAction::CaptureEvidence => {}
        }
        Ok(transition)
    }

    fn capture_text(&self) -> Result<String, String> {
        let serialize = |label: &str, coordinator: &RetainedEditorCoordinator| {
            serialize_scenario_typed_host_evidence(
                coordinator,
                "SCENARIO-CATALOG-FIXED-CAPTURE",
                label,
                "geosolve-scenario-catalog",
                &host_state_markup(coordinator.session()),
            )
        };
        let submitted_parameter = parameter_batch_json(&self.parameter.last_submitted);
        let submitted_external = self
            .external
            .last_submitted
            .to_canonical_json()
            .map_err(|error| error.to_string())?;
        let scenario_transcript = self
            .transcript
            .iter()
            .filter(|observation| observation.action != ScenarioAction::CaptureEvidence)
            .map(ScenarioObservation::summary)
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!(
            "SCENARIO CATALOG EVIDENCE\nprovenance=fixed-scenario-not-runtime-platform\nobjective_checks=direct Rust/WASM state transitions\nhuman_clarity_and_trust=human-UAT judgment only\nSCENARIO_TRANSCRIPT\n{}\nACTIVE_RENDERED_SCENARIO\n{}\nROLE_ACTIVITY\n{}\nPARAMETER\n{}\nSUBMITTED_PARAMETER_TYPED\n{}\nEXTERNAL\n{}\nSUBMITTED_EXTERNAL_TYPED\n{}\nLIFECYCLE\n{}\nERROR_ATTRIBUTION\n{}\nALPHA_PARITY\n{}\nALPHA_BRANCH_RECOVERY\n{}\nADVANCED_ALL_FAMILIES\n{}\nNURBS_BRANCH_TOPOLOGY\n{}\nASSOCIATIVE_COMPANION_OPERATIONS\n{}\nPRODUCTION_TOPOLOGY\n{}\nPRODUCTION_TOPOLOGY_PRESENTATION\n{}",
            scenario_transcript,
            serialize("scenario://active-rendered", self.active_coordinator())?,
            serialize("scenario://role-activity", &self.role.coordinator)?,
            serialize(
                "scenario://parameter-binding-proposal",
                &self.parameter.coordinator
            )?,
            submitted_parameter,
            serialize("scenario://external-rebind", &self.external.coordinator)?,
            submitted_external,
            serialize("scenario://lifecycle-evidence", &self.lifecycle.coordinator)?,
            serialize(
                "scenario://canvas-error-attribution",
                &self.error_attribution.coordinator
            )?,
            serialize("scenario://alpha-parity", &self.alpha_parity)?,
            serialize(
                "scenario://alpha-branch-recovery",
                &self.alpha_branch.coordinator
            )?,
            serialize("scenario://advanced-all-families", &self.advanced_gallery)?,
            serialize(
                "scenario://nurbs-branch-topology",
                &self.nurbs_branches.coordinator
            )?,
            serialize(
                "scenario://associative-companion-operations",
                &self.operations.coordinator
            )?,
            serialize(
                "scenario://production-topology",
                &self.production_topology.coordinator
            )?,
            production_topology_markup(self.production_topology.coordinator.session()),
        ))
    }
}

fn apply_scenario_operation(
    coordinator: &mut RetainedEditorCoordinator,
    request: SketchOperationRequest,
) -> Result<(), String> {
    let outcome = SketchOperationSnapshot::capture(coordinator.session())
        .prepare(request)
        .execute(OperationControl::default())
        .map_err(|error| error.to_string())?;
    let OperationOutcome::Completed { value, .. } = outcome else {
        return Err("scenario operation did not complete".into());
    };
    let SketchOperationResult::Proposed(proposal) = value else {
        return Err(format!("scenario operation was not proposed: {value:?}"));
    };
    let mut session = coordinator.session().clone();
    proposal
        .apply(&mut session)
        .map_err(|error| error.to_string())?;
    *coordinator = RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?;
    Ok(())
}

fn role_fixture() -> Result<RoleFixture, String> {
    let mut document = fixed_document(8.0, 1)?;
    let rectangle = document
        .add_rectangle("Scenario role/activity", [0.0, 0.0], 4.0, 3.0)
        .map_err(|error| error.to_string())?;
    Ok(RoleFixture {
        coordinator: make_coordinator(
            document,
            ParameterBatch::default(),
            ExternalSnapshotSet::default(),
        )?,
        role_curve: rectangle.curves[2],
        activity_dependency: rectangle.curves[1],
        mode_dimension: rectangle.dimensions[1],
        activation_revision: 0,
    })
}

fn parameter_fixture(label: &str, namespace: u128) -> Result<ParameterFixture, String> {
    let mut document = fixed_document(8.0, namespace)?;
    let rectangle = document
        .add_rectangle(label, [0.0, 0.0], 4.0, 4.0)
        .map_err(|error| error.to_string())?;
    let parameter = document
        .add_parameter("Scenario shared size", DocumentParameterKind::Length)
        .map_err(|error| error.to_string())?;
    for dimension in rectangle.dimensions {
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(dimension),
            )
            .map_err(|error| error.to_string())?;
    }
    let target = document
        .add_scalar(
            "Scenario reported size",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .map_err(|error| error.to_string())?;
    let reference = document
        .add_dimension(
            "Scenario output proposal",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[2]),
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .map_err(|error| error.to_string())?;
    let output = document
        .add_parameter("Scenario output", DocumentParameterKind::Length)
        .map_err(|error| error.to_string())?;
    document
        .add_parameter_output(output, reference)
        .map_err(|error| error.to_string())?;
    let batch = parameter_batch(parameter, 10, ParameterValue::Length(4.0))?;
    Ok(ParameterFixture {
        coordinator: make_coordinator(document, batch.clone(), ExternalSnapshotSet::default())?,
        parameter,
        last_submitted: batch,
    })
}

fn lifecycle_fixture() -> Result<LifecycleFixture, String> {
    let fixture = parameter_fixture("Scenario lifecycle", 4)?;
    Ok(LifecycleFixture {
        coordinator: fixture.coordinator,
        parameter: fixture.parameter,
    })
}

fn error_attribution_fixture() -> Result<ErrorAttributionFixture, String> {
    let mut document = fixed_document(1.0, 5)?;
    let first = document
        .add_point("Error first", [0.0, 0.0])
        .map_err(|error| error.to_string())?;
    let second = document
        .add_point("Error second", [2.0, 0.0])
        .map_err(|error| error.to_string())?;
    let line = document
        .add_curve(
            "Error line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|error| error.to_string())?;
    for (label, point, target) in [
        ("Fix error first", first, [0.0, 0.0]),
        ("Fix error second", second, [2.0, 0.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|error| error.to_string())?;
    }
    let target = document
        .add_scalar(
            "Requested incompatible length",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    let dimension = document
        .add_dimension(
            "Error line length",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(line),
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .map_err(|error| error.to_string())?;
    Ok(ErrorAttributionFixture {
        coordinator: make_coordinator(
            document,
            ParameterBatch::default(),
            ExternalSnapshotSet::default(),
        )?,
        dimension,
    })
}

fn alpha_parity_fixture() -> Result<RetainedEditorCoordinator, String> {
    let mut fixture =
        alpha_scenario(AlphaScenarioKind::Corpus, 1.0).map_err(|error| error.to_string())?;
    add_contextual_constraint_examples(&mut fixture.document).map_err(|error| error.to_string())?;
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .map_err(|error| error.to_string())?;
    RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())
}

#[allow(
    clippy::too_many_lines,
    reason = "the three independent typed scenario examples are clearest as one fixture composition"
)]
fn add_contextual_constraint_examples(document: &mut SketchDocument) -> Result<(), DocumentError> {
    let direction_controls = [
        document.add_point("contextual direction A", [230.0, 0.0])?,
        document.add_point("contextual direction B", [231.0, 0.0])?,
        document.add_point("contextual direction C", [232.0, 0.0])?,
    ];
    let direction_curve = document.add_curve(
        "contextual direction curve",
        CurveDefinition::QuadraticBezier {
            controls: direction_controls,
        },
    )?;
    let line_points = [
        document.add_point("contextual direction line A", [230.0, -1.0])?,
        document.add_point("contextual direction line B", [232.0, -1.0])?,
    ];
    let direction_line = document.add_curve(
        "contextual direction line",
        CurveDefinition::Line {
            start: line_points[0],
            end: line_points[1],
            branch_direction: [1.0, 0.0],
        },
    )?;
    let direction_contact = document.add_curve_contact(
        "contextual direction contact",
        CurveSpan::line(direction_curve),
        0.5,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    document.add_constraint(
        "Parallel intent → tangent curve direction",
        DocumentConstraintDefinition::CurveDirection {
            line: CurveSpan::line(direction_line),
            curve_contact: direction_contact,
            relation: DocumentCurveDirectionRelation::Tangent {
                orientation: TangentOrientation::Aligned,
            },
        },
    )?;

    let mut curvature_curves = Vec::new();
    for offset in [0.0, 3.0] {
        let controls = [
            document.add_point("contextual curvature start", [240.0 + offset, 0.0])?,
            document.add_point("contextual curvature middle", [241.0 + offset, 1.0])?,
            document.add_point("contextual curvature end", [242.0 + offset, 0.0])?,
        ];
        curvature_curves.push(document.add_curve(
            "contextual equal curvature curve",
            CurveDefinition::QuadraticBezier { controls },
        )?);
    }
    let mut curvature_contacts = Vec::new();
    for curve in curvature_curves {
        curvature_contacts.push(document.add_curve_contact(
            "contextual curvature contact",
            CurveSpan::line(curve),
            0.5,
            0,
            ContactNeighborhood::Interior,
            None,
        )?);
    }
    document.add_constraint(
        "Equal intent → signed curve curvature",
        DocumentConstraintDefinition::EqualCurvature {
            first_contact: curvature_contacts[0],
            second_contact: curvature_contacts[1],
            relation: DocumentCurveCurvatureRelation::Signed,
        },
    )?;

    let continuity_points = [
        document.add_point("contextual continuity A", [250.0, 0.0])?,
        document.add_point("contextual continuity B", [251.0, 0.0])?,
        document.add_point("contextual continuity seam", [252.0, 0.0])?,
        document.add_point("contextual continuity D", [253.0, 0.0])?,
        document.add_point("contextual continuity E", [254.0, 0.0])?,
    ];
    let continuity_curves = [
        document.add_curve(
            "contextual continuity incoming",
            CurveDefinition::QuadraticBezier {
                controls: [
                    continuity_points[0],
                    continuity_points[1],
                    continuity_points[2],
                ],
            },
        )?,
        document.add_curve(
            "contextual continuity outgoing",
            CurveDefinition::QuadraticBezier {
                controls: [
                    continuity_points[2],
                    continuity_points[3],
                    continuity_points[4],
                ],
            },
        )?,
    ];
    let first = document.add_curve_contact(
        "contextual continuity incoming endpoint",
        CurveSpan::line(continuity_curves[0]),
        1.0,
        0,
        ContactNeighborhood::End,
        None,
    )?;
    let second = document.add_curve_contact(
        "contextual continuity outgoing endpoint",
        CurveSpan::line(continuity_curves[1]),
        0.0,
        0,
        ContactNeighborhood::Start,
        None,
    )?;
    document.add_constraint(
        "Continuity intent → G2 endpoint continuity",
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact: first,
            second_contact: second,
            continuity: DocumentCurveContinuity::G2,
        },
    )?;
    Ok(())
}

fn motion_fixture(kind: AlphaScenarioKind) -> Result<MotionFixture, String> {
    let fixture = alpha_scenario(kind, 1.0).map_err(|error| error.to_string())?;
    let (driver, drag_stability) = match &fixture.ids {
        AlphaScenarioIds::StressCompass(ids) => (ids.first_tip, Vec::new()),
        AlphaScenarioIds::StressBridge(ids) => (ids.left_seam, Vec::new()),
        AlphaScenarioIds::MotionCam(ids) => (
            ids.left_center,
            vec![
                DragStability {
                    driver: ids.left_center,
                    passive: ids.right_center,
                },
                DragStability {
                    driver: ids.right_center,
                    passive: ids.left_center,
                },
            ],
        ),
        AlphaScenarioIds::MotionOrbit(ids) => (ids.moving_center, Vec::new()),
        AlphaScenarioIds::MotionTrammel(ids) => (ids.horizontal_slider, Vec::new()),
        AlphaScenarioIds::MotionScotchYoke(ids) => (ids.crank_pin, Vec::new()),
        AlphaScenarioIds::MotionRotatingSquare(ids) => (ids.corners[1], Vec::new()),
        AlphaScenarioIds::MotionScissor(ids) => (ids.slider, Vec::new()),
        AlphaScenarioIds::MotionScissorTower(ids) => (ids.right_levels[0], Vec::new()),
        AlphaScenarioIds::MotionPeaucellier(ids) => (ids.input, Vec::new()),
        _ => return Err(format!("{} is not a motion fixture", kind.key())),
    };
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .map_err(|error| error.to_string())?;
    let mut coordinator =
        RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Point(driver)]);
    Ok(MotionFixture {
        coordinator,
        drag_stability,
    })
}

fn advanced_gallery_fixture() -> Result<RetainedEditorCoordinator, String> {
    let fixture = alpha_scenario(AlphaScenarioKind::ProfileAllFamilies, 1.0)
        .map_err(|error| error.to_string())?;
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .map_err(|error| error.to_string())?;
    RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())
}

fn nurbs_branch_fixture() -> Result<NurbsBranchFixture, String> {
    let fixture =
        alpha_scenario(AlphaScenarioKind::NurbsPeriodic, 1.0).map_err(|error| error.to_string())?;
    let AlphaScenarioIds::NurbsPeriodic(ids) = fixture.ids else {
        return Err("periodic NURBS scenario IDs are unavailable".into());
    };
    let contact = ids
        .contact
        .ok_or_else(|| "periodic NURBS scenario contact is unavailable".to_owned())?;
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .map_err(|error| error.to_string())?;
    Ok(NurbsBranchFixture {
        coordinator: RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?,
        curve: ids.curve,
        contact,
    })
}

fn operation_fixture() -> Result<OperationFixture, String> {
    let fixture = alpha_scenario(AlphaScenarioKind::M28TrimmedFillet, 1.0)
        .map_err(|error| error.to_string())?;
    let mut document = fixture.document;
    let source_points = [
        document
            .add_point("Operation source start", [12.0, 0.0])
            .map_err(|error| error.to_string())?,
        document
            .add_point("Operation source end", [16.0, 0.0])
            .map_err(|error| error.to_string())?,
    ];
    let source = document
        .add_curve(
            "Operation exact line source",
            CurveDefinition::Line {
                start: source_points[0],
                end: source_points[1],
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|error| error.to_string())?;
    let axis_points = [
        document
            .add_point("Operation mirror axis start", [10.0, -3.0])
            .map_err(|error| error.to_string())?,
        document
            .add_point("Operation mirror axis end", [10.0, 3.0])
            .map_err(|error| error.to_string())?,
    ];
    let axis = document
        .add_curve(
            "Operation mirror axis",
            CurveDefinition::Line {
                start: axis_points[0],
                end: axis_points[1],
                branch_direction: [0.0, 1.0],
            },
        )
        .map_err(|error| error.to_string())?;
    let session =
        RetainedSketchDocumentSession::new(document, fixture.request, SolverConfig::default())
            .map_err(|error| error.to_string())?;
    Ok(OperationFixture {
        coordinator: RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?,
        source,
        axis: CurveSpan::line(axis),
    })
}

fn production_topology_fixture() -> Result<ProductionTopologyFixture, String> {
    let fixture = alpha_scenario(AlphaScenarioKind::ProfileCurvedTopology, 1.0)
        .map_err(|error| error.to_string())?;
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .map_err(|error| error.to_string())?;
    Ok(ProductionTopologyFixture {
        coordinator: RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?,
    })
}

fn alpha_branch_fixture() -> Result<AlphaBranchFixture, String> {
    let fixture = alpha_scenario(AlphaScenarioKind::A3, 1.0).map_err(|error| error.to_string())?;
    let AlphaScenarioIds::A3(ids) = fixture.ids else {
        return Err("A3 scenario IDs are unavailable".into());
    };
    let mut document = fixture.document;
    let points = [
        document
            .add_point("Impossible contact A", [-2.0, 8.0])
            .map_err(|error| error.to_string())?,
        document
            .add_point("Impossible contact B", [2.0, 8.0])
            .map_err(|error| error.to_string())?,
        document
            .add_point("Impossible contact C", [-2.0, 10.0])
            .map_err(|error| error.to_string())?,
        document
            .add_point("Impossible contact D", [2.0, 10.0])
            .map_err(|error| error.to_string())?,
    ];
    let impossible_lines = [
        CurveSpan::line(
            document
                .add_curve(
                    "Impossible fixed line 1",
                    CurveDefinition::Line {
                        start: points[0],
                        end: points[1],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .map_err(|error| error.to_string())?,
        ),
        CurveSpan::line(
            document
                .add_curve(
                    "Impossible fixed line 2",
                    CurveDefinition::Line {
                        start: points[2],
                        end: points[3],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .map_err(|error| error.to_string())?,
        ),
    ];
    for point in points {
        let target = document
            .point(point)
            .ok_or_else(|| "scenario point disappeared".to_owned())?
            .position;
        document
            .add_constraint(
                format!("Fix impossible contact {point}"),
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|error| error.to_string())?;
    }
    let session =
        RetainedSketchDocumentSession::new(document, fixture.request, SolverConfig::default())
            .map_err(|error| error.to_string())?;
    Ok(AlphaBranchFixture {
        coordinator: RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?,
        tangency: ids.tangency,
        impossible_lines,
    })
}

fn external_fixture() -> Result<ExternalFixture, String> {
    let mut document = fixed_document(8.0, 3)?;
    let start = document
        .add_point("start", [0.0, 0.0])
        .map_err(|e| e.to_string())?;
    let end = document
        .add_point("end", [4.0, 0.0])
        .map_err(|e| e.to_string())?;
    let line = document
        .add_curve(
            "Scenario external line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|error| error.to_string())?;
    let binding = document
        .add_external_binding(
            "Scenario datum",
            ExternalFeatureKindV1::LineSegment,
            Some(TOPOLOGY_A),
        )
        .map_err(|error| error.to_string())?;
    let spare = document
        .add_external_binding(
            "Scenario spare",
            ExternalFeatureKindV1::LineSegment,
            Some(TOPOLOGY_A),
        )
        .map_err(|error| error.to_string())?;
    document
        .add_constraint(
            "Scenario external collinearity",
            DocumentConstraintDefinition::ExternalLineCollinear {
                line: DocumentLineSupportRef {
                    span: CurveSpan::line(line),
                    direction: DocumentDirectionSense::Forward,
                },
                external: DocumentExternalLineSupportRef {
                    binding,
                    direction: DocumentDirectionSense::Forward,
                },
            },
        )
        .map_err(|error| error.to_string())?;
    let accepted_snapshot = line_snapshot(10, binding, TOPOLOGY_A)?;
    Ok(ExternalFixture {
        coordinator: make_coordinator(
            document,
            ParameterBatch::default(),
            accepted_snapshot.clone(),
        )?,
        binding,
        spare,
        last_submitted: accepted_snapshot,
    })
}

fn fixed_document(model_scale: f64, fixture: u128) -> Result<SketchDocument, String> {
    let namespace = 0x5300_0000_0000_0000_0000_0000_0000_0000_u128 | fixture;
    SketchDocument::with_id(model_scale, DocumentId(PersistentId::from_u128(namespace)))
        .map_err(|error| error.to_string())
}

fn make_coordinator(
    document: SketchDocument,
    parameters: ParameterBatch,
    external: ExternalSnapshotSet,
) -> Result<RetainedEditorCoordinator, String> {
    let session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        parameters,
        external,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .map_err(|error| error.to_string())?;
    RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())
}

fn parameter_batch(
    parameter: geosolve_sketch::DocumentParameterId,
    revision: u64,
    value: ParameterValue,
) -> Result<ParameterBatch, String> {
    ParameterBatch::new(revision, vec![ParameterBatchEntry { parameter, value }])
        .map_err(|error| error.to_string())
}

fn line_snapshot(
    revision: u64,
    binding: geosolve_sketch::DocumentExternalBindingId,
    topology: ExternalTopologyDigest,
) -> Result<ExternalSnapshotSet, String> {
    ExternalSnapshotSet::new(
        revision,
        vec![ExternalSnapshotEntry {
            binding,
            source_revision: revision,
            source_digest: ExternalSnapshotDigest::from_bytes([0x5a; 32]),
            feature: ExternalSnapshotFeatureV1::LineSegment {
                start: [-1.0, 0.0],
                end: [6.0, 0.0],
                domain: [0.0, 1.0],
                orientation: ExternalLineOrientationV1::StartToEnd,
                scale: 1.0,
                topology_digest: topology,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 2,
                    control_count: 0,
                    span_count: 1,
                },
            },
        }],
    )
    .map_err(|error| error.to_string())
}

fn accepted_stamp(coordinator: &RetainedEditorCoordinator) -> String {
    coordinator.session().accepted_state().map_or_else(
        || "none".into(),
        |accepted| {
            format!(
                "{}@{}",
                accepted.identity().document(),
                accepted.identity().revision().get()
            )
        },
    )
}

fn accepted_evidence(coordinator: &RetainedEditorCoordinator) -> String {
    coordinator.session().accepted_state().map_or_else(
        || "no-accepted-evidence".into(),
        |accepted| {
            format!(
                "{}|parameter={}:{}|external={}:{}|proposals={:?}",
                accepted_stamp(coordinator),
                accepted.input().parameter_revision(),
                format_args!("{:?}", accepted.input().parameter_digest()),
                accepted.input().external_snapshot_set_revision(),
                format_args!("{:?}", accepted.input().external_snapshot_set_digest()),
                accepted.parameter_output_proposals(),
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{EditorProblemScope, EditorProblemTarget, SelectionItem};
    use geosolve_sketch::{CurveSpan, DocumentConstraintDefinition};

    use super::{
        ScenarioAction, ScenarioBoundary, ScenarioCandidate, ScenarioFixture, ScenarioRejection,
    };
    use crate::workbench::panels::{host_state_markup, production_topology_markup, tree_markup};

    #[test]
    fn alpha_catalog_contains_contextual_direction_curvature_and_continuity_examples() {
        let candidate = ScenarioCandidate::new(ScenarioFixture::AlphaParity).unwrap();
        let definitions = candidate
            .alpha_parity
            .session()
            .design_document()
            .constraints()
            .iter()
            .map(|constraint| &constraint.definition)
            .collect::<Vec<_>>();
        assert!(definitions.iter().any(|definition| matches!(
            definition,
            DocumentConstraintDefinition::CurveDirection { .. }
        )));
        assert!(definitions.iter().any(|definition| matches!(
            definition,
            DocumentConstraintDefinition::EqualCurvature { .. }
        )));
        assert!(definitions.iter().any(|definition| matches!(
            definition,
            DocumentConstraintDefinition::EndpointContinuity { .. }
        )));
    }

    #[test]
    fn scenario_candidate_directly_qualifies_role_activity_and_mode_distinctions() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        let role_curve = candidate.role.role_curve;
        let dependency = candidate.role.activity_dependency;
        let dimension = candidate.role.mode_dimension;
        let accepted_geometry = candidate
            .role
            .coordinator
            .session()
            .accepted_state()
            .unwrap()
            .solve_result()
            .geometry
            .clone();
        assert!(
            host_state_markup(candidate.role.coordinator.session())
                .contains(&format!("data-profile-curve=\"{role_curve}\""))
        );

        candidate.perform(ScenarioAction::RoleConstruction).unwrap();
        let construction = host_state_markup(candidate.role.coordinator.session());
        assert!(!construction.contains(&format!("data-profile-curve=\"{role_curve}\"")));
        assert!(construction.contains(&format!(
            "data-activity-element=\"{role_curve}\" data-activity-state=\"active\""
        )));
        assert_eq!(
            candidate
                .role
                .coordinator
                .session()
                .accepted_state()
                .unwrap()
                .solve_result()
                .geometry,
            accepted_geometry
        );
        candidate.perform(ScenarioAction::RoleProfile).unwrap();
        assert!(
            host_state_markup(candidate.role.coordinator.session())
                .contains(&format!("data-profile-curve=\"{role_curve}\""))
        );

        candidate
            .perform(ScenarioAction::SuppressDimension)
            .unwrap();
        assert!(host_state_markup(candidate.role.coordinator.session()).contains(&format!(
            "data-activity-element=\"{dimension}\" data-activity-state=\"inactive\" data-activity-reason=\"user-suppressed\""
        )));
        candidate
            .perform(ScenarioAction::ReactivateDimension)
            .unwrap();
        candidate
            .perform(ScenarioAction::ReferenceDimension)
            .unwrap();
        assert!(
            tree_markup(candidate.role.coordinator.session().design_document(), &[]).contains(
                &format!("data-persistent-id=\"{dimension}\" data-dimension-mode=\"reference\"")
            )
        );

        candidate.perform(ScenarioAction::HostInactive).unwrap();
        assert!(host_state_markup(candidate.role.coordinator.session()).contains(&format!(
            "data-activity-element=\"{dimension}\" data-activity-state=\"inactive\" data-activity-reason=\"host-configuration-inactive\""
        )));
        candidate
            .perform(ScenarioAction::MissingDependency)
            .unwrap();
        let unavailable = host_state_markup(candidate.role.coordinator.session());
        assert!(unavailable.contains(&format!(
            "data-activity-element=\"{dependency}\" data-activity-state=\"inactive\" data-activity-reason=\"unavailable-external-reference\""
        )));
        assert!(unavailable.contains(&format!(
            "data-activity-element=\"{dimension}\" data-activity-state=\"inactive\" data-activity-reason=\"unavailable-dependency\""
        )));
    }

    #[test]
    fn scenario_candidate_qualifies_targeted_and_global_error_recovery() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::ErrorAttribution).unwrap();
        let dimension = candidate.error_attribution.dimension;
        let accepted_before = candidate
            .error_attribution
            .coordinator
            .session()
            .accepted_state()
            .unwrap()
            .identity();

        let conflict = candidate
            .perform(ScenarioAction::AttributedConflict)
            .unwrap();
        assert_eq!(conflict.boundary, ScenarioBoundary::RetainedAccepted);
        let targeted = candidate
            .error_attribution
            .coordinator
            .current_problem_metadata()
            .unwrap();
        assert_eq!(targeted.scope, EditorProblemScope::Targeted);
        assert!(
            targeted
                .targets
                .contains(&EditorProblemTarget::Dimension(dimension))
        );
        assert_eq!(
            candidate
                .error_attribution
                .coordinator
                .session()
                .accepted_state()
                .unwrap()
                .identity(),
            accepted_before
        );

        assert_eq!(
            candidate
                .perform(ScenarioAction::AttributedRecovery)
                .unwrap()
                .boundary,
            ScenarioBoundary::AdvancedAccepted
        );
        assert!(
            candidate
                .error_attribution
                .coordinator
                .current_problem_metadata()
                .is_none()
        );

        candidate
            .perform(ScenarioAction::ParameterInvalidKind)
            .unwrap();
        let global = candidate
            .parameter
            .coordinator
            .current_problem_metadata()
            .unwrap();
        assert_eq!(global.scope, EditorProblemScope::Global);
        assert!(global.targets.is_empty());
        candidate
            .perform(ScenarioAction::ParameterRecovery)
            .unwrap();
        assert!(
            candidate
                .parameter
                .coordinator
                .current_problem_metadata()
                .is_none()
        );
    }

    #[test]
    fn alpha_branch_scenario_uses_typed_orientation_rejection_and_recovery() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::AlphaBranchRecovery).unwrap();
        let tangency = candidate.alpha_branch.tangency;
        candidate
            .alpha_branch
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::Constraint(tangency)]);
        let before = candidate.alpha_branch.coordinator.branch_actions();
        candidate
            .perform(ScenarioAction::AlphaFlipTangency)
            .unwrap();
        candidate
            .alpha_branch
            .coordinator
            .editor_mut()
            .set_selection([SelectionItem::Constraint(tangency)]);
        let after = candidate.alpha_branch.coordinator.branch_actions();
        assert_ne!(before, after);

        let accepted = candidate
            .alpha_branch
            .coordinator
            .session()
            .accepted_state()
            .unwrap()
            .identity();
        let rejected = candidate
            .perform(ScenarioAction::AlphaRejectedContact)
            .unwrap();
        assert_eq!(rejected.boundary, ScenarioBoundary::RetainedAccepted);
        assert_eq!(
            candidate
                .alpha_branch
                .coordinator
                .session()
                .accepted_state()
                .unwrap()
                .identity(),
            accepted
        );
        assert!(
            candidate
                .alpha_branch
                .coordinator
                .current_problem_metadata()
                .is_some()
        );
        candidate.perform(ScenarioAction::AlphaRecovery).unwrap();
        let remaining = candidate
            .alpha_branch
            .coordinator
            .current_problem_metadata();
        assert!(
            remaining.is_none(),
            "remaining={remaining:?}, history={}, can_undo={}",
            candidate.alpha_branch.coordinator.history_len(),
            candidate.alpha_branch.coordinator.can_undo(),
        );
    }

    #[test]
    fn parameter_and_lifecycle_transcript_retains_until_typed_recovery() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::ParameterProposal).unwrap();
        assert_eq!(
            candidate
                .perform(ScenarioAction::ParameterValid)
                .unwrap()
                .boundary,
            ScenarioBoundary::AdvancedAccepted
        );
        let accepted = candidate
            .parameter
            .coordinator
            .session()
            .accepted_state()
            .unwrap();
        assert_eq!(accepted.input().parameter_revision(), 11);
        assert_eq!(accepted.parameter_output_proposals().len(), 1);
        assert_eq!(
            host_state_markup(candidate.parameter.coordinator.session())
                .matches("data-binding-target-type=\"driving-dimension\"")
                .count(),
            2
        );
        let invalid = candidate
            .perform(ScenarioAction::ParameterInvalidKind)
            .unwrap();
        assert_eq!(invalid.boundary, ScenarioBoundary::RetainedAccepted);
        assert_eq!(
            invalid.accepted_evidence_before,
            invalid.accepted_evidence_after
        );
        let stale = candidate.perform(ScenarioAction::ParameterStale).unwrap();
        assert_eq!(stale.boundary, ScenarioBoundary::RetainedAccepted);
        assert_eq!(
            stale.rejection,
            Some(ScenarioRejection::StaleParameter {
                submitted: 1,
                retained: 12,
            })
        );
        assert_eq!(
            stale.accepted_evidence_before,
            stale.accepted_evidence_after
        );
        assert_eq!(
            candidate
                .perform(ScenarioAction::ParameterRecovery)
                .unwrap()
                .boundary,
            ScenarioBoundary::AdvancedAccepted
        );
        assert_eq!(
            candidate
                .parameter
                .coordinator
                .session()
                .accepted_state()
                .unwrap()
                .input()
                .parameter_revision(),
            13
        );

        let rejected = candidate
            .perform(ScenarioAction::LifecycleRejected)
            .unwrap();
        assert_eq!(rejected.boundary, ScenarioBoundary::RetainedAccepted);
        assert_eq!(
            candidate
                .perform(ScenarioAction::LifecycleRecovery)
                .unwrap()
                .boundary,
            ScenarioBoundary::AdvancedAccepted
        );
    }

    #[test]
    fn external_transcript_distinguishes_stale_declaration_and_repeated_rebind() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::ExternalRebind).unwrap();
        for action in [
            ScenarioAction::ExternalMissing,
            ScenarioAction::ExternalStale,
            ScenarioAction::ExternalTopologyChange,
        ] {
            let observation = candidate.perform(action).unwrap();
            assert_eq!(observation.boundary, ScenarioBoundary::RetainedAccepted);
            if action == ScenarioAction::ExternalStale {
                assert_eq!(
                    observation.rejection,
                    Some(ScenarioRejection::StaleExternal {
                        submitted: 1,
                        retained: 10,
                    })
                );
            }
            assert_eq!(
                observation.accepted_evidence_before,
                observation.accepted_evidence_after
            );
        }
        assert_eq!(
            candidate
                .perform(ScenarioAction::ExternalExplicitRebind)
                .unwrap()
                .boundary,
            ScenarioBoundary::ExplicitDeclarationOnly
        );
        assert_eq!(
            candidate
                .perform(ScenarioAction::ExternalFreshRecovery)
                .unwrap()
                .boundary,
            ScenarioBoundary::AdvancedAccepted
        );
        let repeated_rebind = candidate
            .perform(ScenarioAction::ExternalExplicitRebind)
            .unwrap();
        assert_eq!(repeated_rebind.boundary, ScenarioBoundary::AdvancedAccepted);
        assert_ne!(
            repeated_rebind.accepted_before,
            repeated_rebind.accepted_after
        );
    }

    #[test]
    fn advanced_nurbs_actions_preserve_explicit_branch_and_add_one_span() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::NurbsBranches).unwrap();
        let curve = candidate.nurbs_branches.curve;
        let contact = candidate.nurbs_branches.contact;
        let before_contact = candidate
            .nurbs_branches
            .coordinator
            .session()
            .design_document()
            .contact(contact)
            .cloned()
            .unwrap();
        let before_spans = candidate
            .nurbs_branches
            .coordinator
            .session()
            .design_document()
            .curve_spans(curve)
            .unwrap();

        assert_eq!(
            candidate
                .perform(ScenarioAction::NurbsNextSpan)
                .unwrap()
                .boundary,
            ScenarioBoundary::AdvancedAccepted
        );
        let transitioned = candidate
            .nurbs_branches
            .coordinator
            .session()
            .design_document()
            .contact(contact)
            .cloned()
            .unwrap();
        assert_ne!(transitioned.curve, before_contact.curve);
        assert_ne!(transitioned.winding, before_contact.winding);

        assert_eq!(
            candidate
                .perform(ScenarioAction::NurbsInsertKnot)
                .unwrap()
                .boundary,
            ScenarioBoundary::AdvancedAccepted
        );
        let document = candidate
            .nurbs_branches
            .coordinator
            .session()
            .design_document();
        assert_eq!(
            document.curve_spans(curve).unwrap().len(),
            before_spans.len() + 1
        );
        assert!(
            candidate
                .nurbs_branches
                .coordinator
                .session()
                .accepted_state()
                .is_some()
        );
    }

    #[test]
    fn companion_operation_actions_publish_through_accepted_session_boundary() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::Operations).unwrap();
        let source = candidate.operations.source;
        let initial_curves = candidate
            .operations
            .coordinator
            .session()
            .design_document()
            .curves()
            .len();

        candidate.perform(ScenarioAction::OperationSplit).unwrap();
        assert_eq!(
            candidate
                .operations
                .coordinator
                .session()
                .design_document()
                .visible_intervals(CurveSpan::line(source))
                .unwrap()
                .len(),
            2
        );
        candidate.perform(ScenarioAction::OperationMirror).unwrap();
        let after_mirror = candidate
            .operations
            .coordinator
            .session()
            .design_document()
            .curves()
            .len();
        assert!(after_mirror > initial_curves);
        candidate.perform(ScenarioAction::OperationPattern).unwrap();
        assert!(
            candidate
                .operations
                .coordinator
                .session()
                .design_document()
                .curves()
                .len()
                > after_mirror
        );
        assert!(
            candidate
                .operations
                .coordinator
                .session()
                .accepted_state()
                .is_some()
        );
    }

    #[test]
    fn production_topology_actions_fail_closed_cancel_without_mutation_and_recover() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::ProductionTopology).unwrap();
        let initial =
            production_topology_markup(candidate.production_topology.coordinator.session());
        assert!(initial.contains("data-topology-status=\"complete\""));
        assert!(initial.contains("data-production-regions=\"true\""));

        candidate
            .perform(ScenarioAction::TopologyMakeIncomplete)
            .unwrap();
        let incomplete =
            production_topology_markup(candidate.production_topology.coordinator.session());
        assert!(incomplete.contains("data-topology-status=\"skipped\""));
        assert!(!incomplete.contains("data-production-regions=\"true\""));
        let accepted_before_cancel = candidate
            .production_topology
            .coordinator
            .session()
            .accepted_state()
            .unwrap()
            .identity();
        let cancelled = candidate.perform(ScenarioAction::TopologyCancel).unwrap();
        assert_eq!(cancelled.boundary, ScenarioBoundary::RetainedAccepted);
        assert_eq!(
            candidate
                .production_topology
                .coordinator
                .session()
                .accepted_state()
                .unwrap()
                .identity(),
            accepted_before_cancel
        );

        candidate.perform(ScenarioAction::TopologyRecover).unwrap();
        let recovered =
            production_topology_markup(candidate.production_topology.coordinator.session());
        assert_eq!(recovered, initial);
    }

    #[test]
    fn scenario_candidate_evidence_is_deterministic_and_contains_typed_inputs() {
        let mut candidate = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        candidate
            .perform(ScenarioAction::ParameterInvalidKind)
            .unwrap();
        candidate.perform(ScenarioAction::ParameterStale).unwrap();
        candidate.perform(ScenarioAction::ExternalMissing).unwrap();
        candidate.perform(ScenarioAction::ExternalStale).unwrap();
        candidate
            .perform(ScenarioAction::LifecycleRejected)
            .unwrap();
        candidate.perform(ScenarioAction::CaptureEvidence).unwrap();
        let first = candidate.evidence_text.clone();
        candidate.perform(ScenarioAction::CaptureEvidence).unwrap();
        assert_eq!(candidate.evidence_text, first);
        let mut replay = ScenarioCandidate::new(ScenarioFixture::RoleActivity).unwrap();
        replay
            .perform(ScenarioAction::ParameterInvalidKind)
            .unwrap();
        replay.perform(ScenarioAction::ParameterStale).unwrap();
        replay.perform(ScenarioAction::ExternalMissing).unwrap();
        replay.perform(ScenarioAction::ExternalStale).unwrap();
        replay.perform(ScenarioAction::LifecycleRejected).unwrap();
        replay.perform(ScenarioAction::CaptureEvidence).unwrap();
        assert_eq!(replay.evidence_text, first);
        for expected in [
            "SCENARIO-CATALOG-FIXED-CAPTURE",
            "scenario://role-activity",
            "scenario://parameter-binding-proposal",
            "scenario://external-rebind",
            "\"kind\":\"angle\"",
            "SUBMITTED_PARAMETER_TYPED\n{\"revision\":1",
            "SUBMITTED_EXTERNAL_TYPED\n{\"version\":1,\"revision\":1",
            "typed stale-parameter rejection: submitted revision 1, retained revision 12",
            "typed stale-external rejection: submitted revision 1, retained revision 10",
            "external_snapshot_set",
            "design_identity",
            "design_revision",
            "accepted_audit",
            "attempted_audit",
            "human_clarity_and_trust=human-UAT judgment only",
            "scenario://alpha-parity",
            "scenario://alpha-branch-recovery",
            "scenario://advanced-all-families",
            "scenario://nurbs-branch-topology",
            "scenario://associative-companion-operations",
            "scenario://production-topology",
            "PRODUCTION_TOPOLOGY_PRESENTATION",
        ] {
            assert!(first.contains(expected), "missing evidence {expected}");
        }
        assert!(!first.contains("http://"));
        assert!(!first.contains("https://"));
        assert!(!first.contains("\"design_json\":"));
        assert!(!first.contains("\"accepted_json\":"));
        for coordinator in [
            &candidate.role.coordinator,
            &candidate.parameter.coordinator,
            &candidate.external.coordinator,
            &candidate.lifecycle.coordinator,
            &candidate.alpha_parity,
            &candidate.alpha_branch.coordinator,
            &candidate.advanced_gallery,
            &candidate.nurbs_branches.coordinator,
            &candidate.operations.coordinator,
            &candidate.production_topology.coordinator,
        ] {
            let checkpoint = coordinator.checkpoint();
            assert!(!first.contains(checkpoint.design_json()));
            if let Some(accepted_json) = checkpoint.accepted_json() {
                assert!(!first.contains(accepted_json));
            }
        }
    }
}
