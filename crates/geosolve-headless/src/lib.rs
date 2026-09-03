// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]

//! Browser-free orchestration for managed `GeoSolve` code projects.
//!
//! This crate owns input selection, deterministic namespaces, cold native
//! materialization, independent acceptance checks, static scene composition,
//! and atomic output publication. It does not execute TypeScript, start a
//! server, access the network, or duplicate solver or rendering equations.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CodeProjectDemoId, CodeSessionIdentity, KeyedReconcileState,
    ManagedControlEditBatch, ManagedControlManifest, ManagedMutationAuthority, PatchModuleArtifact,
    PreparedManagedMutationReceipt, PreparedManagedMutationRequest, ProjectKey,
    bundled_code_project_demos, managed_control_manifest, materialize_code_project_cold,
    prepare_managed_control_mutation, prepare_managed_mutation, required_generated_members,
    validate_prepared_managed_mutation,
};
use geosolve_sketch_intent::{IntentSessionId, intent_content_digest};
use geosolve_sketch_render::{
    CanvasCamera, PNG_EXPORT_HEIGHT, PNG_EXPORT_WIDTH, compose_static_scene_svg,
    render_default_scene_png, standalone_static_export_svg_with_size,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Version of every deterministic headless report emitted by M87.
pub const HEADLESS_REPORT_VERSION: &str = "geosolve-headless-report-v1";
/// Chord tolerance shared with canonical static browser export.
pub const HEADLESS_CHORD_TOLERANCE_PIXELS: f64 = 0.25;
/// Maximum encoded size of one exact-CAS edit batch accepted by the CLI.
pub const HEADLESS_EDIT_BATCH_LIMIT: usize = 16 * 1024 * 1024;
/// Maximum encoded prepared request or compiler receipt accepted by the CLI.
pub const HEADLESS_PREPARED_EDIT_LIMIT: usize =
    geosolve_sketch_code::PREPARED_MANAGED_MUTATION_WIRE_LIMIT;

const MAX_INPUT_BYTES: usize = geosolve_sketch_code::CODE_PROJECT_LIMIT;
static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(1);

/// One admitted browser-free project input.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadlessInput {
    /// Strict canonical `CodeProject` JSON, including any pinned artifacts.
    CodeProjectJson(String),
    /// One checked-in offline demonstration key.
    BundledDemo(String),
}

/// Stable description of the admitted input authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HeadlessInputIdentity {
    CodeProjectJson { project: ProjectKey },
    BundledDemo { key: String, project: ProjectKey },
}

/// Independent native acceptance evidence included in every solved report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadlessValidationReport {
    pub hard_residuals_validated: bool,
    pub all_active_features_current: bool,
    pub maximum_normalized_hard_residual: Option<f64>,
    pub point_count: usize,
    pub curve_count: usize,
    pub constraint_count: usize,
    pub dimension_count: usize,
    pub feature_count: usize,
    pub computed_edge_count: usize,
}

/// Fitted deterministic camera used for scene and export composition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadlessCameraReport {
    pub logical_size: [u32; 2],
    pub model_center: [f64; 2],
    pub pixels_per_model_unit: f64,
    pub fit_margin_pixels: f64,
    pub chord_tolerance_pixels: f64,
}

/// Versioned report for inspect, render, and edit commands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadlessReport {
    pub version: String,
    pub input: HeadlessInputIdentity,
    pub project_digest: String,
    pub source_digest: String,
    pub expansion_digest: String,
    pub intent_session: String,
    pub document: String,
    pub validation: HeadlessValidationReport,
    pub camera: HeadlessCameraReport,
    pub scene_markup_sha256: String,
    pub svg_sha256: String,
    pub png_sha256: String,
    pub png_dimensions: [u32; 2],
}

/// Inspection output: the exact transient controls plus independently solved authority.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadlessInspection {
    pub report: HeadlessReport,
    pub controls: ManagedControlManifest,
}

