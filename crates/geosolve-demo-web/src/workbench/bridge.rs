// SPDX-License-Identifier: GPL-3.0-or-later

//! Instance-scoped, DOM-free workbench bridge for presentation toolkits.
//!
//! React, a native shell, a worker, and tests all consume this same bounded
//! JSON contract. The bridge owns exactly one existing Rust document/editor
//! authority; it does not duplicate solver equations, accepted geometry,
//! managed-code history, or persistence state.

// Native unit tests compile this WASM bridge to exercise its DOM-free core,
// while pointer/RPC entry points are consumed by the wasm32 wrapper itself.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::str::FromStr as _;

use geosolve_constraint_editor::{
    ActivePointerGestureKind, AuthoringOperand, AuthoringOutcome, AuthoringState, EditorEffect,
    EditorScene, EditorTool, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, GeometryRoleSelectionState, GeometryToolVariant,
    GeometryVisibility, Modifiers, OffsetAuthoringOutcome, OffsetAuthoringState, PickTolerance,
    PointerInput, ScreenPoint, SelectionItem,
};
use geosolve_sketch::GeometryRole;
use geosolve_sketch_code::{
    ExecutedGeneratedMemberAddress, ManagedControlAccess, ManagedMutationTarget,
    ManagedSketchMutation, ManagedSpan, ManagedValue, PreparedManagedMutationReceipt,
    PreparedManagedMutationRequest, PreparedManagedSourceRequest,
};
use geosolve_sketch_intent::{IntentKey, NodeId};
use serde::{Deserialize, Serialize};

use super::code_projects::{
    CodeProjectWorkbench, ManagedControlSubmission, ManagedDeclarationClosureRole,
    ManagedDeclarationPanelRow, ManagedGeneratedPanelRow, PreparedManagedCanvasMutation,
    PreparedManagedSourceApply, ResolvedManagedSourceApply,
};
use super::design_projection::node_family_label;
use super::{
    WorkbenchDocumentAuthority, dispatch_projectional_authoring_application,
    dispatch_projectional_construction_effects, fit_projectional_camera_to_authority,
};

