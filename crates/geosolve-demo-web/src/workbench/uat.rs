// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use geosolve_constraint_editor::{RetainedEditorCoordinator, SelectionItem};
use geosolve_core::SolverConfig;
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentDirectionSense, DocumentEdit, DocumentElementId,
    DocumentExternalLineSupportRef, DocumentId, DocumentLineSupportRef, DocumentParameterKind,
    DocumentParameterTarget, DocumentSolveRequest, ExternalFeatureKindV1,
    ExternalLineOrientationV1, ExternalSnapshotDigest, ExternalSnapshotEntry,
    ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1, ExternalSnapshotSet,
    ExternalTopologyDigest, GeometryRole, HostActivationOverride, HostConfigurationActivation,
    ParameterBatch, ParameterBatchEntry, ParameterValue, PersistentId,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
};

use super::evidence::serialize_m52_typed_host_evidence;
use super::panels::host_state_markup;
use super::persistence::WorkspaceSnapshot;

const TOPOLOGY_A: ExternalTopologyDigest = ExternalTopologyDigest::from_bytes([0x41; 32]);
const TOPOLOGY_B: ExternalTopologyDigest = ExternalTopologyDigest::from_bytes([0x42; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UatFixture {
    RoleActivity,
    ParameterProposal,
    ExternalRebind,
    LifecycleEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UatAction {
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
    CaptureEvidence,
}

impl UatAction {
    pub(crate) fn from_browser_key(value: &str) -> Option<Self> {
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
            "capture" => Self::CaptureEvidence,
            _ => return None,
        })
    }

    const fn fixture(self) -> UatFixture {
        match self {
            Self::RoleConstruction
            | Self::RoleProfile
            | Self::SuppressDimension
            | Self::ReactivateDimension
            | Self::ReferenceDimension
            | Self::HostInactive
            | Self::MissingDependency => UatFixture::RoleActivity,
            Self::ParameterValid
            | Self::ParameterInvalidKind
            | Self::ParameterStale
            | Self::ParameterRecovery => UatFixture::ParameterProposal,
            Self::ExternalMissing
            | Self::ExternalStale
            | Self::ExternalTopologyChange
            | Self::ExternalExplicitRebind
            | Self::ExternalFreshRecovery => UatFixture::ExternalRebind,
            Self::LifecycleRejected | Self::LifecycleRecovery | Self::CaptureEvidence => {
                UatFixture::LifecycleEvidence
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UatBoundary {
    RetainedAccepted,
    AdvancedAccepted,
    ExplicitDeclarationOnly,
    EvidenceCapture,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UatObservation {
    pub action: UatAction,
    pub fixture: UatFixture,
    pub boundary: UatBoundary,
    pub accepted_before: String,
    pub accepted_after: String,
    pub accepted_evidence_before: String,
    pub accepted_evidence_after: String,
}

impl UatObservation {
    pub(crate) fn summary(&self) -> String {
        format!(
            "M52 UAT {:?}: {:?} ({})",
            self.fixture,
            self.action,
            match self.boundary {
                UatBoundary::RetainedAccepted => "accepted identity/evidence retained",
                UatBoundary::AdvancedAccepted => "typed recovery published accepted state",
                UatBoundary::ExplicitDeclarationOnly => {
                    "explicit declaration changed; accepted state retained"
                }
                UatBoundary::EvidenceCapture => "deterministic evidence refreshed",
            }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UatPoint {
    pub number: u8,
    pub instruction: &'static str,
    pub objective_check: &'static str,
    pub human_judgment: &'static str,
}

pub(crate) const UAT_POINTS: [UatPoint; 10] = [
    UatPoint {
        number: 1,
        instruction: "Use Construction, then Profile.",
        objective_check: "Role changes profile participation while geometry remains solver-active.",
        human_judgment: "M53 judges whether role and participation are clear.",
    },
    UatPoint {
        number: 2,
        instruction: "Use Suppress, Reactivate, then Reference.",
        objective_check: "Suppression/reactivation and driving/reference mode are separate typed transitions.",
        human_judgment: "M53 judges whether recovery is discoverable.",
    },
    UatPoint {
        number: 3,
        instruction: "Use Host inactive, then Missing dependency.",
        objective_check: "Typed inactivity reasons remain distinct and accepted geometry is not replaced.",
        human_judgment: "M53 judges whether the reasons read clearly.",
    },
    UatPoint {
        number: 4,
        instruction: "Use Parameter valid.",
        objective_check: "One shared typed input updates two bindings and accepted proposal provenance atomically.",
        human_judgment: "M53 judges ownership/proposal clarity.",
    },
    UatPoint {
        number: 5,
        instruction: "Use Invalid kind, Stale, then Recovery.",
        objective_check: "Invalid/stale input retains accepted evidence; complete typed recovery advances it.",
        human_judgment: "M53 judges recovery clarity.",
    },
    UatPoint {
        number: 6,
        instruction: "Use External missing, Stale, then Topology change.",
        objective_check: "All three typed failures retain accepted external evidence without repair.",
        human_judgment: "M53 judges stale-data trust.",
    },
    UatPoint {
        number: 7,
        instruction: "Use Explicit rebind, then Fresh recovery.",
        objective_check: "Declaration-only rebind retains accepted state; fresh compatible input advances it.",
        human_judgment: "M53 judges explicit ownership clarity.",
    },
    UatPoint {
        number: 8,
        instruction: "Use Lifecycle rejected, inspect tree/Problems/canvas, then Lifecycle recovery.",
        objective_check: "Design, latest attempt, and accepted identities remain separate; scene is accepted-only.",
        human_judgment: "M53 judges visual distinction.",
    },
    UatPoint {
        number: 9,
        instruction: "Use Capture evidence before reset.",
        objective_check: "Fixed-provenance text contains exact typed inputs and accepted/attempted evidence.",
        human_judgment: "M53 adds an OS screenshot only for a visual finding.",
    },
    UatPoint {
        number: 10,
        instruction: "Repeat role/activity and parameter/external recovery naturally.",
        objective_check: "The same typed state machine drives every transition; reload restores only ordinary persisted work.",
        human_judgment: "M53 alone judges overall coherence and trust.",
    },
];

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

pub(crate) struct UatCandidate {
    active: UatFixture,
    role: RoleFixture,
    parameter: ParameterFixture,
    external: ExternalFixture,
    lifecycle: LifecycleFixture,
    transcript: Vec<UatObservation>,
    evidence_text: String,
}

/// Crate-private isolation boundary between the ordinary workbench and the disposable UAT.
///
/// The sidecar owns only the candidate. Ordinary workspace state remains with the caller, and
/// this type is the single authority for UAT admission and persistence suppression.
pub(crate) struct UatWorkbenchState {
    candidate: Option<UatCandidate>,
}

impl UatWorkbenchState {
    pub(crate) const fn new() -> Self {
        Self { candidate: None }
    }

    pub(crate) fn load(&mut self) -> Result<(), String> {
        self.candidate = Some(UatCandidate::new()?);
        Ok(())
    }

    pub(crate) fn exit(&mut self) {
        self.candidate = None;
    }

    pub(crate) fn perform(&mut self, action: UatAction) -> Result<UatObservation, String> {
        self.candidate
            .as_mut()
            .ok_or_else(|| "Load the disposable M52 UAT candidate first".to_owned())?
            .perform(action)
    }

    pub(crate) const fn is_active(&self) -> bool {
        self.candidate.is_some()
    }

    pub(crate) fn coordinator_for_render<'a>(
        &'a self,
        ordinary: &'a RetainedEditorCoordinator,
    ) -> &'a RetainedEditorCoordinator {
        self.candidate
            .as_ref()
            .map_or(ordinary, UatCandidate::active_coordinator)
    }

    pub(crate) fn panel_markup(&self) -> Option<String> {
        self.candidate.as_ref().map(UatCandidate::panel_markup)
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
}

impl UatCandidate {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            active: UatFixture::RoleActivity,
            role: role_fixture()?,
            parameter: parameter_fixture("M52 shared parameter", 2)?,
            external: external_fixture()?,
            lifecycle: lifecycle_fixture()?,
            transcript: Vec::new(),
            evidence_text: "Capture has not been requested.".into(),
        })
    }

    pub(crate) fn active_coordinator(&self) -> &RetainedEditorCoordinator {
        self.coordinator(self.active)
    }

    fn coordinator(&self, fixture: UatFixture) -> &RetainedEditorCoordinator {
        match fixture {
            UatFixture::RoleActivity => &self.role.coordinator,
            UatFixture::ParameterProposal => &self.parameter.coordinator,
            UatFixture::ExternalRebind => &self.external.coordinator,
            UatFixture::LifecycleEvidence => &self.lifecycle.coordinator,
        }
    }

    pub(crate) fn perform(&mut self, action: UatAction) -> Result<UatObservation, String> {
        let fixture = action.fixture();
        self.active = fixture;
        let before = accepted_stamp(self.coordinator(fixture));
        let evidence_before = accepted_evidence(self.coordinator(fixture));
        let explicit_declaration = self.apply(action)?;
        if action == UatAction::CaptureEvidence {
            self.evidence_text = self.capture_text()?;
        }
        let after = accepted_stamp(self.coordinator(fixture));
        let evidence_after = accepted_evidence(self.coordinator(fixture));
        let boundary = if action == UatAction::CaptureEvidence {
            UatBoundary::EvidenceCapture
        } else if explicit_declaration {
            UatBoundary::ExplicitDeclarationOnly
        } else if before == after {
            UatBoundary::RetainedAccepted
        } else {
            UatBoundary::AdvancedAccepted
        };
        let observation = UatObservation {
            action,
            fixture,
            boundary,
            accepted_before: before,
            accepted_after: after,
            accepted_evidence_before: evidence_before,
            accepted_evidence_after: evidence_after,
        };
        self.transcript.push(observation.clone());
        Ok(observation)
    }

    #[allow(clippy::too_many_lines)]
    fn apply(&mut self, action: UatAction) -> Result<bool, String> {
        match action {
            UatAction::RoleConstruction | UatAction::RoleProfile => {
                let expected = self.role.coordinator.session().design_identity();
                self.role
                    .coordinator
                    .set_geometry_role(
                        expected,
                        self.role.role_curve,
                        if action == UatAction::RoleConstruction {
                            GeometryRole::Construction
                        } else {
                            GeometryRole::Profile
                        },
                    )
                    .map_err(|error| error.to_string())?;
            }
            UatAction::SuppressDimension | UatAction::ReactivateDimension => {
                self.role
                    .coordinator
                    .editor_mut()
                    .set_selection([SelectionItem::Dimension(self.role.mode_dimension)]);
                let expected = self.role.coordinator.session().design_identity();
                self.role
                    .coordinator
                    .set_selected_suppressed(expected, action == UatAction::SuppressDimension)
                    .map_err(|error| error.to_string())?;
            }
            UatAction::ReferenceDimension => {
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
            UatAction::HostInactive | UatAction::MissingDependency => {
                self.role.activation_revision += 1;
                let override_value = if action == UatAction::HostInactive {
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
            UatAction::ParameterValid
            | UatAction::ParameterInvalidKind
            | UatAction::ParameterStale
            | UatAction::ParameterRecovery => {
                let (revision, value) = match action {
                    UatAction::ParameterValid => (11, ParameterValue::Length(5.0)),
                    UatAction::ParameterInvalidKind => (12, ParameterValue::Angle(5.0)),
                    UatAction::ParameterStale => (1, ParameterValue::Length(5.0)),
                    UatAction::ParameterRecovery => (13, ParameterValue::Length(6.0)),
                    _ => unreachable!(),
                };
                let batch = parameter_batch(self.parameter.parameter, revision, value)?;
                let expected = self.parameter.coordinator.session().design_identity();
                let result = self.parameter.coordinator.replace_parameter_batch(
                    expected,
                    batch,
                    DocumentSolveRequest::default(),
                );
                if action != UatAction::ParameterStale {
                    result.map_err(|error| error.to_string())?;
                } else if result.is_ok() {
                    return Err("stale parameter candidate unexpectedly succeeded".into());
                }
            }
            UatAction::ExternalMissing
            | UatAction::ExternalStale
            | UatAction::ExternalTopologyChange
            | UatAction::ExternalFreshRecovery => {
                let snapshots = match action {
                    UatAction::ExternalMissing => {
                        line_snapshot(11, self.external.spare, TOPOLOGY_A)?
                    }
                    UatAction::ExternalStale => {
                        line_snapshot(1, self.external.binding, TOPOLOGY_A)?
                    }
                    UatAction::ExternalTopologyChange => {
                        line_snapshot(12, self.external.binding, TOPOLOGY_B)?
                    }
                    UatAction::ExternalFreshRecovery => {
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
                if action != UatAction::ExternalStale {
                    result.map_err(|error| error.to_string())?;
                } else if result.is_ok() {
                    return Err("stale external candidate unexpectedly succeeded".into());
                }
            }
            UatAction::ExternalExplicitRebind => {
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
                return Ok(true);
            }
            UatAction::LifecycleRejected | UatAction::LifecycleRecovery => {
                let (revision, value) = if action == UatAction::LifecycleRejected {
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
            UatAction::CaptureEvidence => {}
        }
        Ok(false)
    }

    fn capture_text(&self) -> Result<String, String> {
        let serialize = |label: &str, coordinator: &RetainedEditorCoordinator| {
            serialize_m52_typed_host_evidence(
                coordinator,
                "M52-UAT-FIXED-CAPTURE",
                label,
                "geosolve-m52-disposable-uat",
                &host_state_markup(coordinator.session()),
            )
        };
        let submitted_external = self
            .external
            .last_submitted
            .to_canonical_json()
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "M52 DISPOSABLE UAT EVIDENCE\nprovenance=fixed-candidate-not-runtime-platform\nobjective_checks=direct Rust/WASM state transitions\nhuman_clarity_and_trust=M53 judgment only\nPARAMETER\n{}\nEXTERNAL\n{}\nSUBMITTED_EXTERNAL_TYPED\n{}\nLIFECYCLE\n{}",
            serialize(
                "m52://parameter-binding-proposal",
                &self.parameter.coordinator
            )?,
            serialize("m52://external-rebind", &self.external.coordinator)?,
            submitted_external,
            serialize("m52://lifecycle-evidence", &self.lifecycle.coordinator)?,
        ))
    }

    pub(crate) fn panel_markup(&self) -> String {
        let mut instructions = String::new();
        for point in UAT_POINTS {
            use std::fmt::Write as _;
            let _ = write!(
                instructions,
                "<li><strong>{}.</strong> {} <span>{}</span></li>",
                point.number, point.instruction, point.human_judgment
            );
        }
        format!(
            concat!(
                "<h2>M52 disposable host-semantics UAT candidate</h2>",
                "<p><strong>Ephemeral:</strong> save is disabled while active. This sidecar never enters workspace/canonical persistence. Exit restores the pre-existing ordinary workspace; reload also restores only that workspace.</p>",
                "<div class=\"wb-uat-controls\">",
                "<button data-uat-action=\"role-construction\">Construction</button><button data-uat-action=\"role-profile\">Profile</button>",
                "<button data-uat-action=\"suppress\">Suppress</button><button data-uat-action=\"reactivate\">Reactivate</button><button data-uat-action=\"reference\">Reference</button>",
                "<button data-uat-action=\"host-inactive\">Host inactive</button><button data-uat-action=\"missing-dependency\">Missing dependency</button>",
                "<button data-uat-action=\"parameter-valid\">Parameter valid</button><button data-uat-action=\"parameter-invalid\">Invalid kind</button><button data-uat-action=\"parameter-stale\">Parameter stale</button><button data-uat-action=\"parameter-recovery\">Parameter recovery</button>",
                "<button data-uat-action=\"external-missing\">External missing</button><button data-uat-action=\"external-stale\">External stale</button><button data-uat-action=\"external-topology\">Topology change</button><button data-uat-action=\"external-rebind\">Explicit rebind</button><button data-uat-action=\"external-fresh\">Fresh recovery</button>",
                "<button data-uat-action=\"lifecycle-rejected\">Lifecycle rejected</button><button data-uat-action=\"lifecycle-recovery\">Lifecycle recovery</button>",
                "<button data-uat-action=\"capture\">Capture typed evidence</button><button data-uat-action=\"load\">Reset candidate</button><button data-uat-action=\"exit\">Exit UAT</button></div>",
                "<ol class=\"wb-uat-instructions\">{}</ol>",
                "<h3>Copyable fixed-provenance evidence</h3><pre class=\"wb-uat-evidence\">{}</pre>"
            ),
            instructions,
            escape(&self.evidence_text),
        )
    }
}

pub(crate) fn inactive_panel_markup() -> &'static str {
    ""
}

fn role_fixture() -> Result<RoleFixture, String> {
    let mut document = fixed_document(8.0, 1)?;
    let rectangle = document
        .add_rectangle("M52 role/activity", [0.0, 0.0], 4.0, 3.0)
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
        .add_parameter("M52 shared size", DocumentParameterKind::Length)
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
            "M52 reported size",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .map_err(|error| error.to_string())?;
    let reference = document
        .add_dimension(
            "M52 output proposal",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[2]),
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .map_err(|error| error.to_string())?;
    let output = document
        .add_parameter("M52 output", DocumentParameterKind::Length)
        .map_err(|error| error.to_string())?;
    document
        .add_parameter_output(output, reference)
        .map_err(|error| error.to_string())?;
    let batch = parameter_batch(parameter, 10, ParameterValue::Length(4.0))?;
    Ok(ParameterFixture {
        coordinator: make_coordinator(document, batch, ExternalSnapshotSet::default())?,
        parameter,
    })
}

fn lifecycle_fixture() -> Result<LifecycleFixture, String> {
    let fixture = parameter_fixture("M52 lifecycle", 4)?;
    Ok(LifecycleFixture {
        coordinator: fixture.coordinator,
        parameter: fixture.parameter,
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
            "M52 external line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|error| error.to_string())?;
    let binding = document
        .add_external_binding(
            "M52 datum",
            ExternalFeatureKindV1::LineSegment,
            Some(TOPOLOGY_A),
        )
        .map_err(|error| error.to_string())?;
    let spare = document
        .add_external_binding(
            "M52 spare",
            ExternalFeatureKindV1::LineSegment,
            Some(TOPOLOGY_A),
        )
        .map_err(|error| error.to_string())?;
    document
        .add_constraint(
            "M52 external collinearity",
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
    let namespace = 0x5200_0000_0000_0000_0000_0000_0000_0000_u128 | fixture;
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

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{UAT_POINTS, UatAction, UatBoundary, UatCandidate, UatWorkbenchState};
    use crate::workbench::panels::{host_state_markup, tree_markup};

    #[test]
    fn m52_candidate_composes_all_ten_preserved_points_without_persistence() {
        let mut candidate = UatCandidate::new().unwrap();
        assert_eq!(
            UAT_POINTS.map(|point| point.number),
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
        for point in UAT_POINTS {
            assert!(!point.objective_check.is_empty());
            assert!(point.human_judgment.contains("M53"));
        }
        for action in [
            UatAction::RoleConstruction,
            UatAction::RoleProfile,
            UatAction::SuppressDimension,
            UatAction::ReactivateDimension,
            UatAction::ReferenceDimension,
            UatAction::HostInactive,
            UatAction::MissingDependency,
            UatAction::ParameterValid,
            UatAction::ParameterInvalidKind,
            UatAction::ParameterStale,
            UatAction::ParameterRecovery,
            UatAction::ExternalMissing,
            UatAction::ExternalStale,
            UatAction::ExternalTopologyChange,
            UatAction::ExternalExplicitRebind,
            UatAction::ExternalFreshRecovery,
            UatAction::LifecycleRejected,
            UatAction::LifecycleRecovery,
            UatAction::CaptureEvidence,
        ] {
            candidate.perform(action).unwrap();
        }
        let panel = candidate.panel_markup();
        assert!(panel.contains("save is disabled while active"));
        assert!(panel.contains("M53 alone judges overall coherence and trust"));
        for forbidden in [
            "localStorage",
            "WorkspaceSnapshot",
            "storage_key",
            "canonical sketch JSON fixture",
        ] {
            assert!(!panel.contains(forbidden));
        }
    }

    #[test]
    fn m52_candidate_directly_qualifies_role_activity_and_mode_distinctions() {
        let mut candidate = UatCandidate::new().unwrap();
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

        candidate.perform(UatAction::RoleConstruction).unwrap();
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
        candidate.perform(UatAction::RoleProfile).unwrap();
        assert!(
            host_state_markup(candidate.role.coordinator.session())
                .contains(&format!("data-profile-curve=\"{role_curve}\""))
        );

        candidate.perform(UatAction::SuppressDimension).unwrap();
        assert!(host_state_markup(candidate.role.coordinator.session()).contains(&format!(
            "data-activity-element=\"{dimension}\" data-activity-state=\"inactive\" data-activity-reason=\"user-suppressed\""
        )));
        candidate.perform(UatAction::ReactivateDimension).unwrap();
        candidate.perform(UatAction::ReferenceDimension).unwrap();
        assert!(
            tree_markup(candidate.role.coordinator.session().design_document(), &[]).contains(
                &format!("data-persistent-id=\"{dimension}\" data-dimension-mode=\"reference\"")
            )
        );

        candidate.perform(UatAction::HostInactive).unwrap();
        assert!(host_state_markup(candidate.role.coordinator.session()).contains(&format!(
            "data-activity-element=\"{dimension}\" data-activity-state=\"inactive\" data-activity-reason=\"host-configuration-inactive\""
        )));
        candidate.perform(UatAction::MissingDependency).unwrap();
        let unavailable = host_state_markup(candidate.role.coordinator.session());
        assert!(unavailable.contains(&format!(
            "data-activity-element=\"{dependency}\" data-activity-state=\"inactive\" data-activity-reason=\"unavailable-external-reference\""
        )));
        assert!(unavailable.contains(&format!(
            "data-activity-element=\"{dimension}\" data-activity-state=\"inactive\" data-activity-reason=\"unavailable-dependency\""
        )));
    }

    #[test]
    fn m52_candidate_transcript_retains_and_advances_only_at_typed_recovery_boundaries() {
        let mut candidate = UatCandidate::new().unwrap();
        assert_eq!(
            candidate
                .perform(UatAction::ParameterValid)
                .unwrap()
                .boundary,
            UatBoundary::AdvancedAccepted
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
        let invalid = candidate.perform(UatAction::ParameterInvalidKind).unwrap();
        assert_eq!(invalid.boundary, UatBoundary::RetainedAccepted);
        assert_eq!(
            invalid.accepted_evidence_before,
            invalid.accepted_evidence_after
        );
        let stale = candidate.perform(UatAction::ParameterStale).unwrap();
        assert_eq!(stale.boundary, UatBoundary::RetainedAccepted);
        assert_eq!(
            stale.accepted_evidence_before,
            stale.accepted_evidence_after
        );
        assert_eq!(
            candidate
                .perform(UatAction::ParameterRecovery)
                .unwrap()
                .boundary,
            UatBoundary::AdvancedAccepted
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

        for action in [
            UatAction::ExternalMissing,
            UatAction::ExternalStale,
            UatAction::ExternalTopologyChange,
        ] {
            let observation = candidate.perform(action).unwrap();
            assert_eq!(observation.boundary, UatBoundary::RetainedAccepted);
            assert_eq!(
                observation.accepted_evidence_before,
                observation.accepted_evidence_after
            );
        }
        assert_eq!(
            candidate
                .perform(UatAction::ExternalExplicitRebind)
                .unwrap()
                .boundary,
            UatBoundary::ExplicitDeclarationOnly
        );
        assert_eq!(
            candidate
                .perform(UatAction::ExternalFreshRecovery)
                .unwrap()
                .boundary,
            UatBoundary::AdvancedAccepted
        );
        let rejected = candidate.perform(UatAction::LifecycleRejected).unwrap();
        assert_eq!(rejected.boundary, UatBoundary::RetainedAccepted);
        assert_eq!(
            candidate
                .perform(UatAction::LifecycleRecovery)
                .unwrap()
                .boundary,
            UatBoundary::AdvancedAccepted
        );
    }

    #[test]
    fn m52_candidate_evidence_is_deterministic_and_contains_typed_inputs() {
        let mut candidate = UatCandidate::new().unwrap();
        candidate.perform(UatAction::ParameterInvalidKind).unwrap();
        candidate.perform(UatAction::ExternalMissing).unwrap();
        candidate.perform(UatAction::LifecycleRejected).unwrap();
        candidate.perform(UatAction::CaptureEvidence).unwrap();
        let first = candidate.evidence_text.clone();
        candidate.perform(UatAction::CaptureEvidence).unwrap();
        assert_eq!(candidate.evidence_text, first);
        let mut replay = UatCandidate::new().unwrap();
        replay.perform(UatAction::ParameterInvalidKind).unwrap();
        replay.perform(UatAction::ExternalMissing).unwrap();
        replay.perform(UatAction::LifecycleRejected).unwrap();
        replay.perform(UatAction::CaptureEvidence).unwrap();
        assert_eq!(replay.evidence_text, first);
        for expected in [
            "M52-UAT-FIXED-CAPTURE",
            "m52://parameter-binding-proposal",
            "m52://external-rebind",
            "\"kind\":\"angle\"",
            "external_snapshot_set",
            "design_identity",
            "design_revision",
            "accepted_audit",
            "attempted_audit",
            "human_clarity_and_trust=M53 judgment only",
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
        ] {
            let checkpoint = coordinator.checkpoint();
            assert!(!first.contains(checkpoint.design_json()));
            if let Some(accepted_json) = checkpoint.accepted_json() {
                assert!(!first.contains(accepted_json));
            }
        }
    }

    #[test]
    fn m52_sidecar_isolates_the_production_workspace_snapshot_flow() {
        let ordinary = super::make_coordinator(
            super::fixed_document(8.0, 99).unwrap(),
            geosolve_sketch::ParameterBatch::default(),
            geosolve_sketch::ExternalSnapshotSet::default(),
        )
        .unwrap();
        let mut sidecar = UatWorkbenchState::new();
        let before = sidecar.persistence_snapshot(&ordinary).unwrap();
        let ordinary_checkpoint = ordinary.checkpoint();

        sidecar.load().unwrap();
        assert!(sidecar.is_active());
        assert_ne!(
            sidecar
                .coordinator_for_render(&ordinary)
                .session()
                .design_identity(),
            ordinary.session().design_identity()
        );
        sidecar.perform(UatAction::ParameterInvalidKind).unwrap();
        sidecar.perform(UatAction::ExternalMissing).unwrap();
        sidecar.perform(UatAction::LifecycleRejected).unwrap();
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
            assert!(!sidecar.ordinary_action_allowed(action));
        }
        assert!(sidecar.ordinary_action_allowed("problems"));
        assert!(sidecar.persistence_snapshot(&ordinary).is_none());
        let unchanged_checkpoint = ordinary.checkpoint();
        assert_eq!(
            unchanged_checkpoint.design_json(),
            ordinary_checkpoint.design_json()
        );
        assert_eq!(
            unchanged_checkpoint.accepted_json(),
            ordinary_checkpoint.accepted_json()
        );
        assert_eq!(
            unchanged_checkpoint.revisions(),
            ordinary_checkpoint.revisions()
        );

        sidecar.exit();
        assert!(!sidecar.is_active());
        let after = sidecar.persistence_snapshot(&ordinary).unwrap();
        assert_eq!(after, before);

        let reloaded =
            crate::workbench::persistence::WorkspaceSnapshot::decode(&after.encode().unwrap())
                .unwrap();
        assert_eq!(reloaded, before);
    }
}