/// Exact compiler-host work required for one headless managed-control edit.
///
/// Pure Rust prepares and later authenticates this transaction. The browser-
/// free caller executes `request` through the pinned Deno mutation sidecar,
/// supplying `patches` as its complete compiler context.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadlessPreparedEdit {
    pub request: PreparedManagedMutationRequest,
    pub patches: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Static scene products ready for atomic publication.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessRender {
    report: HeadlessReport,
    controls: ManagedControlManifest,
    project: CodeProject,
    scene_markup: String,
    svg: String,
    png: Vec<u8>,
}

impl HeadlessRender {
    /// Returns the independently validated report for this exact render.
    #[must_use]
    pub fn report(&self) -> &HeadlessReport {
        &self.report
    }

    /// Returns the transient managed-control manifest for this exact render.
    #[must_use]
    pub fn controls(&self) -> &ManagedControlManifest {
        &self.controls
    }

    /// Returns the complete accepted project authority for this exact render.
    #[must_use]
    pub fn project(&self) -> &CodeProject {
        &self.project
    }

    /// Returns the deterministic logical-scene SVG fragment.
    #[must_use]
    pub fn scene_markup(&self) -> &str {
        &self.scene_markup
    }

    /// Returns the deterministic standalone SVG document.
    #[must_use]
    pub fn svg(&self) -> &str {
        &self.svg
    }

    /// Returns the mandatory native PNG product.
    #[must_use]
    pub fn png(&self) -> &[u8] {
        &self.png
    }
}

/// Rejected headless input, solve, render, edit, or publication.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HeadlessError {
    #[error("headless input exceeds the project byte limit")]
    InputTooLarge,
    #[error("unknown bundled demonstration `{0}`")]
    UnknownDemo(String),
    #[error("headless project is invalid: {0}")]
    Project(String),
    #[error("headless managed controls are invalid: {0}")]
    Controls(String),
    #[error("headless native materialization failed: {0}")]
    Materialization(String),
    #[error("headless accepted authority failed independent validation: {0}")]
    IndependentValidation(String),
    #[error("headless scene composition failed: {0}")]
    Scene(String),
    #[error("headless report encoding failed: {0}")]
    Encoding(String),
    #[error("headless output directory already exists: {0}")]
    OutputExists(String),
    #[error("headless output publication failed: {0}")]
    Io(String),
}

impl HeadlessInput {
    fn load(&self) -> Result<(HeadlessInputIdentity, CodeProject), HeadlessError> {
        match self {
            Self::CodeProjectJson(json) => {
                if json.len() > MAX_INPUT_BYTES {
                    return Err(HeadlessError::InputTooLarge);
                }
                let project = CodeProject::from_json(json)
                    .map_err(|error| HeadlessError::Project(error.to_string()))?;
                let identity = HeadlessInputIdentity::CodeProjectJson {
                    project: project.project.clone(),
                };
                Ok((identity, project))
            }
            Self::BundledDemo(key) => {
                let demo = bundled_code_project_demos()
                    .into_iter()
                    .find(|demo| demo.id.key() == key)
                    .ok_or_else(|| HeadlessError::UnknownDemo(key.clone()))?;
                let project = demo.project();
                Ok((
                    HeadlessInputIdentity::BundledDemo {
                        key: key.clone(),
                        project: project.project.clone(),
                    },
                    project,
                ))
            }
        }
    }
}

/// Lists the complete stable bundled-demo key inventory.
#[must_use]
pub fn bundled_demo_keys() -> Vec<&'static str> {
    const IDS: [CodeProjectDemoId; 12] = [
        CodeProjectDemoId::RoundedPolyline,
        CodeProjectDemoId::TypedPanel,
        CodeProjectDemoId::BracedFrame,
        CodeProjectDemoId::MountingPlate,
        CodeProjectDemoId::AdaptiveLanterns,
        CodeProjectDemoId::SuspensionBridge,
        CodeProjectDemoId::CompassRose,
        CodeProjectDemoId::NeonManifold,
        CodeProjectDemoId::PcWaterManifold,
        CodeProjectDemoId::RoboticRoutingBoard,
        CodeProjectDemoId::CncJoineryFitCoupon,
        CodeProjectDemoId::GridfinityBinSection,
    ];
    IDS.into_iter().map(CodeProjectDemoId::key).collect()
}

