// SPDX-License-Identifier: GPL-3.0-or-later
//! Retained native relation, dimension and computed-feature authoring.
use crate::{AcceptedEvaluation, EditableSession, EngineError};
use geosolve_constraint_editor::{
    AuthoringOperand, AuthoringOptions, AuthoringOutcome, AuthoringState, AuthoringTool,
    ConstraintIntent, DimensionKind, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, IntentNativeBinding, Modifiers,
    OffsetAuthoringOutcome, OffsetAuthoringState, OffsetAuthoringTarget, PickTolerance,
    PointerInput, ProjectionalEditorSession, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveSpan, DocumentAngleOrientation, DocumentCurveContinuity, DocumentCurveCurvatureRelation,
    DocumentDimensionMode, SketchDatum, TangentOrientation,
};
use geosolve_sketch_code::{
    CodeProject, EditorBootstrapDeclaration, ManagedDeclarationDraft, ManagedSketchMutation,
    ManagedValue, PreparedManagedMutationReceipt, PreparedManagedMutationRequest, SemanticSymbol,
};
use geosolve_sketch_intent::{IntentKey, IntentPlanDisposition};
use serde::{Deserialize, Serialize};

mod presentation;

pub const MAX_TOOL_OPERATION_SAMPLES: usize = 4096;
// Match the WASM adapter's per-draft reservation across the complete retained trace,
// including repeated selection batches whose individual payloads fit the request limit.
const MAX_TOOL_OPERATION_TRACE_BYTES: usize = 1024 * 1024;
const CHORD_TOLERANCE_PIXELS: f64 = 0.25;
fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

macro_rules! define_operation_tools {
    (constraints { $( $constraint:ident => ($constraint_key:literal, $constraint_label:literal), )* }
     dimensions { $( $dimension:ident => ($dimension_key:literal, $dimension_label:literal), )* }
     modify { $( $modify:ident => ($modify_key:literal, $modify_label:literal), )* }
     auxiliary { $( $auxiliary:ident => ($auxiliary_key:literal, $auxiliary_label:literal, $auxiliary_command:literal), )* }) => {
        /// Wire projection of native authoring, Modify and contextual actions.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum ToolOperationTool { $( $constraint, )* $( $dimension, )* $( $modify, )* $( #[serde(rename = $auxiliary_command)] $auxiliary, )* }
        impl ToolOperationTool {
            /// Native palette tools; contextual actions remain separately invokable.
            pub const ALL: [Self; AuthoringTool::ALL.len() + geosolve_constraint_editor::ModifyTool::PALETTE.len()] =
                [$( Self::$constraint, )* $( Self::$dimension, )* $( Self::$modify, )*];
            fn authoring(self) -> Option<AuthoringTool> {
                match self {
                    $( Self::$constraint => Some(AuthoringTool::Constraint(ConstraintIntent::$constraint)), )*
                    $( Self::$dimension => Some(AuthoringTool::Dimension(DimensionKind::$dimension)), )*
                    $( Self::$modify => None, )*
                    $( Self::$auxiliary => None, )*
                }
            }
        }
    };
}
geosolve_constraint_editor::authoring_tool_catalog!(define_operation_tools);

/// Exact source/native ownership, independent of allocator identities and coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolOperationOperand {
    Binding {
        symbol: IntentKey,
        binding: usize,
        span: Option<u32>,
        curve_parameter: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        occurrence: Option<crate::ToolCurveOccurrence>,
    },
    Datum {
        datum: SketchDatum,
    },
}
/// Remembered native choices applied before consuming preselection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolOperationOptions {
    #[serde(default, with = "OptionalAuthoringOptionsWire")]
    pub authoring_options: Option<AuthoringOptions>,
    #[serde(default, with = "OptionalFilletOptionsWire")]
    pub fillet_options: Option<FeatureAuthoringOptions>,
    pub offset_distance: Option<f64>,
}
#[allow(non_snake_case)]
mod OptionalAuthoringOptionsWire {
    use super::{AuthoringOptions, AuthoringOptionsWire, Deserialize, Serialize};
    #[derive(Serialize, Deserialize)]
    struct Wrapped(#[serde(with = "AuthoringOptionsWire")] AuthoringOptions);
    #[allow(
        clippy::ref_option,
        reason = "serde with serializers require a reference to the complete field"
    )]
    pub fn serialize<S: serde::Serializer>(
        value: &Option<AuthoringOptions>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Wrapped).serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<AuthoringOptions>, D::Error> {
        Option::<Wrapped>::deserialize(deserializer).map(|value| value.map(|value| value.0))
    }
}
#[allow(non_snake_case)]
mod OptionalFilletOptionsWire {
    use super::{Deserialize, FeatureAuthoringOptions, FilletOptionsWire, Serialize};
    #[derive(Serialize, Deserialize)]
    struct Wrapped(#[serde(with = "FilletOptionsWire")] FeatureAuthoringOptions);
    #[allow(
        clippy::ref_option,
        reason = "serde with serializers require a reference to the complete field"
    )]
    pub fn serialize<S: serde::Serializer>(
        value: &Option<FeatureAuthoringOptions>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Wrapped).serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<FeatureAuthoringOptions>, D::Error> {
        Option::<Wrapped>::deserialize(deserializer).map(|value| value.map(|value| value.0))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolOperationEvent {
    Viewport {
        viewport: Viewport,
    },
    Move {
        position: [f64; 2],
    },
    Click {
        position: [f64; 2],
    },
    Pick {
        operand: ToolOperationOperand,
    },
    PickSelection {
        operands: Vec<ToolOperationOperand>,
    },
    Complete,
    StepBack,
    Reset,
    AuthoringOptions {
        #[serde(with = "AuthoringOptionsWire")]
        options: AuthoringOptions,
    },
    FilletOptions {
        #[serde(with = "FilletOptionsWire")]
        options: FeatureAuthoringOptions,
        selected_corner: Option<usize>,
    },
    FilletRadius {
        radius: f64,
    },
    OffsetDistance {
        distance: f64,
    },
    OffsetFlip,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "AuthoringOptions", deny_unknown_fields)]