const PROTOCOL_VERSION: u8 = 1;
const MAX_REQUEST_BYTES: usize = 40 * 1024 * 1024;
const MAX_TOOL_CATALOG_BYTES: usize = 128 * 1024;
const MAX_COMMAND_BYTES: usize = 1_024;
const MAX_TITLE_BYTES: usize = 1_024;
const MAX_HOST_EXTENT: f64 = 32_768.0;
const MAX_PIXEL_RATIO: f64 = 16.0;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_VISIBILITY_ROWS: usize = 4_096;
const MIDDLE_POINTER_BUTTON: u16 = 4;
const WORKBENCH_PERSISTENCE_FORMAT: &str = "geosolve-workbench-presentation-v1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConstructRequest {
    version: u8,
    #[serde(default)]
    persisted_project: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRequest {
    version: u8,
    command: String,
    #[serde(default)]
    payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct ModifierRequest {
    #[serde(rename = "alt")]
    _alt: bool,
    ctrl: bool,
    meta: bool,
    shift: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PointerPhase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PointerRequest {
    version: u8,
    phase: PointerPhase,
    pointer_id: u64,
    x: f64,
    y: f64,
    buttons: u16,
    modifiers: ModifierRequest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasPanGesture {
    pointer_id: u64,
    origin: ScreenPoint,
    origin_center: [f64; 2],
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WheelRequest {
    version: u8,
    x: f64,
    y: f64,
    delta_x: f64,
    delta_y: f64,
    #[serde(rename = "ctrl")]
    _ctrl: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResizeRequest {
    version: u8,
    width: f64,
    height: f64,
    pixel_ratio: f64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CancelReason {
    Escape,
    LostCapture,
    Blur,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelRequest {
    version: u8,
    reason: CancelReason,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SamplePayload {
    key: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolPayload {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcePayload {
    path: String,
    contents: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcePathPayload {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionPayload {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclarationSourcePayload {
    id: String,
    from: usize,
    to: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DeclarationMoveDirection {
    Up,
    Down,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DeclarationMovePosition {
    Before,
    After,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeclarationMovePayload {
    id: String,
    #[serde(default)]
    direction: Option<DeclarationMoveDirection>,
    #[serde(default)]
    target_id: Option<String>,
    #[serde(default)]
    position: Option<DeclarationMovePosition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclarationSuppressionPayload {
    id: String,
    suppressed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplorerVisibilityPayload {
    id: String,
    visible: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplorerIsolatePayload {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParameterPayload {
    id: String,
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManagedMutationAbortPayload {
    ticket_digest: String,
    candidate_source: String,
    diagnostic: String,
    span: ManagedSpan,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum ProjectStatus {
    Accepted,
    Dirty,
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSnapshot {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_key: Option<String>,
    status: ProjectStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameSnapshot {
    svg: String,
    aria_label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
// These independent capability flags are the stable JSON wire contract for
// presentation hosts; collapsing them into an enum would make valid states
// unrepresentable and move policy inference into the frontend.
#[allow(clippy::struct_excessive_bools)]
struct PresentationSnapshot {
    active_tool: String,
    grid_visible: bool,
    construction_visible: bool,
    visibility_restore_available: bool,
    can_undo: bool,
    can_redo: bool,
    can_finish: bool,
    geometry_role: GeometryRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_geometry_role: Option<GeometryRoleStateSnapshot>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum GeometryRoleStateSnapshot {
    Profile,
    Construction,
    Mixed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceFileSnapshot {
    path: String,
    language: &'static str,
    contents: String,
    read_only: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSnapshot {
    selected_path: String,
    files: Vec<SourceFileSnapshot>,
    dirty: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExplorerSnapshot {
    id: String,
    label: String,
    kind: String,
    row_kind: ExplorerRowKind,
    selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    suppressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<SelectionSourceSnapshot>,
    visible: bool,
    effective_visible: bool,
    visibility_state: ExplorerVisibilitySnapshot,
    children: Vec<Self>,
    capabilities: ExplorerCapabilities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ExplorerRowKind {
    Group,
    Declaration,
    Generated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ExplorerVisibilitySnapshot {
    Visible,
    Hidden,
    Mixed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExplorerCapabilities {
    select: ExplorerCapability,
    navigate: ExplorerCapability,
    edit: ExplorerCapability,
    r#move: ExplorerCapability,
    move_up: ExplorerCapability,
    move_down: ExplorerCapability,
    suppress: ExplorerCapability,
    delete: ExplorerCapability,
}

#[derive(Debug, Serialize)]
struct ExplorerCapability {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct SelectionSnapshot {
    id: String,
    label: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ownership: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<SelectionSourceSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
struct SelectionSourceSnapshot {
    path: String,
    from: usize,
    to: usize,
}

#[derive(Debug, Serialize)]
struct ParameterSnapshot {
    id: String,
    label: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    editable: bool,
}

#[derive(Debug, Serialize)]
struct ProblemSnapshot {
    id: String,
    severity: &'static str,
    title: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}

#[derive(Debug, Serialize)]
struct BridgeSnapshot {
    version: u8,
    revision: u64,
    project: ProjectSnapshot,
    presentation: PresentationSnapshot,
    frame: FrameSnapshot,
    source: SourceSnapshot,
    explorer: Vec<ExplorerSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selection: Option<SelectionSnapshot>,
    parameters: Vec<ParameterSnapshot>,
    problems: Vec<ProblemSnapshot>,
    #[serde(rename = "pendingManagedMutation")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_managed_mutation: Option<PendingManagedMutationSnapshot>,
}

#[derive(Debug, Serialize)]
struct ExportSnapshot {
    version: u8,
    filename: &'static str,
    contents: String,
}

#[derive(Debug, Serialize)]
struct PersistenceSnapshot {
    version: u8,
    contents: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkbenchPersistenceEnvelope {
    format: String,
    project: String,
    presentation: WorkbenchPresentationPersistence,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkbenchPresentationPersistence {
    hidden_rows: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    isolate_restore: Option<Vec<String>>,
    construction_visible: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ExplorerVisibilityState {
    hidden_rows: std::collections::BTreeSet<String>,
    isolate_restore: Option<std::collections::BTreeSet<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedCompilerContextSnapshot {
    version: u8,
    patches: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug)]
enum DeclarationRowTarget {
    Intent {
        node: NodeId,
    },
    Managed {
        node: Option<NodeId>,
        symbol: String,
        from: usize,
        to: usize,
        suppression_control_id: Option<String>,
        closure_role: ManagedDeclarationClosureRole,
    },
    Generated {
        node: Option<NodeId>,
        address: geosolve_sketch_code::GeneratedMemberAddress,
        from: usize,
        to: usize,
        suppressed: bool,
        suppression_token: Option<String>,
    },
}

enum PendingManagedMutation {
    Managed {
        label: String,
        prepared: Box<PreparedManagedCanvasMutation>,
    },
    Source {
        prepared: Box<PreparedManagedSourceApply>,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PendingManagedMutationSnapshot {
    Managed {
        request: Box<PreparedManagedMutationRequest>,
    },
    Source {
        request: Box<PreparedManagedSourceRequest>,
    },
}

fn pending_managed_ticket_digest(pending: &PendingManagedMutation) -> &str {
    match pending {
        PendingManagedMutation::Managed { prepared, .. } => &prepared.request.ticket.ticket_digest,
        PendingManagedMutation::Source { prepared } => &prepared.request.ticket.ticket_digest,
    }
}

fn pending_managed_snapshot(pending: &PendingManagedMutation) -> PendingManagedMutationSnapshot {
    match pending {
        PendingManagedMutation::Managed { prepared, .. } => {
            PendingManagedMutationSnapshot::Managed {
                request: Box::new(prepared.request.clone()),
            }
        }
        PendingManagedMutation::Source { prepared } => PendingManagedMutationSnapshot::Source {
            request: Box::new(prepared.request.clone()),
        },
    }
}

/// One presentation-toolkit instance over the existing Rust authorities.
pub(crate) struct WorkbenchBridge {
    authority: WorkbenchDocumentAuthority,
    code_project: Option<CodeProjectWorkbench>,
    samples: super::samples::SampleCatalogState,
    camera: super::scene::CanvasCamera,
    retained_scene: Option<EditorScene>,
    accepted_frame: Option<FrameSnapshot>,
    preserve_frame_once: bool,
    construction_preview: Option<geosolve_constraint_editor::ConstructionPreview>,
    authoring: AuthoringState,
    feature_authoring: FeatureAuthoringState,
    offset_authoring: OffsetAuthoringState,
    active_tool: String,
    grid_visible: bool,
    explorer_visibility: ExplorerVisibilityState,
    title: String,
    host_size: [f64; 2],
    pixel_ratio: f64,
    captured_pointer: Option<u64>,
    canvas_pan: Option<CanvasPanGesture>,
    revision: u64,
    notice: String,
    last_error: Option<String>,
    interaction_trace: super::interaction_trace::InteractionTrace,
    pending_managed_mutation: Option<PendingManagedMutation>,
}

impl WorkbenchBridge {
    /// Constructs a fresh or atomically restored DOM-free workbench.
    pub(crate) fn construct_json(request: &str) -> Result<Self, String> {
        let request: ConstructRequest = decode_request(request)?;
        require_version(request.version)?;
        match request.persisted_project {
            Some(project) => Self::restore(&project),
            None => Self::fresh(),
        }
    }

    fn fresh() -> Result<Self, String> {
        Self::from_parts(
            super::fresh_projectional_authority()?,
            None,
            super::samples::SampleCatalogState::default(),
            "Untitled sketch".into(),
            "Projectional workspace ready".into(),
        )
    }

    fn restore(encoded: &str) -> Result<Self, String> {
        if encoded.len() > MAX_REQUEST_BYTES {
            return Err(format!(
                "persisted project exceeds the {MAX_REQUEST_BYTES}-byte bridge limit"
            ));
        }
        if let Ok(envelope) = serde_json::from_str::<WorkbenchPersistenceEnvelope>(encoded)
            && envelope.format == WORKBENCH_PERSISTENCE_FORMAT
        {
            let hidden_rows = decode_visibility_rows(envelope.presentation.hidden_rows)?;
            let isolate_restore = envelope
                .presentation
                .isolate_restore
                .map(decode_visibility_rows)
                .transpose()?;
            let mut bridge = Self::restore(&envelope.project)?;
            bridge.explorer_visibility = ExplorerVisibilityState {
                hidden_rows,
                isolate_restore,
            };
            let visibility = GeometryVisibility {
                explicit_construction: envelope.presentation.construction_visible,
                implicit_construction: envelope.presentation.construction_visible,
                ..bridge
                    .editor()
                    .editor()
                    .geometry_interaction_policy()
                    .visibility
            };
            let _ = bridge
                .editor_mut()
                .editor_mut()
                .set_geometry_visibility(visibility);
            bridge.retained_scene = None;
            bridge.accepted_frame = None;
            return Ok(bridge);
        }
        if let Ok(workspace) = crate::reproduction::decode_workspace(encoded) {
            return Self::restore(&workspace).map(|mut bridge| {
                bridge.notice = "Reproduction restored atomically".into();
                bridge
            });
        }
        if let Ok(code_project) = CodeProjectWorkbench::from_persistence_json(encoded) {
            let editor = code_project.restore_accepted_editor()?;
            let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)?;
            let mut samples = super::samples::SampleCatalogState::default();
            if let Some(key) = code_project.demo_key() {
                samples.select_code_key(key)?;
            }
            let title = code_project.title().to_owned();
            return Self::from_parts(
                authority,
                Some(code_project),
                samples,
                title,
                "Code project restored".into(),
            );
        }
        if let Ok((code_project, editor)) =
            CodeProjectWorkbench::import_canonical_project_json(encoded)
        {
            let title = code_project.title().to_owned();
            return Self::from_parts(
                WorkbenchDocumentAuthority::from_projectional_editor(*editor)?,
                Some(code_project),
                super::samples::SampleCatalogState::default(),
                title,
                "Canonical code project imported".into(),
            );
        }
        let snapshot = super::persistence::WorkspaceSnapshot::decode(encoded)?;
        let authority = WorkbenchDocumentAuthority::from_snapshot(&snapshot)?;
        Self::from_parts(
            authority,
            None,
            super::samples::SampleCatalogState::default(),
            "Restored sketch".into(),
            "Workspace restored".into(),
        )
    }

    fn from_parts(
        authority: WorkbenchDocumentAuthority,
        code_project: Option<CodeProjectWorkbench>,
        samples: super::samples::SampleCatalogState,
        title: String,
        notice: String,
    ) -> Result<Self, String> {
        if !authority.is_projectional() {
            return Err("the React bridge requires projectional editor authority".into());
        }
        let mut bridge = Self {
            authority,
            code_project,
            samples,
            camera: super::scene::CanvasCamera::default(),
            retained_scene: None,
            accepted_frame: None,
            preserve_frame_once: false,
            construction_preview: None,
            authoring: AuthoringState::default(),
            feature_authoring: FeatureAuthoringState::default(),
            offset_authoring: OffsetAuthoringState::default(),
            active_tool: "select".into(),
            grid_visible: true,
            explorer_visibility: ExplorerVisibilityState::default(),
            title,
            host_size: super::scene::SCREEN_SIZE,
            pixel_ratio: 1.0,
            captured_pointer: None,
            canvas_pan: None,
            revision: 0,
            notice,
            last_error: None,
            interaction_trace: super::interaction_trace::InteractionTrace::default(),
            pending_managed_mutation: None,
        };
        let _ = fit_projectional_camera_to_authority(&mut bridge.camera, &bridge.authority);
        Ok(bridge)
    }

    pub(crate) fn snapshot_json(&mut self) -> Result<String, String> {
        let snapshot = self.snapshot()?;
        serde_json::to_string(&snapshot).map_err(|error| error.to_string())
    }

    /// Returns the immutable Rust-owned CAD tool and icon catalog.
    ///
    /// This capability is deliberately separate from [`Self::snapshot_json`]
    /// so pointer and camera traffic never retransmits static vector data.
    pub(crate) fn tool_catalog_json() -> Result<String, String> {
        let encoded = serde_json::to_string(&super::command_manifest::tool_catalog())
            .map_err(|error| error.to_string())?;
        if encoded.len() > MAX_TOOL_CATALOG_BYTES {
            return Err(format!(
                "tool catalog exceeds the {MAX_TOOL_CATALOG_BYTES}-byte bridge limit"
            ));
        }
        Ok(encoded)
    }

    /// Returns heavyweight, immutable patch plans only when a compiler host
    /// is about to execute managed source. Keeping this separate from the
    /// hot snapshot makes canvas traffic independent of project artifact size.
    pub(crate) fn managed_compiler_context_json(&self) -> Result<String, String> {
        let patches = self
            .code_project
            .as_ref()
            .ok_or_else(|| "this workbench instance has no active code project".to_owned())?
            .managed_compiler_patches()?;
        serde_json::to_string(&ManagedCompilerContextSnapshot {
            version: PROTOCOL_VERSION,
            patches,
        })
        .map_err(|error| error.to_string())
    }

    pub(crate) fn dispatch_json(&mut self, request: &str) -> Result<String, String> {
        let request: CommandRequest = decode_request(request)?;
        require_version(request.version)?;
        if request.command.is_empty() || request.command.len() > MAX_COMMAND_BYTES {
            return Err("workbench command name is empty or too large".into());
        }
        self.last_error = None;
        self.dispatch(&request.command, request.payload)?;
        self.snapshot_json()
    }

    pub(crate) fn pointer_json(&mut self, request: &str) -> Result<String, String> {
        if self.pending_managed_mutation.is_some() {
            return Err(
                "pointer input is unavailable while a managed-source mutation is compiling".into(),
            );
        }
        let request: PointerRequest = decode_request(request)?;
        require_version(request.version)?;
        if request.pointer_id > MAX_SAFE_INTEGER {
            return Err("pointerId must be a JavaScript-safe unsigned integer".into());
        }
        if !request.x.is_finite() || !request.y.is_finite() {
            return Err("pointer coordinates must be finite".into());
        }
        let Some(input) = self.normalized_pointer(request) else {
            return Ok("null".into());
        };
        let trace_detail = format!(
            "pointer={} client=[{:.3},{:.3}] logical=[{:.9},{:.9}] buttons={} shift={} ctrl={} meta={} revision={}",
            request.pointer_id,
            request.x,
            request.y,
            input.position.x,
            input.position.y,
            request.buttons,
            request.modifiers.shift,
            request.modifiers.ctrl,
            request.modifiers.meta,
            self.revision,
        );
        match request.phase {
            PointerPhase::Down => self
                .interaction_trace
                .begin_gesture("browser.pointerdown", &trace_detail),
            PointerPhase::Move => self
                .interaction_trace
                .record("browser.pointermove", &trace_detail),
            PointerPhase::Up => self
                .interaction_trace
                .record("browser.pointerup", &trace_detail),
        }
        self.last_error = None;
        if !self.route_canvas_pan(request, input) {
            match request.phase {
                PointerPhase::Down => self.pointer_down(input),
                PointerPhase::Move => self.pointer_move(input),
                PointerPhase::Up => self.pointer_up(input),
            }
        }
        if self.last_error.is_some() {
            // Interaction failures are durable Problems information while the
            // prior accepted scene remains authoritative.
            self.notice = "Interaction retained its prior accepted state".into();
        }
        self.interaction_trace.record(
            if self.last_error.is_some() {
                "bridge.presentation.retained"
            } else {
                "bridge.presentation.accepted"
            },
            format!(
                "revision={} active_tool={} captured={:?} notice={} error={}",
                self.revision,
                self.active_tool,
                self.captured_pointer,
                self.notice,
                self.last_error.as_deref().unwrap_or("none"),
            ),
        );
        self.snapshot_json()
    }

    pub(crate) fn wheel_json(&mut self, request: &str) -> Result<String, String> {
        if self.pending_managed_mutation.is_some() {
            return Err(
                "canvas navigation is unavailable while a managed-source mutation is compiling"
                    .into(),
            );
        }
        let request: WheelRequest = decode_request(request)?;
        require_version(request.version)?;
        if ![request.x, request.y, request.delta_x, request.delta_y]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err("wheel coordinates and deltas must be finite".into());
        }
        self.cancel_active_gesture(None)?;
        let anchor = self
            .normalize_client_point([request.x, request.y], false)
            .ok_or_else(|| "wheel anchor is outside the fitted sketch plane".to_owned())?;
        let factor = (-request.delta_y * 0.0015).exp();
        if self.camera.zoom_about(anchor, factor) {
            self.retained_scene = None;
            self.notice = format!(
                "Canvas zoom {:.1} px / unit",
                self.camera.pixels_per_model_unit()
            );
            self.snapshot_json()
        } else {
            Ok("null".into())
        }
    }

    /// Resize is deliberately presentation-only. It updates only CSS-to-scene
    /// coordinate normalization and publishes no replacement frame.
    pub(crate) fn resize_json(&mut self, request: &str) -> Result<String, String> {
        let request: ResizeRequest = decode_request(request)?;
        require_version(request.version)?;
        if !request.width.is_finite()
            || !request.height.is_finite()
            || !request.pixel_ratio.is_finite()
            || request.width <= 0.0
            || request.height <= 0.0
            || request.width > MAX_HOST_EXTENT
            || request.height > MAX_HOST_EXTENT
            || request.pixel_ratio <= 0.0
            || request.pixel_ratio > MAX_PIXEL_RATIO
        {
            return Err("resize requires finite bounded positive presentation extents".into());
        }
        self.host_size = [request.width, request.height];
        self.pixel_ratio = request.pixel_ratio;
        Ok("null".into())
    }

    pub(crate) fn cancel_json(&mut self, request: &str) -> Result<String, String> {
        if self.pending_managed_mutation.is_some() {
            return Err(
                "interaction cancellation is unavailable while a managed-source mutation is compiling"
                    .into(),
            );
        }
        let request: CancelRequest = decode_request(request)?;
        require_version(request.version)?;
        let pointer = self.captured_pointer;
        self.cancel_active_gesture(pointer)?;
        self.notice = match request.reason {
            CancelReason::Escape => "Interaction canceled",
            CancelReason::LostCapture => "Interaction canceled after pointer capture was lost",
            CancelReason::Blur => "Interaction canceled after the workbench lost focus",
        }
        .into();
        self.snapshot_json()
    }

    pub(crate) fn export_project_json(&self) -> Result<String, String> {
        let export = if self.code_project.is_some() {
            let files =
                super::code_projects::canonical_code_project_files(self.code_project.as_ref())
                    .map_err(|error| error.to_string())?;
            ExportSnapshot {
                version: PROTOCOL_VERSION,
                filename: "project.json",
                contents: files.project_json,
            }
        } else {
            ExportSnapshot {
                version: PROTOCOL_VERSION,
                filename: "project.json",
                contents: self.authority.snapshot()?.encode()?,
            }
        };
        serde_json::to_string(&export).map_err(|error| error.to_string())
    }

    /// Exact application persistence for the browser-owned offline slot.
    pub(crate) fn persistence_json(&self) -> Result<String, String> {
        let contents = self.persistence_contents()?;
        serde_json::to_string(&PersistenceSnapshot {
            version: PROTOCOL_VERSION,
            contents,
        })
        .map_err(|error| error.to_string())
    }

    fn persistence_contents(&self) -> Result<String, String> {
        let project = self.code_project.as_ref().map_or_else(
            || {
                self.authority
                    .snapshot()
                    .and_then(|snapshot| snapshot.encode())
            },
            CodeProjectWorkbench::to_persistence_json,
        )?;
        let construction = self
            .editor()
            .editor()
            .geometry_interaction_policy()
            .visibility;
        serde_json::to_string(&WorkbenchPersistenceEnvelope {
            format: WORKBENCH_PERSISTENCE_FORMAT.into(),
            project,
            presentation: WorkbenchPresentationPersistence {
                hidden_rows: self
                    .explorer_visibility
                    .hidden_rows
                    .iter()
                    .cloned()
                    .collect(),
                isolate_restore: self
                    .explorer_visibility
                    .isolate_restore
                    .as_ref()
                    .map(|rows| rows.iter().cloned().collect()),
                construction_visible: construction.explicit_construction
                    && construction.implicit_construction,
            },
        })
        .map_err(|error| error.to_string())
    }

    pub(crate) fn reproduction_json(&self) -> Result<String, String> {
        let contents = crate::reproduction::encode_workspace(&self.persistence_contents()?)
            .map_err(|error| error.to_string())?;
        serde_json::to_string(&ExportSnapshot {
            version: PROTOCOL_VERSION,
            filename: "geosolve-reproduction.txt",
            contents,
        })
        .map_err(|error| error.to_string())
    }

    pub(crate) fn interaction_trace_json(&self) -> Result<String, String> {
        let context = format!(
            "project={} revision={} code={} captured={:?}",
            self.title,
            self.revision,
            self.code_project.is_some(),
            self.captured_pointer,
        );
        let contents = self.interaction_trace.export(&context, &self.notice);
        serde_json::to_string(&ExportSnapshot {
            version: PROTOCOL_VERSION,
            filename: "geosolve-interaction-trace.txt",
            contents,
        })
        .map_err(|error| error.to_string())
    }

    pub(crate) fn apply_intent_rpc_json(&mut self, request: &str) -> String {
        if let Some(response) =
            super::live_intent_rpc::code_authority_rejection(self.code_project.is_some(), request)
        {
            return response;
        }
        let before = self.editor().coordinator().intent().identity();
        let response =
            geosolve_constraint_editor::apply_intent_rpc_json_to_editor(self.editor_mut(), request);
        if self.editor().coordinator().intent().identity() != before {
            self.bump_revision();
            self.retained_scene = None;
            self.notice = "Design intent updated through RPC".into();
        }
        response
    }

    pub(crate) fn apply_code_control_rpc_json(&mut self, request: &str) -> String {
        if self.pending_managed_mutation.is_some()
            && super::code_control_rpc::request_may_change_identity(request)
        {
            return super::code_control_rpc::encode_failure(
                "managed_mutation_pending",
                "resolve or abort the prepared managed-source mutation before moving code history",
                self.code_project
                    .as_ref()
                    .map(|code| code.code_session_identity().clone()),
            );
        }
        let Some(code_project) = self.code_project.as_mut() else {
            return super::code_control_rpc::encode_failure(
                "code_workbench_unavailable",
                "this workbench instance has no active code project",
                None,
            );
        };
        let application = super::code_control_rpc::apply_to_code_project(code_project, request);
        if let Some(editor) = application.editor {
            match WorkbenchDocumentAuthority::from_projectional_editor(*editor) {
                Ok(authority) => {
                    self.authority = authority;
                    self.retained_scene = None;
                }
                Err(error) => {
                    return super::code_control_rpc::encode_failure(
                        "editor_publication_rejected",
                        &error,
                        Some(code_project.code_session_identity().clone()),
                    );
                }
            }
        }
        if application.identity_changed {
            self.bump_revision();
            self.notice = "Managed code history updated".into();
        }
        application.response
    }

    fn dispatch(&mut self, command: &str, payload: serde_json::Value) -> Result<(), String> {
        if self.pending_managed_mutation.is_some()
            && !command_allowed_while_managed_mutation_pending(command)
        {
            return Err(
                "a prepared managed-source mutation is awaiting its compiler receipt".into(),
            );
        }
        match command {
            "managed.mutation.resolve" => {
                let receipt: PreparedManagedMutationReceipt = decode_payload(payload)?;
                self.resolve_pending_managed_mutation(receipt)
            }
            "managed.mutation.abort" => {
                let payload: ManagedMutationAbortPayload = decode_payload(payload)?;
                self.abort_pending_managed_mutation(&payload)
            }
            "project.new" => self.new_sketch(),
            "project.new-code" => self.new_code_project(),
            "project.import" => {
                let payload: SourcePayload = decode_payload(payload)?;
                let mut replacement = Self::restore(&payload.contents)?;
                // Import atomically replaces document/application authority, not the
                // live presentation host. ResizeObserver may not emit another sample
                // when the surrounding DOM box is unchanged, so retain the exact CSS
                // extent and device scale already authenticated by `resize_json`.
                replacement.host_size = self.host_size;
                replacement.pixel_ratio = self.pixel_ratio;
                *self = replacement;
                Ok(())
            }
            "sample.open" => {
                let payload: SamplePayload = decode_payload(payload)?;
                self.open_sample(&payload.key, payload.title.as_deref())
            }
            "history.undo" => self.step_history(true),
            "history.redo" => self.step_history(false),
            "tool.select" => {
                let payload: ToolPayload = decode_payload(payload)?;
                self.select_tool(&payload.id)
            }
            "geometry.role.toggle" => self.toggle_geometry_role(),
            "geometry.authoring-role.toggle" => {
                self.toggle_authoring_geometry_role();
                Ok(())
            }
            "source.change" => {
                let payload: SourcePayload = decode_payload(payload)?;
                self.change_source(&payload.path, payload.contents)
            }
            "source.select" => {
                let payload: SourcePathPayload = decode_payload(payload)?;
                self.select_source(&payload.path)
            }
            "source.prepare" => {
                let payload: SourcePayload = decode_payload(payload)?;
                self.change_source(&payload.path, payload.contents)?;
                self.prepare_source_apply()
            }
            "source.revert" => self.revert_source(),
            "selection.select" => {
                let payload: SelectionPayload = decode_payload(payload)?;
                self.select_declaration(&payload.id)
            }
            "declaration.select" => {
                let payload: SelectionPayload = decode_payload(payload)?;
                self.select_declaration_row(&payload.id)
            }
            "declaration.source.open" => {
                let payload: DeclarationSourcePayload = decode_payload(payload)?;
                self.open_declaration_source(&payload.id, payload.from, payload.to)
            }
            "declaration.move" => {
                let payload: DeclarationMovePayload = decode_payload(payload)?;
                self.move_declaration(&payload)
            }
            "declaration.suppression.set" => {
                let payload: DeclarationSuppressionPayload = decode_payload(payload)?;
                self.set_declaration_suppressed(&payload.id, payload.suppressed)
            }
            "explorer.visibility.set"
            | "explorer.visibility.isolate"
            | "explorer.visibility.restore"
            | "view.construction.toggle" => self.dispatch_explorer_presentation(command, payload),
            "declaration.delete" => {
                let payload: SelectionPayload = decode_payload(payload)?;
                self.delete_declaration_row(&payload.id)
            }
            "parameter.edit" => {
                let payload: ParameterPayload = decode_payload(payload)?;
                self.edit_parameter(&payload.id, payload.value)
            }
            "feature.apply" | "tool.finish" => self.finish_active_tool(),
            "view.grid.toggle" => {
                self.toggle_grid();
                Ok(())
            }
            "view.fit" => self.fit_canvas(),
            "view.origin" => self.center_canvas_origin(),
            _ => Err(format!("unknown workbench command `{command}`")),
        }
    }

    fn dispatch_explorer_presentation(
        &mut self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        match command {
            "explorer.visibility.set" => {
                let payload: ExplorerVisibilityPayload = decode_payload(payload)?;
                self.set_explorer_row_visible(&payload.id, payload.visible)
            }
            "explorer.visibility.isolate" => {
                let payload: ExplorerIsolatePayload = decode_payload(payload)?;
                self.isolate_explorer_group(&payload.id)
            }
            "explorer.visibility.restore" => self.restore_explorer_visibility(),
            "view.construction.toggle" => self.toggle_construction_visibility(),
            _ => Err(format!("unknown Explorer presentation command `{command}`")),
        }
    }

    fn new_sketch(&mut self) -> Result<(), String> {
        self.cancel_active_interaction(None)?;
        self.authority = super::fresh_projectional_authority()?;
        self.code_project = None;
        self.samples = super::samples::SampleCatalogState::default();
        self.camera.reset();
        self.reset_explorer_visibility();
        self.reset_transient_tools();
        self.title = "Untitled sketch".into();
        self.notice = "New sketch created".into();
        self.bump_revision();
        Ok(())
    }

    fn new_code_project(&mut self) -> Result<(), String> {
        self.cancel_active_interaction(None)?;
        let (code_project, editor) = CodeProjectWorkbench::new_authored()?;
        self.authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)?;
        self.title = code_project.title().into();
        self.code_project = Some(code_project);
        self.samples = super::samples::SampleCatalogState::default();
        self.camera.reset();
        self.reset_explorer_visibility();
        let _ = fit_projectional_camera_to_authority(&mut self.camera, &self.authority);
        self.reset_transient_tools();
        self.notice = "New managed code project created".into();
        self.bump_revision();
        Ok(())
    }

    fn open_sample(&mut self, key: &str, claimed_title: Option<&str>) -> Result<(), String> {
        if key.is_empty() || key.len() > MAX_COMMAND_BYTES {
            return Err("sample key is empty or too large".into());
        }
        if claimed_title.is_some_and(|title| title.len() > MAX_TITLE_BYTES) {
            return Err("sample title is too large".into());
        }
        self.cancel_active_interaction(None)?;
        if let Ok((code_project, editor)) = CodeProjectWorkbench::open_key(key) {
            let mut samples = super::samples::SampleCatalogState::default();
            samples.select_code_key(key)?;
            self.title = code_project.title().into();
            self.authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)?;
            self.code_project = Some(code_project);
            self.samples = samples;
        } else {
            let mut samples = super::samples::SampleCatalogState::default();
            let coordinator = samples.open_key(key)?;
            self.authority = WorkbenchDocumentAuthority::from_flat_coordinator(&coordinator)?;
            self.code_project = None;
            self.title = samples
                .selected_title()
                .or(claimed_title)
                .unwrap_or("Sample")
                .to_owned();
            self.samples = samples;
        }
        self.reset_explorer_visibility();
        self.reset_transient_tools();
        self.camera.reset();
        let _ = fit_projectional_camera_to_authority(&mut self.camera, &self.authority);
        self.notice = format!("{} opened", self.title);
        self.bump_revision();
        Ok(())
    }

    fn step_history(&mut self, undo: bool) -> Result<(), String> {
        self.cancel_active_interaction(None)?;
        let moved = if let Some(code_project) = self.code_project.as_mut() {
            let publication = code_project.step_history(undo)?;
            if let Some(publication) = publication {
                self.authority =
                    WorkbenchDocumentAuthority::from_projectional_editor(*publication.editor)?;
                true
            } else {
                false
            }
        } else {
            self.authority.step_history(undo)?
        };
        if moved {
            self.retained_scene = None;
            self.bump_revision();
            self.notice = if undo {
                "Undo complete"
            } else {
                "Redo complete"
            }
            .into();
        }
        Ok(())
    }

    fn change_source(&mut self, path: &str, contents: String) -> Result<(), String> {
        if contents.len() > geosolve_sketch_code::MANAGED_SOURCE_LIMIT {
            return Err(format!(
                "managed source exceeds the {}-byte limit",
                geosolve_sketch_code::MANAGED_SOURCE_LIMIT
            ));
        }
        let code = self
            .code_project
            .as_mut()
            .ok_or_else(|| "the current project has no editable managed source".to_owned())?;
        if path != "sketch.ts" {
            return Err(format!("source file `{path}` is read-only or unavailable"));
        }
        code.select_file(path)?;
        code.set_managed_draft(contents);
        self.preserve_frame_once = true;
        self.notice = "Managed source has unapplied changes".into();
        self.bump_revision();
        Ok(())
    }

    fn select_source(&mut self, path: &str) -> Result<(), String> {
        self.code_project
            .as_mut()
            .ok_or_else(|| "the current project has no code files".to_owned())?
            .select_file(path)?;
        self.notice = format!("{path} selected");
        Ok(())
    }

    fn prepare_source_apply(&mut self) -> Result<(), String> {
        self.cancel_active_interaction(None)?;
        if self.pending_managed_mutation.is_some() {
            return Err("a managed-source mutation is already awaiting compilation".into());
        }
        let prepared = self
            .code_project
            .as_ref()
            .ok_or_else(|| "the current project has no editable managed source".to_owned())?
            .prepare_managed_source_apply()?;
        self.pending_managed_mutation = Some(PendingManagedMutation::Source {
            prepared: Box::new(prepared),
        });
        self.notice = "Validating managed source candidate".into();
        Ok(())
    }

    fn revert_source(&mut self) -> Result<(), String> {
        let changed = self
            .code_project
            .as_mut()
            .ok_or_else(|| "the current project has no editable managed source".to_owned())?
            .revert_managed_draft();
        if changed {
            self.bump_revision();
        }
        self.last_error = None;
        self.preserve_frame_once = false;
        self.notice = "Managed source reverted to accepted bytes".into();
        Ok(())
    }

    fn select_declaration(&mut self, id: &str) -> Result<(), String> {
        let node = NodeId::from_str(id).map_err(|error| error.to_string())?;
        if !self.editor_mut().set_selected_declaration(Some(node)) {
            return Err("the selected declaration is unavailable".into());
        }
        self.notice = "Design declaration selected".into();
        Ok(())
    }

    fn declaration_row_target(&self, id: &str) -> Option<DeclarationRowTarget> {
        if let Some(code) = &self.code_project {
            let projection = code.declaration_panel_projection(self.editor());
            return managed_declaration_row_target(&projection.declarations, id);
        }
        self.editor()
            .workbench_projection()
            .outline
            .into_iter()
            .flat_map(|cell| cell.declarations)
            .find(|declaration| intent_panel_row_id(&declaration.symbol) == id)
            .map(|declaration| DeclarationRowTarget::Intent {
                node: declaration.node,
            })
    }

    fn select_declaration_row(&mut self, id: &str) -> Result<(), String> {
        let node = match self.declaration_row_target(id) {
            Some(DeclarationRowTarget::Intent { node }) => Some(node),
            Some(
                DeclarationRowTarget::Managed { node, .. }
                | DeclarationRowTarget::Generated { node, .. },
            ) => node,
            None => return Err("the declaration row is unavailable or stale".into()),
        }
        .ok_or_else(|| {
            "this source declaration has no independently selectable scene output".to_owned()
        })?;
        if !self.editor_mut().set_selected_declaration(Some(node)) {
            return Err("the declaration row no longer matches accepted scene authority".into());
        }
        self.notice = "Declaration selected".into();
        Ok(())
    }

    fn open_declaration_source(
        &mut self,
        id: &str,
        claimed_from: usize,
        claimed_to: usize,
    ) -> Result<(), String> {
        let (from, to) = match self.declaration_row_target(id) {
            Some(
                DeclarationRowTarget::Managed { from, to, .. }
                | DeclarationRowTarget::Generated { from, to, .. },
            ) => (from, to),
            Some(DeclarationRowTarget::Intent { .. }) => {
                return Err("ordinary design declarations have no writable sketch.ts owner".into());
            }
            None => return Err("the source navigation target is unavailable or stale".into()),
        };
        if claimed_from != from || claimed_to != to {
            return Err("the source navigation span belongs to stale declaration authority".into());
        }
        let code = self
            .code_project
            .as_mut()
            .ok_or_else(|| "the current project has no managed source".to_owned())?;
        if code.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before exact declaration navigation"
                    .into(),
            );
        }
        code.select_file("sketch.ts")?;
        self.notice = "Managed declaration source selected".into();
        Ok(())
    }

    fn move_declaration(&mut self, payload: &DeclarationMovePayload) -> Result<(), String> {
        let direction_route = payload.direction.is_some()
            && payload.target_id.is_none()
            && payload.position.is_none();
        let drop_route = payload.direction.is_none()
            && payload.target_id.is_some()
            && payload.position.is_some();
        if !direction_route && !drop_route {
            return Err(
                "declaration move requires either one direction or one target/position pair".into(),
            );
        }
        let source = match self.declaration_row_target(&payload.id) {
            Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) => {
                return Err(format!(
                    "Profile Offset helper declarations cannot move independently of `{}`",
                    root.0
                ));
            }
            Some(DeclarationRowTarget::Managed { symbol, .. }) => symbol,
            Some(DeclarationRowTarget::Generated { .. }) => {
                return Err("generated outputs cannot move independently of their owner".into());
            }
            Some(DeclarationRowTarget::Intent { .. }) => {
                return Err("ordinary design declaration order is read-only here".into());
            }
            None => return Err("the declaration move target is unavailable or stale".into()),
        };
        let code = self
            .code_project
            .as_ref()
            .ok_or_else(|| "the current project has no managed declaration order".to_owned())?;
        let order = code
            .declaration_panel_projection(self.editor())
            .declarations
            .into_iter()
            .map(|row| row.symbol.0)
            .collect::<Vec<_>>();
        let source_index = order
            .iter()
            .position(|symbol| symbol == &source)
            .ok_or_else(|| "the declaration move source is stale".to_owned())?;
        let before = if let Some(direction) = &payload.direction {
            match direction {
                DeclarationMoveDirection::Up => {
                    let previous = source_index
                        .checked_sub(1)
                        .ok_or_else(|| "the declaration is already first".to_owned())?;
                    Some(order[previous].clone())
                }
                DeclarationMoveDirection::Down => {
                    if source_index + 1 >= order.len() {
                        return Err("the declaration is already last".into());
                    }
                    order.get(source_index + 2).cloned()
                }
            }
        } else {
            let target_id = payload
                .target_id
                .as_deref()
                .expect("validated drop route has a target");
            let target = self.declaration_drop_target(target_id)?;
            if target == source {
                return Ok(());
            }
            let mut remaining = order
                .into_iter()
                .filter(|symbol| symbol != &source)
                .collect::<Vec<_>>();
            let target_index = remaining
                .iter()
                .position(|symbol| symbol == &target)
                .ok_or_else(|| "the declaration drop target is stale".to_owned())?;
            let insertion = match payload.position {
                Some(DeclarationMovePosition::Before) => target_index,
                Some(DeclarationMovePosition::After) => target_index + 1,
                None => unreachable!("validated drop route has a position"),
            };
            remaining.insert(insertion, source.clone());
            remaining.get(insertion + 1).cloned()
        };
        self.begin_structured_managed_mutation(
            "Reorder declaration",
            ManagedSketchMutation::ReorderDeclaration {
                declaration: source,
                before,
            },
        )
    }

    fn declaration_drop_target(&self, id: &str) -> Result<String, String> {
        match self.declaration_row_target(id) {
            Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) => Err(format!(
                "Profile Offset helper declarations cannot be independent move destinations; target `{}` instead",
                root.0
            )),
            Some(DeclarationRowTarget::Managed { symbol, .. }) => Ok(symbol),
            _ => Err("the declaration drop target is unavailable or stale".into()),
        }
    }

    fn set_declaration_suppressed(&mut self, id: &str, suppressed: bool) -> Result<(), String> {
        if self.code_project.is_some() {
            if let Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) = self.declaration_row_target(id)
            {
                return Err(format!(
                    "Profile Offset helper declarations cannot be suppressed independently of `{}`",
                    root.0
                ));
            }
            let target = self.managed_row_target(id)?;
            return self.begin_structured_managed_mutation(
                if suppressed {
                    "Suppress declaration"
                } else {
                    "Restore declaration"
                },
                ManagedSketchMutation::SetSuppressed { target, suppressed },
            );
        }
        match self.declaration_row_target(id) {
            Some(DeclarationRowTarget::Managed { .. } | DeclarationRowTarget::Generated { .. }) => {
                Err("managed declaration suppression requires code-project authority".into())
            }
            Some(DeclarationRowTarget::Intent { .. }) => {
                Err("ordinary design declarations are read-only in this source-owned panel".into())
            }
            None => Err("the declaration suppression target is unavailable or stale".into()),
        }
    }

    fn delete_declaration_row(&mut self, id: &str) -> Result<(), String> {
        if self.code_project.is_some() {
            if let Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) = self.declaration_row_target(id)
            {
                return Err(format!(
                    "Profile Offset helper declarations cannot be deleted independently of `{}`",
                    root.0
                ));
            }
            let target = self.managed_row_target(id)?;
            return self.begin_structured_managed_mutation(
                "Delete declaration",
                ManagedSketchMutation::Delete { target },
            );
        }
        match self.declaration_row_target(id) {
            Some(DeclarationRowTarget::Managed { .. } | DeclarationRowTarget::Generated { .. }) => {
                Err("managed declaration deletion requires code-project authority".into())
            }
            Some(DeclarationRowTarget::Intent { .. }) => {
                Err("ordinary design declarations are read-only in this panel".into())
            }
            None => Err("the declaration deletion target is unavailable or stale".into()),
        }
    }

    fn edit_parameter(&mut self, id: &str, value: serde_json::Value) -> Result<(), String> {
        self.cancel_active_interaction(None)?;
        let submission = {
            let code = self
                .code_project
                .as_ref()
                .ok_or_else(|| "the current project has no managed parameters".to_owned())?;
            let manifest = code.managed_controls_cached()?;
            let control = manifest
                .controls
                .iter()
                .find(|control| control.id.0 == id)
                .ok_or_else(|| "managed parameter is unavailable".to_owned())?;
            match (&control.value, value) {
                (
                    ManagedValue::Number(_) | ManagedValue::Unit(_),
                    serde_json::Value::Number(value),
                ) => ManagedControlSubmission::Number(
                    value
                        .as_f64()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| "managed parameter number must be finite".to_owned())?,
                ),
                (
                    ManagedValue::Number(_) | ManagedValue::Unit(_),
                    serde_json::Value::String(value),
                ) => ManagedControlSubmission::Number(
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| "managed parameter number must be finite".to_owned())?,
                ),
                (ManagedValue::Bool(_), serde_json::Value::Bool(value)) => {
                    ManagedControlSubmission::Boolean(value)
                }
                (ManagedValue::Bool(_), serde_json::Value::String(value)) => {
                    ManagedControlSubmission::Boolean(match value.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => return Err("managed Boolean parameter must be true or false".into()),
                    })
                }
                (ManagedValue::String(_), serde_json::Value::String(value)) => {
                    ManagedControlSubmission::String(value)
                }
                _ => return Err("managed parameter value has the wrong type".into()),
            }
        };
        let mutation = self
            .code_project
            .as_ref()
            .expect("managed parameter authority was checked")
            .managed_control_source_mutation(id, submission)?;
        if let Some(mutation) = mutation {
            return self
                .begin_selected_structured_managed_mutation("Edit managed parameter", mutation);
        }
        self.notice = "Managed parameter is unchanged".into();
        Ok(())
    }

    fn select_tool(&mut self, id: &str) -> Result<(), String> {
        if id.is_empty() || id.len() > MAX_COMMAND_BYTES {
            return Err("tool identity is empty or too large".into());
        }
        // Compatibility aliases from the retained toolbar remain accepted,
        // but presentation/context actions do not become authoring tools.
        match id {
            "construction" | "geometry-role" => return self.toggle_geometry_role(),
            "construction-display" => {
                self.toggle_grid();
                return Ok(());
            }
            "zoom-fit" => return self.fit_canvas(),
            "zoom-origin" => return self.center_canvas_origin(),
            _ => {}
        }
        self.cancel_active_interaction(None)?;
        self.authoring.deactivate();
        self.feature_authoring.deactivate();
        self.editor_mut().clear_authoring_previews();

        if id == "select" {
            let effects = self
                .editor_mut()
                .editor_mut()
                .activate_tool(EditorTool::Select);
            self.dispatch_construction(effects);
            self.active_tool = id.into();
            self.notice = "Select active".into();
            return Ok(());
        }

        if id == "fillet" {
            self.activate_fillet()?;
            self.active_tool = id.into();
            return Ok(());
        }

        if id == "offset" {
            self.activate_offset()?;
            self.active_tool = id.into();
            return Ok(());
        }

        if let Some(tool) = authoring_tool(id) {
            return self.activate_authoring_tool(id, tool);
        }

        let variant = geometry_variant(id)
            .ok_or_else(|| format!("tool `{id}` is not implemented by the Rust editor"))?;
        let effects = self
            .editor_mut()
            .editor_mut()
            .activate_geometry_tool(variant);
        self.dispatch_construction(effects);
        self.active_tool = id.into();
        self.notice = format!("{} active", super::geometry_palette::variant_label(variant));
        Ok(())
    }

    fn toggle_geometry_role(&mut self) -> Result<(), String> {
        let outcome = self.editor_mut().toggle_selected_geometry_role();
        match outcome {
            Ok(_) => {
                if self.publish_generic_code_checkpoint("Toggle construction geometry")? {
                    self.bump_revision();
                    self.notice = "Selected geometry role toggled".into();
                }
                self.retained_scene = None;
            }
            Err(
                geosolve_constraint_editor::ProjectionalEditorError::MissingGeometryRoleSelection,
            ) => {
                let role = match self.editor().editor().authoring_geometry_role() {
                    GeometryRole::Profile => GeometryRole::Construction,
                    GeometryRole::Construction => GeometryRole::Profile,
                };
                self.editor_mut()
                    .editor_mut()
                    .set_authoring_geometry_role(role);
                self.notice = format!(
                    "New curve authoring role set to {}",
                    if role == GeometryRole::Construction {
                        "Construction"
                    } else {
                        "Profile"
                    },
                );
            }
            Err(error) => self.last_error = Some(error.to_string()),
        }
        Ok(())
    }

    /// Changes only the role assigned to geometry created after this point.
    ///
    /// The legacy `geometry.role.toggle` command retains its selection-aware
    /// compatibility semantics. Presentation toolkits use this narrower route
    /// so a canvas setting never silently changes durable selected geometry.
    fn toggle_authoring_geometry_role(&mut self) {
        let role = match self.editor().editor().authoring_geometry_role() {
            GeometryRole::Profile => GeometryRole::Construction,
            GeometryRole::Construction => GeometryRole::Profile,
        };
        self.editor_mut()
            .editor_mut()
            .set_authoring_geometry_role(role);
        self.notice = format!(
            "New curve authoring role set to {}",
            if role == GeometryRole::Construction {
                "Construction"
            } else {
                "Profile"
            },
        );
    }

    fn set_explorer_row_visible(&mut self, id: &str, visible: bool) -> Result<(), String> {
        let rows = self.base_explorer_snapshot();
        if find_explorer_row(&rows, id).is_none() {
            return Err("the Explorer visibility target is unavailable or stale".into());
        }
        self.cancel_active_gesture(None)?;
        let changed = if visible {
            self.explorer_visibility.hidden_rows.remove(id)
        } else {
            if self.explorer_visibility.hidden_rows.len() >= MAX_VISIBILITY_ROWS
                && !self.explorer_visibility.hidden_rows.contains(id)
            {
                return Err(format!(
                    "Explorer visibility exceeds the {MAX_VISIBILITY_ROWS}-row limit"
                ));
            }
            self.explorer_visibility.hidden_rows.insert(id.to_owned())
        };
        if changed {
            self.explorer_visibility.isolate_restore = None;
            self.invalidate_visibility_presentation();
            self.notice = if visible {
                "Explorer item shown"
            } else {
                "Explorer item hidden"
            }
            .into();
        }
        Ok(())
    }

    fn isolate_explorer_group(&mut self, id: &str) -> Result<(), String> {
        let rows = self.base_explorer_snapshot();
        if !rows
            .iter()
            .any(|row| row.id == id && row.row_kind == ExplorerRowKind::Group)
        {
            return Err("only a current top-level Explorer group can be isolated".into());
        }
        self.cancel_active_gesture(None)?;
        let baseline = self
            .explorer_visibility
            .isolate_restore
            .clone()
            .unwrap_or_else(|| self.explorer_visibility.hidden_rows.clone());
        let mut isolated = baseline.clone();
        for group in &rows {
            if group.id == id {
                isolated.remove(&group.id);
            } else {
                isolated.insert(group.id.clone());
            }
        }
        if isolated.len() > MAX_VISIBILITY_ROWS {
            return Err(format!(
                "Explorer visibility exceeds the {MAX_VISIBILITY_ROWS}-row limit"
            ));
        }
        self.explorer_visibility.hidden_rows = isolated;
        self.explorer_visibility.isolate_restore = Some(baseline);
        self.invalidate_visibility_presentation();
        self.notice = "Explorer group isolated; previous visibility can be restored".into();
        Ok(())
    }

    fn restore_explorer_visibility(&mut self) -> Result<(), String> {
        let restore = self
            .explorer_visibility
            .isolate_restore
            .clone()
            .ok_or_else(|| {
                "there is no isolated Explorer visibility state to restore".to_owned()
            })?;
        self.cancel_active_gesture(None)?;
        self.explorer_visibility.isolate_restore = None;
        self.explorer_visibility.hidden_rows = restore;
        self.invalidate_visibility_presentation();
        self.notice = "Explorer visibility restored".into();
        Ok(())
    }

    fn toggle_construction_visibility(&mut self) -> Result<(), String> {
        self.cancel_active_gesture(None)?;
        let current = self
            .editor()
            .editor()
            .geometry_interaction_policy()
            .visibility;
        let show = !(current.explicit_construction && current.implicit_construction);
        let effects = self
            .editor_mut()
            .editor_mut()
            .set_geometry_visibility(GeometryVisibility {
                explicit_construction: show,
                implicit_construction: show,
                ..current
            });
        self.dispatch_construction(effects);
        self.invalidate_visibility_presentation();
        self.notice = if show {
            "Construction geometry shown"
        } else {
            "Construction geometry hidden"
        }
        .into();
        Ok(())
    }

    fn reset_explorer_visibility(&mut self) {
        self.explorer_visibility = ExplorerVisibilityState::default();
        let current = self
            .editor()
            .editor()
            .geometry_interaction_policy()
            .visibility;
        let _ = self
            .editor_mut()
            .editor_mut()
            .set_geometry_visibility(GeometryVisibility {
                explicit_construction: true,
                implicit_construction: true,
                ..current
            });
        self.invalidate_visibility_presentation();
    }

    fn invalidate_visibility_presentation(&mut self) {
        self.retained_scene = None;
        self.accepted_frame = None;
        self.preserve_frame_once = false;
    }

    fn toggle_grid(&mut self) {
        self.grid_visible = !self.grid_visible;
        self.notice = if self.grid_visible {
            "Canvas grid shown"
        } else {
            "Canvas grid hidden"
        }
        .into();
    }

    fn fit_canvas(&mut self) -> Result<(), String> {
        self.cancel_active_gesture(None)?;
        let _ = fit_projectional_camera_to_authority(&mut self.camera, &self.authority);
        self.retained_scene = None;
        self.notice = "Canvas fitted to accepted geometry".into();
        Ok(())
    }

    fn center_canvas_origin(&mut self) -> Result<(), String> {
        self.cancel_active_gesture(None)?;
        if self.camera.center_origin() {
            self.retained_scene = None;
        }
        self.notice = "Canvas centered on the origin".into();
        Ok(())
    }

    fn activate_authoring_tool(
        &mut self,
        id: &str,
        tool: geosolve_constraint_editor::AuthoringTool,
    ) -> Result<(), String> {
        let effects = self
            .editor_mut()
            .editor_mut()
            .activate_tool(EditorTool::Select);
        self.dispatch_construction(effects);
        let document = self
            .editor()
            .coordinator()
            .presentation_session()
            .ok_or_else(|| "relation authoring has no accepted document".to_owned())?
            .design_document()
            .clone();
        let selection = self
            .editor()
            .editor()
            .selection()
            .iter()
            .copied()
            .map(|item| AuthoringOperand::picked(item, self.curve_parameter(item)))
            .collect::<Vec<_>>();
        let outcome = self.authoring.activate(&document, tool, &selection);
        self.handle_authoring_outcome(outcome);
        // A complete preselection is a one-shot relation/dimension command:
        // it applies (or rejects) immediately and never enters a collector.
        // Keep the chrome aligned with that retained state instead of
        // painting an authoring tool that has nothing left to receive input.
        self.active_tool = if self.authoring.active_tool().is_some() {
            id.into()
        } else {
            "select".into()
        };
        Ok(())
    }

    fn finish_active_tool(&mut self) -> Result<(), String> {
        if self.offset_authoring.is_active() {
            return self.finish_offset();
        }
        if self.feature_authoring.active_tool().is_some() {
            let symbol = self.feature_symbol()?;
            let outcome = {
                let Self {
                    authority,
                    feature_authoring,
                    ..
                } = self;
                authority
                    .projectional_mut()
                    .expect("bridge owns projectional authority")
                    .apply_computed_fillet_preview(feature_authoring, symbol)
            };
            match outcome {
                Ok(_) => {
                    let published = self.publish_generic_code_checkpoint("Add Fillet")?;
                    self.retained_scene = None;
                    if published {
                        self.bump_revision();
                        self.notice = "Fillet accepted".into();
                    }
                }
                Err(error) => self.last_error = Some(error.to_string()),
            }
            return Ok(());
        }
        let effects = self.current_scene().map_or_else(Vec::new, |scene| {
            self.editor_mut()
                .editor_mut()
                .complete_draft(scene.design_identity)
        });
        self.dispatch_construction(effects);
        Ok(())
    }

    /// Whether the ordinary Finish route would publish the exact candidate
    /// currently retained and painted by its owning authoring state.
    fn can_finish_active_tool(&self) -> bool {
        if self.offset_authoring.is_active() {
            return self
                .editor()
                .offset_authoring_preview_matches(&self.offset_authoring);
        }
        if self.feature_authoring.active_tool().is_some() {
            return self
                .editor()
                .feature_authoring_preview_matches(&self.feature_authoring);
        }
        self.editor().editor().can_complete_draft()
    }

    fn can_undo(&self) -> bool {
        self.code_project.as_ref().map_or_else(
            || self.editor().coordinator().intent().undo_len() > 0,
            |code| !code.is_dirty() && code.can_undo(),
        )
    }

    fn can_redo(&self) -> bool {
        self.code_project.as_ref().map_or_else(
            || self.editor().coordinator().intent().redo_len() > 0,
            |code| !code.is_dirty() && code.can_redo(),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn pointer_down(&mut self, input: PointerInput) {
        let Some(scene) = self.current_scene() else {
            self.last_error = Some("pointer input has no accepted scene".into());
            return;
        };
        if self.feature_authoring.active_tool().is_some() {
            let result = self.feature_symbol().and_then(|symbol| {
                let Self {
                    authority,
                    feature_authoring,
                    ..
                } = self;
                authority
                    .projectional_mut()
                    .expect("bridge owns projectional authority")
                    .transact_feature_authoring_pick_at(
                        feature_authoring,
                        &scene,
                        input.position,
                        PickTolerance::default(),
                        symbol,
                    )
                    .map_err(|error| error.to_string())
            });
            match result {
                Ok(outcome) => self.handle_feature_outcome(outcome),
                Err(error) => self.last_error = Some(error),
            }
            return;
        }
        if self.offset_authoring.is_active() {
            let symbol = match self.offset_symbol() {
                Ok(symbol) => symbol,
                Err(error) => {
                    self.last_error = Some(error);
                    return;
                }
            };
            let authoring = self.offset_authoring.clone();
            match self
                .editor_mut()
                .pointer_down_offset_authoring_distance(&authoring, &scene, input, symbol)
            {
                Ok(Some(effects)) => {
                    self.dispatch_construction(effects);
                    self.captured_pointer = self
                        .editor()
                        .editor()
                        .active_pointer_gesture()
                        .filter(|gesture| gesture.kind == ActivePointerGestureKind::OffsetDistance)
                        .map(|gesture| gesture.pointer_id);
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    self.last_error = Some(error.to_string());
                    return;
                }
            }
            let policy = self.editor().editor().geometry_interaction_policy();
            let outcome = self.offset_authoring.pick_at(
                &scene,
                input.position,
                PickTolerance::default(),
                policy,
            );
            let rebuild = matches!(outcome, OffsetAuthoringOutcome::OperandChanged { .. });
            self.handle_offset_outcome(outcome);
            if rebuild && let Err(error) = self.refresh_offset_preview() {
                self.last_error = Some(error);
            }
            return;
        }
        if self.authoring.active_tool().is_some() {
            let Some(document) = self
                .editor()
                .coordinator()
                .presentation_session()
                .map(|session| session.design_document().clone())
            else {
                self.last_error = Some("authoring has no accepted document".into());
                return;
            };
            let outcome = self.authoring.pick_at_with_policy(
                &document,
                &scene,
                input.position,
                PickTolerance::default(),
                self.editor().editor().geometry_interaction_policy(),
            );
            self.handle_authoring_outcome(outcome);
            return;
        }
        if self.editor().editor().tool() != EditorTool::Select {
            let authoring = super::effect_adapter::draft_authoring_input(input.modifiers, None);
            let effects = self
                .editor_mut()
                .editor_mut()
                .pointer_down_with_draft_authoring(&scene, input, authoring);
            self.dispatch_construction(effects);
            return;
        }

        let preferred = self
            .code_project
            .as_ref()
            .map(|code| code.selected_managed_declaration(self.editor()));
        let mut effects = match self.editor_mut().pointer_down(&scene, input) {
            Ok(effects) => effects,
            Err(error) => {
                self.last_error = Some(error.to_string());
                return;
            }
        };
        if let Some(route) = self.editor().editor().prepared_point_drag_route() {
            let preferred = match preferred {
                Some(Ok(value)) => value,
                Some(Err(error)) => {
                    self.editor_mut().cancel_interaction();
                    self.last_error = Some(error);
                    return;
                }
                None => None,
            };
            let preparation = match self.code_project.as_mut() {
                Some(code) => code.prepare_semantic_point_drag(
                    self.authority
                        .projectional_ref()
                        .expect("bridge owns projectional authority"),
                    input.pointer_id,
                    route.point,
                    preferred.as_ref(),
                ),
                None => Ok(None),
            };
            match preparation {
                Ok(Some(prepared)) => {
                    self.editor_mut().cancel_interaction();
                    match WorkbenchDocumentAuthority::from_projectional_editor(*prepared.editor) {
                        Ok(authority) => {
                            self.authority = authority;
                            if let Some(detached_scene) = self.current_scene() {
                                match self.editor_mut().pointer_down_exact_point(
                                    &detached_scene,
                                    input,
                                    prepared.point,
                                ) {
                                    Ok(routed) => effects = routed,
                                    Err(error) => self.last_error = Some(error.to_string()),
                                }
                            }
                        }
                        Err(error) => self.last_error = Some(error),
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    self.editor_mut().cancel_interaction();
                    self.last_error = Some(error);
                    return;
                }
            }
        }
        let _ = effects;
        let active = self.editor().editor().active_pointer_gesture();
        let delegated_code_fillet = if active
            .is_some_and(|active| active.kind == ActivePointerGestureKind::FilletRadius)
            && let Some(code) = self.code_project.as_ref()
        {
            match super::delegate_projectional_code_fillet_radius_drag(&mut self.authority, code) {
                Ok(delegated) => delegated,
                Err(error) => {
                    self.editor_mut().cancel_interaction();
                    self.last_error = Some(error);
                    return;
                }
            }
        } else {
            false
        };
        if active.is_some_and(|active| {
            matches!(
                active.kind,
                ActivePointerGestureKind::CurveControl
                    | ActivePointerGestureKind::FilletRadius
                    | ActivePointerGestureKind::OffsetDistance
            )
        }) && self.code_project.is_some()
            && !delegated_code_fillet
            && let Err(error) =
                CodeProjectWorkbench::selected_code_geometry_mutation_permission(self.editor())
        {
            self.editor_mut().cancel_interaction();
            self.last_error = Some(error);
            return;
        }
        self.captured_pointer = active.map(|active| active.pointer_id);
    }

    fn route_canvas_pan(&mut self, request: PointerRequest, input: PointerInput) -> bool {
        if matches!(request.phase, PointerPhase::Down)
            && request.buttons & MIDDLE_POINTER_BUTTON != 0
        {
            let semantic_pointer_active = self.captured_pointer.is_some()
                || self.editor().editor().active_pointer_gesture().is_some()
                || self.editor().feature_authoring_radius_drag_active()
                || self.editor().offset_authoring_distance_drag_active();
            if semantic_pointer_active || self.canvas_pan.is_some() {
                return true;
            }
            let effects = self.editor_mut().editor_mut().invalidate_draft_inference();
            self.dispatch_construction(effects);
            self.canvas_pan = Some(CanvasPanGesture {
                pointer_id: input.pointer_id,
                origin: input.position,
                origin_center: self.camera.model_center(),
            });
            self.notice = "Canvas pan active".into();
            return true;
        }

        let Some(gesture) = self
            .canvas_pan
            .filter(|gesture| gesture.pointer_id == input.pointer_id)
        else {
            return false;
        };
        if !matches!(request.phase, PointerPhase::Move | PointerPhase::Up) {
            return false;
        }

        if self
            .camera
            .pan_from(gesture.origin_center, gesture.origin, input.position)
        {
            self.retained_scene = None;
            self.notice = "Canvas panned".into();
        }
        if matches!(request.phase, PointerPhase::Up) {
            self.canvas_pan = None;
        }
        true
    }

    fn pointer_move(&mut self, input: PointerInput) {
        let Some(scene) = self.current_scene() else {
            return;
        };
        if self.feature_authoring.active_tool().is_some() {
            let outcome = {
                let Self {
                    authority,
                    feature_authoring,
                    ..
                } = self;
                authority
                    .projectional_mut()
                    .expect("bridge owns projectional authority")
                    .pointer_move_feature_authoring(
                        feature_authoring,
                        &scene,
                        input,
                        PickTolerance::default(),
                    )
            };
            match outcome {
                Ok(_) => {}
                Err(error) => self.last_error = Some(error.to_string()),
            }
            return;
        }
        if self.offset_authoring.is_active() {
            if self.editor().offset_authoring_distance_drag_active() {
                let result = {
                    let Self {
                        authority,
                        offset_authoring,
                        ..
                    } = self;
                    authority
                        .projectional_mut()
                        .expect("bridge owns projectional authority")
                        .pointer_move_offset_authoring_distance_audited(
                            offset_authoring,
                            &scene,
                            input,
                        )
                        .outcome
                };
                if let Err(error) = result {
                    self.last_error = Some(error.to_string());
                }
            } else {
                let policy = self.editor().editor().geometry_interaction_policy();
                let outcome = self.offset_authoring.hover_at(
                    &scene,
                    input.position,
                    PickTolerance::default(),
                    policy,
                );
                self.handle_offset_outcome(outcome);
            }
            return;
        }
        if self.authoring.active_tool().is_some() {
            let authoring = self.authoring.clone();
            let _ = self.editor_mut().pointer_move_authoring(
                &authoring,
                &scene,
                input,
                PickTolerance::default(),
            );
            return;
        }
        if self.editor().editor().tool() != EditorTool::Select {
            let authoring = super::effect_adapter::draft_authoring_input(input.modifiers, None);
            let effects = self
                .editor_mut()
                .editor_mut()
                .pointer_move_with_draft_authoring(&scene, input, authoring);
            self.dispatch_construction(effects);
            return;
        }
        if let Err(error) = self.editor_mut().pointer_move(&scene, input) {
            self.last_error = Some(error.to_string());
        }
    }

    // This keeps all terminal pointer routes visibly exhaustive: ordinary,
    // delegated point, delegated Fillet, and generic source checkpoints must
    // not acquire subtly different capture cleanup or publication behavior.
    #[allow(clippy::too_many_lines)]
    fn pointer_up(&mut self, input: PointerInput) {
        if self.offset_authoring.is_active() {
            self.pointer_up_offset(input);
            return;
        }
        if self.authoring.active_tool().is_some()
            || self.feature_authoring.active_tool().is_some()
            || self.editor().editor().tool() != EditorTool::Select
        {
            self.captured_pointer = None;
            return;
        }
        let Some(scene) = self.current_scene() else {
            self.last_error = Some("terminal pointer has no accepted scene".into());
            return;
        };
        let delegated_point = self
            .code_project
            .as_ref()
            .is_some_and(CodeProjectWorkbench::has_any_pending_semantic_point_drag);
        if delegated_point {
            if self
                .code_project
                .as_ref()
                .is_some_and(|code| !code.has_pending_semantic_point_drag(input.pointer_id))
            {
                self.last_error = Some(self.rejected_delegated_point_terminal_error(
                    "terminal pointer does not own the pending semantic point gesture".into(),
                ));
                self.captured_pointer = None;
                self.retained_scene = None;
                return;
            }
            match self.editor_mut().pointer_up_delegated_point(&scene, input) {
                Ok(outcome) => {
                    if let Some(proposal) = outcome.proposal {
                        if let Err(error) =
                            self.publish_delegated_point_terminal(input.pointer_id, &proposal)
                        {
                            self.last_error = Some(error);
                        }
                    } else if let Err(error) = self.cancel_code_drag(Some(input.pointer_id)) {
                        self.last_error = Some(self.rejected_delegated_point_terminal_error(error));
                    }
                }
                Err(error) => {
                    self.last_error =
                        Some(self.rejected_delegated_point_terminal_error(error.to_string()));
                }
            }
            self.captured_pointer = None;
            self.retained_scene = None;
            return;
        }
        match self.editor_mut().pointer_up(&scene, input) {
            Ok(mut outcome) => {
                if let Some(proposal) = outcome.delegated_computed_fillet_radius.take() {
                    match self.begin_delegated_fillet_managed_mutation(&proposal) {
                        Ok(()) => {}
                        Err(error) => self.last_error = Some(error),
                    }
                } else if outcome.transaction.is_some() {
                    match self.publish_generic_code_checkpoint("Direct GUI sketch edit") {
                        Ok(true) => {
                            self.bump_revision();
                            self.notice = "Direct movement accepted".into();
                        }
                        Ok(false) => {}
                        Err(error) => self.last_error = Some(error),
                    }
                } else {
                    self.notice = "Canvas selection updated".into();
                }
            }
            Err(error) => self.last_error = Some(error.to_string()),
        }
        self.captured_pointer = None;
        self.retained_scene = None;
    }

    fn pointer_up_offset(&mut self, input: PointerInput) {
        if self.editor().offset_authoring_distance_drag_active() {
            let Some(scene) = self.current_scene() else {
                self.last_error = Some("terminal Offset pointer has no accepted scene".into());
                return;
            };
            let outcome = {
                let Self {
                    authority,
                    offset_authoring,
                    ..
                } = self;
                authority
                    .projectional_mut()
                    .expect("bridge owns projectional authority")
                    .pointer_up_offset_authoring_distance(offset_authoring, &scene, input)
            };
            match outcome {
                Ok(true) => self.notice = "Offset distance preview updated".into(),
                Ok(false) => self.notice = "Offset distance preview unchanged".into(),
                Err(error) => self.last_error = Some(error.to_string()),
            }
            self.retained_scene = None;
        }
        self.captured_pointer = None;
    }

    fn handle_authoring_outcome(&mut self, outcome: AuthoringOutcome) {
        match outcome {
            AuthoringOutcome::Apply(application) => {
                let dispatch = {
                    let Self {
                        authority,
                        authoring,
                        ..
                    } = self;
                    dispatch_projectional_authoring_application(
                        authority
                            .projectional_mut()
                            .expect("bridge owns projectional authority"),
                        authoring,
                        &application,
                    )
                };
                if dispatch.committed() {
                    match self.publish_generic_code_checkpoint("Add relation") {
                        Ok(true) => {
                            self.bump_revision();
                            self.retained_scene = None;
                            self.notice = "Relation or dimension accepted".into();
                        }
                        Ok(false) => {}
                        Err(error) => self.last_error = Some(error),
                    }
                } else if let Some(error) = dispatch.error {
                    self.last_error = Some(error);
                }
            }
            AuthoringOutcome::Warning(warning) => self.last_error = Some(warning.message),
            AuthoringOutcome::ModeEntered { .. } => {
                self.notice = "Choose the first operand".into();
            }
            AuthoringOutcome::Collecting { operands, .. } => {
                self.notice = format!("{} operand(s) selected", operands.len());
            }
            AuthoringOutcome::PendingCleared { .. } => {
                self.notice = "Pending operands cleared".into();
            }
            AuthoringOutcome::ModeExited => self.notice = "Authoring exited".into(),
            AuthoringOutcome::Inactive => {}
        }
    }

    fn activate_fillet(&mut self) -> Result<(), String> {
        let effects = self
            .editor_mut()
            .editor_mut()
            .activate_tool(EditorTool::Select);
        self.dispatch_construction(effects);
        let selection = self
            .editor()
            .editor()
            .selection()
            .iter()
            .copied()
            .map(|item| (item, self.curve_parameter(item)))
            .collect::<Vec<_>>();
        let symbol = self.feature_symbol()?;
        let outcome = {
            let Self {
                authority,
                feature_authoring,
                ..
            } = self;
            authority
                .projectional_mut()
                .expect("bridge owns projectional authority")
                .activate_feature_authoring(
                    feature_authoring,
                    FeatureAuthoringTool::Fillet,
                    FeatureAuthoringOptions::default(),
                    &selection,
                    symbol,
                )
        }
        .map_err(|error| error.to_string())?;
        self.handle_feature_outcome(outcome);
        Ok(())
    }

    fn activate_offset(&mut self) -> Result<(), String> {
        self.authoring.deactivate();
        self.feature_authoring.deactivate();
        let effects = self
            .editor_mut()
            .editor_mut()
            .activate_tool(EditorTool::Select);
        self.dispatch_construction(effects);
        let outcome = {
            let Self {
                authority,
                offset_authoring,
                ..
            } = self;
            authority
                .projectional_mut()
                .expect("bridge owns projectional authority")
                .activate_offset_authoring(offset_authoring)
        }
        .map_err(|error| error.to_string())?;
        self.handle_offset_outcome(outcome);
        Ok(())
    }

    fn refresh_offset_preview(&mut self) -> Result<(), String> {
        let symbol = self.offset_symbol()?;
        let changed = {
            let Self {
                authority,
                offset_authoring,
                ..
            } = self;
            authority
                .projectional_mut()
                .expect("bridge owns projectional authority")
                .refresh_offset_authoring_preview(offset_authoring, symbol)
        }
        .map_err(|error| error.to_string())?;
        if changed {
            self.notice = format!(
                "{} · Preview ready",
                self.offset_authoring.guidance().message
            );
            self.retained_scene = None;
        }
        Ok(())
    }

    fn finish_offset(&mut self) -> Result<(), String> {
        let requested = self.offset_authoring.apply();
        let ready = matches!(requested, OffsetAuthoringOutcome::ApplyRequested(_));
        self.handle_offset_outcome(requested);
        if !ready {
            return Ok(());
        }
        let symbol = self.offset_symbol()?;
        let outcome = {
            let Self {
                authority,
                offset_authoring,
                ..
            } = self;
            authority
                .projectional_mut()
                .expect("bridge owns projectional authority")
                .apply_profile_offset_preview(offset_authoring, symbol)
        }
        .map_err(|error| error.to_string())?;
        if outcome.disposition != geosolve_sketch_intent::IntentPlanDisposition::Accepted {
            return Err("Offset publication returned a non-accepted transaction".into());
        }
        let published = self.publish_generic_code_checkpoint("Add Offset")?;
        let _ = self.offset_authoring.cancel();
        self.active_tool = "select".into();
        self.retained_scene = None;
        if published {
            self.bump_revision();
            self.notice = "Offset accepted; Select active".into();
        }
        Ok(())
    }

    fn handle_offset_outcome(&mut self, outcome: OffsetAuthoringOutcome) {
        match outcome {
            OffsetAuthoringOutcome::ModeEntered(guidance)
            | OffsetAuthoringOutcome::OperandChanged { guidance, .. }
            | OffsetAuthoringOutcome::DistanceChanged { guidance, .. } => {
                self.notice = guidance.message.into();
            }
            OffsetAuthoringOutcome::ApplyRequested(_) => {
                self.notice = "Offset candidate ready to apply".into();
            }
            OffsetAuthoringOutcome::Warning(warning) => {
                self.last_error = Some(warning.message.clone());
                self.notice = warning.message;
            }
            OffsetAuthoringOutcome::ModeExited => {
                self.notice = "Offset canceled; Select active".into();
            }
            OffsetAuthoringOutcome::HoverChanged(_) | OffsetAuthoringOutcome::Inactive => {}
        }
        self.retained_scene = None;
    }

    fn handle_feature_outcome(&mut self, outcome: FeatureAuthoringOutcome) {
        self.notice = match outcome {
            FeatureAuthoringOutcome::ModeEntered(guidance)
            | FeatureAuthoringOutcome::NoNativeHit(guidance)
            | FeatureAuthoringOutcome::CandidateCleared(guidance)
            | FeatureAuthoringOutcome::Collecting { guidance, .. }
            | FeatureAuthoringOutcome::PreviewRequested { guidance, .. } => guidance.message.into(),
            FeatureAuthoringOutcome::Apply(_) => "Fillet ready to apply".into(),
            FeatureAuthoringOutcome::Warning(warning) => {
                self.last_error = Some(warning.message.clone());
                warning.message
            }
            FeatureAuthoringOutcome::ModeExited => "Fillet authoring exited".into(),
            FeatureAuthoringOutcome::Inactive => self.notice.clone(),
        };
        self.retained_scene = None;
    }

    fn feature_symbol(&self) -> Result<IntentKey, String> {
        IntentKey::new(format!(
            "Fillet {}",
            self.editor()
                .coordinator()
                .intent()
                .identity()
                .revision
                .raw()
                .saturating_add(1)
        ))
        .map_err(|error| error.to_string())
    }

    fn offset_symbol(&self) -> Result<IntentKey, String> {
        IntentKey::new(format!(
            "Offset {}",
            self.editor()
                .coordinator()
                .intent()
                .identity()
                .revision
                .raw()
                .saturating_add(1)
        ))
        .map_err(|error| error.to_string())
    }

    fn dispatch_construction(&mut self, effects: Vec<EditorEffect>) {
        let outcome = {
            let Self {
                authority,
                construction_preview,
                ..
            } = self;
            dispatch_projectional_construction_effects(
                authority
                    .projectional_mut()
                    .expect("bridge owns projectional authority"),
                construction_preview,
                effects,
            )
        };
        if outcome.accepted_terminal {
            match self.publish_generic_code_checkpoint("Add geometry") {
                Ok(true) => {
                    self.bump_revision();
                    self.retained_scene = None;
                    self.notice = "Geometry accepted".into();
                }
                Ok(false) => {}
                Err(error) => self.last_error = Some(error),
            }
        } else if outcome.rejected_terminal {
            self.last_error = outcome.error;
        }
    }

    /// Returns `true` when the already accepted ordinary editor transaction is
    /// complete, and `false` when a code project now awaits its asynchronous
    /// compiler receipt.
    fn publish_generic_code_checkpoint(&mut self, label: &str) -> Result<bool, String> {
        let Some(code) = self.code_project.as_ref() else {
            return Ok(true);
        };
        if self.pending_managed_mutation.is_some() {
            return Err("a managed-source mutation is already awaiting compilation".into());
        }
        let accepted_editor = code.restore_accepted_editor()?;
        let candidate_editor = self
            .authority
            .projectional_ref()
            .expect("bridge owns projectional authority");
        if !code.has_managed_authority() {
            return Err("the code project has no compiled managed authority".into());
        }
        let prepared = code
            .prepare_canvas_managed_mutation(candidate_editor)
            .map(|prepared| PendingManagedMutation::Managed {
                label: label.to_owned(),
                prepared: Box::new(prepared),
            });
        // The terminal GUI candidate remains only inside the prepared Rust
        // ticket. Restore the exact accepted code scene before returning a
        // snapshot so neither browser paint nor persistence can observe a
        // GUI-only hybrid while compilation is in flight.
        self.authority = WorkbenchDocumentAuthority::from_projectional_editor(*accepted_editor)?;
        self.retained_scene = None;
        self.pending_managed_mutation = Some(prepared?);
        self.notice = "Validating managed source candidate".into();
        Ok(false)
    }

    fn managed_row_target(&self, id: &str) -> Result<ManagedMutationTarget, String> {
        match self.declaration_row_target(id) {
            Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) => Err(format!(
                "Profile Offset helper declaration lifecycle is owned by `{}`",
                root.0
            )),
            Some(DeclarationRowTarget::Managed { symbol, .. }) => {
                Ok(ManagedMutationTarget::Declaration {
                    declaration: symbol,
                })
            }
            Some(DeclarationRowTarget::Generated { address, .. }) => {
                Ok(ManagedMutationTarget::Generated {
                    address: ExecutedGeneratedMemberAddress {
                        invocation: address.invocation,
                        template: address.template,
                        member_key: address.member_key,
                        output: address.output,
                    },
                })
            }
            Some(DeclarationRowTarget::Intent { .. }) => {
                Err("ordinary design declarations are not managed mutation targets".into())
            }
            None => Err("the managed declaration target is unavailable or stale".into()),
        }
    }

    fn begin_structured_managed_mutation(
        &mut self,
        label: &str,
        mutation: ManagedSketchMutation,
    ) -> Result<(), String> {
        self.begin_structured_managed_mutation_with_selection(label, mutation, false)
    }

    fn begin_selected_structured_managed_mutation(
        &mut self,
        label: &str,
        mutation: ManagedSketchMutation,
    ) -> Result<(), String> {
        self.begin_structured_managed_mutation_with_selection(label, mutation, true)
    }

    fn begin_structured_managed_mutation_with_selection(
        &mut self,
        label: &str,
        mutation: ManagedSketchMutation,
        preserve_selection: bool,
    ) -> Result<(), String> {
        if self.pending_managed_mutation.is_some() {
            return Err("a managed-source mutation is already awaiting compilation".into());
        }
        let code = self
            .code_project
            .as_ref()
            .ok_or_else(|| "the current project has no managed source authority".to_owned())?;
        if !code.has_managed_authority() {
            return Err("the code project has no compiled managed authority".into());
        }
        self.pending_managed_mutation = Some(PendingManagedMutation::Managed {
            label: label.into(),
            prepared: Box::new(if preserve_selection {
                code.prepare_selected_structured_managed_mutation(self.editor(), mutation)?
            } else {
                code.prepare_structured_managed_mutation(mutation)?
            }),
        });
        self.notice = "Validating managed source candidate".into();
        Ok(())
    }

    fn publish_delegated_point_terminal(
        &mut self,
        pointer_id: u64,
        proposal: &geosolve_constraint_editor::DelegatedPointDragProposal,
    ) -> Result<(), String> {
        if let Err(error) = self.try_publish_delegated_point_terminal(pointer_id, proposal) {
            return Err(self.rejected_delegated_point_terminal_error(error));
        }
        Ok(())
    }

    fn try_publish_delegated_point_terminal(
        &mut self,
        pointer_id: u64,
        proposal: &geosolve_constraint_editor::DelegatedPointDragProposal,
    ) -> Result<(), String> {
        if self.pending_managed_mutation.is_some() {
            return Err("a managed-source mutation is already awaiting compilation".into());
        }
        let Self {
            authority,
            code_project,
            retained_scene,
            notice,
            ..
        } = self;
        let code = code_project.as_mut().ok_or_else(|| {
            "the delegated point release lost managed source authority".to_owned()
        })?;
        let origin = authority.projectional_ref().ok_or_else(|| {
            "the delegated point release requires projectional authority".to_owned()
        })?;
        let publication = code
            .publish_delegated_point_terminal(
                pointer_id,
                origin,
                proposal,
                "Direct GUI sketch edit",
            )?
            .ok_or_else(|| {
                "the moved delegated point terminal produced no outer publication".to_owned()
            })?;
        *authority = WorkbenchDocumentAuthority::from_projectional_editor(*publication.editor)?;
        *retained_scene = None;
        *notice = "Direct movement accepted".into();
        self.bump_revision();
        Ok(())
    }

    /// Rejects one exact delegated point terminal without retaining either
    /// its semantic route or its transient native editor. The original
    /// terminal error remains primary; cleanup failures are appended so a
    /// diagnostic can never disguise the transaction that actually failed.
    fn rejected_delegated_point_terminal_error(&mut self, error: String) -> String {
        match self.restore_after_rejected_delegated_point_terminal() {
            Ok(()) => error,
            Err(cleanup) => format!(
                "{error}; failed to restore accepted code authority after rejected point terminal: {cleanup}"
            ),
        }
    }

    fn restore_after_rejected_delegated_point_terminal(&mut self) -> Result<(), String> {
        let (restored, mut cleanup_errors) = {
            let code = self.code_project.as_mut().ok_or_else(|| {
                "the rejected delegated point terminal lost code-project authority".to_owned()
            })?;
            let mut cleanup_errors = Vec::new();
            let canceled_editor = if code.has_any_pending_semantic_point_drag() {
                match code.cancel_semantic_point_drag(None) {
                    Ok(editor) => editor,
                    Err(error) => {
                        cleanup_errors.push(format!("semantic route cleanup failed: {error}"));
                        None
                    }
                }
            } else {
                None
            };
            (
                canceled_editor.map_or_else(|| code.restore_accepted_editor(), Ok),
                cleanup_errors,
            )
        };
        let restored = match restored {
            Ok(restored) => restored,
            Err(error) => {
                cleanup_errors.push(format!("accepted editor restoration failed: {error}"));
                return Err(cleanup_errors.join("; "));
            }
        };
        match WorkbenchDocumentAuthority::from_projectional_editor(*restored) {
            Ok(authority) => self.authority = authority,
            Err(error) => cleanup_errors.push(format!(
                "accepted editor authority restoration failed: {error}"
            )),
        }
        self.captured_pointer = None;
        self.retained_scene = None;
        if cleanup_errors.is_empty() {
            Ok(())
        } else {
            Err(cleanup_errors.join("; "))
        }
    }

    fn begin_delegated_fillet_managed_mutation(
        &mut self,
        proposal: &geosolve_constraint_editor::DelegatedComputedFilletRadiusProposal,
    ) -> Result<(), String> {
        if self.pending_managed_mutation.is_some() {
            return Err("a managed-source mutation is already awaiting compilation".into());
        }
        let Self {
            authority,
            code_project,
            pending_managed_mutation,
            retained_scene,
            notice,
            ..
        } = self;
        let code = code_project.as_mut().ok_or_else(|| {
            "the delegated Fillet release lost managed source authority".to_owned()
        })?;
        let editor = authority.projectional_ref().ok_or_else(|| {
            "the delegated Fillet release requires projectional authority".to_owned()
        })?;
        let prepared = code.prepare_delegated_fillet_managed_mutation(editor, proposal)?;
        let accepted = code.restore_accepted_editor()?;
        *authority = WorkbenchDocumentAuthority::from_projectional_editor(*accepted)?;
        *retained_scene = None;
        *pending_managed_mutation = Some(PendingManagedMutation::Managed {
            label: "Edit Fillet radius".into(),
            prepared: Box::new(prepared),
        });
        *notice = "Validating managed source candidate".into();
        Ok(())
    }

    fn resolve_pending_managed_mutation(
        &mut self,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<(), String> {
        // Keep the sole prepared authority borrowed until every receipt and
        // cold-native gate succeeds. A malformed, method-confused or stale
        // response may therefore be diagnosed without publishing or
        // consuming the recoverable ticket.
        let pending = self
            .pending_managed_mutation
            .as_ref()
            .ok_or_else(|| "no prepared managed-source mutation is pending".to_owned())?;
        let code = self
            .code_project
            .as_ref()
            .ok_or_else(|| "the pending mutation lost its code-project authority".to_owned())?;
        let resolved = match pending {
            PendingManagedMutation::Managed { label, prepared } => code
                .resolve_canvas_managed_mutation(prepared, receipt)
                .map(|(candidate, publication)| {
                    (
                        candidate,
                        publication,
                        format!("{label} accepted in managed source"),
                    )
                }),
            PendingManagedMutation::Source { prepared } => {
                match code.resolve_managed_source_apply(prepared, receipt) {
                    Ok(ResolvedManagedSourceApply::Accepted {
                        candidate,
                        publication,
                    }) => Ok((*candidate, publication, "Managed source accepted".into())),
                    Ok(ResolvedManagedSourceApply::RetainedFailure { source, diagnostic }) => {
                        self.code_project
                            .as_mut()
                            .expect("code authority borrowed above")
                            .retain_managed_compiler_draft(
                                source,
                                diagnostic,
                                ManagedSpan { start: 0, end: 0 },
                            )?;
                        self.pending_managed_mutation = None;
                        self.last_error = None;
                        self.preserve_frame_once = true;
                        self.notice = "Managed source retained the prior accepted canvas".into();
                        return Ok(());
                    }
                    Err(error) => Err(error),
                }
            }
        };
        let (candidate, publication, notice) = match resolved {
            Ok(publication) => publication,
            Err(error) => {
                self.last_error = Some(error);
                self.notice = "Managed-source candidate retained the prior accepted state".into();
                return Ok(());
            }
        };
        let accepted_authority =
            WorkbenchDocumentAuthority::from_projectional_editor(*publication.editor)?;
        self.pending_managed_mutation = None;
        self.authority = accepted_authority;
        self.code_project = Some(candidate);
        self.retained_scene = None;
        self.last_error = None;
        self.preserve_frame_once = false;
        self.bump_revision();
        self.notice = notice;
        Ok(())
    }

    fn abort_pending_managed_mutation(
        &mut self,
        payload: &ManagedMutationAbortPayload,
    ) -> Result<(), String> {
        let pending = self
            .pending_managed_mutation
            .as_ref()
            .ok_or_else(|| "no prepared managed-source mutation is pending".to_owned())?;
        if pending_managed_ticket_digest(pending) != payload.ticket_digest {
            return Err("managed-source compiler failure belongs to a different ticket".into());
        }
        if let PendingManagedMutation::Source { prepared } = pending
            && payload.candidate_source != prepared.request.candidate_source
        {
            return Err(
                "managed-source compiler failure belongs to different candidate bytes".into(),
            );
        }
        let diagnostic = if payload.diagnostic.is_empty() {
            "managed-source compiler rejected the prepared candidate".into()
        } else {
            payload.diagnostic.clone()
        };
        self.code_project
            .as_mut()
            .ok_or_else(|| "the pending mutation lost its code-project authority".to_owned())?
            .retain_managed_compiler_draft(
                payload.candidate_source.clone(),
                diagnostic.clone(),
                payload.span,
            )?;
        self.pending_managed_mutation = None;
        self.last_error = None;
        self.notice = "Managed-source candidate retained as a correction draft".into();
        Ok(())
    }

    fn cancel_active_interaction(&mut self, pointer: Option<u64>) -> Result<(), String> {
        self.cancel_active_gesture(pointer)?;
        let _ = self.offset_authoring.cancel();
        Ok(())
    }

    /// Cancels only live pointer ownership while preserving the selected
    /// authoring mode. Camera commands and a first Escape during a drag use
    /// this path so the chrome cannot advertise a mode that was silently
    /// deactivated underneath it.
    fn cancel_active_gesture(&mut self, pointer: Option<u64>) -> Result<(), String> {
        // Provisional Fillet and Profile Offset drags retain their authoring
        // collector state outside `ConstraintEditor`. Use their projectional
        // cancellation routes so the exact pointer-down state is restored
        // before consuming the editor's presentation cleanup effects.
        let effects = if self.editor().feature_authoring_radius_drag_active() {
            let Self {
                authority,
                feature_authoring,
                ..
            } = self;
            authority
                .projectional_mut()
                .expect("bridge owns projectional authority")
                .cancel_feature_authoring_radius_drag(feature_authoring)
        } else if self.editor().offset_authoring_distance_drag_active() {
            let Self {
                authority,
                offset_authoring,
                ..
            } = self;
            authority
                .projectional_mut()
                .expect("bridge owns projectional authority")
                .cancel_offset_authoring_distance_drag(offset_authoring)
        } else {
            self.editor_mut().cancel_interaction()
        };
        self.dispatch_construction(effects);
        self.captured_pointer = None;
        self.canvas_pan = None;
        self.retained_scene = None;
        self.cancel_code_drag(pointer)
    }

    fn cancel_code_drag(&mut self, pointer: Option<u64>) -> Result<(), String> {
        if let Some(code) = self.code_project.as_mut()
            && let Some(editor) = code.cancel_semantic_point_drag(pointer)?
        {
            self.authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)?;
        }
        Ok(())
    }

    fn reset_transient_tools(&mut self) {
        self.retained_scene = None;
        self.construction_preview = None;
        self.authoring = AuthoringState::default();
        self.feature_authoring = FeatureAuthoringState::default();
        self.offset_authoring = OffsetAuthoringState::default();
        self.interaction_trace = super::interaction_trace::InteractionTrace::default();
        self.active_tool = "select".into();
        self.captured_pointer = None;
        self.canvas_pan = None;
        self.last_error = None;
    }

    fn normalized_pointer(&self, request: PointerRequest) -> Option<PointerInput> {
        let captured = !matches!(request.phase, PointerPhase::Down)
            && (self.captured_pointer == Some(request.pointer_id)
                || self
                    .canvas_pan
                    .is_some_and(|gesture| gesture.pointer_id == request.pointer_id));
        let position = self.normalize_client_point([request.x, request.y], captured)?;
        Some(PointerInput {
            pointer_id: request.pointer_id,
            position,
            modifiers: Modifiers {
                shift: request.modifiers.shift,
                control: request.modifiers.ctrl,
                command: request.modifiers.meta,
            },
        })
    }

    fn normalize_client_point(&self, client: [f64; 2], captured: bool) -> Option<ScreenPoint> {
        let rect = super::effect_adapter::ClientRect {
            left: 0.0,
            top: 0.0,
            width: self.host_size[0],
            height: self.host_size[1],
        };
        if captured {
            super::effect_adapter::normalize_captured_client_point(
                rect,
                self.camera.viewport().screen_size,
                client,
            )
        } else {
            super::effect_adapter::normalize_client_point(
                rect,
                self.camera.viewport().screen_size,
                client,
            )
        }
    }

    fn curve_parameter(&self, item: SelectionItem) -> Option<f64> {
        match item {
            SelectionItem::Curve(span) => self.editor().editor().curve_pick_parameter(span),
            _ => None,
        }
    }

    fn editor(&self) -> &geosolve_constraint_editor::ProjectionalEditorSession {
        self.authority
            .projectional_ref()
            .expect("WorkbenchBridge construction admits only projectional authority")
    }

    fn editor_mut(&mut self) -> &mut geosolve_constraint_editor::ProjectionalEditorSession {
        self.authority
            .projectional_mut()
            .expect("WorkbenchBridge construction admits only projectional authority")
    }

    fn current_scene(&mut self) -> Option<EditorScene> {
        if let Some(scene) = self.retained_scene.as_ref()
            && scene.viewport == self.camera.viewport()
            && self.authority.retained_scene_is_current(scene)
        {
            return Some(scene.clone());
        }
        let presentation = self.authority.scene_presentation(
            self.camera.viewport(),
            super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
        );
        if let Some(error) = presentation.status_override {
            self.last_error = Some(error);
        }
        let mut scene = presentation.scene;
        if let Some(candidate) = scene.as_mut() {
            let hidden = self.hidden_scene_items(candidate);
            if let Err(error) = candidate.hide_items(hidden) {
                self.last_error = Some(format!(
                    "Explorer visibility could not authenticate the accepted scene: {error}"
                ));
                scene = None;
            }
        }
        self.retained_scene = scene;
        self.retained_scene.clone()
    }

    fn hidden_scene_items(&self, scene: &EditorScene) -> Vec<SelectionItem> {
        let rows = self.explorer_snapshot();
        let mut hidden_rows = Vec::new();
        collect_effectively_hidden_leaves(&rows, &mut hidden_rows);
        let Some(materialization) = self.editor().coordinator().accepted_materialization() else {
            return Vec::new();
        };
        let mut nodes = std::collections::BTreeSet::new();
        for id in hidden_rows {
            let node = match self.declaration_row_target(&id) {
                Some(
                    DeclarationRowTarget::Intent { node }
                    | DeclarationRowTarget::Managed {
                        node: Some(node), ..
                    }
                    | DeclarationRowTarget::Generated {
                        node: Some(node), ..
                    },
                ) => Some(node),
                Some(
                    DeclarationRowTarget::Managed { node: None, .. }
                    | DeclarationRowTarget::Generated { node: None, .. },
                )
                | None => None,
            };
            if let Some(node) = node {
                nodes.insert(node);
            }
        }

        let mut hidden = std::collections::BTreeSet::new();
        for node in nodes {
            let Some(owner) = materialization.ownership.node(node) else {
                continue;
            };
            for binding in &owner.owned {
                match *binding {
                    geosolve_constraint_editor::IntentNativeBinding::Point(point) => {
                        hidden.insert(SelectionItem::Point(point));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Curve(curve) => {
                        hidden.extend(
                            scene
                                .curves
                                .iter()
                                .filter(|candidate| candidate.span.curve == curve)
                                .map(|candidate| SelectionItem::Curve(candidate.span)),
                        );
                    }
                    geosolve_constraint_editor::IntentNativeBinding::CurveSpan(span) => {
                        hidden.insert(SelectionItem::Curve(span));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Constraint(constraint) => {
                        hidden.insert(SelectionItem::Constraint(constraint));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Dimension(dimension) => {
                        hidden.insert(SelectionItem::Dimension(dimension));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::ComputedFeature(feature) => {
                        hidden.insert(SelectionItem::Feature(feature));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::ComputedFeatureCorner(
                        corner,
                    ) => {
                        hidden.extend(
                            scene
                                .computed_curves
                                .iter()
                                .filter(|curve| curve.owner.corner == corner)
                                .map(|curve| SelectionItem::FeatureCorner(curve.owner)),
                        );
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Scalar(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Contact(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Source(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Parameter(_)
                    | geosolve_constraint_editor::IntentNativeBinding::ExternalBinding(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Logical(_) => {}
                }
            }
        }
        hidden.into_iter().collect()
    }

    fn snapshot(&mut self) -> Result<BridgeSnapshot, String> {
        let frame = self.frame_snapshot();
        let source = self.source_snapshot()?;
        let explorer = self.explorer_snapshot();
        let selection = self.selection_snapshot();
        let parameters = self.parameter_snapshot();
        let problems = self.problem_snapshot();
        let status = if !problems.is_empty() {
            ProjectStatus::Failed
        } else if source.dirty {
            ProjectStatus::Dirty
        } else {
            ProjectStatus::Accepted
        };
        let selected_geometry_role = self
            .editor()
            .selected_geometry_role_state()
            .ok()
            .flatten()
            .map(|state| match state {
                GeometryRoleSelectionState::Profile => GeometryRoleStateSnapshot::Profile,
                GeometryRoleSelectionState::Construction => GeometryRoleStateSnapshot::Construction,
                GeometryRoleSelectionState::Mixed => GeometryRoleStateSnapshot::Mixed,
            });
        let geometry_visibility = self
            .editor()
            .editor()
            .geometry_interaction_policy()
            .visibility;
        Ok(BridgeSnapshot {
            version: PROTOCOL_VERSION,
            revision: self.revision,
            project: ProjectSnapshot {
                title: self.title.clone(),
                sample_key: self.samples.selected_key().map(str::to_owned),
                status,
            },
            presentation: PresentationSnapshot {
                active_tool: self.active_tool.clone(),
                grid_visible: self.grid_visible,
                construction_visible: geometry_visibility.explicit_construction
                    && geometry_visibility.implicit_construction,
                visibility_restore_available: self.explorer_visibility.isolate_restore.is_some(),
                can_undo: self.can_undo(),
                can_redo: self.can_redo(),
                can_finish: self.can_finish_active_tool(),
                geometry_role: self.editor().editor().authoring_geometry_role(),
                selected_geometry_role,
            },
            frame,
            source,
            explorer,
            selection,
            parameters,
            problems,
            pending_managed_mutation: self
                .pending_managed_mutation
                .as_ref()
                .map(pending_managed_snapshot),
        })
    }

    fn frame_snapshot(&mut self) -> FrameSnapshot {
        if std::mem::take(&mut self.preserve_frame_once)
            && let Some(frame) = &self.accepted_frame
        {
            return frame.clone();
        }
        let frame = self.compose_frame_snapshot();
        self.accepted_frame = Some(frame.clone());
        frame
    }

    fn compose_frame_snapshot(&mut self) -> FrameSnapshot {
        let scene = self.current_scene();
        let editor = self.editor();
        let accepted = editor.presentation_session().and_then(
            geosolve_sketch::RetainedSketchDocumentSession::accepted_state_for_current_input,
        );
        let mut selection = editor.editor().selection().to_vec();
        if let Some(item) = editor.feature_authoring_preview_item() {
            selection.push(item);
            selection.sort_unstable();
            selection.dedup();
        }
        let markup = super::scene::svg_markup_with_computed_context_action_stamp_and_display(
            scene.as_ref(),
            accepted,
            &[],
            &selection,
            &[],
            editor.editor().hover_state(),
            self.construction_preview.as_ref(),
            editor.editor().draft_inference_resolution(),
            None,
            None,
            None,
            editor.editor().geometry_interaction_policy(),
            super::scene::CanvasDisplayOptions {
                grid_visible: self.grid_visible,
                retain_contextual_annotations: true,
            },
            self.camera.viewport(),
        );
        // This SVG is accepted-scene authority, not a transient status
        // surface. Draft and error notices must not mutate a retained frame.
        let aria_label = format!("{} accepted sketch viewport", self.title);
        let screen = self.camera.viewport().screen_size;
        FrameSnapshot {
            svg: geosolve_sketch_render::interactive_scene_svg(&markup, screen, &aria_label)
                .expect("validated camera viewport has finite positive screen dimensions"),
            aria_label,
        }
    }

    fn source_snapshot(&self) -> Result<SourceSnapshot, String> {
        if let Some(code) = &self.code_project {
            let mut files = vec![SourceFileSnapshot {
                path: "sketch.ts".into(),
                language: "typescript",
                contents: code.managed_draft().to_owned(),
                read_only: false,
            }];
            files.extend(
                code.custom_files()
                    .map(|(path, contents)| SourceFileSnapshot {
                        path: path.to_owned(),
                        language: source_language(path),
                        contents: contents.to_owned(),
                        read_only: true,
                    }),
            );
            Ok(SourceSnapshot {
                selected_path: code.selected_file_path().to_owned(),
                files,
                dirty: code.is_dirty(),
            })
        } else {
            let projection = self.editor().workbench_projection();
            Ok(SourceSnapshot {
                selected_path: "design.intent.json".into(),
                files: vec![SourceFileSnapshot {
                    path: "design.intent.json".into(),
                    language: "json",
                    contents: serde_json::to_string_pretty(&projection)
                        .map_err(|error| error.to_string())?,
                    read_only: true,
                }],
                dirty: false,
            })
        }
    }

    // The capability matrix is serialized as one cohesive projection so the
    // frontend never has to infer source-authority policy from row shape.
    #[allow(clippy::too_many_lines)]
    fn base_explorer_snapshot(&self) -> Vec<ExplorerSnapshot> {
        let selected = self.editor().selected_declaration();
        let Some(code) = &self.code_project else {
            return self
                .editor()
                .workbench_projection()
                .outline
                .into_iter()
                .map(|cell| ExplorerSnapshot {
                    id: format!("intent-group:{}", cell.cell),
                    label: cell.name.to_string(),
                    kind: "Group".into(),
                    row_kind: ExplorerRowKind::Group,
                    selected: false,
                    suppressed: None,
                    source: None,
                    visible: true,
                    effective_visible: true,
                    visibility_state: ExplorerVisibilitySnapshot::Visible,
                    children: cell
                        .declarations
                        .into_iter()
                        .map(|declaration| ExplorerSnapshot {
                            id: intent_panel_row_id(&declaration.symbol),
                            label: declaration.name.to_string(),
                            kind: node_family_label(&declaration.kind).into(),
                            row_kind: ExplorerRowKind::Declaration,
                            selected: selected == Some(declaration.node),
                            suppressed: Some(declaration.suppressed),
                            source: None,
                            visible: true,
                            effective_visible: true,
                            visibility_state: ExplorerVisibilitySnapshot::Visible,
                            children: Vec::new(),
                            capabilities: read_only_intent_capabilities(),
                        })
                        .collect(),
                    capabilities: group_capabilities(),
                })
                .collect();
        };

        let projection = code.declaration_panel_projection(self.editor());
        let blocked = projection.blocked_reason.as_deref();
        let declaration_count = projection.declarations.len();
        let mut groups = Vec::<ExplorerSnapshot>::new();
        let mut run_index = 0usize;
        let mut current_group: Option<String> = None;
        for (index, declaration) in projection.declarations.into_iter().enumerate() {
            let group = declaration
                .group
                .clone()
                .unwrap_or_else(|| "Declarations".into());
            if current_group.as_deref() != Some(&group) {
                run_index += 1;
                current_group = Some(group.clone());
                groups.push(ExplorerSnapshot {
                    id: format!("managed-group:{run_index}:{group}"),
                    label: group,
                    kind: "Group".into(),
                    row_kind: ExplorerRowKind::Group,
                    selected: false,
                    suppressed: None,
                    source: None,
                    visible: true,
                    effective_visible: true,
                    visibility_state: ExplorerVisibilitySnapshot::Visible,
                    children: Vec::new(),
                    capabilities: group_capabilities(),
                });
            }
            groups
                .last_mut()
                .expect("one group is installed before its declaration")
                .children
                .push(managed_declaration_explorer_snapshot(
                    declaration,
                    blocked,
                    index > 0,
                    index + 1 < declaration_count,
                ));
        }
        groups
    }

    fn explorer_snapshot(&self) -> Vec<ExplorerSnapshot> {
        let mut rows = self.base_explorer_snapshot();
        apply_explorer_visibility(&mut rows, &self.explorer_visibility.hidden_rows, true);
        rows
    }

    fn selection_snapshot(&self) -> Option<SelectionSnapshot> {
        let editor = self.editor();
        let projection = editor.workbench_projection();
        if let Some(inspector) = editor.selected_inspector(&projection) {
            let (ownership, source) = self.code_project.as_ref().map_or_else(
                || ("Modifiable instance".into(), None),
                |code| managed_selection_source(code, editor, &inspector),
            );
            return Some(SelectionSnapshot {
                id: inspector.node.to_string(),
                label: inspector.name.to_string(),
                kind: node_family_label(&inspector.kind).into(),
                ownership: Some(ownership),
                source,
            });
        }
        editor
            .editor()
            .selection()
            .first()
            .map(|item| SelectionSnapshot {
                id: format!("{item:?}"),
                label: selection_label(*item),
                kind: selection_kind(*item).into(),
                ownership: Some("Modifiable instance".into()),
                source: None,
            })
    }

    fn parameter_snapshot(&self) -> Vec<ParameterSnapshot> {
        let Some(code) = &self.code_project else {
            return Vec::new();
        };
        let Ok(manifest) = code.managed_controls_cached() else {
            return Vec::new();
        };
        manifest
            .controls
            .iter()
            .filter_map(|control| {
                let editable = matches!(control.access, ManagedControlAccess::Editable { .. });
                let (value, unit) = managed_value_text(&control.value)?;
                Some(ParameterSnapshot {
                    id: control.id.0.clone(),
                    label: managed_control_label(control),
                    value,
                    unit,
                    editable,
                })
            })
            .collect()
    }

    fn problem_snapshot(&self) -> Vec<ProblemSnapshot> {
        let mut problems = Vec::new();
        if let Some(code) = &self.code_project {
            if let Some(diagnostic) = code.draft_diagnostic() {
                problems.push(ProblemSnapshot {
                    id: "managed-source".into(),
                    severity: "error",
                    title: "Managed source is invalid".into(),
                    detail: diagnostic.message.clone(),
                    file: Some("sketch.ts".into()),
                    line: Some(diagnostic.line),
                    column: Some(diagnostic.column),
                });
            } else if let Some((stage, diagnostic)) = code.retained_failure() {
                problems.push(ProblemSnapshot {
                    id: "retained-code-failure".into(),
                    severity: "error",
                    title: format!("Retained {stage} failure"),
                    detail: diagnostic,
                    file: Some("sketch.ts".into()),
                    line: None,
                    column: None,
                });
            }
        }
        let projection = self.editor().workbench_projection();
        if let Some(diagnostic) = projection.latest_diagnostic
            && !problems
                .iter()
                .any(|problem| problem.id == "retained-code-failure")
        {
            problems.push(ProblemSnapshot {
                id: "retained-intent-failure".into(),
                severity: "error",
                title: "Design intent retained a failure".into(),
                detail: diagnostic.to_string(),
                file: None,
                line: None,
                column: None,
            });
        }
        if let Some(error) = &self.last_error
            && !problems.iter().any(|problem| problem.detail == *error)
        {
            problems.push(ProblemSnapshot {
                id: "workbench-interaction".into(),
                severity: "error",
                title: "Workbench action was retained".into(),
                detail: error.clone(),
                file: None,
                line: None,
                column: None,
            });
        }
        problems
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1).min(MAX_SAFE_INTEGER);
    }
}

fn command_allowed_while_managed_mutation_pending(command: &str) -> bool {
    matches!(
        command,
        "managed.mutation.resolve"
            | "managed.mutation.abort"
            | "explorer.visibility.set"
            | "explorer.visibility.isolate"
            | "explorer.visibility.restore"
            | "view.construction.toggle"
    )
}

fn decode_request<T: for<'de> Deserialize<'de>>(request: &str) -> Result<T, String> {
    if request.len() > MAX_REQUEST_BYTES {
        return Err(format!(
            "workbench bridge request exceeds {MAX_REQUEST_BYTES} bytes"
        ));
    }
    serde_json::from_str(request).map_err(|error| format!("invalid workbench request: {error}"))
}

fn decode_payload<T: for<'de> Deserialize<'de>>(payload: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|error| format!("invalid command payload: {error}"))
}

fn decode_visibility_rows(rows: Vec<String>) -> Result<std::collections::BTreeSet<String>, String> {
    if rows.len() > MAX_VISIBILITY_ROWS {
        return Err(format!(
            "Explorer visibility exceeds the {MAX_VISIBILITY_ROWS}-row limit"
        ));
    }
    if rows
        .iter()
        .any(|row| row.is_empty() || row.len() > MAX_COMMAND_BYTES)
    {
        return Err("Explorer visibility contains an invalid row identity".into());
    }
    let count = rows.len();
    let rows = rows.into_iter().collect::<std::collections::BTreeSet<_>>();
    if rows.len() != count {
        return Err("Explorer visibility contains duplicate row identities".into());
    }
    Ok(rows)
}

fn require_version(version: u8) -> Result<(), String> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err("unsupported workbench bridge protocol version".into())
    }
}

fn authoring_tool(id: &str) -> Option<geosolve_constraint_editor::AuthoringTool> {
    let normalized = match id {
        "distance" => "point-distance",
        "angle" => "oriented-angle",
        value => value,
    };
    super::action_surface::authoring_tool_from_key(normalized)
}

fn geometry_variant(id: &str) -> Option<GeometryToolVariant> {
    let exact = super::geometry_palette::variant_from_key(id);
    exact.or_else(|| {
        Some(match id {
            "point" => GeometryToolVariant::SketchPoint,
            "line" => GeometryToolVariant::Segment,
            "polyline" => GeometryToolVariant::Polyline,
            "rectangle" => GeometryToolVariant::TwoPointAlignedRectangle,
            "circle" => GeometryToolVariant::CenterRadiusCircle,
            "arc" => GeometryToolVariant::CenterArc,
            "ellipse" => GeometryToolVariant::CenterAxesEllipse,
            "bezier" => GeometryToolVariant::CubicBezier,
            "conic" => GeometryToolVariant::RationalQuadraticConic,
            "nurbs" => GeometryToolVariant::OpenControlNurbs,
            _ => return None,
        })
    })
}

fn source_language(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
    {
        Some(extension)
            if extension.eq_ignore_ascii_case("ts") || extension.eq_ignore_ascii_case("tsx") =>
        {
            "typescript"
        }
        Some(extension) if extension.eq_ignore_ascii_case("json") => "json",
        _ => "text",
    }
}

fn intent_panel_row_id(symbol: &IntentKey) -> String {
    format!("intent:{symbol}")
}

fn apply_explorer_visibility(
    rows: &mut [ExplorerSnapshot],
    hidden: &std::collections::BTreeSet<String>,
    ancestor_visible: bool,
) {
    for row in rows {
        row.visible = !hidden.contains(&row.id);
        row.effective_visible = ancestor_visible && row.visible;
        apply_explorer_visibility(&mut row.children, hidden, row.effective_visible);
        row.visibility_state = if !row.effective_visible {
            ExplorerVisibilitySnapshot::Hidden
        } else if row.children.is_empty() {
            ExplorerVisibilitySnapshot::Visible
        } else {
            let mut has_visible = false;
            let mut has_hidden = false;
            for child in &row.children {
                match child.visibility_state {
                    ExplorerVisibilitySnapshot::Visible => has_visible = true,
                    ExplorerVisibilitySnapshot::Hidden => has_hidden = true,
                    ExplorerVisibilitySnapshot::Mixed => {
                        has_visible = true;
                        has_hidden = true;
                    }
                }
            }
            if row.row_kind != ExplorerRowKind::Group {
                has_visible = true;
            }
            match (has_visible, has_hidden) {
                (true, true) => ExplorerVisibilitySnapshot::Mixed,
                (false, true) => ExplorerVisibilitySnapshot::Hidden,
                (_, false) => ExplorerVisibilitySnapshot::Visible,
            }
        };
    }
}

fn find_explorer_row<'a>(rows: &'a [ExplorerSnapshot], id: &str) -> Option<&'a ExplorerSnapshot> {
    rows.iter().find_map(|row| {
        (row.id == id)
            .then_some(row)
            .or_else(|| find_explorer_row(&row.children, id))
    })
}

fn collect_effectively_hidden_leaves(rows: &[ExplorerSnapshot], hidden: &mut Vec<String>) {
    for row in rows {
        if row.row_kind != ExplorerRowKind::Group && !row.effective_visible {
            hidden.push(row.id.clone());
        }
        collect_effectively_hidden_leaves(&row.children, hidden);
    }
}

fn managed_declaration_row_target(
    declarations: &[ManagedDeclarationPanelRow],
    id: &str,
) -> Option<DeclarationRowTarget> {
    for declaration in declarations {
        if declaration.id == id {
            return Some(DeclarationRowTarget::Managed {
                node: declaration.selection_node,
                symbol: declaration.symbol.0.clone(),
                from: declaration.source_start,
                to: declaration.source_end,
                suppression_control_id: declaration.suppression_control_id.clone(),
                closure_role: declaration.closure_role.clone(),
            });
        }
        if let Some(generated) = declaration
            .generated
            .iter()
            .find(|generated| generated.id == id)
        {
            return Some(DeclarationRowTarget::Generated {
                node: generated.selection_node,
                address: generated.address.clone(),
                from: generated.source_start,
                to: generated.source_end,
                suppressed: generated.suppressed,
                suppression_token: generated.suppression_token.clone(),
            });
        }
        if let Some(target) = managed_declaration_row_target(&declaration.closure_helpers, id) {
            return Some(target);
        }
    }
    None
}

fn enabled_capability() -> ExplorerCapability {
    ExplorerCapability {
        enabled: true,
        reason: None,
    }
}

fn disabled_capability(reason: impl Into<String>) -> ExplorerCapability {
    ExplorerCapability {
        enabled: false,
        reason: Some(reason.into()),
    }
}

fn capability_if(enabled: bool, reason: impl Into<String>) -> ExplorerCapability {
    if enabled {
        enabled_capability()
    } else {
        disabled_capability(reason)
    }
}

fn managed_generated_explorer_snapshot(
    generated: ManagedGeneratedPanelRow,
    blocked: Option<&str>,
) -> ExplorerSnapshot {
    let has_suppression_target = generated.suppression_token.is_some();
    let suppression = capability_if(
        blocked.is_none() && has_suppression_target,
        blocked.unwrap_or("This generated output has no unique reversible suppression target"),
    );
    ExplorerSnapshot {
        id: generated.id,
        label: generated.label,
        kind: generated.kind,
        row_kind: ExplorerRowKind::Generated,
        selected: generated.selected,
        suppressed: Some(generated.suppressed),
        source: Some(SelectionSourceSnapshot {
            path: "sketch.ts".into(),
            from: generated.source_start,
            to: generated.source_end,
        }),
        visible: true,
        effective_visible: true,
        visibility_state: ExplorerVisibilitySnapshot::Visible,
        children: Vec::new(),
        capabilities: ExplorerCapabilities {
            select: capability_if(
                generated.selection_node.is_some(),
                "This generated output has no independently selectable scene node",
            ),
            navigate: capability_if(
                blocked.is_none(),
                blocked.unwrap_or("Generated source ownership is unavailable"),
            ),
            edit: disabled_capability(
                "Generated outputs are edited through their source invocation",
            ),
            r#move: disabled_capability(
                "Generated outputs remain ordered by their source invocation",
            ),
            move_up: disabled_capability("Generated outputs cannot move independently"),
            move_down: disabled_capability("Generated outputs cannot move independently"),
            suppress: suppression,
            delete: capability_if(
                blocked.is_none() && has_suppression_target && !generated.suppressed,
                blocked.unwrap_or("This generated output has no reversible deletion target"),
            ),
        },
    }
}

fn managed_declaration_explorer_snapshot(
    declaration: ManagedDeclarationPanelRow,
    blocked: Option<&str>,
    move_up_enabled: bool,
    move_down_enabled: bool,
) -> ExplorerSnapshot {
    let navigation = capability_if(
        blocked.is_none(),
        blocked.unwrap_or("Declaration source is unavailable"),
    );
    let select = capability_if(
        declaration.selection_node.is_some(),
        "This source declaration has no independently selectable scene output",
    );
    let helper_root = match &declaration.closure_role {
        ManagedDeclarationClosureRole::Helper { root } => Some(root.0.as_str()),
        ManagedDeclarationClosureRole::Independent | ManagedDeclarationClosureRole::Root => None,
    };
    let move_reason = helper_root.map_or_else(
        || "This declaration cannot move in the current source authority".to_owned(),
        |root| format!("Profile Offset helpers move with `{root}`"),
    );
    let lifecycle_reason =
        helper_root.map(|root| format!("Profile Offset helper lifecycle is owned by `{root}`"));
    let deletion = lifecycle_reason.as_ref().map_or_else(
        || {
            capability_if(
                blocked.is_none(),
                blocked.unwrap_or("This declaration has no authenticated deletion target"),
            )
        },
        |reason| disabled_capability(reason.clone()),
    );
    let suppress = lifecycle_reason.as_ref().map_or_else(
        || {
            capability_if(
                blocked.is_none(),
                blocked
                    .unwrap_or("This declaration has no explicit source-owned suppression target"),
            )
        },
        |reason| disabled_capability(reason.clone()),
    );
    let is_helper = helper_root.is_some();
    let mut children = declaration
        .closure_helpers
        .into_iter()
        .map(|helper| managed_declaration_explorer_snapshot(helper, blocked, false, false))
        .collect::<Vec<_>>();
    children.extend(
        declaration
            .generated
            .into_iter()
            .map(|generated| managed_generated_explorer_snapshot(generated, blocked)),
    );
    ExplorerSnapshot {
        id: declaration.id,
        label: declaration.label,
        kind: declaration.kind,
        row_kind: ExplorerRowKind::Declaration,
        selected: declaration.selected,
        suppressed: declaration.suppressed,
        source: Some(SelectionSourceSnapshot {
            path: "sketch.ts".into(),
            from: declaration.source_start,
            to: declaration.source_end,
        }),
        visible: true,
        effective_visible: true,
        visibility_state: ExplorerVisibilitySnapshot::Visible,
        children,
        capabilities: ExplorerCapabilities {
            select,
            navigate: navigation,
            edit: capability_if(
                blocked.is_none(),
                blocked.unwrap_or("Declaration editing is unavailable"),
            ),
            r#move: capability_if(blocked.is_none() && !is_helper, move_reason.clone()),
            move_up: capability_if(
                blocked.is_none() && !is_helper && move_up_enabled,
                if is_helper {
                    move_reason.clone()
                } else if !move_up_enabled {
                    "Already first in source order".into()
                } else {
                    move_reason.clone()
                },
            ),
            move_down: capability_if(
                blocked.is_none() && !is_helper && move_down_enabled,
                if is_helper {
                    move_reason
                } else if !move_down_enabled {
                    "Already last in source order".into()
                } else {
                    move_reason
                },
            ),
            suppress,
            delete: deletion,
        },
    }
}

fn group_capabilities() -> ExplorerCapabilities {
    ExplorerCapabilities {
        select: disabled_capability("Groups organize declarations and are not selectable"),
        navigate: disabled_capability("This group has no source declaration"),
        edit: disabled_capability("Edit declarations inside this group"),
        r#move: disabled_capability("Group reordering is unavailable"),
        move_up: disabled_capability("Group reordering is unavailable"),
        move_down: disabled_capability("Group reordering is unavailable"),
        suppress: disabled_capability("Groups cannot be suppressed"),
        delete: disabled_capability("Groups cannot be deleted here"),
    }
}

fn read_only_intent_capabilities() -> ExplorerCapabilities {
    ExplorerCapabilities {
        select: enabled_capability(),
        navigate: disabled_capability("Ordinary projects have no writable sketch.ts source"),
        edit: disabled_capability("Ordinary Intent declarations are read-only in this panel"),
        r#move: disabled_capability("Ordinary Intent order is read-only in this panel"),
        move_up: disabled_capability("Ordinary Intent order is read-only in this panel"),
        move_down: disabled_capability("Ordinary Intent order is read-only in this panel"),
        suppress: disabled_capability("Ordinary Intent suppression is read-only in this panel"),
        delete: disabled_capability("Ordinary Intent deletion is read-only in this panel"),
    }
}

fn managed_selection_source(
    code: &CodeProjectWorkbench,
    editor: &geosolve_constraint_editor::ProjectionalEditorSession,
    inspector: &geosolve_constraint_editor::IntentInspectorProjection,
) -> (String, Option<SelectionSourceSnapshot>) {
    let declaration = match code.selected_managed_declaration(editor) {
        Ok(None) => return ("Modifiable instance".into(), None),
        Err(_) => return ("Blocked".into(), None),
        Ok(Some(declaration)) => declaration,
    };

    let Ok(presentations) = code.inspector_parameter_presentations(editor, inspector) else {
        return ("Blocked".into(), None);
    };
    let mut sources = presentations
        .iter()
        .filter_map(|presentation| match &presentation.authority {
            super::design_projection::InspectorParameterAuthority::ModifiableSource {
                source_start,
                source_end,
                source_path,
                ..
            } => Some((source_path.clone(), *source_start, *source_end)),
            _ => None,
        })
        .collect::<Vec<_>>();
    sources.sort_unstable();
    sources.dedup();
    if !sources.is_empty() {
        if let Ok(span) = code.managed_declaration_source_span(&declaration)
            && sources
                .iter()
                .all(|source| span.start <= source.1 && source.2 <= span.end)
        {
            return (
                "Modifiable in source".into(),
                Some(SelectionSourceSnapshot {
                    path: "sketch.ts".into(),
                    from: span.start,
                    to: span.end,
                }),
            );
        }
        return ("Encoded · choose a parameter".into(), None);
    }
    if presentations.iter().any(|presentation| {
        matches!(
            presentation.authority,
            super::design_projection::InspectorParameterAuthority::Blocked { .. }
        )
    }) {
        return ("Blocked".into(), None);
    }
    if presentations.iter().any(|presentation| {
        matches!(
            presentation.authority,
            super::design_projection::InspectorParameterAuthority::ModifiableInstance
        )
    }) {
        return ("Modifiable instance".into(), None);
    }
    ("Encoded".into(), None)
}

fn selection_kind(item: SelectionItem) -> &'static str {
    match item {
        SelectionItem::Point(_) => "Point",
        SelectionItem::Curve(_) => "Curve",
        SelectionItem::Constraint(_) => "Constraint",
        SelectionItem::Dimension(_) => "Dimension",
        SelectionItem::Datum(_) => "Datum",
        SelectionItem::Feature(_) => "Feature",
        SelectionItem::FeatureCorner(_) => "Fillet",
    }
}

fn selection_label(item: SelectionItem) -> String {
    format!("{} · {item:?}", selection_kind(item))
}

fn managed_control_label(control: &geosolve_sketch_code::ManagedControl) -> String {
    let path = control
        .source
        .path
        .0
        .iter()
        .map(|segment| match segment {
            geosolve_sketch_code::ManagedPathSegment::Field(field) => field.clone(),
            geosolve_sketch_code::ManagedPathSegment::Index(index) => format!("item {}", index + 1),
            geosolve_sketch_code::ManagedPathSegment::Member { member } => member.clone(),
        })
        .collect::<Vec<_>>()
        .join(" · ");
    if path.is_empty() {
        control.source.declaration.0.clone()
    } else {
        format!("{} · {path}", control.source.declaration.0)
    }
}

fn managed_value_text(value: &ManagedValue) -> Option<(String, Option<String>)> {
    match value {
        ManagedValue::Number(value) => Some((value.to_string(), None)),
        ManagedValue::Unit(value) => Some((value.value.to_string(), Some(value.unit.clone()))),
        ManagedValue::Bool(value) => Some((value.to_string(), None)),
        ManagedValue::String(value) => Some((value.clone(), None)),
        ManagedValue::Null
        | ManagedValue::Array(_)
        | ManagedValue::Object(_)
        | ManagedValue::Reference { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        EditorHoverTarget, Modifiers, PointerInput, ScreenPoint, SelectionItem,
    };
    use geosolve_sketch::{
        ContactDomain, ContactNeighborhood, CurveDefinition, DocumentConstraintDefinition,
        GeometryRole,
    };
    use geosolve_sketch_code::{
        CodeProject, CompiledManagedSource, ManagedMutationTarget, ManagedSketchMutation,
        ManagedSpan, PreparedManagedMutationReceipt,
    };

    use super::{
        MAX_REQUEST_BYTES, ManagedMutationAbortPayload, PendingManagedMutation, WorkbenchBridge,
        WorkbenchDocumentAuthority,
    };
    use crate::workbench::code_projects::CodeProjectWorkbench;

    fn managed_compiler_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
        )))
        .expect("checked managed compiler fixture")
    }

    fn managed_restored_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope-restored.json"
        )))
        .expect("checked managed suppression-restore fixture")
    }

    fn managed_direct_fillet_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-fillet.json"
        )))
        .expect("checked managed direct-Fillet fixture")
    }

    fn managed_direct_fillet_radius_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-fillet-radius.json"
        )))
        .expect("checked managed direct-Fillet radius fixture")
    }

    fn managed_empty_circle_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-empty-circle.json"
        )))
        .expect("checked managed empty-circle fixture")
    }

    fn managed_two_circles_fixture(with_snapped_segment: bool) -> CompiledManagedSource {
        let fixture = if with_snapped_segment {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-two-circles-snapped-segment.json"
            ))
        } else {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-two-circles.json"
            ))
        };
        CompiledManagedSource::from_json(fixture).expect("checked managed two-circle fixture")
    }

    fn managed_lifecycle_fixture(name: &str) -> CompiledManagedSource {
        let fixture = match name {
            "base" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-lifecycle-base.json"
            )),
            "reordered" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-lifecycle-reordered.json"
            )),
            "direct-suppressed" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-lifecycle-direct-suppressed.json"
            )),
            "both-suppressed" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-lifecycle-both-suppressed.json"
            )),
            unknown => panic!("unknown managed lifecycle fixture `{unknown}`"),
        };
        CompiledManagedSource::from_json(fixture)
            .expect("checked managed lifecycle compiler fixture")
    }

    fn managed_profile_offset_closure_fixture(name: &str) -> CompiledManagedSource {
        let fixture = match name {
            "base" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-base.json"
            )),
            "reordered" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-reordered.json"
            )),
            "restored-order" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-restored-order.json"
            )),
            "deleted" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-deleted.json"
            )),
            unknown => panic!("unknown managed Profile Offset closure fixture `{unknown}`"),
        };
        CompiledManagedSource::from_json(fixture)
            .expect("checked managed Profile Offset closure compiler fixture")
    }

    fn managed_bridge() -> WorkbenchBridge {
        let (code, editor) = CodeProjectWorkbench::open_managed_test_compiled(
            "bridge-managed-test",
            managed_compiler_fixture(),
        )
        .expect("materialized managed bridge fixture");
        let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)
            .expect("managed fixture editor authority");
        WorkbenchBridge::from_parts(
            authority,
            Some(code),
            crate::workbench::samples::SampleCatalogState::default(),
            "Managed bridge test".into(),
            "Managed bridge fixture ready".into(),
        )
        .expect("managed fixture bridge")
    }

    fn managed_direct_fillet_bridge() -> WorkbenchBridge {
        let (code, editor) = CodeProjectWorkbench::open_managed_test_compiled(
            "bridge-managed-direct-fillet-test",
            managed_direct_fillet_fixture(),
        )
        .expect("materialized managed direct-Fillet bridge fixture");
        let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)
            .expect("managed direct-Fillet editor authority");
        WorkbenchBridge::from_parts(
            authority,
            Some(code),
            crate::workbench::samples::SampleCatalogState::default(),
            "Managed direct Fillet bridge test".into(),
            "Managed direct Fillet fixture ready".into(),
        )
        .expect("managed direct-Fillet bridge")
    }

    fn managed_two_circles_bridge() -> WorkbenchBridge {
        let (code, editor) = CodeProjectWorkbench::open_managed_test_compiled_with_high_water(
            "bridge-managed-two-circles-test",
            managed_two_circles_fixture(false),
            2,
        )
        .expect("materialized managed two-circle bridge fixture");
        let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)
            .expect("managed two-circle editor authority");
        WorkbenchBridge::from_parts(
            authority,
            Some(code),
            crate::workbench::samples::SampleCatalogState::default(),
            "Managed two-circle test".into(),
            "Managed two-circle fixture ready".into(),
        )
        .expect("managed two-circle fixture bridge")
    }

    fn managed_lifecycle_bridge() -> WorkbenchBridge {
        let (code, editor) = CodeProjectWorkbench::open_managed_test_compiled_with_demo_pins(
            "bridge-managed-lifecycle-test",
            "typed-panel",
            managed_lifecycle_fixture("base"),
        )
        .expect("materialized managed lifecycle bridge fixture");
        let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)
            .expect("managed lifecycle editor authority");
        WorkbenchBridge::from_parts(
            authority,
            Some(code),
            crate::workbench::samples::SampleCatalogState::default(),
            "Managed lifecycle test".into(),
            "Managed lifecycle fixture ready".into(),
        )
        .expect("managed lifecycle bridge")
    }

    fn managed_profile_offset_closure_bridge() -> WorkbenchBridge {
        let (code, editor) = CodeProjectWorkbench::open_managed_test_compiled(
            "bridge-managed-profile-offset-closure-test",
            managed_profile_offset_closure_fixture("base"),
        )
        .expect("materialized managed Profile Offset closure bridge fixture");
        let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)
            .expect("managed Profile Offset closure editor authority");
        WorkbenchBridge::from_parts(
            authority,
            Some(code),
            crate::workbench::samples::SampleCatalogState::default(),
            "Managed Profile Offset closure test".into(),
            "Managed Profile Offset closure fixture ready".into(),
        )
        .expect("managed Profile Offset closure fixture bridge")
    }

    fn accepted_compiled_project(bridge: &WorkbenchBridge) -> CodeProject {
        CodeProject::from_json(&canonical_project_json(bridge))
            .expect("bridge project is canonical")
    }

    fn canonical_project_json(bridge: &WorkbenchBridge) -> String {
        let export: serde_json::Value = serde_json::from_str(
            &bridge
                .export_project_json()
                .expect("bridge exports accepted canonical project"),
        )
        .expect("bridge export envelope");
        export["contents"]
            .as_str()
            .expect("canonical project export contents")
            .to_owned()
    }

    fn declaration_order(bridge: &WorkbenchBridge) -> Vec<String> {
        accepted_compiled_project(bridge)
            .managed
            .compiled
            .expect("managed authority")
            .ir
            .statements
            .into_iter()
            .filter_map(|statement| match statement {
                geosolve_sketch_code::ManagedStatement::Declaration { symbol, .. } => Some(symbol),
                _ => None,
            })
            .collect()
    }

    fn managed_row_id(snapshot: &serde_json::Value, label: &str) -> String {
        snapshot["explorer"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|group| group["children"].as_array().into_iter().flatten())
            .find(|row| row["label"] == label)
            .and_then(|row| row["id"].as_str())
            .unwrap_or_else(|| panic!("managed declaration row `{label}`"))
            .to_owned()
    }

    fn nested_explorer_row(snapshot: &serde_json::Value, label: &str) -> serde_json::Value {
        fn find<'a>(rows: &'a [serde_json::Value], label: &str) -> Option<&'a serde_json::Value> {
            rows.iter().find_map(|row| {
                (row["label"] == label).then_some(row).or_else(|| {
                    row["children"]
                        .as_array()
                        .and_then(|children| find(children, label))
                })
            })
        }

        let groups = snapshot["explorer"]
            .as_array()
            .expect("snapshot explorer groups");
        find(groups, label)
            .unwrap_or_else(|| panic!("nested explorer row `{label}`"))
            .clone()
    }

    fn pointer_request(
        phase: &str,
        pointer_id: u64,
        position: ScreenPoint,
        buttons: u16,
    ) -> String {
        serde_json::json!({
            "version": 1,
            "phase": phase,
            "pointerId": pointer_id,
            "x": position.x,
            "y": position.y,
            "buttons": buttons,
            "modifiers": {
                "alt": false,
                "ctrl": false,
                "meta": false,
                "shift": false,
            },
        })
        .to_string()
    }

    fn m90_f005_reproduction_bridge() -> WorkbenchBridge {
        const FIXTURE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/m90_f005_native_drag_repro.txt"
        ));
        const WORKSPACE_BYTES: usize = 956_305;
        const WORKSPACE_SHA256: &str =
            "c5f748d31c90f8fd575ab2acaddfb8b7d20bbc9f31189dc0716995ad05a46ee7";

        let workspace = crate::reproduction::decode_workspace(FIXTURE.trim_end())
            .expect("checked exact M90-F005 reproduction capsule");
        assert_eq!(workspace.len(), WORKSPACE_BYTES);
        assert_eq!(
            geosolve_sketch_intent::intent_content_digest(workspace.as_bytes()).to_string(),
            WORKSPACE_SHA256,
        );
        WorkbenchBridge::construct_json(
            &serde_json::json!({
                "version": 1,
                "persistedProject": FIXTURE.trim_end(),
            })
            .to_string(),
        )
        .expect("M90-F005 reproduction restores through the ordinary bridge path")
    }

    fn m90_f005_point_position(bridge: &WorkbenchBridge, point_id: &str) -> [f64; 2] {
        bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("M90-F005 bridge retains accepted native authority")
            .session
            .accepted_state_for_current_input()
            .expect("M90-F005 native authority belongs to current input")
            .document()
            .points()
            .iter()
            .find(|point| point.id.to_string() == point_id)
            .unwrap_or_else(|| panic!("M90-F005 point `{point_id}`"))
            .position
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exact supplied-payload case keeps preview, publication, validation, metadata, persistence, and Undo invariants together"
    )]
    fn m90_f005_drag_and_require_publication(point_id: &str, pointer_id: u64) {
        let mut bridge = m90_f005_reproduction_bridge();
        assert!(bridge.code_project.is_none());
        assert!(bridge.pending_managed_mutation.is_none());

        let persistence_before = bridge.persistence_json().unwrap();
        let revision_before = bridge.revision;
        let history_before = bridge.editor().coordinator().intent().undo_len();
        let point_before = m90_f005_point_position(&bridge, point_id);
        let document_before = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .clone();
        let contacts_before = document_before.contacts().to_vec();
        let scene = bridge.current_scene().expect("accepted M90-F005 scene");
        let start = scene
            .points
            .iter()
            .find(|point| point.id.to_string() == point_id)
            .unwrap_or_else(|| panic!("M90-F005 scene point `{point_id}`"))
            .screen_position;
        let target = ScreenPoint {
            x: start.x + 20.0,
            y: start.y - 15.0,
        };

        bridge
            .pointer_json(&pointer_request("down", pointer_id, start, 1))
            .expect("M90-F005 point pointer down");
        assert_eq!(bridge.captured_pointer, Some(pointer_id));
        bridge
            .pointer_json(&pointer_request("move", pointer_id, target, 1))
            .expect("M90-F005 point pointer move");
        let preview_document = bridge
            .editor()
            .coordinator()
            .presentation_session()
            .expect("M90-F005 constrained drag produces a terminal preview")
            .accepted_state_for_current_input()
            .expect("M90-F005 terminal preview is independently accepted")
            .document()
            .clone();
        let preview = preview_document
            .points()
            .iter()
            .find(|point| point.id.to_string() == point_id)
            .unwrap_or_else(|| panic!("M90-F005 preview point `{point_id}`"))
            .position;
        assert!(preview.into_iter().all(f64::is_finite));
        assert_ne!(preview.map(f64::to_bits), point_before.map(f64::to_bits));

        bridge
            .pointer_json(&pointer_request("up", pointer_id, target, 0))
            .expect("M90-F005 point pointer up");

        assert!(
            bridge.pending_managed_mutation.is_none(),
            "native drag unexpectedly requested managed compilation"
        );
        assert!(
            bridge.last_error.is_none(),
            "M90-F005 point {point_id}: {:?}",
            bridge.last_error
        );
        assert_eq!(bridge.revision, revision_before + 1);
        assert_eq!(
            bridge.editor().coordinator().intent().undo_len(),
            history_before + 1,
            "one terminal release must publish exactly one Intent history row"
        );
        assert_ne!(bridge.persistence_json().unwrap(), persistence_before);

        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("M90-F005 release publishes accepted native authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
        let document = accepted
            .session
            .accepted_state_for_current_input()
            .expect("M90-F005 release belongs to current input")
            .document()
            .clone();
        assert!(
            document
                .points()
                .iter()
                .flat_map(|point| point.position)
                .chain(document.scalars().iter().map(|scalar| scalar.value))
                .all(f64::is_finite)
        );
        let published = document
            .points()
            .iter()
            .find(|point| point.id.to_string() == point_id)
            .unwrap_or_else(|| panic!("M90-F005 published point `{point_id}`"))
            .position;
        assert_eq!(published.map(f64::to_bits), preview.map(f64::to_bits));
        assert_eq!(
            document, preview_document,
            "release must retain the complete accepted terminal, including solver-moved companions",
        );
        assert_eq!(document.contacts().len(), contacts_before.len());
        for (before, after) in contacts_before.iter().zip(document.contacts()) {
            assert_eq!(after.id, before.id);
            assert_eq!(after.curve, before.curve);
            assert_eq!(after.parameter, before.parameter);
            assert_eq!(after.domain, before.domain);
            assert_eq!(after.winding, before.winding);
            assert_eq!(after.neighborhood, before.neighborhood);
            assert_eq!(after.tangent_orientation, before.tangent_orientation);
        }

        bridge
            .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
            .expect("M90-F005 drag Undo");
        assert_eq!(
            bridge
                .editor()
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document(),
            &document_before,
            "Undo must restore the supplied accepted document exactly"
        );
        assert_eq!(
            m90_f005_point_position(&bridge, point_id).map(f64::to_bits),
            point_before.map(f64::to_bits)
        );
        assert_eq!(
            bridge.editor().coordinator().intent().undo_len(),
            history_before
        );
        assert_eq!(bridge.editor().coordinator().intent().redo_len(), 1);

        bridge
            .dispatch_json(r#"{"version":1,"command":"history.redo"}"#)
            .expect("M90-F005 drag Redo");
        let redone = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document();
        assert_eq!(redone, &document);
        assert_eq!(
            m90_f005_point_position(&bridge, point_id).map(f64::to_bits),
            preview.map(f64::to_bits)
        );

        let persisted: serde_json::Value =
            serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
        let restored = WorkbenchBridge::construct_json(
            &serde_json::json!({
                "version": 1,
                "persistedProject": persisted["contents"].as_str().unwrap(),
            })
            .to_string(),
        )
        .expect("M90-F005 redone terminal restores from ordinary persistence");
        assert_eq!(
            m90_f005_point_position(&restored, point_id).map(f64::to_bits),
            preview.map(f64::to_bits)
        );
        assert_eq!(
            restored
                .editor()
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document(),
            &preview_document,
            "persistence must restore the complete accepted terminal",
        );
    }

    fn prepare_radius_restore(bridge: &mut WorkbenchBridge) {
        bridge
            .begin_structured_managed_mutation(
                "Restore radius",
                ManagedSketchMutation::SetSuppressed {
                    target: ManagedMutationTarget::Declaration {
                        declaration: "radius".into(),
                    },
                    suppressed: false,
                },
            )
            .expect("prepared radius restore");
    }

    fn pending_managed_receipt(
        bridge: &WorkbenchBridge,
        compiled: CompiledManagedSource,
    ) -> PreparedManagedMutationReceipt {
        let PendingManagedMutation::Managed { prepared, .. } = bridge
            .pending_managed_mutation
            .as_ref()
            .expect("pending managed mutation")
        else {
            panic!("expected structured managed mutation")
        };
        PreparedManagedMutationReceipt {
            ticket_digest: prepared.request.ticket.ticket_digest.clone(),
            base_source_digest: prepared.request.current.ir.source_digest.clone(),
            candidate_source_digest: compiled.ir.source_digest.clone(),
            compiled,
        }
    }

    fn pending_source_receipt(
        bridge: &WorkbenchBridge,
        compiled: CompiledManagedSource,
    ) -> PreparedManagedMutationReceipt {
        let PendingManagedMutation::Source { prepared } = bridge
            .pending_managed_mutation
            .as_ref()
            .expect("pending source mutation")
        else {
            panic!("expected raw-source mutation")
        };
        PreparedManagedMutationReceipt {
            ticket_digest: prepared.request.ticket.ticket_digest.clone(),
            base_source_digest: prepared.request.current.ir.source_digest.clone(),
            candidate_source_digest: compiled.ir.source_digest.clone(),
            compiled,
        }
    }

    #[test]
    fn middle_button_gesture_pans_without_entering_semantic_interaction() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let point = bridge
            .current_scene()
            .unwrap()
            .points
            .first()
            .expect("Typed Panel has a selectable point")
            .screen_position;
        assert!(bridge.editor().editor().selection().is_empty());

        let project_before = bridge.export_project_json().unwrap();
        let revision_before = bridge.revision;
        let camera_before = bridge.camera;
        bridge
            .pointer_json(
                &serde_json::json!({
                    "version": 1,
                    "phase": "down",
                    "pointerId": 17,
                    "x": point.x,
                    "y": point.y,
                    "buttons": 4,
                    "modifiers": {
                        "alt": false,
                        "ctrl": false,
                        "meta": false,
                        "shift": false,
                    },
                })
                .to_string(),
            )
            .unwrap();
        assert!(bridge.editor().editor().selection().is_empty());
        assert!(bridge.editor().editor().active_pointer_gesture().is_none());

        let moved = ScreenPoint {
            x: point.x + 60.0,
            y: point.y + 30.0,
        };
        let mut expected = camera_before;
        assert!(expected.pan_from(camera_before.model_center(), point, moved));
        bridge
            .pointer_json(
                &serde_json::json!({
                    "version": 1,
                    "phase": "move",
                    "pointerId": 17,
                    "x": moved.x,
                    "y": moved.y,
                    "buttons": 4,
                    "modifiers": {
                        "alt": false,
                        "ctrl": false,
                        "meta": false,
                        "shift": false,
                    },
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(bridge.camera, expected);

        let terminal = ScreenPoint {
            x: point.x + 80.0,
            y: point.y + 40.0,
        };
        assert!(expected.pan_from(camera_before.model_center(), point, terminal));
        bridge
            .pointer_json(
                &serde_json::json!({
                    "version": 1,
                    "phase": "up",
                    "pointerId": 17,
                    "x": terminal.x,
                    "y": terminal.y,
                    "buttons": 0,
                    "modifiers": {
                        "alt": false,
                        "ctrl": false,
                        "meta": false,
                        "shift": false,
                    },
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(bridge.camera, expected);
        assert!(bridge.editor().editor().selection().is_empty());
        assert!(bridge.editor().editor().active_pointer_gesture().is_none());
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(bridge.export_project_json().unwrap(), project_before);
    }

    #[test]
    fn middle_button_down_never_steals_an_existing_semantic_drag() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let point = bridge
            .current_scene()
            .unwrap()
            .points
            .first()
            .expect("Typed Panel has a selectable point")
            .screen_position;
        let pointer = |phase: &str, buttons: u16| {
            serde_json::json!({
                "version": 1,
                "phase": phase,
                "pointerId": 23,
                "x": point.x,
                "y": point.y,
                "buttons": buttons,
                "modifiers": {
                    "alt": false,
                    "ctrl": false,
                    "meta": false,
                    "shift": false,
                },
            })
            .to_string()
        };

        bridge.pointer_json(&pointer("down", 1)).unwrap();
        let active = bridge
            .editor()
            .editor()
            .active_pointer_gesture()
            .expect("primary point press owns a semantic drag");
        let selection = bridge.editor().editor().selection().to_vec();
        let camera = bridge.camera;
        assert_eq!(bridge.captured_pointer, Some(23));

        bridge.pointer_json(&pointer("down", 5)).unwrap();
        assert_eq!(bridge.camera, camera);
        assert!(bridge.canvas_pan.is_none());
        assert_eq!(bridge.captured_pointer, Some(23));
        assert_eq!(
            bridge.editor().editor().active_pointer_gesture(),
            Some(active)
        );
        assert_eq!(bridge.editor().editor().selection(), selection);
    }

    #[test]
    fn click_staged_segment_and_circle_commit_through_the_direct_bridge() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new"}"#)
            .unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"segment"}}"#)
            .unwrap();
        let revision_before = bridge.revision;
        let pointer = |phase: &str, pointer_id: u64, x: f64, y: f64, buttons: u16| {
            serde_json::json!({
                "version": 1,
                "phase": phase,
                "pointerId": pointer_id,
                "x": x,
                "y": y,
                "buttons": buttons,
                "modifiers": {
                    "alt": false,
                    "ctrl": false,
                    "meta": false,
                    "shift": false,
                },
            })
            .to_string()
        };

        for (pointer_id, x, y) in [(31, 339.079, 352.600), (31, 649.094, 467.400)] {
            bridge
                .pointer_json(&pointer("move", pointer_id, x, y, 0))
                .unwrap();
            bridge
                .pointer_json(&pointer("down", pointer_id, x, y, 1))
                .unwrap();
            bridge
                .pointer_json(&pointer("up", pointer_id, x, y, 0))
                .unwrap();
        }

        let document = bridge
            .editor()
            .coordinator()
            .presentation_session()
            .expect("direct bridge retains accepted authoring authority")
            .design_document();
        assert_eq!(document.points().len(), 2);
        assert_eq!(document.curves().len(), 1);
        assert_eq!(bridge.revision, revision_before + 1);
        assert_eq!(bridge.active_tool, "segment");
        assert_eq!(bridge.notice, "Geometry accepted");

        bridge
            .dispatch_json(
                r#"{"version":1,"command":"tool.select","payload":{"id":"center-radius-circle"}}"#,
            )
            .unwrap();
        for (pointer_id, x, y) in [(42, 470.0, 220.0), (42, 560.0, 220.0)] {
            bridge
                .pointer_json(&pointer("move", pointer_id, x, y, 0))
                .unwrap();
            bridge
                .pointer_json(&pointer("down", pointer_id, x, y, 1))
                .unwrap();
            bridge
                .pointer_json(&pointer("up", pointer_id, x, y, 0))
                .unwrap();
        }

        let document = bridge
            .editor()
            .coordinator()
            .presentation_session()
            .expect("second staged family retains accepted authoring authority")
            .design_document();
        assert_eq!(document.points().len(), 3);
        assert_eq!(document.curves().len(), 2);
        assert_eq!(bridge.revision, revision_before + 2);
        assert_eq!(bridge.active_tool, "center-radius-circle");
        assert_eq!(bridge.notice, "Geometry accepted");
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact bridge regression keeps empty-source insertion, compiler receipt, native validation, pointer availability, and Undo together"
    )]
    fn m90_f003_empty_managed_circle_imports_mm_and_publishes_once() {
        const EXPECTED_SOURCE: &str = r#""use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const geometry1 = $.geometry.centerRadiusCircle("geometry1", {
    center: [-0.6, 2.6],
    label: "center-radius-circle-0000000000000001",
    radius: mm(1.7999999999999998),
    role: "profile",
  });
  $.group("Canvas additions", [geometry1]);
  return {};
});
"#;

        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new-code"}"#)
            .expect("empty managed project opens");
        let source_before = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../geosolve-sketch-code/assets/demos/authored-empty.sketch.ts"
        ));
        let revision_before = bridge.revision;
        let identity_before = bridge
            .code_project
            .as_ref()
            .expect("new coded sketch has managed authority")
            .code_session_identity()
            .clone();
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source_before
        );
        let empty: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(empty["presentation"]["canUndo"], false);

        bridge
            .dispatch_json(
                r#"{"version":1,"command":"tool.select","payload":{"id":"center-radius-circle"}}"#,
            )
            .expect("center-radius circle tool activates");

        for (index, position) in [
            ScreenPoint { x: 470.0, y: 220.0 },
            ScreenPoint { x: 560.0, y: 220.0 },
        ]
        .into_iter()
        .enumerate()
        {
            bridge
                .pointer_json(&pointer_request("move", 90_003, position, 0))
                .expect("circle pointer move");
            bridge
                .pointer_json(&pointer_request("down", 90_003, position, 1))
                .expect("circle pointer down");
            if index == 0 {
                bridge
                    .pointer_json(&pointer_request("up", 90_003, position, 0))
                    .expect("first circle pointer up");
            }
        }

        assert!(
            bridge.pending_managed_mutation.is_some(),
            "the completed circle must await its authenticated compiler receipt"
        );
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source_before,
            "a prepared canvas insertion must not publish source early"
        );
        let pending: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(pending["presentation"]["canUndo"], false);

        let compiled = managed_empty_circle_fixture();
        assert_eq!(compiled.normalized_source, EXPECTED_SOURCE);
        let receipt = pending_managed_receipt(&bridge, compiled);
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("empty-circle compiler receipt resolves");

        assert!(
            bridge.pending_managed_mutation.is_none(),
            "compiler receipt did not publish: {:?}",
            bridge.last_error
        );
        assert!(bridge.last_error.is_none(), "{:?}", bridge.last_error);
        assert_eq!(bridge.revision, revision_before + 1);
        bridge
            .pointer_json(&pointer_request(
                "up",
                90_003,
                ScreenPoint { x: 560.0, y: 220.0 },
                0,
            ))
            .expect("the circle's terminal pointer-up is available after receipt publication");
        let code = bridge.code_project.as_ref().unwrap();
        assert_eq!(code.managed_source(), EXPECTED_SOURCE);
        assert_eq!(
            code.code_session_identity().revision,
            identity_before.revision + 1,
            "one circle publication must create exactly one code-history revision"
        );
        assert_eq!(
            code.managed_source()
                .matches("import { sketch, mm } from \"@geosolve/sketch-code\";")
                .count(),
            1,
            "the generated unit helper must be imported exactly once"
        );
        let project = accepted_compiled_project(&bridge);
        let sdk_import = project
            .managed
            .imports
            .iter()
            .find(|import| import.module == "@geosolve/sketch-code")
            .expect("published project retains its SDK import");
        assert_eq!(
            sdk_import
                .bindings
                .iter()
                .filter(|binding| binding.as_str() == "mm")
                .count(),
            1
        );
        let compiled = project
            .managed
            .compiled
            .as_deref()
            .expect("published circle retains compiled authority");
        assert!(compiled.artifact.value_consumers.iter().any(|consumer| {
            matches!(
                &consumer.target,
                geosolve_sketch_code::ExecutedConsumerTarget::Declaration {
                    declaration,
                    family,
                } if declaration == "geometry1" && family == "geometry.centerRadiusCircle"
            ) && consumer.property
                == [geosolve_sketch_code::ManagedPathSegment::Field(
                    "radius".into(),
                )]
        }));

        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("circle receipt publishes accepted native authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
        let document = accepted
            .session
            .accepted_state_for_current_input()
            .expect("circle has current accepted state")
            .document();
        assert_eq!(document.points().len(), 1);
        assert_eq!(document.curves().len(), 1);
        let CurveDefinition::Circle { center, radius } = document.curves()[0].definition else {
            panic!("empty managed circle fixture must materialize one circle")
        };
        assert!(
            document
                .point(center)
                .expect("circle center")
                .position
                .into_iter()
                .all(f64::is_finite)
        );
        let radius = document.scalar(radius).expect("circle radius").value;
        assert!(radius.is_finite() && radius > 0.0);
        assert_eq!(radius.to_bits(), 1.799_999_999_999_999_8_f64.to_bits());

        let published: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(published["presentation"]["canUndo"], true);
        assert_eq!(published["presentation"]["canRedo"], false);
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"select"}}"#)
            .expect("Select activates after circle publication");
        let follow_up = ScreenPoint { x: 620.0, y: 260.0 };
        for (phase, buttons) in [("move", 0), ("down", 1), ("up", 0)] {
            bridge
                .pointer_json(&pointer_request(phase, 90_004, follow_up, buttons))
                .expect("a complete pointer gesture is immediately available after publication");
        }
        assert!(bridge.pending_managed_mutation.is_none());

        let undone: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
                .expect("empty-circle publication Undo"),
        )
        .unwrap();
        assert_eq!(undone["presentation"]["canUndo"], false);
        assert_eq!(undone["presentation"]["canRedo"], true);
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source_before,
            "the sole source-history action must restore the exact empty starter"
        );
        assert!(bridge.pending_managed_mutation.is_none());
        let undone_document = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("Undo restores accepted empty authority")
            .session
            .accepted_state_for_current_input()
            .expect("Undo restores current empty state")
            .document();
        assert!(undone_document.points().is_empty());
        assert!(undone_document.curves().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact bridge regression keeps two inferred Point-on-Curve declarations, compiler publication, pointer recovery, and Undo together"
    )]
    fn m90_f004_two_circles_snapped_segment_publishes_one_managed_batch() {
        const START: [f64; 2] = [-1.800_000_871_932_123_7, 1.757_141_113_281_250_4];
        const END: [f64; 2] = [1.199_999_128_070_513_6, 1.757_141_113_281_25];

        let mut bridge = managed_two_circles_bridge();
        let source_before = bridge
            .code_project
            .as_ref()
            .expect("two-circle source authority")
            .managed_source()
            .to_owned();
        let revision_before = bridge.revision;
        let identity_before = bridge
            .code_project
            .as_ref()
            .expect("two-circle source authority")
            .code_session_identity()
            .clone();
        let scene = bridge.current_scene().expect("accepted two-circle scene");
        let start = scene.viewport.model_to_screen(START);
        let end = scene.viewport.model_to_screen(END);

        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"segment"}}"#)
            .expect("Segment activates");
        for (index, position) in [start, end].into_iter().enumerate() {
            bridge
                .pointer_json(&pointer_request("move", 90_004, position, 0))
                .expect("segment pointer move");
            bridge
                .pointer_json(&pointer_request("down", 90_004, position, 1))
                .expect("segment pointer down");
            if index == 0 {
                bridge
                    .pointer_json(&pointer_request("up", 90_004, position, 0))
                    .expect("first segment pointer up");
            }
        }

        assert!(bridge.pending_managed_mutation.is_some());
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source_before,
            "the line batch must not publish source before receipt authentication"
        );
        let Some(PendingManagedMutation::Managed { prepared, .. }) =
            bridge.pending_managed_mutation.as_ref()
        else {
            panic!("snapped Segment must prepare one managed compiler batch")
        };
        let ManagedSketchMutation::InsertDeclarations { declarations } =
            &prepared.request.ticket.mutation
        else {
            panic!("snapped Segment must insert declarations")
        };
        assert_eq!(
            declarations.len(),
            4,
            "one Segment, two Point-on-Curve relations and one Horizontal relation must compile together"
        );

        let compiled = managed_two_circles_fixture(true);
        let expected_source = compiled.normalized_source.clone();
        let receipt = pending_managed_receipt(&bridge, compiled);
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("snapped-segment compiler receipt resolves");

        assert!(
            bridge.pending_managed_mutation.is_none(),
            "compiler receipt did not publish: {:?}",
            bridge.last_error
        );
        assert!(bridge.last_error.is_none(), "{:?}", bridge.last_error);
        assert_eq!(bridge.revision, revision_before + 1);
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            expected_source
        );
        assert_eq!(
            bridge
                .code_project
                .as_ref()
                .unwrap()
                .code_session_identity()
                .revision,
            identity_before.revision + 1
        );
        bridge
            .pointer_json(&pointer_request("up", 90_004, end, 0))
            .expect("terminal pointer-up is immediately available");

        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("snapped Segment publishes accepted native authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
        let document = accepted
            .session
            .accepted_state_for_current_input()
            .expect("snapped Segment has current accepted state")
            .document();
        assert_eq!(document.curves().len(), 3);
        assert_eq!(document.contacts().len(), 2);
        assert_eq!(document.constraints().len(), 3);
        assert!(
            document
                .points()
                .iter()
                .flat_map(|point| point.position)
                .all(f64::is_finite)
        );
        assert!(
            document
                .scalars()
                .iter()
                .map(|scalar| scalar.value)
                .all(f64::is_finite)
        );

        let (line_start, line_end, line_branch) = document
            .curves()
            .iter()
            .find_map(|curve| match &curve.definition {
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction,
                } => Some((*start, *end, *branch_direction)),
                _ => None,
            })
            .expect("snapped Segment retains one explicit line branch");
        assert!(line_branch.into_iter().all(f64::is_finite));
        assert!(line_branch[0] > 0.0);
        assert!(line_branch[0].hypot(line_branch[1]) > 0.0);
        assert_eq!(
            document
                .point(line_start)
                .unwrap()
                .position
                .map(f64::to_bits),
            START.map(f64::to_bits)
        );
        assert_eq!(
            document.point(line_end).unwrap().position.map(f64::to_bits),
            END.map(f64::to_bits)
        );

        for contact in document.contacts() {
            let ContactDomain::Periodic { period } = contact.domain else {
                panic!("circle Point-on-Curve contacts must remain explicitly periodic")
            };
            assert_eq!(period.to_bits(), std::f64::consts::TAU.to_bits());
            assert_eq!(contact.neighborhood, ContactNeighborhood::Interior);
            assert_eq!(contact.winding, 0);
            assert_eq!(contact.tangent_orientation, None);
        }
        assert_eq!(
            document
                .contacts()
                .iter()
                .map(|contact| document.scalar(contact.parameter).unwrap().value.to_bits())
                .collect::<Vec<_>>(),
            vec![
                6.283_184_096_163_701_f64.to_bits(),
                3.141_593_864_604_212_f64.to_bits(),
            ]
        );
        assert_eq!(
            document
                .constraints()
                .iter()
                .filter(|constraint| matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::PointOnCurve { .. }
                ))
                .count(),
            2
        );
        assert_eq!(
            document
                .constraints()
                .iter()
                .filter(|constraint| matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::Horizontal { .. }
                ))
                .count(),
            1
        );

        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"select"}}"#)
            .expect("Select activates after snapped Segment publication");
        let follow_up = ScreenPoint { x: 640.0, y: 300.0 };
        for (phase, buttons) in [("move", 0), ("down", 1), ("up", 0)] {
            bridge
                .pointer_json(&pointer_request(phase, 90_005, follow_up, buttons))
                .expect("a complete pointer gesture is immediately available after publication");
        }
        assert!(bridge.pending_managed_mutation.is_none());

        let undone: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
                .expect("snapped Segment publication Undo"),
        )
        .unwrap();
        assert_eq!(undone["presentation"]["canUndo"], false);
        assert_eq!(undone["presentation"]["canRedo"], true);
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source_before
        );
        assert!(bridge.pending_managed_mutation.is_none());
        let undone_document = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("Undo restores accepted two-circle authority")
            .session
            .accepted_state_for_current_input()
            .expect("Undo restores current two-circle state")
            .document();
        assert_eq!(undone_document.curves().len(), 2);
        assert!(undone_document.contacts().is_empty());
        assert!(undone_document.constraints().is_empty());
    }

    #[test]
    fn m90_f005_supplied_native_workspace_constrained_drags_publish_and_undo() {
        let cases = [
            ("7b80e0003fe358f731b6e557427a066d", 90_005_001),
            ("7b80e0003fe358f731b6e557427a0670", 90_005_002),
            ("7b80e0003fe358f731b6e557427a0673", 90_005_003),
            ("7b80e0003fe358f731b6e557427a0674", 90_005_004),
            ("7b80e0003fe358f731b6e557427a067e", 90_005_005),
            ("7b80e0003fe358f731b6e557427a0685", 90_005_006),
            ("7b80e0003fe358f731b6e557427a0686", 90_005_007),
        ];
        let failures = cases
            .into_iter()
            .filter_map(|(point_id, pointer_id)| {
                std::panic::catch_unwind(|| {
                    m90_f005_drag_and_require_publication(point_id, pointer_id);
                })
                .err()
                .map(|panic| {
                    let detail = panic
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| panic.downcast_ref::<&str>().map(ToString::to_string))
                        .unwrap_or_else(|| "non-string panic".into());
                    format!("{point_id}: {detail}")
                })
            })
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "M90-F005 independent cases failed:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn m88_f004_finish_readiness_tracks_an_actionable_polyline_draft() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new"}"#)
            .unwrap();
        let selected: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(
                    r#"{"version":1,"command":"tool.select","payload":{"id":"polyline"}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(selected["presentation"]["canFinish"], false);
        let revision_before = bridge.revision;
        let project_before = bridge.export_project_json().unwrap();
        let premature: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"tool.finish"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(premature["presentation"]["canFinish"], false);
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(bridge.export_project_json().unwrap(), project_before);
        assert_eq!(bridge.active_tool, "polyline");
        assert!(bridge.editor().editor().geometry_draft_status().is_some());

        let pointer = |phase: &str, x: f64, y: f64, buttons: u16| {
            serde_json::json!({
                "version": 1,
                "phase": phase,
                "pointerId": 51,
                "x": x,
                "y": y,
                "buttons": buttons,
                "modifiers": {
                    "alt": false,
                    "ctrl": false,
                    "meta": false,
                    "shift": false,
                },
            })
            .to_string()
        };
        let click = |bridge: &mut WorkbenchBridge, x, y| -> serde_json::Value {
            bridge.pointer_json(&pointer("down", x, y, 1)).unwrap();
            serde_json::from_str(&bridge.pointer_json(&pointer("up", x, y, 0)).unwrap()).unwrap()
        };

        let first = click(&mut bridge, 360.0, 280.0);
        assert_eq!(first["presentation"]["canFinish"], false);
        assert_eq!(
            bridge
                .editor()
                .editor()
                .geometry_draft_status()
                .unwrap()
                .completed_stages,
            1
        );

        let second = click(&mut bridge, 560.0, 280.0);
        assert_eq!(second["presentation"]["canFinish"], true);
        assert!(bridge.editor().editor().can_complete_draft());

        let finished: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"tool.finish"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(finished["presentation"]["canFinish"], false);
        let document = bridge
            .editor()
            .coordinator()
            .presentation_session()
            .expect("finished polyline retains accepted authoring authority")
            .design_document();
        assert_eq!(document.curves().len(), 1);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one lifecycle regression keeps authenticated Profile Offset closure presentation, mutation, history, persistence, and headless authority together"
    )]
    fn managed_profile_offset_closure_is_nested_and_mutates_as_one_source_block() {
        let mut bridge = managed_profile_offset_closure_bridge();
        let base_project = canonical_project_json(&bridge);
        assert_eq!(
            declaration_order(&bridge),
            ["base", "guide", "offsetChain11", "profileOffset12"]
        );

        let base_snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let root = nested_explorer_row(&base_snapshot, "profileOffset12");
        let helper = nested_explorer_row(&base_snapshot, "offsetChain11");
        assert_eq!(root["rowKind"], "declaration");
        assert_eq!(helper["rowKind"], "declaration");
        assert!(
            root["children"]
                .as_array()
                .is_some_and(|children| children.iter().any(|row| row["label"] == "offsetChain11"))
        );
        for capability in ["select", "navigate", "edit"] {
            assert_eq!(helper["capabilities"][capability]["enabled"], true);
        }
        for capability in ["move", "moveUp", "moveDown", "suppress", "delete"] {
            assert_eq!(helper["capabilities"][capability]["enabled"], false);
            assert!(
                helper["capabilities"][capability]["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains("Profile Offset"))
            );
        }

        let helper_id = helper["id"].as_str().unwrap();
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.select",
                    "payload": { "id": helper_id },
                })
                .to_string(),
            )
            .expect("private helper remains independently selectable");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.source.open",
                    "payload": {
                        "id": helper_id,
                        "from": helper["source"]["from"],
                        "to": helper["source"]["to"],
                    },
                })
                .to_string(),
            )
            .expect("private helper retains its exact source navigation span");

        let revision = bridge.revision;
        for command in [
            serde_json::json!({
                "version": 1,
                "command": "declaration.move",
                "payload": { "id": helper_id, "direction": "up" },
            }),
            serde_json::json!({
                "version": 1,
                "command": "declaration.suppression.set",
                "payload": { "id": helper_id, "suppressed": true },
            }),
            serde_json::json!({
                "version": 1,
                "command": "declaration.delete",
                "payload": { "id": helper_id },
            }),
        ] {
            assert!(
                bridge
                    .dispatch_json(&command.to_string())
                    .unwrap_err()
                    .contains("Profile Offset")
            );
            assert!(bridge.pending_managed_mutation.is_none());
            assert_eq!(bridge.revision, revision);
            assert_eq!(canonical_project_json(&bridge), base_project);
        }

        let root_id = root["id"].as_str().unwrap();
        let guide_id = managed_row_id(&base_snapshot, "guide");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.move",
                    "payload": {
                        "id": root_id,
                        "targetId": guide_id,
                        "position": "before",
                    },
                })
                .to_string(),
            )
            .expect("Profile Offset closure reorder prepares");
        let receipt =
            pending_managed_receipt(&bridge, managed_profile_offset_closure_fixture("reordered"));
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("Profile Offset closure reorder publishes");
        assert!(
            bridge.last_error.is_none(),
            "Profile Offset closure reorder failed: {:?}",
            bridge.last_error,
        );
        assert_eq!(
            declaration_order(&bridge),
            ["base", "offsetChain11", "profileOffset12", "guide"]
        );

        // A destination root resolves to the start of its closure, not the
        // root declaration after its private helper.
        let reordered_snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let root_id = nested_explorer_row(&reordered_snapshot, "profileOffset12")["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let guide_id = managed_row_id(&reordered_snapshot, "guide");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.move",
                    "payload": {
                        "id": guide_id,
                        "targetId": root_id,
                        "position": "before",
                    },
                })
                .to_string(),
            )
            .expect("independent declaration move before closure prepares");
        let receipt = pending_managed_receipt(
            &bridge,
            managed_profile_offset_closure_fixture("restored-order"),
        );
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("independent declaration lands before earliest closure member");
        assert!(
            bridge.last_error.is_none(),
            "move-before-closure receipt failed: {:?}",
            bridge.last_error
        );
        assert_eq!(
            declaration_order(&bridge),
            ["base", "guide", "offsetChain11", "profileOffset12"]
        );

        // Reorder once more, persist/reload, and require IR-derived nesting to
        // survive without a presentation overlay.
        let snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let root_id = nested_explorer_row(&snapshot, "profileOffset12")["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let guide_id = managed_row_id(&snapshot, "guide");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.move",
                    "payload": {
                        "id": root_id,
                        "targetId": guide_id,
                        "position": "before",
                    },
                })
                .to_string(),
            )
            .unwrap();
        let receipt =
            pending_managed_receipt(&bridge, managed_profile_offset_closure_fixture("reordered"));
        bridge.resolve_pending_managed_mutation(receipt).unwrap();
        let reordered_project = canonical_project_json(&bridge);
        let persistence: serde_json::Value =
            serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
        let mut restored = WorkbenchBridge::construct_json(
            &serde_json::json!({
                "version": 1,
                "persistedProject": persistence["contents"],
            })
            .to_string(),
        )
        .expect("Profile Offset closure persistence reload");
        assert_eq!(canonical_project_json(&restored), reordered_project);
        let restored_snapshot: serde_json::Value =
            serde_json::from_str(&restored.snapshot_json().unwrap()).unwrap();
        assert!(
            nested_explorer_row(&restored_snapshot, "profileOffset12")["children"]
                .as_array()
                .is_some_and(|children| children.iter().any(|row| row["label"] == "offsetChain11"))
        );

        let root_id = nested_explorer_row(&restored_snapshot, "profileOffset12")["id"]
            .as_str()
            .unwrap()
            .to_owned();
        restored
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.delete",
                    "payload": { "id": root_id },
                })
                .to_string(),
            )
            .expect("root closure deletion prepares");
        let receipt =
            pending_managed_receipt(&restored, managed_profile_offset_closure_fixture("deleted"));
        restored
            .resolve_pending_managed_mutation(receipt)
            .expect("root closure deletion publishes atomically");
        assert_eq!(declaration_order(&restored), ["base", "guide"]);
        let deleted_project = canonical_project_json(&restored);

        restored
            .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
            .expect("closure delete Undo");
        assert_eq!(canonical_project_json(&restored), reordered_project);
        assert_eq!(
            declaration_order(&restored),
            ["base", "offsetChain11", "profileOffset12", "guide"]
        );
        restored
            .dispatch_json(r#"{"version":1,"command":"history.redo"}"#)
            .expect("closure delete Redo");
        assert_eq!(canonical_project_json(&restored), deleted_project);

        #[cfg(not(target_arch = "wasm32"))]
        {
            for project in [reordered_project, deleted_project] {
                let inspection = geosolve_headless::inspect(
                    &geosolve_headless::HeadlessInput::CodeProjectJson(project),
                )
                .expect("cold headless Profile Offset closure inspection");
                assert!(inspection.report.validation.hard_residuals_validated);
                assert!(inspection.report.validation.all_active_features_current);
                assert!(
                    inspection
                        .report
                        .validation
                        .maximum_normalized_hard_residual
                        .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
                );
            }
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one bridge-boundary regression proves the complete prepared mutation publication invariant"
    )]
    fn prepared_managed_mutation_is_atomic_recoverable_and_single_history() {
        let mut aborted = managed_bridge();
        let before_snapshot: serde_json::Value =
            serde_json::from_str(&aborted.snapshot_json().unwrap()).unwrap();
        let before_revision = aborted.revision;
        let before_project = aborted.export_project_json().unwrap();
        let before_persistence = aborted.persistence_json().unwrap();
        let before_identity = aborted
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity()
            .clone();
        let before_checkpoint = aborted
            .code_project
            .as_ref()
            .unwrap()
            .accepted_editor_checkpoint()
            .clone();
        prepare_radius_restore(&mut aborted);

        let pending_snapshot: serde_json::Value =
            serde_json::from_str(&aborted.snapshot_json().unwrap()).unwrap();
        assert_eq!(pending_snapshot["revision"], before_snapshot["revision"]);
        assert_eq!(pending_snapshot["source"], before_snapshot["source"]);
        assert_eq!(pending_snapshot["frame"], before_snapshot["frame"]);
        assert_eq!(
            pending_snapshot["presentation"]["canUndo"],
            before_snapshot["presentation"]["canUndo"]
        );
        assert_eq!(aborted.export_project_json().unwrap(), before_project);
        assert_eq!(aborted.persistence_json().unwrap(), before_persistence);
        assert_eq!(
            aborted
                .code_project
                .as_ref()
                .unwrap()
                .code_session_identity(),
            &before_identity
        );
        let history_response = aborted.apply_code_control_rpc_json(
            &serde_json::json!({
                "method": "undo",
                "expected": before_identity.clone(),
            })
            .to_string(),
        );
        let history_response: serde_json::Value = serde_json::from_str(&history_response).unwrap();
        assert_eq!(history_response["outcome"], "failure");
        assert_eq!(
            history_response["failure"]["code"],
            "managed_mutation_pending"
        );
        assert!(aborted.pending_managed_mutation.is_some());
        assert_eq!(aborted.revision, before_revision);

        let accepted_source = aborted
            .code_project
            .as_ref()
            .unwrap()
            .managed_source()
            .to_owned();
        let candidate_source = accepted_source.replacen(
            "export default sketch",
            "const 多字节 = ;\nexport default sketch",
            1,
        );
        let diagnostic_start = candidate_source.find("= ;").unwrap() + 2;
        assert!(
            aborted
                .abort_pending_managed_mutation(&ManagedMutationAbortPayload {
                    ticket_digest: "0".repeat(64),
                    candidate_source: candidate_source.clone(),
                    diagnostic: "wrong compiler ticket".into(),
                    span: ManagedSpan::new(diagnostic_start, diagnostic_start + 1),
                })
                .is_err()
        );
        assert!(aborted.pending_managed_mutation.is_some());
        let PendingManagedMutation::Managed { prepared, .. } = aborted
            .pending_managed_mutation
            .as_ref()
            .expect("pending managed mutation")
        else {
            panic!("expected structured managed mutation")
        };
        let ticket_digest = prepared.request.ticket.ticket_digest.clone();
        aborted
            .abort_pending_managed_mutation(&ManagedMutationAbortPayload {
                ticket_digest,
                candidate_source: candidate_source.clone(),
                diagnostic: "compiler host refused the candidate".into(),
                span: ManagedSpan::new(diagnostic_start, diagnostic_start + 1),
            })
            .unwrap();
        assert!(aborted.pending_managed_mutation.is_none());
        assert_eq!(aborted.revision, before_revision);
        assert!(aborted.export_project_json().is_err());
        assert_ne!(aborted.persistence_json().unwrap(), before_persistence);
        assert_eq!(
            aborted
                .code_project
                .as_ref()
                .unwrap()
                .code_session_identity(),
            &before_identity
        );
        assert_eq!(
            aborted.code_project.as_ref().unwrap().managed_source(),
            accepted_source,
            "compiler failure must not replace accepted source authority"
        );
        let failed_snapshot: serde_json::Value =
            serde_json::from_str(&aborted.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            failed_snapshot["source"]["files"][0]["contents"],
            candidate_source
        );
        assert_eq!(failed_snapshot["source"]["dirty"], true);
        assert_eq!(failed_snapshot["frame"], before_snapshot["frame"]);
        assert_eq!(
            failed_snapshot["presentation"]["canUndo"],
            before_snapshot["presentation"]["canUndo"]
        );
        assert_eq!(failed_snapshot["problems"][0]["file"], "sketch.ts");
        assert_eq!(failed_snapshot["problems"][0]["line"], 4);
        assert_eq!(failed_snapshot["problems"][0]["column"], 13);
        assert_eq!(
            failed_snapshot["problems"][0]["detail"],
            "compiler host refused the candidate"
        );
        let persisted: serde_json::Value =
            serde_json::from_str(&aborted.persistence_json().unwrap()).unwrap();
        let restore_request = serde_json::json!({
            "version": 1,
            "persistedProject": persisted["contents"].as_str().unwrap(),
        });
        let mut restored = WorkbenchBridge::construct_json(&restore_request.to_string()).unwrap();
        let restored_snapshot: serde_json::Value =
            serde_json::from_str(&restored.snapshot_json().unwrap()).unwrap();
        assert_eq!(restored_snapshot["source"], failed_snapshot["source"]);
        assert_eq!(restored_snapshot["problems"], failed_snapshot["problems"]);
        assert!(restored.export_project_json().is_err());
        assert_eq!(
            restored
                .code_project
                .as_ref()
                .unwrap()
                .code_session_identity(),
            &before_identity,
        );
        assert_eq!(
            restored
                .code_project
                .as_ref()
                .unwrap()
                .accepted_editor_checkpoint(),
            &before_checkpoint,
        );

        let mut resolved = managed_bridge();
        let accepted_snapshot: serde_json::Value =
            serde_json::from_str(&resolved.snapshot_json().unwrap()).unwrap();
        let accepted_revision = resolved.revision;
        let accepted_project = resolved.export_project_json().unwrap();
        let accepted_persistence = resolved.persistence_json().unwrap();
        let accepted_identity = resolved
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity()
            .clone();
        prepare_radius_restore(&mut resolved);
        let valid = pending_managed_receipt(&resolved, managed_restored_fixture());
        let mut stale = valid.clone();
        stale.ticket_digest = "f".repeat(64);
        resolved.resolve_pending_managed_mutation(stale).unwrap();
        assert!(resolved.pending_managed_mutation.is_some());
        assert_eq!(resolved.revision, accepted_revision);
        assert_eq!(resolved.export_project_json().unwrap(), accepted_project);
        assert_eq!(resolved.persistence_json().unwrap(), accepted_persistence);
        let retained_snapshot: serde_json::Value =
            serde_json::from_str(&resolved.snapshot_json().unwrap()).unwrap();
        assert_eq!(retained_snapshot["source"], accepted_snapshot["source"]);
        assert_eq!(retained_snapshot["frame"], accepted_snapshot["frame"]);
        assert_eq!(
            retained_snapshot["presentation"]["canUndo"],
            accepted_snapshot["presentation"]["canUndo"]
        );

        let expected_source = valid.compiled.normalized_source.clone();
        resolved.resolve_pending_managed_mutation(valid).unwrap();
        assert!(resolved.pending_managed_mutation.is_none());
        assert_eq!(resolved.revision, accepted_revision + 1);
        assert_ne!(resolved.export_project_json().unwrap(), accepted_project);
        assert_ne!(resolved.persistence_json().unwrap(), accepted_persistence);
        let identity = resolved
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity();
        assert_eq!(identity.revision, accepted_identity.revision + 1);
        let published: serde_json::Value =
            serde_json::from_str(&resolved.snapshot_json().unwrap()).unwrap();
        assert_eq!(published["source"]["files"][0]["contents"], expected_source);
        assert_eq!(published["presentation"]["canUndo"], true);
        assert_eq!(published["presentation"]["canRedo"], false);
        let accepted = resolved
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("resolved compiler receipt has accepted native authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
        assert!(
            accepted
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .points()
                .iter()
                .flat_map(|point| point.position)
                .all(f64::is_finite)
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one crossed-boundary regression keeps selected reference detachment, terminal parity, atomic publication, and history together"
    )]
    fn managed_selected_reference_point_drag_publishes_instance_overlay_without_source_edit() {
        let mut bridge = managed_bridge();
        let opened: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let segment_row = managed_row_id(&opened, "segment");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.select",
                    "payload": { "id": segment_row },
                })
                .to_string(),
            )
            .unwrap();

        let scene = bridge.current_scene().expect("accepted managed scene");
        let origin_model = [1.25_f64, -6.5_f64];
        let origin = scene
            .points
            .iter()
            .find(|point| point.model_position.map(f64::to_bits) == origin_model.map(f64::to_bits))
            .expect("referenced point shared by the producer and selected consumer")
            .screen_position;
        let target = scene.viewport.model_to_screen([-2.0, 4.0]);
        let target_model = scene.viewport.screen_to_model(target);
        let detached_model = [-1.999_999_999_999_999_3_f64, 4.0_f64];
        let revision_before = bridge.revision;
        let identity_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity()
            .clone();
        let source_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_source()
            .to_owned();
        let persistence_before = bridge.persistence_json().unwrap();
        let project_before = canonical_project_json(&bridge);

        bridge
            .pointer_json(&pointer_request("down", 89_001, origin, 1))
            .unwrap();
        assert_eq!(
            bridge.captured_pointer,
            Some(89_001),
            "point press did not capture: error={:?} selection={:?} prepared={:?}",
            bridge.last_error,
            bridge.editor().editor().selection(),
            bridge.editor().editor().prepared_point_drag_route(),
        );
        assert!(
            bridge
                .code_project
                .as_ref()
                .unwrap()
                .has_pending_semantic_point_drag(89_001)
        );
        bridge
            .pointer_json(&pointer_request("move", 89_001, target, 1))
            .unwrap();
        bridge
            .pointer_json(&pointer_request("up", 89_001, target, 0))
            .unwrap();

        assert!(
            bridge.pending_managed_mutation.is_none(),
            "solver-instance detachment unexpectedly requested compilation: {:?}",
            bridge.last_error
        );
        assert_eq!(bridge.revision, revision_before + 1);
        let code = bridge.code_project.as_ref().unwrap();
        assert_eq!(
            code.code_session_identity().revision,
            identity_before.revision + 1
        );
        assert_eq!(code.managed_source(), source_before);
        assert_eq!(canonical_project_json(&bridge), project_before);
        assert_ne!(bridge.persistence_json().unwrap(), persistence_before);
        assert!(!code.managed_test_overlay_is_empty());
        assert_eq!(
            code.selected_managed_declaration(bridge.editor()).unwrap(),
            Some(geosolve_sketch_code::SemanticSymbol("segment".into()))
        );
        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("detached source candidate is independently accepted");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );
        let positions = accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .points()
            .iter()
            .map(|point| point.position.map(f64::to_bits))
            .collect::<Vec<_>>();
        assert!(positions.contains(&origin_model.map(f64::to_bits)));
        assert!(
            positions.contains(&detached_model.map(f64::to_bits)),
            "accepted positions {positions:?} do not contain detached instance terminal {detached_model:?}"
        );
        assert!((detached_model[0] - target_model[0]).abs() <= 2.0 * f64::EPSILON);

        bridge
            .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
            .expect("detached instance overlay Undo");
        assert!(
            bridge
                .code_project
                .as_ref()
                .unwrap()
                .managed_test_overlay_is_empty()
        );
        assert_eq!(canonical_project_json(&bridge), project_before);
        bridge
            .dispatch_json(r#"{"version":1,"command":"history.redo"}"#)
            .expect("detached instance overlay Redo");
        assert!(
            !bridge
                .code_project
                .as_ref()
                .unwrap()
                .managed_test_overlay_is_empty()
        );
        assert_eq!(canonical_project_json(&bridge), project_before);

        let persistence: serde_json::Value =
            serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
        let request = serde_json::json!({
            "version": 1,
            "persistedProject": persistence["contents"].as_str().unwrap(),
        });
        let restored = WorkbenchBridge::construct_json(&request.to_string())
            .expect("detached instance overlay restores from persistence");
        assert_eq!(canonical_project_json(&restored), project_before);
        assert!(
            !restored
                .code_project
                .as_ref()
                .unwrap()
                .managed_test_overlay_is_empty()
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one forced terminal rejection proves semantic-token, accepted-scene, selection, history, and follow-up gesture recovery together"
    )]
    fn managed_selected_reference_rejected_terminal_restores_exact_authority_and_next_gesture() {
        let mut bridge = managed_bridge();
        let opened: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let segment_row = managed_row_id(&opened, "segment");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.select",
                    "payload": { "id": segment_row },
                })
                .to_string(),
            )
            .unwrap();

        let scene_before = bridge.current_scene().expect("accepted managed scene");
        let document_before = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("accepted managed materialization")
            .session
            .accepted_state_for_current_input()
            .expect("accepted managed document")
            .document()
            .clone();
        let selection_before = bridge.editor().editor().selection().to_vec();
        let origin_model = [1.25_f64, -6.5_f64];
        let origin = scene_before
            .points
            .iter()
            .find(|point| point.model_position.map(f64::to_bits) == origin_model.map(f64::to_bits))
            .expect("referenced point shared by producer and selected consumer")
            .screen_position;
        let target = scene_before.viewport.model_to_screen([-2.0, 4.0]);
        let revision_before = bridge.revision;
        let identity_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity()
            .clone();
        let source_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_source()
            .to_owned();
        let persistence_before = bridge.persistence_json().unwrap();
        let project_before = canonical_project_json(&bridge);

        bridge
            .pointer_json(&pointer_request("down", 90_101, origin, 1))
            .expect("selected reference press prepares detachment");
        bridge
            .pointer_json(&pointer_request("move", 90_101, target, 1))
            .expect("selected reference has an accepted detached preview");
        let terminal_scene = bridge
            .current_scene()
            .expect("selected reference terminal preview scene");
        let outcome = bridge
            .editor_mut()
            .pointer_up_delegated_point(
                &terminal_scene,
                PointerInput {
                    pointer_id: 90_101,
                    position: target,
                    modifiers: Modifiers::default(),
                },
            )
            .expect("native delegated terminal proposal");
        let mut proposal = outcome.proposal.expect("accepted delegated point proposal");
        proposal.accepted_position[0] += 1.0;
        let error = bridge
            .publish_delegated_point_terminal(90_101, &proposal)
            .expect_err("tampered terminal parity must reject");
        assert!(
            error.contains("disagrees with its accepted native position"),
            "unexpected primary terminal error: {error}"
        );

        assert!(bridge.pending_managed_mutation.is_none());
        assert_eq!(bridge.captured_pointer, None);
        assert_eq!(bridge.revision, revision_before);
        let code = bridge.code_project.as_ref().unwrap();
        assert!(!code.has_any_pending_semantic_point_drag());
        assert!(code.managed_test_overlay_is_empty());
        assert_eq!(code.code_session_identity(), &identity_before);
        assert_eq!(code.managed_source(), source_before);
        assert_eq!(bridge.persistence_json().unwrap(), persistence_before);
        assert_eq!(canonical_project_json(&bridge), project_before);
        assert_eq!(bridge.editor().editor().selection(), selection_before);
        assert_eq!(
            code.selected_managed_declaration(bridge.editor()).unwrap(),
            Some(geosolve_sketch_code::SemanticSymbol("segment".into()))
        );
        let restored_scene = bridge.current_scene().expect("restored accepted scene");
        assert_eq!(
            restored_scene.accepted_revision,
            scene_before.accepted_revision
        );
        assert_eq!(restored_scene.design_identity, scene_before.design_identity);
        assert_eq!(restored_scene.viewport, scene_before.viewport);
        assert_eq!(restored_scene.points, scene_before.points);
        assert_eq!(restored_scene.curves, scene_before.curves);
        assert_eq!(restored_scene.datums, scene_before.datums);
        assert_eq!(restored_scene.computed_curves, scene_before.computed_curves);
        assert_eq!(restored_scene.curve_controls, scene_before.curve_controls);
        assert_eq!(restored_scene.annotations, scene_before.annotations);
        let restored_document = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("restored accepted materialization")
            .session
            .accepted_state_for_current_input()
            .expect("restored accepted document")
            .document();
        assert_eq!(restored_document, &document_before);

        let restored_scene = bridge.current_scene().expect("follow-up accepted scene");
        let restored_origin = restored_scene
            .points
            .iter()
            .find(|point| point.model_position.map(f64::to_bits) == origin_model.map(f64::to_bits))
            .expect("restored referenced point")
            .screen_position;
        let follow_up_target = restored_scene.viewport.model_to_screen([-3.0, 3.0]);
        bridge
            .pointer_json(&pointer_request("down", 90_102, restored_origin, 1))
            .expect("next selected reference press");
        bridge
            .pointer_json(&pointer_request("move", 90_102, follow_up_target, 1))
            .expect("next selected reference preview");
        bridge
            .pointer_json(&pointer_request("up", 90_102, follow_up_target, 0))
            .expect("next selected reference terminal");
        assert_eq!(bridge.revision, revision_before + 1);
        assert!(bridge.pending_managed_mutation.is_none());
        assert!(
            !bridge
                .code_project
                .as_ref()
                .unwrap()
                .has_any_pending_semantic_point_drag()
        );
        assert!(bridge.last_error.is_none(), "{:?}", bridge.last_error);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the exact user reproduction crosses native terminal parity, source isolation, follow-up input, history, and persistence"
    )]
    fn compass_rose_point_drag_publishes_one_instance_overlay_without_compilation() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"compass-rose"}}"#,
            )
            .expect("Compass Rose opens through its checked-in managed project");
        let scene = bridge.current_scene().expect("accepted Compass Rose scene");
        let expected_points = [
            [0.0_f64, 0.0_f64],
            [0.0, 34.0],
            [34.0, 0.0],
            [0.0, -34.0],
            [-34.0, 0.0],
        ];
        let tracked = expected_points
            .into_iter()
            .map(|position| {
                scene
                    .points
                    .iter()
                    .find(|point| {
                        point.model_position.map(f64::to_bits) == position.map(f64::to_bits)
                    })
                    .unwrap_or_else(|| panic!("Compass Rose point {position:?}"))
                    .id
            })
            .collect::<Vec<_>>();
        assert_eq!(
            tracked
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            tracked.len(),
            "the five native Compass Rose points must remain distinct",
        );
        let center = scene
            .points
            .iter()
            .find(|point| point.id == tracked[0])
            .unwrap()
            .screen_position;
        let target = ScreenPoint {
            x: center.x + 30.0,
            y: center.y + 20.0,
        };
        let revision_before = bridge.revision;
        let session_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity()
            .clone();
        let source_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_source()
            .to_owned();
        let project_before = accepted_compiled_project(&bridge);

        bridge
            .pointer_json(&pointer_request("down", 90_001, center, 1))
            .expect("Compass Rose centre press");
        bridge
            .pointer_json(&pointer_request("move", 90_001, target, 1))
            .expect("Compass Rose native drag preview");
        let terminal_scene = bridge
            .current_scene()
            .expect("Compass Rose terminal preview scene");
        let terminal_positions = tracked
            .iter()
            .map(|id| {
                terminal_scene
                    .points
                    .iter()
                    .find(|point| point.id == *id)
                    .expect("tracked point remains in terminal preview")
                    .model_position
                    .map(f64::to_bits)
            })
            .collect::<Vec<_>>();
        bridge
            .pointer_json(&pointer_request("up", 90_001, target, 0))
            .expect("Compass Rose native terminal");

        assert!(
            bridge.pending_managed_mutation.is_none(),
            "a solver-instance point drag must not prepare managed-source compilation",
        );
        assert_eq!(bridge.revision, revision_before + 1);
        let code = bridge.code_project.as_ref().unwrap();
        assert_eq!(
            code.code_session_identity().revision,
            session_before.revision + 1
        );
        assert_eq!(code.managed_source(), source_before);
        assert!(!code.managed_test_overlay_is_empty());
        assert_eq!(accepted_compiled_project(&bridge), project_before);
        let snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert!(snapshot.get("pendingManagedMutation").is_none());
        assert_eq!(snapshot["presentation"]["canUndo"], true);
        assert_eq!(snapshot["presentation"]["canRedo"], false);

        let published_scene = bridge
            .current_scene()
            .expect("published Compass Rose scene");
        let published_positions = tracked
            .iter()
            .map(|id| {
                published_scene
                    .points
                    .iter()
                    .find(|point| point.id == *id)
                    .expect("tracked point remains in published scene")
                    .model_position
                    .map(f64::to_bits)
            })
            .collect::<Vec<_>>();
        assert_eq!(published_positions, terminal_positions);
        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("published Compass Rose native authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
        assert!(
            accepted
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .points()
                .iter()
                .flat_map(|point| point.position)
                .all(f64::is_finite)
        );

        bridge
            .pointer_json(&pointer_request("move", 90_002, target, 0))
            .expect("hover remains available immediately after point publication");
        bridge
            .wheel_json(
                &serde_json::json!({
                    "version": 1,
                    "x": target.x,
                    "y": target.y,
                    "deltaX": 0.0,
                    "deltaY": -12.0,
                    "ctrl": false,
                })
                .to_string(),
            )
            .expect("wheel remains available immediately after point publication");
        bridge
            .cancel_json(r#"{"version":1,"reason":"escape"}"#)
            .expect("cancel remains available immediately after point publication");

        bridge
            .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
            .expect("Compass Rose point overlay Undo");
        assert!(
            bridge
                .code_project
                .as_ref()
                .unwrap()
                .managed_test_overlay_is_empty()
        );
        let undone_scene = bridge.current_scene().expect("undone Compass Rose scene");
        for (id, expected) in tracked.iter().zip(expected_points) {
            let position = undone_scene
                .points
                .iter()
                .find(|point| point.id == *id)
                .expect("tracked point remains after Undo")
                .model_position;
            assert_eq!(position.map(f64::to_bits), expected.map(f64::to_bits));
        }
        assert_eq!(accepted_compiled_project(&bridge), project_before);

        bridge
            .dispatch_json(r#"{"version":1,"command":"history.redo"}"#)
            .expect("Compass Rose point overlay Redo");
        assert!(
            !bridge
                .code_project
                .as_ref()
                .unwrap()
                .managed_test_overlay_is_empty()
        );
        let redone_scene = bridge.current_scene().expect("redone Compass Rose scene");
        let redone_positions = tracked
            .iter()
            .map(|id| {
                redone_scene
                    .points
                    .iter()
                    .find(|point| point.id == *id)
                    .expect("tracked point remains after Redo")
                    .model_position
                    .map(f64::to_bits)
            })
            .collect::<Vec<_>>();
        assert_eq!(redone_positions, terminal_positions);
        assert_eq!(accepted_compiled_project(&bridge), project_before);

        let persistence: serde_json::Value =
            serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
        let request = serde_json::json!({
            "version": 1,
            "persistedProject": persistence["contents"].as_str().unwrap(),
        });
        let mut restored = WorkbenchBridge::construct_json(&request.to_string())
            .expect("Compass Rose point overlay restores from persistence");
        assert!(restored.pending_managed_mutation.is_none());
        assert_eq!(accepted_compiled_project(&restored), project_before);
        let restored_scene = restored
            .current_scene()
            .expect("restored Compass Rose scene");
        let restored_positions = tracked
            .iter()
            .map(|id| {
                restored_scene
                    .points
                    .iter()
                    .find(|point| point.id == *id)
                    .expect("tracked point remains after persistence replay")
                    .model_position
                    .map(f64::to_bits)
            })
            .collect::<Vec<_>>();
        assert_eq!(restored_positions, terminal_positions);
    }

    #[test]
    fn compass_rose_outer_endpoint_drag_retains_exact_native_terminal_without_compilation() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"compass-rose"}}"#,
            )
            .expect("Compass Rose opens through its checked-in managed project");
        let scene = bridge.current_scene().expect("accepted Compass Rose scene");
        let north = scene
            .points
            .iter()
            .find(|point| {
                point.model_position.map(f64::to_bits) == [0.0_f64, 34.0_f64].map(f64::to_bits)
            })
            .expect("Compass Rose north endpoint");
        let north_id = north.id;
        let origin = north.screen_position;
        let target = ScreenPoint {
            x: origin.x + 24.0,
            y: origin.y - 18.0,
        };
        let source_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_source()
            .to_owned();
        let project_before = accepted_compiled_project(&bridge);

        bridge
            .pointer_json(&pointer_request("down", 90_003, origin, 1))
            .expect("Compass Rose endpoint press");
        bridge
            .pointer_json(&pointer_request("move", 90_003, target, 1))
            .expect("Compass Rose endpoint native preview");
        let terminal = bridge
            .current_scene()
            .unwrap()
            .points
            .iter()
            .find(|point| point.id == north_id)
            .expect("north endpoint remains in terminal preview")
            .model_position
            .map(f64::to_bits);
        bridge
            .pointer_json(&pointer_request("up", 90_003, target, 0))
            .expect("Compass Rose endpoint terminal");

        assert!(bridge.pending_managed_mutation.is_none());
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source_before
        );
        assert_eq!(accepted_compiled_project(&bridge), project_before);
        assert!(
            !bridge
                .code_project
                .as_ref()
                .unwrap()
                .managed_test_overlay_is_empty()
        );
        let published = bridge
            .current_scene()
            .unwrap()
            .points
            .iter()
            .find(|point| point.id == north_id)
            .expect("north endpoint remains after publication")
            .model_position
            .map(f64::to_bits);
        assert_eq!(published, terminal);
        let validation = &bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("accepted Compass Rose endpoint publication")
            .validation;
        assert!(validation.hard_residuals_validated);
        assert!(validation.all_active_features_current);
        assert!(
            validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one crossed-boundary regression keeps two-arc Fillet fan-out, terminal parity, selection restoration, and atomic history together"
    )]
    fn managed_fillet_radius_drag_fans_out_and_publishes_one_source_history() {
        let mut bridge = managed_direct_fillet_bridge();
        let opened: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let round_row = managed_row_id(&opened, "round");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.select",
                    "payload": { "id": round_row },
                })
                .to_string(),
            )
            .unwrap();
        let selected: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            selected["selection"]["ownership"], "Modifiable in source",
            "a direct Fillet with several editable source leaves is still source-owned",
        );
        assert_eq!(selected["selection"]["source"]["path"], "sketch.ts");
        let source_start = usize::try_from(
            selected["selection"]["source"]["from"]
                .as_u64()
                .expect("selected Fillet source start"),
        )
        .expect("selected Fillet source start fits usize");
        let source_end = usize::try_from(
            selected["selection"]["source"]["to"]
                .as_u64()
                .expect("selected Fillet source end"),
        )
        .expect("selected Fillet source end fits usize");
        let expected_span = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_declaration_source_span(&geosolve_sketch_code::SemanticSymbol("round".into()))
            .expect("authenticated direct-Fillet declaration span");
        assert_eq!(
            (source_start, source_end),
            (expected_span.start, expected_span.end),
            "selection-level navigation must use the whole authenticated declaration",
        );
        let selected_source =
            &bridge.code_project.as_ref().unwrap().managed_source()[source_start..source_end];
        assert!(
            selected_source.contains("$.computed.filletSet(\"round\""),
            "selected declaration does not contain the semantic direct-Fillet call: {selected_source}",
        );
        let scene = bridge
            .current_scene()
            .expect("selected direct-Fillet scene");
        assert_eq!(scene.computed_curves.len(), 2);
        assert_eq!(scene.fillet_affordances.len(), 2);
        let rail = scene.fillet_affordances[0].radius_rail;
        let origin_model = scene.viewport.screen_to_model(rail.screen_grip);
        let target = scene.viewport.model_to_screen([
            0.25_f64.mul_add(rail.model_derivative[0], origin_model[0]),
            0.25_f64.mul_add(rail.model_derivative[1], origin_model[1]),
        ]);
        let revision_before = bridge.revision;
        let identity_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .code_session_identity()
            .clone();
        let source_before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_source()
            .to_owned();
        let persistence_before = bridge.persistence_json().unwrap();

        bridge
            .pointer_json(&pointer_request("down", 89_002, rail.screen_grip, 1))
            .unwrap();
        assert_eq!(bridge.captured_pointer, Some(89_002));
        bridge
            .pointer_json(&pointer_request("move", 89_002, target, 1))
            .unwrap();
        bridge
            .pointer_json(&pointer_request("up", 89_002, target, 0))
            .unwrap();

        assert!(bridge.pending_managed_mutation.is_some());
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source_before
        );
        assert_eq!(bridge.persistence_json().unwrap(), persistence_before);

        let valid = pending_managed_receipt(&bridge, managed_direct_fillet_radius_fixture());
        let mut malformed = valid.clone();
        malformed.compiled.artifact.artifact_digest = "0".repeat(64);
        bridge.resolve_pending_managed_mutation(malformed).unwrap();
        assert!(bridge.pending_managed_mutation.is_some());
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(bridge.persistence_json().unwrap(), persistence_before);

        bridge.resolve_pending_managed_mutation(valid).unwrap();
        assert!(bridge.pending_managed_mutation.is_none());
        assert_eq!(bridge.revision, revision_before + 1);
        let code = bridge.code_project.as_ref().unwrap();
        assert_eq!(
            code.code_session_identity().revision,
            identity_before.revision + 1
        );
        assert!(code.managed_source().contains("radius: mm(1.25)"));
        assert!(code.managed_test_overlay_is_empty());
        assert_eq!(
            code.selected_managed_declaration(bridge.editor()).unwrap(),
            Some(geosolve_sketch_code::SemanticSymbol("round".into()))
        );
        let scene = bridge
            .current_scene()
            .expect("published direct-Fillet scene");
        assert_eq!(scene.computed_curves.len(), 2);
        assert_eq!(scene.fillet_affordances.len(), 2);
        assert!(scene.fillet_affordances.iter().all(|affordance| {
            let rail = affordance.radius_rail;
            let radius = (rail.model_grip[0] - rail.model_center[0])
                .hypot(rail.model_grip[1] - rail.model_center[1]);
            (radius - 1.25).abs() <= 1.0e-12
        }));
        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("Fillet source candidate is independently accepted");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );
        assert!(
            accepted
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .points()
                .iter()
                .flat_map(|point| point.position)
                .all(f64::is_finite)
        );
    }

    #[test]
    fn managed_parameter_edit_restores_the_selected_fillet_after_cold_rematerialization() {
        let mut bridge = managed_direct_fillet_bridge();
        let opened: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let round_row = managed_row_id(&opened, "round");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.select",
                    "payload": { "id": round_row },
                })
                .to_string(),
            )
            .unwrap();
        let selected_before = bridge
            .editor()
            .selected_declaration()
            .expect("direct Fillet selected before its source edit");
        let radius_control = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_controls_cached()
            .unwrap()
            .controls
            .iter()
            .find(|control| {
                control.source.declaration.0 == "round"
                    && control.source.path.0
                        == [geosolve_sketch_code::ManagedPathSegment::Field(
                            "radius".into(),
                        )]
            })
            .expect("direct Fillet radius control")
            .id
            .0
            .clone();

        bridge
            .edit_parameter(&radius_control, serde_json::json!(1.25))
            .expect("prepare global Parameters Fillet edit");
        assert!(bridge.pending_managed_mutation.is_some());
        let receipt = pending_managed_receipt(&bridge, managed_direct_fillet_radius_fixture());
        bridge.resolve_pending_managed_mutation(receipt).unwrap();

        assert_eq!(
            bridge.editor().selected_declaration(),
            Some(selected_before)
        );
        assert_eq!(
            bridge
                .code_project
                .as_ref()
                .unwrap()
                .selected_managed_declaration(bridge.editor())
                .unwrap(),
            Some(geosolve_sketch_code::SemanticSymbol("round".into())),
        );
    }

    #[test]
    // One scenario deliberately follows ordinary and managed-code history
    // through their complete readiness transitions.
    #[allow(clippy::too_many_lines)]
    fn history_readiness_tracks_ordinary_outer_and_clean_code_authority() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        let initial: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(initial["presentation"]["canUndo"], false);
        assert_eq!(initial["presentation"]["canRedo"], false);

        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new"}"#)
            .unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"segment"}}"#)
            .unwrap();
        let pointer = |phase: &str, x: f64, y: f64, buttons: u16| {
            serde_json::json!({
                "version": 1,
                "phase": phase,
                "pointerId": 61,
                "x": x,
                "y": y,
                "buttons": buttons,
                "modifiers": {
                    "alt": false,
                    "ctrl": false,
                    "meta": false,
                    "shift": false,
                },
            })
            .to_string()
        };
        for (x, y) in [(340.0, 280.0), (560.0, 340.0)] {
            bridge.pointer_json(&pointer("down", x, y, 1)).unwrap();
            bridge.pointer_json(&pointer("up", x, y, 0)).unwrap();
        }
        let authored: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(authored["presentation"]["canUndo"], true);
        assert_eq!(authored["presentation"]["canRedo"], false);

        let undone: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(undone["presentation"]["canUndo"], false);
        assert_eq!(undone["presentation"]["canRedo"], true);
        let redone: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"history.redo"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(redone["presentation"]["canUndo"], true);
        assert_eq!(redone["presentation"]["canRedo"], false);

        bridge = managed_bridge();
        prepare_radius_restore(&mut bridge);
        let receipt = pending_managed_receipt(&bridge, managed_restored_fixture());
        bridge.resolve_pending_managed_mutation(receipt).unwrap();
        let edited: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(edited["presentation"]["canUndo"], true);
        assert_eq!(edited["presentation"]["canRedo"], false);

        let accepted_source = bridge
            .code_project
            .as_ref()
            .expect("managed fixture is a code project")
            .managed_source()
            .to_owned();
        let dirty: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(
                    &serde_json::json!({
                        "version": 1,
                        "command": "source.change",
                        "payload": {
                            "path": "sketch.ts",
                            "contents": format!("{accepted_source}\n// unapplied draft"),
                        },
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(dirty["presentation"]["canUndo"], false);
        assert_eq!(dirty["presentation"]["canRedo"], false);

        let reverted: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"source.revert"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(reverted["presentation"]["canUndo"], true);
        assert_eq!(reverted["presentation"]["canRedo"], false);
        let code_undone: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(code_undone["presentation"]["canRedo"], true);
    }

    #[test]
    fn canceling_a_staged_geometry_click_clears_its_painted_preview() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new"}"#)
            .unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"segment"}}"#)
            .unwrap();
        let revision_before = bridge.revision;
        bridge
            .pointer_json(
                r#"{"version":1,"phase":"down","pointerId":41,"x":360,"y":280,"buttons":1,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}"#,
            )
            .unwrap();
        assert!(bridge.construction_preview.is_some());

        bridge
            .cancel_json(r#"{"version":1,"reason":"lost-capture"}"#)
            .unwrap();

        assert!(bridge.construction_preview.is_none());
        assert_eq!(bridge.revision, revision_before);
        let document = bridge
            .editor()
            .coordinator()
            .presentation_session()
            .expect("cancel retains prior accepted authority")
            .design_document();
        assert!(document.points().is_empty());
        assert!(document.curves().is_empty());
    }

    #[test]
    fn projectional_cancel_cleanup_does_not_create_a_false_problem() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel retains accepted projectional authority");
        let expected = accepted.computed.input();
        let feature = accepted
            .features
            .features()
            .first()
            .expect("Typed Panel has a computed Fillet")
            .id;
        let revision_before = bridge.revision;

        bridge.dispatch_construction(vec![
            geosolve_constraint_editor::EditorEffect::RestoreComputedFeatureRadius {
                expected,
                feature,
                radius: 4.0,
            },
            geosolve_constraint_editor::EditorEffect::ClearAcceptedProfileOffsetPreview,
        ]);

        assert_eq!(bridge.revision, revision_before);
        assert!(bridge.last_error.is_none());
        let snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert!(snapshot["problems"].as_array().unwrap().is_empty());
    }

    #[test]
    fn cancel_json_restores_a_live_fillet_radius_drag_without_a_false_problem() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let rail = bridge
            .current_scene()
            .unwrap()
            .fillet_affordances
            .first()
            .expect("Typed Panel publishes a Fillet radius rail")
            .radius_rail;
        let revision_before = bridge.revision;
        let project_before = bridge.export_project_json().unwrap();

        bridge
            .pointer_json(
                &serde_json::json!({
                    "version": 1,
                    "phase": "down",
                    "pointerId": 51,
                    "x": rail.screen_grip.x,
                    "y": rail.screen_grip.y,
                    "buttons": 1,
                    "modifiers": {
                        "alt": false,
                        "ctrl": false,
                        "meta": false,
                        "shift": false,
                    },
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(bridge.captured_pointer, Some(51));
        assert_eq!(
            bridge
                .editor()
                .editor()
                .active_pointer_gesture()
                .expect("Fillet radius press owns a live gesture")
                .kind,
            geosolve_constraint_editor::ActivePointerGestureKind::FilletRadius,
        );

        let canceled: serde_json::Value = serde_json::from_str(
            &bridge
                .cancel_json(r#"{"version":1,"reason":"lost-capture"}"#)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(bridge.captured_pointer, None);
        assert!(bridge.editor().editor().active_pointer_gesture().is_none());
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(bridge.export_project_json().unwrap(), project_before);
        assert!(bridge.last_error.is_none());
        assert_eq!(canceled["project"]["status"], "accepted");
        assert!(canceled["problems"].as_array().unwrap().is_empty());
    }

    #[test]
    fn cancel_json_restores_a_live_profile_offset_drag_without_a_false_problem() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new"}"#)
            .unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"rectangle"}}"#)
            .unwrap();
        let pointer = |phase: &str, x: f64, y: f64, buttons: u16| {
            serde_json::json!({
                "version": 1,
                "phase": phase,
                "pointerId": 61,
                "x": x,
                "y": y,
                "buttons": buttons,
                "modifiers": {
                    "alt": false,
                    "ctrl": false,
                    "meta": false,
                    "shift": false,
                },
            })
            .to_string()
        };
        for (x, y) in [(360.0, 280.0), (600.0, 460.0)] {
            bridge.pointer_json(&pointer("down", x, y, 1)).unwrap();
            bridge.pointer_json(&pointer("up", x, y, 0)).unwrap();
        }
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"offset"}}"#)
            .unwrap();

        let face = bridge
            .offset_authoring
            .index()
            .and_then(|index| index.faces().first())
            .expect("the accepted rectangle publishes one Offset face")
            .key
            .clone();
        let outcome = bridge.offset_authoring.pick_target(
            geosolve_constraint_editor::OffsetAuthoringTarget::Face(face),
        );
        assert!(matches!(
            outcome,
            geosolve_constraint_editor::OffsetAuthoringOutcome::OperandChanged { .. }
        ));
        bridge.handle_offset_outcome(outcome);
        bridge.refresh_offset_preview().unwrap();

        let dimension = bridge
            .editor()
            .offset_authoring_provisional_items()
            .iter()
            .find_map(|item| match item {
                geosolve_constraint_editor::SelectionItem::Dimension(dimension) => Some(*dimension),
                _ => None,
            })
            .expect("the Profile Offset preview publishes one provisional dimension");
        let scene = bridge.current_scene().unwrap();
        let label = scene
            .annotations
            .iter()
            .find(|annotation| {
                annotation.item == geosolve_constraint_editor::SelectionItem::Dimension(dimension)
            })
            .and_then(|annotation| annotation.label_bounds)
            .expect("the Profile Offset preview publishes a pickable label");
        let press = ScreenPoint {
            x: 0.5 * (label.min.x + label.max.x),
            y: 0.5 * (label.min.y + label.max.y),
        };
        let revision_before = bridge.revision;
        let project_before = bridge.export_project_json().unwrap();

        bridge
            .pointer_json(&pointer("down", press.x, press.y, 1))
            .unwrap();
        assert_eq!(bridge.captured_pointer, Some(61));
        assert!(bridge.editor().offset_authoring_distance_drag_active());

        let canceled: serde_json::Value = serde_json::from_str(
            &bridge
                .cancel_json(r#"{"version":1,"reason":"lost-capture"}"#)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(bridge.captured_pointer, None);
        assert!(!bridge.editor().offset_authoring_distance_drag_active());
        assert!(bridge.offset_authoring.is_active());
        assert_eq!(bridge.active_tool, "offset");
        assert_eq!(canceled["presentation"]["activeTool"], "offset");
        assert_eq!(bridge.revision, revision_before);
        assert_eq!(bridge.export_project_json().unwrap(), project_before);
        assert!(bridge.last_error.is_none());
        assert_eq!(canceled["project"]["status"], "accepted");
        assert!(canceled["problems"].as_array().unwrap().is_empty());
    }

    #[test]
    fn bridge_is_dom_free_versioned_and_returns_an_authoritative_frame() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        let snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(snapshot["version"], 1);
        assert_eq!(snapshot["revision"], 0);
        assert_eq!(snapshot["project"]["status"], "accepted");
        assert!(
            snapshot["frame"]["svg"]
                .as_str()
                .unwrap()
                .starts_with("<svg")
        );
        assert!(
            snapshot["frame"]["svg"]
                .as_str()
                .unwrap()
                .contains("wb-accepted-scene")
        );
    }

    #[test]
    fn bootstrap_metadata_never_enters_presentation_labels() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        let snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let serialized = snapshot.to_string();
        assert!(!serialized.contains("IntentBootstrapMetadata"));
        assert!(!serialized.contains("Bootstrap {"));
        let imported = snapshot["explorer"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["children"].as_array().unwrap())
            .find(|item| item["label"] == "legacy-document")
            .expect("default document exposes its imported declaration");

        assert_eq!(imported["kind"], "Imported");
        let selected: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(
                    &serde_json::json!({
                        "version": 1,
                        "command": "declaration.select",
                        "payload": { "id": imported["id"] },
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(selected["selection"]["label"], "legacy-document");
        assert_eq!(selected["selection"]["kind"], "Imported");
    }

    #[test]
    fn authoritative_frame_embeds_renderer_owned_interactive_scene_presentation() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let frame = snapshot["frame"]["svg"].as_str().unwrap();

        assert!(frame.starts_with("<svg class=\"geosolve-authoritative-frame\""));
        assert!(frame.contains("<style>"));
        assert!(frame.find("<style>") < frame.find("wb-accepted-scene"));
        assert!(frame.contains(".wb-curve { fill: none; stroke: #e5e8df;"));
        assert!(frame.contains(".wb-point { fill: #131718; stroke: #8fd2ca;"));
        assert!(frame.contains(".wb-computed-fillet { stroke: #8ed5ca;"));
        assert!(frame.contains(".wb-computed-hit { fill: none; stroke: transparent;"));
        assert!(frame.contains(".wb-fillet-radius-grip { fill: #171c1d;"));
        assert!(frame.contains(".wb-dimension { color: #79bfc4;"));
        assert!(frame.contains(".wb-curve[data-role=\"construction\"]"));
        assert!(!frame.contains(".workbench {"));
        assert!(!frame.contains(".wb-app-bar"));
    }

    #[test]
    fn malformed_and_oversized_requests_are_atomic() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        let before = bridge.export_project_json().unwrap();
        assert!(
            bridge
                .dispatch_json(r#"{"version":2,"command":"project.new"}"#)
                .is_err()
        );
        assert!(
            bridge
                .dispatch_json(r#"{"version":1,"command":"project.new","extra":true}"#)
                .is_err()
        );
        assert!(WorkbenchBridge::construct_json(&" ".repeat(MAX_REQUEST_BYTES + 1)).is_err());
        assert_eq!(bridge.export_project_json().unwrap(), before);
    }

    #[test]
    fn resize_changes_no_semantic_or_frame_authority() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        let before = bridge.export_project_json().unwrap();
        let frame_before: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            bridge
                .resize_json(r#"{"version":1,"width":1920,"height":1080,"pixelRatio":2}"#)
                .unwrap(),
            "null"
        );
        assert_eq!(bridge.export_project_json().unwrap(), before);
        let frame_after: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(frame_after["revision"], frame_before["revision"]);
        assert_eq!(frame_after["frame"], frame_before["frame"]);
    }

    #[test]
    fn successful_project_import_retains_live_viewport_and_pointer_alignment() {
        let mut donor = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        donor
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .expect("Typed Panel donor project");
        let exported: serde_json::Value =
            serde_json::from_str(&donor.export_project_json().unwrap()).unwrap();
        let contents = exported["contents"]
            .as_str()
            .expect("project export contents");

        let host_size = [968.75, 820.0];
        let pixel_ratio = 2.5_f64;
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .resize_json(
                &serde_json::json!({
                    "version": 1,
                    "width": host_size[0],
                    "height": host_size[1],
                    "pixelRatio": pixel_ratio,
                })
                .to_string(),
            )
            .expect("non-default live viewport");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "project.import",
                    "payload": {
                        "path": "project.json",
                        "contents": contents,
                    },
                })
                .to_string(),
            )
            .expect("successful atomic project import");

        assert_eq!(
            bridge.host_size.map(f64::to_bits),
            host_size.map(f64::to_bits)
        );
        assert_eq!(bridge.pixel_ratio.to_bits(), pixel_ratio.to_bits());

        let scene = bridge.current_scene().expect("imported accepted scene");
        let point = *scene
            .points
            .first()
            .expect("Typed Panel has a visible point");
        let screen = scene.viewport.screen_size;
        let scale = (host_size[0] / screen[0]).min(host_size[1] / screen[1]);
        let left = (host_size[0] - screen[0] * scale) * 0.5;
        let top = (host_size[1] - screen[1] * scale) * 0.5;
        let client = ScreenPoint {
            x: left + point.screen_position.x * scale,
            y: top + point.screen_position.y * scale,
        };
        bridge
            .pointer_json(&pointer_request("move", 90_006_001, client, 0))
            .expect("browser-relative point sample");
        assert_eq!(
            bridge.editor().editor().hover_state().target,
            Some(EditorHoverTarget::Geometry(SelectionItem::Point(point.id))),
            "the imported SVG point and bridge normalization must retain one coordinate space",
        );
    }

    #[test]
    fn code_sample_source_failure_retains_the_accepted_frame() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let accepted: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let accepted_identity = bridge
            .code_project
            .as_ref()
            .expect("code authority")
            .code_session_identity()
            .clone();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"source.prepare","payload":{"path":"sketch.ts","contents":"not managed source"}}"#,
            )
            .unwrap();
        let PendingManagedMutation::Source { prepared } = bridge
            .pending_managed_mutation
            .as_ref()
            .expect("raw source compilation pending")
        else {
            panic!("expected a raw-source ticket")
        };
        let ticket_digest = prepared.request.ticket.ticket_digest.clone();
        bridge
            .abort_pending_managed_mutation(&ManagedMutationAbortPayload {
                ticket_digest,
                candidate_source: "not managed source".into(),
                diagnostic: "managed directive is unavailable".into(),
                span: ManagedSpan::new(0, 3),
            })
            .unwrap();
        let failed: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(failed["project"]["status"], "failed");
        assert!(!failed["problems"].as_array().unwrap().is_empty());
        assert_eq!(failed["frame"]["svg"], accepted["frame"]["svg"]);
        assert_eq!(
            bridge
                .code_project
                .as_ref()
                .expect("code authority")
                .code_session_identity(),
            &accepted_identity,
        );
    }

    #[test]
    fn source_keystroke_snapshot_reuses_the_cached_accepted_frame() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let accepted: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert!(bridge.accepted_frame.is_some());

        // If source.change attempted presentation/scene work, this sentinel
        // would be repopulated while serializing its returned snapshot.
        bridge.retained_scene = None;
        let draft: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(
                    r#"{"version":1,"command":"source.change","payload":{"path":"sketch.ts","contents":"not managed source"}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert!(bridge.retained_scene.is_none());
        assert_eq!(draft["frame"], accepted["frame"]);
    }

    #[test]
    fn raw_source_apply_is_rust_prepared_compiler_resolved_and_one_history_row() {
        let mut bridge = managed_lifecycle_bridge();
        for retired in ["source.apply", "source.apply-compiled"] {
            assert!(
                bridge
                    .dispatch_json(
                        &serde_json::json!({ "version": 1, "command": retired }).to_string()
                    )
                    .unwrap_err()
                    .contains("unknown workbench command")
            );
        }
        let before = bridge
            .code_project
            .as_ref()
            .expect("code authority")
            .code_session_identity()
            .clone();
        let candidate = managed_lifecycle_fixture("reordered");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "source.prepare",
                    "payload": {
                        "path": "sketch.ts",
                        "contents": candidate.normalized_source.clone(),
                    },
                })
                .to_string(),
            )
            .expect("Rust prepares raw source");
        let pending: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(pending["pendingManagedMutation"]["kind"], "source");
        assert_eq!(
            pending["pendingManagedMutation"]["request"]["ticket"]["format"],
            "geosolve-prepared-managed-source-v1",
        );
        let receipt = pending_source_receipt(&bridge, candidate);
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("Rust resolves and cold-validates raw source");
        let code = bridge
            .code_project
            .as_ref()
            .expect("accepted code authority");
        assert_eq!(code.code_session_identity().revision, before.revision + 1);
        assert_eq!(
            declaration_order(&bridge),
            ["panel", "cornerFillets", "guide"]
        );
        assert!(!code.is_dirty());

        bridge
            .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
            .expect("one undo returns to source before raw Apply");
        assert_eq!(
            declaration_order(&bridge),
            ["guide", "panel", "cornerFillets"]
        );
    }

    #[test]
    fn managed_compiler_context_is_on_demand_and_contains_only_pinned_patch_plans() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let snapshot = bridge.snapshot_json().unwrap();
        assert!(!snapshot.contains("declaration_family"));

        let context: serde_json::Value =
            serde_json::from_str(&bridge.managed_compiler_context_json().unwrap()).unwrap();
        assert_eq!(context["version"], 1);
        let patches = context["patches"].as_object().unwrap();
        assert_eq!(patches.len(), 1);
        let fillets = patches.get("fillets").expect("Typed Panel patch binding");
        assert_eq!(fillets["format"], "geosolve-patch-artifact-v2");
        assert!(
            fillets["templates"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );
    }

    #[test]
    fn selected_generated_fillet_publishes_its_exact_managed_source_owner() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let opened: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let fillet_declaration = opened["explorer"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["children"].as_array().unwrap())
            .find(|item| item["label"] == "cornerFillets")
            .expect("Typed Panel computed Fillet declaration");
        let fillet = fillet_declaration["children"]
            .as_array()
            .unwrap()
            .first()
            .expect("Typed Panel generated Fillet output");
        let selected: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(
                    &serde_json::json!({
                        "version": 1,
                        "command": "declaration.select",
                        "payload": { "id": fillet["id"] },
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(selected["selection"]["ownership"], "Modifiable in source");
        let source = &selected["selection"]["source"];
        assert_eq!(source["path"], "sketch.ts");
        let start = usize::try_from(source["from"].as_u64().expect("source start"))
            .expect("source start fits usize");
        let end = usize::try_from(source["to"].as_u64().expect("source end"))
            .expect("source end fits usize");
        let managed = selected["source"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == "sketch.ts")
            .unwrap()["contents"]
            .as_str()
            .unwrap();
        let owned = &managed[start..end];
        assert_eq!(
            owned,
            r#"$.use("cornerFillets", fillets, {
    corners: {
      lowerLeft: panel.filletableCorners.byKey.lowerLeft,
      upperRight: panel.filletableCorners.byKey.upperRight,
    },
    radius: mm(4),
  })"#
        );
    }

    #[test]
    fn m89_ordered_declaration_panel_is_source_owned_and_nests_generated_outputs() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let groups = snapshot["explorer"].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["rowKind"], "group");
        let declarations = groups[0]["children"].as_array().unwrap();
        assert_eq!(
            declarations
                .iter()
                .map(|row| row["label"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["panel", "cornerFillets"],
        );
        assert!(
            declarations
                .iter()
                .all(|row| row["id"].as_str().unwrap().starts_with("managed:"))
        );
        let invocation = &declarations[1];
        assert_eq!(invocation["kind"], "Patch invocation");
        let generated = invocation["children"].as_array().unwrap();
        assert_eq!(
            generated
                .iter()
                .map(|row| row["label"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["lowerLeft", "upperRight"],
        );
        assert!(generated.iter().all(|row| {
            row["rowKind"] == "generated"
                && row["id"].as_str().unwrap().starts_with("generated:")
                && row["capabilities"]["move"]["enabled"] == false
        }));
        let explorer_wire = snapshot["explorer"].to_string();
        assert!(!explorer_wire.contains("code."));
        assert!(!explorer_wire.contains("GeneratedMemberAddress"));

        let source = &invocation["source"];
        let from = source["from"].as_u64().unwrap();
        let to = source["to"].as_u64().unwrap();
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.source.open",
                    "payload": { "id": invocation["id"], "from": from, "to": to },
                })
                .to_string(),
            )
            .unwrap();
        assert!(
            bridge
                .dispatch_json(
                    &serde_json::json!({
                        "version": 1,
                        "command": "declaration.source.open",
                        "payload": { "id": invocation["id"], "from": from + 1, "to": to },
                    })
                    .to_string(),
                )
                .unwrap_err()
                .contains("stale declaration authority")
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one presentation-state regression keeps row/group composition, isolate restore, paint/pick filtering, accepted authority, and persistence together"
    )]
    fn m91_explorer_visibility_is_composed_presentation_only_and_restorable() {
        let mut bridge = managed_lifecycle_bridge();
        let base_revision = bridge.revision;
        let base_project = canonical_project_json(&bridge);
        let base_source = bridge
            .code_project
            .as_ref()
            .expect("managed project")
            .managed_source()
            .to_owned();
        let base_can_undo = bridge.can_undo();
        let base_can_redo = bridge.can_redo();
        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("accepted materialization");
        let base_design_identity = accepted.session.design_identity();
        let base_document = accepted
            .session
            .accepted_state_for_current_input()
            .expect("accepted document")
            .document()
            .clone();
        let base_validation = accepted.validation.clone();
        let base_scene = bridge.current_scene().expect("base accepted scene");

        let base_snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let lifecycle_group = base_snapshot["explorer"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["label"] == "Lifecycle")
            .expect("Lifecycle group");
        let lifecycle_group_id = lifecycle_group["id"].as_str().unwrap().to_owned();
        let declarations_group_id = base_snapshot["explorer"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["label"] == "Declarations")
            .and_then(|group| group["id"].as_str())
            .expect("ungrouped declarations")
            .to_owned();
        let guide_id = managed_row_id(&base_snapshot, "guide");
        let panel_id = managed_row_id(&base_snapshot, "panel");
        let corner_fillets = nested_explorer_row(&base_snapshot, "cornerFillets");
        let lower_left_id = corner_fillets["children"]
            .as_array()
            .expect("generated Fillet rows")
            .iter()
            .find(|row| row["label"] == "lowerLeft")
            .expect("generated lower-left Fillet row")["id"]
            .as_str()
            .expect("generated lower-left row identity")
            .to_owned();

        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.set",
                    "payload": { "id": lower_left_id, "visible": false },
                })
                .to_string(),
            )
            .expect("generated output hides independently");
        let generated_hidden: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let corner_fillets = nested_explorer_row(&generated_hidden, "cornerFillets");
        let generated = corner_fillets["children"]
            .as_array()
            .expect("generated Fillet rows");
        assert_eq!(
            generated
                .iter()
                .find(|row| row["label"] == "lowerLeft")
                .unwrap()["effectiveVisible"],
            false
        );
        assert_eq!(
            generated
                .iter()
                .find(|row| row["label"] == "upperRight")
                .unwrap()["effectiveVisible"],
            true
        );
        assert_eq!(corner_fillets["visibilityState"], "mixed");
        assert_eq!(
            bridge
                .current_scene()
                .expect("generated-filtered scene")
                .computed_curves
                .len()
                + 1,
            base_scene.computed_curves.len()
        );
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.set",
                    "payload": { "id": lower_left_id, "visible": true },
                })
                .to_string(),
            )
            .expect("generated output shows independently");

        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.set",
                    "payload": { "id": panel_id, "visible": false },
                })
                .to_string(),
            )
            .expect("individual row hides");
        let hidden_child: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            nested_explorer_row(&hidden_child, "panel")["visible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&hidden_child, "panel")["effectiveVisible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&hidden_child, "Lifecycle")["visibilityState"],
            "mixed"
        );

        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.set",
                    "payload": { "id": lifecycle_group_id, "visible": false },
                })
                .to_string(),
            )
            .expect("ancestor group hides");
        let hidden_group: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(nested_explorer_row(&hidden_group, "guide")["visible"], true);
        assert_eq!(
            nested_explorer_row(&hidden_group, "guide")["effectiveVisible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&hidden_group, "panel")["visible"],
            false
        );

        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.set",
                    "payload": { "id": lifecycle_group_id, "visible": true },
                })
                .to_string(),
            )
            .expect("ancestor group shows");
        let reshown_group: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            nested_explorer_row(&reshown_group, "guide")["effectiveVisible"],
            true
        );
        assert_eq!(
            nested_explorer_row(&reshown_group, "panel")["visible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&reshown_group, "panel")["effectiveVisible"],
            false
        );

        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.set",
                    "payload": { "id": panel_id, "visible": true },
                })
                .to_string(),
            )
            .expect("child shows independently");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.set",
                    "payload": { "id": guide_id, "visible": false },
                })
                .to_string(),
            )
            .expect("guide hides independently");

        let hidden_items = bridge.hidden_scene_items(&base_scene);
        let hidden_curve = hidden_items
            .iter()
            .find_map(|item| match item {
                SelectionItem::Curve(span) => Some(*span),
                _ => None,
            })
            .expect("guide owns a native curve");
        let guide_curve = base_scene
            .curves
            .iter()
            .find(|curve| curve.span == hidden_curve)
            .expect("base guide curve");
        let start = guide_curve.screen_polyline.first().copied().unwrap();
        let end = guide_curve.screen_polyline.last().copied().unwrap();
        let midpoint = ScreenPoint {
            x: (start.x + end.x) * 0.5,
            y: (start.y + end.y) * 0.5,
        };
        let tolerance = geosolve_constraint_editor::PickTolerance::default();
        assert_eq!(
            base_scene.hit_test(midpoint, tolerance).map(|hit| hit.item),
            Some(SelectionItem::Curve(hidden_curve))
        );
        let filtered_scene = bridge.current_scene().expect("filtered accepted scene");
        assert!(
            filtered_scene
                .curves
                .iter()
                .all(|curve| curve.span != hidden_curve)
        );
        assert_ne!(
            filtered_scene
                .hit_test(midpoint, tolerance)
                .map(|hit| hit.item),
            Some(SelectionItem::Curve(hidden_curve))
        );

        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.isolate",
                    "payload": { "id": declarations_group_id },
                })
                .to_string(),
            )
            .expect("group isolates");
        let isolated: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(isolated["presentation"]["visibilityRestoreAvailable"], true);
        assert_eq!(
            nested_explorer_row(&isolated, "Lifecycle")["effectiveVisible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&isolated, "Declarations")["effectiveVisible"],
            true
        );

        bridge
            .dispatch_json(r#"{"version":1,"command":"explorer.visibility.restore"}"#)
            .expect("isolate baseline restores");
        let restored_visibility: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            restored_visibility["presentation"]["visibilityRestoreAvailable"],
            false
        );
        assert_eq!(
            nested_explorer_row(&restored_visibility, "guide")["visible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&restored_visibility, "panel")["visible"],
            true
        );

        bridge
            .dispatch_json(r#"{"version":1,"command":"view.construction.toggle"}"#)
            .expect("construction presentation toggles");
        let construction_hidden: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            construction_hidden["presentation"]["constructionVisible"],
            false
        );
        let policy = bridge.editor().editor().geometry_interaction_policy();
        assert!(!policy.visibility.explicit_construction);
        assert!(!policy.visibility.implicit_construction);
        let construction = filtered_scene
            .curves
            .iter()
            .find(|curve| curve.role == GeometryRole::Construction)
            .expect("Fillet source portions expose construction composition");
        assert!(!construction.is_visible(policy));
        assert!(!construction.is_interactive(policy));

        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "explorer.visibility.isolate",
                    "payload": { "id": declarations_group_id },
                })
                .to_string(),
            )
            .expect("persisted isolate starts");
        let persistence: serde_json::Value =
            serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
        let restore_request = serde_json::json!({
            "version": 1,
            "persistedProject": persistence["contents"].as_str().unwrap(),
        });
        let mut reloaded = WorkbenchBridge::construct_json(&restore_request.to_string())
            .expect("presentation envelope reloads");
        let reloaded_snapshot: serde_json::Value =
            serde_json::from_str(&reloaded.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            reloaded_snapshot["presentation"]["visibilityRestoreAvailable"],
            true
        );
        assert_eq!(
            reloaded_snapshot["presentation"]["constructionVisible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&reloaded_snapshot, "Lifecycle")["effectiveVisible"],
            false
        );

        let reproduction: serde_json::Value =
            serde_json::from_str(&bridge.reproduction_json().unwrap()).unwrap();
        let reproduction_request = serde_json::json!({
            "version": 1,
            "persistedProject": reproduction["contents"].as_str().unwrap(),
        });
        let mut reproduced = WorkbenchBridge::construct_json(&reproduction_request.to_string())
            .expect("reproduction restores presentation envelope");
        let reproduced_snapshot: serde_json::Value =
            serde_json::from_str(&reproduced.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            reproduced_snapshot["presentation"]["visibilityRestoreAvailable"],
            true
        );
        assert_eq!(
            reproduced_snapshot["presentation"]["constructionVisible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&reproduced_snapshot, "Lifecycle")["effectiveVisible"],
            false
        );

        reloaded
            .dispatch_json(r#"{"version":1,"command":"explorer.visibility.restore"}"#)
            .expect("reloaded isolate baseline restores");
        let reloaded_restored: serde_json::Value =
            serde_json::from_str(&reloaded.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            nested_explorer_row(&reloaded_restored, "guide")["visible"],
            false
        );
        assert_eq!(
            nested_explorer_row(&reloaded_restored, "panel")["visible"],
            true
        );

        assert_eq!(bridge.revision, base_revision);
        assert_eq!(canonical_project_json(&bridge), base_project);
        assert_eq!(
            bridge
                .code_project
                .as_ref()
                .expect("managed project")
                .managed_source(),
            base_source
        );
        assert_eq!(bridge.can_undo(), base_can_undo);
        assert_eq!(bridge.can_redo(), base_can_redo);
        let accepted = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("accepted materialization remains");
        assert_eq!(accepted.session.design_identity(), base_design_identity);
        assert_eq!(
            accepted
                .session
                .accepted_state_for_current_input()
                .expect("accepted document remains")
                .document(),
            &base_document
        );
        assert_eq!(accepted.validation, base_validation);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one public-workbench regression keeps M89 reorder, both suppression kinds, accepted scene, history, persistence, and cold headless inspection in one authority chain"
    )]
    fn m89_reorder_and_suppression_lifecycle_is_source_scene_history_and_headless_exact() {
        let mut bridge = managed_lifecycle_bridge();
        let base_project = canonical_project_json(&bridge);
        let base_persistence = bridge.persistence_json().unwrap();
        let base_revision = bridge.revision;
        let base_scene = bridge.current_scene().expect("base accepted scene");
        let base_native_curves = base_scene.curves.len();
        let base_computed_curves = base_scene.computed_curves.len();
        let base_validation = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("base accepted authority")
            .validation
            .clone();
        assert_eq!(
            declaration_order(&bridge),
            ["guide", "panel", "cornerFillets"]
        );

        let base_snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let panel_id = managed_row_id(&base_snapshot, "panel");
        let invalid = serde_json::json!({
            "version": 1,
            "command": "declaration.move",
            "payload": { "id": panel_id, "direction": "down" },
        });
        assert!(
            bridge
                .dispatch_json(&invalid.to_string())
                .unwrap_err()
                .contains("consumer before its dependency")
        );
        assert!(bridge.pending_managed_mutation.is_none());
        assert_eq!(bridge.revision, base_revision);
        assert_eq!(canonical_project_json(&bridge), base_project);
        assert_eq!(bridge.persistence_json().unwrap(), base_persistence);

        let guide_id = managed_row_id(&base_snapshot, "guide");
        let corners_id = managed_row_id(&base_snapshot, "cornerFillets");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.move",
                    "payload": {
                        "id": guide_id,
                        "targetId": corners_id,
                        "position": "after",
                    },
                })
                .to_string(),
            )
            .expect("independent declaration move prepares");
        let receipt = pending_managed_receipt(&bridge, managed_lifecycle_fixture("reordered"));
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("reorder publishes");
        assert_eq!(
            declaration_order(&bridge),
            ["panel", "cornerFillets", "guide"]
        );
        let reordered_source = bridge
            .code_project
            .as_ref()
            .expect("code authority")
            .managed_source();
        assert!(
            reordered_source.find("const corners").unwrap()
                < reordered_source.find("const guide").unwrap()
        );

        let reordered_snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let guide_id = managed_row_id(&reordered_snapshot, "guide");
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.suppression.set",
                    "payload": { "id": guide_id, "suppressed": true },
                })
                .to_string(),
            )
            .expect("direct suppression prepares");
        let receipt =
            pending_managed_receipt(&bridge, managed_lifecycle_fixture("direct-suppressed"));
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("direct suppression publishes");

        let direct_snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        let corner_fillets = direct_snapshot["explorer"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["children"].as_array().unwrap())
            .find(|row| row["label"] == "cornerFillets")
            .expect("computed Fillet source declaration");
        let lower_left = corner_fillets["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["label"] == "lowerLeft")
            .expect("lower-left generated row");
        assert_eq!(lower_left["suppressed"], false);
        let lower_left_id = lower_left["id"].as_str().unwrap().to_owned();
        bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 1,
                    "command": "declaration.suppression.set",
                    "payload": { "id": lower_left_id, "suppressed": true },
                })
                .to_string(),
            )
            .expect("generated suppression prepares");
        let receipt =
            pending_managed_receipt(&bridge, managed_lifecycle_fixture("both-suppressed"));
        bridge
            .resolve_pending_managed_mutation(receipt)
            .expect("generated suppression publishes");

        let final_project = canonical_project_json(&bridge);
        let final_persistence = bridge.persistence_json().unwrap();
        let final_source = bridge
            .code_project
            .as_ref()
            .expect("code authority")
            .managed_source()
            .to_owned();
        assert!(final_source.contains("$.suppress(guide);"));
        assert!(final_source.contains("$.suppress(corners.fillets[\"lowerLeft\"]);"));
        let final_compiled = accepted_compiled_project(&bridge)
            .managed
            .compiled
            .expect("managed authority");
        assert_eq!(final_compiled.artifact.suppressions.len(), 2);
        assert_eq!(
            final_compiled
                .artifact
                .suppressions
                .iter()
                .map(|suppression| suppression.target.declaration.as_str())
                .collect::<Vec<_>>(),
            ["guide", "corners"]
        );

        let final_snapshot: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(
            final_snapshot["explorer"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|group| group["children"].as_array().unwrap())
                .map(|row| row["label"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["panel", "cornerFillets", "guide"]
        );
        assert_eq!(
            final_snapshot["explorer"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|group| group["children"].as_array().unwrap())
                .find(|row| row["label"] == "guide")
                .unwrap()["suppressed"],
            true
        );
        assert!(
            final_snapshot["explorer"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|group| group["children"].as_array().unwrap())
                .find(|row| row["label"] == "cornerFillets")
                .and_then(|declaration| declaration["children"].as_array())
                .and_then(|children| children.iter().find(|row| row["label"] == "lowerLeft"))
                .is_some_and(|row| row["suppressed"] == true),
        );

        let final_scene = bridge.current_scene().expect("final accepted scene");
        assert!(final_scene.curves.len() < base_native_curves);
        assert_eq!(final_scene.computed_curves.len() + 1, base_computed_curves);
        let validation = bridge
            .editor()
            .coordinator()
            .accepted_materialization()
            .expect("final accepted authority")
            .validation
            .clone();
        assert!(validation.hard_residuals_validated);
        assert!(validation.all_active_features_current);
        assert_eq!(validation.curve_count + 1, base_validation.curve_count);
        assert!(
            validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );

        for expected in ["direct-suppressed", "reordered", "base"] {
            bridge
                .dispatch_json(r#"{"version":1,"command":"history.undo"}"#)
                .expect("lifecycle undo");
            assert_eq!(
                canonical_project_json(&bridge),
                if expected == "base" {
                    base_project.clone()
                } else {
                    let compiled = managed_lifecycle_fixture(expected);
                    let mut project = accepted_compiled_project(&bridge);
                    project.managed = compiled.into_managed_document().unwrap();
                    project.to_canonical_json().unwrap()
                }
            );
        }
        assert_eq!(canonical_project_json(&bridge), base_project);
        for _ in 0..3 {
            bridge
                .dispatch_json(r#"{"version":1,"command":"history.redo"}"#)
                .expect("lifecycle redo");
        }
        assert_eq!(canonical_project_json(&bridge), final_project);

        let persistence_envelope: serde_json::Value =
            serde_json::from_str(&final_persistence).unwrap();
        let restore_request = serde_json::json!({
            "version": 1,
            "persistedProject": persistence_envelope["contents"].as_str().unwrap(),
        });
        let mut restored = WorkbenchBridge::construct_json(&restore_request.to_string())
            .expect("persisted lifecycle restores");
        assert_eq!(canonical_project_json(&restored), final_project);
        assert_eq!(
            declaration_order(&restored),
            ["panel", "cornerFillets", "guide"]
        );
        let restored_scene = restored.current_scene().expect("restored accepted scene");
        assert_eq!(restored_scene.curves.len(), final_scene.curves.len());
        assert_eq!(
            restored_scene.computed_curves.len(),
            final_scene.computed_curves.len()
        );
        assert_eq!(restored.persistence_json().unwrap(), final_persistence);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let inspection = geosolve_headless::inspect(
                &geosolve_headless::HeadlessInput::CodeProjectJson(final_project.clone()),
            )
            .expect("cold headless lifecycle inspection");
            assert!(inspection.report.validation.hard_residuals_validated);
            assert!(inspection.report.validation.all_active_features_current);
            assert_eq!(
                inspection
                    .report
                    .validation
                    .maximum_normalized_hard_residual,
                validation.maximum_normalized_hard_residual
            );
            assert_eq!(
                inspection.report.validation.point_count,
                validation.point_count
            );
            assert_eq!(
                inspection.report.validation.curve_count,
                validation.curve_count
            );
            assert_eq!(
                inspection.report.validation.feature_count,
                validation.feature_count
            );
            assert_eq!(
                inspection.report.source_digest,
                final_compiled.ir.source_digest
            );
        }
    }

    #[test]
    fn browser_persistence_reproduction_and_trace_transports_are_bounded_and_restorable() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(
                r#"{"version":1,"command":"sample.open","payload":{"key":"typed-panel"}}"#,
            )
            .unwrap();
        let canonical = bridge.export_project_json().unwrap();

        let persisted: serde_json::Value =
            serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
        assert_eq!(persisted["version"], 1);
        let persisted_contents = persisted["contents"].as_str().unwrap();
        assert!(persisted_contents.len() <= MAX_REQUEST_BYTES);
        let restored_request = serde_json::json!({
            "version": 1,
            "persistedProject": persisted_contents,
        });
        let restored = WorkbenchBridge::construct_json(&restored_request.to_string()).unwrap();
        assert_eq!(restored.export_project_json().unwrap(), canonical);

        let reproduction: serde_json::Value =
            serde_json::from_str(&bridge.reproduction_json().unwrap()).unwrap();
        let reproduction_contents = reproduction["contents"].as_str().unwrap();
        assert!(reproduction_contents.len() <= MAX_REQUEST_BYTES);
        let reproduction_request = serde_json::json!({
            "version": 1,
            "persistedProject": reproduction_contents,
        });
        let reproduced =
            WorkbenchBridge::construct_json(&reproduction_request.to_string()).unwrap();
        assert_eq!(reproduced.export_project_json().unwrap(), canonical);

        bridge
            .pointer_json(
                r#"{"version":1,"phase":"down","pointerId":7,"x":500,"y":350,"buttons":1,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}"#,
            )
            .unwrap();
        let trace: serde_json::Value =
            serde_json::from_str(&bridge.interaction_trace_json().unwrap()).unwrap();
        let trace_contents = trace["contents"].as_str().unwrap();
        assert!(trace_contents.contains("GEOSOLVE_INTERACTION_TRACE_V1"));
        assert!(trace_contents.contains("browser.pointerdown"));
        assert!(trace_contents.len() <= super::super::interaction_trace::MAX_TRACE_EXPORT_BYTES);
    }

    #[test]
    fn every_visible_redesign_tool_identity_has_a_rust_route() {
        let commands: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/frontend/src/data/commands.json"
        )))
        .unwrap();
        for id in commands
            .as_object()
            .unwrap()
            .values()
            .flat_map(|entries| entries.as_array().unwrap())
            .map(|entry| entry["id"].as_str().unwrap())
            .chain(["select"])
        {
            let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
            let request = serde_json::json!({
                "version": 1,
                "command": "tool.select",
                "payload": { "id": id },
            });
            assert!(
                bridge.dispatch_json(&request.to_string()).is_ok(),
                "visible tool `{id}` has no bridge route"
            );
        }
    }

    #[test]
    fn canvas_view_actions_do_not_replace_the_active_authoring_tool() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"segment"}}"#)
            .unwrap();
        assert_eq!(bridge.active_tool, "segment");

        for action in ["construction-display", "zoom-fit", "zoom-origin"] {
            bridge
                .dispatch_json(
                    &serde_json::json!({
                        "version": 1,
                        "command": "tool.select",
                        "payload": { "id": action },
                    })
                    .to_string(),
                )
                .unwrap();
            assert_eq!(
                bridge.active_tool, "segment",
                "canvas view action `{action}` replaced the authoring tool",
            );
        }

        for command in ["view.grid.toggle", "view.fit", "view.origin"] {
            let snapshot: serde_json::Value = serde_json::from_str(
                &bridge
                    .dispatch_json(
                        &serde_json::json!({
                            "version": 1,
                            "command": command,
                        })
                        .to_string(),
                    )
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(bridge.active_tool, "segment");
            assert_eq!(snapshot["presentation"]["activeTool"], "segment");
            assert!(snapshot["presentation"]["gridVisible"].is_boolean());
        }

        let revision = bridge.revision;
        let role_snapshot: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"geometry.authoring-role.toggle"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(bridge.active_tool, "segment");
        assert_eq!(bridge.revision, revision);
        assert_eq!(
            role_snapshot["presentation"]["geometryRole"],
            "construction"
        );
        assert!(role_snapshot["presentation"]["selectedGeometryRole"].is_null());
    }

    #[test]
    fn selected_curve_role_is_distinct_from_the_new_curve_authoring_role() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new"}"#)
            .unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"segment"}}"#)
            .unwrap();

        for (x, y) in [(360, 280), (560, 280)] {
            for (phase, buttons) in [("down", 1), ("up", 0)] {
                bridge
                    .pointer_json(
                        &serde_json::json!({
                            "version": 1,
                            "phase": phase,
                            "pointerId": 63,
                            "x": x,
                            "y": y,
                            "buttons": buttons,
                            "modifiers": {
                                "alt": false,
                                "ctrl": false,
                                "meta": false,
                                "shift": false,
                            },
                        })
                        .to_string(),
                    )
                    .unwrap();
            }
        }

        let span = {
            let document = bridge
                .editor()
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document();
            document.curve_spans(document.curves()[0].id).unwrap()[0]
        };
        bridge
            .editor_mut()
            .set_selection([SelectionItem::Curve(span)]);
        let selected: serde_json::Value =
            serde_json::from_str(&bridge.snapshot_json().unwrap()).unwrap();
        assert_eq!(selected["presentation"]["geometryRole"], "profile");
        assert_eq!(selected["presentation"]["selectedGeometryRole"], "profile");

        let toggled: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(r#"{"version":1,"command":"geometry.role.toggle"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(toggled["presentation"]["geometryRole"], "profile");
        assert!(toggled["presentation"]["selectedGeometryRole"].is_null());
        assert_eq!(
            bridge
                .editor()
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document()
                .geometry_role(span.curve),
            Some(GeometryRole::Construction)
        );
    }

    #[test]
    fn complete_preselection_returns_to_select_after_one_shot_authoring() {
        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"project.new"}"#)
            .unwrap();
        bridge
            .dispatch_json(r#"{"version":1,"command":"tool.select","payload":{"id":"segment"}}"#)
            .unwrap();

        for (x, y) in [(360, 280), (560, 340)] {
            for (phase, buttons) in [("down", 1), ("up", 0)] {
                bridge
                    .pointer_json(
                        &serde_json::json!({
                            "version": 1,
                            "phase": phase,
                            "pointerId": 64,
                            "x": x,
                            "y": y,
                            "buttons": buttons,
                            "modifiers": {
                                "alt": false,
                                "ctrl": false,
                                "meta": false,
                                "shift": false,
                            },
                        })
                        .to_string(),
                    )
                    .unwrap();
            }
        }

        let span = {
            let document = bridge
                .editor()
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document();
            document.curve_spans(document.curves()[0].id).unwrap()[0]
        };
        bridge
            .editor_mut()
            .set_selection([SelectionItem::Curve(span)]);
        let revision_before = bridge.revision;
        let applied: serde_json::Value = serde_json::from_str(
            &bridge
                .dispatch_json(
                    r#"{"version":1,"command":"tool.select","payload":{"id":"horizontal"}}"#,
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(bridge.revision, revision_before + 1);
        assert!(bridge.authoring.active_tool().is_none());
        assert_eq!(bridge.active_tool, "select");
        assert_eq!(applied["presentation"]["activeTool"], "select");
        assert_eq!(bridge.notice, "Relation or dimension accepted");
    }

    #[test]
    fn static_tool_catalog_is_complete_bounded_and_absent_from_hot_snapshots() {
        let encoded = WorkbenchBridge::tool_catalog_json().unwrap();
        assert_eq!(encoded, WorkbenchBridge::tool_catalog_json().unwrap());
        assert!(encoded.len() <= super::MAX_TOOL_CATALOG_BYTES);
        let catalog: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(catalog["version"], 1);
        let sections = catalog["sections"].as_array().unwrap();
        assert_eq!(
            sections
                .iter()
                .map(|section| (
                    section["id"].as_str().unwrap(),
                    section["commands"].as_array().unwrap().len(),
                ))
                .collect::<Vec<_>>(),
            [
                ("sketch", 25),
                ("constraint", 13),
                ("dimension", 5),
                ("modify", 2),
            ],
        );

        let mut stable_ids = std::collections::HashSet::new();
        let mut tool_ids = std::collections::HashSet::new();
        let mut icon_keys = std::collections::HashSet::new();
        let commands = std::iter::once(&catalog["select"])
            .chain(std::iter::once(&catalog["geometryRole"]))
            .chain(
                sections
                    .iter()
                    .flat_map(|section| section["commands"].as_array().unwrap().iter()),
            );
        for command in commands {
            assert!(stable_ids.insert(command["stableId"].as_str().unwrap()));
            assert!(tool_ids.insert(command["toolId"].as_str().unwrap()));
            assert!(icon_keys.insert(command["icon"]["key"].as_str().unwrap()));
            let svg = command["icon"]["svg"].as_str().unwrap();
            assert!(svg.starts_with("<svg class=\"wb-palette-icon\""));
            assert!(svg.ends_with("</svg>"));
            assert!(svg.contains("data-icon-key="));
            assert!(!svg.contains("<script"));
            assert!(!svg.contains("<style"));
            assert!(!svg.contains("<text"));
            assert!(!svg.contains("href="));
            assert!(!svg.contains(" on"));
        }

        let mut bridge = WorkbenchBridge::construct_json(r#"{"version":1}"#).unwrap();
        let snapshot = bridge.snapshot_json().unwrap();
        assert!(!snapshot.contains("geometry-segment"));
        assert!(!snapshot.contains("toolCatalog"));
    }
}