/// Inspects one project through the same complete cold solve used by render/edit.
///
/// # Errors
///
/// Returns a bounded input, expansion, control, materialization, validation,
/// scene, or encoding error. No file is written.
pub fn inspect(input: &HeadlessInput) -> Result<HeadlessInspection, HeadlessError> {
    let rendered = render(input)?;
    Ok(HeadlessInspection {
        report: rendered.report,
        controls: rendered.controls,
    })
}

/// Prepares one exact-CAS managed control batch for a pinned compiler host.
///
/// # Errors
///
/// A stale/foreign token, schema mismatch, invalid project, or malformed
/// compiler authority rejects without changing the input.
pub fn prepare_edit(
    input: &HeadlessInput,
    batch: &ManagedControlEditBatch,
) -> Result<HeadlessPreparedEdit, HeadlessError> {
    let (_, project) = input.load()?;
    let staged = reconcile(&project)?;
    let (intent, document) = deterministic_ids(&project)?;
    let materialized = materialize_code_project_cold(&project, &staged, intent, document, 1.0)
        .map_err(|error| HeadlessError::Materialization(error.to_string()))?;
    let mutation = prepare_managed_control_mutation(&project, &materialized.expansion, batch)
        .map_err(|error| HeadlessError::Controls(error.to_string()))?;
    let compiled = project.managed.compiled.as_deref().ok_or_else(|| {
        HeadlessError::Project("project has no compiled managed authority".into())
    })?;
    let authority =
        headless_mutation_authority(&project, &materialized.expansion.digest, compiled)?;
    let request = prepare_managed_mutation(
        &authority,
        compiled,
        mutation,
        project.managed.declaration_name_high_water,
    )
    .map_err(|error| HeadlessError::Controls(error.to_string()))?;
    Ok(HeadlessPreparedEdit {
        request,
        patches: managed_compiler_patches(&project)?,
    })
}

/// Authenticates a pinned-compiler receipt, then cold-solves and renders the
/// complete candidate.
///
/// # Errors
///
/// A stale/tampered receipt, unrelated semantic delta, failed solve, or failed
/// independent validation publishes no candidate and leaves `input`
/// untouched.
pub fn resolve_edit(
    input: &HeadlessInput,
    prepared: &HeadlessPreparedEdit,
    receipt: PreparedManagedMutationReceipt,
) -> Result<HeadlessRender, HeadlessError> {
    let (identity, project) = input.load()?;
    let staged = reconcile(&project)?;
    let (intent, document) = deterministic_ids(&project)?;
    let materialized = materialize_code_project_cold(&project, &staged, intent, document, 1.0)
        .map_err(|error| HeadlessError::Materialization(error.to_string()))?;
    let compiled = project.managed.compiled.as_deref().ok_or_else(|| {
        HeadlessError::Project("project has no compiled managed authority".into())
    })?;
    let authority =
        headless_mutation_authority(&project, &materialized.expansion.digest, compiled)?;
    let validated = validate_prepared_managed_mutation(&authority, &prepared.request, receipt)
        .map_err(|error| HeadlessError::Controls(error.to_string()))?;
    let candidate_high_water = validated.declaration_name_high_water();
    let managed = validated
        .into_compiled()
        .into_managed_document()
        .map_err(|error| HeadlessError::Project(error.to_string()))?;
    let mut edited = project;
    edited.managed = managed;
    edited.managed.declaration_name_high_water = candidate_high_water;
    edited
        .validate()
        .map_err(|error| HeadlessError::Project(error.to_string()))?;
    solve_and_render(identity_for_edited(identity, &edited), edited)
}