struct AuthoringOptionsWire {
    tangent_orientation: TangentOrientation,
    curvature_relation: DocumentCurveCurvatureRelation,
    continuity: DocumentCurveContinuity,
    dimension_mode: DocumentDimensionMode,
    angle_orientation: DocumentAngleOrientation,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "FeatureAuthoringOptions", deny_unknown_fields)]
struct FilletOptionsWire {
    fillet_radius: Option<f64>,
    flip_first_side: bool,
    flip_second_side: bool,
    alternate_arc: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolOperationSample {
    pub sequence: u32,
    pub input: ToolOperationEvent,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolOperationCommand {
    pub basis: String,
    pub gesture_id: u64,
    pub viewport: Viewport,
    pub tool: ToolOperationTool,
    pub selection: Vec<ToolOperationOperand>,
    #[serde(default)]
    pub options: ToolOperationOptions,
    pub samples: Vec<ToolOperationSample>,
    pub expected_declarations: Vec<ManagedDeclarationDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_mutation: Option<ManagedSketchMutation>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent native collector capabilities are exposed directly to the host"
)]
pub struct ToolOperationFrame {
    pub sequence: u32,
    pub completed: bool,
    pub can_finish: bool,
    pub has_pending: bool,
    pub can_reset: bool,
    pub can_step_back: bool,
    pub diagnostic: Option<String>,
    pub pending: Vec<ToolOperationOperand>,
    #[serde(with = "AuthoringOptionsWire")]
    pub authoring_options: AuthoringOptions,
    #[serde(with = "FilletOptionsWire")]
    pub fillet_options: FeatureAuthoringOptions,
    pub fillet_corner_count: usize,
    pub fillet_corners: Vec<ToolFilletCornerOptions>,
    pub offset_distance: Option<f64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFilletCornerOptions {
    pub index: usize,
    #[serde(with = "FilletOptionsWire")]
    pub options: FeatureAuthoringOptions,
}
#[derive(Debug)]
pub struct ToolOperationPrediction {
    origin: AcceptedEvaluation,
    project: CodeProject,
    editor: Box<ProjectionalEditorSession>,
    command: ToolOperationCommand,
    viewport: Viewport,
    sample_bytes: usize,
    authoring: AuthoringState,
    fillet: FeatureAuthoringState,
    offset: OffsetAuthoringState,
    symbol: IntentKey,
    completed: bool,
    diagnostic: Option<String>,
}
#[derive(Debug)]
pub struct ToolOperationTerminal {
    command: ToolOperationCommand,
    symbol: IntentKey,
    editor: Box<ProjectionalEditorSession>,
    declarations: Vec<EditorBootstrapDeclaration>,
    high_water: u64,
}
impl ToolOperationTerminal {
    pub fn command(&self) -> &ToolOperationCommand {
        &self.command
    }
}

/// Exact server replay plus one compiler ticket, held privately until resolution.
#[derive(Debug)]
pub struct PreparedToolOperation {
    operation: crate::authoring_commit::PreparedNativeAuthoring,
}
impl PreparedToolOperation {
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
pub struct PreparedToolOperationCommit {
    operation: crate::authoring_commit::PreparedNativeAuthoringCommit,
}
impl PreparedToolOperationCommit {
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
    fn prepare_tool_operation_terminal(
        &self,
        terminal: ToolOperationTerminal,
    ) -> Result<PreparedToolOperation, EngineError> {
        Ok(PreparedToolOperation {
            operation: self.prepare_native_authoring(
                terminal.editor,
                &terminal.declarations,
                terminal.command.expected_mutation.unwrap_or(
                    ManagedSketchMutation::InsertDeclarations {
                        declarations: terminal.command.expected_declarations,
                    },
                ),
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
    pub fn resolve_tool_operation(
        &self,
        prepared: &PreparedToolOperation,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<PreparedToolOperationCommit, EngineError> {
        Ok(PreparedToolOperationCommit {
            operation: self.resolve_native_authoring(
                &prepared.operation,
                receipt,
                "tool_operation",
                "Apply project",
            )?,
        })
    }

    /// Trusted synchronous publication after host durability, with an exact live-session CAS.
    ///
    /// # Errors
    /// Stale/foreign candidates cannot replace accepted source, geometry or history.
    pub fn apply_tool_operation_commit(
        &mut self,
        prepared: PreparedToolOperationCommit,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.install_native_authoring(prepared.operation)
    }
}

impl EditableSession {
    /// Starts personal native tool collection on an isolated accepted fork.
    /// # Errors
    /// Rejects invalid routing, camera or semantic preselection without publication.
    pub fn begin_tool_operation(
        &self,
        tool: ToolOperationTool,
        gesture_id: u64,
        viewport: Viewport,
        selection: Vec<ToolOperationOperand>,
    ) -> Result<ToolOperationPrediction, EngineError> {
        self.begin_tool_operation_with_options(
            tool,
            gesture_id,
            viewport,
            selection,
            ToolOperationOptions::default(),
        )
    }
    /// Starts a native tool with remembered options applied before preselection.
    /// # Errors
    /// Rejects invalid options, routing or semantic operands without publication.
    pub fn begin_tool_operation_with_options(
        &self,
        tool: ToolOperationTool,
        gesture_id: u64,
        viewport: Viewport,
        selection: Vec<ToolOperationOperand>,
        options: ToolOperationOptions,
    ) -> Result<ToolOperationPrediction, EngineError> {
        self.begin_tool_operation_with_symbol(tool, gesture_id, viewport, selection, options, None)
    }

    // Only server-owned replay may provide a feature label, after authenticating
    // the complete original command on its trusted basis. It is never a wire input.
    fn begin_tool_operation_with_symbol(
        &self,
        tool: ToolOperationTool,
        gesture_id: u64,
        viewport: Viewport,
        selection: Vec<ToolOperationOperand>,
        options: ToolOperationOptions,
        authenticated_symbol: Option<IntentKey>,
    ) -> Result<ToolOperationPrediction, EngineError> {
        Viewport::new(
            viewport.screen_size,
            viewport.model_center,
            viewport.pixels_per_model_unit,
        )
        .map_err(error)?;
        if gesture_id == 0
            || gesture_id > geosolve_sketch_code::MAX_CODE_SESSION_WIRE_INTEGER
            || selection.len() > 256
        {
            return Err(error(
                "tool operation identity or selection is outside bounds",
            ));
        }
        let editor = Box::new(
            self.accepted()
                .0
                .materialized
                .editor
                .fork_accepted_authority()
                .map_err(error)?,
        );
        let symbol = authenticated_symbol.map_or_else(
            || {
                IntentKey::new(format!(
                    "{} {}",
                    if tool == ToolOperationTool::Offset {
                        "Offset"
                    } else {
                        "Fillet"
                    },
                    editor
                        .coordinator()
                        .intent()
                        .identity()
                        .revision
                        .raw()
                        .saturating_add(1)
                ))
                .map_err(error)
            },
            Ok,
        )?;
        let mut prediction = ToolOperationPrediction {
            origin: self.accepted().clone(),
            project: self
                .code_snapshot()
                .code_project
                .clone()
                .ok_or_else(|| error("missing accepted project"))?,
            editor,
            command: ToolOperationCommand {
                basis: self.source_design_digest()?,
                gesture_id,
                viewport,
                tool,
                selection,
                options,
                samples: Vec::new(),
                expected_declarations: Vec::new(),
                expected_mutation: None,
            },
            viewport,
            sample_bytes: 0,
            authoring: AuthoringState::default(),
            fillet: FeatureAuthoringState::default(),
            offset: OffsetAuthoringState::default(),
            symbol,
            completed: false,
            diagnostic: None,
        };
        prediction.activate()?;
        Ok(prediction)
    }
    /// Exact semantic operand for a native accepted item. Hosts must translate presentation namespaces first.
    /// # Errors
    /// Rejects unowned or ambiguous items and invalid curve parameters.
    pub fn tool_operation_operand(
        &self,
        item: SelectionItem,
        curve_parameter: Option<f64>,
    ) -> Result<ToolOperationOperand, EngineError> {
        operand_for_item(
            &self.accepted().0.materialized.editor,
            item,
            curve_parameter,
        )
    }
    /// Independently replays and prepares a genuine compiler request.
    /// # Errors
    /// Rejects stale origin, altered operands, branches or nonterminal traces.
    pub fn prepare_tool_operation(
        &self,
        command: &ToolOperationCommand,
    ) -> Result<PreparedToolOperation, EngineError> {
        let terminal = self.replay_tool_operation(command)?;
        self.prepare_tool_operation_terminal(terminal)
    }
    fn trace_tool_operation(
        &self,
        command: &ToolOperationCommand,
    ) -> Result<ToolOperationTerminal, EngineError> {
        self.trace_tool_operation_with_symbol(command, None)
    }

    fn trace_tool_operation_with_symbol(
        &self,
        command: &ToolOperationCommand,
        symbol: Option<IntentKey>,
    ) -> Result<ToolOperationTerminal, EngineError> {
        if command.samples.len() > MAX_TOOL_OPERATION_SAMPLES {
            return Err(error("tool operation trace exceeds bounds"));
        }
        let mut prediction = self.begin_tool_operation_with_symbol(
            command.tool,
            command.gesture_id,
            command.viewport,
            command.selection.clone(),
            command.options.clone(),
            symbol,
        )?;
        for sample in &command.samples {
            prediction.advance(command.gesture_id, sample.clone())?;
        }
        prediction.finish(command.gesture_id)
    }
    fn replay_tool_operation(
        &self,
        command: &ToolOperationCommand,
    ) -> Result<ToolOperationTerminal, EngineError> {
        if command.basis != self.source_design_digest()? {
            return Err(error("tool operation source/design basis is stale"));
        }
        let mut terminal = self.trace_tool_operation(command)?;
        if terminal.command.expected_declarations != command.expected_declarations {
            let cold = self
                .cold_prediction_basis()?
                .trace_tool_operation(command)?;
            if cold.command.expected_declarations == command.expected_declarations
                && cold.command.expected_mutation == command.expected_mutation
            {
                if matches!(
                    command.tool,
                    ToolOperationTool::Fillet | ToolOperationTool::Offset
                ) {
                    terminal =
                        self.trace_tool_operation_with_symbol(command, Some(cold.symbol.clone()))?;
                }
                retain_original_default_labels(&cold, &mut terminal);
            }
        }
        if terminal.command.expected_declarations != command.expected_declarations
            || terminal.command.expected_mutation != command.expected_mutation
        {
            return Err(error(
                "tool operation replay changed semantic operands or explicit branches",
            ));
        }
        Ok(terminal)
    }
    /// Historical authentication followed by fresh server-owned replay.
    /// # Errors
    /// Rejects changed operands/branches; host must authenticate returned declaration lifetimes.
    pub fn prepare_tool_operation_replay(
        &self,
        basis: &Self,
        command: &ToolOperationCommand,
    ) -> Result<(PreparedToolOperation, crate::ConstructionReplayWitness), EngineError> {
        crate::replay::same_project(self, basis)?;
        let original = basis.replay_tool_operation(command)?;
        let mut latest =
            self.trace_tool_operation_with_symbol(command, Some(original.symbol.clone()))?;
        retain_original_default_labels(&original, &mut latest);
        let mut witness = crate::replay::construction_witness(
            &original.command.expected_declarations,
            &latest.command.expected_declarations,
        )?;
        if let Some(ManagedSketchMutation::SetValues { values }) =
            &original.command.expected_mutation
        {
            let Some(ManagedSketchMutation::SetValues {
                values: latest_values,
            }) = &latest.command.expected_mutation
            else {
                return Err(error("tool operation changed mutation family"));
            };
            if values != latest_values {
                return Err(error("tool operation source role changed during replay"));
            }
            witness.required_stable_declarations.extend(
                values
                    .iter()
                    .map(|value| SemanticSymbol(value.declaration.clone())),
            );
        } else if original.command.expected_mutation != latest.command.expected_mutation {
            return Err(error("tool operation changed mutation family"));
        }
        witness
            .required_stable_declarations
            .extend(crate::tool_selection::required_declarations(
                basis, command,
            )?);
        witness.required_stable_declarations.sort();
        witness.required_stable_declarations.dedup();
        Ok((self.prepare_tool_operation_terminal(latest)?, witness))
    }
}

fn retain_original_default_labels(
    original: &ToolOperationTerminal,
    latest: &mut ToolOperationTerminal,
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

fn resolve_operand(
    editor: &ProjectionalEditorSession,
    operand: &ToolOperationOperand,
) -> Result<AuthoringOperand, EngineError> {
    let ToolOperationOperand::Binding {
        symbol,
        binding,
        span,
        curve_parameter,
        occurrence,
    } = operand
    else {
        let ToolOperationOperand::Datum { datum } = operand else {
            unreachable!()
        };
        return Ok(AuthoringOperand::selected(SelectionItem::Datum(*datum)));
    };
    if curve_parameter.is_some_and(|v| !v.is_finite()) {
        return Err(error("nonfinite curve occurrence"));
    }
    let bindings = editor
        .presentation_bindings()
        .ok_or_else(|| error("missing native source bindings"))?;
    let native = *bindings
        .nodes
        .get(symbol)
        .and_then(|v| v.get(*binding))
        .ok_or_else(|| error("tool operand source ownership is stale"))?;
    let item = match native {
        IntentNativeBinding::Point(p) if span.is_none() && curve_parameter.is_none() => {
            SelectionItem::Point(p)
        }
        IntentNativeBinding::Curve(curve) => SelectionItem::Curve(CurveSpan {
            curve,
            segment: span.ok_or_else(|| error("curve operand requires an explicit span"))?,
        }),
        IntentNativeBinding::CurveSpan(value) if span.is_none() || *span == Some(value.segment) => {
            SelectionItem::Curve(value)
        }
        _ => return Err(error("tool operand is not an exact point, curve or datum")),
    };
    let document = editor
        .presentation_session()
        .ok_or_else(|| error("missing accepted document"))?
        .design_document();
    if let SelectionItem::Curve(value) = item {
        if let Some(occurrence) = occurrence {
            crate::tool_selection::validate_occurrence(
                editor,
                value,
                *curve_parameter,
                occurrence,
            )?;
        }
        if !document
            .curve_spans(value.curve)
            .map_err(error)?
            .contains(&value)
        {
            return Err(error("curve span is unavailable"));
        }
    }
    Ok(AuthoringOperand::picked(item, *curve_parameter))
}
fn operand_for_item(
    editor: &ProjectionalEditorSession,
    item: SelectionItem,
    curve_parameter: Option<f64>,
) -> Result<ToolOperationOperand, EngineError> {
    if let SelectionItem::Datum(datum) = item {
        return Ok(ToolOperationOperand::Datum { datum });
    }
    if curve_parameter.is_some_and(|v| !v.is_finite()) {
        return Err(error("nonfinite curve occurrence"));
    }
    let bindings = editor
        .presentation_bindings()
        .ok_or_else(|| error("missing native source bindings"))?;
    let mut found = Vec::new();
    for (symbol, values) in bindings.nodes {
        for (binding, value) in values.into_iter().enumerate() {
            let span = match (item, value) {
                (SelectionItem::Point(a), IntentNativeBinding::Point(b)) if a == b => None,
                (SelectionItem::Curve(a), IntentNativeBinding::Curve(b)) if a.curve == b => {
                    Some(a.segment)
                }
                _ => continue,
            };
            found.push(ToolOperationOperand::Binding {
                occurrence: None,
                symbol: symbol.clone(),
                binding,
                span,
                curve_parameter: if span.is_some() {
                    curve_parameter
                } else {
                    None
                },
            });
        }
    }
    if found.len() != 1 {
        return Err(error(
            "tool operand must have one exact native source owner",
        ));
    }
    Ok(found.remove(0))
}

impl ToolOperationPrediction {
    fn activate(&mut self) -> Result<(), EngineError> {
        let selection = self
            .command
            .selection
            .iter()
            .map(|operand| resolve_operand(&self.editor, operand))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(options) = self.command.options.authoring_options {
            self.authoring.set_options(options);
        }
        if self.command.tool == ToolOperationTool::ToggleGeometryRole {
            self.editor
                .set_selection(selection.iter().map(|operand| operand.item));
            let outcome = self.editor.toggle_selected_geometry_role().map_err(error)?;
            if outcome.disposition != IntentPlanDisposition::Accepted {
                return Err(error("geometry role change was rejected"));
            }
            self.completed = true;
        } else if let Some(tool) = self.command.tool.authoring() {
            let document = self
                .editor
                .presentation_session()
                .ok_or_else(|| error("missing authoring document"))?
                .design_document()
                .clone();
            let outcome = self.authoring.activate(&document, tool, &selection);
            self.authoring_outcome(outcome);
        } else if self.command.tool == ToolOperationTool::Fillet {
            let picks = selection
                .iter()
                .map(|v| (v.item, v.curve_parameter))
                .collect::<Vec<_>>();
            let outcome = self
                .editor
                .activate_feature_authoring(
                    &mut self.fillet,
                    FeatureAuthoringTool::Fillet,
                    self.command.options.fillet_options.unwrap_or_default(),
                    &picks,
                    self.symbol.clone(),
                )
                .map_err(error)?;
            self.feature_outcome(outcome);
        } else {
            let outcome = self
                .editor
                .activate_offset_authoring(&mut self.offset)
                .map_err(error)?;
            self.offset_outcome(outcome)?;
            if let Some(distance) = self.command.options.offset_distance {
                let outcome = self
                    .editor
                    .transact_offset_authoring_distance(
                        &mut self.offset,
                        distance,
                        self.symbol.clone(),
                    )
                    .map_err(error)?;
                self.offset_outcome(outcome)?;
            }
            for operand in selection {
                self.pick_offset(operand)?;
            }
        }
        Ok(())
    }
    fn authoring_outcome(&mut self, outcome: AuthoringOutcome) {
        match outcome {
            AuthoringOutcome::Apply(application) => {
                // The projectional editor deliberately retains unsolved intent. A personal
                // toolbar attempt must instead retain its last valid draft on refusal, so a
                // subsequent pick cannot inherit the refused relation or its source history.
                let attempt = self
                    .editor
                    .fork_accepted_authority()
                    .and_then(|mut candidate| {
                        let outcome = candidate.apply_authoring_application(&application)?;
                        if outcome.disposition == IntentPlanDisposition::Accepted {
                            *self.editor = candidate;
                            self.completed = true;
                        }
                        Ok(outcome)
                    });
                match attempt {
                    Ok(outcome) if outcome.disposition == IntentPlanDisposition::Accepted => {}
                    Ok(_) => {
                        self.diagnostic = Some(
                            "The relation or dimension could not satisfy its constraints".into(),
                        );
                    }
                    Err(error) => self.diagnostic = Some(error.to_string()),
                }
                self.authoring.transaction_finished();
            }
            AuthoringOutcome::Warning(warning) => self.diagnostic = Some(warning.message),
            _ => {}
        }
    }
    fn feature_outcome(&mut self, outcome: FeatureAuthoringOutcome) {
        if let FeatureAuthoringOutcome::Warning(warning) = outcome {
            self.diagnostic = Some(warning.message);
        }
    }
    fn offset_outcome(&mut self, outcome: OffsetAuthoringOutcome) -> Result<(), EngineError> {
        match outcome {
            OffsetAuthoringOutcome::Warning(warning) => self.diagnostic = Some(warning.message),
            OffsetAuthoringOutcome::OperandChanged { .. }
            | OffsetAuthoringOutcome::DistanceChanged { .. } => {
                self.editor
                    .refresh_offset_authoring_preview(&self.offset, self.symbol.clone())
                    .map_err(error)?;
            }
            _ => {}
        }
        Ok(())
    }
    fn pick_offset(&mut self, operand: AuthoringOperand) -> Result<(), EngineError> {
        let SelectionItem::Curve(span) = operand.item else {
            self.diagnostic = Some("Choose a profile edge for Offset".into());
            return Ok(());
        };
        let outcome = self.offset.pick_target(OffsetAuthoringTarget::Span(span));
        self.offset_outcome(outcome)
    }
    /// Applies one contiguous native authoring input on the isolated draft.
    /// # Errors
    /// Rejects malformed routing, sequence or geometry; accepted engine remains unchanged.
    pub fn advance(
        &mut self,
        gesture_id: u64,
        sample: ToolOperationSample,
    ) -> Result<ToolOperationFrame, EngineError> {
        self.authenticate(gesture_id)?;
        if self.completed {
            return Err(error("tool operation already completed"));
        }
        if self.command.samples.len() >= MAX_TOOL_OPERATION_SAMPLES
            || usize::try_from(sample.sequence).map_err(error)? != self.command.samples.len() + 1
        {
            return Err(error(
                "tool operation sequence is missing, repeated or out of order",
            ));
        }
        let sample_bytes = serde_json::to_vec(&sample).map_err(error)?.len();
        if self.sample_bytes.saturating_add(sample_bytes) > MAX_TOOL_OPERATION_TRACE_BYTES {
            return Err(error("tool operation trace byte limit exceeded"));
        }
        let position = match sample.input {
            ToolOperationEvent::Move { position } | ToolOperationEvent::Click { position } => {
                Some(position)
            }
            _ => None,
        };
        if position.is_some_and(|p| !p.into_iter().all(f64::is_finite)) {
            return Err(error("nonfinite tool pointer"));
        }
        let previous = (
            self.authoring.clone(),
            self.fillet.clone(),
            self.offset.clone(),
            self.diagnostic.clone(),
        );
        self.diagnostic = None;
        if let Err(failure) = self.dispatch(&sample.input) {
            (self.authoring, self.fillet, self.offset, self.diagnostic) = previous;
            if matches!(
                sample.input,
                ToolOperationEvent::AuthoringOptions { .. }
                    | ToolOperationEvent::FilletOptions { .. }
                    | ToolOperationEvent::FilletRadius { .. }
                    | ToolOperationEvent::OffsetDistance { .. }
                    | ToolOperationEvent::OffsetFlip
            ) {
                self.diagnostic = Some(failure.to_string());
            } else {
                return Err(failure);
            }
        }
        self.sample_bytes += sample_bytes;
        self.command.samples.push(sample);
        self.frame()
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one closed event router delegates every operation to its existing native owner"
    )]
    fn dispatch(&mut self, input: &ToolOperationEvent) -> Result<(), EngineError> {
        let scene = self
            .editor
            .scene(self.viewport, CHORD_TOLERANCE_PIXELS)
            .map_err(error)?;
        match input {
            ToolOperationEvent::Viewport { viewport } => {
                Viewport::new(
                    viewport.screen_size,
                    viewport.model_center,
                    viewport.pixels_per_model_unit,
                )
                .map_err(error)?;
                self.viewport = *viewport;
            }
            ToolOperationEvent::Move { position } | ToolOperationEvent::Click { position } => {
                let position = self.viewport.model_to_screen(*position);
                if !position.x.is_finite() || !position.y.is_finite() {
                    return Err(error("tool pointer overflow"));
                }
                let pointer = PointerInput {
                    pointer_id: self.command.gesture_id,
                    position,
                    modifiers: Modifiers::default(),
                };
                if matches!(input, ToolOperationEvent::Move { .. }) {
                    if self.command.tool.authoring().is_some() {
                        self.editor.pointer_move_authoring(
                            &self.authoring,
                            &scene,
                            pointer,
                            PickTolerance::default(),
                        );
                    } else if self.command.tool == ToolOperationTool::Fillet {
                        self.editor
                            .pointer_move_feature_authoring(
                                &self.fillet,
                                &scene,
                                pointer,
                                PickTolerance::default(),
                            )
                            .map_err(error)?;
                    } else {
                        let _ = self.offset.hover_at(
                            &scene,
                            position,
                            PickTolerance::default(),
                            self.editor.editor().geometry_interaction_policy(),
                        );
                    }
                } else if self.command.tool.authoring().is_some() {
                    let document = self
                        .editor
                        .presentation_session()
                        .ok_or_else(|| error("missing authoring document"))?
                        .design_document();
                    let outcome = self.authoring.pick_at_with_policy(
                        document,
                        &scene,
                        position,
                        PickTolerance::default(),
                        self.editor.editor().geometry_interaction_policy(),
                    );
                    self.authoring_outcome(outcome);
                } else if self.command.tool == ToolOperationTool::Fillet {
                    let outcome = self
                        .editor
                        .transact_feature_authoring_pick_at(
                            &mut self.fillet,
                            &scene,
                            position,
                            PickTolerance::default(),
                            self.symbol.clone(),
                        )
                        .map_err(error)?;
                    self.feature_outcome(outcome);
                } else {
                    let outcome = self.offset.pick_at(
                        &scene,
                        position,
                        PickTolerance::default(),
                        self.editor.editor().geometry_interaction_policy(),
                    );
                    self.offset_outcome(outcome)?;
                }
            }
            ToolOperationEvent::Pick { operand } => {
                let operand = resolve_operand(&self.editor, operand)?;
                if self.command.tool.authoring().is_some() {
                    let document = self
                        .editor
                        .presentation_session()
                        .ok_or_else(|| error("missing authoring document"))?
                        .design_document();
                    let outcome = self.authoring.pick(document, operand);
                    self.authoring_outcome(outcome);
                } else if self.command.tool == ToolOperationTool::Fillet {
                    let outcome = self
                        .editor
                        .transact_feature_authoring_pick_items(
                            &mut self.fillet,
                            &[(operand.item, operand.curve_parameter)],
                            self.symbol.clone(),
                        )
                        .map_err(error)?;
                    self.feature_outcome(outcome);
                } else {
                    self.pick_offset(operand)?;
                }
            }
            ToolOperationEvent::PickSelection { operands } => {
                if operands.len() > 256 {
                    return Err(error("tool selection exceeds bounds"));
                }
                let operands = operands
                    .iter()
                    .map(|operand| resolve_operand(&self.editor, operand))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(tool) = self.command.tool.authoring() {
                    let mut complete = self.authoring.pending().to_vec();
                    complete.extend(operands);
                    let document = self
                        .editor
                        .presentation_session()
                        .ok_or_else(|| error("missing authoring document"))?
                        .design_document();
                    let outcome = self.authoring.activate(document, tool, &complete);
                    self.authoring_outcome(outcome);
                } else if self.command.tool == ToolOperationTool::Fillet {
                    let picks = operands
                        .iter()
                        .map(|operand| (operand.item, operand.curve_parameter))
                        .collect::<Vec<_>>();
                    let outcome = self
                        .editor
                        .transact_feature_authoring_pick_items(
                            &mut self.fillet,
                            &picks,
                            self.symbol.clone(),
                        )
                        .map_err(error)?;
                    self.feature_outcome(outcome);
                } else if self.command.tool == ToolOperationTool::Offset {
                    for operand in operands {
                        self.pick_offset(operand)?;
                    }
                } else {
                    return Err(error("the selected tool cannot collect another selection"));
                }
            }
            ToolOperationEvent::Complete => {
                let outcome = if self.command.tool == ToolOperationTool::Fillet {
                    self.editor
                        .apply_computed_fillet_preview(&mut self.fillet, self.symbol.clone())
                } else if self.command.tool == ToolOperationTool::Offset {
                    self.editor
                        .apply_profile_offset_preview(&mut self.offset, self.symbol.clone())
                } else {
                    self.diagnostic =
                        Some("Choose the remaining relation or dimension operands".into());
                    return Ok(());
                };
                match outcome {
                    Ok(outcome) if outcome.disposition == IntentPlanDisposition::Accepted => {
                        self.completed = true;
                    }
                    Ok(_) => self.diagnostic = Some("Tool candidate was rejected".into()),
                    Err(error) => self.diagnostic = Some(error.to_string()),
                }
            }
            ToolOperationEvent::AuthoringOptions { options } => {
                if self.command.tool.authoring().is_none() {
                    return Err(error("options do not belong to the active tool"));
                }
                self.authoring.set_options(*options);
            }
            ToolOperationEvent::FilletOptions {
                options,
                selected_corner,
            } => {
                if self.command.tool != ToolOperationTool::Fillet {
                    return Err(error("Fillet is not active"));
                }
                let outcome = self
                    .editor
                    .transact_feature_authoring_options(
                        &mut self.fillet,
                        *options,
                        *selected_corner,
                        self.symbol.clone(),
                    )
                    .map_err(error)?;
                self.feature_outcome(outcome);
            }
            ToolOperationEvent::FilletRadius { radius } => {
                if self.command.tool != ToolOperationTool::Fillet {
                    return Err(error("Fillet is not active"));
                }
                let outcome = self
                    .editor
                    .transact_feature_authoring_radius(
                        &mut self.fillet,
                        *radius,
                        self.symbol.clone(),
                    )
                    .map_err(error)?;
                self.feature_outcome(outcome);
            }
            ToolOperationEvent::OffsetDistance { distance } => {
                if self.command.tool != ToolOperationTool::Offset {
                    return Err(error("Offset is not active"));
                }
                let outcome = self
                    .editor
                    .transact_offset_authoring_distance(
                        &mut self.offset,
                        *distance,
                        self.symbol.clone(),
                    )
                    .map_err(error)?;
                self.offset_outcome(outcome)?;
            }
            ToolOperationEvent::OffsetFlip => {
                if self.command.tool != ToolOperationTool::Offset {
                    return Err(error("Offset is not active"));
                }
                let outcome = self.offset.flip();
                self.offset_outcome(outcome)?;
            }
            ToolOperationEvent::StepBack | ToolOperationEvent::Reset => {
                if self.command.tool.authoring().is_some() {
                    if self.authoring.pending().is_empty() {
                        return Ok(());
                    }
                    let document = self
                        .editor
                        .presentation_session()
                        .ok_or_else(|| error("missing authoring document"))?
                        .design_document();
                    let outcome = self.authoring.cancel(document);
                    self.authoring_outcome(outcome);
                } else if self.command.tool == ToolOperationTool::Fillet {
                    if self.fillet.completed_corner_count() > 0
                        || self.fillet.guidance().stage
                            == geosolve_constraint_editor::FeatureAuthoringStage::PickSecondFilletCurve
                    {
                        let outcome = self.fillet.cancel();
                        self.feature_outcome(outcome);
                    }
                    self.editor.clear_authoring_previews();
                } else {
                    let outcome = if matches!(input, ToolOperationEvent::Reset) {
                        self.offset.reset()
                    } else {
                        self.offset.backspace()
                    };
                    self.offset_outcome(outcome)?;
                }
            }
        }
        Ok(())
    }
    /// Current native collector state, including immediate preselection completion.
    /// # Errors
    /// Rejects missing semantic ownership of pending native operands.
    pub fn frame(&self) -> Result<ToolOperationFrame, EngineError> {
        let has_pending = !self.completed
            && (!self.authoring.pending().is_empty()
                || self.fillet.completed_corner_count() > 0
                || self.fillet.guidance().stage
                    == geosolve_constraint_editor::FeatureAuthoringStage::PickSecondFilletCurve
                || self.offset.operand().is_some());
        Ok(ToolOperationFrame {
            sequence: u32::try_from(self.command.samples.len()).map_err(error)?,
            completed: self.completed,
            can_finish: !self.completed
                && (self.editor.feature_authoring_preview_matches(&self.fillet)
                    || self.editor.offset_authoring_preview_matches(&self.offset)),
            has_pending,
            can_reset: has_pending,
            can_step_back: has_pending,
            diagnostic: self.diagnostic.clone(),
            pending: self
                .authoring
                .pending()
                .iter()
                .map(|operand| {
                    operand_for_item(&self.editor, operand.item, operand.curve_parameter)
                })
                .collect::<Result<_, _>>()?,
            authoring_options: self.authoring.options(),
            fillet_options: self.fillet.options(),
            fillet_corner_count: self.fillet.completed_corner_count(),
            fillet_corners: match self.fillet.apply() {
                FeatureAuthoringOutcome::Apply(candidate) => candidate
                    .corners()
                    .iter()
                    .enumerate()
                    .map(|(index, corner)| ToolFilletCornerOptions {
                        index,
                        options: FeatureAuthoringOptions {
                            fillet_radius: Some(candidate.radius()),
                            flip_first_side: corner.options.flip_first_side,
                            flip_second_side: corner.options.flip_second_side,
                            alternate_arc: corner.options.alternate_arc,
                        },
                    })
                    .collect(),
                _ => Vec::new(),
            },
            offset_distance: self.offset.distance(),
        })
    }
    /// Detached provisional scene and source namespace correspondence.
    /// # Errors
    /// Rejects unavailable native authority or encoding failure.
    pub fn presentation_json(&self) -> Result<String, EngineError> {
        let bindings = self
            .editor
            .presentation_bindings()
            .ok_or_else(|| error("missing prediction source bindings"))?;
        serde_json::to_string(&serde_json::json!({"scene":self.scene_json()?,"bindings":bindings,"operation":self.presentation()}))
            .map_err(error)
    }
    /// # Errors
    /// Rejects unavailable native scene.
    pub fn scene_json(&self) -> Result<String, EngineError> {
        self.editor
            .scene(self.viewport, CHORD_TOLERANCE_PIXELS)
            .map_err(error)?
            .to_detached_json()
            .map_err(error)
    }
    /// Consumes a completed native operation into source-level replay intent.
    /// # Errors
    /// Rejects incomplete or unsupported source projections.
    pub fn finish(mut self, gesture_id: u64) -> Result<ToolOperationTerminal, EngineError> {
        self.authenticate(gesture_id)?;
        if !self.completed {
            return Err(error("tool operation has no accepted terminal"));
        }
        let origin = &self.origin.0.materialized;
        if self.command.tool == ToolOperationTool::ToggleGeometryRole {
            self.command.expected_mutation = Some(role_mutation(
                &self.project,
                &origin.expansion,
                &origin.editor,
                &self.editor,
            )?);
            return Ok(ToolOperationTerminal {
                command: self.command,
                symbol: self.symbol,
                editor: self.editor,
                declarations: Vec::new(),
                high_water: self.project.managed.declaration_name_high_water,
            });
        }
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
        Ok(ToolOperationTerminal {
            command: self.command,
            symbol: self.symbol,
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
            return Err(error("tool operation belongs to another gesture"));
        }
        Ok(())
    }
}

fn role_mutation(
    project: &CodeProject,
    expansion: &geosolve_sketch_code::ExpandedCodeProject,
    origin: &ProjectionalEditorSession,
    candidate: &ProjectionalEditorSession,
) -> Result<ManagedSketchMutation, EngineError> {
    let compiled = project
        .managed
        .compiled
        .as_deref()
        .ok_or_else(|| error("missing compiled source"))?;
    let graph = origin.coordinator().intent().graph();
    let candidate_graph = candidate.coordinator().intent().graph();
    let mut values = Vec::new();
    for node in graph.nodes().values() {
        let next = candidate_graph
            .node(node.id)
            .ok_or_else(|| error("role edit removed a native owner"))?;
        if next.fields == node.fields {
            continue;
        }
        let matching = project
            .managed
            .program
            .declarations
            .iter()
            .filter(|declaration| {
                expansion.declaration_provenance.get(&node.symbol) == Some(&declaration.symbol)
                    && declaration
                        .builder_path
                        .first()
                        .is_some_and(|namespace| namespace == "geometry")
            })
            .collect::<Vec<_>>();
        let [declaration] = matching.as_slice() else {
            return Err(error(
                "Selected geometry role is owned by generated source; edit its authored role parameter",
            ));
        };
        let role = next
            .fields
            .iter()
            .find(|(field, _)| field.0.as_str() == "role")
            .and_then(|(_, value)| match value {
                geosolve_sketch_intent::IntentLiteral::Enum(value) => Some(value.as_str()),
                _ => None,
            })
            .ok_or_else(|| error("native role edit lacks explicit role"))?;
        let mut mutation = geosolve_sketch_code::derive_managed_value_mutation(
            compiled,
            &declaration.symbol,
            &geosolve_sketch_code::SemanticOutputPath(Vec::new()),
            declaration.arguments.clone(),
        )
        .map_err(error)?;
        let ManagedValue::Object(arguments) = &mut mutation.value else {
            return Err(error(
                "geometry role requires an authored object definition",
            ));
        };
        arguments.insert("role".into(), ManagedValue::String(role.into()));
        values.push(mutation);
    }
    if values.is_empty() {
        return Err(error("geometry role edit changed no source owners"));
    }
    Ok(ManagedSketchMutation::SetValues { values })
}
