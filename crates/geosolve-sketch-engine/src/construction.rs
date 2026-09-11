// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordinary retained construction through existing headless drafting and source insertion.

use geosolve_constraint_editor::{
    AdvancedConstructionKind, ConicConstructionOptions, ConstructionPreview,
    ConstructionPreviewGeometry, DraftAuthoringInput, DraftGuideGeometry, DraftInferenceInput,
    EditorEffect, GeometryDraftIssue, GeometryToolVariant, Modifiers, NurbsConstructionOptions,
    PointerInput, ProjectionalEditorSession, Viewport,
};
use geosolve_sketch::{
    DocumentArcSweep, DocumentBSplineForm, DocumentHyperbolaBranch, GeometryRole,
};
use geosolve_sketch_code::{
    CodeProject, CodeSessionIdentity, EditorBootstrapDeclaration, ManagedDeclarationDraft,
    ManagedSketchMutation, PreparedManagedMutationReceipt, PreparedManagedMutationRequest,
    SemanticSymbol, prepare_editor_declaration_insertions,
};
use serde::{Deserialize, Serialize};

use crate::terminal::PreparedDeclarationLabelProjection;
use crate::{AcceptedEvaluation, EditableSession, EngineError, PreparedAuthoringMutation};

pub const MAX_CONSTRUCTION_SAMPLES: usize = 4096;
// Matches the retained WASM construction reservation; option arrays cannot
// multiply one reserved trace into MAX_CONSTRUCTION_SAMPLES independent arrays.
const MAX_CONSTRUCTION_TRACE_BYTES: usize = 1024 * 1024;
const CHORD_TOLERANCE_PIXELS: f64 = 0.25;
fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstructionTool {
    SketchPoint,
    Segment,
    Polyline,
    MidpointLine,
    TwoPointAlignedRectangle,
    ThreePointCornerRectangle,
    CenterRectangle,
    ThreePointCenterRectangle,
    CenterRadiusCircle,
    TwoPointDiameterCircle,
    ThreePointCircle,
    CenterArc,
    ThreePointArc,
    TangentArc,
    CenterAxesEllipse,
    AxisEndpointsEllipse,
    CenterAxesEllipticalArc,
    AxisEndpointsEllipticalArc,
    QuadraticBezier,
    CubicBezier,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    OpenControlNurbs,
    PeriodicControlNurbs,
}
impl ConstructionTool {
    /// Every existing native construction recipe in palette order.
    pub const ALL: [Self; 25] = [
        Self::SketchPoint,
        Self::Segment,
        Self::Polyline,
        Self::MidpointLine,
        Self::TwoPointAlignedRectangle,
        Self::ThreePointCornerRectangle,
        Self::CenterRectangle,
        Self::ThreePointCenterRectangle,
        Self::CenterRadiusCircle,
        Self::TwoPointDiameterCircle,
        Self::ThreePointCircle,
        Self::CenterArc,
        Self::ThreePointArc,
        Self::TangentArc,
        Self::CenterAxesEllipse,
        Self::AxisEndpointsEllipse,
        Self::CenterAxesEllipticalArc,
        Self::AxisEndpointsEllipticalArc,
        Self::QuadraticBezier,
        Self::CubicBezier,
        Self::RationalQuadraticConic,
        Self::Parabola,
        Self::Hyperbola,
        Self::OpenControlNurbs,
        Self::PeriodicControlNurbs,
    ];
    /// Exact native recipe; no legacy family coalescing.
    #[must_use]
    pub const fn variant(self) -> GeometryToolVariant {
        match self {
            Self::SketchPoint => GeometryToolVariant::SketchPoint,
            Self::Segment => GeometryToolVariant::Segment,
            Self::Polyline => GeometryToolVariant::Polyline,
            Self::MidpointLine => GeometryToolVariant::MidpointLine,
            Self::TwoPointAlignedRectangle => GeometryToolVariant::TwoPointAlignedRectangle,
            Self::ThreePointCornerRectangle => GeometryToolVariant::ThreePointCornerRectangle,
            Self::CenterRectangle => GeometryToolVariant::CenterRectangle,
            Self::ThreePointCenterRectangle => GeometryToolVariant::ThreePointCenterRectangle,
            Self::CenterRadiusCircle => GeometryToolVariant::CenterRadiusCircle,
            Self::TwoPointDiameterCircle => GeometryToolVariant::TwoPointDiameterCircle,
            Self::ThreePointCircle => GeometryToolVariant::ThreePointCircle,
            Self::CenterArc => GeometryToolVariant::CenterArc,
            Self::ThreePointArc => GeometryToolVariant::ThreePointArc,
            Self::TangentArc => GeometryToolVariant::TangentArc,
            Self::CenterAxesEllipse => GeometryToolVariant::CenterAxesEllipse,
            Self::AxisEndpointsEllipse => GeometryToolVariant::AxisEndpointsEllipse,
            Self::CenterAxesEllipticalArc => GeometryToolVariant::CenterAxesEllipticalArc,
            Self::AxisEndpointsEllipticalArc => GeometryToolVariant::AxisEndpointsEllipticalArc,
            Self::QuadraticBezier => GeometryToolVariant::QuadraticBezier,
            Self::CubicBezier => GeometryToolVariant::CubicBezier,
            Self::RationalQuadraticConic => GeometryToolVariant::RationalQuadraticConic,
            Self::Parabola => GeometryToolVariant::Parabola,
            Self::Hyperbola => GeometryToolVariant::Hyperbola,
            Self::OpenControlNurbs => GeometryToolVariant::OpenControlNurbs,
            Self::PeriodicControlNurbs => GeometryToolVariant::PeriodicControlNurbs,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConstructionEvent {
    Viewport {
        viewport: Viewport,
    },
    Move {
        position: [f64; 2],
        suppressed: bool,
        regularized: bool,
    },
    Click {
        position: [f64; 2],
        suppressed: bool,
        regularized: bool,
    },
    Complete,
    StepBack,
    Reset,
    FlipBranch,
    CycleInference,
    ConicOptions {
        #[serde(with = "ConicOptionsWire")]
        options: ConicConstructionOptions,
    },
    NurbsOptions {
        #[serde(with = "NurbsOptionsWire")]
        options: NurbsConstructionOptions,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionSample {
    pub sequence: u32,
    pub input: ConstructionEvent,
}

// Serde-only projections retain the existing native option and curve-kind owners.
#[derive(Serialize, Deserialize)]
#[serde(remote = "ConicConstructionOptions", deny_unknown_fields)]
struct ConicOptionsWire {
    minor_axis_ratio: f64,
    arc_start: f64,
    arc_end: f64,
    arc_sweep: DocumentArcSweep,
    middle_weight: f64,
    trim_start: f64,
    trim_end: f64,
    semi_conjugate: f64,
    hyperbola_branch: DocumentHyperbolaBranch,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "NurbsConstructionOptions", deny_unknown_fields)]
struct NurbsOptionsWire {
    form: DocumentBSplineForm,
    degree: u32,
    weights: Vec<f64>,
    gauge_index: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "AdvancedConstructionKind", rename_all = "snake_case")]
enum CurveKindWire {
    QuadraticBezier,
    CubicBezier,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    Nurbs,
}

/// Semantic replay input; expected declarations authenticate resolved operands/branches,
/// not a client success claim. Server recomputes all geometry and source instructions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionCommand {
    pub basis: String,
    pub gesture_id: u64,
    pub viewport: Viewport,
    pub tool: ConstructionTool,
    pub role: GeometryRole,
    pub samples: Vec<ConstructionSample>,
    pub expected_declarations: Vec<ManagedDeclarationDraft>,
}

/// Equation-free model coordinates resolved by the shared construction/inference owner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConstructionGuide {
    Point {
        position: [f64; 2],
    },
    Polyline {
        points: Vec<[f64; 2]>,
        closed: bool,
    },
    Rectangle {
        first: [f64; 2],
        second: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    ArcRadius {
        center: [f64; 2],
        start: [f64; 2],
    },
    EllipticalArcSupport {
        center: [f64; 2],
        major_axis_point: [f64; 2],
        support_points: Vec<[f64; 2]>,
        trim_start: Option<[f64; 2]>,
    },
    ControlPolygon {
        #[serde(with = "CurveKindWire")]
        curve_kind: AdvancedConstructionKind,
        points: Vec<[f64; 2]>,
    },
    CircularArc {
        center: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
        radius: f64,
        sweep_radians: f64,
        large_arc: bool,
        sweep: DocumentArcSweep,
    },
    AdvancedCurve {
        #[serde(with = "CurveKindWire")]
        curve_kind: AdvancedConstructionKind,
        control_points: Vec<[f64; 2]>,
        curve_points: Vec<[f64; 2]>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent native finish, sweep and inference capabilities accompany the completion state in the wire DTO"
)]
pub struct ConstructionFrame {
    pub sequence: u32,
    pub completed: bool,
    pub can_finish: bool,
    pub has_pending: bool,
    pub can_reset: bool,
    pub can_step_back: bool,
    pub can_cycle_inference: bool,
    pub can_flip_branch: bool,
    pub stage: Option<String>,
    #[serde(with = "ConicOptionsWire")]
    pub conic_options: ConicConstructionOptions,
    #[serde(with = "NurbsOptionsWire")]
    pub nurbs_options: NurbsConstructionOptions,
    pub preview: Option<ConstructionGuide>,
    pub inference_guides: Vec<ConstructionGuide>,
    pub adjusted_position: Option<[f64; 2]>,
    pub diagnostic: Option<String>,
}

#[derive(Debug)]
pub struct ConstructionPrediction {
    origin: AcceptedEvaluation,
    project: CodeProject,
    editor: Box<ProjectionalEditorSession>,
    command: ConstructionCommand,
    viewport: Viewport,
    preview: Option<ConstructionGuide>,
    completed: bool,
    preferred_candidate: Option<geosolve_constraint_editor::DraftInferenceCandidateId>,
    last_pointer: Option<[f64; 2]>,
    diagnostic: Option<String>,
    sample_bytes: usize,
    circle_authoring_points: Vec<[f64; 2]>,
}
#[derive(Debug)]
pub struct ConstructionTerminal {
    command: ConstructionCommand,
    editor: Box<ProjectionalEditorSession>,
    declarations: Vec<EditorBootstrapDeclaration>,
    high_water: u64,
}
impl ConstructionTerminal {
    pub fn command(&self) -> &ConstructionCommand {
        &self.command
    }
}

/// Exact server replay plus one compiler ticket, held privately until resolution.
#[derive(Debug)]
pub struct PreparedConstruction {
    expected: CodeSessionIdentity,
    mutation: PreparedAuthoringMutation,
    editor: Box<ProjectionalEditorSession>,
    projections: Vec<PreparedDeclarationLabelProjection>,
}
impl PreparedConstruction {
    pub fn request(&self) -> &PreparedManagedMutationRequest {
        self.mutation.request()
    }
    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &SemanticSymbol> {
        self.projections
            .iter()
            .map(|projection| &projection.declaration)
    }
}
#[derive(Debug)]
pub struct PreparedConstructionCommit {
    expected: CodeSessionIdentity,
    candidate: EditableSession,
    project: String,
    design: crate::EditableDesign,
    digest: String,
    declarations: Vec<SemanticSymbol>,
}
impl PreparedConstructionCommit {
    pub fn result(&self) -> &crate::EngineAcceptedResult {
        self.candidate.accepted().result()
    }
    pub fn project_json(&self) -> &str {
        &self.project
    }
    pub fn design(&self) -> &crate::EditableDesign {
        &self.design
    }
    pub fn source_design_digest(&self) -> &str {
        &self.digest
    }
    pub fn declarations(&self) -> &[SemanticSymbol] {
        &self.declarations
    }
}

impl EditableSession {
    /// Forks accepted native authority for one ordinary construction without changing
    /// accepted navigation, source, design, history or lifecycle allocation.
    ///
    /// # Errors
    /// Rejects invalid gesture identity/camera or missing accepted native authority.
    pub fn begin_construction(
        &self,
        tool: ConstructionTool,
        gesture_id: u64,
        viewport: Viewport,
        role: GeometryRole,
    ) -> Result<ConstructionPrediction, EngineError> {
        Viewport::new(
            viewport.screen_size,
            viewport.model_center,
            viewport.pixels_per_model_unit,
        )
        .map_err(error)?;
        if gesture_id == 0 || gesture_id > geosolve_sketch_code::MAX_CODE_SESSION_WIRE_INTEGER {
            return Err(error("invalid construction gesture identity"));
        }
        let project = self
            .code_snapshot()
            .code_project
            .clone()
            .ok_or_else(|| error("missing accepted project"))?;
        let mut editor = Box::new(
            self.accepted()
                .0
                .materialized
                .editor
                .fork_accepted_authority()
                .map_err(error)?,
        );
        editor.editor_mut().activate_geometry_tool(tool.variant());
        editor.editor_mut().set_authoring_geometry_role(role);
        Ok(ConstructionPrediction {
            origin: self.accepted().clone(),
            project,
            editor,
            command: ConstructionCommand {
                basis: self.source_design_digest()?,
                gesture_id,
                viewport,
                tool,
                role,
                samples: Vec::new(),
                expected_declarations: Vec::new(),
            },
            viewport,
            preview: None,
            completed: false,
            preferred_candidate: None,
            last_pointer: None,
            diagnostic: None,
            sample_bytes: 0,
            circle_authoring_points: Vec::new(),
        })
    }

    /// Server-owned exact-basis replay. Resolved source references and explicit branch
    /// semantics must match the client command before a compiler request can be prepared.
    ///
    /// # Errors
    /// Rejects stale basis, invalid event order, unfinished construction or changed operands.
    pub fn prepare_construction(
        &self,
        command: &ConstructionCommand,
    ) -> Result<PreparedConstruction, EngineError> {
        let terminal = self.replay_construction(command)?;
        self.prepare_construction_terminal(terminal)
    }

    fn replay_construction(
        &self,
        command: &ConstructionCommand,
    ) -> Result<ConstructionTerminal, EngineError> {
        if command.basis != self.source_design_digest()? {
            return Err(error("construction source/design basis is stale"));
        }
        if command.samples.is_empty() || command.samples.len() > MAX_CONSTRUCTION_SAMPLES {
            return Err(error("construction sample count is outside bounds"));
        }
        let terminal = self.trace_construction(command)?;
        if terminal.command.expected_declarations != command.expected_declarations {
            return Err(error(
                "construction replay resolved different semantic operands or branch intent",
            ));
        }
        Ok(terminal)
    }

    fn trace_construction(
        &self,
        command: &ConstructionCommand,
    ) -> Result<ConstructionTerminal, EngineError> {
        let mut prediction = self.begin_construction(
            command.tool,
            command.gesture_id,
            command.viewport,
            command.role,
        )?;
        for sample in &command.samples {
            prediction.advance(command.gesture_id, sample.clone())?;
        }
        prediction.finish(command.gesture_id)
    }

    /// Replays against a trusted historical basis, then independently against this session.
    /// Only freshly allocated declaration names and their typed references may differ.
    /// The host must also authenticate every returned external declaration's lifetime.
    ///
    /// # Errors
    /// Rejects forged original intent, changed inference/branches, or foreign projects.
    pub fn prepare_construction_replay(
        &self,
        basis: &Self,
        command: &ConstructionCommand,
    ) -> Result<(PreparedConstruction, crate::ConstructionReplayWitness), EngineError> {
        crate::replay::same_project(self, basis)?;
        let original = basis.replay_construction(command)?;
        let mut latest = self.trace_construction(command)?;
        retain_original_default_labels(&original, &mut latest);
        let witness = crate::replay::construction_witness(
            &original.command.expected_declarations,
            &latest.command.expected_declarations,
        )?;
        Ok((self.prepare_construction_terminal(latest)?, witness))
    }

    fn prepare_construction_terminal(
        &self,
        terminal: ConstructionTerminal,
    ) -> Result<PreparedConstruction, EngineError> {
        let mutation = self.prepare_managed_mutation(
            ManagedSketchMutation::InsertDeclarations {
                declarations: terminal.command.expected_declarations,
            },
            terminal.high_water,
        )?;
        let graph = terminal.editor.coordinator().intent().graph();
        let projections = terminal
            .declarations
            .iter()
            .map(|declaration| {
                let node = graph
                    .node(declaration.node)
                    .ok_or_else(|| error("construction declaration disappeared"))?;
                Ok(PreparedDeclarationLabelProjection {
                    node: declaration.node,
                    terminal_symbol: node.symbol.clone(),
                    declaration: declaration.symbol.clone(),
                })
            })
            .collect::<Result<Vec<_>, EngineError>>()?;
        Ok(PreparedConstruction {
            expected: self.token().clone(),
            mutation,
            editor: terminal.editor,
            projections,
        })
    }

    /// Validates the exact compiler receipt and complete native parity on an isolated
    /// session. Persist returned project/design/digest before final installation.
    ///
    /// # Errors
    /// Stale/foreign preparations, forged receipts and native/feature parity failures
    /// retain every accepted source, design, lifecycle, geometry and history value.
    pub fn resolve_construction(
        &self,
        prepared: &PreparedConstruction,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<PreparedConstructionCommit, EngineError> {
        if self.token() != &prepared.expected {
            return Err(error("construction preparation is stale or foreign"));
        }
        let mut candidate = self.fork_for_preparation();
        candidate.apply_managed_mutation(&prepared.mutation, receipt)?;
        let materialized = &candidate.accepted().0.materialized;
        crate::terminal::validate_construction_parity(
            &prepared.editor,
            &materialized.editor,
            &materialized.expansion,
            &prepared.projections,
        )
        .map_err(error)?;
        Ok(PreparedConstructionCommit {
            expected: prepared.expected.clone(),
            project: candidate.export_project_json()?,
            design: candidate.design(),
            digest: candidate.source_design_digest()?,
            candidate,
            declarations: prepared
                .projections
                .iter()
                .map(|projection| projection.declaration.clone())
                .collect(),
        })
    }

    /// Trusted synchronous publication after host durability, with an exact live-session CAS.
    ///
    /// # Errors
    /// Stale/foreign candidates cannot replace accepted source, geometry or history.
    pub fn apply_construction_commit(
        &mut self,
        prepared: PreparedConstructionCommit,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.install_prepared_session(&prepared.expected, prepared.candidate)
    }
}

impl ConstructionPrediction {
    /// Runs one bounded contiguous event through the retained M70 drafting/inference owner.
    /// Complete plans validate on this isolated fork; pointer movement never compiles source.
    ///
    /// # Errors
    /// Malformed routing/order/nonfinite events do not consume a sample. Native terminal
    /// rejection remains a correction-ready draft and never changes the accepted engine.
    pub fn advance(
        &mut self,
        gesture_id: u64,
        sample: ConstructionSample,
    ) -> Result<ConstructionFrame, EngineError> {
        self.authenticate(gesture_id)?;
        if self.completed {
            return Err(error("construction already completed"));
        }
        if self.command.samples.len() >= MAX_CONSTRUCTION_SAMPLES {
            return Err(error("construction sample limit exceeded"));
        }
        if usize::try_from(sample.sequence).map_err(error)? != self.command.samples.len() + 1 {
            return Err(error(
                "construction sample is missing, repeated or out of order",
            ));
        }
        let sample_bytes = serde_json::to_vec(&sample).map_err(error)?.len();
        if self.sample_bytes.saturating_add(sample_bytes) > MAX_CONSTRUCTION_TRACE_BYTES {
            return Err(error("construction trace byte limit exceeded"));
        }
        let pointer = match &sample.input {
            ConstructionEvent::Move { position, .. }
            | ConstructionEvent::Click { position, .. } => {
                if !position.iter().all(|value| value.is_finite()) {
                    return Err(error("nonfinite construction sample"));
                }
                if self.last_pointer != Some(*position) {
                    self.preferred_candidate = None;
                }
                self.last_pointer = Some(*position);
                let position = self.viewport.model_to_screen(*position);
                if !position.x.is_finite() || !position.y.is_finite() {
                    return Err(error("construction screen coordinate overflow"));
                }
                Some(PointerInput {
                    pointer_id: gesture_id,
                    position,
                    modifiers: Modifiers::default(),
                })
            }
            _ => None,
        };
        let next_viewport = match &sample.input {
            ConstructionEvent::Viewport { viewport } => Viewport::new(
                viewport.screen_size,
                viewport.model_center,
                viewport.pixels_per_model_unit,
            )
            .map_err(error)?,
            _ => self.viewport,
        };
        let scene = self
            .editor
            .scene(next_viewport, CHORD_TOLERANCE_PIXELS)
            .map_err(error)?;
        let prior_stages = self
            .editor
            .editor()
            .geometry_draft_status()
            .map_or(0, |status| status.completed_stages);
        let input = sample.input.clone();
        let effects = self.event_effects(gesture_id, input.clone(), &scene, pointer)?;
        if matches!(
            sample.input,
            ConstructionEvent::Click { .. } | ConstructionEvent::StepBack
        ) {
            self.preferred_candidate = None;
        }
        self.sample_bytes += sample_bytes;
        self.command.samples.push(sample);
        let resolved_position = self
            .editor
            .editor()
            .draft_inference_resolution()
            .map(|resolution| resolution.adjusted_model_position);
        self.diagnostic = self.dispatch_effects(effects)?;
        if self.command.tool == ConstructionTool::ThreePointCircle {
            let next_stages = self
                .editor
                .editor()
                .geometry_draft_status()
                .map_or(0, |status| status.completed_stages);
            match input {
                ConstructionEvent::Click { position, .. }
                    if self.diagnostic.is_none()
                        && (self.completed || next_stages > prior_stages) =>
                {
                    self.circle_authoring_points
                        .push(resolved_position.unwrap_or(position));
                }
                ConstructionEvent::StepBack if next_stages < prior_stages => {
                    self.circle_authoring_points.pop();
                }
                _ => {}
            }
        }
        Ok(self.frame())
    }

    /// Current native draft state and defaults, including before the first pointer sample.
    #[must_use]
    pub fn frame(&self) -> ConstructionFrame {
        let inference = self.editor.editor().draft_inference_resolution();
        let adjusted_position = inference.map(|resolution| resolution.adjusted_model_position);
        let inference_guides = inference.map_or_else(Vec::new, |resolution| {
            resolution
                .guides
                .iter()
                .map(|guide| match guide.geometry {
                    DraftGuideGeometry::Point { position } => ConstructionGuide::Point { position },
                    DraftGuideGeometry::Segment { start, end } => ConstructionGuide::Polyline {
                        points: vec![start, end],
                        closed: false,
                    },
                })
                .collect()
        });
        let status = self.editor.editor().geometry_draft_status();
        let has_pending = !self.completed
            && status
                .as_ref()
                .is_some_and(|status| status.completed_stages > 0);
        ConstructionFrame {
            sequence: self
                .command
                .samples
                .last()
                .map_or(0, |sample| sample.sequence),
            completed: self.completed,
            can_finish: self.editor.editor().can_complete_draft(),
            has_pending,
            can_reset: has_pending,
            can_step_back: has_pending,
            can_cycle_inference: inference
                .is_some_and(|resolution| resolution.next_cycle_candidate_id().is_some()),
            can_flip_branch: status
                .as_ref()
                .is_some_and(|status| status.completed_stages > 0 && status.branch.sweep.is_some()),
            stage: status
                .as_ref()
                .map(|status| stage_label(status.stage).to_owned()),
            conic_options: self.editor.editor().conic_options(),
            nurbs_options: self.editor.editor().nurbs_options().clone(),
            preview: self.preview.clone(),
            inference_guides,
            adjusted_position,
            diagnostic: self.diagnostic.clone(),
        }
    }

    fn event_effects(
        &mut self,
        gesture_id: u64,
        input: ConstructionEvent,
        scene: &geosolve_constraint_editor::EditorScene,
        pointer: Option<PointerInput>,
    ) -> Result<Vec<EditorEffect>, EngineError> {
        Ok(match input {
            ConstructionEvent::Viewport { viewport } => {
                self.viewport = viewport;
                self.preferred_candidate = None;
                self.refresh_preview(scene, gesture_id)
            }
            ConstructionEvent::Reset => {
                self.preferred_candidate = None;
                self.last_pointer = None;
                self.circle_authoring_points.clear();
                self.editor.editor_mut().cancel()
            }
            ConstructionEvent::Move {
                suppressed,
                regularized,
                ..
            } => self.editor.editor_mut().pointer_move_with_draft_authoring(
                scene,
                pointer.ok_or_else(|| error("missing pointer"))?,
                authoring(suppressed, regularized, self.preferred_candidate),
            ),
            ConstructionEvent::Click {
                suppressed,
                regularized,
                ..
            } => self.editor.editor_mut().pointer_down_with_draft_authoring(
                scene,
                pointer.ok_or_else(|| error("missing pointer"))?,
                authoring(suppressed, regularized, self.preferred_candidate),
            ),
            ConstructionEvent::Complete => self
                .editor
                .editor_mut()
                .complete_draft(scene.design_identity),
            ConstructionEvent::StepBack => self.editor.editor_mut().step_back_draft(),
            ConstructionEvent::FlipBranch => self.editor.editor_mut().flip_geometry_draft_branch(),
            ConstructionEvent::CycleInference => {
                let next = self.editor.editor().draft_inference_resolution().and_then(
                    geosolve_constraint_editor::DraftInferenceResolution::next_cycle_candidate_id,
                );
                if let Some(next) = next {
                    self.preferred_candidate = Some(next);
                    self.refresh_preview(scene, gesture_id)
                } else {
                    Vec::new()
                }
            }
            ConstructionEvent::ConicOptions { options } => {
                self.editor
                    .editor_mut()
                    .set_conic_options(options)
                    .map_err(error)?;
                self.refresh_preview(scene, gesture_id)
            }
            ConstructionEvent::NurbsOptions { options } => {
                if options.weights.len() > MAX_CONSTRUCTION_SAMPLES {
                    return Err(error("NURBS construction weight count is outside bounds"));
                }
                self.editor
                    .editor_mut()
                    .set_nurbs_options(options)
                    .map_err(error)?;
                self.refresh_preview(scene, gesture_id)
            }
        })
    }

    fn refresh_preview(
        &mut self,
        scene: &geosolve_constraint_editor::EditorScene,
        gesture_id: u64,
    ) -> Vec<EditorEffect> {
        let pointer = self
            .command
            .samples
            .iter()
            .rev()
            .take_while(|sample| !matches!(sample.input, ConstructionEvent::Reset))
            .find_map(|sample| match sample.input {
                ConstructionEvent::Move {
                    position,
                    suppressed,
                    regularized,
                }
                | ConstructionEvent::Click {
                    position,
                    suppressed,
                    regularized,
                } => Some((position, suppressed, regularized)),
                _ => None,
            });
        pointer.map_or_else(Vec::new, |(position, suppressed, regularized)| {
            self.editor.editor_mut().pointer_move_with_draft_authoring(
                scene,
                PointerInput {
                    pointer_id: gesture_id,
                    position: self.viewport.model_to_screen(position),
                    modifiers: Modifiers::default(),
                },
                authoring(suppressed, regularized, self.preferred_candidate),
            )
        })
    }

    fn dispatch_effects(
        &mut self,
        effects: Vec<EditorEffect>,
    ) -> Result<Option<String>, EngineError> {
        let mut diagnostic = None;
        let mut pending = std::collections::VecDeque::from(effects);
        while let Some(effect) = pending.pop_front() {
            match effect {
                EditorEffect::PreviewConstruction(preview) => {
                    self.preview = Some(project_preview(preview));
                }
                EditorEffect::ClearConstructionPreview => self.preview = None,
                EditorEffect::CommitConstructionPlan { .. } => {
                    match self.editor.apply_construction_editor_effect(&effect) {
                        Ok(outcome) => {
                            self.completed = true;
                            pending.extend(outcome.effects);
                        }
                        Err(error) => diagnostic = Some(error.to_string()),
                    }
                }
                EditorEffect::CommitConstruction { .. } => {
                    return Err(error("construction requires an authenticated native plan"));
                }
                EditorEffect::DraftInferenceChanged(_)
                | EditorEffect::SelectionChanged(_)
                | EditorEffect::HoverChanged(_)
                | EditorEffect::ClearPointPreview
                | EditorEffect::ClearCurveControlPreview
                | EditorEffect::ClearComputedFeaturePreview
                | EditorEffect::ClearComputedFeatureContactPreview
                | EditorEffect::ClearAcceptedProfileOffsetPreview => {}
                _ => {
                    return Err(error(
                        "ordinary construction emitted an unrelated edit effect",
                    ));
                }
            }
        }
        if diagnostic.is_none() {
            diagnostic = self
                .editor
                .editor()
                .geometry_draft_status()
                .and_then(|status| status.issue)
                .map(|issue| {
                    match issue {
                        GeometryDraftIssue::InvalidTerminalGeometry => {
                            "The next point would create invalid geometry"
                        }
                        GeometryDraftIssue::IncompatibleConstraintIntent => {
                            "The snapped operand is incompatible with this construction"
                        }
                        GeometryDraftIssue::CannotFinish => {
                            "More points are needed to finish this construction"
                        }
                        GeometryDraftIssue::ConstructionRejected => {
                            "The construction could not satisfy its constraints"
                        }
                        _ => "The construction needs correction",
                    }
                    .to_owned()
                });
        }
        Ok(diagnostic)
    }

    /// Detached scene plus exact semantic/native presentation correspondence.
    ///
    /// # Errors
    /// Rejects unavailable native presentation or its encoding failure.
    pub fn presentation_json(&self) -> Result<String, EngineError> {
        let bindings = self
            .editor
            .presentation_bindings()
            .ok_or_else(|| error("prediction presentation bindings are unavailable"))?;
        serde_json::to_string(&serde_json::json!({"scene":self.scene_json()?, "bindings":bindings}))
            .map_err(error)
    }

    /// Detached provisional geometry without namespace correspondence.
    ///
    /// # Errors
    /// Rejects unavailable or unrepresentable provisional scenes.
    pub fn scene_json(&self) -> Result<String, EngineError> {
        self.editor
            .scene(self.viewport, CHORD_TOLERANCE_PIXELS)
            .map_err(error)?
            .to_detached_json()
            .map_err(error)
    }

    /// Consumes completed prediction and captures source-level operand/branch semantics.
    /// This command still requires independent server replay and compiler/native validation.
    ///
    /// # Errors
    /// Rejects incomplete gestures and unsupported or ambiguous reverse source projection.
    pub fn finish(mut self, gesture_id: u64) -> Result<ConstructionTerminal, EngineError> {
        self.authenticate(gesture_id)?;
        if !self.completed {
            return Err(error("construction has no accepted terminal"));
        }
        let origin = &self.origin.0.materialized;
        let accepted_nodes = origin.editor.coordinator().intent().graph().nodes();
        let added = self
            .editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .filter(|node| !accepted_nodes.contains_key(&node.id))
            .collect::<Vec<_>>();
        if added.is_empty() || added.len() > geosolve_sketch_code::MANAGED_MUTATION_BATCH_LIMIT {
            return Err(error("construction declaration count is outside bounds"));
        }
        let (declarations, high_water) =
            crate::construction_names::allocate_canvas_declaration_names(
                &self.project,
                &added,
                self.project.managed.declaration_name_high_water,
            )
            .map_err(error)?;
        let mut insertion = prepare_editor_declaration_insertions(
            &self.project,
            &origin.expansion,
            &origin.editor,
            &self.editor,
            &declarations,
        )
        .map_err(error)?;
        // Three-point Circle's native recipe stores the derived center/radius.
        // Retain the actual resolved authoring triplet, instead of deriving a
        // second equilateral triplet whose recomputation can change last bits.
        // This is replayed source intent, not a tolerance in terminal validation.
        if self.command.tool == ConstructionTool::ThreePointCircle {
            let [first, second, third] = self.circle_authoring_points.as_slice() else {
                return Err(error(
                    "three-point circle lost its authenticated authoring samples",
                ));
            };
            let source = declarations
                .iter()
                .find(|declaration| {
                    self.editor
                        .coordinator()
                        .intent()
                        .graph()
                        .node(declaration.node)
                        .is_some_and(|node| {
                            matches!(
                                node.kind,
                                geosolve_sketch_intent::IntentNodeKind::Geometry {
                                    recipe:
                                        geosolve_sketch_intent::GeometryRecipeKind::ThreePointCircle
                                }
                            )
                        })
                })
                .ok_or_else(|| error("three-point circle lost its source declaration"))?;
            let declaration = insertion
                .declarations
                .iter_mut()
                .find(|declaration| declaration.symbol == source.symbol)
                .ok_or_else(|| error("three-point circle source insertion disappeared"))?;
            let geosolve_sketch_code::ManagedValue::Object(arguments) = &mut declaration.arguments
            else {
                return Err(error(
                    "three-point circle source arguments are not an object",
                ));
            };
            for (name, position) in [("first", first), ("second", second), ("third", third)] {
                arguments.insert(
                    name.into(),
                    geosolve_sketch_code::ManagedValue::Array(
                        position
                            .iter()
                            .copied()
                            .map(geosolve_sketch_code::ManagedValue::Number)
                            .collect(),
                    ),
                );
            }
        }
        self.command.expected_declarations = insertion
            .declarations
            .into_iter()
            .map(ManagedDeclarationDraft::from)
            .collect();
        Ok(ConstructionTerminal {
            command: self.command,
            editor: self.editor,
            declarations,
            high_water,
        })
    }
    pub fn cancel(mut self) {
        self.editor.cancel_interaction();
    }
    fn authenticate(&self, gesture_id: u64) -> Result<(), EngineError> {
        if gesture_id != self.command.gesture_id {
            return Err(error("construction belongs to a different gesture"));
        }
        Ok(())
    }
}
fn stage_label(stage: geosolve_constraint_editor::GeometryDraftStage) -> &'static str {
    use geosolve_constraint_editor::GeometryDraftStage as S;
    match stage {
        S::Point => "Point",
        S::Start => "Start",
        S::End => "End",
        S::Center => "Center",
        S::Corner => "Corner",
        S::AdjacentCorner => "Adjacent corner",
        S::OppositeCorner => "Opposite corner",
        S::SideMidpoint => "Side midpoint",
        S::DiameterStart => "Diameter start",
        S::DiameterEnd => "Diameter end",
        S::ThroughPoint => "Through point",
        S::SourceEndpoint => "Source endpoint",
        S::MajorAxisEndpoint => "Major axis endpoint",
        S::OppositeAxisEndpoint => "Opposite axis endpoint",
        S::MinorExtent => "Minor extent",
        S::ControlPoint => "Control point",
        S::Vertex => "Vertex",
        S::Focus => "Focus",
        S::TransverseAxisEndpoint => "Transverse axis endpoint",
        S::ConjugateExtent => "Conjugate extent",
        S::TrimStart => "Trim start",
        S::TrimEnd => "Trim end",
        _ => "Next point",
    }
}

fn authoring(
    suppressed: bool,
    regularized: bool,
    preferred_candidate: Option<geosolve_constraint_editor::DraftInferenceCandidateId>,
) -> DraftAuthoringInput {
    DraftAuthoringInput {
        inference: DraftInferenceInput {
            suppressed,
            preferred_candidate,
        },
        regularized,
    }
}
fn project_preview(preview: ConstructionPreview) -> ConstructionGuide {
    match preview {
        ConstructionPreview::Anchor { position } => ConstructionGuide::Point { position },
        ConstructionPreview::GuidePolyline { points, closed } => {
            ConstructionGuide::Polyline { points, closed }
        }
        ConstructionPreview::ArcRadiusGuide { center, start } => {
            ConstructionGuide::ArcRadius { center, start }
        }
        ConstructionPreview::EllipticalArcSupport {
            center,
            major_axis_point,
            support_points,
            trim_start,
        } => ConstructionGuide::EllipticalArcSupport {
            center,
            major_axis_point,
            support_points,
            trim_start,
        },
        ConstructionPreview::ControlPolygon { kind, points } => ConstructionGuide::ControlPolygon {
            curve_kind: kind,
            points,
        },
        ConstructionPreview::Complete { geometry, .. } => match geometry {
            ConstructionPreviewGeometry::Point { position } => {
                ConstructionGuide::Point { position }
            }
            ConstructionPreviewGeometry::Polyline { points } => ConstructionGuide::Polyline {
                points,
                closed: false,
            },
            ConstructionPreviewGeometry::Rectangle { first, second } => {
                ConstructionGuide::Rectangle { first, second }
            }
            ConstructionPreviewGeometry::Circle { center, radius } => {
                ConstructionGuide::Circle { center, radius }
            }
            ConstructionPreviewGeometry::CounterClockwiseArc {
                center,
                start,
                end,
                radius,
                sweep_radians,
                large_arc,
            } => ConstructionGuide::CircularArc {
                center,
                start,
                end,
                radius,
                sweep_radians,
                large_arc,
                sweep: DocumentArcSweep::CounterClockwise,
            },
            ConstructionPreviewGeometry::CircularArc {
                center,
                start,
                end,
                radius,
                sweep_radians,
                large_arc,
                sweep,
            } => ConstructionGuide::CircularArc {
                center,
                start,
                end,
                radius,
                sweep_radians,
                large_arc,
                sweep,
            },
            ConstructionPreviewGeometry::AdvancedCurve {
                kind,
                control_points,
                curve_points,
            } => ConstructionGuide::AdvancedCurve {
                curve_kind: kind,
                control_points,
                curve_points,
            },
        },
    }
}

/// Draft labels default to private native allocation symbols. Preserve the exact
/// original authenticated display label while allocating latest persistent names.
/// This is restricted to unrenamed newly created nodes; arbitrary metadata never
/// participates in alpha normalization.
fn retain_original_default_labels(
    original: &ConstructionTerminal,
    latest: &mut ConstructionTerminal,
) {
    if original.command.expected_declarations.len() != latest.command.expected_declarations.len() {
        return;
    }
    let default_symbol =
        |terminal: &ConstructionTerminal, draft: &ManagedDeclarationDraft| -> Option<String> {
            let declaration = terminal
                .declarations
                .iter()
                .find(|value| value.symbol.0 == draft.symbol)?;
            let intent = terminal.editor.coordinator().intent();
            let node = intent.graph().node(declaration.node)?;
            if intent
                .organization()
                .node_names()
                .get(&declaration.node)
                .is_some_and(|name| name != &node.symbol)
            {
                return None;
            }
            Some(
                intent
                    .graph()
                    .node(declaration.node)?
                    .symbol
                    .as_str()
                    .to_owned(),
            )
        };
    let old_defaults = original
        .command
        .expected_declarations
        .iter()
        .map(|draft| default_symbol(original, draft))
        .collect::<Vec<_>>();
    let new_defaults = latest
        .command
        .expected_declarations
        .iter()
        .map(|draft| default_symbol(latest, draft))
        .collect::<Vec<_>>();
    for (((old, new), old_default), new_default) in original
        .command
        .expected_declarations
        .iter()
        .zip(&mut latest.command.expected_declarations)
        .zip(old_defaults)
        .zip(new_defaults)
    {
        let (Some(old_default), Some(new_default)) = (old_default, new_default) else {
            continue;
        };
        let (
            geosolve_sketch_code::ManagedValue::Object(old_fields),
            geosolve_sketch_code::ManagedValue::Object(new_fields),
        ) = (&old.arguments, &mut new.arguments)
        else {
            continue;
        };
        if old_fields.get("label")
            == Some(&geosolve_sketch_code::ManagedValue::String(
                old_default.clone(),
            ))
            && new_fields.get("label")
                == Some(&geosolve_sketch_code::ManagedValue::String(new_default))
        {
            new_fields.insert(
                "label".into(),
                geosolve_sketch_code::ManagedValue::String(old_default),
            );
        }
    }
}