/// Cold-solves and composes one authoritative static scene without a browser.
///
/// # Errors
///
/// Returns a bounded input, expansion, materialization, independent-validation,
/// scene, or encoding error. No file is written.
pub fn render(input: &HeadlessInput) -> Result<HeadlessRender, HeadlessError> {
    let (identity, project) = input.load()?;
    solve_and_render(identity, project)
}

fn identity_for_edited(
    identity: HeadlessInputIdentity,
    project: &CodeProject,
) -> HeadlessInputIdentity {
    match identity {
        HeadlessInputIdentity::CodeProjectJson { .. } => HeadlessInputIdentity::CodeProjectJson {
            project: project.project.clone(),
        },
        HeadlessInputIdentity::BundledDemo { key, .. } => HeadlessInputIdentity::BundledDemo {
            key,
            project: project.project.clone(),
        },
    }
}

fn headless_mutation_authority(
    project: &CodeProject,
    expansion_digest: &str,
    compiled: &geosolve_sketch_code::CompiledManagedSource,
) -> Result<ManagedMutationAuthority, HeadlessError> {
    let project_json = project
        .to_canonical_json()
        .map_err(|error| HeadlessError::Project(error.to_string()))?;
    let digest = intent_content_digest(project_json.as_bytes()).to_string();
    ManagedMutationAuthority::new(
        project.project.clone(),
        CodeSessionIdentity {
            session: 1,
            revision: 0,
            digest,
        },
        expansion_digest.to_owned(),
        project.managed.declaration_name_high_water,
        compiled,
    )
    .map_err(|error| HeadlessError::Controls(error.to_string()))
}

