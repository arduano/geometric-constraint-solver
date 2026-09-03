// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use geosolve_constraint_editor::{
    AuthoringMutation, AuthoringOperand, AuthoringOutcome, AuthoringState, AuthoringTool,
    ComputedSceneState, ConstraintIntent, RetainedEditorCoordinator,
    SceneCurveOrigin, SceneFilletActionAvailability, SelectionItem,
};
use geosolve_core::SolverConfig;
use geosolve_sketch::{
    CurveDefinition, DocumentConstraintDefinition, DocumentId, DocumentSolveRequest, PersistentId,
    RetainedSketchDocumentSession, SketchDocument,
};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, KeyedReconcileState, MaterializedCodeProject, ProjectKey,
    export_sketch_document_to_managed_source,
    export_sketch_document_with_features_to_managed_source, materialize_code_project_cold,
    required_generated_members,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocatorHighWater, ComputedEvaluationRevision, ComputedFeatureDefinition,
    ComputedFeatureDocument,
};
use geosolve_sketch_intent::{
    IntentAttemptDisposition, IntentPlanDisposition, IntentSessionId, intent_content_digest,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::super::WorkbenchDocumentAuthority;

const MANIFEST_FORMAT: &str = "geosolve-golden-scene-backend-parity-v1";
const SCENE_CASES: [&str; 4] = [
    "scene.current-computed.empty",
    "scene.current-native.withheld",
    "scene.current-computed.fillet",
    "scene.rejected-historical.detached",
];

#[derive(Clone)]
struct HarnessContext {
    case_id: String,
    directory: PathBuf,
    result: PathBuf,
}

impl HarnessContext {
    fn from_environment() -> Option<Self> {
        let case_id = env::var("GEOSOLVE_GOLDEN_ORACLE_CASE").ok();
        let directory = env::var_os("GEOSOLVE_GOLDEN_PARITY_DIRECTORY");
        let result = env::var_os("GEOSOLVE_GOLDEN_PARITY_RESULT");
        if case_id.is_none() && directory.is_none() && result.is_none() {
            return None;
        }
        Some(Self {
            case_id: case_id.expect("scene parity case is required"),
            directory: PathBuf::from(directory.expect("scene parity directory is required")),
            result: PathBuf::from(result.expect("scene parity result is required")),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SceneParityManifest {
    format: String,
    case_id: String,
    model_scale: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompileBatchRequest {
    entries: Vec<CompileBatchRequestEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompileBatchRequestEntry {
    id: usize,
    source: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompileBatchOutput {
    entries: Vec<CompileBatchOutputEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompileBatchOutputEntry {
    id: usize,
    #[serde(default)]
    compiled: Option<Value>,
    #[serde(default)]
    error: Option<CompileBatchError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompileBatchError {
    name: String,
    message: String,
    #[serde(default)]
    span: Option<Value>,
}

enum PhaseResult {
    Pass,
    Defect(String),
    Panic(String),
}

#[test]
fn golden_scene_backend_parity_export() {
    let Some(context) = HarnessContext::from_environment() else {
        return;
    };
    let result = context.result.clone();
    let output = run_with_large_stack(move || export_phase(&context));
    record_result(&result, output);
}

#[test]
fn golden_scene_backend_parity_validate() {
    let Some(context) = HarnessContext::from_environment() else {
        return;
    };
    let result = context.result.clone();
    let output = run_with_large_stack(move || validate_phase(&context));
    record_result(&result, output);
}

fn run_with_large_stack(
    run: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> PhaseResult {
    match std::thread::Builder::new()
        .name("golden-scene-backend-parity".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(move || catch_unwind(AssertUnwindSafe(run)))
    {
        Ok(thread) => match thread.join() {
            Ok(Ok(Ok(()))) => PhaseResult::Pass,
            Ok(Ok(Err(message))) => PhaseResult::Defect(message),
            Ok(Err(payload)) | Err(payload) => PhaseResult::Panic(panic_payload(&payload)),
        },
        Err(error) => PhaseResult::Panic(format!("cannot start parity worker: {error}")),
    }
}

fn record_result(path: &Path, result: PhaseResult) {
    let (status, class, detail) = match result {
        PhaseResult::Pass => ("PASS", "-", "ok".to_owned()),
        PhaseResult::Defect(message) => ("DEFECT", "scene-backend-parity", message),
        PhaseResult::Panic(message) => ("PANIC", "scene-backend-parity-panic", message),
    };
    let detail = sanitize(&detail);
    let fingerprint = if status == "PASS" {
        detail
    } else {
        format!("{:016x}:{detail}", fnv1a64(detail.as_bytes()))
    };
    fs::write(path, format!("{status}\t{class}\t{fingerprint}\n"))
        .expect("write scene parity result");
}

fn export_phase(context: &HarnessContext) -> Result<(), String> {
    require_case(&context.case_id)?;
    fs::create_dir_all(&context.directory).map_err(|error| {
        format!(
            "cannot create scene parity directory {}: {error}",
            context.directory.display()
        )
    })?;
    let (source, model_scale) = exported_source(&context.case_id)?;
    fs::write(context.directory.join("stage-0.sketch.ts"), source.as_bytes())
        .map_err(|error| format!("cannot write scene parity source: {error}"))?;
    let request = CompileBatchRequest {
        entries: vec![CompileBatchRequestEntry {
            id: 0,
            source,
        }],
    };
    fs::write(
        context.directory.join("compile-batch.request.json"),
        serde_json::to_vec(&request)
            .map_err(|error| format!("cannot encode scene compile request: {error}"))?,
    )
    .map_err(|error| format!("cannot write scene compile request: {error}"))?;
    let manifest = SceneParityManifest {
        format: MANIFEST_FORMAT.into(),
        case_id: context.case_id.clone(),
        model_scale,
    };
    fs::write(
        context.directory.join("scene-parity.manifest.json"),
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("cannot encode scene parity manifest: {error}"))?,
    )
    .map_err(|error| format!("cannot write scene parity manifest: {error}"))
}

fn validate_phase(context: &HarnessContext) -> Result<(), String> {
    require_case(&context.case_id)?;
    let manifest: SceneParityManifest = serde_json::from_slice(
        &fs::read(context.directory.join("scene-parity.manifest.json"))
            .map_err(|error| format!("cannot read scene parity manifest: {error}"))?,
    )
    .map_err(|error| format!("invalid scene parity manifest: {error}"))?;
    if manifest.format != MANIFEST_FORMAT
        || manifest.case_id != context.case_id
        || !manifest.model_scale.is_finite()
        || manifest.model_scale <= 0.0
    {
        return Err("scene parity manifest identity or scale is invalid".into());
    }
    let source = fs::read_to_string(context.directory.join("stage-0.sketch.ts"))
        .map_err(|error| format!("cannot read scene parity source: {error}"))?;
    let compiled = read_compiled(&context.directory)?;
    if compiled.input_source_digest != intent_content_digest(source.as_bytes()).to_string() {
        return Err("scene compiler did not authenticate the exported source bytes".into());
    }
    let project = CodeProject::managed(
        ProjectKey(format!("golden-scene-parity-{}", context.case_id)),
        compiled,
    )
    .map_err(|error| format!("scene code project rejected: {error}"))?;
    let desired = required_generated_members(&project)
        .map_err(|error| format!("scene generated inventory failed: {error}"))?;
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .map_err(|error| format!("scene generated reconciliation failed: {error}"))?
        .into_staged();
    let raw = 0x91_5cee_0000_0000_0000_0000_0000_0000_u128
        | u128::from(fnv1a64(context.case_id.as_bytes()));
    let managed = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(raw),
        DocumentId(PersistentId::from_u128(raw)),
        manifest.model_scale,
    )
    .map_err(|error| format!("scene managed materialization failed: {error}"))?;
    compare_case(&context.case_id, managed, &context.directory)
}

fn require_case(case_id: &str) -> Result<(), String> {
    if SCENE_CASES.contains(&case_id) {
        Ok(())
    } else {
        Err(format!("unknown scene parity case `{case_id}`"))
    }
}

fn exported_source(case_id: &str) -> Result<(String, f64), String> {
    match case_id {
        "scene.current-computed.empty" | "scene.current-native.withheld" => {
            let (coordinator, _, _) = super::grouped_fillet_fixture();
            let document = coordinator.session().design_document();
            let source = export_sketch_document_to_managed_source(document)
                .map_err(|error| format!("scene document export failed: {error}"))?;
            Ok((source, document.model_scale()))
        }
        "scene.current-computed.fillet" => {
            let coordinator = committed_fillet_coordinator()?;
            let document = coordinator.session().design_document();
            let source = export_sketch_document_with_features_to_managed_source(
                document,
                coordinator.feature_document(),
            )
            .map_err(|error| format!("scene Fillet export failed: {error}"))?;
            Ok((source, document.model_scale()))
        }
        "scene.rejected-historical.detached" => {
            let document = rejected_base_document()?;
            let source = export_sketch_document_to_managed_source(&document)
                .map_err(|error| format!("rejected scene export failed: {error}"))?;
            Ok((source, document.model_scale()))
        }
        _ => Err(format!("unknown scene parity case `{case_id}`")),
    }
}

fn read_compiled(directory: &Path) -> Result<CompiledManagedSource, String> {
    let output: CompileBatchOutput = serde_json::from_slice(
        &fs::read(directory.join("compile-batch.output.json"))
            .map_err(|error| format!("cannot read scene compiler output: {error}"))?,
    )
    .map_err(|error| format!("invalid scene compiler output: {error}"))?;
    let [entry] = output.entries.as_slice() else {
        return Err("scene compiler did not return exactly one entry".into());
    };
    if entry.id != 0 {
        return Err(format!("scene compiler returned unexpected entry {}", entry.id));
    }
    match (&entry.compiled, &entry.error) {
        (Some(compiled), None) => CompiledManagedSource::from_json(
            &serde_json::to_string(compiled)
                .map_err(|error| format!("cannot encode scene compiler envelope: {error}"))?,
        )
        .map_err(|error| format!("scene compiler envelope rejected: {error}")),
        (None, Some(error)) => Err(format!(
            "scene managed compile failed: {}: {}{}",
            error.name,
            error.message,
            error
                .span
                .as_ref()
                .map_or_else(String::new, |span| format!(" at {span}")),
        )),
        _ => Err("scene compiler returned an ambiguous outcome".into()),
    }
}

fn compare_case(
    case_id: &str,
    managed: MaterializedCodeProject,
    diagnostic_directory: &Path,
) -> Result<(), String> {
    let (native, managed) = authority_pair(case_id, managed)?;
    let viewport = super::super::scene::viewport();
    let native_presentation = native.scene_presentation(viewport, 0.25);
    let managed_presentation = managed.scene_presentation(viewport, 0.25);
    if native_presentation.status_override.is_some()
        || managed_presentation.status_override.is_some()
    {
        return Err(format!(
            "scene presentation failed: native {:?}, managed {:?}",
            native_presentation.status_override, managed_presentation.status_override
        ));
    }
    let native_scene = native_presentation
        .scene
        .ok_or_else(|| "native scene parity authority produced no scene".to_owned())?;
    let managed_scene = managed_presentation
        .scene
        .ok_or_else(|| "managed scene parity authority produced no scene".to_owned())?;
    validate_accepted_authority(case_id, &native, &native_scene, "native")?;
    validate_accepted_authority(case_id, &managed, &managed_scene, "managed")?;
    let native_observation = scene_observation(&native, &native_scene)?;
    let managed_observation = scene_observation(&managed, &managed_scene)?;
    if let Err(detail) = compare_values(&native_observation, &managed_observation, 2.0e-9, "$") {
        let native_path = diagnostic_directory.join("native-scene.json");
        let managed_path = diagnostic_directory.join("managed-scene.json");
        let _ = fs::write(
            &native_path,
            serde_json::to_vec_pretty(&native_observation).unwrap_or_default(),
        );
        let _ = fs::write(
            &managed_path,
            serde_json::to_vec_pretty(&managed_observation).unwrap_or_default(),
        );
        return Err(format!(
            "{case_id} native/managed scene semantics differ: {detail} (native {}, managed {})",
            native_path.display(),
            managed_path.display()
        ));
    }
    validate_case_contract(case_id, &native, &managed, &native_scene, &managed_scene)
}

fn authority_pair(
    case_id: &str,
    mut managed: MaterializedCodeProject,
) -> Result<(WorkbenchDocumentAuthority, WorkbenchDocumentAuthority), String> {
    match case_id {
        "scene.current-computed.empty" => {
            let (native, _, _) = super::grouped_fillet_fixture();
            Ok((
                WorkbenchDocumentAuthority::flat(native),
                WorkbenchDocumentAuthority::from_projectional_editor(managed.editor)?,
            ))
        }
        "scene.current-native.withheld" => {
            let (native_base, _, _) = super::grouped_fillet_fixture();
            let native = withheld_coordinator(
                native_base.session().clone(),
                native_base.feature_document().clone(),
                native_base.feature_document().lifecycle_high_water(),
            )?;
            let accepted = managed
                .editor
                .coordinator()
                .accepted_materialization()
                .ok_or_else(|| "managed withheld base has no accepted authority".to_owned())?;
            let managed = withheld_coordinator(
                accepted.session.clone(),
                accepted.features.clone(),
                accepted.feature_lifecycle_high_water,
            )?;
            Ok((
                WorkbenchDocumentAuthority::flat(native),
                WorkbenchDocumentAuthority::flat(managed),
            ))
        }
        "scene.current-computed.fillet" => {
            let native = committed_fillet_coordinator()?;
            let feature = managed
                .editor
                .coordinator()
                .accepted_materialization()
                .and_then(|accepted| accepted.features.features().first())
                .map(|feature| feature.id)
                .ok_or_else(|| "managed Fillet source materialized no feature".to_owned())?;
            managed
                .editor
                .set_selection([SelectionItem::Feature(feature)]);
            Ok((
                WorkbenchDocumentAuthority::flat(native),
                WorkbenchDocumentAuthority::from_projectional_editor(managed.editor)?,
            ))
        }
        "scene.rejected-historical.detached" => {
            let session = RetainedSketchDocumentSession::new(
                rejected_base_document()?,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .map_err(|error| format!("native rejected base solve failed: {error}"))?;
            let mut native = RetainedEditorCoordinator::new(session)
                .map_err(|error| format!("native rejected coordinator failed: {error}"))?;
            retain_native_vertical_failure(&mut native)?;
            retain_managed_vertical_failure(&mut managed.editor)?;
            Ok((
                WorkbenchDocumentAuthority::flat(native),
                WorkbenchDocumentAuthority::from_projectional_editor(managed.editor)?,
            ))
        }
        _ => Err(format!("unknown scene parity case `{case_id}`")),
    }
}

fn withheld_coordinator(
    session: RetainedSketchDocumentSession,
    features: ComputedFeatureDocument,
    feature_high_water: geosolve_sketch_features::ComputedFeatureLifecycleHighWater,
) -> Result<RetainedEditorCoordinator, String> {
    RetainedEditorCoordinator::with_features_and_high_water(
        session,
        features,
        feature_high_water,
        ComputedEvaluationAllocatorHighWater {
            next_revision: ComputedEvaluationRevision::from_raw(u64::MAX),
        },
    )
    .map_err(|error| format!("withheld parity coordinator failed: {error}"))
}

fn committed_fillet_coordinator() -> Result<RetainedEditorCoordinator, String> {
    let (mut coordinator, _, points) = super::grouped_fillet_fixture();
    let mut state = geosolve_constraint_editor::FeatureAuthoringState::default();
    let (candidate, metadata) =
        super::prepare_grouped_fillet(&mut coordinator, &mut state, [points[1], points[2]]);
    let feature = coordinator
        .apply_feature_authoring_preview(metadata.token, &candidate)
        .map_err(|error| format!("cannot commit scene parity Fillet: {error}"))?
        .value;
    coordinator.set_selection([SelectionItem::Feature(feature)]);
    Ok(coordinator)
}

fn rejected_base_document() -> Result<SketchDocument, String> {
    let mut document = SketchDocument::new(10.0).map_err(|error| error.to_string())?;
    let start = document
        .add_point("fixed horizontal start", [0.0, 0.0])
        .map_err(|error| error.to_string())?;
    let end = document
        .add_point("fixed horizontal end", [2.0, 0.0])
        .map_err(|error| error.to_string())?;
    document
        .add_curve(
            "fixed horizontal line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|error| error.to_string())?;
    for (label, point, target) in [
        ("lock horizontal start", start, [0.0, 0.0]),
        ("lock horizontal end", end, [2.0, 0.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(document)
}

fn vertical_application(document: &SketchDocument) -> Result<geosolve_constraint_editor::AuthoringApplication, String> {
    let curve = document
        .curves()
        .first()
        .ok_or_else(|| "rejected parity base has no line".to_owned())?
        .id;
    let span = document
        .curve_spans(curve)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "rejected parity base line has no span".to_owned())?;
    let mut state = AuthoringState::default();
    match state.activate(
        document,
        AuthoringTool::Constraint(ConstraintIntent::Vertical),
        &[AuthoringOperand::selected(SelectionItem::Curve(span))],
    ) {
        AuthoringOutcome::Apply(application) => Ok(application),
        other => Err(format!(
            "rejected parity vertical authoring did not complete: {other:?}"
        )),
    }
}

fn retain_native_vertical_failure(coordinator: &mut RetainedEditorCoordinator) -> Result<(), String> {
    let application = vertical_application(coordinator.session().design_document())?;
    let mutation = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .map_err(|error| format!("native vertical authoring failed unexpectedly: {error}"))?;
    let AuthoringMutation::Constraint(outcome) = mutation else {
        return Err("native vertical authoring returned a dimension".into());
    };
    if outcome.published_accepted.is_some()
        || coordinator
            .session()
            .accepted_state_for_current_input()
            .is_some()
    {
        return Err("native conflicting vertical relation replaced accepted geometry".into());
    }
    Ok(())
}

fn retain_managed_vertical_failure(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
) -> Result<(), String> {
    let document = editor
        .coordinator()
        .presentation_session()
        .ok_or_else(|| "managed rejected base has no presentation session".to_owned())?
        .design_document()
        .clone();
    let accepted_before = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "managed rejected base has no accepted materialization".to_owned())?
        .session
        .design_document()
        .to_draft_v5_json()
        .map_err(|error| error.to_string())?;
    let application = vertical_application(&document)?;
    let outcome = editor
        .apply_authoring_application(&application)
        .map_err(|error| format!("managed vertical authoring failed unexpectedly: {error}"))?;
    if outcome.disposition != IntentPlanDisposition::RetainedFailed {
        return Err(format!(
            "managed conflicting vertical relation returned {:?}",
            outcome.disposition
        ));
    }
    let retained = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "managed retained failure discarded accepted authority".to_owned())?
        .session
        .design_document()
        .to_draft_v5_json()
        .map_err(|error| error.to_string())?;
    if retained != accepted_before {
        return Err("managed retained failure replaced accepted geometry".into());
    }
    Ok(())
}

fn validate_accepted_authority(
    case_id: &str,
    authority: &WorkbenchDocumentAuthority,
    scene: &geosolve_constraint_editor::EditorScene,
    backend: &str,
) -> Result<(), String> {
    let session = presentation_session(authority)?;
    let accepted = session
        .accepted_state()
        .ok_or_else(|| format!("{case_id} {backend} authority has no accepted geometry"))?;
    let solve = accepted
        .diagnostics()
        .solve
        .ok_or_else(|| format!("{case_id} {backend} accepted geometry has no solve report"))?;
    if !solve.accepted
        || !solve.hard_residuals_validated
        || solve
            .maximum_normalized_hard_residual
            .is_none_or(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(format!(
            "{case_id} {backend} accepted scene lacks independent residual validation"
        ));
    }
    for value in scene_observation(authority, scene)?.numbers() {
        if !value.is_finite() {
            return Err(format!("{case_id} {backend} scene contains a non-finite value"));
        }
    }
    Ok(())
}

trait JsonNumbers {
    fn numbers(&self) -> Vec<f64>;
}

impl JsonNumbers for Value {
    fn numbers(&self) -> Vec<f64> {
        let mut values = Vec::new();
        collect_numbers(self, &mut values);
        values
    }
}

fn collect_numbers(value: &Value, values: &mut Vec<f64>) {
    match value {
        Value::Number(number) => {
            if let Some(number) = number.as_f64() {
                values.push(number);
            }
        }
        Value::Array(array) => {
            for value in array {
                collect_numbers(value, values);
            }
        }
        Value::Object(object) => {
            for value in object.values() {
                collect_numbers(value, values);
            }
        }
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

fn validate_case_contract(
    case_id: &str,
    native: &WorkbenchDocumentAuthority,
    managed: &WorkbenchDocumentAuthority,
    native_scene: &geosolve_constraint_editor::EditorScene,
    managed_scene: &geosolve_constraint_editor::EditorScene,
) -> Result<(), String> {
    let current = |authority: &WorkbenchDocumentAuthority| current_attempt_state(authority).0;
    match case_id {
        "scene.current-computed.empty" => {
            if !current(native)
                || !current(managed)
                || native_scene.computed_input.is_none()
                || managed_scene.computed_input.is_none()
                || !native_scene.computed_curves.is_empty()
                || !managed_scene.computed_curves.is_empty()
                || native_scene
                    .curves
                    .iter()
                    .chain(&managed_scene.curves)
                    .any(|curve| curve.origin != SceneCurveOrigin::Native)
            {
                return Err("empty scene did not retain current empty computed authority".into());
            }
        }
        "scene.current-native.withheld" => {
            for authority in [native, managed] {
                let WorkbenchDocumentAuthority::Flat(coordinator) = authority else {
                    return Err("withheld parity must exercise the native fallback owner".into());
                };
                if !matches!(coordinator.computed_scene_state(), ComputedSceneState::Withheld)
                    || coordinator.computed_feature_problems().len() != 1
                {
                    return Err("withheld parity did not retain its global evaluation problem".into());
                }
            }
            if !current(native)
                || !current(managed)
                || native_scene.computed_input.is_some()
                || managed_scene.computed_input.is_some()
                || !native_scene.computed_curves.is_empty()
                || !managed_scene.computed_curves.is_empty()
            {
                return Err("withheld scene leaked computed geometry or lost native currentness".into());
            }
        }
        "scene.current-computed.fillet" => {
            if !current(native)
                || !current(managed)
                || native_scene.computed_curves.len() != 2
                || managed_scene.computed_curves.len() != 2
                || native_scene.fillet_affordances.len() != 2
                || managed_scene.fillet_affordances.len() != 2
                || !native_scene
                    .curves
                    .iter()
                    .any(|curve| curve.origin.is_implicit_construction())
                || !managed_scene
                    .curves
                    .iter()
                    .any(|curve| curve.origin.is_implicit_construction())
            {
                return Err("computed Fillet scene is incomplete or not current".into());
            }
        }
        "scene.rejected-historical.detached" => {
            let (native_current, native_failed) = current_attempt_state(native);
            let (managed_current, managed_failed) = current_attempt_state(managed);
            if native_current || managed_current || !native_failed || !managed_failed {
                return Err("rejected scene did not retain a historical accepted authority".into());
            }
            if native_scene.points.iter().any(|point| {
                point.model_position.map(f64::to_bits)
                    != [0.0_f64, 0.0].map(f64::to_bits)
                    && point.model_position.map(f64::to_bits)
                        != [2.0_f64, 0.0].map(f64::to_bits)
            }) || managed_scene.points.iter().any(|point| {
                point.model_position.map(f64::to_bits)
                    != [0.0_f64, 0.0].map(f64::to_bits)
                    && point.model_position.map(f64::to_bits)
                        != [2.0_f64, 0.0].map(f64::to_bits)
            }) {
                return Err("rejected candidate coordinates replaced accepted scene geometry".into());
            }
        }
        _ => return Err(format!("unknown scene parity case `{case_id}`")),
    }
    Ok(())
}

fn current_attempt_state(authority: &WorkbenchDocumentAuthority) -> (bool, bool) {
    match authority {
        WorkbenchDocumentAuthority::Flat(coordinator) => {
            let current = coordinator
                .session()
                .accepted_state_for_current_input()
                .is_some();
            (current, !current && coordinator.session().accepted_state().is_some())
        }
        WorkbenchDocumentAuthority::Projectional { editor, .. } => {
            let disposition = editor
                .coordinator()
                .intent()
                .latest_attempt()
                .map(|attempt| attempt.disposition);
            (
                disposition == Some(IntentAttemptDisposition::Accepted),
                disposition == Some(IntentAttemptDisposition::RetainedFailed),
            )
        }
    }
}

fn presentation_session(
    authority: &WorkbenchDocumentAuthority,
) -> Result<&RetainedSketchDocumentSession, String> {
    match authority {
        WorkbenchDocumentAuthority::Flat(coordinator) => Ok(coordinator
            .visible_preview_session()
            .unwrap_or(coordinator.session())),
        WorkbenchDocumentAuthority::Projectional { editor, .. } => editor
            .presentation_session()
            .ok_or_else(|| "projectional authority has no presentation session".to_owned()),
    }
}

struct SemanticIndex {
    points: BTreeMap<geosolve_sketch::DesignPointId, [f64; 2]>,
    curves: BTreeMap<geosolve_sketch::CurveId, usize>,
    features: BTreeMap<geosolve_sketch_features::ComputedFeatureId, usize>,
    corners: BTreeMap<geosolve_sketch_features::ComputedFeatureCornerId, (usize, usize)>,
}

impl SemanticIndex {
    fn capture(
        document: &SketchDocument,
        features: &ComputedFeatureDocument,
    ) -> Result<Self, String> {
        let points = document
            .points()
            .iter()
            .map(|point| (point.id, point.position))
            .collect();
        let curves = document
            .curves()
            .iter()
            .enumerate()
            .map(|(index, curve)| (curve.id, index))
            .collect();
        let mut feature_index = BTreeMap::new();
        let mut corners = BTreeMap::new();
        for (index, feature) in features.features().iter().enumerate() {
            feature_index.insert(feature.id, index);
            let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
            for (corner_index, corner) in fillet.corners.iter().enumerate() {
                corners.insert(corner.id, (index, corner_index));
            }
        }
        Ok(Self {
            points,
            curves,
            features: feature_index,
            corners,
        })
    }

    fn curve(&self, span: geosolve_sketch::CurveSpan) -> Result<Value, String> {
        Ok(json!({
            "curve": self.curves.get(&span.curve).copied().ok_or_else(|| {
                format!("scene references unknown curve {:?}", span.curve)
            })?,
            "segment": span.segment,
        }))
    }

    fn source(
        &self,
        source: geosolve_sketch_features::NativeCurveSpanSource,
    ) -> Result<Value, String> {
        self.curve(source.span)
    }

    fn owner(
        &self,
        owner: geosolve_sketch_features::ComputedCornerRef,
    ) -> Result<Value, String> {
        let feature = self.features.get(&owner.feature).copied().ok_or_else(|| {
            format!("scene references unknown computed feature {:?}", owner.feature)
        })?;
        let (corner_feature, corner) = self.corners.get(&owner.corner).copied().ok_or_else(|| {
            format!("scene references unknown computed corner {:?}", owner.corner)
        })?;
        if feature != corner_feature {
            return Err("scene computed corner belongs to another feature".into());
        }
        Ok(json!({ "feature": feature, "corner": corner }))
    }

    fn item(&self, item: SelectionItem) -> Result<Value, String> {
        match item {
            SelectionItem::Point(point) => Ok(json!({
                "kind": "point",
                "position": self.points.get(&point).copied().ok_or_else(|| {
                    format!("scene references unknown point {point:?}")
                })?,
            })),
            SelectionItem::Curve(curve) => {
                Ok(json!({ "kind": "curve", "value": self.curve(curve)? }))
            }
            SelectionItem::Datum(datum) => Ok(json!({ "kind": "datum", "value": format!("{datum:?}") })),
            SelectionItem::Feature(feature) => Ok(json!({
                "kind": "feature",
                "index": self.features.get(&feature).copied().ok_or_else(|| {
                    format!("scene references unknown feature {feature:?}")
                })?,
            })),
            SelectionItem::FeatureCorner(owner) => {
                Ok(json!({ "kind": "featureCorner", "value": self.owner(owner)? }))
            }
            SelectionItem::Constraint(_) => Ok(json!({ "kind": "constraint" })),
            SelectionItem::Dimension(_) => Ok(json!({ "kind": "dimension" })),
        }
    }
}

fn scene_observation(
    authority: &WorkbenchDocumentAuthority,
    scene: &geosolve_constraint_editor::EditorScene,
) -> Result<Value, String> {
    let (document, features) = authority_documents(authority)?;
    let index = SemanticIndex::capture(document, features)?;
    let mut points = scene
        .points
        .iter()
        .map(|point| {
            Ok(json!({
                "modelPosition": point.model_position,
                "screenPosition": [point.screen_position.x, point.screen_position.y],
                "roles": {
                    "profile": point.role_incidence.profile,
                    "construction": point.role_incidence.construction,
                },
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    points.sort_by_cached_key(|point| serde_json::to_string(point).unwrap_or_default());
    let mut curves = scene
        .curves
        .iter()
        .map(|curve| {
            let origin = match curve.origin {
                SceneCurveOrigin::Native => json!({ "kind": "native" }),
                SceneCurveOrigin::FilletDiscarded {
                    source,
                    interval,
                    provenance,
                    ..
                } => json!({
                    "kind": "filletDiscarded",
                    "source": index.source(source)?,
                    "interval": [interval.start, interval.end],
                    "provenance": {
                        "owner": index.owner(provenance.owner)?,
                        "endpoint": format!("{:?}", provenance.endpoint),
                        "baseInterval": [
                            provenance.base_interval.start,
                            provenance.base_interval.end,
                        ],
                    },
                }),
            };
            let drag_handle = curve
                .drag_handle_point
                .map(|point| {
                    index
                        .points
                        .get(&point)
                        .copied()
                        .ok_or_else(|| "curve drag handle has no accepted point owner".to_owned())
                })
                .transpose()?;
            Ok(json!({
                "span": index.curve(curve.span)?,
                "authoringEligible": curve.authoring_eligible,
                "affine": curve.affine,
                "contactDomain": format!("{:?}", curve.contact_domain),
                "role": format!("{:?}", curve.role),
                "sourceRole": format!("{:?}", curve.source_role),
                "origin": origin,
                "screenPolyline": screen_points(&curve.screen_polyline),
                "screenParameters": curve.screen_parameters,
                "dragHandle": drag_handle,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    curves.sort_by_cached_key(|curve| serde_json::to_string(curve).unwrap_or_default());
    let mut computed = scene
        .computed_curves
        .iter()
        .map(|curve| {
            let contacts = curve
                .contacts
                .iter()
                .map(|contact| {
                    Ok(json!({
                        "source": index.source(contact.source)?,
                        "parameter": contact.parameter,
                        "winding": contact.winding,
                        "totalParameter": contact.total_parameter,
                        "position": contact.position,
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(json!({
                "owner": index.owner(curve.owner)?,
                "role": format!("{:?}", curve.role),
                "center": curve.center,
                "radius": curve.radius,
                "startAngle": curve.start_angle,
                "endAngle": curve.end_angle,
                "sweep": format!("{:?}", curve.sweep),
                "contacts": contacts,
                "screenPolyline": screen_points(&curve.screen_polyline),
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    computed.sort_by_cached_key(|curve| serde_json::to_string(curve).unwrap_or_default());
    let mut affordances = scene
        .fillet_affordances
        .iter()
        .map(|affordance| {
            let affected = affordance
                .affected_owners
                .iter()
                .copied()
                .map(|owner| index.owner(owner))
                .collect::<Result<Vec<_>, _>>()?;
            let contacts = affordance
                .contacts
                .iter()
                .map(|contact| {
                    Ok(json!({
                        "parent": format!("{:?}", contact.parent),
                        "source": index.source(contact.source)?,
                        "parameter": contact.parameter,
                        "modelPosition": contact.model_position,
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let actions = affordance
                .actions
                .iter()
                .map(|action| {
                    let availability = match &action.availability {
                        SceneFilletActionAvailability::Applicable => json!({ "kind": "applicable" }),
                        SceneFilletActionAvailability::Disabled { reason } => {
                            json!({ "kind": "disabled", "reason": reason })
                        }
                    };
                    Ok(json!({
                        "id": format!("{:?}", action.id),
                        "availability": availability,
                        "control": action.control_geometry.map(|control| json!({
                            "anchor": control.model_anchor,
                            "direction": control.model_direction,
                        })),
                        "alternative": action.dashed_alternative_arc.as_ref().map(|arc| &arc.model_polyline),
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(json!({
                "owner": index.owner(affordance.owner)?,
                "affectedOwners": affected,
                "radiusRail": {
                    "center": affordance.radius_rail.model_center,
                    "grip": affordance.radius_rail.model_grip,
                    "derivative": affordance.radius_rail.model_derivative,
                },
                "contacts": contacts,
                "actions": actions,
                "hasContinuationLimit": affordance.continuation_status.is_some(),
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    affordances.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());
    let (current, retained_failed) = current_attempt_state(authority);
    let mut constraints = if retained_failed {
        Vec::new()
    } else {
        scene
            .constraint_entries
            .iter()
            .map(|entry| {
                Ok(json!({
                    "glyph": format!("{:?}", entry.glyph),
                    "operands": entry.operands.iter().copied().map(|item| index.item(item)).collect::<Result<Vec<_>, _>>()?,
                    "suppressed": entry.suppressed,
                }))
            })
            .collect::<Result<Vec<_>, String>>()?
    };
    constraints.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());
    let datums = scene.datums.iter().map(|datum| json!({
        "datum": format!("{:?}", datum.datum),
        "origin": datum.model_origin,
        "direction": datum.model_direction,
        "screenStart": [datum.screen_start.x, datum.screen_start.y],
        "screenEnd": [datum.screen_end.x, datum.screen_end.y],
    })).collect::<Vec<_>>();
    Ok(json!({
        "authority": {
            "currentAttemptAccepted": current,
            "retainedFailed": retained_failed,
        },
        "viewport": {
            "screenSize": scene.viewport.screen_size,
            "modelCenter": scene.viewport.model_center,
            "pixelsPerModelUnit": scene.viewport.pixels_per_model_unit,
        },
        "points": points,
        "curves": curves,
        "datums": datums,
        "computedCurves": computed,
        "filletAffordances": affordances,
        "computedContinuationLimitCount": scene.computed_fillet_continuation_statuses.len(),
        "constraints": constraints,
    }))
}

fn authority_documents(
    authority: &WorkbenchDocumentAuthority,
) -> Result<(&SketchDocument, &ComputedFeatureDocument), String> {
    match authority {
        WorkbenchDocumentAuthority::Flat(coordinator) => {
            let session = coordinator
                .visible_preview_session()
                .unwrap_or(coordinator.session());
            let accepted = session
                .accepted_state()
                .ok_or_else(|| "flat scene has no accepted document".to_owned())?;
            Ok((accepted.document(), coordinator.feature_document()))
        }
        WorkbenchDocumentAuthority::Projectional { editor, .. } => {
            let accepted = editor
                .coordinator()
                .accepted_materialization()
                .ok_or_else(|| "projectional scene has no accepted materialization".to_owned())?;
            let document = accepted
                .session
                .accepted_state()
                .ok_or_else(|| "projectional scene has no accepted document".to_owned())?
                .document();
            Ok((document, &accepted.features))
        }
    }
}

fn screen_points(points: &[geosolve_constraint_editor::ScreenPoint]) -> Vec<[f64; 2]> {
    points.iter().map(|point| [point.x, point.y]).collect()
}

fn compare_values(
    expected: &Value,
    actual: &Value,
    tolerance: f64,
    path: &str,
) -> Result<(), String> {
    match (expected, actual) {
        (Value::Null, Value::Null) => Ok(()),
        (Value::Bool(expected), Value::Bool(actual)) if expected == actual => Ok(()),
        (Value::String(expected), Value::String(actual)) if expected == actual => Ok(()),
        (Value::Number(expected), Value::Number(actual)) => {
            let expected = expected
                .as_f64()
                .ok_or_else(|| format!("{path}: expected number is outside f64"))?;
            let actual = actual
                .as_f64()
                .ok_or_else(|| format!("{path}: actual number is outside f64"))?;
            let allowed = tolerance * expected.abs().max(actual.abs()).max(1.0);
            if expected.is_finite()
                && actual.is_finite()
                && (expected - actual).abs() <= allowed
            {
                Ok(())
            } else {
                Err(format!(
                    "{path}: expected {expected}, got {actual}, tolerance {allowed}"
                ))
            }
        }
        (Value::Array(expected), Value::Array(actual)) => {
            if expected.len() != actual.len() {
                return Err(format!(
                    "{path}: expected {} rows, got {}",
                    expected.len(),
                    actual.len()
                ));
            }
            for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
                compare_values(expected, actual, tolerance, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        (Value::Object(expected), Value::Object(actual)) => {
            compare_objects(expected, actual, tolerance, path)
        }
        _ => Err(format!("{path}: expected {expected:?}, got {actual:?}")),
    }
}

fn compare_objects(
    expected: &Map<String, Value>,
    actual: &Map<String, Value>,
    tolerance: f64,
    path: &str,
) -> Result<(), String> {
    let expected_keys = expected.keys().collect::<BTreeSet<_>>();
    let actual_keys = actual.keys().collect::<BTreeSet<_>>();
    if expected_keys != actual_keys {
        return Err(format!(
            "{path}: object keys differ: expected {expected_keys:?}, got {actual_keys:?}"
        ));
    }
    for key in expected_keys {
        compare_values(
            &expected[key],
            &actual[key],
            tolerance,
            &format!("{path}.{key}"),
        )?;
    }
    Ok(())
}

fn panic_payload(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".into()
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .take(1_024)
        .collect()
}

const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}
