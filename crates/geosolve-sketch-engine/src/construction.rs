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
    CodeProject, EditorBootstrapDeclaration, ManagedDeclarationDraft, ManagedSketchMutation,
    PreparedManagedMutationReceipt, PreparedManagedMutationRequest, SemanticSymbol,
};
use serde::{Deserialize, Serialize};

use crate::{AcceptedEvaluation, EditableSession, EngineError};

pub const MAX_CONSTRUCTION_SAMPLES: usize = 4096;
// Matches the retained WASM construction reservation; option arrays cannot
// multiply one reserved trace into MAX_CONSTRUCTION_SAMPLES independent arrays.
const MAX_CONSTRUCTION_TRACE_BYTES: usize = 1024 * 1024;
const CHORD_TOLERANCE_PIXELS: f64 = 0.25;
fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

macro_rules! define_construction_tools {
    ($( $family:ident => ($family_key:literal, $default:ident) {
        $( $variant:ident => $key:literal, )*
    } )*) => {
        /// Wire projection of the complete native construction inventory.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum ConstructionTool { $( $( $variant, )* )* }
        impl ConstructionTool {
            /// Every existing native construction recipe in palette order.
            pub const ALL: [Self; GeometryToolVariant::ALL.len()] = [$( $( Self::$variant, )* )*];
            /// Exact native recipe; no legacy family coalescing.
            #[must_use]
            pub const fn variant(self) -> GeometryToolVariant {
                match self { $( $( Self::$variant => GeometryToolVariant::$variant, )* )* }
            }
        }
    };
}
geosolve_constraint_editor::geometry_tool_catalog!(define_construction_tools);

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
    operation: crate::authoring_commit::PreparedNativeAuthoring,
}
impl PreparedConstruction {
    pub fn request(&self) -> &PreparedManagedMutationRequest {
        self.operation.mutation.request()
    }
    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &SemanticSymbol> {
        self.operation
            .projections
            .iter()
            .map(|projection| &projection.declaration)
    }
}
#[derive(Debug)]
pub struct PreparedConstructionCommit {
    operation: crate::authoring_commit::PreparedNativeAuthoringCommit,
}
impl PreparedConstructionCommit {
    pub fn result(&self) -> &crate::EngineAcceptedResult {
        self.operation.candidate.accepted().result()
    }
    pub fn project_json(&self) -> &str {
        &self.operation.project
    }
    pub fn design(&self) -> &crate::EditableDesign {
        &self.operation.design
    }
    pub fn source_design_digest(&self) -> &str {
        &self.operation.digest
    }
    pub fn declarations(&self) -> &[SemanticSymbol] {
        &self.operation.declarations
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
        let mut terminal = self.trace_construction(command)?;
        if terminal.command.expected_declarations != command.expected_declarations {
            // Retained history can advance native default-label allocation without
            // changing the public source/design basis. Authenticate a cold client's
            // entire command independently; only native-proven default labels may
            // then transfer to the retained terminal. Operands/branches stay exact.
            let cold = self.cold_prediction_basis()?.trace_construction(command)?;
            if cold.command.expected_declarations == command.expected_declarations {
                retain_original_default_labels(&cold, &mut terminal);
            }
        }
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
        Ok(PreparedConstruction {
            operation: self.prepare_native_authoring(
                terminal.editor,
                &terminal.declarations,
                ManagedSketchMutation::InsertDeclarations {
                    declarations: terminal.command.expected_declarations,
                },
                terminal.high_water,
            )?,
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
        Ok(PreparedConstructionCommit {
            operation: self.resolve_native_authoring(
                &prepared.operation,
                receipt,
                "construction",
                "Apply project",
            )?,
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
        self.install_native_authoring(prepared.operation)
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
        let effects = self.event_effects(gesture_id, sample.input.clone(), &scene, pointer)?;
        if matches!(
            sample.input,
            ConstructionEvent::Click { .. } | ConstructionEvent::StepBack
        ) {
            self.preferred_candidate = None;
        }
        self.sample_bytes += sample_bytes;
        self.command.samples.push(sample);
        self.diagnostic = self.dispatch_effects(effects)?;
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
        let (declarations, high_water, drafts) =
            geosolve_sketch_code::prepare_editor_source_insertion(
                &self.project,
                &origin.expansion,
                &origin.editor,
                &self.editor,
            )
            .map_err(error)?
            .into_parts();
        self.command.expected_declarations = drafts;
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

fn retain_original_default_labels(
    original: &ConstructionTerminal,
    latest: &mut ConstructionTerminal,
) {
    geosolve_sketch_code::retain_editor_default_labels(
        &original.editor,
        &original.declarations,
        &original.command.expected_declarations,
        &latest.editor,
        &latest.declarations,
        &mut latest.command.expected_declarations,
    );
}