fn managed_compiler_patches(
    project: &CodeProject,
) -> Result<std::collections::BTreeMap<String, serde_json::Value>, HeadlessError> {
    let mut patches = std::collections::BTreeMap::new();
    for import in &project.managed.imports {
        for binding in &import.bindings {
            let matches = project
                .artifacts
                .values()
                .filter_map(|value| {
                    let artifact =
                        serde_json::from_value::<PatchModuleArtifact>(value.clone()).ok()?;
                    (artifact.module_specifier == import.module && artifact.export_name == *binding)
                        .then_some(value.clone())
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => {}
                [artifact] => {
                    if patches.insert(binding.clone(), artifact.clone()).is_some() {
                        return Err(HeadlessError::Project(format!(
                            "managed compiler patch binding `{binding}` is ambiguous"
                        )));
                    }
                }
                _ => {
                    return Err(HeadlessError::Project(format!(
                        "managed compiler patch binding `{binding}` resolves more than once"
                    )));
                }
            }
        }
    }
    Ok(patches)
}

#[allow(
    clippy::too_many_lines,
    reason = "one headless boundary keeps deterministic identity, cold solve, independent validation, final camera fit, and static composition auditable"
)]
fn solve_and_render(
    input: HeadlessInputIdentity,
    project: CodeProject,
) -> Result<HeadlessRender, HeadlessError> {
    let generated = reconcile(&project)?;
    let (intent, document) = deterministic_ids(&project)?;
    let materialized = materialize_code_project_cold(&project, &generated, intent, document, 1.0)
        .map_err(|error| HeadlessError::Materialization(error.to_string()))?;
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| {
            HeadlessError::IndependentValidation("no accepted native authority".into())
        })?;
    let validation = &accepted.validation;
    if !validation.hard_residuals_validated {
        return Err(HeadlessError::IndependentValidation(
            "hard residuals were not independently validated".into(),
        ));
    }
    if !validation.all_active_features_current {
        return Err(HeadlessError::IndependentValidation(
            "one or more active computed features are not Current".into(),
        ));
    }
    if validation
        .maximum_normalized_hard_residual
        .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(HeadlessError::IndependentValidation(
            "maximum normalized hard residual exceeds 1e-9 or is non-finite".into(),
        ));
    }

    let mut camera = CanvasCamera::default();
    let initial = materialized
        .editor
        .scene(camera.viewport(), HEADLESS_CHORD_TOLERANCE_PIXELS)
        .map_err(|error| HeadlessError::Scene(error.to_string()))?;
    let model_bounds = initial.model_bounds();
    if model_bounds
        .is_some_and(|(minimum, maximum)| !minimum.into_iter().chain(maximum).all(f64::is_finite))
    {
        return Err(HeadlessError::IndependentValidation(
            "scene model bounds are non-finite".into(),
        ));
    }
    if model_bounds.is_some() && !camera.fit_model_bounds(model_bounds) {
        return Err(HeadlessError::Scene(
            "scene model bounds cannot fit the canonical finite camera".into(),
        ));
    }
    let viewport = camera.viewport();
    let scene = materialized
        .editor
        .scene(viewport, HEADLESS_CHORD_TOLERANCE_PIXELS)
        .map_err(|error| HeadlessError::Scene(error.to_string()))?;
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .and_then(|accepted| accepted.session.accepted_state_for_current_input())
        .ok_or_else(|| {
            HeadlessError::IndependentValidation("accepted state is not current".into())
        })?;
    let scene_markup = compose_static_scene_svg(Some(&scene), Some(accepted), camera);
    let svg =
        standalone_static_export_svg_with_size(&scene_markup, PNG_EXPORT_WIDTH, PNG_EXPORT_HEIGHT)
            .ok_or_else(|| HeadlessError::Scene("static export dimensions are invalid".into()))?;
    let png = render_default_scene_png(&scene_markup)
        .map_err(|error| HeadlessError::Scene(error.to_string()))?;
    let controls = managed_control_manifest(&project, &materialized.expansion)
        .map_err(|error| HeadlessError::Controls(error.to_string()))?;
    let project_json = project
        .to_canonical_json()
        .map_err(|error| HeadlessError::Project(error.to_string()))?;
    let project_digest = sha256(project_json.as_bytes());
    let validation_report = HeadlessValidationReport {
        hard_residuals_validated: validation.hard_residuals_validated,
        all_active_features_current: validation.all_active_features_current,
        maximum_normalized_hard_residual: validation.maximum_normalized_hard_residual,
        point_count: validation.point_count,
        curve_count: validation.curve_count,
        constraint_count: validation.constraint_count,
        dimension_count: accepted.document().dimensions().len(),
        feature_count: validation.feature_count,
        computed_edge_count: validation.computed_edge_count,
    };
    let report = HeadlessReport {
        version: HEADLESS_REPORT_VERSION.into(),
        input,
        project_digest,
        source_digest: project.managed.source_digest.clone(),
        expansion_digest: materialized.expansion.digest.clone(),
        intent_session: intent.to_string(),
        document: document.to_string(),
        validation: validation_report,
        camera: HeadlessCameraReport {
            logical_size: [1_000, 700],
            model_center: camera.model_center(),
            pixels_per_model_unit: camera.pixels_per_model_unit(),
            fit_margin_pixels: geosolve_sketch_render::FIT_MARGIN_PIXELS,
            chord_tolerance_pixels: HEADLESS_CHORD_TOLERANCE_PIXELS,
        },
        scene_markup_sha256: sha256(scene_markup.as_bytes()),
        svg_sha256: sha256(svg.as_bytes()),
        png_sha256: sha256(&png),
        png_dimensions: [PNG_EXPORT_WIDTH, PNG_EXPORT_HEIGHT],
    };
    Ok(HeadlessRender {
        report,
        controls,
        project,
        scene_markup,
        svg,
        png,
    })
}

fn reconcile(project: &CodeProject) -> Result<KeyedReconcileState, HeadlessError> {
    let desired = required_generated_members(project)
        .map_err(|error| HeadlessError::Materialization(error.to_string()))?;
    KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .map(geosolve_sketch_code::KeyedReconcilePlan::into_staged)
        .map_err(|error| HeadlessError::Materialization(error.to_string()))
}

