// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(any(target_arch = "wasm32", test))]
mod action_surface;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod bridge;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod code_control_rpc;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod code_projects;
#[cfg(any(target_arch = "wasm32", test))]
mod command_manifest;
#[cfg(any(target_arch = "wasm32", test))]
mod design_projection;
#[cfg(any(target_arch = "wasm32", test))]
mod effect_adapter;
#[cfg(any(target_arch = "wasm32", test))]
mod geometry_palette;
#[cfg(any(target_arch = "wasm32", test))]
mod icons;
#[cfg(any(target_arch = "wasm32", test))]
mod interaction_trace;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod live_intent_rpc;
#[cfg(any(target_arch = "wasm32", test))]
mod persistence;
#[cfg(any(target_arch = "wasm32", test))]
mod samples;
#[cfg(any(target_arch = "wasm32", test))]
mod scene;

#[cfg(any(target_arch = "wasm32", test))]
const WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS: f64 = 0.25;

#[cfg(any(target_arch = "wasm32", test))]
const CANVAS_POINTER_TERMINAL_EVENTS: [&str; 3] =
    ["pointerup", "pointercancel", "lostpointercapture"];

#[cfg(any(target_arch = "wasm32", test))]
const CANVAS_PAN_POINTER_EVENTS: [&str; 3] = ["pointerdown", "pointermove", "pointerup"];

#[cfg(target_arch = "wasm32")]
const CAMERA_WHEEL_IDLE_RECONCILIATION_MS: i32 = 120;

#[cfg(any(target_arch = "wasm32", test))]
const RECENT_SAMPLE_SCHEMA_VERSION: u8 = 1;

#[cfg(any(target_arch = "wasm32", test))]
const MAX_RECENT_SAMPLES: usize = 6;

#[cfg(any(target_arch = "wasm32", test))]
const MAX_RECENT_SAMPLE_STORAGE_BYTES: usize = 8 * 1024;

/// The Problems host presents one current-attempt surface. An invalid source
/// supersedes older retained-failure detail in that surface, so coincident
/// metadata must not inflate its badge into two independently rendered items.
#[cfg(any(target_arch = "wasm32", test))]
const fn current_problem_count(source_invalid: bool, retained_failure: bool) -> usize {
    if source_invalid || retained_failure {
        1
    } else {
        0
    }
}

/// One presentation projection for the shared Problems host. Source failures
/// take precedence because they own an exact editor range, while a retained
/// intent failure remains visible whenever no current source diagnostic
/// exists. Returning to accepted source bytes therefore clears an older
/// source-only problem immediately; Apply is not required to invalidate a
/// diagnostic that authenticated different draft bytes.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct CurrentProblemPresentation {
    count: usize,
    message: String,
    opens_source_position: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
fn current_problem_presentation(
    source_diagnostic: Option<&str>,
    retained_diagnostic: Option<&str>,
) -> CurrentProblemPresentation {
    let count = current_problem_count(source_diagnostic.is_some(), retained_diagnostic.is_some());
    if let Some(diagnostic) = source_diagnostic {
        CurrentProblemPresentation {
            count,
            message: format!("Managed source · {diagnostic}"),
            opens_source_position: true,
        }
    } else if let Some(diagnostic) = retained_diagnostic {
        CurrentProblemPresentation {
            count,
            message: format!(
                "The latest intent attempt was retained but could not materialize: {diagnostic}"
            ),
            opens_source_position: false,
        }
    } else {
        CurrentProblemPresentation {
            count,
            message: "No current projectional intent problem".into(),
            opens_source_position: false,
        }
    }
}

/// One browser-local shortcut to a bundled sample. This is presentation state
/// only: it never contains project, source, checkpoint, or accepted-scene
/// authority bytes.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RecentSampleKind {
    Native,
    Code,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RecentSampleShortcut {
    kind: RecentSampleKind,
    key: String,
    title: String,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RecentSampleEnvelope {
    version: u8,
    entries: Vec<RecentSampleShortcut>,
}

#[cfg(any(target_arch = "wasm32", test))]
fn recent_sample_field_is_bounded(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

#[cfg(any(target_arch = "wasm32", test))]
fn normalize_recent_samples(entries: Vec<RecentSampleShortcut>) -> Vec<RecentSampleShortcut> {
    let mut normalized = Vec::new();
    for entry in entries {
        if !recent_sample_field_is_bounded(&entry.key, 128)
            || !entry
                .key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            || !recent_sample_field_is_bounded(&entry.title, 256)
            || normalized.iter().any(|existing: &RecentSampleShortcut| {
                existing.kind == entry.kind && existing.key == entry.key
            })
        {
            continue;
        }
        normalized.push(entry);
        if normalized.len() == MAX_RECENT_SAMPLES {
            break;
        }
    }
    normalized
}

#[cfg(any(target_arch = "wasm32", test))]
fn decode_recent_samples(encoded: &str) -> Vec<RecentSampleShortcut> {
    if encoded.len() > MAX_RECENT_SAMPLE_STORAGE_BYTES {
        return Vec::new();
    }
    let Ok(envelope) = serde_json::from_str::<RecentSampleEnvelope>(encoded) else {
        return Vec::new();
    };
    if envelope.version != RECENT_SAMPLE_SCHEMA_VERSION {
        return Vec::new();
    }
    normalize_recent_samples(envelope.entries)
}

#[cfg(any(target_arch = "wasm32", test))]
fn encode_recent_samples(entries: &[RecentSampleShortcut]) -> Result<String, String> {
    let encoded = serde_json::to_string(&RecentSampleEnvelope {
        version: RECENT_SAMPLE_SCHEMA_VERSION,
        entries: normalize_recent_samples(entries.to_vec()),
    })
    .map_err(|error| error.to_string())?;
    if encoded.len() > MAX_RECENT_SAMPLE_STORAGE_BYTES {
        return Err("recent sample shortcuts exceed their presentation bound".into());
    }
    Ok(encoded)
}

#[cfg(any(target_arch = "wasm32", test))]
fn remember_recent_sample(entries: &mut Vec<RecentSampleShortcut>, shortcut: RecentSampleShortcut) {
    entries.retain(|entry| entry.kind != shortcut.kind || entry.key != shortcut.key);
    entries.insert(0, shortcut);
    *entries = normalize_recent_samples(std::mem::take(entries));
}

#[cfg(any(target_arch = "wasm32", test))]
fn sample_search_matches(query: &str, candidate: &str) -> bool {
    let candidate = candidate.to_lowercase();
    query
        .split_whitespace()
        .map(str::to_lowercase)
        .all(|term| candidate.contains(&term))
}

#[cfg(any(target_arch = "wasm32", test))]
fn utf16_code_unit_length(value: &str) -> u32 {
    u32::try_from(value.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// One captured middle-button camera gesture shared by both browser adapters.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasPanGesture {
    pointer_id: i32,
    origin: geosolve_constraint_editor::ScreenPoint,
    origin_center: [f64; 2],
}

/// Applies the terminal pointer coordinate to the desired camera before an
/// exact retained-scene reconciliation. Pointer-up is an input sample in its
/// own right; relying on the preceding move would lose a valid final delta.
#[cfg(any(target_arch = "wasm32", test))]
fn finish_canvas_pan_camera(
    camera: &mut scene::CanvasCamera,
    gesture: CanvasPanGesture,
    terminal: Option<geosolve_constraint_editor::ScreenPoint>,
) -> bool {
    terminal
        .is_some_and(|terminal| camera.pan_from(gesture.origin_center, gesture.origin, terminal))
}

/// Shared admission contract for retained Select-hover presentation.
///
/// Both browser adapters must authenticate the same viewport and display
/// policy against their own current scene authority before the headless hover
/// state or stable SVG DOM may be updated in place.
#[cfg(any(target_arch = "wasm32", test))]
fn retained_hover_scene_is_admitted(
    scene: Option<&geosolve_constraint_editor::EditorScene>,
    viewport: geosolve_constraint_editor::Viewport,
    annotations_visible: bool,
    show_all_constraint_annotations: bool,
    is_current: impl FnOnce(&geosolve_constraint_editor::EditorScene) -> bool,
) -> bool {
    scene.is_some_and(|scene| {
        scene.viewport == viewport
            && scene.annotations_visible == annotations_visible
            && scene.show_all_constraint_annotations == show_all_constraint_annotations
            && is_current(scene)
    })
}

/// Browser presentation work admitted after one exact pointer lifecycle event.
///
/// Pointer motion is intentionally transient: it republishes the exact current
/// canvas and status surfaces, but cannot serialize the workspace or rebuild
/// panels backed by durable scene state. The authenticated terminal release is
/// the single durable presentation boundary for that gesture.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkbenchPresentationEvent {
    PointerMoveFrame,
    PointerRelease,
    AuthenticatedPointerRelease(u64),
    /// The outer code/source transaction has already published both source
    /// history and its accepted checkpoint. Presentation may persist those
    /// exact authorities, but must not attempt a second delegated checkpoint
    /// publication while saving the terminal frame.
    CodeSourcePointerRelease,
    PointerReleaseWithoutTransaction,
    InteractionCancellation,
}

#[cfg(any(target_arch = "wasm32", test))]
impl WorkbenchPresentationEvent {
    const fn policy(self) -> WorkbenchPresentationPolicy {
        match self {
            Self::PointerMoveFrame => WorkbenchPresentationPolicy {
                render_scope: WorkbenchRenderScope::Transient,
                saves_workspace: false,
            },
            Self::PointerRelease
            | Self::AuthenticatedPointerRelease(_)
            | Self::CodeSourcePointerRelease => WorkbenchPresentationPolicy {
                render_scope: WorkbenchRenderScope::Durable,
                saves_workspace: true,
            },
            Self::PointerReleaseWithoutTransaction | Self::InteractionCancellation => {
                WorkbenchPresentationPolicy {
                    render_scope: WorkbenchRenderScope::Durable,
                    saves_workspace: false,
                }
            }
        }
    }

    const fn code_source_already_published(self) -> bool {
        matches!(self, Self::CodeSourcePointerRelease)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkbenchRenderScope {
    Transient,
    Durable,
}

/// Closed presentation-only camera commands shared by both browser adapters.
///
/// Keeping the command semantics outside either DOM callback makes route
/// parity testable without a browser and prevents toolbar navigation from
/// drifting back into workspace persistence or durable scene rendering.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CameraToolbarAction {
    ZoomIn,
    ZoomOut,
    Fit,
    Origin,
}

#[cfg(any(target_arch = "wasm32", test))]
impl CameraToolbarAction {
    #[cfg(test)]
    const ALL: [Self; 4] = [Self::ZoomIn, Self::ZoomOut, Self::Fit, Self::Origin];

    fn from_workbench_action(action: &str) -> Option<Self> {
        match action {
            "zoom-in" => Some(Self::ZoomIn),
            "zoom-out" => Some(Self::ZoomOut),
            "zoom-fit" => Some(Self::Fit),
            "zoom-origin" => Some(Self::Origin),
            _ => None,
        }
    }

    fn apply(
        self,
        camera: &mut scene::CanvasCamera,
        scene: Option<&geosolve_constraint_editor::EditorScene>,
    ) -> CameraToolbarOutcome {
        let before = *camera;
        let fitted_geometry = match self {
            Self::ZoomIn => {
                camera.zoom_about(
                    geosolve_constraint_editor::ScreenPoint { x: 500.0, y: 350.0 },
                    1.25,
                );
                None
            }
            Self::ZoomOut => {
                camera.zoom_about(
                    geosolve_constraint_editor::ScreenPoint { x: 500.0, y: 350.0 },
                    0.8,
                );
                None
            }
            Self::Fit => Some(camera.fit_scene_or_reset(scene)),
            Self::Origin => {
                camera.center_origin();
                None
            }
        };
        CameraToolbarOutcome {
            changed: *camera != before,
            fitted_geometry,
        }
    }

    fn notice(self, outcome: CameraToolbarOutcome) -> &'static str {
        match (self, outcome.fitted_geometry) {
            (Self::ZoomIn, _) => "Canvas zoomed in",
            (Self::ZoomOut, _) => "Canvas zoomed out",
            (Self::Fit, Some(true)) => "View fitted to sketch geometry",
            (Self::Fit, Some(false)) => "Empty sketch reset to the Origin view",
            (Self::Origin, _) => "View centred on Origin",
            (Self::Fit, None) => unreachable!("Fit always records whether geometry was fitted"),
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CameraToolbarOutcome {
    changed: bool,
    fitted_geometry: Option<bool>,
}

/// Scheduling authority returned by a presentation-only toolbar command.
/// Camera controls may schedule retained paint and exact idle reconciliation,
/// but can never authorize persistence or a durable render.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RetainedCameraAdmission {
    frame_generation: Option<u64>,
    idle_generation: Option<u64>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl RetainedCameraAdmission {
    #[cfg(test)]
    #[allow(
        clippy::unused_self,
        reason = "the assertion reads as policy on one admission"
    )]
    const fn saves_workspace(self) -> bool {
        false
    }

    #[cfg(test)]
    #[allow(
        clippy::unused_self,
        reason = "the assertion reads as policy on one admission"
    )]
    const fn durable_render_scope(self) -> Option<WorkbenchRenderScope> {
        None
    }
}

/// Non-semantic projection selected in the Design panel.
///
/// This is deliberately presentation state: changing tabs cannot alter graph,
/// instance, organization, external-input, or accepted-materialization identity.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DesignProjectionTab {
    Outline,
    StructuredSource,
    History,
    Code,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DesignProjectionTab {
    const ALL: [Self; 4] = [
        Self::Outline,
        Self::StructuredSource,
        Self::History,
        Self::Code,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::StructuredSource => "source",
            Self::History => "history",
            Self::Code => "code",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.key() == key)
    }

    const fn button_id(self) -> &'static str {
        match self {
            Self::Outline => "wb-design-tab-outline",
            Self::StructuredSource => "wb-design-tab-source",
            Self::History => "wb-design-tab-history",
            Self::Code => "wb-design-tab-code",
        }
    }

    const fn panel_id(self) -> &'static str {
        match self {
            Self::Outline => "wb-design-outline",
            Self::StructuredSource => "wb-design-source",
            Self::History => "wb-design-history",
            Self::Code => "wb-design-code",
        }
    }

    const fn adjacent(self, direction: i8) -> Self {
        match (self, direction.signum()) {
            (Self::Outline, -1) => Self::Code,
            (Self::StructuredSource, -1) | (Self::Code, 1) => Self::Outline,
            (Self::Outline, 1) | (Self::History, -1) => Self::StructuredSource,
            (Self::StructuredSource, 1) | (Self::Code, -1) => Self::History,
            (Self::History, 1) => Self::Code,
            (_, _) => self,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl WorkbenchRenderScope {
    const fn rebuilds_durable_panels(self) -> bool {
        matches!(self, Self::Durable)
    }
}

/// Coalesces presentation-only camera input independently from semantic
/// pointer motion. `camera` on the workbench remains the desired camera; this
/// owner records which camera the retained SVG was painted for and
/// authenticates animation-frame and wheel-idle callbacks by generation.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Default)]
struct RetainedCameraQueue {
    exact_camera: scene::CanvasCamera,
    pending_camera: Option<scene::CanvasCamera>,
    next_frame_generation: u64,
    scheduled_frame_generation: Option<u64>,
    admitted_frame_generation: Option<u64>,
    next_idle_generation: u64,
    scheduled_idle_generation: Option<u64>,
    exact_reconciliation_needed: bool,
}

/// One generation-authenticated camera paint that has not yet been recorded
/// as presented. Browser adapters complete it only after every retained DOM
/// mutation succeeds, so failed and stale callbacks remain truthful zeros in
/// the actual-work ledger.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct AdmittedCameraFrame {
    generation: u64,
    target_camera: scene::CanvasCamera,
    transform: scene::RetainedCameraTransform,
}

#[cfg(any(target_arch = "wasm32", test))]
impl AdmittedCameraFrame {
    fn complete(self, queue: &mut RetainedCameraQueue) -> bool {
        queue.complete_frame(self)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl RetainedCameraQueue {
    /// Retains the newest desired camera and schedules at most one RAF.
    fn request_frame(&mut self, camera: scene::CanvasCamera) -> Option<u64> {
        self.pending_camera = Some(camera);
        self.exact_reconciliation_needed |= camera != self.exact_camera;
        if self.scheduled_frame_generation.is_some() {
            return None;
        }
        self.next_frame_generation = self.next_frame_generation.wrapping_add(1);
        self.scheduled_frame_generation = Some(self.next_frame_generation);
        Some(self.next_frame_generation)
    }

    /// Admits only the currently authenticated RAF; stale callbacks are inert
    /// and the latest camera wins over every raw event it coalesced. The
    /// desired camera remains pending until presentation completes, so a DOM
    /// failure cannot consume the only retryable copy.
    fn take_frame(&mut self, generation: u64) -> Option<AdmittedCameraFrame> {
        if self.scheduled_frame_generation != Some(generation) {
            return None;
        }
        self.scheduled_frame_generation = None;
        let target_camera = self.pending_camera?;
        let transform = scene::RetainedCameraTransform::between(self.exact_camera, target_camera)?;
        self.admitted_frame_generation = Some(generation);
        Some(AdmittedCameraFrame {
            generation,
            target_camera,
            transform,
        })
    }

    /// Commits an admitted frame only after every retained DOM mutation has
    /// succeeded. A newer desired camera, if one was coalesced meanwhile, is
    /// deliberately retained for its own frame.
    fn complete_frame(&mut self, frame: AdmittedCameraFrame) -> bool {
        if self.admitted_frame_generation != Some(frame.generation) {
            return false;
        }
        self.admitted_frame_generation = None;
        if self.pending_camera == Some(frame.target_camera) {
            self.pending_camera = None;
        }
        true
    }

    #[cfg_attr(test, allow(dead_code))]
    fn cancel_frame(&mut self, generation: u64) {
        if self.scheduled_frame_generation == Some(generation) {
            self.scheduled_frame_generation = None;
        }
    }

    /// Restarts the short wheel-idle boundary. Older timers remain allocated
    /// by the browser but cannot authorize an exact scene reconstruction.
    fn request_idle_reconciliation(&mut self) -> u64 {
        self.next_idle_generation = self.next_idle_generation.wrapping_add(1);
        self.scheduled_idle_generation = Some(self.next_idle_generation);
        self.next_idle_generation
    }

    /// Admits one toolbar camera mutation to the same retained queue used by
    /// pan and wheel input. A no-op command updates its status synchronously in
    /// the adapter and schedules no presentation work.
    fn admit_toolbar_change(
        &mut self,
        camera: scene::CanvasCamera,
        changed: bool,
    ) -> RetainedCameraAdmission {
        if !changed {
            return RetainedCameraAdmission::default();
        }
        RetainedCameraAdmission {
            frame_generation: self.request_frame(camera),
            idle_generation: Some(self.request_idle_reconciliation()),
        }
    }

    fn take_idle_reconciliation(&mut self, generation: u64) -> bool {
        if self.scheduled_idle_generation != Some(generation) {
            return false;
        }
        self.scheduled_idle_generation = None;
        self.exact_reconciliation_needed
    }

    const fn needs_exact_reconciliation(&self) -> bool {
        self.exact_reconciliation_needed
    }

    #[cfg_attr(
        test,
        allow(dead_code, reason = "browser-only semantic input reconciliation")
    )]
    fn require_exact_reconciliation(&mut self) {
        self.exact_reconciliation_needed = true;
    }

    /// Publishes a new exact base and revokes every callback created for the
    /// previous retained group. The browser closures may still run, but their
    /// generations no longer match this owner.
    fn exact_reconciled(&mut self, camera: scene::CanvasCamera) {
        self.exact_camera = camera;
        self.pending_camera = None;
        self.scheduled_frame_generation = None;
        self.admitted_frame_generation = None;
        self.scheduled_idle_generation = None;
        self.exact_reconciliation_needed = false;
    }
}

/// One frame's projectional canvas authority and any presentation-only failure.
///
/// Scene-composition failures must not be collapsed into the same `None` used
/// for a legitimate history position with no accepted authority. The error is
/// deliberately frame-local rather than copied into the durable workbench
/// notice, so the next successful composition restores the ordinary status.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
struct ProjectionalScenePresentation {
    scene: Option<geosolve_constraint_editor::EditorScene>,
    status_override: Option<String>,
    work: geosolve_constraint_editor::InteractionWorkReceipt,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ProjectionalScenePresentation {
    fn from_editor(
        editor: &geosolve_constraint_editor::ProjectionalEditorSession,
        viewport: geosolve_constraint_editor::Viewport,
        chord_tolerance_pixels: f64,
        annotations_visible: bool,
        show_all_constraints: bool,
    ) -> Self {
        let audited = editor.scene_audited(viewport, chord_tolerance_pixels);
        let work = audited.work;
        match audited.outcome {
            Ok(mut scene) => {
                apply_projectional_scene_display(
                    &mut scene,
                    annotations_visible,
                    show_all_constraints,
                );
                Self {
                    scene: Some(scene),
                    status_override: None,
                    work,
                }
            }
            Err(geosolve_constraint_editor::ProjectionalEditorError::NoAcceptedAuthority) => Self {
                scene: None,
                status_override: None,
                work,
            },
            Err(error) => Self {
                scene: None,
                status_override: Some(format!("Canvas scene unavailable: {error}")),
                work,
            },
        }
    }
}
/// Exactly one durable document/history authority installed in the workbench.
///
/// Strict v1-v6 workspaces and in-process samples normalize into a history-free
/// projectional editor. A canonical v8 workspace installs only the projectional
/// editor session whose native materialization was independently cold-authenticated.
#[cfg(any(target_arch = "wasm32", test))]
enum WorkbenchDocumentAuthority {
    Flat(Box<geosolve_constraint_editor::RetainedEditorCoordinator>),
    Projectional {
        editor: Box<geosolve_constraint_editor::ProjectionalEditorSession>,
        computed_evaluation_high_water:
            geosolve_constraint_editor::ComputedEvaluationAllocatorHighWater,
        revisions: persistence::WorkspaceRevisions,
    },
}

#[cfg(any(target_arch = "wasm32", test))]
impl WorkbenchDocumentAuthority {
    fn from_snapshot(snapshot: &persistence::WorkspaceSnapshot) -> Result<Self, String> {
        let computed_evaluation_high_water = snapshot.computed_evaluation_high_water();
        let revisions = snapshot.revisions;
        if snapshot.intent_session()?.is_some() {
            persistence::projectional_editor_from_snapshot(snapshot)
                .map(|editor| Self::projectional(editor, computed_evaluation_high_water, revisions))
        } else if snapshot.legacy_bootstrap().is_some() {
            persistence::projectional_editor_from_legacy_snapshot(snapshot)
                .map(|editor| Self::projectional(editor, computed_evaluation_high_water, revisions))
        } else {
            persistence::coordinator_from_snapshot(snapshot)
                .map(Box::new)
                .map(Self::Flat)
        }
    }

    #[cfg_attr(test, allow(dead_code, reason = "used by the WASM startup adapter"))]
    fn flat(coordinator: geosolve_constraint_editor::RetainedEditorCoordinator) -> Self {
        Self::Flat(Box::new(coordinator))
    }

    fn projectional(
        editor: geosolve_constraint_editor::ProjectionalEditorSession,
        computed_evaluation_high_water: geosolve_constraint_editor::ComputedEvaluationAllocatorHighWater,
        revisions: persistence::WorkspaceRevisions,
    ) -> Self {
        Self::Projectional {
            editor: Box::new(editor),
            computed_evaluation_high_water,
            revisions,
        }
    }

    #[cfg_attr(
        test,
        allow(dead_code, reason = "used by the WASM code-project adapter")
    )]
    fn from_projectional_editor(
        editor: geosolve_constraint_editor::ProjectionalEditorSession,
    ) -> Result<Self, String> {
        let (computed_evaluation_high_water, revisions) =
            persistence::WorkspaceSnapshot::projectional_authority_metadata(&editor)?;
        Ok(Self::projectional(
            editor,
            computed_evaluation_high_water,
            revisions,
        ))
    }

    fn from_flat_coordinator(
        coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
    ) -> Result<Self, String> {
        let checkpoint = coordinator
            .persistence_checkpoint()
            .map_err(|error| error.to_string())?;
        let computed_evaluation_high_water = checkpoint.computed_evaluation_high_water();
        let revisions = checkpoint.revisions();
        let editor = persistence::projectional_editor_from_flat_coordinator(coordinator)?;
        Ok(Self::projectional(
            editor,
            computed_evaluation_high_water,
            persistence::WorkspaceRevisions {
                design: revisions.design().get(),
                attempt: revisions.attempt().get(),
                accepted: revisions
                    .accepted()
                    .map(geosolve_sketch::SketchAcceptedRevision::get),
            },
        ))
    }

    const fn is_projectional(&self) -> bool {
        matches!(self, Self::Projectional { .. })
    }

    const fn flat_ref(&self) -> Option<&geosolve_constraint_editor::RetainedEditorCoordinator> {
        match self {
            Self::Flat(coordinator) => Some(coordinator),
            Self::Projectional { .. } => None,
        }
    }

    fn projectional_mut(
        &mut self,
    ) -> Option<&mut geosolve_constraint_editor::ProjectionalEditorSession> {
        match self {
            Self::Flat(_) => None,
            Self::Projectional { editor, .. } => Some(editor),
        }
    }

    const fn projectional_ref(
        &self,
    ) -> Option<&geosolve_constraint_editor::ProjectionalEditorSession> {
        match self {
            Self::Flat(_) => None,
            Self::Projectional { editor, .. } => Some(editor),
        }
    }

    fn snapshot(&self) -> Result<persistence::WorkspaceSnapshot, String> {
        match self {
            Self::Flat(coordinator) => {
                persistence::WorkspaceSnapshot::from_coordinator(coordinator)
            }
            Self::Projectional {
                editor,
                computed_evaluation_high_water,
                revisions,
            } => {
                persistence::WorkspaceSnapshot::from_projectional_editor_with_persistence_high_water(
                    editor,
                    *computed_evaluation_high_water,
                    *revisions,
                )
            }
        }
    }

    fn scene_presentation(
        &self,
        viewport: geosolve_constraint_editor::Viewport,
        chord_tolerance_pixels: f64,
    ) -> ProjectionalScenePresentation {
        match self {
            Self::Flat(coordinator) => ProjectionalScenePresentation {
                scene: compose_editor_scene(coordinator, viewport, chord_tolerance_pixels),
                status_override: None,
                work: geosolve_constraint_editor::InteractionWorkReceipt::default(),
            },
            Self::Projectional { editor, .. } => ProjectionalScenePresentation::from_editor(
                editor,
                viewport,
                chord_tolerance_pixels,
                true,
                false,
            ),
        }
    }

    #[cfg_attr(
        test,
        allow(dead_code, reason = "browser-only retained scene authentication")
    )]
    fn retained_scene_is_current(&self, scene: &geosolve_constraint_editor::EditorScene) -> bool {
        match self {
            Self::Flat(coordinator) => {
                use geosolve_constraint_editor::ComputedSceneState;

                let source = coordinator
                    .visible_preview_session()
                    .unwrap_or(coordinator.session());
                if !scene.belongs_to_retained_session(source) {
                    return false;
                }
                if source.accepted_state_for_current_input().is_none() {
                    return scene.computed_input.is_none();
                }
                match coordinator.computed_scene_state() {
                    ComputedSceneState::Current { expected, .. } => {
                        scene.computed_input.as_ref() == Some(expected)
                    }
                    ComputedSceneState::Withheld | ComputedSceneState::Absent => {
                        scene.computed_input.is_none()
                    }
                }
            }
            Self::Projectional { editor, .. } => editor.scene_is_current(scene),
        }
    }

    fn step_history(&mut self, undo: bool) -> Result<bool, String> {
        match self {
            Self::Flat(coordinator) => if undo {
                coordinator.undo()
            } else {
                coordinator.redo()
            }
            .map(|()| true)
            .map_err(|error| error.to_string()),
            Self::Projectional { editor, .. } => if undo { editor.undo() } else { editor.redo() }
                .map(|moved| moved.is_some())
                .map_err(|error| error.to_string()),
        }
    }
}

/// Fits one newly installed accepted authority through the same composed scene
/// that the canvas will render. Empty or unavailable authority deliberately
/// falls back to the canonical Origin camera.
#[cfg(any(target_arch = "wasm32", test))]
fn fit_projectional_camera_to_authority(
    camera: &mut scene::CanvasCamera,
    authority: &WorkbenchDocumentAuthority,
) -> bool {
    let presentation =
        authority.scene_presentation(camera.viewport(), WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS);
    camera.fit_scene_or_reset(presentation.scene.as_ref())
}

/// Builds the canonical empty projectional workspace used by startup and New.
///
/// Keeping both routes on this shared boundary prevents the browser action
/// from falling back to the retired flat event path or creating a second
/// document/history authority.
#[cfg(any(target_arch = "wasm32", test))]
fn fresh_projectional_authority() -> Result<WorkbenchDocumentAuthority, String> {
    let document = geosolve_sketch::SketchDocument::new(10.0).map_err(|error| error.to_string())?;
    let session = geosolve_sketch::RetainedSketchDocumentSession::new(
        document,
        geosolve_sketch::DocumentSolveRequest::default(),
        geosolve_core::SolverConfig::default(),
    )
    .map_err(|error| error.to_string())?;
    let coordinator = geosolve_constraint_editor::RetainedEditorCoordinator::new(session)
        .map_err(|error| error.to_string())?;
    WorkbenchDocumentAuthority::from_flat_coordinator(&coordinator)
}

/// The established flat workbench remains intentionally unchanged behind its
/// own listener installation. Dereferencing is therefore available only to
/// that flat-only adapter; projectional startup installs a disjoint event path.
#[cfg(target_arch = "wasm32")]
impl std::ops::Deref for WorkbenchDocumentAuthority {
    type Target = geosolve_constraint_editor::RetainedEditorCoordinator;

    fn deref(&self) -> &Self::Target {
        self.flat_ref()
            .expect("flat-only workbench adapter received projectional authority")
    }
}

#[cfg(target_arch = "wasm32")]
impl std::ops::DerefMut for WorkbenchDocumentAuthority {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Flat(coordinator) => coordinator,
            Self::Projectional { .. } => {
                panic!("flat-only workbench adapter received projectional authority")
            }
        }
    }
}

/// Browser-ready markup derived only from editor-owned projection DTOs.
#[cfg(any(target_arch = "wasm32", test))]
struct ProjectionalDesignMarkup {
    declaration_count: usize,
    outline: String,
    source: String,
    history: String,
    inspector: String,
}

#[cfg(any(target_arch = "wasm32", test))]
fn projectional_design_markup(
    projectional: &geosolve_constraint_editor::ProjectionalEditorSession,
    code_project: Option<&code_projects::CodeProjectWorkbench>,
    managed_controls: Option<Result<&geosolve_sketch_code::ManagedControlManifest, &str>>,
) -> Result<ProjectionalDesignMarkup, String> {
    let projection = projectional.workbench_projection();
    let selection = projectional
        .selected_declaration()
        .map(|node| design_projection::DesignProjectionSelection { node });
    let inspector = projectional.selected_inspector(&projection);
    let descriptor_index = inspector
        .as_ref()
        .map(design_projection::InspectorDescriptorIndex::new);
    let parameters = match (code_project, inspector.as_ref(), managed_controls) {
        (Some(code_project), Some(inspector), Some(manifest)) => code_project
            .inspector_parameter_presentations_with_manifest(
                projectional,
                &projection,
                inspector,
                descriptor_index
                    .as_ref()
                    .expect("selected Inspector has one descriptor index"),
                manifest,
            )?,
        (Some(code_project), Some(inspector), None) => {
            code_project.inspector_parameter_presentations(projectional, inspector)?
        }
        (Some(_), None, _) | (None, _, _) => Vec::new(),
    };
    let name_authority = design_projection::inspector_name_authority(&parameters);
    Ok(ProjectionalDesignMarkup {
        declaration_count: design_projection::declaration_count(&projection),
        outline: design_projection::outline_markup(&projection, selection),
        source: design_projection::structured_source_markup(&projection, selection),
        history: design_projection::history_markup(&projection),
        inspector: inspector.as_ref().map_or_else(String::new, |inspector| {
            design_projection::inspector_markup_with_presentation_and_descriptors(
                inspector,
                design_projection::InspectorPresentation {
                    parameters: &parameters,
                    name_authority: name_authority.as_ref(),
                },
                descriptor_index
                    .as_ref()
                    .expect("selected Inspector has one descriptor index"),
            )
        }),
    })
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkbenchPresentationPolicy {
    render_scope: WorkbenchRenderScope,
    saves_workspace: bool,
}

/// Deterministic audit counters for the browser presentation policy.
///
/// These counters intentionally model admitted work rather than wall-clock
/// timing, so the regression remains stable on native and WASM builders.
#[cfg(test)]
#[derive(Default, Debug, Eq, PartialEq)]
struct WorkbenchPresentationCounters {
    transient_renders: usize,
    durable_renders: usize,
    workspace_saves: usize,
    durable_panel_rebuilds: usize,
}

#[cfg(test)]
impl WorkbenchPresentationCounters {
    fn record(&mut self, event: WorkbenchPresentationEvent) {
        let policy = event.policy();
        if policy.saves_workspace {
            self.workspace_saves += 1;
        }
        match policy.render_scope {
            WorkbenchRenderScope::Transient => self.transient_renders += 1,
            WorkbenchRenderScope::Durable => self.durable_renders += 1,
        }
        if policy.render_scope.rebuilds_durable_panels() {
            self.durable_panel_rebuilds += 1;
        }
    }
}

/// Browser-side result of consuming headless projectional geometry effects.
///
/// Preview and inference effects are already reflected in the disposable
/// [`geosolve_constraint_editor::ConstraintEditor`]. Only the authenticated
/// construction terminal may cross into durable intent, and it does so through
/// [`geosolve_constraint_editor::ProjectionalEditorSession`] exactly once.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProjectionalConstructionDispatch {
    accepted_terminal: bool,
    rejected_terminal: bool,
    error: Option<String>,
}

#[cfg(any(target_arch = "wasm32", test))]
fn dispatch_projectional_construction_effects(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    preview: &mut Option<geosolve_constraint_editor::ConstructionPreview>,
    effects: Vec<geosolve_constraint_editor::EditorEffect>,
) -> ProjectionalConstructionDispatch {
    use geosolve_constraint_editor::EditorEffect;

    let mut outcome = ProjectionalConstructionDispatch::default();
    let mut pending = std::collections::VecDeque::from(effects);
    while let Some(effect) = pending.pop_front() {
        match effect {
            EditorEffect::PreviewConstruction(next) => *preview = Some(next),
            EditorEffect::ClearConstructionPreview => *preview = None,
            EditorEffect::CommitConstructionPlan { .. } => {
                match editor.apply_construction_editor_effect(&effect) {
                    Ok(terminal) => {
                        outcome.accepted_terminal = true;
                        pending.extend(terminal.effects);
                    }
                    Err(error) => {
                        outcome.rejected_terminal = true;
                        outcome.error = Some(error.to_string());
                    }
                }
            }
            EditorEffect::CommitConstruction { .. } => {
                outcome.rejected_terminal = true;
                outcome.error = Some(
                    "projectional geometry requires an authenticated construction plan".into(),
                );
            }
            EditorEffect::DraftInferenceChanged(_)
            | EditorEffect::SelectionChanged(_)
            | EditorEffect::HoverChanged(_)
            | EditorEffect::PreviewPointMove { .. }
            | EditorEffect::ClearPointPreview
            | EditorEffect::PreviewCurveControl { .. }
            | EditorEffect::ClearCurveControlPreview
            | EditorEffect::FilletBranchPreviewChanged { .. }
            | EditorEffect::ClearComputedFeaturePreview
            | EditorEffect::ClearComputedFeatureContactPreview
            // Projectional cancellation has already discarded the
            // history-free property previews, and the bridge exits any
            // provisional Offset state immediately after this dispatch.
            // These are cleanup acknowledgements, not durable requests.
            | EditorEffect::RestoreComputedFeatureRadius { .. }
            | EditorEffect::RestoreComputedFeatureContact { .. }
            | EditorEffect::RestoreOffsetAuthoringDistance { .. }
            | EditorEffect::ClearAcceptedProfileOffsetPreview => {}
            EditorEffect::RequestProjectedPointMove { .. }
            | EditorEffect::CommitPointMove { .. }
            | EditorEffect::RequestCurveControlPreview { .. }
            | EditorEffect::CommitCurveControl { .. }
            | EditorEffect::PreviewOffsetAuthoringDistance { .. }
            | EditorEffect::FinishOffsetAuthoringDistance { .. }
            | EditorEffect::PreviewAcceptedProfileOffsetDistance { .. }
            | EditorEffect::CommitAcceptedProfileOffsetDistance { .. }
            | EditorEffect::PreviewComputedFeatureRadius { .. }
            | EditorEffect::CommitComputedFeatureRadius { .. }
            | EditorEffect::PreviewComputedFeatureContact { .. }
            | EditorEffect::CommitComputedFeatureContact { .. }
            | EditorEffect::CommitComputedFilletAction { .. } => {
                outcome.rejected_terminal = true;
                outcome.error = Some(
                    "the geometry-authoring adapter received an unrelated durable effect".into(),
                );
            }
        }
    }
    outcome
}

/// Browser-side result of one complete relation or dimension application.
///
/// Operand collection and hover remain disposable
/// [`geosolve_constraint_editor::AuthoringState`] state.
/// Only a complete application can reach the projectional coordinator, where
/// both an accepted declaration and retained-invalid explicit intent become
/// exactly one durable history transaction.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProjectionalAuthoringDispatch {
    disposition: Option<geosolve_sketch_intent::IntentPlanDisposition>,
    error: Option<String>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ProjectionalAuthoringDispatch {
    const fn committed(&self) -> bool {
        self.disposition.is_some()
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn dispatch_projectional_authoring_application(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    authoring: &mut geosolve_constraint_editor::AuthoringState,
    application: &geosolve_constraint_editor::AuthoringApplication,
) -> ProjectionalAuthoringDispatch {
    let result = editor.apply_authoring_application(application);
    authoring.transaction_finished();
    if let Some(document) = editor
        .coordinator()
        .presentation_session()
        .map(geosolve_sketch::RetainedSketchDocumentSession::design_document)
    {
        let _ = authoring.reconcile(document);
    }
    match result {
        Ok(outcome) => ProjectionalAuthoringDispatch {
            disposition: Some(outcome.disposition),
            error: None,
        },
        Err(error) => ProjectionalAuthoringDispatch {
            disposition: None,
            error: Some(error.to_string()),
        },
    }
}

/// Exact durable-projection stamp copied onto one rendered Inspector.
///
/// The session digest covers every independently revisioned intent component,
/// accepted authority, history cursor and allocator high-water. A detached or
/// stale browser control therefore cannot be rebound to a freshly generated
/// Inspector merely because its visible field strings still happen to match.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProjectionalInspectorStamp {
    session: Option<String>,
    revision: Option<String>,
    digest: Option<String>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ProjectionalInspectorStamp {
    fn matches(&self, identity: geosolve_sketch_intent::IntentSessionIdentity) -> bool {
        let session = identity.session.to_string();
        let revision = identity.revision.to_string();
        let digest = identity.digest.to_string();
        self.session.as_deref() == Some(session.as_str())
            && self.revision.as_deref() == Some(revision.as_str())
            && self.digest.as_deref() == Some(digest.as_str())
    }
}

/// Browser value carried by one terminal Inspector control event.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectionalInspectorSubmission {
    Text(String),
    Checked(bool),
    Point {
        x: Option<String>,
        y: Option<String>,
    },
}

/// Untrusted DOM coordinate submitted by one schema-generated Inspector form.
///
/// Every string is deliberately retained until it is compared against the
/// current Rust projection. DOM attributes never become arbitrary graph
/// addresses and never choose a literal kind or unit by themselves.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectionalInspectorControl {
    stamp: ProjectionalInspectorStamp,
    edit: Option<String>,
    node: Option<String>,
    field: Option<String>,
    port: Option<String>,
    leaf: Option<String>,
    port_kind: Option<String>,
    schema: Option<String>,
    unit: Option<String>,
    component: Option<String>,
    submission: ProjectionalInspectorSubmission,
}

/// Publication policy for a browser Inspector terminal.
///
/// Only `Committed` may save the v8 workspace or rebuild durable panels as a
/// new transaction. Retained-invalid intent is still a committed transaction;
/// its disposition truthfully preserves the prior accepted canvas authority.
/// A code-owned accepted edit replaces the complete workbench authority so
/// persistence high-water metadata cannot trail the delegated editor.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectionalInspectorDispatch {
    Unchanged,
    Committed(geosolve_sketch_intent::IntentPlanDisposition),
    Rejected(String),
}

/// Authenticates and delegates a selected source-owned Fillet radius before
/// the first pointer frame. `false` leaves ordinary GUI-owned Fillets on their
/// existing direct-manipulation route.
#[cfg(any(target_arch = "wasm32", test))]
fn delegate_projectional_code_fillet_radius_drag(
    authority: &mut WorkbenchDocumentAuthority,
    code_project: &code_projects::CodeProjectWorkbench,
) -> Result<bool, String> {
    let features = {
        let editor = authority.projectional_ref().ok_or_else(|| {
            "the grouped Fillet-radius gesture requires projectional authority".to_owned()
        })?;
        code_project.delegated_computed_fillet_radius_group(editor)?
    };
    let Some(features) = features else {
        return Ok(false);
    };
    authority
        .projectional_mut()
        .expect("projectional authority was authenticated above")
        .delegate_computed_fillet_radius_drag(&features)
        .map_err(|error| error.to_string())?;
    Ok(true)
}

#[cfg(any(target_arch = "wasm32", test))]
impl ProjectionalInspectorDispatch {
    const fn saves_workspace(&self) -> bool {
        matches!(self, Self::Committed(_))
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn dispatch_projectional_inspector_control(
    authority: &mut WorkbenchDocumentAuthority,
    control: &ProjectionalInspectorControl,
) -> ProjectionalInspectorDispatch {
    let Some(editor) = authority.projectional_ref() else {
        return ProjectionalInspectorDispatch::Rejected(
            "the Inspector requires projectional editor authority".into(),
        );
    };
    let projection = editor.workbench_projection();
    if !control.stamp.matches(projection.identity) {
        return ProjectionalInspectorDispatch::Rejected(
            "the Inspector control belongs to a stale design projection".into(),
        );
    }
    let Some(inspector) = editor.selected_inspector(&projection) else {
        return ProjectionalInspectorDispatch::Rejected(
            "the Inspector declaration is no longer selected".into(),
        );
    };
    let decoded = match decode_projectional_inspector_control(&inspector, control) {
        Ok(decoded) => decoded,
        Err(error) => return ProjectionalInspectorDispatch::Rejected(error),
    };
    let Some((target, value)) = decoded else {
        return ProjectionalInspectorDispatch::Unchanged;
    };

    let Some(editor) = authority.projectional_mut() else {
        return ProjectionalInspectorDispatch::Rejected(
            "the Inspector requires projectional editor authority".into(),
        );
    };
    match editor.edit_inspector(&inspector, &target, value) {
        Ok(outcome) => ProjectionalInspectorDispatch::Committed(outcome.disposition),
        Err(error) => ProjectionalInspectorDispatch::Rejected(error.to_string()),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive browser-boundary decoder keeps every authenticated Inspector coordinate family reviewable"
)]
fn decode_projectional_inspector_control(
    inspector: &geosolve_constraint_editor::IntentInspectorProjection,
    control: &ProjectionalInspectorControl,
) -> Result<
    Option<(
        geosolve_constraint_editor::IntentInspectorEditTarget,
        geosolve_constraint_editor::IntentInspectorEditValue,
    )>,
    String,
> {
    use geosolve_constraint_editor::{
        IntentInspectorEditTarget, IntentInspectorEditValue, IntentInspectorField,
    };
    use geosolve_sketch_intent::{IntentFieldKey, IntentKey, LeafField, LeafRef, PortId};

    let node = control
        .node
        .as_deref()
        .ok_or_else(|| "the Inspector control is missing its declaration coordinate".to_owned())?
        .parse()
        .map_err(|_| "the Inspector declaration coordinate is malformed".to_owned())?;
    if node != inspector.node {
        return Err("the Inspector control addresses a different declaration".into());
    }

    match control.edit.as_deref() {
        Some("suppressed") => {
            if control.field.is_some()
                || control.port.is_some()
                || control.leaf.is_some()
                || control.port_kind.is_some()
                || control.component.is_some()
                || control.unit.is_some()
                || control.schema.as_deref() != Some("boolean")
            {
                return Err("the suppression control coordinate is malformed".into());
            }
            let ProjectionalInspectorSubmission::Checked(suppressed) = &control.submission else {
                return Err("the suppression control did not submit a boolean".into());
            };
            if *suppressed == inspector.suppressed {
                return Ok(None);
            }
            Ok(Some((
                IntentInspectorEditTarget::Suppressed,
                IntentInspectorEditValue::Suppressed {
                    suppressed: *suppressed,
                },
            )))
        }
        Some("definition") => {
            if control.port.is_some() || control.leaf.is_some() || control.port_kind.is_some() {
                return Err("the definition control contains an instance coordinate".into());
            }
            let field = IntentFieldKey(
                IntentKey::new(control.field.clone().ok_or_else(|| {
                    "the definition control is missing its field coordinate".to_owned()
                })?)
                .map_err(|error| error.to_string())?,
            );
            let current = inspector
                .fields
                .iter()
                .find_map(|candidate| match candidate {
                    IntentInspectorField::Definition {
                        definition: candidate,
                        value,
                    } if *candidate == field => Some(value.as_ref()),
                    _ => None,
                })
                .ok_or_else(|| {
                    "the definition control is not present in the current schema".to_owned()
                })?;
            let schema = inspector
                .definition_descriptor(&field)
                .ok_or_else(|| {
                    "the definition control has no central declaration descriptor".to_owned()
                })?
                .schema
                .literal;
            let literal = decode_projectional_inspector_literal(schema, control)?;
            if current == Some(&literal) {
                return Ok(None);
            }
            Ok(Some((
                IntentInspectorEditTarget::Definition { field },
                IntentInspectorEditValue::Literal { literal },
            )))
        }
        Some("instance") => {
            if control.field.is_some() {
                return Err("the instance control contains a definition coordinate".into());
            }
            let port: PortId = control
                .port
                .as_deref()
                .ok_or_else(|| "the instance control is missing its port coordinate".to_owned())?
                .parse()
                .map_err(|_| "the instance port coordinate is malformed".to_owned())?;
            let field = match control.leaf.as_deref() {
                Some("x") => LeafField::X,
                Some("y") => LeafField::Y,
                Some("value") => LeafField::Value,
                Some("angle") => LeafField::Angle,
                Some("weight") => LeafField::Weight,
                Some("parameter") => LeafField::Parameter,
                _ => return Err("the instance leaf coordinate is malformed".into()),
            };
            let leaf = LeafRef { node, port, field };
            let current = inspector
                .fields
                .iter()
                .find_map(|candidate| match candidate {
                    IntentInspectorField::Instance {
                        leaf: candidate,
                        value,
                    } if *candidate == leaf => Some(value.as_ref()),
                    _ => None,
                })
                .ok_or_else(|| "the instance control is not a current writable leaf".to_owned())?;
            let port_kind = inspector
                .output_descriptor(leaf)
                .ok_or_else(|| "the instance control has no central output descriptor".to_owned())?
                .kind;
            if control.port_kind.as_deref() != Some(format!("{port_kind:?}").as_str()) {
                return Err("the instance control has the wrong port kind".into());
            }
            let schema = design_projection::literal_schema_for_instance(field, current);
            let literal = decode_projectional_inspector_literal(schema, control)?;
            if current == Some(&literal) {
                return Ok(None);
            }
            Ok(Some((
                IntentInspectorEditTarget::Instance { leaf },
                IntentInspectorEditValue::Literal { literal },
            )))
        }
        _ => Err("the Inspector control has an unknown edit owner".into()),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn decode_projectional_inspector_literal(
    schema: geosolve_sketch_intent::IntentLiteralSchema,
    control: &ProjectionalInspectorControl,
) -> Result<geosolve_sketch_intent::IntentLiteral, String> {
    use geosolve_sketch_intent::{IntentKey, IntentLiteral, IntentLiteralSchema};

    if control.schema.as_deref() != Some(design_projection::literal_schema_key(schema).as_str()) {
        return Err("the Inspector control has the wrong literal schema".into());
    }
    match schema {
        IntentLiteralSchema::Boolean => {
            if control.unit.is_some() || control.component.is_some() {
                return Err("the boolean control contains scalar coordinates".into());
            }
            let ProjectionalInspectorSubmission::Checked(value) = &control.submission else {
                return Err("the boolean control did not submit a checkbox value".into());
            };
            Ok(IntentLiteral::Boolean(*value))
        }
        IntentLiteralSchema::Integer => {
            require_scalar_inspector_control(control, None)?;
            let value = inspector_text_submission(control)?
                .parse()
                .map_err(|_| "the Inspector integer is invalid".to_owned())?;
            Ok(IntentLiteral::Integer(value))
        }
        IntentLiteralSchema::Natural => {
            require_scalar_inspector_control(control, None)?;
            let value = inspector_text_submission(control)?
                .parse()
                .map_err(|_| "the Inspector natural number is invalid".to_owned())?;
            Ok(IntentLiteral::Natural(value))
        }
        IntentLiteralSchema::Text => {
            require_scalar_inspector_control(control, None)?;
            IntentKey::new(inspector_text_submission(control)?.to_owned())
                .map(IntentLiteral::Text)
                .map_err(|error| error.to_string())
        }
        IntentLiteralSchema::Enum => {
            require_scalar_inspector_control(control, None)?;
            IntentKey::new(inspector_text_submission(control)?.to_owned())
                .map(IntentLiteral::Enum)
                .map_err(|error| error.to_string())
        }
        IntentLiteralSchema::Point => {
            if control.unit.is_some() || !matches!(control.component.as_deref(), Some("x" | "y")) {
                return Err("the point control coordinate is malformed".into());
            }
            let ProjectionalInspectorSubmission::Point { ref x, ref y } = control.submission else {
                return Err("the point control did not submit both components".into());
            };
            Ok(IntentLiteral::Point([
                parse_finite_inspector_number(x.as_deref())?,
                parse_finite_inspector_number(y.as_deref())?,
            ]))
        }
        IntentLiteralSchema::Quantity(unit) => {
            require_scalar_inspector_control(
                control,
                Some(design_projection::intent_unit_key(unit)),
            )?;
            let value = parse_finite_inspector_number(Some(inspector_text_submission(control)?))?;
            Ok(IntentLiteral::Quantity { value, unit })
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn require_scalar_inspector_control(
    control: &ProjectionalInspectorControl,
    unit: Option<&str>,
) -> Result<(), String> {
    if control.component.is_some() || control.unit.as_deref() != unit {
        return Err("the Inspector scalar control coordinate is malformed".into());
    }
    Ok(())
}

#[cfg(any(target_arch = "wasm32", test))]
fn inspector_text_submission(control: &ProjectionalInspectorControl) -> Result<&str, String> {
    let ProjectionalInspectorSubmission::Text(value) = &control.submission else {
        return Err("the Inspector control did not submit a text value".into());
    };
    Ok(value)
}

#[cfg(any(target_arch = "wasm32", test))]
fn parse_finite_inspector_number(value: Option<&str>) -> Result<f64, String> {
    let value: f64 = value
        .ok_or_else(|| "the Inspector numeric value is incomplete".to_owned())?
        .parse()
        .map_err(|_| "the Inspector numeric value is invalid".to_owned())?;
    if !value.is_finite() {
        return Err("the Inspector numeric value must be finite".into());
    }
    Ok(value)
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerCaptureKind {
    Point,
    CurveControl,
    Annotation,
    Fillet,
    OffsetDistance,
    Pan,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerOwnership {
    Owned,
    Foreign,
    Uncaptured,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPanPointerDownRoute {
    BeginPan,
    PreserveCapturedInteraction,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPrimaryPointerDownRoute {
    Dispatch,
    PreserveCapturedInteraction,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapturedCanvasPointer {
    pointer_id: i32,
    kind: CanvasPointerCaptureKind,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerTerminal {
    PointerUp { pointer_id: i32 },
    PointerCancel { pointer_id: i32 },
    LostPointerCapture { pointer_id: i32 },
    InteractionCancel,
    CameraCancel,
    GeometryPolicyCancel,
}

#[cfg(any(target_arch = "wasm32", test))]
impl CanvasPointerTerminal {
    const fn pointer_id(self) -> Option<i32> {
        match self {
            Self::PointerUp { pointer_id }
            | Self::PointerCancel { pointer_id }
            | Self::LostPointerCapture { pointer_id } => Some(pointer_id),
            Self::InteractionCancel | Self::CameraCancel | Self::GeometryPolicyCancel => None,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerTerminalDisposition {
    Complete,
    Cancel,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanvasPointerTerminalRoute {
    captured: CapturedCanvasPointer,
    disposition: CanvasPointerTerminalDisposition,
    release_platform_capture: bool,
}

/// Browser-only pointer ownership bookkeeping.
///
/// Gesture meaning remains in the headless editor. This state records only which
/// platform pointers the SVG promised to keep delivering so terminal browser
/// events can release or cancel that promise exactly once.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct CanvasPointerCaptures {
    active: Option<CapturedCanvasPointer>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl CanvasPointerCaptures {
    fn begin(&mut self, pointer: CapturedCanvasPointer) -> bool {
        if pointer.pointer_id < 0 || self.active.is_some() {
            return false;
        }
        self.active = Some(pointer);
        true
    }

    fn ownership(&self, pointer_id: i32) -> CanvasPointerOwnership {
        match self.active {
            Some(active) if active.pointer_id == pointer_id => CanvasPointerOwnership::Owned,
            Some(_) => CanvasPointerOwnership::Foreign,
            None => CanvasPointerOwnership::Uncaptured,
        }
    }

    fn contains(&self, pointer_id: i32) -> bool {
        self.ownership(pointer_id) == CanvasPointerOwnership::Owned
    }

    fn is_empty(&self) -> bool {
        self.active.is_none()
    }

    fn route_terminal(
        &mut self,
        terminal: CanvasPointerTerminal,
    ) -> Option<CanvasPointerTerminalRoute> {
        if terminal
            .pointer_id()
            .is_some_and(|pointer_id| !self.contains(pointer_id))
        {
            return None;
        }
        let captured = self.active.take()?;
        Some(CanvasPointerTerminalRoute {
            captured,
            disposition: if matches!(terminal, CanvasPointerTerminal::PointerUp { .. }) {
                CanvasPointerTerminalDisposition::Complete
            } else {
                CanvasPointerTerminalDisposition::Cancel
            },
            release_platform_capture: !matches!(
                terminal,
                CanvasPointerTerminal::LostPointerCapture { .. }
            ),
        })
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn route_canvas_pan_pointer_down(captures: &CanvasPointerCaptures) -> CanvasPanPointerDownRoute {
    if captures.is_empty() {
        CanvasPanPointerDownRoute::BeginPan
    } else {
        CanvasPanPointerDownRoute::PreserveCapturedInteraction
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn route_canvas_primary_pointer_down(
    captures: &CanvasPointerCaptures,
) -> CanvasPrimaryPointerDownRoute {
    if captures.is_empty() {
        CanvasPrimaryPointerDownRoute::Dispatch
    } else {
        CanvasPrimaryPointerDownRoute::PreserveCapturedInteraction
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn canvas_pointer_capture_kind(
    kind: geosolve_constraint_editor::ActivePointerGestureKind,
) -> CanvasPointerCaptureKind {
    match kind {
        geosolve_constraint_editor::ActivePointerGestureKind::Point => {
            CanvasPointerCaptureKind::Point
        }
        geosolve_constraint_editor::ActivePointerGestureKind::CurveControl => {
            CanvasPointerCaptureKind::CurveControl
        }
        geosolve_constraint_editor::ActivePointerGestureKind::Annotation => {
            CanvasPointerCaptureKind::Annotation
        }
        geosolve_constraint_editor::ActivePointerGestureKind::FilletRadius
        | geosolve_constraint_editor::ActivePointerGestureKind::FilletContact => {
            CanvasPointerCaptureKind::Fillet
        }
        geosolve_constraint_editor::ActivePointerGestureKind::OffsetDistance => {
            CanvasPointerCaptureKind::OffsetDistance
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct DraftingPointerSample {
    input: geosolve_constraint_editor::PointerInput,
    authoring: geosolve_constraint_editor::DraftAuthoringInput,
    painted_item: Option<geosolve_constraint_editor::SelectionItem>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DraftingPointerSample {
    #[cfg(test)]
    const fn from_input(input: geosolve_constraint_editor::PointerInput) -> Self {
        Self {
            authoring: effect_adapter::draft_authoring_input(input.modifiers, None),
            input,
            painted_item: None,
        }
    }

    const fn with_painted_item(
        input: geosolve_constraint_editor::PointerInput,
        painted_item: Option<geosolve_constraint_editor::SelectionItem>,
        preferred_candidate: Option<geosolve_constraint_editor::DraftInferenceCandidateId>,
    ) -> Self {
        Self {
            authoring: effect_adapter::draft_authoring_input(input.modifiers, preferred_candidate),
            input,
            painted_item,
        }
    }

    const fn with_state(
        input: geosolve_constraint_editor::PointerInput,
        suppressed: bool,
        regularized: bool,
        preferred_candidate: Option<geosolve_constraint_editor::DraftInferenceCandidateId>,
    ) -> Self {
        Self {
            input,
            authoring: effect_adapter::draft_authoring_input_for_state(
                suppressed,
                regularized,
                preferred_candidate,
            ),
            painted_item: None,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct StationaryDraftInferenceContext {
    pointer_id: u64,
    position: geosolve_constraint_editor::ScreenPoint,
    modifiers: geosolve_constraint_editor::Modifiers,
}

#[cfg(any(target_arch = "wasm32", test))]
impl StationaryDraftInferenceContext {
    const fn from_input(input: geosolve_constraint_editor::PointerInput) -> Self {
        Self {
            pointer_id: input.pointer_id,
            position: input.position,
            modifiers: input.modifiers,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct StationaryDraftInferenceChoice {
    context: StationaryDraftInferenceContext,
    candidate: geosolve_constraint_editor::DraftInferenceCandidateId,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct PointerMoveQueue {
    pending: Option<DraftingPointerSample>,
    last_input: Option<geosolve_constraint_editor::PointerInput>,
    modifiers: geosolve_constraint_editor::Modifiers,
    stationary_choice: Option<StationaryDraftInferenceChoice>,
    next_generation: u64,
    scheduled_generation: Option<u64>,
}

/// Coalesces projectional pointer samples while reusing the presentation-only
/// modifier/candidate envelope shared by geometry drafting. The queue carries
/// no durable flat coordinator or history; the newest sample always wins and a
/// terminal event drains it synchronously before exact publication.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct ProjectionalPointerMoveQueue {
    inner: PointerMoveQueue,
}

#[cfg(any(target_arch = "wasm32", test))]
#[cfg_attr(test, allow(dead_code))]
impl ProjectionalPointerMoveQueue {
    fn push(&mut self, input: geosolve_constraint_editor::PointerInput) -> Option<u64> {
        self.inner.push_with_painted_item(input, None)
    }

    fn observe_for_pointer_down(
        &mut self,
        input: geosolve_constraint_editor::PointerInput,
    ) -> DraftingPointerSample {
        self.inner.observe_for_pointer_down(input)
    }

    fn observe(&mut self, input: geosolve_constraint_editor::PointerInput) {
        let _ = self.inner.observe(input);
    }

    fn take_for_frame(&mut self, generation: u64) -> Option<DraftingPointerSample> {
        self.inner.take_for_frame(generation)
    }

    fn cancel_frame(&mut self, generation: u64) {
        self.inner.cancel_frame(generation);
    }

    fn drain_before_terminal(&mut self) -> Option<DraftingPointerSample> {
        self.inner.drain_before_terminal()
    }

    fn invalidate(&mut self) -> bool {
        let changed = self.inner.scheduled_generation.is_some() || self.inner.pending.is_some();
        self.inner.invalidate_before_immediate_action();
        changed
    }

    fn clear_stationary_sample(&mut self) -> bool {
        self.inner.clear_stationary_sample()
    }

    fn window_blur(&mut self, owns_queued_sample: bool) -> Option<DraftingPointerSample> {
        self.inner.window_blur(owns_queued_sample)
    }

    fn stationary_authoring_state(
        &mut self,
        modifiers: geosolve_constraint_editor::Modifiers,
        owns_queued_sample: bool,
    ) -> Option<DraftingPointerSample> {
        self.inner
            .stationary_authoring_state(modifiers, owns_queued_sample)
    }

    fn drain_before_stationary_cycle(
        &mut self,
        owns_queued_sample: bool,
    ) -> Option<DraftingPointerSample> {
        self.inner.drain_before_stationary_cycle(owns_queued_sample)
    }

    fn stationary_candidate(
        &mut self,
        candidate: geosolve_constraint_editor::DraftInferenceCandidateId,
        owns_queued_sample: bool,
    ) -> Option<DraftingPointerSample> {
        self.inner
            .stationary_candidate(candidate, owns_queued_sample)
    }

    const fn last_input(&self) -> Option<geosolve_constraint_editor::PointerInput> {
        self.inner.last_input
    }
}

/// Replays a coalesced projectional sample, when present, and then always
/// evaluates the exact terminal browser sample. A rejected intermediate frame
/// is presentation-only and cannot veto a newer valid release.
#[cfg(any(target_arch = "wasm32", test))]
fn replay_projectional_terminal_samples<T, E>(
    pending: Option<T>,
    terminal: T,
    mut apply: impl FnMut(T) -> Result<(), E>,
) -> Result<(), E> {
    if let Some(pending) = pending {
        let _ = apply(pending);
    }
    apply(terminal)
}

#[cfg(any(target_arch = "wasm32", test))]
impl PointerMoveQueue {
    #[cfg(test)]
    fn push(&mut self, input: geosolve_constraint_editor::PointerInput) -> Option<u64> {
        let sample = self.observe(input);
        self.push_sample(sample)
    }

    fn push_with_painted_item(
        &mut self,
        input: geosolve_constraint_editor::PointerInput,
        painted_item: Option<geosolve_constraint_editor::SelectionItem>,
    ) -> Option<u64> {
        let sample = self.observe_with_painted_item(input, painted_item);
        self.push_sample(sample)
    }

    fn push_sample(&mut self, sample: DraftingPointerSample) -> Option<u64> {
        self.pending = Some(sample);
        if self.scheduled_generation.is_some() {
            return None;
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        self.scheduled_generation = Some(self.next_generation);
        Some(self.next_generation)
    }

    fn observe(
        &mut self,
        input: geosolve_constraint_editor::PointerInput,
    ) -> DraftingPointerSample {
        self.observe_with_painted_item(input, None)
    }

    fn observe_with_painted_item(
        &mut self,
        input: geosolve_constraint_editor::PointerInput,
        painted_item: Option<geosolve_constraint_editor::SelectionItem>,
    ) -> DraftingPointerSample {
        let context = StationaryDraftInferenceContext::from_input(input);
        if self
            .stationary_choice
            .is_some_and(|choice| choice.context != context)
        {
            self.clear_candidate_preference();
        }
        self.last_input = Some(input);
        self.modifiers = input.modifiers;
        let preferred_candidate = self
            .stationary_choice
            .filter(|choice| choice.context == context)
            .map(|choice| choice.candidate);
        DraftingPointerSample::with_painted_item(input, painted_item, preferred_candidate)
    }

    fn observe_for_pointer_down(
        &mut self,
        input: geosolve_constraint_editor::PointerInput,
    ) -> DraftingPointerSample {
        let sample = self.observe(input);
        // An exact stationary choice may authorize this pointer-down once. A
        // rejected or stale click must never leave an ID behind for a retry.
        self.clear_candidate_preference();
        sample
    }

    fn stationary_authoring_state(
        &mut self,
        modifiers: geosolve_constraint_editor::Modifiers,
        owns_queued_sample: bool,
    ) -> Option<DraftingPointerSample> {
        if !owns_queued_sample {
            self.clear_candidate_preference();
        }
        if self.modifiers == modifiers {
            return None;
        }
        self.modifiers = modifiers;
        self.clear_candidate_preference();
        if let Some(input) = self.last_input.as_mut() {
            input.modifiers = modifiers;
        }
        if !owns_queued_sample {
            // Select drags, Fillet gestures, authoring overlays, and pan share
            // this RAF queue but do not consume geometry recipe intent. Keep their
            // exact queued movement while still tracking the browser modifier.
            return None;
        }
        self.scheduled_generation = None;
        self.pending = None;
        self.last_input.map(|input| {
            DraftingPointerSample::with_state(
                input,
                modifiers.control || modifiers.command,
                modifiers.shift,
                None,
            )
        })
    }

    #[cfg_attr(test, allow(dead_code))]
    fn stationary_candidate(
        &mut self,
        preferred_candidate: geosolve_constraint_editor::DraftInferenceCandidateId,
        owns_queued_sample: bool,
    ) -> Option<DraftingPointerSample> {
        if !owns_queued_sample {
            self.clear_candidate_preference();
            return None;
        }
        let input = self.last_input?;
        let context = StationaryDraftInferenceContext::from_input(input);
        self.stationary_choice = Some(StationaryDraftInferenceChoice {
            context,
            candidate: preferred_candidate,
        });
        self.scheduled_generation = None;
        self.pending = None;
        Some(DraftingPointerSample::with_state(
            input,
            self.modifiers.control || self.modifiers.command,
            self.modifiers.shift,
            Some(preferred_candidate),
        ))
    }

    fn clear_candidate_preference(&mut self) {
        self.stationary_choice = None;
        if let Some(pending) = self.pending.as_mut() {
            pending.authoring.inference.preferred_candidate = None;
        }
    }

    fn clear_candidate_and_refresh(
        &mut self,
        owns_queued_sample: bool,
    ) -> Option<DraftingPointerSample> {
        self.clear_candidate_preference();
        if !owns_queued_sample {
            return None;
        }
        self.scheduled_generation = None;
        self.pending = None;
        self.last_input.map(|input| {
            DraftingPointerSample::with_state(
                input,
                self.modifiers.control || self.modifiers.command,
                self.modifiers.shift,
                None,
            )
        })
    }

    fn window_blur(&mut self, owns_queued_sample: bool) -> Option<DraftingPointerSample> {
        let had_choice = self.stationary_choice.is_some();
        self.clear_candidate_preference();
        let modifiers = geosolve_constraint_editor::Modifiers::default();
        let modifiers_changed = self.modifiers != modifiers;
        let had_owned_pending = owns_queued_sample && self.pending.is_some();
        if !had_choice && !modifiers_changed && !had_owned_pending {
            return None;
        }
        self.modifiers = modifiers;
        if let Some(input) = self.last_input.as_mut() {
            input.modifiers = modifiers;
        }
        if !owns_queued_sample {
            return None;
        }
        self.scheduled_generation = None;
        self.pending = None;
        self.last_input
            .map(|input| DraftingPointerSample::with_state(input, false, false, None))
    }

    fn clear_stationary_sample(&mut self) -> bool {
        let cleared = self.last_input.take().is_some();
        self.modifiers = geosolve_constraint_editor::Modifiers::default();
        self.clear_candidate_preference();
        self.invalidate_before_immediate_action();
        cleared
    }

    fn take_for_frame(&mut self, generation: u64) -> Option<DraftingPointerSample> {
        if self.scheduled_generation != Some(generation) {
            return None;
        }
        self.scheduled_generation = None;
        self.pending.take()
    }

    fn cancel_frame(&mut self, generation: u64) {
        if self.scheduled_generation == Some(generation) {
            self.scheduled_generation = None;
        }
    }

    fn drain_before_terminal(&mut self) -> Option<DraftingPointerSample> {
        self.scheduled_generation = None;
        self.pending.take()
    }

    fn drain_before_stationary_cycle(
        &mut self,
        owns_queued_sample: bool,
    ) -> Option<DraftingPointerSample> {
        if !owns_queued_sample {
            self.clear_candidate_preference();
            return None;
        }
        self.drain_before_terminal()
    }

    /// Invalidates a coalesced ordinary move and stationary inference choice
    /// before an immediately handled semantic lifecycle transition.
    ///
    /// The scheduled animation-frame closure will observe the missing
    /// generation and do nothing, so it cannot later clear the newer Fillet
    /// action preview with an older canvas sample.
    fn invalidate_before_immediate_action(&mut self) {
        self.scheduled_generation = None;
        self.pending = None;
        self.clear_candidate_preference();
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn draft_inference_preference_is_stale(
    resolution: Option<&geosolve_constraint_editor::DraftInferenceResolution>,
) -> bool {
    resolution.is_some_and(|resolution| {
        matches!(
            &resolution.status,
            geosolve_constraint_editor::DraftInferenceStatus::StalePreferredCandidate { .. }
        )
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn geometry_variant_keyboard_target(
    current: geosolve_constraint_editor::GeometryToolVariant,
    key: &str,
) -> Option<geosolve_constraint_editor::GeometryToolVariant> {
    let variants = current.family().variants();
    let index = variants.iter().position(|variant| *variant == current)?;
    let target = match key {
        "ArrowRight" | "ArrowDown" => (index + 1) % variants.len(),
        "ArrowLeft" | "ArrowUp" => (index + variants.len() - 1) % variants.len(),
        "Home" => 0,
        "End" => variants.len() - 1,
        _ => return None,
    };
    variants.get(target).copied()
}

/// Editable controls and dialogs normally own their keyboard input. The
/// non-modal tool-options dialog is the deliberate Escape exception: its own
/// guidance promises that Escape cancels/exits the active authoring tool, so
/// focus on a variant button or option input must not strand that tool.
#[cfg(any(target_arch = "wasm32", test))]
const fn isolate_projectional_keyboard_target(
    editable_or_dialog: bool,
    inside_tool_options: bool,
    key_is_escape: bool,
) -> bool {
    editable_or_dialog && !(inside_tool_options && key_is_escape)
}

#[cfg(any(target_arch = "wasm32", test))]
fn geometry_sweep_flip_available(
    status: Option<&geosolve_constraint_editor::GeometryDraftStatus>,
    repeated: bool,
    modified: bool,
) -> bool {
    !repeated
        && !modified
        && status.is_some_and(|status| status.completed_stages > 0 && status.branch.sweep.is_some())
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct FinishDoubleClickTracker {
    first_click: Option<(geosolve_constraint_editor::GeometryToolVariant, usize)>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl FinishDoubleClickTracker {
    fn observe_click(
        &mut self,
        click_detail: i32,
        status: Option<&geosolve_constraint_editor::GeometryDraftStatus>,
    ) -> bool {
        let eligible = status.filter(|status| finish_double_click_eligible(status));
        match click_detail {
            1 => {
                self.first_click = eligible.map(|status| (status.variant, status.completed_stages));
                false
            }
            2 => {
                let first = self.first_click.take();
                first
                    .zip(eligible)
                    .is_some_and(|((variant, stages), status)| {
                        status.variant == variant
                            && stages
                                .checked_add(1)
                                .is_some_and(|next| status.completed_stages == next)
                    })
            }
            _ => {
                self.first_click = None;
                false
            }
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn finish_double_click_eligible(status: &geosolve_constraint_editor::GeometryDraftStatus) -> bool {
    status.can_finish
        && matches!(
            status.variant,
            geosolve_constraint_editor::GeometryToolVariant::Polyline
                | geosolve_constraint_editor::GeometryToolVariant::OpenControlNurbs
                | geosolve_constraint_editor::GeometryToolVariant::PeriodicControlNurbs
        )
}

/// Browser input-ownership transitions that retire a canvas pointer sample.
///
/// Overlay and focus ownership always revoke the sample. An unmapped sample
/// does so only when no captured gesture still owns the pointer.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerContextRoute {
    OverlayOrFocus,
    UnmappedCanvas { pointer_is_captured: bool },
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Default, PartialEq)]
struct CanvasPointerContextRevocation {
    effects: Vec<geosolve_constraint_editor::EditorEffect>,
    cleared_stationary_sample: bool,
}

/// Applies one browser pointer-ownership route without DOM state.
///
/// Keeping queue invalidation and headless hover invalidation together means
/// an already-scheduled animation frame cannot repaint the retired owner.
#[cfg(any(target_arch = "wasm32", test))]
fn revoke_canvas_pointer_context(
    pointer_moves: &mut PointerMoveQueue,
    editor: &mut geosolve_constraint_editor::ConstraintEditor,
    route: CanvasPointerContextRoute,
) -> CanvasPointerContextRevocation {
    let revoke = match route {
        CanvasPointerContextRoute::OverlayOrFocus => true,
        CanvasPointerContextRoute::UnmappedCanvas {
            pointer_is_captured,
        } => matches!(
            effect_adapter::unmapped_canvas_pointer_action(pointer_is_captured),
            effect_adapter::UnmappedCanvasPointerAction::RevokePointerContext
        ),
    };
    if !revoke {
        return CanvasPointerContextRevocation::default();
    }
    CanvasPointerContextRevocation {
        cleared_stationary_sample: pointer_moves.clear_stationary_sample(),
        effects: editor.pointer_leave(),
    }
}

/// Converts current retained diagnostic targets into the exact selection
/// identities consumed by problem-aware pointer move/down wrappers.
#[cfg(any(target_arch = "wasm32", test))]
fn current_problem_items(
    coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
    scene: &geosolve_constraint_editor::EditorScene,
) -> Vec<geosolve_constraint_editor::SelectionItem> {
    coordinator
        .current_problem_metadata()
        .map(|problem| {
            problem
                .targets
                .iter()
                .filter_map(|target| scene::problem_selection_item(*target, Some(scene)))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanvasPointerMoveOwner {
    Editor,
    OrdinaryAuthoring,
    FeatureAuthoring,
    OffsetAuthoring,
}

/// Routes one mapped canvas move to the same headless state machine that will
/// own an unchanged press. Captured gestures remain editor-owned until their
/// matching terminal sample.
#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::fn_params_excessive_bools)]
const fn canvas_pointer_move_owner(
    ordinary_authoring_active: bool,
    feature_authoring_active: bool,
    offset_authoring_active: bool,
    pointer_is_captured: bool,
) -> CanvasPointerMoveOwner {
    if pointer_is_captured
        || (!ordinary_authoring_active && !feature_authoring_active && !offset_authoring_active)
    {
        CanvasPointerMoveOwner::Editor
    } else if offset_authoring_active {
        CanvasPointerMoveOwner::OffsetAuthoring
    } else if feature_authoring_active {
        CanvasPointerMoveOwner::FeatureAuthoring
    } else {
        CanvasPointerMoveOwner::OrdinaryAuthoring
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoringItemInput {
    CanvasPointerDown,
    CanvasClick,
    TreeClick,
}

#[cfg(any(target_arch = "wasm32", test))]
const fn owns_authoring_pick(input: AuthoringItemInput) -> bool {
    matches!(
        input,
        AuthoringItemInput::CanvasPointerDown | AuthoringItemInput::TreeClick
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn change_owns_option_control_click(
    tag_name: &str,
    in_tool_options: bool,
    in_branch_editor: bool,
) -> bool {
    matches!(tag_name, "INPUT" | "SELECT" | "OPTION") && (in_tool_options || in_branch_editor)
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryShortcut {
    Undo,
    Redo,
}

#[cfg(any(target_arch = "wasm32", test))]
fn history_shortcut(
    key: &str,
    modifiers: geosolve_constraint_editor::Modifiers,
    alt: bool,
) -> Option<HistoryShortcut> {
    if alt || modifiers.control == modifiers.command {
        return None;
    }
    if key.eq_ignore_ascii_case("z") {
        return Some(if modifiers.shift {
            HistoryShortcut::Redo
        } else {
            HistoryShortcut::Undo
        });
    }
    (modifiers.control && !modifiers.shift && key.eq_ignore_ascii_case("y"))
        .then_some(HistoryShortcut::Redo)
}

#[cfg(target_arch = "wasm32")]
fn projectional_history_shortcut(
    key: &str,
    modifiers: geosolve_constraint_editor::Modifiers,
    alt: bool,
) -> Option<HistoryShortcut> {
    history_shortcut(key, modifiers, alt)
}

#[cfg(any(target_arch = "wasm32", test))]
const fn projectional_direct_gesture_is_capturable(
    kind: geosolve_constraint_editor::ActivePointerGestureKind,
) -> bool {
    matches!(
        kind,
        geosolve_constraint_editor::ActivePointerGestureKind::Point
            | geosolve_constraint_editor::ActivePointerGestureKind::CurveControl
            | geosolve_constraint_editor::ActivePointerGestureKind::Annotation
            | geosolve_constraint_editor::ActivePointerGestureKind::FilletRadius
            | geosolve_constraint_editor::ActivePointerGestureKind::OffsetDistance
    )
}

/// A pointer-specific terminal may mutate projectional gesture state only
/// while that exact pointer still owns capture. In particular, the browser's
/// expected `lostpointercapture` after a successful release is a stale event,
/// not a second cancellation terminal.
#[cfg(any(target_arch = "wasm32", test))]
fn projectional_terminal_owns_capture(
    captured_pointer: Option<i32>,
    terminal_pointer: Option<i32>,
) -> bool {
    match terminal_pointer {
        Some(pointer) => captured_pointer == Some(pointer),
        None => true,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn route_projectional_terminal_capture(
    captured_pointer: &mut Option<i32>,
    terminal_pointer: Option<i32>,
) -> Option<i32> {
    if projectional_terminal_owns_capture(*captured_pointer, terminal_pointer) {
        captured_pointer.take()
    } else {
        None
    }
}

/// Applies the projectional browser's presentation-only constraint-mark
/// policy to the same scene DTO used by paint and picking.
#[cfg(any(target_arch = "wasm32", test))]
fn apply_projectional_scene_display(
    scene: &mut geosolve_constraint_editor::EditorScene,
    annotations_visible: bool,
    show_all_constraints: bool,
) {
    scene.set_annotations_visible(annotations_visible);
    scene.set_show_all_constraint_annotations(show_all_constraints);
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionalOutlineMove {
    Up,
    Down,
    Drop {
        cell: geosolve_sketch_intent::CellId,
        before: Option<geosolve_sketch_intent::NodeId>,
    },
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionalCellMove {
    Up,
    Down,
    Drop {
        before: Option<geosolve_sketch_intent::CellId>,
    },
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionalOutlineDrag {
    Declaration {
        painted_identity: geosolve_sketch_intent::IntentSessionIdentity,
        node: geosolve_sketch_intent::NodeId,
    },
    Cell {
        painted_identity: geosolve_sketch_intent::IntentSessionIdentity,
        cell: geosolve_sketch_intent::CellId,
    },
}

/// Resolves the insertion boundary represented by one painted declaration row.
///
/// Native HTML drag/drop reports the row under the pointer, not an insertion
/// slot. Treating every row as "before" makes a downward drag onto the next row
/// a no-op, while the same gesture over a later row happens to work. The row's
/// upper and lower halves instead denote the adjacent before/after slots, with
/// the dragged declaration removed before the successor is calculated.
#[cfg(any(target_arch = "wasm32", test))]
fn projectional_outline_drop_before(
    projection: &geosolve_constraint_editor::IntentWorkbenchProjection,
    dragged: geosolve_sketch_intent::NodeId,
    cell: geosolve_sketch_intent::CellId,
    anchor: Option<geosolve_sketch_intent::NodeId>,
    after: bool,
) -> Result<Option<geosolve_sketch_intent::NodeId>, &'static str> {
    let Some(anchor) = anchor else {
        return Ok(None);
    };
    if anchor == dragged {
        return Err("the declaration cannot be dropped onto itself");
    }
    let hidden = design_projection::grouped_outline_helper_nodes(projection);
    let visible = projection
        .outline
        .iter()
        .find(|candidate| candidate.cell == cell)
        .ok_or("the drop cell belongs to a stale Outline")?
        .declarations
        .iter()
        .map(|declaration| declaration.node)
        .filter(|node| !hidden.contains(node) && *node != dragged)
        .collect::<Vec<_>>();
    let index = visible
        .iter()
        .position(|node| *node == anchor)
        .ok_or("the drop target belongs to a different or stale cell")?;
    Ok(if after {
        visible.get(index + 1).copied()
    } else {
        Some(anchor)
    })
}

/// Resolves the insertion boundary represented by one painted cell header.
#[cfg(any(target_arch = "wasm32", test))]
fn projectional_cell_drop_before(
    projection: &geosolve_constraint_editor::IntentWorkbenchProjection,
    dragged: geosolve_sketch_intent::CellId,
    anchor: Option<geosolve_sketch_intent::CellId>,
    after: bool,
) -> Result<Option<geosolve_sketch_intent::CellId>, &'static str> {
    let Some(anchor) = anchor else {
        return Ok(None);
    };
    if anchor == dragged {
        return Err("the cell cannot be dropped onto itself");
    }
    let visible = projection
        .outline
        .iter()
        .map(|cell| cell.cell)
        .filter(|cell| *cell != dragged)
        .collect::<Vec<_>>();
    let index = visible
        .iter()
        .position(|cell| *cell == anchor)
        .ok_or("the drop cell belongs to a stale Outline")?;
    Ok(if after {
        visible.get(index + 1).copied()
    } else {
        Some(anchor)
    })
}

/// Resolves one Outline organization gesture against the exact projection it
/// was painted from. Geometry and dependency order never participate.
#[cfg(any(target_arch = "wasm32", test))]
fn projectional_outline_move_patch(
    projection: &geosolve_constraint_editor::IntentWorkbenchProjection,
    node: geosolve_sketch_intent::NodeId,
    movement: ProjectionalOutlineMove,
) -> Result<geosolve_sketch_intent::IntentPatch, &'static str> {
    let grouped_helpers = design_projection::grouped_outline_helper_nodes(projection);
    let (current_cell, index) = projection
        .outline
        .iter()
        .find_map(|cell| {
            cell.declarations
                .iter()
                .filter(|declaration| !grouped_helpers.contains(&declaration.node))
                .position(|declaration| declaration.node == node)
                .map(|index| (cell, index))
        })
        .ok_or("the dragged declaration belongs to a stale Outline")?;
    let visible = current_cell
        .declarations
        .iter()
        .filter(|declaration| !grouped_helpers.contains(&declaration.node))
        .collect::<Vec<_>>();
    let (cell, before) = match movement {
        ProjectionalOutlineMove::Up => {
            let before = index
                .checked_sub(1)
                .and_then(|index| visible.get(index))
                .map(|declaration| declaration.node)
                .ok_or("the declaration is already first in its cell")?;
            (current_cell.cell, Some(before))
        }
        ProjectionalOutlineMove::Down => {
            if index + 1 >= visible.len() {
                return Err("the declaration is already last in its cell");
            }
            let before = visible.get(index + 2).map(|declaration| declaration.node);
            (current_cell.cell, before)
        }
        ProjectionalOutlineMove::Drop { cell, before } => {
            let target = projection
                .outline
                .iter()
                .find(|candidate| candidate.cell == cell)
                .ok_or("the drop cell belongs to a stale Outline")?;
            if before == Some(node) {
                return Err("the declaration cannot be dropped before itself");
            }
            if before.is_some_and(|before| {
                !target
                    .declarations
                    .iter()
                    .any(|declaration| declaration.node == before)
            }) {
                return Err("the drop target belongs to a different or stale cell");
            }
            (cell, before)
        }
    };
    Ok(geosolve_sketch_intent::IntentPatch::new(
        projection.identity,
        geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
        vec![
            geosolve_sketch_intent::IntentPatchOperation::MoveDeclaration {
                node,
                cell: geosolve_sketch_intent::CellTarget::Stable { cell },
                before,
            },
        ],
    ))
}

/// Resolves one cell organization gesture against the exact painted
/// projection. Cell order is presentation-only and never consults geometry or
/// dependency order.
#[cfg(any(target_arch = "wasm32", test))]
fn projectional_cell_move_patch(
    projection: &geosolve_constraint_editor::IntentWorkbenchProjection,
    cell: geosolve_sketch_intent::CellId,
    movement: ProjectionalCellMove,
) -> Result<geosolve_sketch_intent::IntentPatch, &'static str> {
    let original = projection
        .outline
        .iter()
        .map(|candidate| candidate.cell)
        .collect::<Vec<_>>();
    let index = original
        .iter()
        .position(|candidate| *candidate == cell)
        .ok_or("the moved cell belongs to a stale Outline")?;
    let mut exact_order = original.clone();
    match movement {
        ProjectionalCellMove::Up => {
            let previous = index.checked_sub(1).ok_or("the cell is already first")?;
            exact_order.swap(previous, index);
        }
        ProjectionalCellMove::Down => {
            if index + 1 >= exact_order.len() {
                return Err("the cell is already last");
            }
            exact_order.swap(index, index + 1);
        }
        ProjectionalCellMove::Drop { before } => {
            if before == Some(cell) {
                return Err("the cell cannot be dropped before itself");
            }
            if before.is_some_and(|before| !original.contains(&before)) {
                return Err("the drop cell belongs to a stale Outline");
            }
            exact_order.remove(index);
            let insertion = before
                .and_then(|before| {
                    exact_order
                        .iter()
                        .position(|candidate| *candidate == before)
                })
                .unwrap_or(exact_order.len());
            exact_order.insert(insertion, cell);
        }
    }
    if exact_order == original {
        return Err("the cell is already at that position");
    }
    Ok(geosolve_sketch_intent::IntentPatch::new(
        projection.identity,
        geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
        vec![geosolve_sketch_intent::IntentPatchOperation::ReorderCells { exact_order }],
    ))
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::fn_params_excessive_bools)]
const fn canvas_cursor_key(
    tool: geosolve_constraint_editor::EditorTool,
    authoring_active: bool,
    feature_authoring_active: bool,
    offset_authoring_active: bool,
    panning: bool,
) -> &'static str {
    if panning {
        "pan"
    } else if offset_authoring_active {
        "offset"
    } else if feature_authoring_active {
        "fillet"
    } else if authoring_active {
        "constraint"
    } else if matches!(tool, geosolve_constraint_editor::EditorTool::Select) {
        "select"
    } else {
        "draw"
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::fn_params_excessive_bools)]
fn canvas_cursor_key_with_curve_control(
    tool: geosolve_constraint_editor::EditorTool,
    authoring_active: bool,
    feature_authoring_active: bool,
    offset_authoring_active: bool,
    panning: bool,
    hover: geosolve_constraint_editor::EditorHoverState,
    active: Option<geosolve_constraint_editor::ActivePointerGesture>,
) -> &'static str {
    if panning {
        return "pan";
    }
    if offset_authoring_active {
        if active.is_some_and(|gesture| {
            gesture.kind == geosolve_constraint_editor::ActivePointerGestureKind::OffsetDistance
        }) {
            return "offset-distance-active";
        }
        if matches!(
            hover.target,
            Some(
                geosolve_constraint_editor::EditorHoverTarget::Geometry(_)
                    | geosolve_constraint_editor::EditorHoverTarget::Annotation(_)
            )
        ) {
            return "offset-distance";
        }
        return "offset";
    }
    if authoring_active
        || feature_authoring_active
        || tool != geosolve_constraint_editor::EditorTool::Select
    {
        return canvas_cursor_key(
            tool,
            authoring_active,
            feature_authoring_active,
            offset_authoring_active,
            panning,
        );
    }
    if active.is_some_and(|gesture| {
        gesture.kind == geosolve_constraint_editor::ActivePointerGestureKind::CurveControl
    }) {
        "curve-control-active"
    } else if matches!(
        hover.target,
        Some(geosolve_constraint_editor::EditorHoverTarget::CurveControl { .. })
    ) {
        "curve-control"
    } else {
        canvas_cursor_key(
            tool,
            authoring_active,
            feature_authoring_active,
            offset_authoring_active,
            panning,
        )
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct CoordinateHud {
    text: String,
    title: String,
    adjusted: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
fn coordinate_hud(
    viewport: geosolve_constraint_editor::Viewport,
    pointer: Option<geosolve_constraint_editor::PointerInput>,
    inference: Option<&geosolve_constraint_editor::DraftInferenceResolution>,
) -> CoordinateHud {
    let Some(pointer) = pointer else {
        return CoordinateHud {
            text: "X — · Y —".into(),
            title: "Move the pointer over the sketch plane to inspect coordinates".into(),
            adjusted: false,
        };
    };
    let raw = viewport.screen_to_model(pointer.position);
    let matching_inference = inference.filter(|resolution| {
        screen_distance(resolution.raw_screen_position, pointer.position) <= 1.0e-6
    });
    let displayed = matching_inference.map_or(raw, |resolution| resolution.adjusted_model_position);
    let adjusted = matching_inference.is_some_and(|resolution| {
        screen_distance(
            resolution.adjusted_screen_position,
            resolution.raw_screen_position,
        ) > 1.0e-6
    });
    let displayed = displayed.map(normalize_display_zero);
    let raw = raw.map(normalize_display_zero);
    CoordinateHud {
        text: format!("X {:.3} · Y {:.3}", displayed[0], displayed[1]),
        title: if adjusted {
            format!("Inferred position · raw X {:.3}, Y {:.3}", raw[0], raw[1])
        } else {
            "Canvas pointer coordinates".into()
        },
        adjusted,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn normalize_display_zero(value: f64) -> f64 {
    if value.abs() < 0.0005 { 0.0 } else { value }
}

#[cfg(any(target_arch = "wasm32", test))]
fn screen_distance(
    first: geosolve_constraint_editor::ScreenPoint,
    second: geosolve_constraint_editor::ScreenPoint,
) -> f64 {
    (first.x - second.x).hypot(first.y - second.y)
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct AnnotationInspectorPresentation {
    family: &'static str,
    detail: String,
    meta: String,
}

#[cfg(any(target_arch = "wasm32", test))]
fn annotation_inspector_presentation(
    scene: Option<&geosolve_constraint_editor::EditorScene>,
    selection: &[geosolve_constraint_editor::SelectionItem],
) -> Option<AnnotationInspectorPresentation> {
    let [item] = selection else {
        return None;
    };
    let annotation = scene?
        .annotations
        .iter()
        .find(|entry| entry.item == *item)?;
    let meta = match annotation.kind {
        geosolve_constraint_editor::SceneAnnotationKind::Constraint(_) => format!(
            "Constraint · {} direct operand{}",
            annotation.operands.len(),
            if annotation.operands.len() == 1 {
                ""
            } else {
                "s"
            },
        ),
        _ => format!(
            "{} dimension · Canvas value {}",
            if annotation.reference {
                "Reference"
            } else {
                "Driving"
            },
            annotation.visible_text.as_deref().unwrap_or("—"),
        ),
    };
    Some(AnnotationInspectorPresentation {
        family: annotation_family_name(annotation.kind),
        detail: annotation.accessible_label.clone(),
        meta,
    })
}

/// Browser-neutral markup for the exact selected-curve property fallback.
///
/// The metadata already identifies persistent scalar ownership and explicit
/// branch state. Keeping this formatter outside the WASM module lets native
/// adapter tests prove that the browser never inspects curve definitions or
/// reconstructs homogeneous rational coordinates.
#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::too_many_lines,
    reason = "one closed metadata formatter keeps every selected-curve property role and action auditable"
)]
fn curve_control_inspector_markup(
    metadata: &geosolve_constraint_editor::SelectedCurvePropertyMetadata,
) -> String {
    use std::fmt::Write as _;

    use geosolve_constraint_editor::CurveNumericPropertyKind;
    use geosolve_sketch::{
        DocumentArcSweep, DocumentCurveControlAvailability, DocumentHyperbolaBranch,
    };

    let mut output = String::new();
    if let Some(reason) = curve_property_read_only_reason(metadata.direct_edit_availability) {
        let _ = write!(
            output,
            "<p class=\"wb-read-only-note\" data-curve-properties-read-only>Read-only: {reason}.</p>",
        );
    }
    if let Some(degree) = metadata.degree {
        let _ = write!(
            output,
            "<div class=\"wb-curve-control-summary\"><span>Degree</span><output>{degree}</output></div>",
        );
    }
    if let Some(control) = metadata.rational_control {
        use geosolve_sketch::DocumentRationalConicControl;

        let (label, coordinate, note) = match control {
            DocumentRationalConicControl::Euclidean { middle, .. } => (
                "Middle control P1",
                middle,
                "Euclidean control P1; the conic is not required to pass through this point.",
            ),
            DocumentRationalConicControl::Projective {
                weighted_middle, ..
            } => (
                "Projective middle Qh",
                weighted_middle,
                "Zero-weight projective vector Qh; this is deliberately not an ordinary point.",
            ),
            _ => ("Middle control", [0.0, 0.0], "Unsupported control mode."),
        };
        let disabled = curve_property_disabled_attributes(metadata.direct_edit_availability);
        let action = if disabled.is_empty() {
            " data-wb-action=\"curve-rational-middle\""
        } else {
            ""
        };
        let _ = write!(
            output,
            concat!(
                "<fieldset data-curve-rational-middle><legend>{label}</legend>",
                "<div class=\"wb-curve-coordinate-row\">",
                "<label for=\"wb-curve-rational-middle-x\">X</label>",
                "<input id=\"wb-curve-rational-middle-x\" type=\"number\" step=\"any\" value=\"{}\"{disabled} />",
                "<label for=\"wb-curve-rational-middle-y\">Y</label>",
                "<input id=\"wb-curve-rational-middle-y\" type=\"number\" step=\"any\" value=\"{}\"{disabled} />",
                "</div><button type=\"button\"{action}{disabled}>Apply exact coordinates</button>",
                "<span class=\"wb-read-only-note\">{note}</span></fieldset>"
            ),
            coordinate[0],
            coordinate[1],
            label = label,
            note = note,
            action = action,
            disabled = disabled,
        );
    }
    for property in &metadata.numeric {
        let key = curve_numeric_property_key(property.kind);
        let id = format!("wb-curve-property-{key}");
        let active_gauge = metadata.nurbs_gauge == Some(property.scalar);
        let ordinal = match property.kind {
            CurveNumericPropertyKind::NurbsWeight { ordinal } => Some(ordinal),
            _ => None,
        };
        let limits = curve_numeric_input_limits(property.domain);
        let disabled = curve_property_disabled_attributes(property.availability);
        let _ = write!(
            output,
            concat!(
                "<fieldset data-curve-property=\"{key}\"><legend>{}</legend>",
                "<div class=\"wb-curve-property-row\"><label for=\"{id}\">Exact value</label>",
                "<input id=\"{id}\" type=\"number\" step=\"any\" value=\"{}\"{limits}{disabled} />"
            ),
            curve_numeric_property_label(property.kind),
            property.value,
            key = key,
            id = id,
            limits = limits,
            disabled = disabled,
        );
        if active_gauge {
            output.push_str(
                "<button type=\"button\" disabled aria-disabled=\"true\">Active gauge</button>",
            );
        } else if disabled.is_empty() {
            let _ = write!(
                output,
                "<button type=\"button\" data-wb-action=\"curve-property-{key}\">Apply</button>",
            );
        } else {
            output.push_str(
                "<button type=\"button\" disabled aria-disabled=\"true\">Read-only</button>",
            );
        }
        output.push_str("</div>");
        let _ = write!(
            output,
            "<span class=\"wb-read-only-note\">{} · {}</span>",
            curve_scalar_unit_label(property.unit),
            curve_scalar_domain_label(property.domain),
        );
        if let Some(reason) = curve_property_read_only_reason(property.availability) {
            let _ = write!(
                output,
                "<span class=\"wb-read-only-note\">Read-only: {reason}.</span>",
            );
        }
        if let Some(ordinal) = ordinal
            && !active_gauge
        {
            let gauge_availability = metadata
                .nurbs_gauge_availability
                .unwrap_or(DocumentCurveControlAvailability::Editable);
            if gauge_availability == DocumentCurveControlAvailability::Editable {
                let _ = write!(
                    output,
                    "<button type=\"button\" class=\"wb-secondary\" data-wb-action=\"curve-nurbs-gauge-{ordinal}\">Make gauge</button>",
                );
            } else {
                let _ = write!(
                    output,
                    "<button type=\"button\" class=\"wb-secondary\" disabled aria-disabled=\"true\" title=\"{}\">Make gauge</button>",
                    curve_property_read_only_reason(gauge_availability)
                        .unwrap_or("gauge change is unavailable"),
                );
            }
        }
        output.push_str("</fieldset>");
    }
    if let Some(sweep) = metadata.sweep {
        let disabled = curve_property_disabled_attributes(metadata.direct_edit_availability);
        let action = if disabled.is_empty() {
            " data-wb-action=\"curve-sweep\""
        } else {
            ""
        };
        let counter_clockwise = if sweep == DocumentArcSweep::CounterClockwise {
            " selected"
        } else {
            ""
        };
        let clockwise = if sweep == DocumentArcSweep::Clockwise {
            " selected"
        } else {
            ""
        };
        let _ = write!(
            output,
            concat!(
                "<fieldset><legend>Arc sweep</legend><div class=\"wb-curve-property-row\">",
                "<label for=\"wb-curve-sweep\">Explicit traversal</label>",
                "<select id=\"wb-curve-sweep\"{disabled}><option value=\"counter-clockwise\"{counter_clockwise}>Counter-clockwise</option>",
                "<option value=\"clockwise\"{clockwise}>Clockwise</option></select>",
                "<button type=\"button\"{action}{disabled}>Apply</button></div></fieldset>"
            ),
            counter_clockwise = counter_clockwise,
            clockwise = clockwise,
            action = action,
            disabled = disabled,
        );
    }
    if let Some(branch) = metadata.hyperbola_branch {
        let disabled = curve_property_disabled_attributes(metadata.direct_edit_availability);
        let action = if disabled.is_empty() {
            " data-wb-action=\"curve-hyperbola-branch\""
        } else {
            ""
        };
        let positive = if branch == DocumentHyperbolaBranch::Positive {
            " selected"
        } else {
            ""
        };
        let negative = if branch == DocumentHyperbolaBranch::Negative {
            " selected"
        } else {
            ""
        };
        let _ = write!(
            output,
            concat!(
                "<fieldset><legend>Hyperbola branch</legend><div class=\"wb-curve-property-row\">",
                "<label for=\"wb-curve-hyperbola-branch\">Explicit branch</label>",
                "<select id=\"wb-curve-hyperbola-branch\"{disabled}><option value=\"positive\"{positive}>Positive</option>",
                "<option value=\"negative\"{negative}>Negative</option></select>",
                "<button type=\"button\"{action}{disabled}>Apply</button></div></fieldset>"
            ),
            positive = positive,
            negative = negative,
            action = action,
            disabled = disabled,
        );
    }
    if output.is_empty() {
        output.push_str(
            "<p class=\"wb-read-only-note\">This curve uses ordinary stored point controls; select and drag those points on the canvas.</p>",
        );
    }
    output
}

#[cfg(any(target_arch = "wasm32", test))]
const fn curve_property_disabled_attributes(
    availability: geosolve_sketch::DocumentCurveControlAvailability,
) -> &'static str {
    match availability {
        geosolve_sketch::DocumentCurveControlAvailability::Editable => "",
        geosolve_sketch::DocumentCurveControlAvailability::ReadOnly(_) => {
            " disabled aria-disabled=\"true\""
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn curve_property_read_only_reason(
    availability: geosolve_sketch::DocumentCurveControlAvailability,
) -> Option<&'static str> {
    use geosolve_sketch::{
        DocumentCurveControlAvailability, DocumentCurveControlWithholdingReason,
    };

    match availability {
        DocumentCurveControlAvailability::Editable => None,
        DocumentCurveControlAvailability::ReadOnly(reason) => Some(match reason {
            DocumentCurveControlWithholdingReason::InactiveCurve => "the curve is inactive",
            DocumentCurveControlWithholdingReason::AssociativeFilletOutput => {
                "the associative Fillet owns this output"
            }
            DocumentCurveControlWithholdingReason::HostParameterOwned => {
                "the value is owned by a host parameter"
            }
            DocumentCurveControlWithholdingReason::GaugeOwned => {
                "the value is the active NURBS gauge"
            }
            DocumentCurveControlWithholdingReason::DrivingDimensionOwned => {
                "an active driving radius or diameter dimension owns this size"
            }
            DocumentCurveControlWithholdingReason::EqualRadiusOwned => {
                "an active equal-radius relation owns this size"
            }
            _ => "the curve owner does not expose this direct edit",
        }),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn curve_control_inspector_detail(
    metadata: &geosolve_constraint_editor::SelectedCurvePropertyMetadata,
) -> &'static str {
    use geosolve_constraint_editor::CurvePropertyFamily;
    use geosolve_sketch::DocumentRationalConicControl;

    match metadata.family {
        CurvePropertyFamily::RationalQuadraticConic => match metadata.rational_control {
            Some(DocumentRationalConicControl::Euclidean { .. }) => {
                "Canvas and numeric edits use the ordinary middle control P1; weight remains an exact scalar."
            }
            Some(DocumentRationalConicControl::Projective { .. }) => {
                "At zero weight, the middle control is explicitly the projective Qh vector."
            }
            _ => "The selected rational control mode is unavailable.",
        },
        CurvePropertyFamily::Nurbs => {
            "Stored controls remain ordinary points. Weights are exact numeric values; one read-only weight owns the gauge."
        }
        CurvePropertyFamily::QuadraticBezier
        | CurvePropertyFamily::CubicBezier
        | CurvePropertyFamily::BSpline
        | CurvePropertyFamily::Line
        | CurvePropertyFamily::Polyline => {
            "Stored controls remain ordinary draggable points; the selected cage shows their curve relationship."
        }
        _ => {
            "Canvas handles are transient views of persistent curve parameters; exact values remain available here."
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn curve_numeric_property_key(
    kind: geosolve_constraint_editor::CurveNumericPropertyKind,
) -> String {
    use geosolve_constraint_editor::CurveNumericPropertyKind;

    match kind {
        CurveNumericPropertyKind::Radius => "radius".into(),
        CurveNumericPropertyKind::MinorAxisRatio => "minor-axis-ratio".into(),
        CurveNumericPropertyKind::TrimStart => "trim-start".into(),
        CurveNumericPropertyKind::TrimEnd => "trim-end".into(),
        CurveNumericPropertyKind::SemiConjugate => "semi-conjugate".into(),
        CurveNumericPropertyKind::RationalWeight => "rational-weight".into(),
        CurveNumericPropertyKind::NurbsWeight { ordinal } => format!("nurbs-weight-{ordinal}"),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn curve_numeric_property_label(
    kind: geosolve_constraint_editor::CurveNumericPropertyKind,
) -> &'static str {
    use geosolve_constraint_editor::CurveNumericPropertyKind;

    match kind {
        CurveNumericPropertyKind::NurbsWeight { .. } => "Control weight",
        _ => kind.label(),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn curve_numeric_input_limits(domain: geosolve_sketch::ScalarDomain) -> String {
    use geosolve_sketch::ScalarDomain;

    match domain {
        ScalarDomain::Finite | ScalarDomain::Periodic { .. } => String::new(),
        ScalarDomain::Positive => " min=\"0\"".into(),
        ScalarDomain::Bounded { lower, upper } => {
            format!(" min=\"{lower}\" max=\"{upper}\"")
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn curve_scalar_unit_label(unit: geosolve_sketch::ScalarUnit) -> &'static str {
    use geosolve_sketch::ScalarUnit;

    match unit {
        ScalarUnit::Length => "model units",
        ScalarUnit::Angle => "radians",
        ScalarUnit::Parameter => "unitless parameter",
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn curve_scalar_domain_label(domain: geosolve_sketch::ScalarDomain) -> String {
    use geosolve_sketch::ScalarDomain;

    match domain {
        ScalarDomain::Finite => "finite".into(),
        ScalarDomain::Positive => "positive".into(),
        ScalarDomain::Bounded { lower, upper } => format!("range {lower} to {upper}"),
        ScalarDomain::Periodic { period } => format!("period {period}"),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn annotation_family_name(
    kind: geosolve_constraint_editor::SceneAnnotationKind,
) -> &'static str {
    use geosolve_constraint_editor::{SceneAnnotationKind, SceneConstraintGlyph};

    match kind {
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Fixed) => "Fixed constraint",
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Coincident) => {
            "Coincident constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Horizontal) => {
            "Horizontal constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Vertical) => "Vertical constraint",
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::PointOnCurve) => {
            "Point-on-curve constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Parallel) => "Parallel constraint",
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Perpendicular) => {
            "Perpendicular constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Concentric) => {
            "Concentric constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Collinear) => "Collinear constraint",
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::EqualLength) => {
            "Equal-length constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::EqualRadius) => {
            "Equal-radius constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Midpoint) => "Midpoint constraint",
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Symmetry) => "Symmetry constraint",
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Contact) => {
            "Curve-contact constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Tangency) => "Tangency constraint",
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Direction) => {
            "Tangent-direction constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Normal) => {
            "Normal-direction constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::EqualCurvature) => {
            "Equal-curvature constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Continuity) => {
            "Endpoint-continuity constraint"
        }
        SceneAnnotationKind::Constraint(SceneConstraintGlyph::Fillet) => "Fillet constraint",
        SceneAnnotationKind::PointDistance => "Point-distance dimension",
        SceneAnnotationKind::CurveLength => "Curve-length dimension",
        SceneAnnotationKind::Radius => "Radius dimension",
        SceneAnnotationKind::Diameter => "Diameter dimension",
        SceneAnnotationKind::OrientedAngle => "Oriented-angle dimension",
        SceneAnnotationKind::SupportingLineOffset => "Supporting-line offset dimension",
        SceneAnnotationKind::ExactTranslatedSegmentOffset => {
            "Exact translated-segment offset dimension"
        }
        SceneAnnotationKind::ProfileOffset => "Profile offset dimension",
    }
}

/// Exactly one nonmodal tool-option family may occupy the canvas overlay stack.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OptionOverlayKind {
    GeometryFamily(geosolve_constraint_editor::GeometryToolFamily),
    Equal,
    Tangent,
    Continuity,
    Dimension(geosolve_constraint_editor::DimensionKind),
    Fillet,
    Offset,
    ConstructionDisplay,
}

#[cfg(any(target_arch = "wasm32", test))]
impl OptionOverlayKind {
    const fn for_authoring_tool(tool: geosolve_constraint_editor::AuthoringTool) -> Option<Self> {
        use geosolve_constraint_editor::{AuthoringTool, ConstraintIntent};

        match tool {
            AuthoringTool::Constraint(ConstraintIntent::Equal) => Some(Self::Equal),
            AuthoringTool::Constraint(ConstraintIntent::Tangent) => Some(Self::Tangent),
            AuthoringTool::Constraint(ConstraintIntent::Continuity) => Some(Self::Continuity),
            AuthoringTool::Dimension(kind) => Some(Self::Dimension(kind)),
            AuthoringTool::Constraint(_) => None,
        }
    }

    const fn key(self) -> &'static str {
        use geosolve_constraint_editor::DimensionKind;

        match self {
            Self::GeometryFamily(family) => match family {
                geosolve_constraint_editor::GeometryToolFamily::Point => "geometry-point",
                geosolve_constraint_editor::GeometryToolFamily::Lines => "geometry-lines",
                geosolve_constraint_editor::GeometryToolFamily::Rectangles => "geometry-rectangles",
                geosolve_constraint_editor::GeometryToolFamily::Circles => "geometry-circles",
                geosolve_constraint_editor::GeometryToolFamily::Arcs => "geometry-arcs",
                geosolve_constraint_editor::GeometryToolFamily::Ellipses => "geometry-ellipses",
                geosolve_constraint_editor::GeometryToolFamily::Beziers => "geometry-beziers",
                geosolve_constraint_editor::GeometryToolFamily::Conics => "geometry-conics",
                geosolve_constraint_editor::GeometryToolFamily::Splines => "geometry-splines",
                _ => "geometry",
            },
            Self::Equal => "equal",
            Self::Tangent => "tangent",
            Self::Continuity => "continuity",
            Self::Dimension(DimensionKind::PointDistance) => "dimension-point-distance",
            Self::Dimension(DimensionKind::SegmentLength) => "dimension-segment-length",
            Self::Dimension(DimensionKind::Radius) => "dimension-radius",
            Self::Dimension(DimensionKind::Diameter) => "dimension-diameter",
            Self::Dimension(DimensionKind::OrientedAngle) => "dimension-oriented-angle",
            Self::Fillet => "fillet",
            Self::Offset => "offset",
            Self::ConstructionDisplay => "construction-display",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        use geosolve_constraint_editor::DimensionKind;

        Some(match key {
            "equal" => Self::Equal,
            "tangent" => Self::Tangent,
            "continuity" => Self::Continuity,
            "dimension-point-distance" => Self::Dimension(DimensionKind::PointDistance),
            "dimension-segment-length" => Self::Dimension(DimensionKind::SegmentLength),
            "dimension-radius" => Self::Dimension(DimensionKind::Radius),
            "dimension-diameter" => Self::Dimension(DimensionKind::Diameter),
            "dimension-oriented-angle" => Self::Dimension(DimensionKind::OrientedAngle),
            "fillet" => Self::Fillet,
            "offset" => Self::Offset,
            "construction-display" => Self::ConstructionDisplay,
            _ => return None,
        })
    }

    const fn title(self) -> &'static str {
        use geosolve_constraint_editor::DimensionKind;

        match self {
            Self::GeometryFamily(family) => geometry_palette::family_label(family),
            Self::Equal => "Equal options",
            Self::Tangent => "Tangent options",
            Self::Continuity => "Continuity options",
            Self::Dimension(DimensionKind::PointDistance) => "Point distance options",
            Self::Dimension(DimensionKind::SegmentLength) => "Segment length options",
            Self::Dimension(DimensionKind::Radius) => "Radius options",
            Self::Dimension(DimensionKind::Diameter) => "Diameter options",
            Self::Dimension(DimensionKind::OrientedAngle) => "Oriented angle options",
            Self::Fillet => "Fillet options",
            Self::Offset => "Offset",
            Self::ConstructionDisplay => "Canvas display",
        }
    }

    const fn first_control_id(self) -> &'static str {
        match self {
            Self::GeometryFamily(_) => "wb-geometry-variant-list",
            Self::Equal => "wb-authoring-curvature",
            Self::Tangent => "wb-authoring-tangent-orientation",
            Self::Continuity => "wb-authoring-continuity",
            Self::Dimension(_) => "wb-authoring-dimension-mode",
            Self::Fillet => "wb-feature-fillet-radius",
            Self::Offset => "wb-offset-distance",
            Self::ConstructionDisplay => "wb-geometry-pick-scope",
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct OptionOverlayState {
    open: Option<OptionOverlayKind>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl OptionOverlayState {
    fn open(&mut self, kind: OptionOverlayKind) {
        self.open = Some(kind);
    }

    fn close(&mut self) {
        self.open = None;
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn offset_operand_status(authoring: &geosolve_constraint_editor::OffsetAuthoringState) -> String {
    if let Some(message) = authoring
        .hover()
        .and_then(|hover| hover.availability.message())
    {
        return format!("Unavailable · {message}");
    }
    let operand = authoring.operand();
    let Some(operand) = operand else {
        return "Select a face or curve".into();
    };
    let count = operand.span_count();
    match operand {
        geosolve_constraint_editor::OffsetAuthoringOperand::OpenChain { .. } => format!(
            "{} · {count} ordered edge{} · Start → End",
            operand.kind_label(),
            if count == 1 { "" } else { "s" },
        ),
        geosolve_constraint_editor::OffsetAuthoringOperand::Face { .. } => format!(
            "{} · {count} edge{}",
            operand.kind_label(),
            if count == 1 { "" } else { "s" },
        ),
    }
}

#[cfg(target_arch = "wasm32")]
const fn offset_direction_label(
    operand: Option<&geosolve_constraint_editor::OffsetAuthoringOperand>,
) -> &'static str {
    use geosolve_constraint_editor::OffsetAuthoringOperand;
    use geosolve_sketch::{DocumentFaceOffsetDirection, DocumentLineSide};

    match operand {
        Some(OffsetAuthoringOperand::Face {
            direction: DocumentFaceOffsetDirection::Outward,
            ..
        }) => "Outward",
        Some(OffsetAuthoringOperand::Face {
            direction: DocumentFaceOffsetDirection::Inward,
            ..
        }) => "Inward",
        Some(OffsetAuthoringOperand::OpenChain {
            side: DocumentLineSide::Left,
            ..
        }) => "Left",
        Some(OffsetAuthoringOperand::OpenChain {
            side: DocumentLineSide::Right,
            ..
        }) => "Right",
        None => "—",
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn offset_canvas_presentation(
    authoring: &geosolve_constraint_editor::OffsetAuthoringState,
) -> scene::OffsetCanvasPresentation {
    use geosolve_constraint_editor::{
        OffsetAuthoringOperand, OffsetAuthoringTarget, OffsetAuthoringTargetAvailability,
        SelectionItem,
    };

    fn push_target(items: &mut Vec<SelectionItem>, target: &OffsetAuthoringTarget) {
        match target {
            OffsetAuthoringTarget::Face(key) => key
                .outer
                .spans
                .iter()
                .chain(key.holes.iter().flat_map(|hole| &hole.spans))
                .for_each(|directed| items.push(SelectionItem::Curve(directed.span))),
            OffsetAuthoringTarget::Span(span) => items.push(SelectionItem::Curve(*span)),
        }
    }

    let mut presentation = scene::OffsetCanvasPresentation {
        chain: authoring.chain_presentation(),
        ..scene::OffsetCanvasPresentation::default()
    };
    match authoring.operand() {
        Some(OffsetAuthoringOperand::Face { key, .. }) => {
            key.outer
                .spans
                .iter()
                .chain(key.holes.iter().flat_map(|hole| &hole.spans))
                .for_each(|directed| {
                    presentation
                        .pending
                        .push(SelectionItem::Curve(directed.span));
                });
        }
        Some(OffsetAuthoringOperand::OpenChain { spans, .. }) => {
            for directed in spans {
                presentation
                    .pending
                    .push(SelectionItem::Curve(directed.span));
            }
        }
        None => {}
    }
    if let Some(hover) = authoring.hover() {
        match &hover.availability {
            OffsetAuthoringTargetAvailability::Available => {
                push_target(&mut presentation.pending, &hover.target);
            }
            OffsetAuthoringTargetAvailability::Unavailable { message, .. } => {
                push_target(&mut presentation.unavailable, &hover.target);
                presentation.unavailable_message = Some(message.clone());
            }
        }
    }
    presentation.pending.sort_unstable();
    presentation.pending.dedup();
    presentation.unavailable.sort_unstable();
    presentation.unavailable.dedup();
    presentation
}

#[cfg(any(target_arch = "wasm32", test))]
fn offset_target_for_selection(
    item: geosolve_constraint_editor::SelectionItem,
) -> Option<geosolve_constraint_editor::OffsetAuthoringTarget> {
    match item {
        geosolve_constraint_editor::SelectionItem::Curve(span) => Some(
            geosolve_constraint_editor::OffsetAuthoringTarget::Span(span),
        ),
        _ => None,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn offset_click_owns_semantic_pick(is_canvas_item: bool, is_pointer_click: bool) -> bool {
    !is_canvas_item || !is_pointer_click
}

/// Presentation-only disclosure state for an exact current value.
#[cfg(any(target_arch = "wasm32", test))]
struct DismissibleDisclosure<T> {
    current: Option<T>,
    dismissed: Option<T>,
    manual_open: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
impl<T> Default for DismissibleDisclosure<T> {
    fn default() -> Self {
        Self {
            current: None,
            dismissed: None,
            manual_open: false,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl<T: Clone + PartialEq> DismissibleDisclosure<T> {
    fn reconcile(&mut self, current: Option<&T>) -> bool {
        if self.current.as_ref() != current {
            let recovered = self.current.is_some() && current.is_none();
            self.current = current.cloned();
            self.dismissed = None;
            self.manual_open = current.is_some() && !recovered;
        }
        current.map_or(self.manual_open, |value| {
            self.dismissed.as_ref() != Some(value)
        })
    }

    fn dismiss(&mut self, current: Option<&T>) {
        self.dismissed = current.cloned();
        self.manual_open = false;
    }

    fn reopen(&mut self) {
        self.dismissed = None;
        self.manual_open = true;
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, PartialEq)]
struct ProblemSetIdentity {
    sketch: Option<geosolve_constraint_editor::EditorProblemMetadata>,
    computed: Vec<geosolve_constraint_editor::ComputedFeatureProblemMetadata>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectionalProblemIdentity {
    session: geosolve_sketch_intent::IntentSessionIdentity,
    diagnostic: geosolve_sketch_intent::IntentKey,
}

#[cfg(target_arch = "wasm32")]
impl ProblemSetIdentity {
    fn current(
        coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
    ) -> Option<Self> {
        let sketch = coordinator.current_problem_metadata();
        let computed = coordinator.computed_feature_problems();
        (sketch.is_some() || !computed.is_empty()).then_some(Self { sketch, computed })
    }
}

#[cfg(target_arch = "wasm32")]
fn markup_fingerprint(value: &str) -> String {
    let hash = value
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("{hash:016x}")
}

/// Browser-render identity paired with one exact headless computed input.
///
/// Owner/action IDs in markup are intentionally insufficient to activate a
/// Fillet branch. A DOM control also carries this opaque stamp, and an event is
/// admitted only while the adapter still holds the exact input that produced
/// that stamp. This prevents an old element from being silently upgraded to a
/// newer feature revision by rebuilding a target from persistent IDs.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct FilletActionRenderAuthority {
    next_stamp: u64,
    active: Option<(
        u64,
        geosolve_sketch_features::ComputedFeatureEvaluationInput,
    )>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl FilletActionRenderAuthority {
    fn reconcile(
        &mut self,
        input: Option<&geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    ) -> Option<u64> {
        let Some(input) = input else {
            self.active = None;
            return None;
        };
        if let Some((stamp, active)) = self.active
            && active == *input
        {
            return Some(stamp);
        }
        let Some(stamp) = self.next_stamp.checked_add(1) else {
            self.active = None;
            return None;
        };
        self.next_stamp = stamp;
        self.active = Some((stamp, *input));
        Some(stamp)
    }

    fn accepts(
        &self,
        stamp: u64,
        input: Option<&geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    ) -> bool {
        matches!((self.active, input), (Some((active_stamp, active)), Some(current))
            if active_stamp == stamp && active == *current)
    }
}

/// Reconciles every painted action below one canvas sample with the
/// headless nearest-action result.
///
/// SVG action hit corridors can overlap. Their paint order is presentation
/// detail, so the topmost corridor must not suppress a closer independently
/// validated action. A stale or foreign stack still produces no route.
#[cfg(any(target_arch = "wasm32", test))]
fn resolve_canvas_fillet_action_candidates(
    scene: &geosolve_constraint_editor::EditorScene,
    policy: geosolve_constraint_editor::GeometryInteractionPolicy,
    position: geosolve_constraint_editor::ScreenPoint,
    painted: impl IntoIterator<Item = geosolve_constraint_editor::SceneFilletActionTarget>,
) -> Option<geosolve_constraint_editor::SceneFilletActionTarget> {
    painted.into_iter().find(|target| {
        scene.resolve_fillet_action_with_policy(
            geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                position,
                painted: Some(*target),
            },
            geosolve_constraint_editor::PickTolerance::default(),
            policy,
        ) == Some(*target)
    })
}

/// Reconciles the complete browser paint stack with one headless radius hit.
///
/// A native point or curve can be painted above the selected computed Fillet
/// grip. Paint order is presentation detail: when the headless scene resolves
/// an exact radius owner, that owner remains the intent hint if it occurs
/// anywhere in the browser stack. The coordinator still authenticates the
/// retained preview, scene provenance, policy, and exact hit before hover or
/// pointer-down can consume the hint. Without a matching headless radius hit,
/// the top painted item is retained and no browser-side semantic priority is
/// invented.
#[cfg(any(target_arch = "wasm32", test))]
fn reconcile_feature_authoring_painted_items(
    radius_owner: Option<geosolve_sketch_features::ComputedCornerRef>,
    painted: impl IntoIterator<Item = geosolve_constraint_editor::SelectionItem>,
) -> Option<geosolve_constraint_editor::SelectionItem> {
    let mut first = None;
    for item in painted {
        if first.is_none() {
            first = Some(item);
        }
        if matches!(
            (radius_owner, item),
            (
                Some(expected),
                geosolve_constraint_editor::SelectionItem::FeatureCorner(actual)
            ) if actual == expected
        ) {
            return Some(item);
        }
    }
    first
}

/// Revokes one temporary computed-feature owner. Selection cleanup belongs to
/// the headless coordinator so every caller gets identical lifetime semantics.
#[cfg(any(target_arch = "wasm32", test))]
fn revoke_held_feature_authoring_preview(
    coordinator: &mut geosolve_constraint_editor::RetainedEditorCoordinator,
) {
    coordinator.clear_feature_authoring_preview();
}

/// Synchronizes temporary preview lifetime with every headless transition that
/// no longer exposes a complete candidate.
#[cfg(any(target_arch = "wasm32", test))]
fn observe_feature_authoring_preview_lifecycle(
    coordinator: &mut geosolve_constraint_editor::RetainedEditorCoordinator,
    outcome: &geosolve_constraint_editor::FeatureAuthoringOutcome,
) {
    if matches!(
        outcome,
        geosolve_constraint_editor::FeatureAuthoringOutcome::ModeEntered(_)
            | geosolve_constraint_editor::FeatureAuthoringOutcome::Collecting { .. }
            | geosolve_constraint_editor::FeatureAuthoringOutcome::CandidateCleared(_)
            | geosolve_constraint_editor::FeatureAuthoringOutcome::ModeExited
    ) {
        revoke_held_feature_authoring_preview(coordinator);
    }
}

/// Browser presentation for the second, explicitly native Fillet publication choice.
///
/// Visibility follows the ordinary complete-preview action, while enabled state comes only from
/// the coordinator's exact held-preview/topology authentication. The adapter never guesses from
/// curve labels or browser paint state.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeFilletApplyPresentation {
    visible: bool,
    disabled: bool,
    reason: Option<String>,
}

#[cfg(any(target_arch = "wasm32", test))]
fn native_fillet_apply_presentation(
    coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
    candidate: Option<&geosolve_constraint_editor::FeatureAuthoringCandidate>,
    stage: geosolve_constraint_editor::FeatureAuthoringStage,
) -> NativeFilletApplyPresentation {
    let Some(candidate) = candidate
        .filter(|_| stage == geosolve_constraint_editor::FeatureAuthoringStage::PreviewReady)
    else {
        return NativeFilletApplyPresentation {
            visible: false,
            disabled: true,
            reason: None,
        };
    };
    let availability = coordinator
        .feature_authoring_preview()
        .ok_or_else(|| "the exact current Fillet preview is unavailable".to_owned())
        .and_then(|preview| {
            coordinator
                .native_feature_authoring_availability(preview.metadata().token, candidate)
                .map_err(|error| concise_native_fillet_unavailable(&error.to_string()))
        });
    match availability {
        Ok(()) => NativeFilletApplyPresentation {
            visible: true,
            disabled: false,
            reason: None,
        },
        Err(reason) => NativeFilletApplyPresentation {
            visible: true,
            disabled: true,
            reason: Some(reason),
        },
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn concise_native_fillet_unavailable(message: &str) -> String {
    message
        .strip_prefix("native Fillet output is unavailable: ")
        .unwrap_or(message)
        .to_owned()
}

/// A successful feature publication hides its invoking guide control and exits authoring. Move
/// keyboard focus to the newly active Select tool; a rejected publication keeps its authoring
/// surface and current focus intact.
#[cfg(any(target_arch = "wasm32", test))]
fn feature_apply_returns_focus_to_select(action: &str, feature_authoring_active: bool) -> bool {
    matches!(action, "feature-apply" | "feature-apply-native") && !feature_authoring_active
}

/// Publishes the exact held preview as native Profile geometry and performs the complete shared
/// workbench success transition. Any error preserves the authoring candidate, overlay and
/// selection so a stale or unsupported click is presentation-neutral.
#[cfg(any(target_arch = "wasm32", test))]
fn apply_native_fillet_profile(
    coordinator: &mut geosolve_constraint_editor::RetainedEditorCoordinator,
    authoring: &mut geosolve_constraint_editor::FeatureAuthoringState,
    candidate: &mut Option<geosolve_constraint_editor::FeatureAuthoringCandidate>,
    pending: &mut Vec<geosolve_constraint_editor::FeatureAuthoringPick>,
    overlay: &mut OptionOverlayState,
) -> Result<Vec<geosolve_constraint_editor::EditorEffect>, String> {
    let candidate_value = candidate
        .as_ref()
        .ok_or_else(|| "the exact current Fillet candidate is unavailable".to_owned())?;
    let preview = coordinator
        .feature_authoring_preview()
        .map(|preview| preview.metadata().clone())
        .ok_or_else(|| "the exact current Fillet preview is unavailable".to_owned())?;
    let mutation = coordinator
        .apply_feature_authoring_native_profile(preview.token, candidate_value)
        .map_err(|error| error.to_string())?;
    let effects = coordinator
        .editor_mut()
        .activate_tool(geosolve_constraint_editor::EditorTool::Select);
    coordinator.set_selection([geosolve_constraint_editor::SelectionItem::Curve(
        geosolve_sketch::CurveSpan::line(mutation.value.arc),
    )]);
    let _ = authoring.publication_succeeded();
    candidate.take();
    pending.clear();
    overlay.close();
    Ok(effects)
}

/// Runs the state-changing half of a reproduction load only after the complete
/// replacement has been decoded and independently validated.
#[cfg(any(target_arch = "wasm32", test))]
fn apply_validated_reproduction<State, Candidate, Error>(
    state: &mut State,
    validate: impl FnOnce() -> Result<Candidate, Error>,
    commit: impl FnOnce(&mut State, Candidate) -> Result<(), Error>,
) -> Result<(), Error> {
    let candidate = validate()?;
    commit(state, candidate)
}

#[cfg(any(target_arch = "wasm32", test))]
const fn reproduction_overlay_presentation(open: bool) -> (&'static str, bool) {
    (if open { "true" } else { "false" }, !open)
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ReproductionOverlayMode {
    #[default]
    Payload,
    Trace,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ReproductionOverlayMode {
    const fn is_trace(self) -> bool {
        matches!(self, Self::Trace)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ReproductionFocusReturn {
    Copy,
    Trace,
    #[default]
    Load,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ReproductionFocusReturn {
    const fn element_id(self) -> &'static str {
        match self {
            Self::Copy => "wb-reproduction-copy-trigger",
            Self::Trace => "wb-interaction-trace-copy-trigger",
            Self::Load => "wb-reproduction-load-trigger",
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn reproduction_focus_target_after_action(
    action: &str,
    overlay_open: bool,
    return_to: ReproductionFocusReturn,
) -> Option<&'static str> {
    (action == "reproduction-close" || (action == "reproduction-load" && !overlay_open))
        .then(|| return_to.element_id())
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ForegroundOverlayEscapeOwner {
    Reproduction,
    Samples,
    None,
}

#[cfg(any(target_arch = "wasm32", test))]
const fn foreground_overlay_escape_owner(
    reproduction_open: bool,
    samples_open: bool,
) -> ForegroundOverlayEscapeOwner {
    if reproduction_open {
        ForegroundOverlayEscapeOwner::Reproduction
    } else if samples_open {
        ForegroundOverlayEscapeOwner::Samples
    } else {
        ForegroundOverlayEscapeOwner::None
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn should_route_stationary_draft_inference(
    reproduction_open: bool,
    ordinary_owner: bool,
) -> bool {
    ordinary_owner && !reproduction_open
}

#[cfg(any(target_arch = "wasm32", test))]
fn reproduction_payload_size_label(bytes: usize) -> String {
    format!("{bytes} payload bytes")
}

/// Composes the best honest native/computed scene for one retained coordinator.
///
/// A historical accepted state beneath a newer rejected design remains valid
/// presentation geometry, but it deliberately lacks authority to publish inferred
/// construction. Keep that scene detached instead of confusing the missing authority
/// with missing geometry. Current computed output remains fail-closed on any provenance
/// or affordance-composition error; only the historical presentation row is detached.
#[cfg(any(target_arch = "wasm32", test))]
fn compose_editor_scene(
    coordinator: &geosolve_constraint_editor::RetainedEditorCoordinator,
    viewport: geosolve_constraint_editor::Viewport,
    chord_tolerance_pixels: f64,
) -> Option<geosolve_constraint_editor::EditorScene> {
    use geosolve_constraint_editor::{ComputedSceneState, EditorScene, SelectionItem};

    let source = coordinator
        .visible_preview_session()
        .unwrap_or(coordinator.session());
    let accepted = source.accepted_state()?;
    let current_accepted = source.accepted_state_for_current_input().is_some();
    let prepared_curve_preview = coordinator.curve_control_preview_active();
    let scene_revision = accepted.identity().revision().get();
    let scene_design_identity = source.design_identity();
    let native_scene = || {
        EditorScene::from_accepted_for_design(
            scene_revision,
            scene_design_identity,
            accepted.document(),
            source.design_document(),
            viewport,
            chord_tolerance_pixels,
        )
        .ok()
    };
    if !current_accepted {
        return native_scene();
    }
    let scene = match coordinator.computed_scene_state() {
        ComputedSceneState::Current { expected, snapshot } => {
            let accepted_input = source.accepted_prepared_input()?;
            let mut scene = EditorScene::from_accepted_with_computed(
                scene_revision,
                scene_design_identity,
                accepted.document(),
                source.design_document(),
                &accepted_input,
                expected,
                snapshot,
                viewport,
                chord_tolerance_pixels,
            )
            .ok()?;
            let mut action_items = coordinator.editor().selection().to_vec();
            if let Some(preview) = coordinator.feature_authoring_preview() {
                action_items.push(SelectionItem::Feature(preview.metadata().feature));
                action_items.sort_unstable();
                action_items.dedup();
            }
            coordinator
                .populate_computed_fillet_affordances(
                    &mut scene,
                    &action_items,
                    chord_tolerance_pixels,
                )
                .ok()?;
            scene
        }
        ComputedSceneState::Withheld | ComputedSceneState::Absent => native_scene()?,
    };
    let mut scene = scene;
    if !scene.update_annotation_values(accepted) {
        return None;
    }
    scene.apply_annotation_layout(&coordinator.editor().annotation_layout_for_scene());
    // A prepared curve-control candidate keeps truthful candidate geometry/computed provenance
    // and remains detached from drafting authority. The coordinator separately authenticates the
    // durable pointer-down origin after the exact selected-control layer has been rebuilt.
    let mut scene = if prepared_curve_preview {
        scene
    } else {
        scene.with_retained_session(source).ok()?
    };
    coordinator
        .editor()
        .populate_curve_controls(&mut scene)
        .ok()?;
    if matches!(
        coordinator.editor().active_pointer_gesture(),
        Some(geosolve_constraint_editor::ActivePointerGesture {
            kind: geosolve_constraint_editor::ActivePointerGestureKind::OffsetDistance,
            ..
        })
    ) {
        coordinator
            .retain_offset_distance_interaction_origin(&mut scene)
            .ok()?;
    }
    if prepared_curve_preview {
        coordinator
            .retain_curve_control_preview_interaction_origin(&mut scene)
            .ok()?;
    }
    Some(scene)
}

/// Stable retained-SVG owners for one semantic canvas item.
///
/// A span or computed feature may have multiple painted occurrences, so the
/// hover presenter must update every matching element rather than assuming
/// persistent identity is unique in the SVG.
#[cfg(any(target_arch = "wasm32", test))]
fn projectional_item_element_ids(
    scene: &geosolve_constraint_editor::EditorScene,
    item: geosolve_constraint_editor::SelectionItem,
) -> Vec<String> {
    use geosolve_constraint_editor::SelectionItem;

    match item {
        SelectionItem::Point(point) => scene
            .points
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.id == point)
            .map(|(index, _)| format!("wb-scene-point-{index}"))
            .collect(),
        SelectionItem::Curve(span) => scene
            .curves
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.span == span)
            .map(|(index, _)| format!("wb-scene-curve-{index}"))
            .collect(),
        SelectionItem::Datum(datum) => {
            if datum == geosolve_sketch::SketchDatum::Origin {
                Vec::new()
            } else {
                scene
                    .datums
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| candidate.datum == datum)
                    .map(|(index, _)| format!("wb-scene-datum-{index}"))
                    .collect()
            }
        }
        SelectionItem::FeatureCorner(corner) => scene
            .computed_curves
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.owner == corner)
            .map(|(index, _)| format!("wb-scene-computed-curve-{index}"))
            .collect(),
        SelectionItem::Feature(feature) => scene
            .computed_curves
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.owner.feature == feature)
            .map(|(index, _)| format!("wb-scene-computed-curve-{index}"))
            .collect(),
        SelectionItem::Constraint(_) | SelectionItem::Dimension(_) => Vec::new(),
    }
}

/// Origin is intentionally represented by the protected X/Y-axis
/// intersection and therefore has no duplicate retained SVG owner. Every
/// other scene-mapped item must resolve to at least one concrete DOM node.
#[cfg(any(target_arch = "wasm32", test))]
fn projectional_item_intentionally_has_no_dom_owner(
    item: geosolve_constraint_editor::SelectionItem,
) -> bool {
    matches!(
        item,
        geosolve_constraint_editor::SelectionItem::Datum(geosolve_sketch::SketchDatum::Origin)
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn projectional_hover_intentionally_has_no_dom_owner(
    target: geosolve_constraint_editor::EditorHoverTarget,
) -> bool {
    matches!(
        target,
        geosolve_constraint_editor::EditorHoverTarget::Geometry(item)
            if projectional_item_intentionally_has_no_dom_owner(item)
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn projectional_annotation_element_id(
    scene: &geosolve_constraint_editor::EditorScene,
    item: geosolve_constraint_editor::SelectionItem,
) -> Option<String> {
    use geosolve_constraint_editor::SelectionItem;

    if !matches!(
        item,
        SelectionItem::Constraint(_) | SelectionItem::Dimension(_)
    ) {
        return None;
    }
    scene
        .annotations
        .iter()
        .position(|annotation| annotation.item == item)
        .map(|index| format!("wb-scene-annotation-{index}"))
}

/// Exact retained DOM class target for a headless hover occurrence.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectionalHoverDomTarget {
    Geometry(String),
    CurveControl(String),
    CurveControlGuide(String),
    AnnotationRoot(String),
    AnnotationMarker { root: String, selector: String },
}

#[cfg(any(target_arch = "wasm32", test))]
fn projectional_hover_dom_targets(
    scene: &geosolve_constraint_editor::EditorScene,
    target: geosolve_constraint_editor::EditorHoverTarget,
) -> Vec<ProjectionalHoverDomTarget> {
    use geosolve_constraint_editor::EditorHoverTarget;

    match target {
        EditorHoverTarget::Geometry(item) => projectional_item_element_ids(scene, item)
            .into_iter()
            .map(ProjectionalHoverDomTarget::Geometry)
            .collect(),
        EditorHoverTarget::CurveControl { control, owner } => {
            let mut targets = Vec::new();
            if let Some(index) = scene.curve_controls.iter().position(|candidate| {
                candidate.id == control
                    && candidate.owner == owner
                    && matches!(
                        candidate.interaction,
                        geosolve_constraint_editor::SceneCurveControlInteraction::Direct
                    )
            }) {
                targets.push(ProjectionalHoverDomTarget::CurveControl(format!(
                    "wb-scene-control-{index}"
                )));
            }
            targets.extend(
                scene
                    .curve_control_guides
                    .iter()
                    .enumerate()
                    .filter(|(_, guide)| guide.control == Some(control))
                    .map(|(index, _)| {
                        ProjectionalHoverDomTarget::CurveControlGuide(format!(
                            "wb-scene-control-guide-{index}"
                        ))
                    }),
            );
            targets
        }
        EditorHoverTarget::Annotation(occurrence) => {
            let Some(root) = projectional_annotation_element_id(scene, occurrence.item) else {
                return Vec::new();
            };
            if let Some(index) = occurrence.marker_index {
                vec![ProjectionalHoverDomTarget::AnnotationMarker {
                    root,
                    selector: format!(".wb-constraint-symbol[data-annotation-marker=\"{index}\"]"),
                }]
            } else {
                vec![ProjectionalHoverDomTarget::AnnotationRoot(root)]
            }
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn projectional_effects_are_retained_hover_only(
    effects: &[geosolve_constraint_editor::EditorEffect],
) -> bool {
    use geosolve_constraint_editor::EditorEffect;

    effects.iter().all(|effect| {
        matches!(
            effect,
            EditorEffect::HoverChanged(_)
                | EditorEffect::FilletBranchPreviewChanged { target: None }
        )
    })
}

/// The flat retained presenter admits exactly the same headless effect set as
/// the projectional route. Keeping this explicit adapter seam makes parity a
/// native-testable contract while both browser workbenches remain available.
#[cfg(any(target_arch = "wasm32", test))]
fn flat_effects_are_retained_hover_only(
    effects: &[geosolve_constraint_editor::EditorEffect],
) -> bool {
    projectional_effects_are_retained_hover_only(effects)
}

#[cfg(test)]
mod tests {
    mod golden_scene_backend_parity {
        include!("golden_scene_backend_parity.rs");
    }

    use geosolve_constraint_editor::{
        ActivePointerGesture, ActivePointerGestureKind, AuthoringOperand, AuthoringOutcome,
        AuthoringState, AuthoringTool, ColdIntentMaterializer, ComputedSceneState,
        ConstraintEditor, ConstraintIntent, DimensionKind, DraftInferenceCandidateId,
        DraftInferenceCompleteness, DraftInferenceResolution, DraftInferenceStatus,
        EditorHoverState, EditorHoverTarget, EditorProblemScope, EditorScene, EditorTool,
        FeatureAuthoringCandidate, FeatureAuthoringOptions, FeatureAuthoringOutcome,
        FeatureAuthoringPreviewMetadata, FeatureAuthoringStage, FeatureAuthoringState,
        FeatureAuthoringTool, GeometryDraftBranch, GeometryDraftStage, GeometryDraftStatus,
        GeometryInteractionPolicy, GeometryPickScope, GeometryToolVariant, GeometryVisibility,
        IntentGraphNodeKind, IntentInspectorField, IntentInspectorProjection, Modifiers,
        OffsetAuthoringOutcome, OffsetAuthoringState, OffsetAuthoringWarning,
        OffsetAuthoringWarningKind, PickTolerance, PointerInput, ProjectionalEditorSession,
        ProjectionalIntentCoordinator, RetainedEditorCoordinator, SceneAnnotationGeometry,
        SceneAnnotationKind, SceneAnnotationOccurrence, SceneAnnotationVisibility,
        SceneConstraintGlyph, SceneCurveOrigin, ScreenPoint, SelectionItem, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId,
        DocumentArcSweep, DocumentBSplineForm, DocumentConstraintDefinition,
        DocumentCurveNormalSide, DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit,
        DocumentId, DocumentSolveRequest, GeometryRole, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
        SketchAcceptedStateIdentity, SketchDocument,
    };
    use geosolve_sketch_intent::{
        GeometryRecipeKind, IntentDeclarationDescriptor, IntentDefinitionFieldDescriptor,
        IntentDefinitionFieldSchema, IntentEditClassification, IntentFieldChoices,
        IntentFieldDefault, IntentFieldKey, IntentKey, IntentLiteral, IntentLiteralSchema,
        IntentNodeDraft, IntentNodeKind, IntentNodeSchema, IntentPatch, IntentPatchOperation,
        IntentPatchPolicy, IntentPlanDisposition, IntentPortRole, IntentPortSelector,
        IntentProjectionPath, IntentSession, IntentSessionId, IntentSessionIdentity, IntentUnit,
        LeafField, NodeId,
    };

    use super::ProjectionalCellMove;
    use super::persistence::WorkspaceSnapshot;
    use super::{
        AuthoringItemInput, CANVAS_PAN_POINTER_EVENTS, CANVAS_POINTER_TERMINAL_EVENTS,
        CameraToolbarAction, CanvasPanPointerDownRoute, CanvasPointerCaptureKind,
        CanvasPointerCaptures, CanvasPointerContextRoute, CanvasPointerMoveOwner,
        CanvasPointerOwnership, CanvasPointerTerminal, CanvasPointerTerminalDisposition,
        CanvasPrimaryPointerDownRoute, CapturedCanvasPointer, DismissibleDisclosure,
        DraftingPointerSample, FilletActionRenderAuthority, FinishDoubleClickTracker,
        ForegroundOverlayEscapeOwner, HistoryShortcut, OptionOverlayKind, OptionOverlayState,
        PointerMoveQueue, ProjectionalConstructionDispatch, ProjectionalInspectorControl,
        ProjectionalInspectorDispatch, ProjectionalInspectorStamp, ProjectionalInspectorSubmission,
        ProjectionalPointerMoveQueue, ReproductionFocusReturn, ReproductionOverlayMode,
        RetainedCameraQueue, WorkbenchDocumentAuthority, WorkbenchPresentationCounters,
        WorkbenchPresentationEvent, WorkbenchRenderScope, annotation_family_name,
        annotation_inspector_presentation, apply_native_fillet_profile,
        apply_projectional_scene_display, apply_validated_reproduction,
        canvas_cursor_key_with_curve_control, canvas_pointer_capture_kind,
        canvas_pointer_move_owner, change_owns_option_control_click, compose_editor_scene,
        coordinate_hud, current_problem_items, curve_control_inspector_detail,
        curve_control_inspector_markup, decode_projectional_inspector_control,
        delegate_projectional_code_fillet_radius_drag, dispatch_projectional_authoring_application,
        dispatch_projectional_construction_effects, dispatch_projectional_inspector_control,
        draft_inference_preference_is_stale, feature_apply_returns_focus_to_select,
        foreground_overlay_escape_owner, geometry_sweep_flip_available,
        geometry_variant_keyboard_target, history_shortcut, native_fillet_apply_presentation,
        observe_feature_authoring_preview_lifecycle, offset_canvas_presentation,
        offset_click_owns_semantic_pick, offset_operand_status, offset_target_for_selection,
        owns_authoring_pick, projectional_cell_drop_before, projectional_cell_move_patch,
        projectional_design_markup, projectional_direct_gesture_is_capturable,
        projectional_outline_drop_before, projectional_outline_move_patch,
        projectional_terminal_owns_capture, reconcile_feature_authoring_painted_items,
        reproduction_focus_target_after_action, reproduction_overlay_presentation,
        reproduction_payload_size_label, resolve_canvas_fillet_action_candidates,
        revoke_canvas_pointer_context, revoke_held_feature_authoring_preview,
        route_canvas_pan_pointer_down, route_canvas_primary_pointer_down,
        route_projectional_terminal_capture, should_route_stationary_draft_inference,
    };

    #[test]
    fn open_search_matches_all_terms_independent_of_order_and_case() {
        assert!(super::sample_search_matches(
            "FILLET keyed",
            "Typed panel · keyed Fillets",
        ));
        assert!(super::sample_search_matches(
            "grid stack",
            "Gridfinity stacking profile",
        ));
        assert!(super::sample_search_matches("   ", "Any sample"));
        assert!(!super::sample_search_matches(
            "fillet grid",
            "Typed panel · keyed Fillets",
        ));
    }

    #[test]
    fn recent_sample_shortcuts_are_bounded_deduplicated_and_authority_free() {
        let mut entries = Vec::new();
        for index in 0..10 {
            super::remember_recent_sample(
                &mut entries,
                super::RecentSampleShortcut {
                    kind: if index % 2 == 0 {
                        super::RecentSampleKind::Native
                    } else {
                        super::RecentSampleKind::Code
                    },
                    key: format!("sample-{index}"),
                    title: format!("Sample {index}"),
                },
            );
        }
        assert_eq!(entries.len(), super::MAX_RECENT_SAMPLES);
        assert_eq!(entries[0].key, "sample-9");

        let newest = entries[3].clone();
        super::remember_recent_sample(&mut entries, newest.clone());
        assert_eq!(entries[0], newest);
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.kind == newest.kind && entry.key == newest.key)
                .count(),
            1,
        );
        let encoded = super::encode_recent_samples(&entries).expect("bounded recents");
        assert!(!encoded.contains("managed_source"));
        assert!(!encoded.contains("checkpoint"));
        assert_eq!(super::decode_recent_samples(&encoded), entries);
        assert!(
            super::decode_recent_samples(&"x".repeat(super::MAX_RECENT_SAMPLE_STORAGE_BYTES + 1))
                .is_empty()
        );
    }

    #[test]
    fn current_problem_presentation_counts_current_attempt_not_metadata_rows() {
        assert_eq!(super::current_problem_count(false, false), 0);
        assert_eq!(super::current_problem_count(true, false), 1);
        assert_eq!(super::current_problem_count(false, true), 1);
        assert_eq!(super::current_problem_count(true, true), 1);
    }

    #[test]
    fn current_problem_presentation_prefers_invalid_source_over_retained_intent() {
        let invalid = super::current_problem_presentation(
            Some("Line 1, column 1 · unexpected token"),
            Some("older retained intent"),
        );
        assert_eq!(
            invalid,
            super::CurrentProblemPresentation {
                count: 1,
                message: "Managed source · Line 1, column 1 · unexpected token".into(),
                opens_source_position: true,
            }
        );
        let retained = super::current_problem_presentation(None, Some("current retained intent"));
        assert_eq!(
            retained,
            super::CurrentProblemPresentation {
                count: 1,
                message: concat!(
                    "The latest intent attempt was retained but could not materialize: ",
                    "current retained intent"
                )
                .into(),
                opens_source_position: false,
            }
        );
    }

    #[test]
    fn current_problem_presentation_clears_stale_source_problem_for_valid_bytes() {
        let invalid_bytes = super::current_problem_presentation(Some("unexpected token"), None);
        assert_eq!(invalid_bytes.count, 1);
        assert!(invalid_bytes.opens_source_position);

        let valid_bytes = super::current_problem_presentation(None, None);
        assert_eq!(
            valid_bytes,
            super::CurrentProblemPresentation {
                count: 0,
                message: "No current projectional intent problem".into(),
                opens_source_position: false,
            }
        );
    }

    #[test]
    fn browser_selection_limits_count_utf16_code_units_not_utf8_bytes() {
        assert_eq!(super::utf16_code_unit_length("aé"), 2);
        assert_eq!(super::utf16_code_unit_length("a😀z"), 4);
    }

    #[test]
    fn retained_camera_queue_coalesces_latest_frame_and_authenticates_idle_boundary() {
        let exact = super::scene::CanvasCamera::default();
        let mut queue = RetainedCameraQueue::default();
        queue.exact_reconciled(exact);
        let first = super::scene::CanvasCamera::new([1.0, -2.0], 75.0).expect("valid first camera");
        let latest =
            super::scene::CanvasCamera::new([-3.0, 4.0], 100.0).expect("valid latest camera");

        let frame = queue
            .request_frame(first)
            .expect("first raw event schedules RAF");
        assert_eq!(queue.request_frame(latest), None, "one RAF per frame");
        assert!(queue.take_frame(frame.wrapping_add(1)).is_none());
        let admitted = queue
            .take_frame(frame)
            .expect("current RAF uses newest camera");
        assert_eq!(
            admitted.transform,
            super::scene::RetainedCameraTransform::between(exact, latest)
                .expect("finite retained transform"),
        );
        assert!(admitted.complete(&mut queue));
        assert!(queue.needs_exact_reconciliation());

        let stale_idle = queue.request_idle_reconciliation();
        let current_idle = queue.request_idle_reconciliation();
        assert!(!queue.take_idle_reconciliation(stale_idle));
        assert!(queue.take_idle_reconciliation(current_idle));

        queue.exact_reconciled(latest);
        assert!(!queue.needs_exact_reconciliation());
        assert!(queue.take_frame(frame).is_none());
        assert!(!queue.take_idle_reconciliation(current_idle));
    }

    #[test]
    fn retained_camera_exact_reconciliation_revokes_an_unpainted_frame() {
        let mut queue = RetainedCameraQueue::default();
        let desired =
            super::scene::CanvasCamera::new([8.0, 5.0], 35.0).expect("valid desired camera");
        let stale = queue
            .request_frame(desired)
            .expect("scheduled camera frame");
        assert!(queue.needs_exact_reconciliation());

        queue.exact_reconciled(desired);
        assert!(queue.take_frame(stale).is_none());
        assert!(!queue.needs_exact_reconciliation());
        assert!(queue.request_frame(desired).is_some());
        let identity = queue
            .take_frame(queue.next_frame_generation)
            .expect("matching camera has a finite identity mapping");
        assert_eq!(identity.transform.scale.to_bits(), 1.0_f64.to_bits());
        assert_eq!(
            identity.transform.translate.map(f64::to_bits),
            [0.0, 0.0].map(f64::to_bits)
        );
    }

    #[test]
    fn toolbar_camera_commands_have_route_parity_and_admit_no_durable_work() {
        let mut document = SketchDocument::new(8.0).expect("document");
        document
            .add_rectangle("camera parity", [-2.0, -1.0], 7.0, 4.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            test_viewport(),
            0.8,
        )
        .expect("scene");
        let initial =
            super::scene::CanvasCamera::new([3.5, -4.25], 73.0).expect("valid initial camera");
        let mut projectional_camera = initial;
        let mut flat_camera = initial;
        let mut projectional_queue = RetainedCameraQueue::default();
        let mut flat_queue = RetainedCameraQueue::default();
        projectional_queue.exact_reconciled(initial);
        flat_queue.exact_reconciled(initial);
        let mut projectional_frame = None;
        let mut flat_frame = None;
        let mut projectional_idle = None;
        let mut flat_idle = None;

        for (action, key) in CameraToolbarAction::ALL.into_iter().zip([
            "zoom-in",
            "zoom-out",
            "zoom-fit",
            "zoom-origin",
        ]) {
            assert_eq!(
                CameraToolbarAction::from_workbench_action(key),
                Some(action)
            );
            let projectional_outcome = action.apply(&mut projectional_camera, Some(&scene));
            let flat_outcome = action.apply(&mut flat_camera, Some(&scene));
            assert_eq!(projectional_outcome, flat_outcome);
            assert!(!action.notice(projectional_outcome).is_empty());
            assert_eq!(
                projectional_camera.model_center().map(f64::to_bits),
                flat_camera.model_center().map(f64::to_bits),
            );
            assert_eq!(
                projectional_camera.pixels_per_model_unit().to_bits(),
                flat_camera.pixels_per_model_unit().to_bits(),
            );

            let projectional = projectional_queue
                .admit_toolbar_change(projectional_camera, projectional_outcome.changed);
            let flat = flat_queue.admit_toolbar_change(flat_camera, flat_outcome.changed);
            assert!(!projectional.saves_workspace());
            assert!(!flat.saves_workspace());
            assert_eq!(projectional.durable_render_scope(), None);
            assert_eq!(flat.durable_render_scope(), None);
            projectional_frame = projectional_frame.or(projectional.frame_generation);
            flat_frame = flat_frame.or(flat.frame_generation);
            projectional_idle = projectional.idle_generation;
            flat_idle = flat.idle_generation;
        }

        assert_eq!(projectional_frame, flat_frame);
        assert_eq!(projectional_idle, flat_idle);
        let projectional_transform = projectional_queue
            .take_frame(projectional_frame.expect("first mutation schedules one retained frame"))
            .expect("projectional retained camera frame");
        let flat_transform = flat_queue
            .take_frame(flat_frame.expect("first mutation schedules one retained frame"))
            .expect("flat retained camera frame");
        assert_eq!(projectional_transform, flat_transform);
        assert!(projectional_queue.take_idle_reconciliation(
            projectional_idle.expect("latest toolbar command authenticates exact idle paint")
        ));
        assert!(flat_queue.take_idle_reconciliation(
            flat_idle.expect("latest toolbar command authenticates exact idle paint")
        ));
    }

    #[test]
    fn terminal_pan_sample_owns_the_exact_final_camera_for_both_routes() {
        let initial =
            super::scene::CanvasCamera::new([8.0, -3.0], 40.0).expect("valid initial camera");
        let gesture = super::CanvasPanGesture {
            pointer_id: 17,
            origin: ScreenPoint { x: 100.0, y: 150.0 },
            origin_center: initial.model_center(),
        };
        let preceding_move = ScreenPoint { x: 160.0, y: 180.0 };
        let terminal = ScreenPoint { x: 220.0, y: 90.0 };
        let mut projectional = initial;
        let mut flat = initial;

        assert!(projectional.pan_from(gesture.origin_center, gesture.origin, preceding_move,));
        assert!(flat.pan_from(gesture.origin_center, gesture.origin, preceding_move,));
        assert!(super::finish_canvas_pan_camera(
            &mut projectional,
            gesture,
            Some(terminal),
        ));
        assert!(super::finish_canvas_pan_camera(
            &mut flat,
            gesture,
            Some(terminal),
        ));

        let expected = [5.0, -4.5];
        assert_eq!(
            projectional.model_center().map(f64::to_bits),
            expected.map(f64::to_bits)
        );
        assert_eq!(
            flat.model_center().map(f64::to_bits),
            expected.map(f64::to_bits)
        );
        assert_eq!(
            projectional, flat,
            "browser adapters share the terminal camera oracle"
        );

        let mut missing_terminal = initial;
        assert!(!super::finish_canvas_pan_camera(
            &mut missing_terminal,
            gesture,
            None,
        ));
        assert_eq!(missing_terminal, initial);
    }

    #[test]
    fn retained_hover_routes_share_admission_and_reject_semantic_effects() {
        use geosolve_constraint_editor::EditorEffect;

        let cases = [
            (
                vec![EditorEffect::HoverChanged(EditorHoverState::default())],
                true,
                "ordinary hover",
            ),
            (
                vec![
                    EditorEffect::HoverChanged(EditorHoverState::default()),
                    EditorEffect::FilletBranchPreviewChanged { target: None },
                ],
                true,
                "hover plus preview retirement",
            ),
            (
                vec![EditorEffect::SelectionChanged(Vec::new())],
                false,
                "selection",
            ),
            (
                vec![EditorEffect::ClearPointPreview],
                false,
                "semantic preview",
            ),
            (
                vec![EditorEffect::ClearConstructionPreview],
                false,
                "construction",
            ),
        ];
        for (effects, expected, label) in cases {
            let projectional = super::projectional_effects_are_retained_hover_only(&effects);
            let flat = super::flat_effects_are_retained_hover_only(&effects);
            assert_eq!(projectional, expected, "projectional {label}");
            assert_eq!(flat, expected, "flat {label}");
            assert_eq!(flat, projectional, "route parity for {label}");
        }
    }

    #[test]
    fn retained_hover_admission_authenticates_scene_view_and_display_policy() {
        let mut document = SketchDocument::new(8.0).expect("document");
        document
            .add_rectangle("hover admission", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            test_viewport(),
            0.8,
        )
        .expect("scene");
        let viewport = scene.viewport;
        let annotations_visible = scene.annotations_visible;
        let show_all = scene.show_all_constraint_annotations;
        let admitted = |candidate: Option<&EditorScene>,
                        candidate_viewport: Viewport,
                        candidate_annotations: bool,
                        candidate_show_all: bool,
                        current: bool| {
            super::retained_hover_scene_is_admitted(
                candidate,
                candidate_viewport,
                candidate_annotations,
                candidate_show_all,
                |_| current,
            )
        };

        assert!(admitted(
            Some(&scene),
            viewport,
            annotations_visible,
            show_all,
            true,
        ));
        assert!(!admitted(
            None,
            viewport,
            annotations_visible,
            show_all,
            true,
        ));
        let mismatched_viewport = Viewport::new(
            viewport.screen_size,
            [viewport.model_center[0] + 1.0, viewport.model_center[1]],
            viewport.pixels_per_model_unit,
        )
        .expect("mismatched viewport");
        assert!(!admitted(
            Some(&scene),
            mismatched_viewport,
            annotations_visible,
            show_all,
            true,
        ));
        assert!(!admitted(
            Some(&scene),
            viewport,
            !annotations_visible,
            show_all,
            true,
        ));
        assert!(!admitted(
            Some(&scene),
            viewport,
            annotations_visible,
            !show_all,
            true,
        ));
        assert!(!admitted(
            Some(&scene),
            viewport,
            annotations_visible,
            show_all,
            false,
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one compact native fixture freezes every retained geometry and annotation selector"
    )]
    fn retained_hover_native_selectors_cover_geometry_datums_and_annotations() {
        use super::ProjectionalHoverDomTarget;

        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("retained hover", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            test_viewport(),
            0.8,
        )
        .expect("scene");

        let point = rectangle.points[0];
        let point_index = scene
            .points
            .iter()
            .position(|candidate| candidate.id == point)
            .expect("painted point");
        let point_target = EditorHoverTarget::Geometry(SelectionItem::Point(point));
        let point_dom_targets = super::projectional_hover_dom_targets(&scene, point_target);
        assert_eq!(
            point_dom_targets,
            vec![ProjectionalHoverDomTarget::Geometry(format!(
                "wb-scene-point-{point_index}"
            ))]
        );
        assert_eq!(
            super::projectional_hover_dom_targets(&scene, point_target),
            point_dom_targets,
            "both runtime presenters share this exact retained SVG target mapping",
        );

        let span = CurveSpan::line(rectangle.curves[0]);
        let span_ids = scene
            .curves
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.span == span)
            .map(|(index, _)| {
                ProjectionalHoverDomTarget::Geometry(format!("wb-scene-curve-{index}"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            super::projectional_hover_dom_targets(
                &scene,
                EditorHoverTarget::Geometry(SelectionItem::Curve(span)),
            ),
            span_ids,
        );

        for datum in [
            geosolve_sketch::SketchDatum::XAxis,
            geosolve_sketch::SketchDatum::YAxis,
        ] {
            let index = scene
                .datums
                .iter()
                .position(|candidate| candidate.datum == datum)
                .expect("axis datum");
            assert_eq!(
                super::projectional_hover_dom_targets(
                    &scene,
                    EditorHoverTarget::Geometry(SelectionItem::Datum(datum)),
                ),
                vec![ProjectionalHoverDomTarget::Geometry(format!(
                    "wb-scene-datum-{index}"
                ))],
            );
        }
        assert!(
            super::projectional_hover_dom_targets(
                &scene,
                EditorHoverTarget::Geometry(SelectionItem::Datum(
                    geosolve_sketch::SketchDatum::Origin,
                )),
            )
            .is_empty(),
            "Origin deliberately has no duplicate painted hover target"
        );
        let origin = SelectionItem::Datum(geosolve_sketch::SketchDatum::Origin);
        assert!(super::projectional_item_intentionally_has_no_dom_owner(
            origin,
        ));
        assert!(super::projectional_hover_intentionally_has_no_dom_owner(
            EditorHoverTarget::Geometry(origin),
        ));

        let forged_point = SelectionItem::Point(DesignPointId(PersistentId::from_u128(
            0x8500_f0f0_u128 << 64,
        )));
        assert!(super::projectional_item_element_ids(&scene, forged_point).is_empty());
        assert!(
            super::projectional_hover_dom_targets(
                &scene,
                EditorHoverTarget::Geometry(forged_point),
            )
            .is_empty(),
        );
        assert!(!super::projectional_item_intentionally_has_no_dom_owner(
            forged_point,
        ));
        assert!(!super::projectional_hover_intentionally_has_no_dom_owner(
            EditorHoverTarget::Geometry(forged_point),
        ));
        assert!(!super::projectional_item_intentionally_has_no_dom_owner(
            SelectionItem::Datum(geosolve_sketch::SketchDatum::XAxis),
        ));

        for item in [
            SelectionItem::Constraint(rectangle.constraints[0]),
            SelectionItem::Dimension(rectangle.dimensions[0]),
        ] {
            let root = super::projectional_annotation_element_id(&scene, item)
                .expect("annotation root identity");
            assert_eq!(
                super::projectional_hover_dom_targets(
                    &scene,
                    EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                        item,
                        marker_index: None,
                    }),
                ),
                vec![ProjectionalHoverDomTarget::AnnotationRoot(root.clone())],
            );
            assert_eq!(
                super::projectional_hover_dom_targets(
                    &scene,
                    EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                        item,
                        marker_index: Some(1),
                    }),
                ),
                vec![ProjectionalHoverDomTarget::AnnotationMarker {
                    root,
                    selector: ".wb-constraint-symbol[data-annotation-marker=\"1\"]".into(),
                }],
                "a proximate marker must not also classify the annotation root as hovered",
            );
        }
    }

    #[test]
    fn retained_hover_selectors_cover_features_and_direct_curve_controls() {
        use super::ProjectionalHoverDomTarget;

        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let preview = coordinator
            .feature_authoring_preview()
            .expect("held grouped preview");
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted source");
        let mut scene = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &coordinator
                .session()
                .accepted_prepared_input()
                .expect("accepted prepared input"),
            &metadata.input,
            preview.snapshot(),
            test_viewport(),
            0.8,
        )
        .expect("computed scene");
        let owner = scene.computed_curves[0].owner;
        assert_eq!(
            super::projectional_hover_dom_targets(
                &scene,
                EditorHoverTarget::Geometry(SelectionItem::FeatureCorner(owner)),
            ),
            vec![ProjectionalHoverDomTarget::Geometry(
                "wb-scene-computed-curve-0".into(),
            )],
        );
        let feature_targets = scene
            .computed_curves
            .iter()
            .enumerate()
            .filter(|(_, curve)| curve.owner.feature == owner.feature)
            .map(|(index, _)| {
                ProjectionalHoverDomTarget::Geometry(format!("wb-scene-computed-curve-{index}"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            super::projectional_hover_dom_targets(
                &scene,
                EditorHoverTarget::Geometry(SelectionItem::Feature(owner.feature)),
            ),
            feature_targets,
        );

        let (mut curve_coordinator, curve) = m77_rational_coordinator(0.5);
        curve_coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(curve)]);
        scene = compose_editor_scene(&curve_coordinator, test_viewport(), 0.25)
            .expect("selected rational scene");
        let (control_index, control) = scene
            .curve_controls
            .iter()
            .enumerate()
            .find(|(_, control)| {
                matches!(
                    control.interaction,
                    geosolve_constraint_editor::SceneCurveControlInteraction::Direct
                )
            })
            .expect("direct rational control");
        let mut expected = vec![ProjectionalHoverDomTarget::CurveControl(format!(
            "wb-scene-control-{control_index}"
        ))];
        expected.extend(
            scene
                .curve_control_guides
                .iter()
                .enumerate()
                .filter(|(_, guide)| guide.control == Some(control.id))
                .map(|(index, _)| {
                    ProjectionalHoverDomTarget::CurveControlGuide(format!(
                        "wb-scene-control-guide-{index}"
                    ))
                }),
        );
        assert_eq!(
            super::projectional_hover_dom_targets(
                &scene,
                EditorHoverTarget::CurveControl {
                    control: control.id,
                    owner: control.owner,
                },
            ),
            expected,
        );
    }

    #[test]
    fn design_projection_tabs_are_closed_and_presentation_only() {
        use super::DesignProjectionTab::{Code, History, Outline, StructuredSource};

        assert_eq!(super::DesignProjectionTab::ALL.len(), 4);
        assert_eq!(
            super::DesignProjectionTab::from_key("outline"),
            Some(Outline)
        );
        assert_eq!(
            super::DesignProjectionTab::from_key("source"),
            Some(StructuredSource)
        );
        assert_eq!(
            super::DesignProjectionTab::from_key("history"),
            Some(History)
        );
        assert_eq!(super::DesignProjectionTab::from_key("code"), Some(Code));
        assert_eq!(super::DesignProjectionTab::from_key("solver"), None);
        assert_eq!(Outline.adjacent(-1), Code);
        assert_eq!(History.adjacent(1), Code);
        assert_eq!(Code.adjacent(1), Outline);
        assert_eq!(StructuredSource.adjacent(-1), Outline);
        assert_eq!(StructuredSource.adjacent(1), History);
        assert_eq!(Outline.button_id(), "wb-design-tab-outline");
        assert_eq!(StructuredSource.panel_id(), "wb-design-source");
        assert_eq!(History.panel_id(), "wb-design-history");
        assert_eq!(Code.panel_id(), "wb-design-code");
    }

    #[test]
    fn fresh_and_sample_flat_inputs_activate_history_free_projectional_authority() {
        run_projectional_test_with_large_stack("projectional-startup-and-sample-bootstrap", || {
            let document = SketchDocument::with_id(
                10.0,
                DocumentId(PersistentId::from_u128(0x8308_0001_u128 << 64)),
            )
            .unwrap();
            let native = RetainedSketchDocumentSession::new(
                document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap();
            let coordinator = RetainedEditorCoordinator::new(native).unwrap();
            let authority =
                WorkbenchDocumentAuthority::from_flat_coordinator(&coordinator).unwrap();
            let fresh = authority.projectional_ref().unwrap();
            assert!(authority.is_projectional());
            assert!(authority.flat_ref().is_none());
            assert!(fresh.coordinator().presentation_session().is_some());
            assert_eq!(fresh.coordinator().intent().undo_len(), 0);

            let mut samples = super::samples::SampleCatalogState::default();
            let sample = samples.open_key("constraint-dimension-sampler").unwrap();
            let expected_document = sample.session().design_document().clone();
            let authority = WorkbenchDocumentAuthority::from_flat_coordinator(&sample).unwrap();
            let projectional = authority.projectional_ref().unwrap();
            assert_eq!(
                projectional
                    .coordinator()
                    .presentation_session()
                    .unwrap()
                    .design_document(),
                &expected_document,
            );
            assert_eq!(projectional.coordinator().intent().undo_len(), 0);
            assert!(
                projectional
                    .coordinator()
                    .intent()
                    .graph()
                    .nodes()
                    .values()
                    .all(|node| matches!(node.kind, IntentNodeKind::Bootstrap { .. }))
            );
        });
    }

    #[test]
    fn projectional_new_authority_has_empty_scene_and_round_trips_as_workspace_v8() {
        run_projectional_test_with_large_stack("projectional-new-authority", || {
            let authority = super::fresh_projectional_authority().unwrap();
            assert!(authority.is_projectional());
            assert!(authority.flat_ref().is_none());

            let projectional = authority.projectional_ref().unwrap();
            let session = projectional.coordinator().presentation_session().unwrap();
            let design = session.design_document();
            let accepted = session
                .accepted_state_for_current_input()
                .unwrap()
                .document();
            for document in [design, accepted] {
                assert!(document.points().is_empty());
                assert!(document.curves().is_empty());
                assert!(document.constraints().is_empty());
                assert!(document.dimensions().is_empty());
            }
            assert!(
                projectional
                    .coordinator()
                    .intent()
                    .graph()
                    .nodes()
                    .values()
                    .all(|node| matches!(node.kind, IntentNodeKind::Bootstrap { .. })),
                "an empty native sketch may contain only its history-free bootstrap header",
            );
            assert_eq!(projectional.coordinator().intent().undo_len(), 0);
            assert_eq!(projectional.coordinator().intent().redo_len(), 0);

            let encoded = authority.snapshot().unwrap().encode().unwrap();
            assert!(encoded.contains("\"version\":8"));
            let decoded = WorkspaceSnapshot::decode(&encoded).unwrap();
            let restored = WorkbenchDocumentAuthority::from_snapshot(&decoded).unwrap();
            assert!(restored.is_projectional());
            assert_eq!(restored.snapshot().unwrap().encode().unwrap(), encoded);
        });
    }

    #[test]
    fn projectional_outline_moves_are_organization_only_and_stale_safe() {
        run_projectional_test_with_large_stack("projectional-outline-organization", || {
            let mut editor = projectional_authoring_fixture();
            let projection = editor.workbench_projection();
            let original = projection.outline[0]
                .declarations
                .iter()
                .map(|declaration| declaration.node)
                .collect::<Vec<_>>();
            assert_eq!(original.len(), 2);
            let accepted_before = editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .evidence
                .clone();

            let moved = projectional_outline_move_patch(
                &projection,
                original[1],
                super::ProjectionalOutlineMove::Up,
            )
            .unwrap();
            let outcome = editor.apply_patch(moved).unwrap();
            assert_eq!(outcome.disposition, IntentPlanDisposition::OrganizationOnly);
            assert_eq!(
                editor.workbench_projection().outline[0]
                    .declarations
                    .iter()
                    .map(|declaration| declaration.node)
                    .collect::<Vec<_>>(),
                vec![original[1], original[0]],
            );
            assert_eq!(
                editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
                    .evidence,
                accepted_before,
            );

            let moved = projectional_outline_move_patch(
                &editor.workbench_projection(),
                original[1],
                super::ProjectionalOutlineMove::Down,
            )
            .unwrap();
            assert_eq!(
                editor.apply_patch(moved).unwrap().disposition,
                IntentPlanDisposition::OrganizationOnly,
            );
            assert_eq!(
                editor.workbench_projection().outline[0]
                    .declarations
                    .iter()
                    .map(|declaration| declaration.node)
                    .collect::<Vec<_>>(),
                original,
            );

            let projection = editor.workbench_projection();
            let dropped = projectional_outline_move_patch(
                &projection,
                original[1],
                super::ProjectionalOutlineMove::Drop {
                    cell: projection.outline[0].cell,
                    before: Some(original[0]),
                },
            )
            .unwrap();
            assert_eq!(
                editor.apply_patch(dropped).unwrap().disposition,
                IntentPlanDisposition::OrganizationOnly,
            );
            assert_eq!(
                editor.workbench_projection().outline[0]
                    .declarations
                    .iter()
                    .map(|declaration| declaration.node)
                    .collect::<Vec<_>>(),
                vec![original[1], original[0]],
            );

            let stale_patch = projectional_outline_move_patch(
                &projection,
                original[1],
                super::ProjectionalOutlineMove::Up,
            )
            .unwrap();
            let history_before = editor.coordinator().intent().undo_len();
            assert!(editor.apply_patch(stale_patch).is_err());
            assert_eq!(editor.coordinator().intent().undo_len(), history_before);
            assert_eq!(
                editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
                    .evidence,
                accepted_before,
            );
        });
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one drop-slot regression proves adjacent, end, authority and Undo behavior together"
    )]
    fn projectional_outline_drop_halves_resolve_adjacent_insertion_slots() {
        run_projectional_test_with_large_stack("projectional-outline-drop-slots", || {
            let mut editor = projectional_authoring_fixture();
            let third = editor
                .apply_patch(IntentPatch::new(
                    editor.coordinator().intent().identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::CreateNode {
                        alias: IntentKey::new("third").unwrap(),
                        draft: Box::new(
                            geosolve_sketch_intent::IntentNodeDraft::new(
                                IntentNodeKind::Geometry {
                                    recipe: geosolve_sketch_intent::GeometryRecipeKind::SketchPoint,
                                },
                                IntentKey::new("third-point").unwrap(),
                            )
                            .with_instance_leaf(
                                geosolve_sketch_intent::IntentPortSelector::Node {
                                    role: geosolve_sketch_intent::IntentPortRole::Primary,
                                    index: 0,
                                },
                                geosolve_sketch_intent::LeafField::X,
                                geosolve_sketch_intent::IntentLiteral::Quantity {
                                    value: 8.0,
                                    unit: geosolve_sketch_intent::IntentUnit::Length,
                                },
                            )
                            .with_instance_leaf(
                                geosolve_sketch_intent::IntentPortSelector::Node {
                                    role: geosolve_sketch_intent::IntentPortRole::Primary,
                                    index: 0,
                                },
                                geosolve_sketch_intent::LeafField::Y,
                                geosolve_sketch_intent::IntentLiteral::Quantity {
                                    value: -3.0,
                                    unit: geosolve_sketch_intent::IntentUnit::Length,
                                },
                            ),
                        ),
                        cell: None,
                    }],
                ))
                .unwrap()
                .aliases
                .node(&IntentKey::new("third").unwrap())
                .unwrap();
            let projection = editor.workbench_projection();
            let cell = projection.outline[0].cell;
            let nodes = projection.outline[0]
                .declarations
                .iter()
                .map(|declaration| declaration.node)
                .collect::<Vec<_>>();
            assert_eq!(nodes.len(), 3);
            assert_eq!(nodes[2], third);

            assert_eq!(
                projectional_outline_drop_before(&projection, nodes[0], cell, Some(nodes[1]), true)
                    .unwrap(),
                Some(nodes[2]),
                "the lower half of the immediately following row must move after that row",
            );
            assert_eq!(
                projectional_outline_drop_before(
                    &projection,
                    nodes[2],
                    cell,
                    Some(nodes[0]),
                    false,
                )
                .unwrap(),
                Some(nodes[0]),
            );
            assert_eq!(
                projectional_outline_drop_before(&projection, nodes[1], cell, Some(nodes[2]), true)
                    .unwrap(),
                None,
                "the lower half of the final row must resolve to the end slot",
            );
            assert!(
                projectional_outline_drop_before(
                    &projection,
                    nodes[0],
                    cell,
                    Some(nodes[0]),
                    false,
                )
                .is_err()
            );

            let evidence_before = editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .evidence
                .clone();
            let before =
                projectional_outline_drop_before(&projection, nodes[0], cell, Some(nodes[1]), true)
                    .unwrap();
            let patch = projectional_outline_move_patch(
                &projection,
                nodes[0],
                super::ProjectionalOutlineMove::Drop { cell, before },
            )
            .unwrap();
            assert_eq!(
                editor.apply_patch(patch).unwrap().disposition,
                IntentPlanDisposition::OrganizationOnly,
            );
            assert_eq!(
                editor.workbench_projection().outline[0]
                    .declarations
                    .iter()
                    .map(|declaration| declaration.node)
                    .collect::<Vec<_>>(),
                vec![nodes[1], nodes[0], nodes[2]],
                "dropping onto the lower half of an adjacent row must visibly move after it",
            );
            assert_eq!(
                editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
                    .evidence,
                evidence_before,
                "Outline drag/drop remains organization-only",
            );
            editor.undo().unwrap().unwrap();
            assert_eq!(
                editor.workbench_projection().outline[0]
                    .declarations
                    .iter()
                    .map(|declaration| declaration.node)
                    .collect::<Vec<_>>(),
                nodes,
            );
        });
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one cell gesture contract keeps history and accepted authority checks together"
    )]
    fn projectional_cell_moves_are_one_organization_only_history_entry_and_stale_safe() {
        run_projectional_test_with_large_stack("projectional-cell-organization", || {
            let mut editor = projectional_authoring_fixture();
            let first = editor
                .apply_patch(IntentPatch::new(
                    editor.coordinator().intent().identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::CreateCell {
                        alias: IntentKey::new("first-cell").unwrap(),
                        name: IntentKey::new("First cell").unwrap(),
                        before: None,
                    }],
                ))
                .unwrap();
            assert_eq!(first.disposition, IntentPlanDisposition::OrganizationOnly);
            let first_cell = first
                .aliases
                .cell(&IntentKey::new("first-cell").unwrap())
                .unwrap();
            let second = editor
                .apply_patch(IntentPatch::new(
                    editor.coordinator().intent().identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::CreateCell {
                        alias: IntentKey::new("second-cell").unwrap(),
                        name: IntentKey::new("Second cell").unwrap(),
                        before: None,
                    }],
                ))
                .unwrap();
            assert_eq!(second.disposition, IntentPlanDisposition::OrganizationOnly);
            let second_cell = second
                .aliases
                .cell(&IntentKey::new("second-cell").unwrap())
                .unwrap();

            let projection = editor.workbench_projection();
            let original = projection
                .outline
                .iter()
                .map(|cell| cell.cell)
                .collect::<Vec<_>>();
            assert_eq!(original.len(), 3);
            assert_eq!(original[1..], [first_cell, second_cell]);
            assert_eq!(
                projectional_cell_drop_before(&projection, original[0], Some(first_cell), true,)
                    .unwrap(),
                Some(second_cell),
                "the lower half of the next cell header must resolve after that cell",
            );
            assert_eq!(
                projectional_cell_drop_before(&projection, first_cell, Some(second_cell), true,)
                    .unwrap(),
                None,
            );
            assert!(
                projectional_cell_drop_before(&projection, first_cell, Some(first_cell), false,)
                    .is_err()
            );
            let markup = super::design_projection::outline_markup(&projection, None);
            assert!(markup.contains("data-intent-cell-drag="));
            assert!(markup.contains("data-intent-cell-drop-before="));
            assert!(markup.contains("data-intent-cell-drop-end=\"true\""));
            assert!(markup.contains("aria-label=\"Move cell up\""));
            assert!(markup.contains("aria-label=\"Move cell down\""));
            assert!(markup.contains(&format!(
                "data-intent-cell-move=\"up\" data-intent-cell=\"{}\" disabled",
                original[0],
            )));
            assert!(markup.contains(&format!(
                "data-intent-cell-move=\"down\" data-intent-cell=\"{}\" disabled",
                original[2],
            )));

            let accepted = editor.coordinator().accepted_materialization().unwrap();
            let evidence_before = accepted.evidence.clone();
            let ownership_before = accepted.ownership.clone();
            let document_before = accepted.session.design_document().clone();
            let history_before = editor.coordinator().intent().undo_len();

            let stale_drop = projectional_cell_move_patch(
                &projection,
                second_cell,
                ProjectionalCellMove::Drop {
                    before: Some(original[0]),
                },
            )
            .unwrap();
            assert!(matches!(
                stale_drop.operations(),
                [IntentPatchOperation::ReorderCells { exact_order }]
                    if exact_order == &vec![second_cell, original[0], first_cell]
            ));
            let down =
                projectional_cell_move_patch(&projection, first_cell, ProjectionalCellMove::Down)
                    .unwrap();
            assert!(matches!(
                down.operations(),
                [IntentPatchOperation::ReorderCells { exact_order }]
                    if exact_order == &vec![original[0], second_cell, first_cell]
            ));
            let moved =
                projectional_cell_move_patch(&projection, second_cell, ProjectionalCellMove::Up)
                    .unwrap();
            assert!(matches!(
                moved.operations(),
                [IntentPatchOperation::ReorderCells { exact_order }]
                    if exact_order == &vec![original[0], second_cell, first_cell]
            ));
            let outcome = editor.apply_patch(moved).unwrap();
            assert_eq!(outcome.disposition, IntentPlanDisposition::OrganizationOnly);
            assert_eq!(
                editor
                    .workbench_projection()
                    .outline
                    .iter()
                    .map(|cell| cell.cell)
                    .collect::<Vec<_>>(),
                vec![original[0], second_cell, first_cell],
            );
            assert_eq!(editor.coordinator().intent().undo_len(), history_before + 1,);
            let accepted = editor.coordinator().accepted_materialization().unwrap();
            assert_eq!(accepted.evidence, evidence_before);
            assert_eq!(accepted.ownership, ownership_before);
            assert_eq!(accepted.session.design_document(), &document_before);

            let history_after_move = editor.coordinator().intent().undo_len();
            assert!(editor.apply_patch(stale_drop).is_err());
            assert_eq!(editor.coordinator().intent().undo_len(), history_after_move,);
            let accepted = editor.coordinator().accepted_materialization().unwrap();
            assert_eq!(accepted.evidence, evidence_before);
            assert_eq!(accepted.ownership, ownership_before);
            assert_eq!(accepted.session.design_document(), &document_before);

            assert!(editor.undo().unwrap().is_some());
            assert_eq!(
                editor
                    .workbench_projection()
                    .outline
                    .iter()
                    .map(|cell| cell.cell)
                    .collect::<Vec<_>>(),
                original,
            );
            let accepted = editor.coordinator().accepted_materialization().unwrap();
            assert_eq!(accepted.evidence, evidence_before);
            assert_eq!(accepted.ownership, ownership_before);
            assert_eq!(accepted.session.design_document(), &document_before);
        });
    }

    #[test]
    fn projectional_browser_captures_every_supported_direct_manipulation_route() {
        assert!(projectional_direct_gesture_is_capturable(
            ActivePointerGestureKind::Point,
        ));
        assert!(projectional_direct_gesture_is_capturable(
            ActivePointerGestureKind::CurveControl,
        ));
        assert!(projectional_direct_gesture_is_capturable(
            ActivePointerGestureKind::FilletRadius,
        ));
        assert!(projectional_direct_gesture_is_capturable(
            ActivePointerGestureKind::OffsetDistance,
        ));
        assert!(projectional_direct_gesture_is_capturable(
            ActivePointerGestureKind::Annotation,
        ));
        assert!(!projectional_direct_gesture_is_capturable(
            ActivePointerGestureKind::FilletContact,
        ));
    }

    #[test]
    fn projectional_pointer_terminal_is_exact_once_after_capture_release() {
        let mut released = Some(83);
        assert_eq!(
            route_projectional_terminal_capture(&mut released, Some(83)),
            Some(83),
            "the owning pointer-up must retire capture exactly once",
        );
        assert_eq!(released, None);
        assert_eq!(
            route_projectional_terminal_capture(&mut released, Some(83)),
            None,
            "the resulting lostpointercapture must be a stale no-op",
        );
        let mut lost_first = Some(83);
        assert_eq!(
            route_projectional_terminal_capture(&mut lost_first, Some(83)),
            Some(83),
            "lostpointercapture may cancel the live owner once",
        );
        assert_eq!(
            route_projectional_terminal_capture(&mut lost_first, Some(83)),
            None,
            "a repeated loss cannot cancel twice",
        );

        let mut foreign = Some(83);
        assert_eq!(
            route_projectional_terminal_capture(&mut foreign, Some(84)),
            None,
            "a foreign terminal cannot retire the owning gesture",
        );
        assert_eq!(foreign, Some(83));
        assert!(projectional_terminal_owns_capture(Some(83), None));
    }

    #[test]
    fn projectional_canvas_display_owns_constraint_mark_visibility() {
        run_projectional_test_with_large_stack("projectional-constraint-display", || {
            let editor = projectional_authoring_fixture();
            let mut scene = editor
                .scene(
                    Viewport::new([800.0, 600.0], [0.0, 0.0], 50.0).unwrap(),
                    0.5,
                )
                .unwrap();
            assert!(!scene.show_all_constraint_annotations);

            apply_projectional_scene_display(&mut scene, true, true);
            assert!(scene.annotations_visible);
            assert!(scene.show_all_constraint_annotations);
            apply_projectional_scene_display(&mut scene, false, false);
            assert!(!scene.annotations_visible);
            assert!(!scene.show_all_constraint_annotations);
        });
    }

    #[test]
    fn projectional_scene_presentation_retains_computed_preview_work() {
        run_projectional_test_with_large_stack("projectional-scene-work", || {
            let mut editor = projectional_authoring_fixture();
            let viewport = test_viewport();
            let scene = editor.scene(viewport, 0.5).unwrap();
            let point = scene
                .points
                .iter()
                .find(|point| {
                    point.model_position.map(f64::to_bits) == [1.0, 2.0].map(f64::to_bits)
                })
                .expect("authored point")
                .id;
            let pointer = |model_position| PointerInput {
                pointer_id: 85_001,
                position: viewport.model_to_screen(model_position),
                modifiers: Modifiers::default(),
            };
            editor
                .pointer_down_exact_point(&scene, pointer([1.0, 2.0]), point)
                .unwrap();
            editor.pointer_move(&scene, pointer([1.25, 2.5])).unwrap();

            let presentation = super::ProjectionalScenePresentation::from_editor(
                &editor, viewport, 0.5, true, false,
            );
            assert!(presentation.scene.is_some());
            assert_eq!(presentation.work.native_preview_attempts(), 0);
            assert_eq!(presentation.work.intent_materialization_attempts(), 0);
            assert_eq!(presentation.work.computed_evaluation_attempts(), 1);
            assert_eq!(presentation.work.history_publications(), 0);
        });
    }

    fn test_viewport() -> Viewport {
        Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).unwrap()
    }

    fn projectional_workbench_fixture()
    -> (WorkbenchDocumentAuthority, geosolve_sketch_intent::NodeId) {
        let document = DocumentId(PersistentId::from_u128(0x8308_1001_u128 << 64));
        let mut coordinator = ProjectionalIntentCoordinator::empty(
            IntentSessionId::from_raw(0x8308_1001),
            ColdIntentMaterializer::with_default_policy(document, 1.0).unwrap(),
        )
        .unwrap();
        let primary = IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        };
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            IntentKey::new("point.main").unwrap(),
        )
        .with_instance_leaf(
            primary,
            LeafField::X,
            IntentLiteral::Quantity {
                value: 2.0,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            primary,
            LeafField::Y,
            IntentLiteral::Quantity {
                value: 3.0,
                unit: IntentUnit::Length,
            },
        );
        coordinator
            .apply_patch(IntentPatch::new(
                coordinator.intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateNode {
                    alias: IntentKey::new("point").unwrap(),
                    draft: Box::new(draft),
                    cell: None,
                }],
            ))
            .unwrap();
        let node = *coordinator.intent().graph().nodes().keys().next().unwrap();
        let projectional = ProjectionalEditorSession::new(coordinator);
        let snapshot = WorkspaceSnapshot::from_projectional_editor(&projectional).unwrap();
        let encoded = snapshot.encode().unwrap();
        let decoded = WorkspaceSnapshot::decode(&encoded).unwrap();
        (
            WorkbenchDocumentAuthority::from_snapshot(&decoded).unwrap(),
            node,
        )
    }

    fn projectional_authoring_fixture() -> ProjectionalEditorSession {
        let document = DocumentId(PersistentId::from_u128(0x8308_2001_u128 << 64));
        let mut coordinator = ProjectionalIntentCoordinator::empty(
            IntentSessionId::from_raw(0x8308_2001),
            ColdIntentMaterializer::with_default_policy(document, 1.0).unwrap(),
        )
        .unwrap();
        let selector = |role| IntentPortSelector::Node { role, index: 0 };
        let length = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let point = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            IntentKey::new("contact point").unwrap(),
        )
        .with_instance_leaf(selector(IntentPortRole::Primary), LeafField::X, length(1.0))
        .with_instance_leaf(selector(IntentPortRole::Primary), LeafField::Y, length(2.0));
        let segment = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            IntentKey::new("edge").unwrap(),
        )
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::X, length(0.0))
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::Y, length(0.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(4.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(0.0));
        let outcome = coordinator
            .apply_patch(IntentPatch::new(
                coordinator.intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![
                    IntentPatchOperation::CreateNode {
                        alias: IntentKey::new("point").unwrap(),
                        draft: Box::new(point),
                        cell: None,
                    },
                    IntentPatchOperation::CreateNode {
                        alias: IntentKey::new("edge").unwrap(),
                        draft: Box::new(segment),
                        cell: None,
                    },
                ],
            ))
            .unwrap();
        assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
        ProjectionalEditorSession::new(coordinator)
    }

    fn run_projectional_test_with_large_stack(name: &str, test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .name(name.to_owned())
            .stack_size(32 * 1024 * 1024)
            .spawn(test)
            .expect("spawn projectional browser test")
            .join()
            .expect("projectional browser test thread");
    }

    fn projectional_inspector_stamp(identity: IntentSessionIdentity) -> ProjectionalInspectorStamp {
        ProjectionalInspectorStamp {
            session: Some(identity.session.to_string()),
            revision: Some(identity.revision.to_string()),
            digest: Some(identity.digest.to_string()),
        }
    }

    fn instance_inspector_control(
        editor: &ProjectionalEditorSession,
        field: LeafField,
        current: Option<f64>,
        value: &str,
    ) -> ProjectionalInspectorControl {
        let projection = editor.workbench_projection();
        let inspector = editor.selected_inspector(&projection).unwrap();
        let leaf = inspector
            .fields
            .iter()
            .find_map(|candidate| match candidate {
                IntentInspectorField::Instance { leaf, value }
                    if leaf.field == field
                        && current.is_none_or(|expected| {
                            matches!(
                                value,
                                Some(IntentLiteral::Quantity { value, .. })
                                    if value.to_bits() == expected.to_bits()
                            )
                        }) =>
                {
                    Some(*leaf)
                }
                _ => None,
            })
            .unwrap();
        let port_kind = inspector.output_descriptor(leaf).unwrap().kind;
        let current = inspector
            .fields
            .iter()
            .find_map(|candidate| match candidate {
                IntentInspectorField::Instance {
                    leaf: candidate,
                    value,
                    ..
                } if *candidate == leaf => value.as_ref(),
                _ => None,
            });
        let schema = super::design_projection::literal_schema_for_instance(field, current);
        let unit = match schema {
            IntentLiteralSchema::Quantity(unit) => {
                Some(super::design_projection::intent_unit_key(unit).to_owned())
            }
            _ => None,
        };
        ProjectionalInspectorControl {
            stamp: projectional_inspector_stamp(projection.identity),
            edit: Some("instance".into()),
            node: Some(leaf.node.to_string()),
            field: None,
            port: Some(leaf.port.to_string()),
            leaf: Some(
                match leaf.field {
                    LeafField::X => "x",
                    LeafField::Y => "y",
                    LeafField::Value => "value",
                    LeafField::Angle => "angle",
                    LeafField::Weight => "weight",
                    LeafField::Parameter => "parameter",
                }
                .into(),
            ),
            port_kind: Some(format!("{port_kind:?}")),
            schema: Some(super::design_projection::literal_schema_key(schema)),
            unit,
            component: None,
            submission: ProjectionalInspectorSubmission::Text(value.into()),
        }
    }

    fn current_definition_inspector_control(
        editor: &ProjectionalEditorSession,
        field: &str,
        submission: ProjectionalInspectorSubmission,
    ) -> ProjectionalInspectorControl {
        let projection = editor.workbench_projection();
        let inspector = editor.selected_inspector(&projection).unwrap();
        let schema = inspector
            .fields
            .iter()
            .find_map(|candidate| match candidate {
                IntentInspectorField::Definition {
                    definition: candidate,
                    ..
                } if candidate.0.as_str() == field => Some(
                    inspector
                        .definition_descriptor(candidate)
                        .unwrap()
                        .schema
                        .literal,
                ),
                _ => None,
            })
            .unwrap();
        let mut control = definition_inspector_control(inspector.node, field, schema, submission);
        control.stamp = projectional_inspector_stamp(projection.identity);
        control
    }

    fn suppression_inspector_control(
        editor: &ProjectionalEditorSession,
        suppressed: bool,
    ) -> ProjectionalInspectorControl {
        let projection = editor.workbench_projection();
        let inspector = editor.selected_inspector(&projection).unwrap();
        ProjectionalInspectorControl {
            stamp: projectional_inspector_stamp(projection.identity),
            edit: Some("suppressed".into()),
            node: Some(inspector.node.to_string()),
            field: None,
            port: None,
            leaf: None,
            port_kind: None,
            schema: Some("boolean".into()),
            unit: None,
            component: None,
            submission: ProjectionalInspectorSubmission::Checked(suppressed),
        }
    }

    fn definition_inspector_control(
        node: NodeId,
        field: &str,
        schema: IntentLiteralSchema,
        submission: ProjectionalInspectorSubmission,
    ) -> ProjectionalInspectorControl {
        ProjectionalInspectorControl {
            stamp: ProjectionalInspectorStamp::default(),
            edit: Some("definition".into()),
            node: Some(node.to_string()),
            field: Some(field.into()),
            port: None,
            leaf: None,
            port_kind: None,
            schema: Some(super::design_projection::literal_schema_key(schema)),
            unit: match schema {
                IntentLiteralSchema::Quantity(unit) => {
                    Some(super::design_projection::intent_unit_key(unit).to_owned())
                }
                _ => None,
            },
            component: None,
            submission,
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one table-driven browser regression reviews every closed Inspector literal family together"
    )]
    fn projectional_browser_inspector_decodes_every_schema_control_family() {
        let node = NodeId::from_raw(0x8308_1101);
        let schemas = [
            ("enabled", IntentLiteralSchema::Boolean),
            (
                "distance",
                IntentLiteralSchema::Quantity(IntentUnit::Length),
            ),
            ("winding", IntentLiteralSchema::Integer),
            ("degree", IntentLiteralSchema::Natural),
            ("branch", IntentLiteralSchema::Enum),
            ("digest", IntentLiteralSchema::Text),
            ("origin", IntentLiteralSchema::Point),
        ];
        let definition_schemas = schemas
            .iter()
            .map(|(field, literal)| IntentDefinitionFieldSchema {
                field: IntentFieldKey(IntentKey::new(*field).unwrap()),
                literal: *literal,
                required: false,
            })
            .collect::<Vec<_>>();
        let inspector = IntentInspectorProjection {
            identity: IntentSession::with_id(IntentSessionId::from_raw(0x8308_1100))
                .unwrap()
                .identity(),
            node,
            symbol: IntentKey::new("fixture").unwrap(),
            name: IntentKey::new("Fixture").unwrap(),
            kind: IntentGraphNodeKind::Annotation,
            suppressed: false,
            retained_failure: false,
            inputs: Vec::new(),
            descriptor: IntentDeclarationDescriptor {
                schema: IntentNodeSchema {
                    inputs: Vec::new(),
                    input_choices: Vec::new(),
                    fields: definition_schemas.clone(),
                    minimum_children: 0,
                    maximum_children: 0,
                },
                inputs: Vec::new(),
                fields: definition_schemas
                    .iter()
                    .cloned()
                    .map(|schema| IntentDefinitionFieldDescriptor {
                        path: IntentProjectionPath::field(schema.field.0.clone()),
                        schema,
                        default: IntentFieldDefault::Contextual,
                        choices: IntentFieldChoices::NotApplicable,
                        edit: IntentEditClassification::Definition,
                    })
                    .collect(),
                outputs: Vec::new(),
                suppression_edit: IntentEditClassification::Definition,
                input_edit: IntentEditClassification::InputBinding,
                name_edit: IntentEditClassification::Organization,
            },
            fields: definition_schemas
                .iter()
                .map(|schema| IntentInspectorField::Definition {
                    definition: schema.field.clone(),
                    value: None,
                })
                .collect(),
        };
        let cases = [
            (
                "enabled",
                IntentLiteralSchema::Boolean,
                ProjectionalInspectorSubmission::Checked(true),
                IntentLiteral::Boolean(true),
            ),
            (
                "distance",
                IntentLiteralSchema::Quantity(IntentUnit::Length),
                ProjectionalInspectorSubmission::Text("12.5".into()),
                IntentLiteral::Quantity {
                    value: 12.5,
                    unit: IntentUnit::Length,
                },
            ),
            (
                "winding",
                IntentLiteralSchema::Integer,
                ProjectionalInspectorSubmission::Text("-3".into()),
                IntentLiteral::Integer(-3),
            ),
            (
                "degree",
                IntentLiteralSchema::Natural,
                ProjectionalInspectorSubmission::Text("4".into()),
                IntentLiteral::Natural(4),
            ),
            (
                "branch",
                IntentLiteralSchema::Enum,
                ProjectionalInspectorSubmission::Text("clockwise".into()),
                IntentLiteral::Enum(IntentKey::new("clockwise").unwrap()),
            ),
            (
                "digest",
                IntentLiteralSchema::Text,
                ProjectionalInspectorSubmission::Text("topology-8308".into()),
                IntentLiteral::Text(IntentKey::new("topology-8308").unwrap()),
            ),
            (
                "origin",
                IntentLiteralSchema::Point,
                ProjectionalInspectorSubmission::Point {
                    x: Some("1.25".into()),
                    y: Some("-2.5".into()),
                },
                IntentLiteral::Point([1.25, -2.5]),
            ),
        ];
        for (field, schema, submission, expected) in cases {
            let mut control = definition_inspector_control(node, field, schema, submission);
            if schema == IntentLiteralSchema::Point {
                control.component = Some("x".into());
            }
            let decoded = decode_projectional_inspector_control(&inspector, &control)
                .unwrap()
                .unwrap();
            assert_eq!(
                decoded.0,
                geosolve_constraint_editor::IntentInspectorEditTarget::Definition {
                    field: IntentFieldKey(IntentKey::new(field).unwrap()),
                }
            );
            assert_eq!(
                decoded.1,
                geosolve_constraint_editor::IntentInspectorEditValue::Literal { literal: expected }
            );
        }

        let suppression = ProjectionalInspectorControl {
            stamp: ProjectionalInspectorStamp::default(),
            edit: Some("suppressed".into()),
            node: Some(node.to_string()),
            field: None,
            port: None,
            leaf: None,
            port_kind: None,
            schema: Some("boolean".into()),
            unit: None,
            component: None,
            submission: ProjectionalInspectorSubmission::Checked(true),
        };
        assert!(
            decode_projectional_inspector_control(&inspector, &suppression)
                .unwrap()
                .is_some()
        );
    }

    struct ProjectionalInspectorFixture(WorkbenchDocumentAuthority);

    impl ProjectionalInspectorFixture {
        fn from_editor(editor: ProjectionalEditorSession) -> Self {
            Self(
                WorkbenchDocumentAuthority::from_projectional_editor(editor)
                    .expect("projectional Inspector fixture authority"),
            )
        }
    }

    impl std::ops::Deref for ProjectionalInspectorFixture {
        type Target = ProjectionalEditorSession;

        fn deref(&self) -> &Self::Target {
            self.0.projectional_ref().unwrap()
        }
    }

    impl std::ops::DerefMut for ProjectionalInspectorFixture {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.0.projectional_mut().unwrap()
        }
    }

    fn projectional_circle_inspector_fixture() -> ProjectionalInspectorFixture {
        let document = DocumentId(PersistentId::from_u128(0x8308_1201_u128 << 64));
        let mut coordinator = ProjectionalIntentCoordinator::empty(
            IntentSessionId::from_raw(0x8308_1201),
            ColdIntentMaterializer::with_default_policy(document, 1.0).unwrap(),
        )
        .unwrap();
        let selector = |role| IntentPortSelector::Node { role, index: 0 };
        let length = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::CenterRadiusCircle,
            },
            IntentKey::new("circle.main").unwrap(),
        )
        .with_instance_leaf(selector(IntentPortRole::Center), LeafField::X, length(0.0))
        .with_instance_leaf(selector(IntentPortRole::Center), LeafField::Y, length(0.0))
        .with_instance_leaf(
            selector(IntentPortRole::Target),
            LeafField::Value,
            length(2.0),
        );
        let outcome = coordinator
            .apply_patch(IntentPatch::new(
                coordinator.intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateNode {
                    alias: IntentKey::new("circle").unwrap(),
                    draft: Box::new(draft),
                    cell: None,
                }],
            ))
            .unwrap();
        assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
        let node = *coordinator.intent().graph().nodes().keys().next().unwrap();
        let mut editor = ProjectionalEditorSession::new(coordinator);
        assert!(editor.set_selected_declaration(Some(node)));
        ProjectionalInspectorFixture::from_editor(editor)
    }

    fn accepted_circle_radius(editor: &ProjectionalEditorSession) -> f64 {
        let document = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document();
        let CurveDefinition::Circle { radius, .. } = document.curves()[0].definition else {
            panic!("Inspector fixture must materialize a circle");
        };
        document.scalar(radius).unwrap().value
    }

    #[test]
    fn projectional_browser_inspector_commits_accepted_edits_once_and_undo_redo_restores_them() {
        run_projectional_test_with_large_stack("projectional-browser-inspector-accepted", || {
            let mut authority = projectional_circle_inspector_fixture();
            let history_before = authority
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();

            let unchanged =
                instance_inspector_control(&authority, LeafField::Value, Some(2.0), "2.0");
            assert_eq!(
                dispatch_projectional_inspector_control(&mut authority.0, &unchanged),
                ProjectionalInspectorDispatch::Unchanged
            );
            assert_eq!(
                authority
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before
            );

            let edit = instance_inspector_control(&authority, LeafField::Value, Some(2.0), "3.5");
            let dispatch = dispatch_projectional_inspector_control(&mut authority.0, &edit);
            assert_eq!(
                dispatch,
                ProjectionalInspectorDispatch::Committed(IntentPlanDisposition::Accepted)
            );
            assert!(dispatch.saves_workspace());
            assert_eq!(
                accepted_circle_radius(&authority).to_bits(),
                3.5_f64.to_bits()
            );
            assert_eq!(
                authority
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before + 1
            );

            assert!(authority.undo().unwrap().is_some());
            assert_eq!(
                accepted_circle_radius(&authority).to_bits(),
                2.0_f64.to_bits()
            );
            assert!(authority.redo().unwrap().is_some());
            assert_eq!(
                accepted_circle_radius(&authority).to_bits(),
                3.5_f64.to_bits()
            );

            let suppression = suppression_inspector_control(&authority, true);
            assert_eq!(
                dispatch_projectional_inspector_control(&mut authority.0, &suppression),
                ProjectionalInspectorDispatch::Committed(IntentPlanDisposition::Accepted)
            );
            assert!(
                authority
                    .coordinator()
                    .intent()
                    .graph()
                    .node(authority.selected_declaration().unwrap())
                    .unwrap()
                    .suppressed
            );
            assert!(authority.undo().unwrap().is_some());
            assert!(
                !authority
                    .coordinator()
                    .intent()
                    .graph()
                    .node(authority.selected_declaration().unwrap())
                    .unwrap()
                    .suppressed
            );
        });
    }
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one regression keeps the complete grouped Fillet interruption and restoration transaction auditable"
    )]
    fn m87_typed_panel_grouped_fillet_camera_interruption_restores_without_history() {
        run_projectional_test_with_large_stack("m87-typed-panel-grouped-fillet-camera", || {
            let (code_project, editor) =
                super::code_projects::CodeProjectWorkbench::open_key("typed-panel")
                    .expect("Typed Panel code project");
            let mut authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor)
                .expect("Typed Panel browser authority");
            let mut camera = super::scene::CanvasCamera::default();
            assert!(super::fit_projectional_camera_to_authority(
                &mut camera,
                &authority,
            ));
            let viewport = camera.viewport();
            let mut features = authority
                .projectional_ref()
                .unwrap()
                .coordinator()
                .accepted_materialization()
                .expect("accepted Typed Panel materialization")
                .features
                .features()
                .iter()
                .map(|feature| feature.id)
                .collect::<Vec<_>>();
            features.sort_unstable();
            assert_eq!(features.len(), 2);
            let initiating = features[0];
            authority
                .projectional_mut()
                .unwrap()
                .set_selection([SelectionItem::Feature(initiating)]);
            let scene = authority
                .projectional_ref()
                .unwrap()
                .scene(viewport, super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS)
                .expect("selected Typed Panel scene");
            let rail = scene
                .fillet_affordances
                .iter()
                .find(|affordance| affordance.owner.feature == initiating)
                .expect("selected generated Fillet radius rail")
                .radius_rail;
            let pointer_id = 87_004;
            let pointer = |position| PointerInput {
                pointer_id,
                position,
                modifiers: Modifiers::default(),
            };
            authority
                .projectional_mut()
                .unwrap()
                .pointer_down(&scene, pointer(rail.screen_grip))
                .expect("start generated Fillet radius gesture");
            assert!(
                delegate_projectional_code_fillet_radius_drag(&mut authority, &code_project)
                    .expect("authenticate complete managed Fillet consumer group")
            );

            let source_before = code_project.managed_source().to_owned();
            let checkpoint_before = code_project.accepted_editor_checkpoint().clone();
            let persistence_before = code_project
                .to_persistence_json()
                .expect("code authority before camera interruption");
            let code_before = code_project.code_session_identity().clone();
            let intent_before = authority
                .projectional_ref()
                .unwrap()
                .coordinator()
                .intent()
                .identity();
            let nested_undo_before = authority
                .projectional_ref()
                .unwrap()
                .coordinator()
                .intent()
                .undo_len();
            let accepted_before = authority
                .snapshot()
                .expect("accepted authority before camera interruption")
                .encode()
                .expect("encoded accepted authority");

            let origin = viewport.screen_to_model(rail.screen_grip);
            let target = viewport.model_to_screen([
                (-1.5_f64).mul_add(rail.model_derivative[0], origin[0]),
                (-1.5_f64).mul_add(rail.model_derivative[1], origin[1]),
            ]);
            authority
                .projectional_mut()
                .unwrap()
                .pointer_move(&scene, pointer(target))
                .expect("preview grouped managed radius before camera change");
            let preview = authority
                .projectional_ref()
                .unwrap()
                .scene(viewport, super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS)
                .expect("grouped managed preview scene");
            assert!(features.iter().all(|feature| {
                let radii = preview
                    .computed_curves
                    .iter()
                    .filter(|curve| curve.owner.feature == *feature)
                    .map(|curve| curve.radius)
                    .collect::<Vec<_>>();
                !radii.is_empty()
                    && radii
                        .iter()
                        .all(|radius| radius.is_finite() && radius.to_bits() != 4.0_f64.to_bits())
            }));

            // Browser camera admission calls this exact presentation-independent
            // cancellation before it changes the camera. No terminal proposal is
            // available afterward for the outer code owner to publish.
            authority.projectional_mut().unwrap().cancel_interaction();
            assert!(
                authority
                    .projectional_ref()
                    .unwrap()
                    .editor()
                    .active_pointer_gesture()
                    .is_none()
            );
            assert_eq!(code_project.managed_source(), source_before);
            assert_eq!(
                code_project.accepted_editor_checkpoint(),
                &checkpoint_before
            );
            assert_eq!(code_project.code_session_identity(), &code_before);
            assert_eq!(
                code_project
                    .to_persistence_json()
                    .expect("code authority after camera interruption"),
                persistence_before,
            );
            assert_eq!(
                authority
                    .projectional_ref()
                    .unwrap()
                    .coordinator()
                    .intent()
                    .identity(),
                intent_before,
            );
            assert_eq!(
                authority
                    .projectional_ref()
                    .unwrap()
                    .coordinator()
                    .intent()
                    .undo_len(),
                nested_undo_before,
            );
            assert_eq!(
                authority
                    .snapshot()
                    .expect("accepted authority after camera interruption")
                    .encode()
                    .expect("encoded restored authority"),
                accepted_before,
            );
            let restored = authority
                .projectional_ref()
                .unwrap()
                .scene(viewport, super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS)
                .expect("accepted scene after camera interruption");
            for feature in features {
                let radii = restored
                    .computed_curves
                    .iter()
                    .filter(|curve| curve.owner.feature == feature)
                    .map(|curve| curve.radius.to_bits())
                    .collect::<Vec<_>>();
                assert!(!radii.is_empty());
                assert!(radii.iter().all(|radius| *radius == 4.0_f64.to_bits()));
            }
        });
    }
    #[test]
    fn projectional_browser_inspector_definition_and_point_component_edits_are_typed() {
        run_projectional_test_with_large_stack("projectional-browser-inspector-definition", || {
            let mut editor = projectional_authoring_fixture();
            let segment = editor
                .coordinator()
                .intent()
                .graph()
                .nodes()
                .iter()
                .find_map(|(node, declaration)| {
                    matches!(
                        declaration.kind,
                        IntentNodeKind::Geometry {
                            recipe: GeometryRecipeKind::Segment
                        }
                    )
                    .then_some(*node)
                })
                .unwrap();
            assert!(editor.set_selected_declaration(Some(segment)));
            let mut branch = current_definition_inspector_control(
                &editor,
                "branch_direction",
                ProjectionalInspectorSubmission::Point {
                    x: Some("0".into()),
                    y: Some("1".into()),
                },
            );
            branch.component = Some("y".into());
            let mut editor = ProjectionalInspectorFixture::from_editor(editor);
            assert_eq!(
                dispatch_projectional_inspector_control(&mut editor.0, &branch),
                ProjectionalInspectorDispatch::Committed(IntentPlanDisposition::Accepted)
            );
            assert_eq!(
                editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node(segment)
                    .unwrap()
                    .fields
                    .get(&IntentFieldKey(IntentKey::new("branch_direction").unwrap())),
                Some(&IntentLiteral::Point([0.0, 1.0]))
            );

            let role = current_definition_inspector_control(
                &editor,
                "role",
                ProjectionalInspectorSubmission::Text("construction".into()),
            );
            assert_eq!(
                dispatch_projectional_inspector_control(&mut editor.0, &role),
                ProjectionalInspectorDispatch::Committed(IntentPlanDisposition::Accepted)
            );
            assert_eq!(
                editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node(segment)
                    .unwrap()
                    .fields
                    .get(&IntentFieldKey(IntentKey::new("role").unwrap())),
                Some(&IntentLiteral::Enum(
                    IntentKey::new("construction").unwrap()
                ))
            );
        });
    }

    #[test]
    fn projectional_browser_inspector_retains_invalid_intent_over_accepted_scene() {
        run_projectional_test_with_large_stack("projectional-browser-inspector-retained", || {
            let mut authority = projectional_circle_inspector_fixture();
            let accepted_before = authority
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document()
                .clone();
            let history_before = authority
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let invalid = instance_inspector_control(&authority, LeafField::Value, Some(2.0), "0");
            let dispatch = dispatch_projectional_inspector_control(&mut authority.0, &invalid);
            assert_eq!(
                dispatch,
                ProjectionalInspectorDispatch::Committed(IntentPlanDisposition::RetainedFailed)
            );
            assert!(dispatch.saves_workspace());
            assert_eq!(
                authority
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before + 1
            );
            assert_eq!(
                authority
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
                    .session
                    .design_document(),
                &accepted_before
            );
            assert!(authority.undo().unwrap().is_some());
            assert_eq!(
                accepted_circle_radius(&authority).to_bits(),
                2.0_f64.to_bits()
            );
            assert!(authority.redo().unwrap().is_some());
            assert_eq!(
                authority
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
                    .session
                    .design_document(),
                &accepted_before
            );
        });
    }

    #[test]
    fn projectional_browser_inspector_rejects_wrong_and_stale_controls_without_history() {
        run_projectional_test_with_large_stack("projectional-browser-inspector-auth", || {
            let mut authority = projectional_circle_inspector_fixture();
            let stale = instance_inspector_control(&authority, LeafField::Value, Some(2.0), "4");
            let history_before = authority
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();

            let mut wrong_unit = stale.clone();
            wrong_unit.unit = Some("angle".into());
            let rejected = dispatch_projectional_inspector_control(&mut authority.0, &wrong_unit);
            assert!(matches!(
                rejected,
                ProjectionalInspectorDispatch::Rejected(_)
            ));
            assert!(!rejected.saves_workspace());

            let mut wrong_node = stale.clone();
            wrong_node.node = Some(NodeId::from_raw(0xdead).to_string());
            let rejected = dispatch_projectional_inspector_control(&mut authority.0, &wrong_node);
            assert!(matches!(
                rejected,
                ProjectionalInspectorDispatch::Rejected(_)
            ));
            assert!(!rejected.saves_workspace());
            assert_eq!(
                authority
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before
            );

            let current = instance_inspector_control(&authority, LeafField::Value, Some(2.0), "3");
            assert!(matches!(
                dispatch_projectional_inspector_control(&mut authority.0, &current),
                ProjectionalInspectorDispatch::Committed(IntentPlanDisposition::Accepted)
            ));
            let history_after_current = authority
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let rejected = dispatch_projectional_inspector_control(&mut authority.0, &stale);
            assert!(matches!(
                rejected,
                ProjectionalInspectorDispatch::Rejected(_)
            ));
            assert!(!rejected.saves_workspace());
            assert_eq!(
                authority
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_after_current
            );
            assert_eq!(
                accepted_circle_radius(&authority).to_bits(),
                3.0_f64.to_bits()
            );
        });
    }

    #[test]
    fn projectional_v8_routes_scene_save_history_and_design_markup_through_one_authority() {
        run_projectional_test_with_large_stack("projectional-v8-authority", || {
            let (mut authority, node) = projectional_workbench_fixture();
            assert!(authority.is_projectional());
            assert!(authority.flat_ref().is_none());
            assert!(
                authority
                    .scene_presentation(test_viewport(), 0.5)
                    .scene
                    .is_some()
            );
            assert!(
                authority
                    .snapshot()
                    .unwrap()
                    .encode()
                    .unwrap()
                    .contains("\"version\":8")
            );

            let projectional = authority.projectional_mut().unwrap();
            assert!(projectional.set_selected_declaration(Some(node)));
            let markup = projectional_design_markup(projectional, None, None).unwrap();
            assert_eq!(markup.declaration_count, 1);
            assert!(markup.outline.contains("aria-selected=\"true\""));
            assert!(markup.source.contains("wb-intent-source-token"));
            assert!(markup.source.contains("data-intent-drop-before="));
            assert!(markup.source.contains("draggable=\"true\""));
            assert!(
                markup
                    .history
                    .contains("data-intent-history-state=\"applied\"")
            );
            assert!(markup.inspector.contains("data-intent-edit=\"name\""));
            let projection = projectional.workbench_projection();
            let name_token = projection
                .structured_source
                .tokens
                .iter()
                .find(|token| {
                    matches!(
                        token.target,
                        geosolve_constraint_editor::IntentSourceTokenTarget::NodeName { .. }
                    )
                })
                .unwrap()
                .id;
            projectional
                .edit_source_token(&projection, name_token, "\"renamed point\"")
                .unwrap();
            assert!(
                projectional_design_markup(projectional, None, None)
                    .unwrap()
                    .inspector
                    .contains("renamed point")
            );

            assert!(authority.step_history(true).unwrap());
            assert!(
                authority
                    .scene_presentation(test_viewport(), 0.5)
                    .scene
                    .is_some()
            );
            assert!(authority.step_history(true).unwrap());
            assert!(
                authority
                    .scene_presentation(test_viewport(), 0.5)
                    .scene
                    .is_none()
            );
            assert!(authority.step_history(false).unwrap());
            assert!(
                authority
                    .scene_presentation(test_viewport(), 0.5)
                    .scene
                    .is_some()
            );
            assert!(authority.step_history(false).unwrap());
            assert!(
                authority
                    .scene_presentation(test_viewport(), 0.5)
                    .scene
                    .is_some()
            );
        });
    }

    #[test]
    fn projectional_pointer_queue_keeps_the_newest_sample_and_authenticates_generations() {
        let input = |x| PointerInput {
            pointer_id: 83,
            position: ScreenPoint { x, y: 120.0 },
            modifiers: Modifiers::default(),
        };
        let mut queue = ProjectionalPointerMoveQueue::default();
        let first = queue.push(input(100.0)).expect("first RAF generation");
        assert_eq!(queue.push(input(110.0)), None);
        assert_eq!(
            queue.take_for_frame(first).map(|sample| sample.input),
            Some(input(110.0))
        );
        assert_eq!(queue.take_for_frame(first), None);

        let failed = queue.push(input(120.0)).expect("failed RAF generation");
        queue.cancel_frame(failed);
        let retried = queue
            .push(input(125.0))
            .expect("replacement RAF generation");
        assert_ne!(retried, failed);
        assert_eq!(queue.take_for_frame(failed), None);
        assert_eq!(
            queue.take_for_frame(retried).map(|sample| sample.input),
            Some(input(125.0))
        );

        let stale = queue.push(input(130.0)).expect("terminal RAF generation");
        assert_eq!(queue.push(input(140.0)), None);
        assert_eq!(
            queue.drain_before_terminal().map(|sample| sample.input),
            Some(input(140.0))
        );
        assert_eq!(queue.take_for_frame(stale), None);
        assert!(!queue.invalidate());

        let invalidated = queue.push(input(150.0)).expect("cancelled RAF generation");
        assert!(queue.invalidate());
        assert_eq!(queue.take_for_frame(invalidated), None);
    }

    #[test]
    fn projectional_terminal_sample_recovers_from_rejected_queued_preview_on_every_drag_route() {
        for route in ["point", "pre-Apply Fillet", "pre-Apply Offset"] {
            let mut visited = Vec::new();
            let outcome = super::replay_projectional_terminal_samples(
                Some("rejected queued sample"),
                "exact valid release",
                |sample| {
                    visited.push(sample);
                    if sample == "rejected queued sample" {
                        Err(route)
                    } else {
                        Ok(())
                    }
                },
            );
            assert_eq!(outcome, Ok(()), "{route}");
            assert_eq!(
                visited,
                ["rejected queued sample", "exact valid release"],
                "{route} must evaluate the exact release after preview rejection"
            );
        }
    }

    #[test]
    fn projectional_browser_geometry_dispatch_commits_one_typed_terminal() {
        run_projectional_test_with_large_stack("projectional-browser-geometry-terminal", || {
            let (mut authority, _node) = projectional_workbench_fixture();
            let projectional = authority.projectional_mut().unwrap();
            projectional
                .editor_mut()
                .set_authoring_geometry_role(GeometryRole::Construction);
            let activation = projectional
                .editor_mut()
                .activate_geometry_tool(GeometryToolVariant::Segment);
            assert!(activation.is_empty());
            let history_before = projectional
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let viewport = test_viewport();
            let input = |model_position| PointerInput {
                pointer_id: 831,
                position: viewport.model_to_screen(model_position),
                modifiers: Modifiers::default(),
            };
            let mut preview = None;

            let scene = projectional.scene(viewport, 0.5).unwrap();
            let effects = projectional.editor_mut().pointer_down_with_draft_authoring(
                &scene,
                input([-1.0, -1.0]),
                super::effect_adapter::draft_authoring_input(Modifiers::default(), None),
            );
            let first =
                dispatch_projectional_construction_effects(projectional, &mut preview, effects);
            assert_eq!(first, ProjectionalConstructionDispatch::default());
            assert!(preview.is_some());
            assert_eq!(
                projectional
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before,
                "a staged browser click must remain transient",
            );

            let scene = projectional.scene(viewport, 0.5).unwrap();
            let effects = projectional.editor_mut().pointer_down_with_draft_authoring(
                &scene,
                input([1.0, -1.0]),
                super::effect_adapter::draft_authoring_input(Modifiers::default(), None),
            );
            let terminal =
                dispatch_projectional_construction_effects(projectional, &mut preview, effects);
            assert!(terminal.accepted_terminal);
            assert!(!terminal.rejected_terminal);
            assert!(terminal.error.is_none());
            assert!(preview.is_none());
            assert_eq!(
                projectional
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before + 1,
            );
            let design = projectional
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document();
            assert_eq!(design.curves().len(), 1);
            assert_eq!(
                design.geometry_role(design.curves()[0].id),
                Some(GeometryRole::Construction)
            );
        });
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one browser-boundary regression keeps collection, exact hover/pick metadata, terminal publication and retained native output together"
    )]
    fn projectional_browser_relation_collection_preserves_exact_pick_and_repeated_history() {
        run_projectional_test_with_large_stack("projectional-browser-relation-authoring", || {
            let mut editor = projectional_authoring_fixture();
            let viewport = test_viewport();
            let document = editor
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document()
                .clone();
            let point = document.points()[0].id;
            let span = document.curve_spans(document.curves()[0].id).unwrap()[0];
            let mut authoring = AuthoringState::default();
            assert!(matches!(
                authoring.activate(
                    &document,
                    AuthoringTool::Constraint(ConstraintIntent::Coincident),
                    &[],
                ),
                AuthoringOutcome::ModeEntered { .. }
            ));
            let history_before = editor
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let scene = editor.scene(viewport, 0.5).unwrap();
            let point_position = scene.viewport.model_to_screen([1.0, 2.0]);
            assert!(matches!(
                authoring.pick_at_with_policy(
                    &document,
                    &scene,
                    point_position,
                    PickTolerance::default(),
                    GeometryInteractionPolicy::default(),
                ),
                AuthoringOutcome::Collecting { .. }
            ));
            assert_eq!(authoring.pending()[0].item, SelectionItem::Point(point));

            let picked_parameter = 0.75;
            let curve_position = scene
                .viewport
                .model_to_screen([4.0 * picked_parameter, 0.0]);
            let hover_effects = editor.pointer_move_authoring(
                &authoring,
                &scene,
                PointerInput {
                    pointer_id: 832,
                    position: curve_position,
                    modifiers: Modifiers::default(),
                },
                PickTolerance::default(),
            );
            assert!(!hover_effects.is_empty());
            assert_eq!(editor.editor().hovered(), Some(SelectionItem::Curve(span)));
            assert_eq!(
                editor
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before,
                "an authoring hover frame must not serialize intent",
            );

            let application = match authoring.pick_at_with_policy(
                &document,
                &scene,
                curve_position,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ) {
                AuthoringOutcome::Apply(application) => application,
                outcome => panic!("expected a complete point/curve application, got {outcome:?}"),
            };
            assert_eq!(application.operands[1].item, SelectionItem::Curve(span));
            assert!(
                (application.operands[1].curve_parameter.unwrap() - picked_parameter).abs()
                    <= f64::EPSILON
            );
            let dispatch = dispatch_projectional_authoring_application(
                &mut editor,
                &mut authoring,
                &application,
            );
            assert_eq!(dispatch.disposition, Some(IntentPlanDisposition::Accepted));
            assert!(dispatch.error.is_none());
            assert_eq!(
                editor
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before + 1,
            );
            assert_eq!(
                authoring.active_tool(),
                Some(AuthoringTool::Constraint(ConstraintIntent::Coincident)),
            );
            assert!(authoring.pending().is_empty());

            let accepted = editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document();
            let DocumentConstraintDefinition::PointOnCurve {
                point: actual,
                contact,
            } = accepted.constraints()[0].definition
            else {
                panic!("browser point/curve application must lower to point-on-curve");
            };
            assert_eq!(actual, point);
            let contact = accepted.contact(contact).unwrap();
            assert_eq!(contact.curve, span);
            assert_eq!(
                contact.domain,
                ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                }
            );
            assert_eq!(contact.neighborhood, ContactNeighborhood::Interior);
        });
    }

    #[test]
    fn projectional_browser_dimension_application_uses_accepted_measurement_once() {
        run_projectional_test_with_large_stack("projectional-browser-dimension-authoring", || {
            let mut editor = projectional_authoring_fixture();
            let document = editor
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document()
                .clone();
            let span = document.curve_spans(document.curves()[0].id).unwrap()[0];
            let mut authoring = AuthoringState::default();
            let mut options = authoring.options();
            options.dimension_mode = DocumentDimensionMode::Reference;
            authoring.set_options(options);
            let outcome = authoring.activate(
                &document,
                AuthoringTool::Dimension(DimensionKind::SegmentLength),
                &[AuthoringOperand::picked(
                    SelectionItem::Curve(span),
                    Some(0.625),
                )],
            );
            let AuthoringOutcome::Apply(application) = outcome else {
                panic!("a selected segment must complete length authoring");
            };
            let history_before = editor
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let dispatch = dispatch_projectional_authoring_application(
                &mut editor,
                &mut authoring,
                &application,
            );
            assert_eq!(dispatch.disposition, Some(IntentPlanDisposition::Accepted));
            assert_eq!(
                editor
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before + 1,
            );
            let accepted = editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document();
            assert_eq!(accepted.dimensions().len(), 1);
            assert_eq!(
                accepted.dimensions()[0].mode,
                DocumentDimensionMode::Reference
            );
            let DocumentDimensionDefinition::CurveLength { curve, target } =
                accepted.dimensions()[0].definition
            else {
                panic!("segment length authoring must retain a curve-length dimension");
            };
            assert_eq!(curve, span);
            assert_eq!(
                accepted.scalar(target).unwrap().value.to_bits(),
                4.0_f64.to_bits(),
                "the target comes from the accepted native scene measurement",
            );
        });
    }

    #[test]
    fn projectional_browser_retained_invalid_relation_is_a_single_durable_terminal() {
        run_projectional_test_with_large_stack("projectional-browser-retained-relation", || {
            let mut editor = projectional_authoring_fixture();
            let document = editor
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document()
                .clone();
            let span = document.curve_spans(document.curves()[0].id).unwrap()[0];
            let CurveDefinition::Line { start, end, .. } =
                document.curve(span.curve).unwrap().definition
            else {
                panic!("fixture curve must be a line");
            };
            let mut authoring = AuthoringState::default();
            for point in [start, end] {
                let AuthoringOutcome::Apply(application) = authoring.activate(
                    &document,
                    AuthoringTool::Constraint(ConstraintIntent::Lock),
                    &[AuthoringOperand::selected(SelectionItem::Point(point))],
                ) else {
                    panic!("fixture endpoint must accept Lock");
                };
                assert_eq!(
                    dispatch_projectional_authoring_application(
                        &mut editor,
                        &mut authoring,
                        &application,
                    )
                    .disposition,
                    Some(IntentPlanDisposition::Accepted),
                );
            }
            let accepted_before = editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document()
                .clone();
            let history_before = editor
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let AuthoringOutcome::Apply(application) = authoring.activate(
                &accepted_before,
                AuthoringTool::Constraint(ConstraintIntent::Vertical),
                &[AuthoringOperand::selected(SelectionItem::Curve(span))],
            ) else {
                panic!("the explicit vertical relation is applicable before solving");
            };
            let dispatch = dispatch_projectional_authoring_application(
                &mut editor,
                &mut authoring,
                &application,
            );
            assert!(dispatch.committed());
            assert_eq!(
                dispatch.disposition,
                Some(IntentPlanDisposition::RetainedFailed)
            );
            assert_eq!(
                editor
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before + 1,
            );
            assert_eq!(
                editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
                    .session
                    .design_document(),
                &accepted_before,
                "retained invalid intent must not replace accepted browser geometry",
            );
        });
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one queued-drag regression keeps terminal commit, capture release and durable presentation counts together"
    )]
    fn projectional_queued_drag_commits_only_the_exact_terminal_sample_once() {
        run_projectional_test_with_large_stack("projectional-queued-drag-commit", || {
            let (mut authority, _node) = projectional_workbench_fixture();
            let projectional = authority.projectional_mut().unwrap();
            let point = projectional
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document()
                .points()
                .first()
                .expect("fixture point")
                .id;
            let viewport = test_viewport();
            let pointer = |model_position| PointerInput {
                pointer_id: 83,
                position: viewport.model_to_screen(model_position),
                modifiers: Modifiers::default(),
            };
            let position = |session: &ProjectionalEditorSession| {
                session
                    .coordinator()
                    .presentation_session()
                    .unwrap()
                    .accepted_state_for_current_input()
                    .unwrap()
                    .document()
                    .point(point)
                    .unwrap()
                    .position
            };
            let initial_identity = projectional.coordinator().intent().identity();
            let initial_history = projectional
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let scene = projectional.scene(viewport, 0.5).unwrap();
            projectional
                .pointer_down(&scene, pointer([2.0, 3.0]))
                .unwrap();

            let mut queue = ProjectionalPointerMoveQueue::default();
            let generation = queue.push(pointer([3.0, 4.0])).unwrap();
            assert_eq!(queue.push(pointer([4.0, 5.0])), None);
            let newest_frame = queue.take_for_frame(generation).unwrap().input;
            let scene = projectional.scene(viewport, 0.5).unwrap();
            projectional.pointer_move(&scene, newest_frame).unwrap();
            assert_eq!(
                position(projectional).map(f64::to_bits),
                [4.0_f64, 5.0].map(f64::to_bits),
            );
            assert_eq!(
                projectional.coordinator().intent().identity(),
                initial_identity
            );
            assert_eq!(
                projectional
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                initial_history,
                "a resolved pointer frame must not enter durable history",
            );

            let stale_terminal_frame = queue.push(pointer([5.0, 6.0])).unwrap();
            assert_eq!(queue.push(pointer([6.0, 7.0])), None);
            let pending = queue.drain_before_terminal().unwrap().input;
            assert_eq!(queue.take_for_frame(stale_terminal_frame), None);
            let scene = projectional.scene(viewport, 0.5).unwrap();
            projectional.pointer_move(&scene, pending).unwrap();
            let exact_terminal = pointer([7.0, 8.0]);
            let scene = projectional.scene(viewport, 0.5).unwrap();
            projectional.pointer_move(&scene, exact_terminal).unwrap();
            assert_eq!(
                projectional.coordinator().intent().identity(),
                initial_identity
            );

            let scene = projectional.scene(viewport, 0.5).unwrap();
            let outcome = projectional.pointer_up(&scene, exact_terminal).unwrap();
            assert!(outcome.transaction.is_some());
            let mut captured_pointer = Some(83);
            assert_eq!(
                route_projectional_terminal_capture(&mut captured_pointer, Some(83)),
                Some(83),
                "the successful browser pointer-up retires capture before platform release",
            );
            let mut notice = "Projectional direct movement accepted".to_owned();
            let mut presentation = WorkbenchPresentationCounters::default();
            presentation.record(WorkbenchPresentationEvent::PointerRelease);
            if route_projectional_terminal_capture(&mut captured_pointer, Some(83)).is_some() {
                projectional.cancel_interaction();
                notice =
                    "Projectional interaction canceled because pointer capture was lost".to_owned();
                presentation.record(WorkbenchPresentationEvent::InteractionCancellation);
            }
            assert_eq!(
                position(projectional).map(f64::to_bits),
                [7.0_f64, 8.0].map(f64::to_bits),
            );
            assert_eq!(
                projectional
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                initial_history + 1,
            );
            assert_eq!(notice, "Projectional direct movement accepted");
            assert_eq!(
                presentation,
                WorkbenchPresentationCounters {
                    transient_renders: 0,
                    durable_renders: 1,
                    workspace_saves: 1,
                    durable_panel_rebuilds: 1,
                },
                "lostpointercapture after pointer-up cannot overwrite success or render again",
            );
            projectional.undo().unwrap().unwrap();
            assert_eq!(
                position(projectional).map(f64::to_bits),
                [2.0_f64, 3.0].map(f64::to_bits),
            );
        });
    }

    #[test]
    fn projectional_queued_drag_cancellation_restores_accepted_authority_without_history() {
        run_projectional_test_with_large_stack("projectional-queued-drag-cancel", || {
            let (mut authority, _node) = projectional_workbench_fixture();
            let projectional = authority.projectional_mut().unwrap();
            let point = projectional
                .coordinator()
                .presentation_session()
                .unwrap()
                .design_document()
                .points()
                .first()
                .expect("fixture point")
                .id;
            let viewport = test_viewport();
            let pointer = |model_position| PointerInput {
                pointer_id: 84,
                position: viewport.model_to_screen(model_position),
                modifiers: Modifiers::default(),
            };
            let history_before = projectional
                .coordinator()
                .intent()
                .history_projection()
                .applied
                .len();
            let scene = projectional.scene(viewport, 0.5).unwrap();
            projectional
                .pointer_down(&scene, pointer([2.0, 3.0]))
                .unwrap();
            let mut queue = ProjectionalPointerMoveQueue::default();
            let generation = queue.push(pointer([-3.0, 5.0])).unwrap();
            let sample = queue.take_for_frame(generation).unwrap().input;
            let scene = projectional.scene(viewport, 0.5).unwrap();
            projectional.pointer_move(&scene, sample).unwrap();
            assert_eq!(
                projectional
                    .coordinator()
                    .presentation_session()
                    .unwrap()
                    .accepted_state_for_current_input()
                    .unwrap()
                    .document()
                    .point(point)
                    .unwrap()
                    .position
                    .map(f64::to_bits),
                [-3.0_f64, 5.0].map(f64::to_bits),
            );

            let stale = queue.push(pointer([9.0, 9.0])).unwrap();
            assert!(queue.invalidate());
            assert_eq!(queue.take_for_frame(stale), None);
            projectional.cancel_interaction();
            assert_eq!(
                projectional
                    .coordinator()
                    .presentation_session()
                    .unwrap()
                    .accepted_state_for_current_input()
                    .unwrap()
                    .document()
                    .point(point)
                    .unwrap()
                    .position
                    .map(f64::to_bits),
                [2.0_f64, 3.0].map(f64::to_bits),
            );
            assert_eq!(
                projectional
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                history_before,
            );
            let cancellation = super::WorkbenchPresentationEvent::InteractionCancellation.policy();
            assert!(!cancellation.saves_workspace);
            assert_eq!(cancellation.render_scope, WorkbenchRenderScope::Durable);
        });
    }

    #[test]
    fn flat_v6_routes_through_history_free_projectional_bootstrap_authority() {
        run_projectional_test_with_large_stack("projectional-flat-v6-bootstrap", || {
            let coordinator = RetainedEditorCoordinator::new(
                RetainedSketchDocumentSession::new(
                    SketchDocument::new(1.0).unwrap(),
                    DocumentSolveRequest::default(),
                    SolverConfig::default(),
                )
                .unwrap(),
            )
            .unwrap();
            let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator).unwrap();
            let decoded = WorkspaceSnapshot::decode(&snapshot.encode().unwrap()).unwrap();
            let authority = WorkbenchDocumentAuthority::from_snapshot(&decoded).unwrap();
            assert!(authority.is_projectional());
            assert!(authority.flat_ref().is_none());
            let projectional = authority.projectional_ref().unwrap();
            assert_eq!(
                projectional
                    .coordinator()
                    .intent()
                    .history_projection()
                    .applied
                    .len(),
                0
            );
            assert!(
                authority
                    .scene_presentation(test_viewport(), 0.5)
                    .scene
                    .is_some()
            );
            assert!(
                authority
                    .snapshot()
                    .unwrap()
                    .encode()
                    .unwrap()
                    .contains("\"version\":8")
            );
        });
    }

    fn rejected_constraint_fixture() -> (
        RetainedEditorCoordinator,
        [CurveSpan; 2],
        SketchAcceptedStateIdentity,
        String,
    ) {
        let mut document = SketchDocument::new(1.0).expect("document");
        let points = [
            document.add_point("first start", [0.0, 0.0]).unwrap(),
            document.add_point("first end", [2.0, 0.0]).unwrap(),
            document.add_point("second start", [0.0, 2.0]).unwrap(),
            document.add_point("second end", [2.0, 2.0]).unwrap(),
        ];
        let lines = [
            CurveSpan::line(
                document
                    .add_curve(
                        "first line",
                        CurveDefinition::Line {
                            start: points[0],
                            end: points[1],
                            branch_direction: [1.0, 0.0],
                        },
                    )
                    .unwrap(),
            ),
            CurveSpan::line(
                document
                    .add_curve(
                        "second line",
                        CurveDefinition::Line {
                            start: points[2],
                            end: points[3],
                            branch_direction: [1.0, 0.0],
                        },
                    )
                    .unwrap(),
            ),
        ];
        for point in points {
            let target = document.point(point).expect("fixed point").position;
            document
                .add_constraint(
                    format!("fix {point}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
        let mut session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted fixed lines");
        let accepted_before = session.accepted_state().expect("accepted parent");
        let accepted_identity = accepted_before.identity();
        let accepted_json = accepted_before
            .document()
            .to_canonical_json()
            .expect("accepted parent JSON");
        let outcome = session
            .transact(session.design_identity(), |document| {
                document.set_point_position(points[0], [40.0, 40.0])?;
                document.add_constraint(
                    "conflicting attempted point",
                    DocumentConstraintDefinition::FixedPoint {
                        point: points[0],
                        target: [40.0, 40.0],
                    },
                )
            })
            .expect("retained rejected constraint and coordinate edit");
        assert!(outcome.published_accepted_identity().is_none());
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        assert!(
            coordinator
                .session()
                .design_document()
                .point(points[0])
                .is_some_and(|point| {
                    point.position.map(f64::to_bits) == [40.0, 40.0].map(f64::to_bits)
                })
        );
        (coordinator, lines, accepted_identity, accepted_json)
    }

    #[test]
    fn rejected_constraint_keeps_a_detached_accepted_canvas_scene() {
        let (coordinator, lines, accepted_identity, accepted_json) = rejected_constraint_fixture();
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_none()
        );
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("historical accepted parent");
        assert_eq!(accepted.identity(), accepted_identity);
        assert_eq!(
            accepted.document().to_canonical_json().unwrap(),
            accepted_json
        );
        let attempted_point = match &coordinator
            .session()
            .design_document()
            .curve(lines[0].curve)
            .expect("attempted line")
            .definition
        {
            CurveDefinition::Line { start, .. } => *start,
            _ => panic!("line definition"),
        };
        assert_ne!(
            coordinator
                .session()
                .design_document()
                .point(attempted_point)
                .expect("attempted point")
                .position
                .map(f64::to_bits),
            accepted
                .document()
                .point(attempted_point)
                .expect("accepted point")
                .position
                .map(f64::to_bits),
            "the fixture must distinguish attempted from accepted geometry"
        );

        let viewport = super::scene::viewport();
        let scene = compose_editor_scene(&coordinator, viewport, 0.25)
            .expect("detached accepted presentation scene");
        let expected = geosolve_constraint_editor::EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport,
            0.25,
        )
        .expect("expected accepted presentation");
        assert_eq!(scene.points, expected.points);
        assert_eq!(scene.curves, expected.curves);
        assert_eq!(scene.curves.len(), 2);
        assert_eq!(scene.points.len(), 4);
        assert_rejected_constraint_scene_authority(&coordinator, accepted, &scene);
        assert!(
            scene
                .clone()
                .with_retained_session(coordinator.session())
                .is_err(),
            "historical accepted presentation must not gain inference-publication authority"
        );
        let problem = coordinator
            .current_problem_metadata()
            .expect("visible rejected-attempt problem");
        let markup = super::scene::svg_markup(
            Some(&scene),
            Some(accepted),
            &[],
            None,
            Some(&problem),
            viewport,
        );
        assert!(markup.contains("data-scene-provenance=\"accepted\""));
        assert!(markup.contains("data-problem-scope=\""));
        assert!(markup.contains("data-problem-marker=\""));
        assert!(markup.contains("wb-error-marker-icon"));
        for line in lines {
            assert!(markup.contains(&format!("data-persistent-id=\"{}\"", line.curve)));
        }
    }

    #[test]
    fn current_problem_targets_are_forwarded_to_problem_aware_pointer_input() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [2.0, 0.0]).expect("point");
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        for (label, point, target) in [
            ("fix first", first, [0.0, 0.0]),
            ("fix second", second, [2.0, 0.0]),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("fixed point");
        }
        let target = document
            .add_scalar(
                "conflicting length",
                3.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("length target");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted fixed line");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateDimension {
                    label: "conflicting length".into(),
                    definition: DocumentDimensionDefinition::CurveLength {
                        curve: CurveSpan::line(line),
                        target,
                    },
                    mode: DocumentDimensionMode::Driving,
                },
            )
            .expect("retained rejected dimension");
        let mut scene = compose_editor_scene(&coordinator, super::scene::viewport(), 0.25)
            .expect("detached accepted presentation scene");
        let problem_items = current_problem_items(&coordinator, &scene);
        assert!(
            !problem_items.is_empty(),
            "the rejected attempt must expose at least one mapped problem target",
        );
        let item = problem_items
            .iter()
            .copied()
            .find(|item| matches!(item, SelectionItem::Dimension(_)))
            .expect("rejected dimension target must be forwarded");
        let annotation = scene
            .annotations
            .first_mut()
            .expect("fixed-line scene retains annotation geometry");
        let probe = ScreenPoint { x: 40.0, y: 40.0 };
        annotation.item = item;
        annotation.visibility = SceneAnnotationVisibility::Contextual;
        annotation.geometry = SceneAnnotationGeometry::Label {
            anchor: probe,
            leader_from: None,
        };
        scene
            .annotations
            .retain(|annotation| annotation.item == item);

        let input = PointerInput {
            pointer_id: 301,
            position: probe,
            modifiers: Modifiers::default(),
        };
        let mut ordinary = ConstraintEditor::default();
        assert!(ordinary.pointer_move(&scene, input).is_empty());

        let mut problem_aware = ConstraintEditor::default();
        let effects = problem_aware.pointer_move_with_problem_items(&scene, input, &problem_items);
        assert_eq!(
            effects,
            vec![geosolve_constraint_editor::EditorEffect::HoverChanged(
                EditorHoverState {
                    target: Some(EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                        item,
                        marker_index: None,
                    })),
                    context_owner: None,
                },
            )],
            "the browser adapter's current problem items must reach headless hover resolution",
        );
    }

    fn assert_rejected_constraint_scene_authority(
        coordinator: &RetainedEditorCoordinator,
        accepted: &geosolve_sketch::SketchAcceptedDocumentState,
        scene: &EditorScene,
    ) {
        let rejected_constraint = coordinator
            .session()
            .design_document()
            .constraints()
            .iter()
            .find(|constraint| constraint.label == "conflicting attempted point")
            .expect("design-only rejected constraint");
        assert!(
            accepted
                .document()
                .constraint(rejected_constraint.id)
                .is_none(),
            "the rejected constraint must not enter accepted geometry authority"
        );
        let rejected_entry = scene
            .constraint_entries
            .iter()
            .find(|entry| entry.id == rejected_constraint.id)
            .expect("composed scene must retain rejected design intent");
        assert_eq!(rejected_entry.source, rejected_constraint.source_id);
        assert_eq!(rejected_entry.label, rejected_constraint.label);
        assert!(
            scene.annotations.iter().all(|annotation| {
                annotation.item != SelectionItem::Constraint(rejected_constraint.id)
            }),
            "the composed scene must not invent annotation geometry for rejected intent"
        );
    }

    #[test]
    fn current_computed_fillet_canvas_scene_stays_composite_and_authorized() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        assert!(source.accepted_state_for_current_input().is_some());

        let scene = compose_editor_scene(&coordinator, super::scene::viewport(), 0.25)
            .expect("current composite Fillet scene");
        assert_eq!(scene.computed_input.as_ref(), Some(&metadata.input));
        assert_eq!(scene.computed_curves.len(), 2);
        assert_eq!(scene.fillet_affordances.len(), 2);
        assert!(
            scene
                .curves
                .iter()
                .any(|curve| matches!(curve.origin, SceneCurveOrigin::FilletDiscarded { .. }))
        );
        assert!(
            scene.with_retained_session(source).is_ok(),
            "a current exact-stamped composite scene retains inference authority"
        );
    }

    type SceneOracleCheck = Result<(), &'static str>;
    type SceneOracleCase = (&'static str, fn() -> SceneOracleCheck);

    #[derive(Clone, Copy)]
    struct SceneOracleResult {
        case_id: &'static str,
        status: &'static str,
        failure_class: &'static str,
        fingerprint: &'static str,
    }

    fn scene_oracle_require(condition: bool, fingerprint: &'static str) -> SceneOracleCheck {
        if condition { Ok(()) } else { Err(fingerprint) }
    }

    fn scene_oracle_current_native_expected(
        coordinator: &RetainedEditorCoordinator,
        viewport: Viewport,
    ) -> Result<EditorScene, &'static str> {
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        let accepted = source
            .accepted_state_for_current_input()
            .ok_or("current-accepted-state-missing")?;
        EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport,
            0.25,
        )
        .map_err(|_| "native-scene-construction-failed")?
        .with_retained_session(source)
        .map_err(|_| "native-scene-authentication-failed")
    }

    fn scene_oracle_markup(
        coordinator: &RetainedEditorCoordinator,
        scene: &EditorScene,
        viewport: Viewport,
    ) -> String {
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        let computed_problems = coordinator.computed_feature_problems();
        let current_problem = coordinator.current_problem_metadata();
        super::scene::svg_markup_with_computed_context(
            Some(scene),
            source.accepted_state(),
            &computed_problems,
            &[],
            &[],
            EditorHoverState::default(),
            None,
            current_problem.as_ref(),
            None,
            viewport,
        )
    }

    fn scene_oracle_current_computed_empty() -> SceneOracleCheck {
        let (coordinator, _, _) = grouped_fillet_fixture();
        let (expected_input, snapshot) = match coordinator.computed_scene_state() {
            ComputedSceneState::Current { expected, snapshot } => (*expected, snapshot),
            ComputedSceneState::Withheld | ComputedSceneState::Absent => {
                return Err("expected-current-empty-computed-state");
            }
        };
        let source = coordinator.session();
        let accepted = source
            .accepted_state_for_current_input()
            .ok_or("current-accepted-state-missing")?;
        scene_oracle_require(
            accepted
                .document()
                .to_canonical_json()
                .map_err(|_| "accepted-json-failed")?
                == source
                    .design_document()
                    .to_canonical_json()
                    .map_err(|_| "design-json-failed")?,
            "accepted-design-geometry-diverged",
        )?;

        let viewport = super::scene::viewport();
        let scene = compose_editor_scene(&coordinator, viewport, 0.25)
            .ok_or("compose-returned-no-scene")?;
        let accepted_input = source
            .accepted_prepared_input()
            .ok_or("empty-computed-accepted-input-missing")?;
        let mut expected = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &accepted_input,
            &expected_input,
            snapshot,
            viewport,
            0.25,
        )
        .map_err(|_| "expected-empty-composite-construction-failed")?;
        coordinator
            .populate_computed_fillet_affordances(&mut expected, &[], 0.25)
            .map_err(|_| "expected-empty-composite-affordances-failed")?;
        let expected = expected
            .with_retained_session(source)
            .map_err(|_| "expected-empty-composite-authentication-failed")?;
        scene_oracle_require(scene == expected, "empty-composite-provenance-mismatch")?;
        scene_oracle_require(
            scene.computed_input.as_ref() == Some(&expected_input)
                && scene.computed_curves.is_empty()
                && scene.fillet_affordances.is_empty()
                && scene
                    .curves
                    .iter()
                    .all(|curve| curve.origin == SceneCurveOrigin::Native),
            "empty-composite-leaked-generated-geometry",
        )?;
        scene_oracle_require(
            scene.clone().with_retained_session(source).is_ok(),
            "current-empty-computed-scene-lost-authentication",
        )?;
        scene_oracle_require(
            coordinator.current_problem_metadata().is_none()
                && coordinator.computed_feature_problems().is_empty(),
            "clean-native-scene-published-problem",
        )?;
        let markup = scene_oracle_markup(&coordinator, &scene, viewport);
        scene_oracle_require(
            markup.contains("data-scene-provenance=\"accepted\"")
                && !markup.contains("data-problem-marker="),
            "clean-empty-computed-scene-markup-provenance",
        )
    }

    fn scene_oracle_current_native_withheld() -> SceneOracleCheck {
        let (base, _, _) = grouped_fillet_fixture();
        let features = base.feature_document().clone();
        let coordinator = RetainedEditorCoordinator::with_features_and_high_water(
            base.session().clone(),
            features.clone(),
            features.lifecycle_high_water(),
            geosolve_sketch_features::ComputedEvaluationAllocatorHighWater {
                next_revision: geosolve_sketch_features::ComputedEvaluationRevision::from_raw(
                    u64::MAX,
                ),
            },
        )
        .map_err(|_| "withheld-coordinator-construction-failed")?;
        scene_oracle_require(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some(),
            "withheld-current-accepted-state-missing",
        )?;
        scene_oracle_require(
            matches!(
                coordinator.computed_scene_state(),
                ComputedSceneState::Withheld
            ),
            "expected-withheld-computed-state",
        )?;
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .ok_or("withheld-current-accepted-state-missing")?;
        scene_oracle_require(
            accepted
                .document()
                .to_canonical_json()
                .map_err(|_| "accepted-json-failed")?
                == coordinator
                    .session()
                    .design_document()
                    .to_canonical_json()
                    .map_err(|_| "design-json-failed")?,
            "withheld-accepted-design-geometry-diverged",
        )?;

        let viewport = super::scene::viewport();
        let scene = compose_editor_scene(&coordinator, viewport, 0.25)
            .ok_or("withheld-compose-returned-no-scene")?;
        let expected = scene_oracle_current_native_expected(&coordinator, viewport)?;
        scene_oracle_require(scene == expected, "withheld-native-fallback-mismatch")?;
        scene_oracle_require(
            scene
                .clone()
                .with_retained_session(coordinator.session())
                .is_ok(),
            "withheld-current-native-lost-authentication",
        )?;
        scene_oracle_require(
            scene.computed_input.is_none()
                && scene.computed_curves.is_empty()
                && scene.fillet_affordances.is_empty(),
            "withheld-scene-leaked-computed-geometry",
        )?;
        let problems = coordinator.computed_feature_problems();
        scene_oracle_require(
            coordinator.current_problem_metadata().is_none()
                && matches!(problems.as_slice(), [problem]
                    if problem.scope == EditorProblemScope::Global
                        && problem.message.contains("identity space is exhausted")),
            "withheld-global-problem-metadata-missing",
        )?;
        let markup = scene_oracle_markup(&coordinator, &scene, viewport);
        scene_oracle_require(
            markup.contains("data-scene-provenance=\"accepted\"")
                && markup.contains("data-computed-problems=\"1\"")
                && markup.contains("class=\"wb-error-marker computed global\"")
                && markup.contains("data-feature-id=\"global\""),
            "withheld-global-problem-not-visible",
        )
    }

    fn scene_oracle_current_computed_fillet() -> SceneOracleCheck {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let (expected_input, snapshot) = match coordinator.computed_scene_state() {
            ComputedSceneState::Current { expected, snapshot } => (*expected, snapshot),
            ComputedSceneState::Withheld | ComputedSceneState::Absent => {
                return Err("expected-current-computed-state");
            }
        };
        let source = coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session());
        let accepted = source
            .accepted_state_for_current_input()
            .ok_or("computed-current-accepted-state-missing")?;
        scene_oracle_require(
            accepted
                .document()
                .to_canonical_json()
                .map_err(|_| "accepted-json-failed")?
                == coordinator
                    .session()
                    .design_document()
                    .to_canonical_json()
                    .map_err(|_| "design-json-failed")?,
            "computed-accepted-design-geometry-diverged",
        )?;

        let viewport = super::scene::viewport();
        let scene = compose_editor_scene(&coordinator, viewport, 0.25)
            .ok_or("computed-compose-returned-no-scene")?;
        let accepted_input = source
            .accepted_prepared_input()
            .ok_or("computed-accepted-input-missing")?;
        let mut expected = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &accepted_input,
            &expected_input,
            snapshot,
            viewport,
            0.25,
        )
        .map_err(|_| "expected-composite-construction-failed")?;
        let mut action_items = coordinator.editor().selection().to_vec();
        action_items.push(SelectionItem::Feature(metadata.feature));
        action_items.sort_unstable();
        action_items.dedup();
        coordinator
            .populate_computed_fillet_affordances(&mut expected, &action_items, 0.25)
            .map_err(|_| "expected-composite-affordances-failed")?;
        let expected = expected
            .with_retained_session(source)
            .map_err(|_| "expected-composite-authentication-failed")?;
        scene_oracle_require(scene == expected, "computed-composite-provenance-mismatch")?;
        scene_oracle_require(
            scene.computed_input.as_ref() == Some(&metadata.input)
                && scene.computed_curves.len() == 2
                && scene.fillet_affordances.len() == 2
                && scene
                    .curves
                    .iter()
                    .any(|curve| matches!(curve.origin, SceneCurveOrigin::FilletDiscarded { .. })),
            "computed-fillet-geometry-incomplete",
        )?;
        scene_oracle_require(
            scene.clone().with_retained_session(source).is_ok(),
            "current-computed-scene-lost-authentication",
        )?;
        scene_oracle_require(
            coordinator.current_problem_metadata().is_none()
                && coordinator.computed_feature_problems().is_empty(),
            "clean-computed-scene-published-problem",
        )?;
        let markup = scene_oracle_markup(&coordinator, &scene, viewport);
        scene_oracle_require(
            markup.contains("data-scene-provenance=\"accepted\"")
                && markup.contains("class=\"wb-computed-geometry\"")
                && !markup.contains("data-problem-marker="),
            "computed-scene-markup-provenance",
        )
    }

    fn scene_oracle_rejected_historical_detached() -> SceneOracleCheck {
        let (coordinator, _, accepted_identity, accepted_json) = rejected_constraint_fixture();
        scene_oracle_require(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_none(),
            "rejected-state-unexpectedly-current",
        )?;
        let accepted = coordinator
            .session()
            .accepted_state()
            .ok_or("historical-accepted-state-missing")?;
        scene_oracle_require(
            accepted.identity() == accepted_identity
                && accepted
                    .document()
                    .to_canonical_json()
                    .map_err(|_| "accepted-json-failed")?
                    == accepted_json,
            "historical-accepted-provenance-changed",
        )?;
        scene_oracle_require(
            coordinator
                .session()
                .design_document()
                .to_canonical_json()
                .map_err(|_| "design-json-failed")?
                != accepted_json,
            "rejected-attempt-does-not-distinguish-geometry",
        )?;

        let viewport = super::scene::viewport();
        let scene = compose_editor_scene(&coordinator, viewport, 0.25)
            .ok_or("rejected-compose-returned-no-scene")?;
        let expected = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport,
            0.25,
        )
        .map_err(|_| "historical-scene-construction-failed")?;
        let attempted = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            coordinator.session().design_document(),
            coordinator.session().design_document(),
            viewport,
            0.25,
        )
        .map_err(|_| "attempted-scene-construction-failed")?;
        scene_oracle_require(scene == expected, "historical-accepted-scene-mismatch")?;
        scene_oracle_require(scene != attempted, "attempted-geometry-was-painted")?;
        scene_oracle_require(
            scene
                .clone()
                .with_retained_session(coordinator.session())
                .is_err(),
            "detached-historical-scene-gained-authentication",
        )?;
        let problem = coordinator
            .current_problem_metadata()
            .ok_or("rejected-problem-metadata-missing")?;
        scene_oracle_require(
            problem.attempt == coordinator.session().last_attempt().identity(),
            "rejected-problem-attempt-provenance-mismatch",
        )?;
        let markup = scene_oracle_markup(&coordinator, &scene, viewport);
        scene_oracle_require(
            markup.contains("data-scene-provenance=\"accepted\"")
                && markup.contains("data-problem-scope=\"")
                && markup.contains("data-problem-marker=\"")
                && markup.contains("wb-error-marker-icon"),
            "rejected-problem-not-visible-over-accepted-scene",
        )
    }

    fn run_scene_oracle_row(
        case_id: &'static str,
        check: fn() -> SceneOracleCheck,
    ) -> SceneOracleResult {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)) {
            Ok(Ok(())) => SceneOracleResult {
                case_id,
                status: "PASS",
                failure_class: "-",
                fingerprint: "ok",
            },
            Ok(Err(fingerprint)) => SceneOracleResult {
                case_id,
                status: "DEFECT",
                failure_class: "semantic-contract",
                fingerprint,
            },
            Err(_) => SceneOracleResult {
                case_id,
                status: "PANIC",
                failure_class: "unexpected-panic",
                fingerprint: "row-panicked",
            },
        }
    }

    fn render_scene_oracle_results(rows: &[SceneOracleResult]) -> String {
        use std::fmt::Write as _;

        let mut output =
            String::from("case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint\n");
        for row in rows {
            writeln!(
                output,
                "{}\tscene-authority\t{}\t-\t{}\t{}",
                row.case_id, row.status, row.failure_class, row.fingerprint,
            )
            .expect("writing a String cannot fail");
        }
        output
    }

    #[test]
    fn golden_scene_authority_oracle_survey() {
        let cases: [SceneOracleCase; 4] = [
            (
                "scene.current-computed.empty",
                scene_oracle_current_computed_empty,
            ),
            (
                "scene.current-native.withheld",
                scene_oracle_current_native_withheld,
            ),
            (
                "scene.current-computed.fillet",
                scene_oracle_current_computed_fillet,
            ),
            (
                "scene.rejected-historical.detached",
                scene_oracle_rejected_historical_detached,
            ),
        ];
        let selected = std::env::var("GEOSOLVE_GOLDEN_ORACLE_CASE").ok();
        let rows = cases
            .into_iter()
            .filter(|(case_id, _)| selected.as_deref().is_none_or(|value| value == *case_id))
            .map(|(case_id, check)| run_scene_oracle_row(case_id, check))
            .collect::<Vec<_>>();
        assert!(
            selected.is_none() || rows.len() == 1,
            "unknown golden scene-authority oracle case: {}",
            selected.as_deref().unwrap_or_default()
        );
        let output = render_scene_oracle_results(&rows);
        if let Some(path) = std::env::var_os("GEOSOLVE_GOLDEN_ORACLE_OUTPUT") {
            std::fs::write(&path, output.as_bytes()).unwrap_or_else(|error| {
                panic!(
                    "failed to write GEOSOLVE_GOLDEN_ORACLE_OUTPUT {}: {error}",
                    std::path::Path::new(&path).display()
                )
            });
        } else {
            println!("{output}");
            assert!(
                rows.iter().all(|row| row.status == "PASS"),
                "scene-authority oracle recorded one or more defects:\n{output}"
            );
        }
    }

    #[test]
    fn reproduction_load_validates_before_any_state_commit() {
        let mut state = String::from("retained workspace");
        let mut commit_called = false;
        let rejected: Result<(), &str> = apply_validated_reproduction(
            &mut state,
            || Err("corrupt capsule"),
            |state, replacement: String| {
                commit_called = true;
                *state = replacement;
                Ok(())
            },
        );
        assert_eq!(rejected, Err("corrupt capsule"));
        assert_eq!(state, "retained workspace");
        assert!(
            !commit_called,
            "invalid input must never enter the commit half"
        );

        apply_validated_reproduction(
            &mut state,
            || Ok::<_, &str>(String::from("validated replacement")),
            |state, replacement| {
                *state = replacement;
                Ok(())
            },
        )
        .expect("validated replacement");
        assert_eq!(state, "validated replacement");
    }

    #[test]
    fn reproduction_dialog_owns_keyboard_focus_and_exact_size_reporting() {
        assert!(should_route_stationary_draft_inference(false, true));
        assert!(
            !should_route_stationary_draft_inference(true, true),
            "the foreground payload dialog must isolate Shift from a live draft"
        );
        assert!(!should_route_stationary_draft_inference(false, false));

        assert_eq!(
            foreground_overlay_escape_owner(true, true),
            ForegroundOverlayEscapeOwner::Reproduction,
            "the payload dialog must own Escape ahead of the background Samples menu"
        );
        assert_eq!(
            foreground_overlay_escape_owner(false, true),
            ForegroundOverlayEscapeOwner::Samples
        );
        assert_eq!(
            foreground_overlay_escape_owner(false, false),
            ForegroundOverlayEscapeOwner::None
        );

        assert_eq!(
            reproduction_focus_target_after_action(
                "reproduction-close",
                false,
                ReproductionFocusReturn::Copy,
            ),
            Some("wb-reproduction-copy-trigger")
        );
        assert_eq!(
            reproduction_focus_target_after_action(
                "reproduction-close",
                false,
                ReproductionFocusReturn::Trace,
            ),
            Some("wb-interaction-trace-copy-trigger"),
            "closing copied trace evidence must restore its own invoker",
        );
        assert_eq!(
            reproduction_focus_target_after_action(
                "reproduction-load",
                false,
                ReproductionFocusReturn::Load,
            ),
            Some("wb-reproduction-load-trigger"),
            "a successful load must not leave focus inside the hidden dialog"
        );
        assert_eq!(
            reproduction_focus_target_after_action(
                "reproduction-load",
                true,
                ReproductionFocusReturn::Load,
            ),
            None,
            "a rejected load keeps the dialog and its current focus open"
        );
        assert_eq!(
            reproduction_payload_size_label(12_345),
            "12345 payload bytes"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one static UI ownership test covers payload and trace modes on their shared overlay"
    )]
    fn reproduction_controls_use_one_non_layout_shifting_canvas_overlay() {
        assert_eq!(reproduction_overlay_presentation(false), ("false", true));
        assert_eq!(reproduction_overlay_presentation(true), ("true", false));
        assert!(!ReproductionOverlayMode::Payload.is_trace());
        assert!(ReproductionOverlayMode::Trace.is_trace());
        assert_eq!(
            ReproductionFocusReturn::Copy.element_id(),
            "wb-reproduction-copy-trigger"
        );
        assert_eq!(
            ReproductionFocusReturn::Trace.element_id(),
            "wb-interaction-trace-copy-trigger"
        );
        assert_eq!(
            ReproductionFocusReturn::Load.element_id(),
            "wb-reproduction-load-trigger"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one table qualifies every production capture owner against every terminal route"
    )]
    fn canvas_pointer_capture_route_machine_has_exact_terminal_ownership() {
        use geosolve_constraint_editor::ActivePointerGestureKind;

        assert_eq!(
            CANVAS_POINTER_TERMINAL_EVENTS,
            ["pointerup", "pointercancel", "lostpointercapture"]
        );
        assert_eq!(
            CANVAS_PAN_POINTER_EVENTS,
            ["pointerdown", "pointermove", "pointerup"],
            "pointercancel has one centralized owner rather than a second pan listener"
        );
        assert_eq!(
            canvas_pointer_capture_kind(ActivePointerGestureKind::Point),
            CanvasPointerCaptureKind::Point
        );
        assert_eq!(
            canvas_pointer_capture_kind(ActivePointerGestureKind::CurveControl),
            CanvasPointerCaptureKind::CurveControl
        );
        assert_eq!(
            canvas_pointer_capture_kind(ActivePointerGestureKind::Annotation),
            CanvasPointerCaptureKind::Annotation
        );
        for fillet_kind in [
            ActivePointerGestureKind::FilletRadius,
            ActivePointerGestureKind::FilletContact,
        ] {
            assert_eq!(
                canvas_pointer_capture_kind(fillet_kind),
                CanvasPointerCaptureKind::Fillet,
                "radius and higher-priority contact overlap routes share exact Fillet capture"
            );
        }
        assert_eq!(
            canvas_pointer_capture_kind(ActivePointerGestureKind::OffsetDistance),
            CanvasPointerCaptureKind::OffsetDistance,
        );

        let terminals = [
            (
                CanvasPointerTerminal::PointerUp { pointer_id: 11 },
                CanvasPointerTerminalDisposition::Complete,
                true,
            ),
            (
                CanvasPointerTerminal::PointerCancel { pointer_id: 11 },
                CanvasPointerTerminalDisposition::Cancel,
                true,
            ),
            (
                CanvasPointerTerminal::LostPointerCapture { pointer_id: 11 },
                CanvasPointerTerminalDisposition::Cancel,
                false,
            ),
            (
                CanvasPointerTerminal::InteractionCancel,
                CanvasPointerTerminalDisposition::Cancel,
                true,
            ),
            (
                CanvasPointerTerminal::CameraCancel,
                CanvasPointerTerminalDisposition::Cancel,
                true,
            ),
            (
                CanvasPointerTerminal::GeometryPolicyCancel,
                CanvasPointerTerminalDisposition::Cancel,
                true,
            ),
        ];
        for kind in [
            CanvasPointerCaptureKind::Point,
            CanvasPointerCaptureKind::CurveControl,
            CanvasPointerCaptureKind::Annotation,
            CanvasPointerCaptureKind::Fillet,
            CanvasPointerCaptureKind::OffsetDistance,
            CanvasPointerCaptureKind::Pan,
        ] {
            for (terminal, disposition, release_platform_capture) in terminals {
                let captured = CapturedCanvasPointer {
                    pointer_id: 11,
                    kind,
                };
                let mut route_machine = CanvasPointerCaptures::default();
                assert_eq!(
                    route_machine.ownership(11),
                    CanvasPointerOwnership::Uncaptured,
                    "uncaptured pointercancel remains available to cancel editor drafts"
                );
                assert!(route_machine.begin(captured));
                assert_eq!(route_machine.ownership(11), CanvasPointerOwnership::Owned);
                assert_eq!(route_machine.ownership(12), CanvasPointerOwnership::Foreign);
                assert!(
                    !route_machine.begin(CapturedCanvasPointer {
                        pointer_id: 12,
                        kind,
                    }),
                    "a foreign pointer cannot steal {kind:?} capture"
                );
                assert_eq!(
                    route_machine
                        .route_terminal(CanvasPointerTerminal::PointerCancel { pointer_id: 12 }),
                    None,
                    "a foreign terminal cannot release {kind:?} capture"
                );
                assert!(route_machine.contains(11));

                let route = route_machine
                    .route_terminal(terminal)
                    .expect("the owning terminal must route exactly once");
                assert_eq!(route.captured, captured);
                assert_eq!(route.disposition, disposition);
                assert_eq!(
                    route.release_platform_capture, release_platform_capture,
                    "lostpointercapture is already released by the browser"
                );
                assert!(route_machine.is_empty(), "{kind:?} capture must not strand");
                assert_eq!(
                    route_machine.route_terminal(terminal),
                    None,
                    "a repeated terminal cannot release {kind:?} twice"
                );
            }
        }

        let mut captures = CanvasPointerCaptures::default();
        assert!(!captures.begin(CapturedCanvasPointer {
            pointer_id: -1,
            kind: CanvasPointerCaptureKind::Point,
        }));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one inspector regression keeps the closed twenty-constraint/seven-dimension semantic name catalog exhaustive"
    )]
    fn annotation_inspector_uses_scene_semantics_and_names_every_family() {
        let cases = [
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Fixed),
                "Fixed constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Coincident),
                "Coincident constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Horizontal),
                "Horizontal constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Vertical),
                "Vertical constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::PointOnCurve),
                "Point-on-curve constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Parallel),
                "Parallel constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Perpendicular),
                "Perpendicular constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Concentric),
                "Concentric constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Collinear),
                "Collinear constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::EqualLength),
                "Equal-length constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::EqualRadius),
                "Equal-radius constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Midpoint),
                "Midpoint constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Symmetry),
                "Symmetry constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Contact),
                "Curve-contact constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Tangency),
                "Tangency constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Direction),
                "Tangent-direction constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Normal),
                "Normal-direction constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::EqualCurvature),
                "Equal-curvature constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Continuity),
                "Endpoint-continuity constraint",
            ),
            (
                SceneAnnotationKind::Constraint(SceneConstraintGlyph::Fillet),
                "Fillet constraint",
            ),
            (
                SceneAnnotationKind::PointDistance,
                "Point-distance dimension",
            ),
            (SceneAnnotationKind::CurveLength, "Curve-length dimension"),
            (SceneAnnotationKind::Radius, "Radius dimension"),
            (SceneAnnotationKind::Diameter, "Diameter dimension"),
            (
                SceneAnnotationKind::OrientedAngle,
                "Oriented-angle dimension",
            ),
            (
                SceneAnnotationKind::SupportingLineOffset,
                "Supporting-line offset dimension",
            ),
            (
                SceneAnnotationKind::ExactTranslatedSegmentOffset,
                "Exact translated-segment offset dimension",
            ),
            (
                SceneAnnotationKind::ProfileOffset,
                "Profile offset dimension",
            ),
        ];
        assert_eq!(cases.len(), 28);
        for (kind, expected) in cases {
            assert_eq!(annotation_family_name(kind), expected);
        }

        let mut document = SketchDocument::new(8.0).expect("inspector document");
        let start = document
            .add_point("inspector start", [0.0, 0.0])
            .expect("start");
        let end = document
            .add_point("inspector end", [4.0, 0.0])
            .expect("end");
        let line = document
            .add_curve(
                "inspector line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        let constraint = document
            .add_constraint(
                "Workbench horizontal",
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(line),
                },
            )
            .expect("horizontal constraint");
        let target = document
            .add_scalar(
                "Workbench distance target",
                4.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("distance target");
        let dimension = document
            .add_dimension(
                "Workbench endpoint distance",
                DocumentDimensionDefinition::PointDistance {
                    first: start,
                    second: end,
                    target,
                },
                DocumentDimensionMode::Reference,
            )
            .expect("point-distance dimension");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("inspector session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("inspector coordinator");
        let scene = compose_editor_scene(
            &coordinator,
            Viewport::new([800.0, 600.0], [2.0, 0.0], 50.0).expect("inspector viewport"),
            0.25,
        )
        .expect("inspector scene");

        let constraint_item = SelectionItem::Constraint(constraint);
        let constraint = annotation_inspector_presentation(Some(&scene), &[constraint_item])
            .expect("constraint inspector presentation");
        assert_eq!(constraint.family, "Horizontal constraint");
        assert!(constraint.detail.contains("Workbench horizontal"));
        assert!(constraint.detail.contains("horizontal constraint"));
        assert_eq!(constraint.meta, "Constraint · 1 direct operand");

        let dimension_item = SelectionItem::Dimension(dimension);
        let dimension = annotation_inspector_presentation(Some(&scene), &[dimension_item])
            .expect("dimension inspector presentation");
        assert_eq!(dimension.family, "Point-distance dimension");
        assert!(dimension.detail.contains("Workbench endpoint distance"));
        assert!(dimension.detail.contains("point-distance dimension"));
        assert_eq!(dimension.meta, "Reference dimension · Canvas value (4)");
        assert!(
            annotation_inspector_presentation(Some(&scene), &[constraint_item, dimension_item],)
                .is_none(),
            "multi-selection has no single semantic annotation owner",
        );
    }

    #[test]
    fn history_shortcuts_are_platform_complete_without_claiming_editable_ownership() {
        for (key, control, command, shift, expected) in [
            ("z", true, false, false, Some(HistoryShortcut::Undo)),
            ("Z", false, true, false, Some(HistoryShortcut::Undo)),
            ("z", true, false, true, Some(HistoryShortcut::Redo)),
            ("Z", false, true, true, Some(HistoryShortcut::Redo)),
            ("y", true, false, false, Some(HistoryShortcut::Redo)),
            ("y", false, true, false, None),
            ("y", true, false, true, None),
            ("z", false, false, false, None),
            ("z", true, true, false, None),
        ] {
            assert_eq!(
                history_shortcut(
                    key,
                    Modifiers {
                        shift,
                        control,
                        command,
                    },
                    false,
                ),
                expected,
                "shortcut route for {key} ctrl={control} cmd={command} shift={shift}"
            );
        }
        assert_eq!(
            history_shortcut(
                "z",
                Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
                true,
            ),
            None
        );
    }

    #[test]
    fn coordinate_hud_prefers_the_authenticated_adjusted_inference_sample() {
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).unwrap();
        let pointer = PointerInput {
            pointer_id: 7,
            position: ScreenPoint { x: 510.0, y: 340.0 },
            modifiers: Modifiers::default(),
        };
        let inference = DraftInferenceResolution {
            status: DraftInferenceStatus::None,
            completeness: DraftInferenceCompleteness::Complete,
            raw_model_position: [0.2, 0.2],
            adjusted_model_position: [0.0, 0.0],
            raw_screen_position: pointer.position,
            adjusted_screen_position: ScreenPoint { x: 500.0, y: 350.0 },
            candidates: Vec::new(),
            guides: Vec::new(),
        };
        let hud = coordinate_hud(viewport, Some(pointer), Some(&inference));
        assert_eq!(hud.text, "X 0.000 · Y 0.000");
        assert!(hud.adjusted);
        assert!(hud.title.contains("raw X 0.200, Y 0.200"));

        let stale = PointerInput {
            position: ScreenPoint { x: 550.0, y: 300.0 },
            ..pointer
        };
        let raw = coordinate_hud(viewport, Some(stale), Some(&inference));
        assert_eq!(raw.text, "X 1.000 · Y 1.000");
        assert!(!raw.adjusted);
        assert_eq!(coordinate_hud(viewport, None, None).text, "X — · Y —");
    }

    fn m77_rational_coordinator(weight: f64) -> (RetainedEditorCoordinator, CurveSpan) {
        let mut document = SketchDocument::new(4.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("start");
        let end = document.add_point("end", [4.0, 0.0]).expect("end");
        let middle_weight = document
            .add_scalar(
                "weight",
                weight,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                    upper: f64::MAX,
                },
            )
            .expect("weight");
        let curve = CurveSpan::line(
            document
                .add_curve(
                    "rational demo",
                    CurveDefinition::RationalQuadraticConic {
                        start,
                        weighted_middle: [weight * 2.0, weight * 3.0],
                        middle_weight,
                        end,
                    },
                )
                .expect("rational curve"),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        (
            RetainedEditorCoordinator::new(session).expect("coordinator"),
            curve,
        )
    }

    #[test]
    fn m77_demo_composition_and_inspector_consume_selected_headless_curve_metadata() {
        let (mut coordinator, curve) = m77_rational_coordinator(0.5);
        let viewport = Viewport::new([1000.0, 700.0], [2.0, 1.0], 80.0).unwrap();
        assert!(
            compose_editor_scene(&coordinator, viewport, 0.25)
                .unwrap()
                .curve_controls
                .is_empty(),
            "an unselected curve must not acquire browser-created controls",
        );

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(curve)]);
        let scene = compose_editor_scene(&coordinator, viewport, 0.25).unwrap();
        assert!(!scene.curve_controls.is_empty());
        assert!(!scene.curve_control_guides.is_empty());
        let middle = scene
            .curve_controls
            .iter()
            .find(|control| {
                control.id.kind == geosolve_sketch::DocumentCurveControlKind::RationalMiddle
            })
            .expect("headless rational middle control");
        let hover = EditorHoverState {
            target: Some(EditorHoverTarget::CurveControl {
                control: middle.id,
                owner: middle.owner,
            }),
            context_owner: Some(SelectionItem::Curve(middle.owner)),
        };
        assert_eq!(
            canvas_cursor_key_with_curve_control(
                EditorTool::Select,
                false,
                false,
                false,
                false,
                hover,
                None,
            ),
            "curve-control",
        );
        assert_eq!(
            canvas_cursor_key_with_curve_control(
                EditorTool::Select,
                false,
                false,
                false,
                false,
                EditorHoverState::default(),
                Some(ActivePointerGesture {
                    pointer_id: 7,
                    kind: ActivePointerGestureKind::CurveControl,
                }),
            ),
            "curve-control-active",
        );
        assert_eq!(
            canvas_cursor_key_with_curve_control(
                EditorTool::Select,
                false,
                false,
                false,
                true,
                hover,
                Some(ActivePointerGesture {
                    pointer_id: 7,
                    kind: ActivePointerGestureKind::CurveControl,
                }),
            ),
            "pan",
            "camera ownership must outrank any stale curve-control presentation",
        );

        let metadata = coordinator
            .selected_curve_property_metadata()
            .expect("selected curve metadata");
        let markup = curve_control_inspector_markup(&metadata);
        assert!(markup.contains("<legend>Middle control P1</legend>"));
        assert!(markup.contains("value=\"2\""));
        assert!(markup.contains("value=\"3\""));
        assert!(markup.contains("data-wb-action=\"curve-rational-middle\""));
        assert!(markup.contains("data-wb-action=\"curve-property-rational-weight\""));
        assert!(curve_control_inspector_detail(&metadata).contains("ordinary middle control P1"));

        coordinator.editor_mut().activate_tool(EditorTool::Line);
        assert!(
            compose_editor_scene(&coordinator, viewport, 0.25)
                .unwrap()
                .curve_controls
                .is_empty(),
            "non-Select tools must revoke the selected-curve handle layer",
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one browser-adapter regression compares the complete point-alias preview transaction across both arc families"
    )]
    fn m77_f012_arc_point_alias_preview_remains_visible_for_both_families() {
        for (label, elliptical, move_major_axis) in [
            ("circular centre", false, false),
            ("elliptical centre", true, false),
            ("elliptical major axis", true, true),
        ] {
            let mut document = SketchDocument::new(10.0).expect("document");
            let center = document.add_point("centre", [0.0, 0.0]).unwrap();
            let radius = document
                .add_scalar("radius", 3.0, ScalarUnit::Length, ScalarDomain::Positive)
                .unwrap();
            let start = document
                .add_scalar("start", -0.5, ScalarUnit::Angle, ScalarDomain::Finite)
                .unwrap();
            let end = document
                .add_scalar("end", 1.25, ScalarUnit::Angle, ScalarDomain::Finite)
                .unwrap();
            let (curve, point) = if elliptical {
                let major_axis = document.add_point("major axis", [3.0, 0.0]).unwrap();
                let ratio = document
                    .add_scalar(
                        "minor ratio",
                        0.5,
                        ScalarUnit::Parameter,
                        ScalarDomain::Bounded {
                            lower: f64::from_bits(1),
                            upper: 1.0,
                        },
                    )
                    .unwrap();
                let curve = document
                    .add_curve(
                        "elliptical arc",
                        CurveDefinition::EllipticalArc {
                            center,
                            major_axis_point: major_axis,
                            minor_axis_ratio: ratio,
                            start_angle: start,
                            end_angle: end,
                            sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
                        },
                    )
                    .unwrap();
                (curve, if move_major_axis { major_axis } else { center })
            } else {
                let curve = document
                    .add_curve(
                        "circular arc",
                        CurveDefinition::CircularArc {
                            center,
                            radius,
                            start_angle: start,
                            end_angle: end,
                            sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
                        },
                    )
                    .unwrap();
                (curve, center)
            };
            let session = RetainedSketchDocumentSession::new(
                document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap();
            let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
            let owner = CurveSpan::line(curve);
            coordinator
                .editor_mut()
                .set_selection([SelectionItem::Curve(owner)]);
            let viewport = Viewport::new([1000.0, 700.0], [1.5, 0.5], 80.0).unwrap();
            let scene = compose_editor_scene(&coordinator, viewport, 0.25)
                .unwrap_or_else(|| panic!("{label}: initial scene"));
            let control = scene
                .curve_controls
                .iter()
                .find(|control| {
                    matches!(
                        control.interaction,
                        geosolve_constraint_editor::SceneCurveControlInteraction::PointAlias(
                            candidate
                        ) if candidate == point
                    )
                })
                .unwrap_or_else(|| panic!("{label}: point alias"));
            let pointer = |position| PointerInput {
                pointer_id: 77,
                position,
                modifiers: Modifiers::default(),
            };
            assert!(
                coordinator
                    .pointer_down(&scene, pointer(control.screen_position))
                    .is_empty(),
                "{label}: pointer down"
            );
            let before = coordinator
                .session()
                .design_document()
                .point(point)
                .unwrap()
                .position;
            let target = [before[0] + 0.75, before[1] + 0.5];
            let target_screen = viewport.model_to_screen(target);
            let request = coordinator
                .editor_mut()
                .pointer_move(&scene, pointer(target_screen));
            let [
                geosolve_constraint_editor::EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point: requested_point,
                    model_position,
                },
            ] = request.as_slice()
            else {
                panic!("{label}: projected point request: {request:?}")
            };
            assert_eq!(*requested_point, point, "{label}: request owner");
            let acknowledgement = coordinator.resolve_projected_point_move(
                *pointer_id,
                *request_id,
                *requested_point,
                *model_position,
            );
            assert!(
                matches!(
                    acknowledgement.as_slice(),
                    [geosolve_constraint_editor::EditorEffect::PreviewPointMove {
                        point: previewed,
                        ..
                    }] if *previewed == point
                ),
                "{label}: preview acknowledgement: {acknowledgement:?}"
            );

            let preview_scene = compose_editor_scene(&coordinator, viewport, 0.25)
                .unwrap_or_else(|| panic!("{label}: accepted preview must remain renderable"));
            let previewed = preview_scene
                .points
                .iter()
                .find(|candidate| candidate.id == point)
                .unwrap_or_else(|| panic!("{label}: preview point"));
            assert_eq!(
                previewed.model_position.map(f64::to_bits),
                target.map(f64::to_bits),
                "{label}: visible preview"
            );

            let expected = coordinator.session().design_identity();
            let release = coordinator.editor_mut().pointer_up(
                &preview_scene,
                expected,
                pointer(target_screen),
            );
            let [
                effect @ geosolve_constraint_editor::EditorEffect::CommitPointMove {
                    point: committed,
                    ..
                },
            ] = release.as_slice()
            else {
                panic!("{label}: commit effect: {release:?}")
            };
            assert_eq!(*committed, point, "{label}: commit owner");
            coordinator
                .apply_editor_effect(effect)
                .unwrap_or_else(|error| panic!("{label}: commit failed: {error}"));
            assert_eq!(
                coordinator
                    .session()
                    .design_document()
                    .point(point)
                    .unwrap()
                    .position
                    .map(f64::to_bits),
                target.map(f64::to_bits),
                "{label}: durable position"
            );
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one browser-adapter regression compares the complete direct-trim preview transaction across both arc families"
    )]
    fn m77_f012_arc_direct_trim_preview_stays_visible_and_commits_for_both_families() {
        for (label, elliptical) in [("circular trim", false), ("elliptical trim", true)] {
            let mut document = SketchDocument::new(10.0).expect("document");
            let center = document.add_point("centre", [0.0, 0.0]).unwrap();
            let radius = document
                .add_scalar("radius", 3.0, ScalarUnit::Length, ScalarDomain::Positive)
                .unwrap();
            let start = document
                .add_scalar("start", -0.5, ScalarUnit::Angle, ScalarDomain::Finite)
                .unwrap();
            let end = document
                .add_scalar("end", 1.25, ScalarUnit::Angle, ScalarDomain::Finite)
                .unwrap();
            let curve = if elliptical {
                let major_axis = document.add_point("major axis", [3.0, 0.0]).unwrap();
                let ratio = document
                    .add_scalar(
                        "minor ratio",
                        0.5,
                        ScalarUnit::Parameter,
                        ScalarDomain::Bounded {
                            lower: f64::from_bits(1),
                            upper: 1.0,
                        },
                    )
                    .unwrap();
                document
                    .add_curve(
                        "elliptical arc",
                        CurveDefinition::EllipticalArc {
                            center,
                            major_axis_point: major_axis,
                            minor_axis_ratio: ratio,
                            start_angle: start,
                            end_angle: end,
                            sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
                        },
                    )
                    .unwrap()
            } else {
                document
                    .add_curve(
                        "circular arc",
                        CurveDefinition::CircularArc {
                            center,
                            radius,
                            start_angle: start,
                            end_angle: end,
                            sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
                        },
                    )
                    .unwrap()
            };
            let session = RetainedSketchDocumentSession::new(
                document,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap();
            let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
            let owner = CurveSpan::line(curve);
            coordinator
                .editor_mut()
                .set_selection([SelectionItem::Curve(owner)]);
            let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 80.0).unwrap();
            let scene = compose_editor_scene(&coordinator, viewport, 0.25)
                .unwrap_or_else(|| panic!("{label}: initial scene"));
            let control = scene
                .curve_controls
                .iter()
                .find(|control| {
                    control.id.kind == geosolve_sketch::DocumentCurveControlKind::TrimStart
                })
                .unwrap_or_else(|| panic!("{label}: start control"));
            let history_before = coordinator.history_len();
            let cursor_before = coordinator.history_cursor();
            let pointer = |position| PointerInput {
                pointer_id: 78,
                position,
                modifiers: Modifiers::default(),
            };
            assert!(
                coordinator
                    .pointer_down(&scene, pointer(control.screen_position))
                    .is_empty(),
                "{label}: pointer down"
            );
            let target_model = if elliptical {
                [3.0 * 0.5f64.cos(), 1.5 * 0.5f64.sin()]
            } else {
                [3.0 * 0.5f64.cos(), 3.0 * 0.5f64.sin()]
            };
            let target_screen = viewport.model_to_screen(target_model);
            let request = coordinator
                .editor_mut()
                .pointer_move(&scene, pointer(target_screen));
            let [
                geosolve_constraint_editor::EditorEffect::RequestCurveControlPreview {
                    pointer_id,
                    request_id,
                    expected,
                    control: requested_control,
                    model_position,
                },
            ] = request.as_slice()
            else {
                panic!("{label}: curve-control request: {request:?}")
            };
            let acknowledgement = coordinator.resolve_curve_control_preview(
                *pointer_id,
                *request_id,
                *expected,
                *requested_control,
                *model_position,
            );
            assert!(
                matches!(
                    acknowledgement.as_slice(),
                    [geosolve_constraint_editor::EditorEffect::PreviewCurveControl {
                        control: previewed,
                        ..
                    }] if *previewed == control.id
                ),
                "{label}: preview acknowledgement: {acknowledgement:?}"
            );
            let preview_scene = compose_editor_scene(&coordinator, viewport, 0.25)
                .unwrap_or_else(|| panic!("{label}: accepted preview must remain renderable"));
            let preview_control = preview_scene
                .curve_controls
                .iter()
                .find(|candidate| candidate.id == control.id)
                .unwrap_or_else(|| panic!("{label}: preview control"));
            assert_ne!(
                preview_control.model_position.map(f64::to_bits),
                control.model_position.map(f64::to_bits),
                "{label}: visible preview"
            );
            let expected = coordinator.session().design_identity();
            let release = coordinator.editor_mut().pointer_up(
                &preview_scene,
                expected,
                pointer(target_screen),
            );
            let [effect @ geosolve_constraint_editor::EditorEffect::CommitCurveControl { .. }] =
                release.as_slice()
            else {
                panic!("{label}: commit effect: {release:?}")
            };
            coordinator
                .apply_editor_effect(effect)
                .unwrap_or_else(|error| panic!("{label}: commit failed: {error}"));
            assert_eq!(
                coordinator.history_len(),
                history_before + 1,
                "{label}: one durable history row"
            );
            assert_eq!(
                coordinator.history_cursor(),
                cursor_before + 1,
                "{label}: one durable history step"
            );
            let (geosolve_sketch::CurveDefinition::CircularArc { start_angle, .. }
            | geosolve_sketch::CurveDefinition::EllipticalArc { start_angle, .. }) = &coordinator
                .session()
                .design_document()
                .curve(curve)
                .unwrap()
                .definition
            else {
                panic!("{label}: arc family changed")
            };
            assert!(
                (coordinator
                    .session()
                    .design_document()
                    .scalar(*start_angle)
                    .unwrap()
                    .value
                    - 0.5)
                    .abs()
                    < 1.0e-12
            );
        }
    }

    #[test]
    fn m77_inspector_disables_every_withheld_property_action_with_a_reason() {
        let (mut coordinator, curve) = m77_rational_coordinator(0.5);
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(curve)]);
        let mut metadata = coordinator
            .selected_curve_property_metadata()
            .expect("selected curve metadata");
        metadata.direct_edit_availability =
            geosolve_sketch::DocumentCurveControlAvailability::ReadOnly(
                geosolve_sketch::DocumentCurveControlWithholdingReason::AssociativeFilletOutput,
            );
        metadata.numeric[0].availability =
            geosolve_sketch::DocumentCurveControlAvailability::ReadOnly(
                geosolve_sketch::DocumentCurveControlWithholdingReason::HostParameterOwned,
            );
        metadata.sweep = Some(geosolve_sketch::DocumentArcSweep::CounterClockwise);
        metadata.hyperbola_branch = Some(geosolve_sketch::DocumentHyperbolaBranch::Positive);

        let markup = curve_control_inspector_markup(&metadata);
        assert!(markup.contains(
            "data-curve-properties-read-only>Read-only: the associative Fillet owns this output."
        ));
        assert!(markup.contains(
            "id=\"wb-curve-rational-middle-x\" type=\"number\" step=\"any\" value=\"2\" disabled aria-disabled=\"true\""
        ));
        assert!(!markup.contains("data-wb-action=\"curve-rational-middle\""));
        assert!(markup.contains("Read-only: the value is owned by a host parameter."));
        assert!(!markup.contains("data-wb-action=\"curve-property-rational-weight\""));
        assert!(markup.contains("id=\"wb-curve-sweep\" disabled aria-disabled=\"true\""));
        assert!(!markup.contains("data-wb-action=\"curve-sweep\""));
        assert!(
            markup.contains("id=\"wb-curve-hyperbola-branch\" disabled aria-disabled=\"true\"")
        );
        assert!(!markup.contains("data-wb-action=\"curve-hyperbola-branch\""));

        metadata.direct_edit_availability =
            geosolve_sketch::DocumentCurveControlAvailability::Editable;
        for (reason, copy) in [
            (
                geosolve_sketch::DocumentCurveControlWithholdingReason::DrivingDimensionOwned,
                "an active driving radius or diameter dimension owns this size",
            ),
            (
                geosolve_sketch::DocumentCurveControlWithholdingReason::EqualRadiusOwned,
                "an active equal-radius relation owns this size",
            ),
        ] {
            metadata.numeric[0].availability =
                geosolve_sketch::DocumentCurveControlAvailability::ReadOnly(reason);
            let markup = curve_control_inspector_markup(&metadata);
            assert!(markup.contains(copy));
            assert!(!markup.contains("data-wb-action=\"curve-property-rational-weight\""));
            assert!(markup.contains("disabled aria-disabled=\"true\""));
        }
    }

    #[test]
    fn m77_nurbs_inspector_keeps_the_gauge_read_only_and_round_trips_numbers() {
        let mut document = SketchDocument::new(3.0).unwrap();
        let controls = [[0.0, 0.0], [3.0, 0.0]]
            .map(|position| document.add_point("control", position).unwrap());
        let weights = [1.0, 0.300_000_000_000_000_04].map(|value| {
            document
                .add_scalar(
                    "weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        });
        let curve = document
            .add_curve(
                "gauge-aware NURBS",
                CurveDefinition::Nurbs {
                    form: DocumentBSplineForm::Clamped,
                    degree: 1,
                    controls: controls.to_vec(),
                    weights: weights.to_vec(),
                    gauge_weight: weights[0],
                    knots: vec![0.0, 0.0, 1.0, 1.0],
                    span_ids: vec![4],
                    next_span_id: 5,
                },
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan { curve, segment: 4 })]);
        let metadata = coordinator.selected_curve_property_metadata().unwrap();
        let markup = curve_control_inspector_markup(&metadata);
        assert!(markup.contains("<span>Degree</span><output>1</output>"));
        assert!(markup.contains("id=\"wb-curve-property-nurbs-weight-0\""));
        assert!(markup.contains("disabled aria-disabled=\"true\""));
        assert!(markup.contains("Active gauge"));
        assert!(!markup.contains("data-wb-action=\"curve-nurbs-gauge-0\""));
        assert!(markup.contains("data-wb-action=\"curve-nurbs-gauge-1\""));
        assert!(markup.contains("value=\"0.30000000000000004\""));
    }

    #[test]
    fn canvas_pointer_down_preserves_every_existing_capture() {
        let empty = CanvasPointerCaptures::default();
        assert_eq!(
            route_canvas_pan_pointer_down(&empty),
            CanvasPanPointerDownRoute::BeginPan
        );
        assert_eq!(
            route_canvas_primary_pointer_down(&empty),
            CanvasPrimaryPointerDownRoute::Dispatch
        );

        for kind in [
            CanvasPointerCaptureKind::Point,
            CanvasPointerCaptureKind::CurveControl,
            CanvasPointerCaptureKind::Annotation,
            CanvasPointerCaptureKind::Fillet,
            CanvasPointerCaptureKind::OffsetDistance,
            CanvasPointerCaptureKind::Pan,
        ] {
            let mut route_machine = CanvasPointerCaptures::default();
            assert!(route_machine.begin(CapturedCanvasPointer {
                pointer_id: 11,
                kind,
            }));
            assert_eq!(
                route_canvas_pan_pointer_down(&route_machine),
                CanvasPanPointerDownRoute::PreserveCapturedInteraction,
                "foreign middle-button pointerdown must not steal {kind:?} capture"
            );
            assert_eq!(
                route_canvas_primary_pointer_down(&route_machine),
                CanvasPrimaryPointerDownRoute::PreserveCapturedInteraction,
                "a second primary pointerdown must not reach selection or authoring for {kind:?}",
            );
            assert!(route_machine.contains(11));
            assert_eq!(route_machine.ownership(12), CanvasPointerOwnership::Foreign);
        }
    }

    #[test]
    fn fillet_local_action_dom_keys_round_trip_semantic_normal_sides() {
        use geosolve_constraint_editor::SceneFilletActionId;

        for first in [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] {
            for second in [
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Right,
            ] {
                let action = SceneFilletActionId::LocalAlternative { first, second };
                let key = super::scene::fillet_action_key(action);
                assert_eq!(super::scene::fillet_action_from_key(&key), Some(action));
                assert!(key.contains(if first == DocumentCurveNormalSide::Left {
                    "left"
                } else {
                    "right"
                }));
            }
        }
        assert_eq!(
            super::scene::fillet_action_from_key("local-alternative-0"),
            None,
            "DOM identity must not regress to visible-list ordinals"
        );
    }

    fn grouped_fillet_fixture() -> (
        RetainedEditorCoordinator,
        [CurveSpan; 3],
        [DesignPointId; 4],
    ) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [6.0, 3.0]]
            .map(|position| document.add_point("corner point", position).expect("point"));
        let curve = document
            .add_curve(
                "corner support",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
                },
            )
            .expect("polyline");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        (
            RetainedEditorCoordinator::new(session).expect("coordinator"),
            [
                CurveSpan { curve, segment: 0 },
                CurveSpan { curve, segment: 1 },
                CurveSpan { curve, segment: 2 },
            ],
            points,
        )
    }

    fn prepare_grouped_fillet(
        coordinator: &mut RetainedEditorCoordinator,
        state: &mut FeatureAuthoringState,
        corners: [DesignPointId; 2],
    ) -> (FeatureAuthoringCandidate, FeatureAuthoringPreviewMetadata) {
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current feature-authoring snapshot");
        let document = snapshot.sketch_document().clone();
        let mut picks = Vec::new();
        for corner in corners {
            picks.extend(
                coordinator
                    .feature_authoring_picks_for_item(SelectionItem::Point(corner), None)
                    .expect("expanded native corner picks"),
            );
        }
        let _ = state.activate(&snapshot, &document, FeatureAuthoringTool::Fillet, &[]);
        assert!(matches!(
            state.set_options(
                &snapshot,
                FeatureAuthoringOptions {
                    fillet_radius: Some(0.5),
                    ..FeatureAuthoringOptions::default()
                },
            ),
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        let outcome = state.pick_many(&snapshot, picks);
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate,
            guidance,
        } = outcome
        else {
            panic!("two complete corners should request one grouped preview: {outcome:?}");
        };
        assert_eq!(guidance.completed_corners, 2);
        let metadata = coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "Grouped Fillet",
            )
            .expect("prepare grouped computed preview");
        (candidate, metadata)
    }

    fn native_line_fillet_fixture() -> (
        RetainedEditorCoordinator,
        FeatureAuthoringState,
        FeatureAuthoringCandidate,
        FeatureAuthoringPreviewMetadata,
        [CurveSpan; 2],
        DesignPointId,
    ) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let start = document
            .add_point("horizontal start", [0.0, 0.0])
            .expect("start");
        let corner = document
            .add_point("sharp corner", [3.0, 0.0])
            .expect("corner");
        let end = document.add_point("vertical end", [3.0, 3.0]).expect("end");
        let lines = [
            CurveSpan::line(
                document
                    .add_curve(
                        "horizontal parent",
                        CurveDefinition::Line {
                            start,
                            end: corner,
                            branch_direction: [1.0, 0.0],
                        },
                    )
                    .expect("horizontal line"),
            ),
            CurveSpan::line(
                document
                    .add_curve(
                        "vertical parent",
                        CurveDefinition::Line {
                            start: corner,
                            end,
                            branch_direction: [0.0, 1.0],
                        },
                    )
                    .expect("vertical line"),
            ),
        ];
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted line corner");
        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("line-corner coordinator");
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current feature-authoring snapshot");
        let accepted_document = snapshot.sketch_document().clone();
        let picks = coordinator
            .feature_authoring_picks_for_item(SelectionItem::Point(corner), None)
            .expect("standalone line-line corner picks");
        let mut state = FeatureAuthoringState::default();
        let _ = state.activate(
            &snapshot,
            &accepted_document,
            FeatureAuthoringTool::Fillet,
            &[],
        );
        assert!(matches!(
            state.set_options(
                &snapshot,
                FeatureAuthoringOptions {
                    fillet_radius: Some(0.5),
                    ..FeatureAuthoringOptions::default()
                },
            ),
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate,
            guidance,
        } = state.pick_many(&snapshot, picks)
        else {
            panic!("one standalone line-line corner should request a preview");
        };
        assert_eq!(guidance.completed_corners, 1);
        let metadata = coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "Native profile preview",
            )
            .expect("exact held line-line preview");
        (coordinator, state, candidate, metadata, lines, corner)
    }

    fn high_valence_native_line_fillet_fixture()
    -> (RetainedEditorCoordinator, FeatureAuthoringCandidate) {
        let (base, _, _, _, lines, corner) = native_line_fillet_fixture();
        let mut document = base.session().design_document().clone();
        let branch_end = document
            .add_point("branch end", [1.5, 1.5])
            .expect("branch endpoint");
        document
            .add_curve(
                "third corner owner",
                CurveDefinition::Line {
                    start: corner,
                    end: branch_end,
                    branch_direction: [
                        -std::f64::consts::FRAC_1_SQRT_2,
                        std::f64::consts::FRAC_1_SQRT_2,
                    ],
                },
            )
            .expect("third incident line");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted high-valence corner");
        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("high-valence coordinator");
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current feature-authoring snapshot");
        let accepted_document = snapshot.sketch_document().clone();
        let picks = [(lines[0], 0.75), (lines[1], 0.25)]
            .into_iter()
            .flat_map(|(line, parameter)| {
                coordinator
                    .feature_authoring_picks_for_item(SelectionItem::Curve(line), Some(parameter))
                    .expect("explicit selected-line Fillet pick")
            })
            .collect::<Vec<_>>();
        let mut state = FeatureAuthoringState::default();
        let _ = state.activate(
            &snapshot,
            &accepted_document,
            FeatureAuthoringTool::Fillet,
            &[],
        );
        assert!(matches!(
            state.set_options(
                &snapshot,
                FeatureAuthoringOptions {
                    fillet_radius: Some(0.5),
                    ..FeatureAuthoringOptions::default()
                },
            ),
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } =
            state.pick_many(&snapshot, picks)
        else {
            panic!("the explicitly selected line pair should request a computed preview");
        };
        coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "High-valence native profile preview",
            )
            .expect("computed Fillet preview remains valid");
        (coordinator, candidate)
    }

    #[test]
    fn native_profile_apply_action_is_enabled_for_applicable_native_line_fillet() {
        let (coordinator, _, candidate, _, _, _) = native_line_fillet_fixture();
        let applicable = native_fillet_apply_presentation(
            &coordinator,
            Some(&candidate),
            FeatureAuthoringStage::PreviewReady,
        );
        assert!(applicable.visible);
        assert!(!applicable.disabled);
        assert_eq!(applicable.reason, None);
    }

    #[test]
    fn native_profile_apply_action_reports_exact_grouped_fillet_unavailability() {
        let (mut grouped, _, points) = grouped_fillet_fixture();
        let mut grouped_state = FeatureAuthoringState::default();
        let (grouped_candidate, _) =
            prepare_grouped_fillet(&mut grouped, &mut grouped_state, [points[1], points[2]]);
        let unavailable = native_fillet_apply_presentation(
            &grouped,
            Some(&grouped_candidate),
            FeatureAuthoringStage::PreviewReady,
        );
        assert!(unavailable.visible);
        assert!(unavailable.disabled);
        assert_eq!(
            unavailable.reason.as_deref(),
            Some("Native profile output currently requires exactly one line-line corner")
        );
    }

    #[test]
    fn native_profile_apply_action_reports_exact_high_valence_unavailability() {
        let (high_valence, high_valence_candidate) = high_valence_native_line_fillet_fixture();
        let high_valence_unavailable = native_fillet_apply_presentation(
            &high_valence,
            Some(&high_valence_candidate),
            FeatureAuthoringStage::PreviewReady,
        );
        assert!(high_valence_unavailable.visible);
        assert!(high_valence_unavailable.disabled);
        assert_eq!(
            high_valence_unavailable.reason.as_deref(),
            Some("shared corner must be owned only by the two selected source lines")
        );
    }

    #[test]
    fn native_profile_apply_action_is_hidden_when_fillet_authoring_is_inactive() {
        let (coordinator, _, _, _, _, _) = native_line_fillet_fixture();
        let inactive = native_fillet_apply_presentation(
            &coordinator,
            None,
            FeatureAuthoringStage::PickFirstFilletCurve,
        );
        assert!(!inactive.visible);
        assert!(inactive.disabled);
        assert_eq!(inactive.reason, None);
    }

    #[test]
    fn completed_computed_and_native_feature_apply_return_focus_to_select() {
        for action in ["feature-apply", "feature-apply-native"] {
            assert!(
                feature_apply_returns_focus_to_select(action, false),
                "a successful {action} hides its invoking guide control"
            );
            assert!(
                !feature_apply_returns_focus_to_select(action, true),
                "a rejected {action} keeps the active Fillet surface and its focus"
            );
        }
        assert!(!feature_apply_returns_focus_to_select(
            "offset-apply",
            false
        ));
    }

    #[test]
    fn native_profile_apply_selects_arc_closes_fillet_and_keeps_computed_apply_separate() {
        let (mut coordinator, mut state, candidate, _, lines, corner) =
            native_line_fillet_fixture();
        let history_before = coordinator.history_len();
        let mut candidate = Some(candidate);
        let mut pending = Vec::new();
        let mut overlay = OptionOverlayState::default();
        overlay.open(OptionOverlayKind::Fillet);

        let effects = apply_native_fillet_profile(
            &mut coordinator,
            &mut state,
            &mut candidate,
            &mut pending,
            &mut overlay,
        )
        .expect("native Profile Fillet publication");
        assert!(effects.is_empty(), "Fillet authoring already owns Select");
        assert_eq!(coordinator.editor().tool(), EditorTool::Select);
        assert!(state.active_tool().is_none());
        assert!(candidate.is_none());
        assert!(pending.is_empty());
        assert_eq!(overlay.open, None);
        assert_eq!(coordinator.history_len(), history_before + 1);
        assert!(
            coordinator.feature_document().features().is_empty(),
            "native publication must not create a computed FilletSet"
        );
        let [SelectionItem::Curve(arc)] = coordinator.editor().selection() else {
            panic!("the created native arc should be the sole selection");
        };
        let document = coordinator.session().design_document();
        assert!(matches!(
            document.curve(arc.curve).map(|curve| &curve.definition),
            Some(CurveDefinition::CircularArc { .. })
        ));
        assert_eq!(arc.segment, 0);
        assert!(document.point(corner).is_none());
        for line in lines {
            assert!(
                document.curve(line.curve).is_some(),
                "native output preserves both source line identities"
            );
        }
        assert!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some()
        );
    }

    #[test]
    fn stale_native_profile_apply_is_ui_and_document_neutral() {
        let (mut coordinator, mut state, _, metadata, lines, _) = native_line_fillet_fixture();
        coordinator.set_selection([SelectionItem::Curve(lines[0])]);
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current authoring snapshot");
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate: changed, ..
        } = state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.75),
                ..state.options()
            },
        )
        else {
            panic!("radius change should produce a distinct complete candidate");
        };
        let design_before = coordinator.session().design_identity();
        let history_before = coordinator.history_len();
        let selection_before = coordinator.editor().selection().to_vec();
        let mut candidate = Some(changed.clone());
        let mut pending = Vec::new();
        let mut overlay = OptionOverlayState::default();
        overlay.open(OptionOverlayKind::Fillet);

        let error = apply_native_fillet_profile(
            &mut coordinator,
            &mut state,
            &mut candidate,
            &mut pending,
            &mut overlay,
        )
        .expect_err("a changed candidate cannot consume the older exact preview");
        assert!(error.contains("stale"));
        assert_eq!(coordinator.session().design_identity(), design_before);
        assert_eq!(coordinator.history_len(), history_before);
        assert_eq!(coordinator.editor().selection(), selection_before);
        assert_eq!(candidate.as_ref(), Some(&changed));
        assert_eq!(overlay.open, Some(OptionOverlayKind::Fillet));
        assert_eq!(state.active_tool(), Some(FeatureAuthoringTool::Fillet));
        assert_eq!(
            coordinator
                .feature_authoring_preview()
                .expect("failed application keeps the held preview")
                .metadata()
                .token,
            metadata.token
        );
    }

    #[test]
    fn headless_radius_owner_survives_an_overlying_native_paint_item() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let owners = coordinator
            .feature_authoring_preview()
            .expect("held grouped preview")
            .corner_bindings()
            .iter()
            .map(|binding| binding.owner)
            .collect::<Vec<_>>();
        let expected = SelectionItem::FeatureCorner(owners[0]);
        let native = SelectionItem::Point(points[1]);

        assert_eq!(
            reconcile_feature_authoring_painted_items(
                Some(owners[0]),
                [native, expected, SelectionItem::FeatureCorner(owners[1])],
            ),
            Some(expected),
            "an overlying native SVG item must not hide the exact headless radius owner",
        );
        assert_eq!(
            reconcile_feature_authoring_painted_items(Some(owners[0]), [expected, native]),
            Some(expected),
            "ordinary topmost radius paint remains unchanged",
        );
        assert_eq!(
            reconcile_feature_authoring_painted_items(
                Some(owners[0]),
                [native, SelectionItem::FeatureCorner(owners[1])],
            ),
            Some(native),
            "a foreign computed corner cannot be upgraded to the headless owner",
        );
        assert_eq!(
            reconcile_feature_authoring_painted_items(None, [native, expected]),
            Some(native),
            "without a headless radius hit, browser paint order stays an intent hint only",
        );
    }

    fn ordinary_horizontal_inference_candidate() -> DraftInferenceCandidateId {
        let document = SketchDocument::new(10.0).expect("document");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted empty document");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            super::scene::viewport(),
            0.5,
        )
        .expect("empty editor scene")
        .with_retained_session(&session)
        .expect("authenticated empty editor scene");
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let pointer = |model_position| PointerInput {
            pointer_id: 91,
            position: scene.viewport.model_to_screen(model_position),
            modifiers: Modifiers::default(),
        };
        assert!(
            editor
                .pointer_down_with_draft_authoring(
                    &scene,
                    pointer([1.0, 1.0]),
                    geosolve_constraint_editor::DraftAuthoringInput::default(),
                )
                .is_empty(),
        );
        editor.pointer_move_with_draft_authoring(
            &scene,
            pointer([2.0, 1.0]),
            geosolve_constraint_editor::DraftAuthoringInput::default(),
        );
        editor
            .draft_inference_resolution()
            .and_then(|resolution| resolution.candidates.first())
            .map(|candidate| candidate.id)
            .expect("exact horizontal draft candidate")
    }

    #[test]
    fn pointer_move_queue_keeps_only_latest_sample_and_terminal_invalidates_old_frame() {
        let input = |x| PointerInput {
            pointer_id: 7,
            position: ScreenPoint { x, y: 3.0 },
            modifiers: Modifiers::default(),
        };
        let sample = |x| DraftingPointerSample::from_input(input(x));
        let mut queue = PointerMoveQueue::default();
        let first_frame = queue.push(input(1.0)).unwrap();
        assert_eq!(queue.push(input(2.0)), None);
        assert_eq!(queue.take_for_frame(first_frame), Some(sample(2.0)));
        assert_eq!(queue.take_for_frame(first_frame), None);

        let failed_frame = queue.push(input(2.5)).unwrap();
        queue.cancel_frame(failed_frame);
        let retried_frame = queue.push(input(2.75)).unwrap();
        assert_ne!(retried_frame, failed_frame);
        assert_eq!(queue.take_for_frame(retried_frame), Some(sample(2.75)));

        let stale_frame = queue.push(input(3.0)).unwrap();
        assert_eq!(queue.push(input(4.0)), None);
        assert_eq!(queue.drain_before_terminal(), Some(sample(4.0)));
        let next_frame = queue.push(input(5.0)).unwrap();
        assert_ne!(next_frame, stale_frame);
        assert_eq!(queue.take_for_frame(stale_frame), None);
        assert_eq!(queue.take_for_frame(next_frame), Some(sample(5.0)));

        let stale_before_action = queue.push(input(6.0)).unwrap();
        assert_eq!(queue.push(input(6.5)), None);
        queue.invalidate_before_immediate_action();
        assert_eq!(queue.take_for_frame(stale_before_action), None);
        let after_action = queue.push(input(7.0)).unwrap();
        assert_ne!(after_action, stale_before_action);
        assert_eq!(queue.take_for_frame(after_action), Some(sample(7.0)));
        queue.clear_candidate_preference();

        let suppressed = PointerInput {
            modifiers: Modifiers {
                control: true,
                shift: true,
                ..Modifiers::default()
            },
            ..input(8.0)
        };
        let suppression_frame = queue.push(suppressed).unwrap();
        let captured = queue
            .take_for_frame(suppression_frame)
            .expect("captured suppression sample");
        assert!(captured.authoring.inference.suppressed);
        assert!(captured.authoring.regularized);
        assert_eq!(captured.input, suppressed);

        let mut painted_queue = PointerMoveQueue::default();
        let first_painted = SelectionItem::Datum(geosolve_sketch::SketchDatum::XAxis);
        let latest_painted = SelectionItem::Datum(geosolve_sketch::SketchDatum::YAxis);
        let painted_frame = painted_queue
            .push_with_painted_item(input(9.0), Some(first_painted))
            .expect("painted browser frame");
        assert_eq!(
            painted_queue.push_with_painted_item(input(10.0), Some(latest_painted)),
            None,
        );
        assert_eq!(
            painted_queue.take_for_frame(painted_frame),
            Some(DraftingPointerSample::with_painted_item(
                input(10.0),
                Some(latest_painted),
                None,
            )),
            "RAF coalescing must keep the painted intent hint paired with the latest position",
        );

        let cycle_frame = queue.push(input(11.0)).expect("queued move before Tab");
        assert_eq!(
            queue.drain_before_stationary_cycle(true),
            Some(sample(11.0)),
            "Tab must resolve the newest queued coordinate before reading candidates",
        );
        assert_eq!(
            queue.take_for_frame(cycle_frame),
            None,
            "the retired RAF callback cannot replay the older resolution",
        );

        let foreign_frame = queue
            .push(input(12.0))
            .expect("queued non-drafting-owner move");
        assert_eq!(queue.drain_before_stationary_cycle(false), None);
        assert_eq!(
            queue.take_for_frame(foreign_frame),
            Some(sample(12.0)),
            "Tab outside geometry drafting must not consume another owner's movement",
        );
    }

    #[test]
    fn transient_pointer_frames_do_no_durable_presentation_work_until_release() {
        let mut counters = WorkbenchPresentationCounters::default();
        for _ in 0..5 {
            counters.record(WorkbenchPresentationEvent::PointerMoveFrame);
        }
        assert_eq!(
            counters,
            WorkbenchPresentationCounters {
                transient_renders: 5,
                durable_renders: 0,
                workspace_saves: 0,
                durable_panel_rebuilds: 0,
            },
            "exact move previews must remain persistence- and durable-panel-neutral",
        );
        assert_eq!(
            WorkbenchPresentationEvent::PointerMoveFrame
                .policy()
                .render_scope,
            WorkbenchRenderScope::Transient,
        );

        counters.record(WorkbenchPresentationEvent::AuthenticatedPointerRelease(83));
        assert_eq!(
            counters,
            WorkbenchPresentationCounters {
                transient_renders: 5,
                durable_renders: 1,
                workspace_saves: 1,
                durable_panel_rebuilds: 1,
            },
            "authenticated release is exactly one durable save/render boundary",
        );

        let mut source_owned = WorkbenchPresentationCounters::default();
        source_owned.record(WorkbenchPresentationEvent::CodeSourcePointerRelease);
        assert!(
            WorkbenchPresentationEvent::CodeSourcePointerRelease.code_source_already_published(),
            "source-owned terminal must bypass delegated checkpoint publication",
        );
        assert!(
            !WorkbenchPresentationEvent::PointerRelease.code_source_already_published(),
            "ordinary GUI terminal keeps its existing checkpoint publication route",
        );
        assert_eq!(
            source_owned,
            WorkbenchPresentationCounters {
                transient_renders: 0,
                durable_renders: 1,
                workspace_saves: 1,
                durable_panel_rebuilds: 1,
            },
            "an already-published source terminal persists and renders exactly once",
        );

        let mut non_mutating = WorkbenchPresentationCounters::default();
        non_mutating.record(WorkbenchPresentationEvent::PointerReleaseWithoutTransaction);
        non_mutating.record(WorkbenchPresentationEvent::InteractionCancellation);
        assert_eq!(
            non_mutating,
            WorkbenchPresentationCounters {
                transient_renders: 0,
                durable_renders: 2,
                workspace_saves: 0,
                durable_panel_rebuilds: 2,
            },
            "click-only releases and cancellation restore durable presentation without saving",
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one queue-owner regression exercises every exact stationary-choice invalidation transition"
    )]
    fn stationary_candidate_choice_is_exact_one_shot_and_recovers_without_a_stale_id() {
        let candidate = ordinary_horizontal_inference_candidate();
        let mut resolution = DraftInferenceResolution {
            status: DraftInferenceStatus::StalePreferredCandidate {
                preferred: candidate,
            },
            completeness: DraftInferenceCompleteness::Complete,
            raw_model_position: [0.0, 0.0],
            adjusted_model_position: [0.0, 0.0],
            raw_screen_position: ScreenPoint { x: 11.0, y: 3.0 },
            adjusted_screen_position: ScreenPoint { x: 11.0, y: 3.0 },
            candidates: Vec::new(),
            guides: Vec::new(),
        };
        assert!(draft_inference_preference_is_stale(Some(&resolution)));
        resolution.status = DraftInferenceStatus::Resolved { candidate };
        assert!(!draft_inference_preference_is_stale(Some(&resolution)));

        let input = |x, pointer_id, modifiers| PointerInput {
            pointer_id,
            position: ScreenPoint { x, y: 3.0 },
            modifiers,
        };
        let base = input(11.0, 7, Modifiers::default());
        let mut queue = PointerMoveQueue::default();
        queue.observe(base);

        assert_eq!(queue.stationary_candidate(candidate, false), None);
        assert_eq!(
            queue.observe(base).authoring.inference.preferred_candidate,
            None,
            "a non-drafting owner must never seed a later preference",
        );

        let selected = queue
            .stationary_candidate(candidate, true)
            .expect("owned stationary choice");
        assert_eq!(
            selected.authoring.inference.preferred_candidate,
            Some(candidate),
        );
        let ownership_frame = queue
            .push(base)
            .expect("queued preferred sample before ownership loss");
        assert_eq!(
            queue.stationary_authoring_state(Modifiers::default(), false),
            None,
        );
        assert_eq!(
            queue
                .take_for_frame(ownership_frame)
                .expect("foreign owner keeps its movement")
                .authoring
                .inference
                .preferred_candidate,
            None,
            "ownership loss must scrub a candidate from preserved foreign movement",
        );
        queue
            .stationary_candidate(candidate, true)
            .expect("reselected stationary choice");
        assert_eq!(
            queue
                .observe_for_pointer_down(base)
                .authoring
                .inference
                .preferred_candidate,
            Some(candidate),
            "the unchanged pointer-down may forward the exact choice once",
        );
        assert_eq!(
            queue
                .observe_for_pointer_down(base)
                .authoring
                .inference
                .preferred_candidate,
            None,
            "a rejected pointer-down must not retry its candidate",
        );

        queue
            .stationary_candidate(candidate, true)
            .expect("second stationary choice");
        let moved = input(12.0, 7, Modifiers::default());
        assert_eq!(
            queue.observe(moved).authoring.inference.preferred_candidate,
            None,
            "real movement retires the choice",
        );
        assert_eq!(
            queue.observe(base).authoring.inference.preferred_candidate,
            None,
            "returning to the old coordinate cannot resurrect it",
        );

        queue
            .stationary_candidate(candidate, true)
            .expect("pointer identity choice");
        assert_eq!(
            queue
                .observe(input(11.0, 8, Modifiers::default()))
                .authoring
                .inference
                .preferred_candidate,
            None,
            "pointer identity is part of the stationary context",
        );

        queue.observe(base);
        queue
            .stationary_candidate(candidate, true)
            .expect("modifier choice");
        let shifted = queue
            .stationary_authoring_state(
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                true,
            )
            .expect("stationary modifier refresh");
        assert_eq!(shifted.authoring.inference.preferred_candidate, None);
        assert!(shifted.authoring.regularized);

        queue
            .stationary_candidate(candidate, true)
            .expect("stale recovery choice");
        let refreshed = queue
            .clear_candidate_and_refresh(true)
            .expect("one unpreferred stale recovery sample");
        assert_eq!(refreshed.authoring.inference.preferred_candidate, None);
        assert_eq!(
            queue
                .observe(refreshed.input)
                .authoring
                .inference
                .preferred_candidate,
            None,
            "the recovery refresh does not advertise a reusable fallback ID",
        );
        queue
            .stationary_candidate(candidate, true)
            .expect("blur choice");
        let blur_frame = queue
            .push(refreshed.input)
            .expect("queued preferred sample before blur");
        let blurred = queue.window_blur(true).expect("blur modifier release");
        assert_eq!(blurred.authoring.inference.preferred_candidate, None);
        assert_eq!(
            queue.take_for_frame(blur_frame),
            None,
            "blur must retire a queued sample carrying the old choice",
        );

        queue
            .stationary_candidate(candidate, true)
            .expect("lifecycle choice");
        queue.invalidate_before_immediate_action();
        assert_eq!(
            queue
                .observe(blurred.input)
                .authoring
                .inference
                .preferred_candidate,
            None,
            "history, stage, tool, and overlay transitions share one invalidation path",
        );
    }

    #[test]
    fn geometry_variant_radio_arrows_wrap_within_the_current_family() {
        assert_eq!(
            geometry_variant_keyboard_target(GeometryToolVariant::Segment, "ArrowRight"),
            Some(GeometryToolVariant::Polyline),
        );
        assert_eq!(
            geometry_variant_keyboard_target(GeometryToolVariant::Segment, "ArrowLeft"),
            Some(GeometryToolVariant::MidpointLine),
        );
        assert_eq!(
            geometry_variant_keyboard_target(GeometryToolVariant::Polyline, "End"),
            Some(GeometryToolVariant::MidpointLine),
        );
        assert_eq!(
            geometry_variant_keyboard_target(GeometryToolVariant::MidpointLine, "Home"),
            Some(GeometryToolVariant::Segment),
        );
        assert_eq!(
            geometry_variant_keyboard_target(GeometryToolVariant::Segment, "PageDown"),
            None,
        );
    }

    #[test]
    fn tool_options_escape_reaches_active_authoring_without_leaking_other_dialog_keys() {
        assert!(super::isolate_projectional_keyboard_target(
            true, false, true
        ));
        assert!(super::isolate_projectional_keyboard_target(
            true, true, false
        ));
        assert!(!super::isolate_projectional_keyboard_target(
            true, true, true
        ));
        assert!(!super::isolate_projectional_keyboard_target(
            false, false, true
        ));
    }

    #[test]
    fn sweep_flip_and_double_click_finish_require_a_live_eligible_draft() {
        let status = |variant, completed_stages, can_finish, sweep| GeometryDraftStatus {
            variant,
            stage: GeometryDraftStage::End,
            completed_stages,
            required_stages: None,
            can_finish,
            regularized: false,
            branch: GeometryDraftBranch {
                sweep,
                ..GeometryDraftBranch::default()
            },
            measurements: Vec::new(),
            issue: None,
        };
        let stage_zero = status(
            GeometryToolVariant::CenterArc,
            0,
            false,
            Some(DocumentArcSweep::CounterClockwise),
        );
        assert!(!geometry_sweep_flip_available(
            Some(&stage_zero),
            false,
            false
        ));
        let live_arc = status(
            GeometryToolVariant::CenterArc,
            1,
            false,
            Some(DocumentArcSweep::CounterClockwise),
        );
        assert!(geometry_sweep_flip_available(Some(&live_arc), false, false));
        assert!(!geometry_sweep_flip_available(Some(&live_arc), true, false));
        assert!(!geometry_sweep_flip_available(Some(&live_arc), false, true));

        let polyline_first = status(GeometryToolVariant::Polyline, 3, true, None);
        let polyline_second = status(GeometryToolVariant::Polyline, 4, true, None);
        let mut tracker = FinishDoubleClickTracker::default();
        assert!(!tracker.observe_click(1, Some(&polyline_first)));
        assert!(tracker.observe_click(2, Some(&polyline_second)));

        let mut rejected_second = FinishDoubleClickTracker::default();
        assert!(!rejected_second.observe_click(1, Some(&polyline_first)));
        assert!(!rejected_second.observe_click(2, Some(&polyline_first)));
        let segment = status(GeometryToolVariant::Segment, 1, true, None);
        let mut fixed_recipe = FinishDoubleClickTracker::default();
        assert!(!fixed_recipe.observe_click(1, Some(&segment)));
        assert!(!fixed_recipe.observe_click(2, Some(&segment)));
    }

    #[test]
    fn feature_authoring_routes_uncaptured_hover_and_keeps_its_captured_radius_gesture() {
        assert_eq!(
            canvas_pointer_move_owner(false, false, false, false),
            CanvasPointerMoveOwner::Editor,
        );
        assert_eq!(
            canvas_pointer_move_owner(true, false, false, false),
            CanvasPointerMoveOwner::OrdinaryAuthoring,
        );
        assert_eq!(
            canvas_pointer_move_owner(false, true, false, false),
            CanvasPointerMoveOwner::FeatureAuthoring,
            "an uncaptured Fillet-authoring move must reach its native authoring-owner resolver",
        );
        assert_eq!(
            canvas_pointer_move_owner(false, true, false, true),
            CanvasPointerMoveOwner::Editor,
            "the editor must continue an already captured Fillet-radius gesture",
        );
        assert_eq!(
            canvas_pointer_move_owner(false, false, true, false),
            CanvasPointerMoveOwner::OffsetAuthoring,
            "an uncaptured Offset move must reach its exact shared hover/click resolver",
        );
        assert_eq!(
            canvas_pointer_move_owner(false, false, true, true),
            CanvasPointerMoveOwner::Editor,
            "a captured Offset distance gesture must stay with the headless editor",
        );
    }

    #[test]
    fn overlay_focus_and_letterbox_routes_revoke_queued_and_current_canvas_hover() {
        let (coordinator, _, _, _) = rejected_constraint_fixture();
        let scene = compose_editor_scene(&coordinator, super::scene::viewport(), 0.25)
            .expect("detached accepted presentation scene");
        let point = scene.points.first().expect("accepted point");
        let input = PointerInput {
            pointer_id: 302,
            position: point.screen_position,
            modifiers: Modifiers::default(),
        };
        let expected_hover = EditorHoverState {
            target: Some(EditorHoverTarget::Geometry(SelectionItem::Point(point.id))),
            context_owner: Some(SelectionItem::Point(point.id)),
        };

        for owner in ["overlay", "focus"] {
            let mut editor = ConstraintEditor::default();
            let _ = editor.pointer_move(&scene, input);
            assert_eq!(editor.hover_state(), expected_hover, "{owner} precondition");
            let mut queue = PointerMoveQueue::default();
            let generation = queue.push(input).expect("queued browser frame");
            let revoked = revoke_canvas_pointer_context(
                &mut queue,
                &mut editor,
                CanvasPointerContextRoute::OverlayOrFocus,
            );
            assert!(
                revoked.cleared_stationary_sample,
                "{owner} clears HUD input"
            );
            assert_eq!(
                revoked.effects,
                vec![geosolve_constraint_editor::EditorEffect::HoverChanged(
                    EditorHoverState::default(),
                )],
            );
            assert_eq!(
                queue.take_for_frame(generation),
                None,
                "{owner} revokes RAF"
            );
            assert_eq!(editor.hover_state(), EditorHoverState::default());
        }

        let mut editor = ConstraintEditor::default();
        let _ = editor.pointer_move(&scene, input);
        let mut queue = PointerMoveQueue::default();
        let generation = queue.push(input).expect("queued letterbox frame");
        let revoked = revoke_canvas_pointer_context(
            &mut queue,
            &mut editor,
            CanvasPointerContextRoute::UnmappedCanvas {
                pointer_is_captured: false,
            },
        );
        assert!(revoked.cleared_stationary_sample);
        assert!(!revoked.effects.is_empty());
        assert_eq!(queue.take_for_frame(generation), None);
        assert_eq!(editor.hover_state(), EditorHoverState::default());

        let mut captured_editor = ConstraintEditor::default();
        let _ = captured_editor.pointer_move(&scene, input);
        let mut captured_queue = PointerMoveQueue::default();
        let captured_generation = captured_queue.push(input).expect("captured browser frame");
        assert_eq!(
            revoke_canvas_pointer_context(
                &mut captured_queue,
                &mut captured_editor,
                CanvasPointerContextRoute::UnmappedCanvas {
                    pointer_is_captured: true,
                },
            ),
            super::CanvasPointerContextRevocation::default(),
        );
        assert_eq!(captured_editor.hover_state(), expected_hover);
        assert!(captured_queue.take_for_frame(captured_generation).is_some());
    }

    #[test]
    fn foreign_regularization_transition_preserves_queued_projected_pointer_sample() {
        let input = |x, shift| PointerInput {
            pointer_id: 23,
            position: ScreenPoint { x, y: 19.0 },
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
        };
        let mut queue = PointerMoveQueue::default();

        let press_frame = queue
            .push(input(40.0, false))
            .expect("projected drag frame before Shift press");
        assert_eq!(
            queue.stationary_authoring_state(
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                false,
            ),
            None,
        );
        assert_eq!(
            queue.drain_before_terminal(),
            Some(DraftingPointerSample::from_input(input(40.0, false)))
        );
        assert_eq!(queue.take_for_frame(press_frame), None);

        let release_frame = queue
            .push(input(44.0, true))
            .expect("projected drag frame before Shift release");
        assert_eq!(
            queue.stationary_authoring_state(Modifiers::default(), false),
            None,
        );
        assert_eq!(
            queue.take_for_frame(release_frame),
            Some(DraftingPointerSample::from_input(input(44.0, true)))
        );
    }

    #[test]
    fn stationary_modifier_transitions_replay_one_sample_with_independent_intent() {
        let input = PointerInput {
            pointer_id: 17,
            position: ScreenPoint { x: 412.5, y: 91.25 },
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
        };
        let mut queue = PointerMoveQueue::default();
        let stale_frame = queue.push(input).expect("scheduled pointer frame");

        let pressed_modifiers = Modifiers {
            control: true,
            shift: true,
            ..Modifiers::default()
        };
        let pressed = queue
            .stationary_authoring_state(pressed_modifiers, true)
            .expect("stationary Shift press");
        assert_eq!(pressed.input.modifiers, pressed_modifiers);
        assert_eq!(pressed.input.position, input.position);
        assert!(pressed.authoring.inference.suppressed);
        assert!(pressed.authoring.regularized);
        assert_eq!(queue.take_for_frame(stale_frame), None);
        assert_eq!(
            queue.stationary_authoring_state(pressed_modifiers, true),
            None,
        );

        let released_modifiers = Modifiers {
            control: true,
            ..Modifiers::default()
        };
        let released = queue
            .stationary_authoring_state(released_modifiers, true)
            .expect("stationary Shift release");
        assert_eq!(released.input.modifiers, released_modifiers);
        assert_eq!(released.input.position, input.position);
        assert!(released.authoring.inference.suppressed);
        assert!(!released.authoring.regularized);

        queue
            .stationary_authoring_state(pressed_modifiers, true)
            .expect("second stationary Shift press");
        let blurred = queue.window_blur(true).expect("blur releases suppression");
        assert_eq!(blurred.input.modifiers, Modifiers::default());
        assert_eq!(blurred.input.position, input.position);
        assert!(!blurred.authoring.inference.suppressed);
        assert!(!blurred.authoring.regularized);
        assert_eq!(queue.window_blur(true), None);

        assert!(queue.clear_stationary_sample());
        assert!(!queue.clear_stationary_sample());
        assert_eq!(
            queue.stationary_authoring_state(pressed_modifiers, true),
            None,
        );
    }

    #[test]
    fn fillet_action_render_authority_rejects_stale_dom_stamps_and_inputs() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, first) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let mut authority = FilletActionRenderAuthority::default();
        let first_stamp = authority
            .reconcile(Some(&first.input))
            .expect("first exact action render stamp");
        assert_eq!(authority.reconcile(Some(&first.input)), Some(first_stamp));
        assert!(authority.accepts(first_stamp, Some(&first.input)));

        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current authoring snapshot");
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate: changed, ..
        } = state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.7),
                ..state.options()
            },
        )
        else {
            panic!("radius change should refresh the complete Fillet batch");
        };
        let second = coordinator
            .refresh_feature_authoring_preview(first.input, &changed)
            .expect("new exact computed input");
        assert_ne!(second.input, first.input);
        assert!(
            !authority.accepts(first_stamp, Some(&second.input)),
            "a changed scene must not be upgraded through the old DOM stamp"
        );
        let second_stamp = authority
            .reconcile(Some(&second.input))
            .expect("replacement exact action render stamp");
        assert_ne!(second_stamp, first_stamp);
        assert!(!authority.accepts(first_stamp, Some(&first.input)));
        assert!(authority.accepts(second_stamp, Some(&second.input)));

        assert_eq!(authority.reconcile(None), None);
        assert!(!authority.accepts(second_stamp, Some(&second.input)));
    }

    #[test]
    fn option_inputs_and_selects_defer_render_to_their_change_owner() {
        for tag in ["INPUT", "SELECT", "OPTION"] {
            assert!(change_owns_option_control_click(tag, true, false));
            assert!(change_owns_option_control_click(tag, false, true));
        }
        for tag in ["BUTTON", "DETAILS", "LABEL", "SUMMARY"] {
            assert!(!change_owns_option_control_click(tag, true, true));
        }
        assert!(!change_owns_option_control_click("INPUT", false, false));
    }

    #[test]
    fn option_overlay_catalog_covers_only_option_bearing_tools() {
        use geosolve_constraint_editor::DimensionKind;

        for (key, kind) in [
            ("equal", OptionOverlayKind::Equal),
            ("tangent", OptionOverlayKind::Tangent),
            ("continuity", OptionOverlayKind::Continuity),
            (
                "dimension-point-distance",
                OptionOverlayKind::Dimension(DimensionKind::PointDistance),
            ),
            (
                "dimension-segment-length",
                OptionOverlayKind::Dimension(DimensionKind::SegmentLength),
            ),
            (
                "dimension-radius",
                OptionOverlayKind::Dimension(DimensionKind::Radius),
            ),
            (
                "dimension-diameter",
                OptionOverlayKind::Dimension(DimensionKind::Diameter),
            ),
            (
                "dimension-oriented-angle",
                OptionOverlayKind::Dimension(DimensionKind::OrientedAngle),
            ),
            ("fillet", OptionOverlayKind::Fillet),
            ("offset", OptionOverlayKind::Offset),
            (
                "construction-display",
                OptionOverlayKind::ConstructionDisplay,
            ),
        ] {
            assert_eq!(OptionOverlayKind::from_key(key), Some(kind));
            assert_eq!(kind.key(), key);
            assert!(!kind.title().is_empty());
            assert!(kind.first_control_id().starts_with("wb-"));
        }
        for family in geosolve_constraint_editor::GeometryToolFamily::ALL {
            let kind = OptionOverlayKind::GeometryFamily(family);
            assert_eq!(kind.key(), format!("geometry-{}", family.key()));
            assert!(!kind.title().is_empty());
            assert_eq!(kind.first_control_id(), "wb-geometry-variant-list");
        }
        assert_eq!(OptionOverlayKind::from_key("unknown"), None);
        assert_eq!(
            OptionOverlayKind::for_authoring_tool(AuthoringTool::Constraint(
                ConstraintIntent::Horizontal,
            )),
            None,
            "an unrelated constraint must never parse another family's options"
        );
    }

    #[test]
    fn option_overlay_state_is_mutually_exclusive_and_explicitly_dismissed() {
        let mut state = OptionOverlayState::default();
        state.open(OptionOverlayKind::Equal);
        assert_eq!(state.open, Some(OptionOverlayKind::Equal));
        state.open(OptionOverlayKind::Tangent);
        assert_eq!(state.open, Some(OptionOverlayKind::Tangent));
        state.open(OptionOverlayKind::Tangent);
        assert_eq!(
            state.open,
            Some(OptionOverlayKind::Tangent),
            "reinvoking the current family must not toggle it closed"
        );
        state.open(OptionOverlayKind::Continuity);
        assert_eq!(state.open, Some(OptionOverlayKind::Continuity));
        state.close();
        assert_eq!(state.open, None);
    }

    #[test]
    fn exact_problem_disclosure_dismisses_until_change_or_reopen() {
        let first = "first problem".to_owned();
        let second = "second problem".to_owned();
        let mut disclosure = DismissibleDisclosure::default();
        assert!(!disclosure.reconcile(None::<&String>));
        assert!(disclosure.reconcile(Some(&first)), "new errors auto-open");
        disclosure.dismiss(Some(&first));
        assert!(!disclosure.reconcile(Some(&first)));
        assert!(
            disclosure.reconcile(Some(&second)),
            "a different exact problem set auto-opens"
        );
        disclosure.dismiss(Some(&second));
        disclosure.reopen();
        assert!(disclosure.reconcile(Some(&second)));
        assert!(
            !disclosure.reconcile(None),
            "recovery clears stale visibility"
        );
        disclosure.reopen();
        assert!(
            disclosure.reconcile(None),
            "the footer can open an empty card"
        );
    }

    #[test]
    fn canvas_authoring_click_sequence_contributes_one_operand_and_rearms_after_terminal_attempt() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let origin = document.add_point("origin", [0.0, 0.0]).unwrap();
        let first_tip = document.add_point("first tip", [2.0, 0.0]).unwrap();
        let second_tip = document.add_point("second tip", [0.0, 2.0]).unwrap();
        let first = SelectionItem::Curve(CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: origin,
                        end: first_tip,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        ));
        let second = SelectionItem::Curve(CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: origin,
                        end: second_tip,
                        branch_direction: [0.0, 1.0],
                    },
                )
                .unwrap(),
        ));

        let mut horizontal = AuthoringState::default();
        let _ = horizontal.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            &[],
        );
        let horizontal_outcomes = [
            AuthoringItemInput::CanvasPointerDown,
            AuthoringItemInput::CanvasClick,
        ]
        .into_iter()
        .filter(|input| owns_authoring_pick(*input))
        .map(|_| horizontal.pick(&document, AuthoringOperand::selected(first)))
        .collect::<Vec<_>>();
        assert_eq!(horizontal_outcomes.len(), 1);
        assert!(matches!(horizontal_outcomes[0], AuthoringOutcome::Apply(_)));
        horizontal.transaction_finished();
        assert!(horizontal.pending().is_empty());

        let mut normal = AuthoringState::default();
        let _ = normal.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Perpendicular),
            &[],
        );
        let mut normal_outcomes = Vec::new();
        for item in [first, second] {
            for input in [
                AuthoringItemInput::CanvasPointerDown,
                AuthoringItemInput::CanvasClick,
            ] {
                if owns_authoring_pick(input) {
                    normal_outcomes.push(normal.pick(&document, AuthoringOperand::selected(item)));
                }
            }
        }
        assert_eq!(normal_outcomes.len(), 2);
        assert!(matches!(
            normal_outcomes[0],
            AuthoringOutcome::Collecting { .. }
        ));
        assert!(matches!(normal_outcomes[1], AuthoringOutcome::Apply(_)));
        normal.transaction_finished();
        assert!(normal.pending().is_empty());

        assert!(owns_authoring_pick(AuthoringItemInput::TreeClick));
    }

    #[test]
    fn offset_tree_and_keyboard_activation_route_once_and_preserve_typed_presentation() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let start = document.add_point("start", [-2.0, 0.0]).unwrap();
        let end = document.add_point("end", [2.0, 0.0]).unwrap();
        let line = CurveSpan::line(
            document
                .add_curve(
                    "line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let controls = [
            document.add_point("q0", [-2.0, 3.0]).unwrap(),
            document.add_point("q1", [0.0, 5.0]).unwrap(),
            document.add_point("q2", [2.0, 3.0]).unwrap(),
        ];
        let unsupported = CurveSpan::line(
            document
                .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
                .unwrap(),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let mut authoring = OffsetAuthoringState::default();
        let _ = coordinator
            .activate_offset_authoring(&mut authoring)
            .expect("complete Offset index");

        assert!(!offset_click_owns_semantic_pick(true, true));
        assert!(offset_click_owns_semantic_pick(false, true));
        assert!(offset_click_owns_semantic_pick(true, false));
        let target = offset_target_for_selection(SelectionItem::Curve(line))
            .expect("tree/keyboard curve target");
        assert!(matches!(
            authoring.pick_target(target),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
        let presentation = offset_canvas_presentation(&authoring);
        assert_eq!(presentation.pending, vec![SelectionItem::Curve(line)]);
        assert!(presentation.unavailable.is_empty());
        assert_eq!(
            presentation.chain.as_ref().map(|chain| chain.spans[0].span),
            Some(line)
        );
        assert!(offset_operand_status(&authoring).contains("ordered edge"));

        let _ = authoring.reset();
        let unsupported_target = offset_target_for_selection(SelectionItem::Curve(unsupported))
            .expect("unsupported native curve remains a typed target");
        assert!(matches!(
            authoring.pick_target(unsupported_target),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::UnsupportedOperand,
                ..
            })
        ));
        let presentation = offset_canvas_presentation(&authoring);
        assert!(presentation.pending.is_empty());
        assert_eq!(
            presentation.unavailable,
            vec![SelectionItem::Curve(unsupported)]
        );
        assert!(offset_operand_status(&authoring).starts_with("Unavailable ·"));
        assert!(offset_target_for_selection(SelectionItem::Point(start)).is_none());
    }

    #[test]
    fn workbench_owns_no_direct_operations_companion_dependency() {
        let manifest = include_str!("../../Cargo.toml");
        assert!(manifest.contains("geosolve-constraint-editor ="));
        assert!(!manifest.contains("geosolve-sketch-ops ="));
    }

    #[test]
    fn restarted_or_incomplete_fillet_collection_revokes_an_older_preview() {
        let (mut coordinator, spans, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (candidate, _) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);

        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("feature-authoring snapshot");
        let document = snapshot.sketch_document().clone();
        let mut restarted = FeatureAuthoringState::default();
        let entered = restarted.activate(&snapshot, &document, FeatureAuthoringTool::Fillet, &[]);
        assert!(matches!(entered, FeatureAuthoringOutcome::ModeEntered(_)));
        observe_feature_authoring_preview_lifecycle(&mut coordinator, &entered);
        assert!(coordinator.feature_authoring_preview().is_none());

        coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "Rebuilt Fillet",
            )
            .expect("rebuilt preview");
        let pending_pick = coordinator
            .feature_authoring_picks_for_item(SelectionItem::Curve(spans[0]), None)
            .expect("one native pending pick");
        let collecting = state.pick_many(&snapshot, pending_pick);
        assert!(matches!(
            collecting,
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        observe_feature_authoring_preview_lifecycle(&mut coordinator, &collecting);
        assert!(coordinator.feature_authoring_preview().is_none());
    }

    #[test]
    fn fillet_clear_cancel_and_exit_purge_only_preview_owned_selection() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let owner = coordinator
            .feature_authoring_preview()
            .expect("preview")
            .corner_bindings()[0]
            .owner;
        coordinator.editor_mut().set_selection([
            SelectionItem::Point(points[0]),
            SelectionItem::Feature(metadata.feature),
            SelectionItem::FeatureCorner(owner),
        ]);
        let cleared = state.cancel();
        assert!(matches!(
            cleared,
            FeatureAuthoringOutcome::CandidateCleared(_)
        ));
        observe_feature_authoring_preview_lifecycle(&mut coordinator, &cleared);
        assert_eq!(
            coordinator.editor().selection(),
            &[SelectionItem::Point(points[0])]
        );

        let mut exited_state = FeatureAuthoringState::default();
        let (_, exited_metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut exited_state, [points[1], points[2]]);
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(exited_metadata.feature)]);
        observe_feature_authoring_preview_lifecycle(
            &mut coordinator,
            &FeatureAuthoringOutcome::ModeExited,
        );
        assert!(coordinator.editor().selection().is_empty());

        let mut cleared_state = FeatureAuthoringState::default();
        let (_, cleared_metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut cleared_state, [points[1], points[2]]);
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Feature(cleared_metadata.feature)]);
        revoke_held_feature_authoring_preview(&mut coordinator);
        assert!(coordinator.feature_authoring_preview().is_none());
        assert!(coordinator.editor().selection().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end assertion binds exact Apply, sketch non-mutation and problem attribution"
    )]
    fn grouped_computed_fillet_apply_requires_the_exact_latest_held_preview() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let ordinary_identity = coordinator.session().design_identity();
        let ordinary_counts = {
            let document = coordinator.session().design_document();
            (
                document.points().len(),
                document.curves().len(),
                document.constraints().len(),
                document.dimensions().len(),
                document.contacts().len(),
                document.trim_views().len(),
            )
        };
        let mut state = FeatureAuthoringState::default();
        let (initial, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        assert_eq!(initial.corners().len(), 2);

        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current authoring snapshot");
        let changed = state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.7),
                ..state.options()
            },
        );
        let FeatureAuthoringOutcome::PreviewRequested {
            candidate: changed, ..
        } = changed
        else {
            panic!("shared-radius edit should re-resolve the complete batch");
        };
        let refreshed = coordinator
            .refresh_feature_authoring_preview(metadata.input, &changed)
            .expect("fresh exact whole-batch preview");
        assert_ne!(refreshed.token, metadata.token);
        assert!(
            coordinator
                .apply_feature_authoring_preview(metadata.token, &changed)
                .is_err()
        );
        assert_eq!(
            coordinator
                .feature_authoring_preview()
                .expect("stale token retains the exact current preview")
                .metadata()
                .token,
            refreshed.token
        );
        let created = coordinator
            .apply_feature_authoring_preview(refreshed.token, &changed)
            .expect("latest candidate and token apply atomically");
        assert_eq!(coordinator.feature_document().features().len(), 1);
        assert!(
            coordinator
                .feature_document()
                .feature(created.value)
                .is_some()
        );
        assert_eq!(coordinator.session().design_identity(), ordinary_identity);
        let document = coordinator.session().design_document();
        assert_eq!(
            (
                document.points().len(),
                document.curves().len(),
                document.constraints().len(),
                document.dimensions().len(),
                document.contacts().len(),
                document.trim_views().len(),
            ),
            ordinary_counts,
            "ordinary Fillet authoring must not add M28 sketch graph objects"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one presentation fixture proves exact-preview, shared-owner and accessible action rendering together"
    )]
    fn grouped_preview_renders_both_corner_arcs_with_feature_provenance() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let preview = coordinator
            .feature_authoring_preview()
            .expect("held grouped preview");
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted source");
        let viewport = Viewport::new([1000.0, 700.0], [3.0, 1.5], 80.0).expect("viewport");
        let mut scene = geosolve_constraint_editor::EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &coordinator
                .session()
                .accepted_prepared_input()
                .expect("current accepted grouped-preview input"),
            &metadata.input,
            preview.snapshot(),
            viewport,
            0.8,
        )
        .expect("exact grouped preview scene");
        assert_eq!(scene.computed_curves.len(), 2);
        let implicit_span = scene
            .curves
            .iter()
            .find_map(|curve| {
                matches!(curve.origin, SceneCurveOrigin::FilletDiscarded { .. })
                    .then_some(curve.span)
            })
            .expect("Fillet-discarded native source occurrence");
        let source_occurrences = scene
            .curves
            .iter()
            .filter(|curve| curve.span == implicit_span)
            .count();
        assert!(source_occurrences >= 2);
        let selected_source_markup =
            super::scene::svg_markup_with_computed_context_and_action_stamp(
                Some(&scene),
                Some(accepted),
                &[],
                &[SelectionItem::Curve(implicit_span)],
                &[],
                EditorHoverState::default(),
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy::default(),
                viewport,
            );
        assert!(selected_source_markup.contains("data-construction-origin=\"implicit\""));
        let source_identity = format!(
            "data-persistent-id=\"{}\" data-editor-item=\"curve\" data-editor-segment=\"{}\"",
            implicit_span.curve, implicit_span.segment,
        );
        assert_eq!(
            selected_source_markup
                .split("<path")
                .filter(|path| {
                    path.contains(&source_identity) && path.contains("class=\"wb-curve selected")
                })
                .count(),
            source_occurrences,
            "retained and discarded occurrences must share complete-source selection styling"
        );
        let hidden_implicit_markup =
            super::scene::svg_markup_with_computed_context_and_action_stamp(
                Some(&scene),
                Some(accepted),
                &[],
                &[],
                &[],
                EditorHoverState::default(),
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy {
                    scope: GeometryPickScope::All,
                    visibility: GeometryVisibility {
                        explicit_construction: true,
                        implicit_construction: false,
                        reference_geometry: true,
                    },
                },
                viewport,
            );
        assert!(!hidden_implicit_markup.contains("data-construction-origin=\"implicit\""));
        let profile_pick_scope_markup =
            super::scene::svg_markup_with_computed_context_and_action_stamp(
                Some(&scene),
                Some(accepted),
                &[],
                &[],
                &[],
                EditorHoverState::default(),
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy {
                    scope: GeometryPickScope::Profile,
                    ..GeometryInteractionPolicy::default()
                },
                viewport,
            );
        assert!(
            profile_pick_scope_markup.contains("data-construction-origin=\"implicit\""),
            "pick scope and construction visibility must remain independent"
        );
        let selected = SelectionItem::FeatureCorner(scene.computed_curves[0].owner);
        coordinator
            .populate_computed_fillet_affordances(
                &mut scene,
                &[SelectionItem::Feature(metadata.feature)],
                0.8,
            )
            .expect("grouped Fillet affordances");
        assert_eq!(scene.fillet_affordances.len(), 2);
        let mut construction_scene = scene.clone();
        for curve in &mut construction_scene.computed_curves {
            curve.role = GeometryRole::Construction;
        }
        let profile_policy = GeometryInteractionPolicy {
            scope: GeometryPickScope::Profile,
            ..GeometryInteractionPolicy::default()
        };
        let scoped_markup = super::scene::svg_markup_with_computed_context_and_action_stamp(
            Some(&construction_scene),
            Some(accepted),
            &[],
            &[selected],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            Some(91),
            profile_policy,
            viewport,
        );
        assert!(
            scoped_markup.contains("wb-computed-fillet construction"),
            "pick scope must not hide a visible Construction result"
        );
        let scoped_computed_item = scoped_markup
            .split("<g id=\"wb-scene-computed-curve-0\" class=\"wb-computed-item")
            .nth(1)
            .and_then(|markup| markup.split("</g>").next())
            .expect("visible scope-excluded computed item");
        assert!(scoped_computed_item.contains("interaction-disabled"));
        assert!(scoped_computed_item.contains("data-interactive=\"false\""));
        assert!(!scoped_computed_item.contains("data-editor-item="));
        assert!(!scoped_computed_item.contains("wb-computed-hit"));
        assert!(!scoped_markup.contains("wb-fillet-radius-affordance"));
        assert!(!scoped_markup.contains("data-fillet-action="));
        assert!(
            super::scene::fillet_action_panel_markup_with_stamp(
                &construction_scene,
                Some(91),
                profile_policy,
            )
            .is_empty(),
            "excluded Construction results must not expose accessible actions"
        );
        let markup = super::scene::svg_markup_with_context(
            Some(&scene),
            Some(accepted),
            &[selected],
            &[],
            EditorHoverState::default(),
            None,
            None,
            viewport,
        );
        assert_eq!(markup.matches("class=\"wb-computed-item").count(), 2);
        assert_eq!(
            markup
                .matches("class=\"wb-computed-item selected shared-radius-affected\"")
                .count(),
            1
        );
        assert_eq!(markup.matches("shared-radius-affected").count(), 2);
        let radius_affordance_tag = markup
            .split("<g class=\"wb-fillet-radius-affordance\"")
            .nth(1)
            .and_then(|markup| markup.split('>').next())
            .expect("selected Fillet radius affordance tag");
        assert!(radius_affordance_tag.contains("data-editor-item=\"feature-corner\""));
        assert!(radius_affordance_tag.contains(&format!(
            "data-feature-id=\"{}\"",
            scene.computed_curves[0].owner.feature
        )));
        assert!(radius_affordance_tag.contains(&format!(
            "data-feature-corner-id=\"{}\"",
            scene.computed_curves[0].owner.corner
        )));
        assert_eq!(markup.matches("class=\"wb-fillet-radius-rail\"").count(), 1);
        assert_eq!(
            markup.matches("class=\"wb-fillet-radius-spoke\"").count(),
            1
        );
        assert_eq!(
            markup.matches("class=\"wb-fillet-radius-grip\"").count(),
            1,
            "the selected Fillet corner exposes one central radius handle"
        );
        assert!(
            !markup.contains("wb-fillet-contact"),
            "Fillet endpoint contact metadata must not render redundant canvas handles"
        );
        assert!(
            !markup.contains("wb-fillet-alternative-ghost"),
            "unpreviewed alternatives must not be painted as CSS-owned ghosts"
        );
        let preview_action = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .find(|action| {
                action.dashed_alternative_arc.is_some()
                    && matches!(
                        action.availability,
                        geosolve_constraint_editor::SceneFilletActionAvailability::Applicable
                    )
            })
            .expect("applicable branch alternative");
        let target = scene
            .fillet_action_target(preview_action.owner, preview_action.id)
            .expect("exact semantic action target");
        let (canvas_target, action_position) = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .filter(|action| {
                matches!(
                    action.availability,
                    geosolve_constraint_editor::SceneFilletActionAvailability::Applicable
                )
            })
            .find_map(|action| {
                let canvas_target = scene.fillet_action_target(action.owner, action.id)?;
                let mut positions = Vec::new();
                if let Some(control) = action.control_geometry {
                    positions.push(control.screen_end);
                    positions.push(geosolve_constraint_editor::ScreenPoint {
                        x: (control.screen_start.x + control.screen_end.x) * 0.5,
                        y: (control.screen_start.y + control.screen_end.y) * 0.5,
                    });
                }
                if let Some(geometry) = &action.dashed_alternative_arc {
                    positions.extend(geometry.screen_polyline.iter().copied());
                }
                positions
                    .into_iter()
                    .find(|position| {
                        scene.resolve_fillet_action(
                            geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                                position: *position,
                                painted: Some(canvas_target),
                            },
                            PickTolerance::default(),
                        ) == Some(canvas_target)
                    })
                    .map(|position| (canvas_target, position))
            })
            .expect("unoccluded branch action hit point");
        assert_eq!(
            scene.resolve_fillet_action(
                geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                    position: action_position,
                    painted: Some(canvas_target),
                },
                PickTolerance::default(),
            ),
            Some(canvas_target)
        );
        let overlapping_paint_order_target = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .filter_map(|action| scene.fillet_action_target(action.owner, action.id))
            .find(|target| *target != canvas_target)
            .expect("another painted action target");
        assert_eq!(
            resolve_canvas_fillet_action_candidates(
                &scene,
                GeometryInteractionPolicy::default(),
                action_position,
                [overlapping_paint_order_target, canvas_target],
            ),
            Some(canvas_target),
            "an overlapping topmost corridor must not suppress the headless nearest action"
        );
        assert_eq!(
            resolve_canvas_fillet_action_candidates(
                &scene,
                GeometryInteractionPolicy::default(),
                action_position,
                std::iter::empty(),
            ),
            None,
            "an invalid DOM stamp must not be upgraded from current geometry"
        );
        let direct = scene
            .fillet_affordances
            .iter()
            .find(|affordances| affordances.owner == canvas_target.owner)
            .expect("selected corner affordances");
        for crowded in [
            direct.contacts[0].screen_position,
            direct.radius_rail.screen_grip,
        ] {
            assert_eq!(
                scene.resolve_fillet_action(
                    geosolve_constraint_editor::SceneFilletActionInput::Canvas {
                        position: crowded,
                        painted: Some(canvas_target),
                    },
                    PickTolerance::default(),
                ),
                Some(canvas_target),
                "a painted and independently verified action must not start a Fillet drag"
            );
        }
        let action_stamp = 73;
        let preview_markup = super::scene::svg_markup_with_computed_context_and_action_stamp(
            Some(&scene),
            Some(accepted),
            &[],
            &[selected],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            Some(&target),
            Some(action_stamp),
            GeometryInteractionPolicy::default(),
            viewport,
        );
        assert_eq!(
            preview_markup
                .matches("class=\"wb-fillet-alternative-ghost\"")
                .count(),
            1,
            "only the editor's exact active preview may paint a ghost"
        );
        assert!(preview_markup.contains("wb-fillet-action previewed"));
        assert!(preview_markup.contains(&format!("data-fillet-action-stamp=\"{action_stamp}\"")));
        let panel = super::scene::fillet_action_panel_markup_with_stamp(
            &scene,
            Some(action_stamp),
            GeometryInteractionPolicy::default(),
        );
        assert!(panel.contains("data-fillet-action-input=\"accessible\""));
        assert!(panel.contains(&format!("data-fillet-action-stamp=\"{action_stamp}\"")));
        assert!(panel.contains(&format!(
            "data-fillet-action=\"{}\"",
            super::scene::fillet_action_key(target.action)
        )));
        let mut second_only = scene.clone();
        let second_owner = second_only.fillet_affordances[1].owner;
        second_only
            .fillet_affordances
            .retain(|affordances| affordances.owner == second_owner);
        let disabled_action = second_only.fillet_affordances[0]
            .actions
            .first_mut()
            .expect("second corner action");
        disabled_action.availability =
            geosolve_constraint_editor::SceneFilletActionAvailability::Disabled {
                reason: "Retained <root> & rail unavailable".into(),
            };
        let disabled_key = super::scene::fillet_action_key(disabled_action.id);
        let reason_id = format!(
            "wb-fillet-action-reason-{}-{}-{disabled_key}",
            second_owner.feature, second_owner.corner,
        );
        let second_panel = super::scene::fillet_action_panel_markup_with_stamp(
            &second_only,
            None,
            GeometryInteractionPolicy::default(),
        );
        assert!(second_panel.contains(&format!(
            "aria-label=\"Fillet corner {} actions\"",
            second_owner.corner,
        )));
        assert!(second_panel.contains(&format!(
            "<strong>Fillet corner {}</strong>",
            second_owner.corner,
        )));
        assert!(second_panel.contains(&format!("aria-describedby=\"{reason_id}\"")));
        assert!(second_panel.contains(&format!(
            "<small id=\"{reason_id}\" class=\"wb-fillet-action-reason\">Unavailable: Retained &lt;root&gt; &amp; rail unavailable</small>"
        )));
        assert!(
            !second_panel.contains("<strong>Corner 1</strong>"),
            "a filtered second corner must not be relabelled as the first persisted corner"
        );
        let retained = scene
            .fillet_affordances
            .iter()
            .flat_map(|affordances| &affordances.actions)
            .find_map(|action| action.control_geometry)
            .expect("retained-direction control geometry");
        assert!(preview_markup.contains(&format!(
            "L{:.3} {:.3}",
            retained.screen_end.x, retained.screen_end.y
        )));
        assert!(markup.contains(&format!("data-feature-id=\"{}\"", metadata.feature)));
        assert_eq!(markup.matches("data-computed-edge=").count(), 2);
        assert!(markup.contains("data-scene-provenance=\"accepted\""));
        assert!(markup.contains(&format!(
            "data-accepted-revision=\"{}\"",
            accepted.identity().revision().get()
        )));
        let hovered_markup = super::scene::svg_markup_with_context(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            EditorHoverState {
                target: Some(EditorHoverTarget::Geometry(selected)),
                context_owner: Some(selected),
            },
            None,
            None,
            viewport,
        );
        assert_eq!(
            hovered_markup
                .matches("class=\"wb-computed-item geometry-hovered\"")
                .count(),
            1,
            "only the exact headless computed-corner target is emphasized"
        );
    }

    #[test]
    fn native_authoring_hit_remains_available_at_a_computed_fillet_contact() {
        let (mut coordinator, _, points) = grouped_fillet_fixture();
        let mut state = FeatureAuthoringState::default();
        let (_, metadata) =
            prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
        let preview = coordinator
            .feature_authoring_preview()
            .expect("held grouped preview");
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted source");
        let viewport = Viewport::new([1000.0, 700.0], [3.0, 1.5], 80.0).expect("viewport");
        let scene = geosolve_constraint_editor::EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            &coordinator
                .session()
                .accepted_prepared_input()
                .expect("current accepted grouped-preview input"),
            &metadata.input,
            preview.snapshot(),
            viewport,
            0.8,
        )
        .expect("exact grouped preview scene");
        let (contact, source) = preview
            .snapshot()
            .edges()
            .iter()
            .find_map(|edge| match &edge.geometry {
                geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc) => {
                    Some((arc.contacts[0].position, arc.contacts[0].source.span))
                }
                _ => None,
            })
            .expect("computed Fillet contact");
        let hit = scene
            .native_authoring_hit_test(viewport.model_to_screen(contact), PickTolerance::default())
            .expect("native source remains authorable at the computed contact");
        assert_eq!(hit.item, SelectionItem::Curve(source));
        assert!(matches!(hit.item, SelectionItem::Curve(_)));
    }

    #[test]
    fn newly_installed_off_origin_code_scene_is_fitted_to_the_canvas() {
        run_projectional_test_with_large_stack("m84-code-project-camera-fit", || {
            let (_, editor) =
                super::code_projects::CodeProjectWorkbench::open_key("mounting-plate")
                    .expect("off-origin bundled code project");
            let authority = super::WorkbenchDocumentAuthority::from_projectional_editor(*editor)
                .expect("projectional code authority");
            let mut camera = super::scene::CanvasCamera::default();
            let initial = authority.scene_presentation(
                camera.viewport(),
                super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
            );
            let initial_scene = initial.scene.expect("accepted off-origin code scene");
            let (_, initial_maximum) = initial_scene.model_bounds().expect("finite scene bounds");
            let initial_maximum = camera.viewport().model_to_screen(initial_maximum);
            assert!(
                initial_maximum.x > 1_000.0 || initial_maximum.y > 700.0,
                "the fixture must begin outside the canonical Origin camera"
            );

            assert!(super::fit_projectional_camera_to_authority(
                &mut camera,
                &authority,
            ));
            assert_ne!(camera, super::scene::CanvasCamera::default());
            let fitted = authority.scene_presentation(
                camera.viewport(),
                super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
            );
            let fitted_scene = fitted.scene.expect("fitted accepted code scene");
            let (minimum, maximum) = fitted_scene.model_bounds().expect("finite fitted bounds");
            let viewport = camera.viewport();
            for corner in [
                [minimum[0], minimum[1]],
                [minimum[0], maximum[1]],
                [maximum[0], minimum[1]],
                [maximum[0], maximum[1]],
            ] {
                let point = viewport.model_to_screen(corner);
                assert!((63.0..=937.0).contains(&point.x), "fitted x={}", point.x);
                assert!((63.0..=637.0).contains(&point.y), "fitted y={}", point.y);
            }
        });
    }

    #[test]
    fn rounded_polyline_code_scene_paints_four_visible_radius_four_fillets() {
        run_projectional_test_with_large_stack("m84-rounded-polyline-visible-fillets", || {
            let (_, editor) =
                super::code_projects::CodeProjectWorkbench::open_key("rounded-polyline")
                    .expect("rounded Polyline code project");
            let authority = super::WorkbenchDocumentAuthority::from_projectional_editor(*editor)
                .expect("projectional code authority");
            let mut camera = super::scene::CanvasCamera::default();
            assert!(super::fit_projectional_camera_to_authority(
                &mut camera,
                &authority,
            ));
            let presentation = authority.scene_presentation(
                camera.viewport(),
                super::WORKBENCH_CURVE_CHORD_TOLERANCE_PIXELS,
            );
            let scene = presentation.scene.expect("accepted rounded Polyline scene");
            assert_eq!(scene.computed_curves.len(), 4);
            let accepted_materialization = authority
                .projectional_ref()
                .expect("projectional code editor")
                .coordinator()
                .accepted_materialization()
                .expect("accepted code materialization");
            let accepted = accepted_materialization
                .session
                .accepted_state_for_current_input()
                .expect("accepted native sketch state");
            let markup = super::scene::svg_markup(
                Some(&scene),
                Some(accepted),
                &[],
                None,
                None,
                camera.viewport(),
            );
            assert_eq!(
                markup
                    .matches("class=\"wb-curve wb-computed-fillet\"")
                    .count(),
                4,
            );

            for fillet in &scene.computed_curves {
                assert_eq!(fillet.radius.to_bits(), 4.0_f64.to_bits());
                assert!(fillet.screen_polyline.len() >= 3);
                assert!(fillet.screen_polyline.iter().all(|point| {
                    point.x.is_finite()
                        && point.y.is_finite()
                        && (0.0..=1_000.0).contains(&point.x)
                        && (0.0..=700.0).contains(&point.y)
                }));

                let visible_clearance = fillet
                    .screen_polyline
                    .iter()
                    .map(|sample| {
                        scene
                            .points
                            .iter()
                            .map(|point| {
                                (sample.x - point.screen_position.x)
                                    .hypot(sample.y - point.screen_position.y)
                            })
                            .fold(f64::INFINITY, f64::min)
                    })
                    .fold(0.0_f64, f64::max);
                assert!(
                    visible_clearance > 10.0,
                    "the radius-4 arc must extend clearly beyond every 5 px point marker; clearance={visible_clearance}"
                );
            }
        });
    }
}