fn deterministic_ids(
    project: &CodeProject,
) -> Result<(IntentSessionId, DocumentId), HeadlessError> {
    let json = project
        .to_canonical_json()
        .map_err(|error| HeadlessError::Project(error.to_string()))?;
    let digest = intent_content_digest(json.as_bytes());
    let bytes = digest.bytes();
    let mut left = [0_u8; 16];
    let mut right = [0_u8; 16];
    left.copy_from_slice(&bytes[..16]);
    right.copy_from_slice(&bytes[16..]);
    let intent = u128::from_be_bytes(left).max(1);
    let document = u128::from_be_bytes(right).max(1);
    Ok((
        IntentSessionId::from_raw(intent),
        DocumentId(PersistentId::from_u128(document)),
    ))
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

/// Atomically publishes a render into a new directory without overwriting an existing path.
///
/// The directory contains `report.json`, `controls.json`, `project.json`,
/// `sketch.ts`, `scene.svg`, and `scene.png`. A failure before
/// the final rename removes only the newly created sibling temporary directory.
/// Atomic no-replace directory publication is available on Linux, Android,
/// Apple platforms, and Redox; other targets fail closed as unsupported.
///
/// # Errors
///
/// Returns an existing-target, encoding, or I/O error. A failed publication
/// never leaves the requested output directory behind.
pub fn publish_render(render: &HeadlessRender, output: &Path) -> Result<(), HeadlessError> {
    if output.exists() {
        return Err(HeadlessError::OutputExists(output.display().to_string()));
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| HeadlessError::Io("output directory name is not valid Unicode".into()))?;
    let ordinal = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{name}.geosolve-tmp-{}-{ordinal}",
        std::process::id()
    ));
    if temporary.exists() {
        return Err(HeadlessError::Io(format!(
            "temporary output path already exists: {}",
            temporary.display()
        )));
    }
    fs::create_dir(&temporary).map_err(|error| io_error(&error))?;
    let result = publish_render_into(render, &temporary)
        .and_then(|()| sync_directory(&temporary))
        .and_then(|()| rename_directory_no_replace(&temporary, output));
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

fn publish_render_into(render: &HeadlessRender, directory: &Path) -> Result<(), HeadlessError> {
    let report = serde_json::to_string_pretty(&render.report)
        .map_err(|error| HeadlessError::Encoding(error.to_string()))?;
    let controls = serde_json::to_string_pretty(&render.controls)
        .map_err(|error| HeadlessError::Encoding(error.to_string()))?;
    let project = render
        .project
        .to_canonical_json()
        .map_err(|error| HeadlessError::Encoding(error.to_string()))?;
    write_new(&directory.join("report.json"), report.as_bytes())?;
    write_new(&directory.join("controls.json"), controls.as_bytes())?;
    write_new(&directory.join("project.json"), project.as_bytes())?;
    write_new(
        &directory.join("sketch.ts"),
        render.project.managed.source.as_bytes(),
    )?;
    write_new(&directory.join("scene.svg"), render.svg.as_bytes())?;
    write_new(&directory.join("scene.png"), &render.png)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), HeadlessError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error(&error))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
))]
fn rename_directory_no_replace(source: &Path, destination: &Path) -> Result<(), HeadlessError> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    match renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        Err(error) if error == rustix::io::Errno::EXIST || error == rustix::io::Errno::NOTEMPTY => {
            Err(HeadlessError::OutputExists(
                destination.display().to_string(),
            ))
        }
        Err(error) => Err(HeadlessError::Io(error.to_string())),
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
)))]
fn rename_directory_no_replace(_source: &Path, _destination: &Path) -> Result<(), HeadlessError> {
    Err(HeadlessError::Io(
        "atomic no-clobber directory publication is unsupported on this target".into(),
    ))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), HeadlessError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error(&error))?;
    file.write_all(bytes).map_err(|error| io_error(&error))?;
    file.sync_all().map_err(|error| io_error(&error))
}

fn io_error(error: &std::io::Error) -> HeadlessError {
    HeadlessError::Io(error.to_string())
}
