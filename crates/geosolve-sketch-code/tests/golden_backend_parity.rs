// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(not(target_arch = "wasm32"))]
#![allow(
    clippy::too_many_lines,
    reason = "one process-isolated adapter keeps export, managed execution, semantic comparison, and lifecycle checks together"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DocumentBSplineForm,
    DocumentDirectedProfileOffsetCurve, DocumentId, DocumentLineSide, DocumentOffsetTraversal,
    DocumentProfileOffsetChain, DocumentProfileOffsetEdgePair, DocumentProfileOffsetOperand,
    DocumentProfileOffsetTerminalPolicy, DocumentSolveRequest, OperationControl, OperationOutcome,
    PersistentId, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_code::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringAvailability, CodeAuthoringDeclarationKind, CodeProject,
    CompiledManagedSource, KeyedReconcileState, ManagedSketchExportError, MaterializedCodeProject,
    ProjectKey, SketchCodeSession, export_sketch_document_to_managed_source,
    export_sketch_document_with_features_to_managed_source, materialize_code_project_cold,
    required_generated_members,
};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedEdge, ComputedEdgeGeometry, ComputedEdgeProvenance,
    ComputedEvaluationAllocator, ComputedFeatureCornerId, ComputedFeatureDefinition,
    ComputedFeatureDocument, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureId, ComputedFeatureSnapshot,
    ComputedFilletParent, NativeCurveSpanSource,
};
use geosolve_sketch_intent::{ConstraintKind, IntentSessionId, intent_content_digest};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

const MANIFEST_FORMAT: &str = "geosolve-golden-native-parity-v1";
const FILLET_MANIFEST_FORMAT: &str = "geosolve-golden-fillet-parity-v1";
const EXCLUSION_LEDGER: &str = include_str!("fixtures/golden_backend_parity_exclusions.tsv");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeParityManifest {
    format: String,
    case_id: String,
    family: String,
    lifecycle: String,
    stages: Vec<NativeParityStage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeParityStage {
    name: String,
    design_json: String,
    accepted_json: String,
    diagnostics: Value,
    history_len: usize,
    history_cursor: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FilletParityManifest {
    format: String,
    case_id: String,
    cases: Vec<FilletParityCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FilletParityCase {
    name: String,
    design_json: String,
    #[serde(default)]
    feature_json: Option<String>,
}

struct HarnessContext {
    manifest: PathBuf,
    directory: PathBuf,
    result: PathBuf,
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

#[test]
fn golden_backend_parity_exclusion_ledger_is_explicit_and_reviewable() {
    let rows = EXCLUSION_LEDGER
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.first().copied(),
        Some("case_pattern\tboundary\towning_contract\treason")
    );
    assert_eq!(
        rows.len(),
        5,
        "the reviewed ledger has exactly four exclusions"
    );
    let mut patterns = BTreeSet::new();
    for row in &rows[1..] {
        let columns = row.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 4, "malformed exclusion row: {row}");
        assert!(columns.iter().all(|column| !column.trim().is_empty()));
        assert!(patterns.insert(columns[0]), "duplicate exclusion pattern");
        assert!(columns[0].ends_with(".*"));
        assert!(columns[2].contains("::"));
        assert!(columns[3].ends_with('.'));
    }
    assert_eq!(
        patterns,
        BTreeSet::from([
            "constraint.external-line-collinear.*",
            "constraint.external-point-coincident.*",
            "dimension.profile-offset.*",
            "spline.noncanonical-knot-topology.*",
        ])
    );
    assert!(
        !patterns.iter().any(|pattern| pattern.contains("fillet")),
        "Fillet is a required parity family, never an exclusion"
    );
}

#[test]
fn golden_backend_parity_exclusions_reach_their_exact_owning_refusal_boundaries() {
    let gated = CODE_AUTHORING_FAMILIES
        .iter()
        .filter(|family| family.availability == CodeAuthoringAvailability::RequiresHostSnapshot)
        .map(|family| family.declaration)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        gated,
        BTreeSet::from([
            CodeAuthoringDeclarationKind::Constraint(ConstraintKind::ExternalLineCollinear),
            CodeAuthoringDeclarationKind::Constraint(ConstraintKind::ExternalPointCoincident),
        ]),
        "only the two reviewed external relations may stop at the host-snapshot gate",
    );

    let mut profile = SketchDocument::new(4.0).expect("Profile Offset document");
    let line = |document: &mut SketchDocument, label: &str, start: [f64; 2], end: [f64; 2]| {
        let start = document
            .add_point(format!("{label} start"), start)
            .expect("line start");
        let end = document
            .add_point(format!("{label} end"), end)
            .expect("line end");
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line")
    };
    let source = CurveSpan::line(line(&mut profile, "profile source", [0.0, 0.0], [4.0, 0.0]));
    let target = CurveSpan::line(line(&mut profile, "profile target", [0.0, 1.0], [4.0, 1.0]));
    let offset = profile
        .add_profile_offset(
            "profile offset",
            1.0,
            DocumentProfileOffsetOperand::OpenChain {
                side: DocumentLineSide::Left,
                chain: DocumentProfileOffsetChain {
                    edges: vec![DocumentProfileOffsetEdgePair {
                        source: DocumentDirectedProfileOffsetCurve {
                            curve: source,
                            traversal: DocumentOffsetTraversal::Forward,
                        },
                        target: DocumentDirectedProfileOffsetCurve {
                            curve: target,
                            traversal: DocumentOffsetTraversal::Forward,
                        },
                    }],
                    junctions: Vec::new(),
                    start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                    end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                },
            },
        )
        .expect("Profile Offset");
    assert!(matches!(
        export_sketch_document_to_managed_source(&profile),
        Err(ManagedSketchExportError::ProfileOffsetDimension { id })
            if id == offset.dimension.to_string()
    ));

    let mut spline = SketchDocument::new(2.0).expect("spline document");
    let controls = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]]
        .into_iter()
        .enumerate()
        .map(|(index, position)| {
            spline
                .add_point(format!("control {index}"), position)
                .expect("spline control")
        })
        .collect::<Vec<_>>();
    let curve = spline
        .add_curve(
            "canonical spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls,
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
                span_ids: vec![7, 11],
                next_span_id: 12,
            },
        )
        .expect("canonical spline");
    export_sketch_document_to_managed_source(&spline)
        .expect("canonical source recipe must remain supported");
    spline
        .insert_bspline_knot(curve, 0.5)
        .expect("geometry-preserving knot insertion");
    assert_eq!(
        export_sketch_document_to_managed_source(&spline),
        Err(ManagedSketchExportError::UnsupportedSplineTopology { id: curve }),
    );

    let proven = BTreeSet::from([
        "constraint.external-line-collinear.*",
        "constraint.external-point-coincident.*",
        "dimension.profile-offset.*",
        "spline.noncanonical-knot-topology.*",
    ]);
    let ledger = EXCLUSION_LEDGER
        .lines()
        .skip(2)
        .map(|row| row.split('\t').next().expect("exclusion pattern"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ledger, proven,
        "no unproved or unledgered boundary is allowed"
    );
}

#[test]
fn golden_backend_parity_export() {
    let Some(context) = HarnessContext::from_environment() else {
        return;
    };
    record_result(&context.result, catch_phase(|| export_phase(&context)));
}

#[test]
fn golden_backend_parity_validate() {
    let Some(context) = HarnessContext::from_environment() else {
        return;
    };
    record_result(&context.result, catch_phase(|| validate_phase(&context)));
}

#[test]
fn golden_fillet_backend_parity_export() {
    let Some(context) = HarnessContext::from_environment() else {
        return;
    };
    record_result(
        &context.result,
        catch_phase(|| export_fillet_phase(&context)),
    );
}

#[test]
fn golden_fillet_backend_parity_validate() {
    let Some(context) = HarnessContext::from_environment() else {
        return;
    };
    record_result(
        &context.result,
        catch_phase(|| validate_fillet_phase(&context)),
    );
}

impl HarnessContext {
    fn from_environment() -> Option<Self> {
        let manifest = env::var_os("GEOSOLVE_GOLDEN_PARITY_MANIFEST");
        let directory = env::var_os("GEOSOLVE_GOLDEN_PARITY_DIRECTORY");
        let result = env::var_os("GEOSOLVE_GOLDEN_PARITY_RESULT");
        if manifest.is_none() && directory.is_none() && result.is_none() {
            return None;
        }
        Some(Self {
            manifest: PathBuf::from(manifest.expect("parity manifest path is required")),
            directory: PathBuf::from(directory.expect("parity directory is required")),
            result: PathBuf::from(result.expect("parity result path is required")),
        })
    }
}

fn catch_phase(run: impl FnOnce() -> Result<(), String>) -> PhaseResult {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(())) => PhaseResult::Pass,
        Ok(Err(message)) => PhaseResult::Defect(message),
        Err(payload) => PhaseResult::Panic(panic_payload(&payload)),
    }
}

enum PhaseResult {
    Pass,
    Defect(String),
    Panic(String),
}

fn record_result(path: &Path, result: PhaseResult) {
    let (status, class, detail) = match result {
        PhaseResult::Pass => ("PASS", "-", "ok".to_owned()),
        PhaseResult::Defect(message) => ("DEFECT", "backend-parity", message),
        PhaseResult::Panic(message) => ("PANIC", "backend-parity-panic", message),
    };
    let detail = sanitize(&detail);
    let fingerprint = if status == "PASS" {
        detail
    } else {
        format!("{:016x}:{detail}", fnv1a64(detail.as_bytes()))
    };
    fs::write(path, format!("{status}\t{class}\t{fingerprint}\n"))
        .unwrap_or_else(|error| panic!("cannot write parity result {}: {error}", path.display()));
}

fn read_manifest(path: &Path) -> Result<NativeParityManifest, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read parity manifest {}: {error}", path.display()))?;
    let manifest: NativeParityManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid parity manifest {}: {error}", path.display()))?;
    if manifest.format != MANIFEST_FORMAT {
        return Err(format!(
            "unsupported parity manifest format `{}`",
            manifest.format
        ));
    }
    if manifest.case_id.is_empty() || manifest.family.is_empty() {
        return Err("parity manifest has an empty case or family".into());
    }
    let expected_names: &[&str] = match manifest.lifecycle.as_str() {
        "create-undo-redo" => &["base", "created", "undo", "redo"],
        "create-edit-undo-redo" => &["base", "created", "edited", "undo", "redo"],
        other => return Err(format!("unsupported parity lifecycle `{other}`")),
    };
    let actual_names = manifest
        .stages
        .iter()
        .map(|stage| stage.name.as_str())
        .collect::<Vec<_>>();
    if actual_names != expected_names {
        return Err(format!(
            "parity lifecycle `{}` has stages {actual_names:?}, expected {expected_names:?}",
            manifest.lifecycle
        ));
    }
    Ok(manifest)
}

fn read_fillet_manifest(path: &Path) -> Result<FilletParityManifest, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "cannot read Fillet parity manifest {}: {error}",
            path.display()
        )
    })?;
    let manifest: FilletParityManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid Fillet parity manifest {}: {error}", path.display()))?;
    if manifest.format != FILLET_MANIFEST_FORMAT {
        return Err(format!(
            "unsupported Fillet parity manifest format `{}`",
            manifest.format
        ));
    }
    if manifest.case_id.is_empty() || manifest.cases.is_empty() {
        return Err("Fillet parity manifest has no case identity or variants".into());
    }
    let mut names = BTreeSet::new();
    for case in &manifest.cases {
        if case.name.is_empty() || !names.insert(case.name.as_str()) {
            return Err("Fillet parity variant name is empty or duplicated".into());
        }
    }
    Ok(manifest)
}

fn export_phase(context: &HarnessContext) -> Result<(), String> {
    let manifest = read_manifest(&context.manifest)?;
    fs::create_dir_all(&context.directory).map_err(|error| {
        format!(
            "cannot create parity directory {}: {error}",
            context.directory.display()
        )
    })?;
    let mut entries = Vec::new();
    let mut source_owners = BTreeMap::new();
    for (stage_index, stage) in manifest.stages.iter().enumerate() {
        let document = SketchDocument::from_draft_v5_json(&stage.design_json).map_err(|error| {
            format!(
                "{} stage `{}` native document is invalid: {error}",
                manifest.case_id, stage.name
            )
        })?;
        let source = export_sketch_document_to_managed_source(&document).map_err(|error| {
            format!(
                "{} stage `{}` cannot project to managed source: {error}",
                manifest.case_id, stage.name
            )
        })?;
        let source_path = context
            .directory
            .join(format!("stage-{stage_index}.sketch.ts"));
        fs::write(&source_path, source.as_bytes()).map_err(|error| {
            format!(
                "cannot write managed source {}: {error}",
                source_path.display()
            )
        })?;
        if source_owners.insert(source.clone(), stage_index).is_none() {
            entries.push(CompileBatchRequestEntry {
                id: stage_index,
                source,
            });
        }
    }
    let request = serde_json::to_vec(&CompileBatchRequest { entries })
        .map_err(|error| format!("cannot encode batched compiler request: {error}"))?;
    fs::write(
        context.directory.join("compile-batch.request.json"),
        request,
    )
    .map_err(|error| format!("cannot write batched compiler request: {error}"))?;
    Ok(())
}

fn export_fillet_phase(context: &HarnessContext) -> Result<(), String> {
    let manifest = read_fillet_manifest(&context.manifest)?;
    fs::create_dir_all(&context.directory).map_err(|error| {
        format!(
            "cannot create Fillet parity directory {}: {error}",
            context.directory.display()
        )
    })?;
    let mut entries = Vec::new();
    let mut source_owners = BTreeMap::new();
    for (index, case) in manifest.cases.iter().enumerate() {
        let document = SketchDocument::from_draft_v5_json(&case.design_json).map_err(|error| {
            format!(
                "{} variant `{}` native document is invalid: {error}",
                manifest.case_id, case.name
            )
        })?;
        let source = match &case.feature_json {
            Some(json) => {
                let features = ComputedFeatureDocument::from_json(json).map_err(|error| {
                    format!(
                        "{} variant `{}` feature intent is invalid: {error}",
                        manifest.case_id, case.name
                    )
                })?;
                export_sketch_document_with_features_to_managed_source(&document, &features)
            }
            None => export_sketch_document_to_managed_source(&document),
        }
        .map_err(|error| {
            format!(
                "{} variant `{}` cannot project to managed source: {error}",
                manifest.case_id, case.name
            )
        })?;
        let source_path = context.directory.join(format!("stage-{index}.sketch.ts"));
        fs::write(&source_path, source.as_bytes()).map_err(|error| {
            format!(
                "cannot write managed source {}: {error}",
                source_path.display()
            )
        })?;
        if source_owners.insert(source.clone(), index).is_none() {
            entries.push(CompileBatchRequestEntry { id: index, source });
        }
    }
    let request = serde_json::to_vec(&CompileBatchRequest { entries })
        .map_err(|error| format!("cannot encode Fillet compiler request: {error}"))?;
    fs::write(
        context.directory.join("compile-batch.request.json"),
        request,
    )
    .map_err(|error| format!("cannot write Fillet compiler request: {error}"))
}

fn validate_phase(context: &HarnessContext) -> Result<(), String> {
    let manifest = read_manifest(&context.manifest)?;
    validate_native_lifecycle_shape(&manifest)?;

    let compile_output = fs::read(context.directory.join("compile-batch.output.json"))
        .map_err(|error| format!("cannot read batched compiler output: {error}"))?;
    let compile_output: CompileBatchOutput = serde_json::from_slice(&compile_output)
        .map_err(|error| format!("invalid batched compiler output: {error}"))?;
    let mut compiled_by_stage = BTreeMap::new();
    for entry in compile_output.entries {
        match (entry.compiled, entry.error) {
            (Some(compiled), None) => {
                if compiled_by_stage.insert(entry.id, compiled).is_some() {
                    return Err(format!("compiler returned duplicate stage {}", entry.id));
                }
            }
            (None, Some(error)) => {
                return Err(format!(
                    "managed compile stage {} failed: {}: {}{}",
                    entry.id,
                    error.name,
                    error.message,
                    error
                        .span
                        .map_or_else(String::new, |span| format!(" at {span}")),
                ));
            }
            _ => return Err(format!("compiler stage {} has ambiguous outcome", entry.id)),
        }
    }

    let mut projects = Vec::with_capacity(manifest.stages.len());
    let mut materialized = Vec::with_capacity(manifest.stages.len());
    let mut source_owners = BTreeMap::new();
    for (stage_index, stage) in manifest.stages.iter().enumerate() {
        let native_design =
            SketchDocument::from_draft_v5_json(&stage.design_json).map_err(|error| {
                format!(
                    "{} `{}` design decode failed: {error}",
                    manifest.case_id, stage.name
                )
            })?;
        let source_path = context
            .directory
            .join(format!("stage-{stage_index}.sketch.ts"));
        let source = fs::read_to_string(&source_path).map_err(|error| {
            format!(
                "cannot read exported stage {}: {error}",
                source_path.display()
            )
        })?;
        let compiled_owner = *source_owners.entry(source.clone()).or_insert(stage_index);
        let compiled_json = compiled_by_stage.get(&compiled_owner).ok_or_else(|| {
            format!("compiler omitted unique source owned by stage {compiled_owner}")
        })?;
        let compiled_json = serde_json::to_string(compiled_json)
            .map_err(|error| format!("cannot encode compiled stage {compiled_owner}: {error}"))?;
        let compiled = CompiledManagedSource::from_json(&compiled_json).map_err(|error| {
            format!(
                "{} `{}` compiler envelope rejected: {error}",
                manifest.case_id, stage.name
            )
        })?;
        if compiled.input_source_digest != intent_content_digest(source.as_bytes()).to_string() {
            return Err(format!(
                "{} `{}` compiler did not authenticate the exported source bytes",
                manifest.case_id, stage.name
            ));
        }
        let project = CodeProject::managed(
            ProjectKey(format!("golden-parity-{}", manifest.case_id)),
            compiled,
        )
        .map_err(|error| {
            format!(
                "{} `{}` project rejected: {error}",
                manifest.case_id, stage.name
            )
        })?;
        let generated = reconciled(&project)?;
        let candidate = materialize(
            &manifest.case_id,
            stage_index,
            &project,
            &generated,
            native_design.model_scale(),
        )?;
        validate_stage(
            &manifest,
            stage,
            stage_index,
            &native_design,
            &source,
            &candidate,
            &context.directory,
        )?;
        projects.push(project);
        materialized.push((generated, candidate));
    }
    validate_code_lifecycle(&manifest, &projects, &materialized)
}

fn validate_fillet_phase(context: &HarnessContext) -> Result<(), String> {
    let manifest = read_fillet_manifest(&context.manifest)?;
    let compiled_by_stage = read_compiled_stages(&context.directory)?;
    let mut source_owners = BTreeMap::new();
    for (index, case) in manifest.cases.iter().enumerate() {
        let native_design =
            SketchDocument::from_draft_v5_json(&case.design_json).map_err(|error| {
                format!(
                    "{} `{}` native design decode failed: {error}",
                    manifest.case_id, case.name
                )
            })?;
        let source_path = context.directory.join(format!("stage-{index}.sketch.ts"));
        let source = fs::read_to_string(&source_path).map_err(|error| {
            format!(
                "cannot read exported Fillet source {}: {error}",
                source_path.display()
            )
        })?;
        let compiled_owner = *source_owners.entry(source.clone()).or_insert(index);
        let compiled_json = compiled_by_stage.get(&compiled_owner).ok_or_else(|| {
            format!("compiler omitted unique Fillet source owned by variant {compiled_owner}")
        })?;
        let compiled_json = serde_json::to_string(compiled_json)
            .map_err(|error| format!("cannot encode compiled Fillet source: {error}"))?;
        let compiled = CompiledManagedSource::from_json(&compiled_json).map_err(|error| {
            format!(
                "{} `{}` compiler envelope rejected: {error}",
                manifest.case_id, case.name
            )
        })?;
        if compiled.input_source_digest != intent_content_digest(source.as_bytes()).to_string() {
            return Err(format!(
                "{} `{}` compiler did not authenticate the exported source bytes",
                manifest.case_id, case.name
            ));
        }
        let project = CodeProject::managed(
            ProjectKey(format!("golden-fillet-parity-{}-{index}", manifest.case_id)),
            compiled,
        )
        .map_err(|error| {
            format!(
                "{} `{}` project rejected: {error}",
                manifest.case_id, case.name
            )
        })?;
        let generated = reconciled(&project)?;
        let managed = materialize(
            &format!("{}-{}", manifest.case_id, case.name),
            index,
            &project,
            &generated,
            native_design.model_scale(),
        )?;
        validate_fillet_variant(
            &manifest.case_id,
            case,
            &native_design,
            &managed,
            &context.directory,
            index,
        )?;
    }
    Ok(())
}

fn read_compiled_stages(directory: &Path) -> Result<BTreeMap<usize, Value>, String> {
    let bytes = fs::read(directory.join("compile-batch.output.json"))
        .map_err(|error| format!("cannot read batched compiler output: {error}"))?;
    let output: CompileBatchOutput = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid batched compiler output: {error}"))?;
    let mut compiled = BTreeMap::new();
    for entry in output.entries {
        match (entry.compiled, entry.error) {
            (Some(value), None) => {
                if compiled.insert(entry.id, value).is_some() {
                    return Err(format!("compiler returned duplicate stage {}", entry.id));
                }
            }
            (None, Some(error)) => {
                return Err(format!(
                    "managed compile stage {} failed: {}: {}{}",
                    entry.id,
                    error.name,
                    error.message,
                    error
                        .span
                        .map_or_else(String::new, |span| format!(" at {span}")),
                ));
            }
            _ => return Err(format!("compiler stage {} has ambiguous outcome", entry.id)),
        }
    }
    Ok(compiled)
}

fn validate_fillet_variant(
    case_id: &str,
    case: &FilletParityCase,
    native_design: &SketchDocument,
    managed: &MaterializedCodeProject,
    diagnostic_directory: &Path,
    index: usize,
) -> Result<(), String> {
    let accepted = managed
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| {
            format!(
                "{case_id} `{}` has no managed accepted authority",
                case.name
            )
        })?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(format!(
            "{case_id} `{}` managed validation is incomplete: {:?}",
            case.name, accepted.validation
        ));
    }

    let native_session = RetainedSketchDocumentSession::new(
        native_design.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .map_err(|error| format!("{case_id} `{}` native solve failed: {error}", case.name))?;
    let native_accepted = native_session
        .accepted_state_for_current_input()
        .ok_or_else(|| format!("{case_id} `{}` native acceptance is not current", case.name))?;
    let native_solve = native_accepted
        .diagnostics()
        .solve
        .ok_or_else(|| format!("{case_id} `{}` native solve has no diagnostics", case.name))?;
    if !native_solve.hard_residuals_validated
        || native_solve
            .maximum_normalized_hard_residual
            .is_none_or(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(format!(
            "{case_id} `{}` native success lacks independent residual validation",
            case.name
        ));
    }

    let managed_design_json = accepted
        .session
        .design_document()
        .to_draft_v5_json()
        .map_err(|error| format!("managed Fillet design encoding failed: {error}"))?;
    let (native_design_semantic, native_colors) = semantic_document(&case.design_json)?;
    let (managed_design_semantic, managed_colors) = semantic_document(&managed_design_json)?;
    compare_values(
        &native_design_semantic,
        &managed_design_semantic,
        1.0e-12,
        "$",
    )
    .map_err(
        |detail| {
            let native_path = diagnostic_directory.join(format!("native-fillet-{index}.json"));
            let managed_path = diagnostic_directory.join(format!("managed-fillet-{index}.json"));
            let native_raw_path =
                diagnostic_directory.join(format!("native-fillet-{index}.raw.json"));
            let managed_raw_path =
                diagnostic_directory.join(format!("managed-fillet-{index}.raw.json"));
            let _ = fs::write(
                &native_path,
                serde_json::to_vec_pretty(&native_design_semantic).unwrap_or_default(),
            );
            let _ = fs::write(
                &managed_path,
                serde_json::to_vec_pretty(&managed_design_semantic).unwrap_or_default(),
            );
            let _ = fs::write(&native_raw_path, case.design_json.as_bytes());
            let _ = fs::write(&managed_raw_path, managed_design_json.as_bytes());
            format!(
                "{case_id} `{}` Fillet design semantics differ: {detail} (native {}, managed {}; raw native {}, managed {})",
                case.name,
                native_path.display(),
                managed_path.display(),
                native_raw_path.display(),
                managed_raw_path.display(),
            )
        },
    )?;
    let native_accepted_json = native_accepted
        .document()
        .to_draft_v5_json()
        .map_err(|error| format!("native Fillet accepted encoding failed: {error}"))?;
    let managed_accepted_json = accepted
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| format!("{case_id} `{}` managed input is not current", case.name))?
        .document()
        .to_draft_v5_json()
        .map_err(|error| format!("managed Fillet accepted encoding failed: {error}"))?;
    compare_values(
        &semantic_document_with_colors(&native_accepted_json, &native_colors)?,
        &semantic_document_with_colors(&managed_accepted_json, &managed_colors)?,
        1.0e-9 * native_design.model_scale().max(1.0),
        "$",
    )
    .map_err(|detail| {
        format!(
            "{case_id} `{}` Fillet accepted geometry differs: {detail}",
            case.name
        )
    })?;

    let native_features = match &case.feature_json {
        Some(json) => ComputedFeatureDocument::from_json(json)
            .map_err(|error| format!("native Fillet feature decode failed: {error}"))?,
        None => ComputedFeatureDocument::new(native_design.id()),
    };
    let native_feature_semantics = feature_document_observation(&native_features, &native_colors)?;
    let managed_feature_semantics =
        feature_document_observation(&accepted.features, &managed_colors)?;
    compare_values(
        &native_feature_semantics,
        &managed_feature_semantics,
        1.0e-12,
        "$.features",
    )
    .map_err(|detail| format!("{case_id} `{}` Fillet intent differs: {detail}", case.name))?;

    let native_computed = evaluate_features(&native_session, &native_features)?;
    let native_snapshot =
        computed_snapshot_observation(&native_computed, &native_features, &native_colors)?;
    let managed_snapshot =
        computed_snapshot_observation(&accepted.computed, &accepted.features, &managed_colors)?;
    compare_values(
        &native_snapshot,
        &managed_snapshot,
        2.0e-9 * native_design.model_scale().max(1.0),
        "$.computed",
    )
    .map_err(|detail| {
        format!(
            "{case_id} `{}` computed Fillet geometry/provenance differs: {detail}",
            case.name
        )
    })
}

fn evaluate_features(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
) -> Result<ComputedFeatureSnapshot, String> {
    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        session,
        features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .map_err(|error| format!("computed Fillet capture failed: {error}"))?;
    let outcome = snapshot
        .prepare(&mut ComputedEvaluationAllocator::default())
        .map_err(|error| format!("computed Fillet preparation failed: {error}"))?
        .execute(OperationControl::unlimited())
        .map_err(|error| format!("computed Fillet evaluation failed: {error}"))?;
    match outcome {
        OperationOutcome::Completed { value, .. } => Ok(value),
        other => Err(format!(
            "computed Fillet evaluation did not complete: {other:?}"
        )),
    }
}

struct FeatureOrdinals {
    features: BTreeMap<ComputedFeatureId, usize>,
    corners: BTreeMap<ComputedFeatureCornerId, (usize, usize)>,
}

fn feature_ordinals(features: &ComputedFeatureDocument) -> Result<FeatureOrdinals, String> {
    let mut feature_ids = BTreeMap::new();
    let mut corner_ids = BTreeMap::new();
    for (feature_index, feature) in features.features().iter().enumerate() {
        if feature_ids.insert(feature.id, feature_index).is_some() {
            return Err("computed feature identity is duplicated".into());
        }
        let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
        for (corner_index, corner) in fillet.corners.iter().enumerate() {
            if corner_ids
                .insert(corner.id, (feature_index, corner_index))
                .is_some()
            {
                return Err("computed Fillet corner identity is duplicated".into());
            }
        }
    }
    Ok(FeatureOrdinals {
        features: feature_ids,
        corners: corner_ids,
    })
}

fn feature_document_observation(
    features: &ComputedFeatureDocument,
    colors: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let mut rows = Vec::new();
    for feature in features.features() {
        let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
        let corners = fillet
            .corners
            .iter()
            .map(|corner| {
                Ok(json!({
                    "parents": [
                        fillet_parent_observation(corner.first, colors)?,
                        fillet_parent_observation(corner.second, colors)?,
                    ],
                    "endpointOrder": format!("{:?}", corner.endpoint_order),
                    "sweep": format!("{:?}", corner.sweep),
                }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        rows.push(json!({
            "suppressed": feature.suppressed,
            "radius": fillet.radius,
            "corners": corners,
        }));
    }
    Ok(Value::Array(rows))
}

fn fillet_parent_observation(
    parent: ComputedFilletParent,
    colors: &BTreeMap<String, String>,
) -> Result<Value, String> {
    Ok(json!({
        "source": source_observation(parent.source, colors)?,
        "parameter": parent.picked_parameter,
        "winding": parent.winding,
        "neighborhood": match parent.neighborhood {
            ContactNeighborhood::Interior => json!({ "kind": "interior" }),
            ContactNeighborhood::Start => json!({ "kind": "start" }),
            ContactNeighborhood::End => json!({ "kind": "end" }),
            ContactNeighborhood::Local { lower, upper } => {
                json!({ "kind": "local", "lower": lower, "upper": upper })
            }
        },
        "normalSide": format!("{:?}", parent.normal_side),
        "trimEndpoint": format!("{:?}", parent.retained_endpoint),
        "periodicAnchor": parent.periodic_anchor.map(|anchor| json!({
            "parameter": anchor.parameter,
            "winding": anchor.winding,
        })),
    }))
}

fn source_observation(
    source: NativeCurveSpanSource,
    colors: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let curve = colors.get(&source.span.curve.to_string()).ok_or_else(|| {
        format!(
            "computed feature source {:?} has no semantic native curve",
            source.span
        )
    })?;
    Ok(json!({ "curve": curve, "segment": source.span.segment }))
}

fn corner_ref_observation(
    owner: ComputedCornerRef,
    ordinals: &FeatureOrdinals,
) -> Result<Value, String> {
    let feature = ordinals
        .features
        .get(&owner.feature)
        .copied()
        .ok_or_else(|| {
            format!(
                "computed snapshot references unknown feature {:?}",
                owner.feature
            )
        })?;
    let (corner_feature, corner) =
        ordinals
            .corners
            .get(&owner.corner)
            .copied()
            .ok_or_else(|| {
                format!(
                    "computed snapshot references unknown corner {:?}",
                    owner.corner
                )
            })?;
    if feature != corner_feature {
        return Err("computed snapshot corner belongs to a different feature".into());
    }
    Ok(json!({ "feature": feature, "corner": corner }))
}

fn computed_snapshot_observation(
    snapshot: &ComputedFeatureSnapshot,
    features: &ComputedFeatureDocument,
    colors: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let ordinals = feature_ordinals(features)?;
    let mut evaluations = Vec::new();
    for evaluation in snapshot.feature_evaluations() {
        let feature = ordinals
            .features
            .get(&evaluation.feature)
            .copied()
            .ok_or_else(|| {
                format!(
                    "computed evaluation references unknown feature {:?}",
                    evaluation.feature
                )
            })?;
        let state = match &evaluation.state {
            ComputedFeatureEvaluationState::Current { corner_edges } => {
                let mut corners = corner_edges
                    .iter()
                    .map(|(corner, _)| {
                        ordinals
                            .corners
                            .get(corner)
                            .map(|(_, corner)| *corner)
                            .ok_or_else(|| {
                                format!("Current evaluation references unknown corner {corner:?}")
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                corners.sort_unstable();
                json!({ "kind": "current", "corners": corners })
            }
            ComputedFeatureEvaluationState::Suppressed => json!({ "kind": "suppressed" }),
            ComputedFeatureEvaluationState::Failed { failure } => {
                json!({ "kind": "failed", "failure": format!("{failure:?}") })
            }
        };
        evaluations.push(json!({ "feature": feature, "state": state }));
    }
    evaluations.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());

    let mut edges = snapshot
        .edges()
        .iter()
        .map(|edge| computed_edge_observation(edge, &ordinals, colors))
        .collect::<Result<Vec<_>, _>>()?;
    edges.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());

    let mut construction = snapshot
        .construction_fragments()
        .iter()
        .map(|fragment| {
            Ok(json!({
                "source": source_observation(fragment.source, colors)?,
                "interval": [fragment.interval.start, fragment.interval.end],
                "sourceRole": format!("{:?}", fragment.source_role),
                "provenance": {
                    "owner": corner_ref_observation(fragment.provenance.owner, &ordinals)?,
                    "endpoint": format!("{:?}", fragment.provenance.endpoint),
                    "baseInterval": [
                        fragment.provenance.base_interval.start,
                        fragment.provenance.base_interval.end,
                    ],
                },
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    construction.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());

    let mut replaced = snapshot
        .replaced_sources()
        .iter()
        .copied()
        .map(|source| source_observation(source, colors))
        .collect::<Result<Vec<_>, _>>()?;
    replaced.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());
    Ok(json!({
        "evaluations": evaluations,
        "edges": edges,
        "constructionFragments": construction,
        "replacedSources": replaced,
    }))
}

fn computed_edge_observation(
    edge: &ComputedEdge,
    ordinals: &FeatureOrdinals,
    colors: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let geometry = match &edge.geometry {
        ComputedEdgeGeometry::NativeSourceFragment { source, interval } => json!({
            "kind": "sourceFragment",
            "source": source_observation(*source, colors)?,
            "interval": [interval.start, interval.end],
        }),
        ComputedEdgeGeometry::CircularArc(arc) => json!({
            "kind": "circularArc",
            "center": arc.center,
            "radius": arc.radius,
            "startAngle": arc.start_angle,
            "endAngle": arc.end_angle,
            "sweep": format!("{:?}", arc.sweep),
            "contacts": arc.contacts.iter().map(|contact| {
                Ok(json!({
                    "source": source_observation(contact.source, colors)?,
                    "parameter": contact.parameter,
                    "winding": contact.winding,
                    "totalParameter": contact.total_parameter,
                    "position": contact.position,
                }))
            }).collect::<Result<Vec<_>, String>>()?,
            "tangentOrientations": arc.tangent_orientations.map(|value| format!("{value:?}")),
        }),
        other => return Err(format!("unsupported computed edge geometry: {other:?}")),
    };
    let provenance = match &edge.provenance {
        ComputedEdgeProvenance::SourceFragment {
            source,
            interval,
            start_claim,
            end_claim,
        } => json!({
            "kind": "sourceFragment",
            "source": source_observation(*source, colors)?,
            "interval": [interval.start, interval.end],
            "startClaim": start_claim.map(|owner| corner_ref_observation(owner, ordinals)).transpose()?,
            "endClaim": end_claim.map(|owner| corner_ref_observation(owner, ordinals)).transpose()?,
        }),
        ComputedEdgeProvenance::FilletArc { owner, sources } => json!({
            "kind": "filletArc",
            "owner": corner_ref_observation(*owner, ordinals)?,
            "sources": [
                source_observation(sources[0], colors)?,
                source_observation(sources[1], colors)?,
            ],
        }),
        other => return Err(format!("unsupported computed edge provenance: {other:?}")),
    };
    Ok(json!({
        "role": format!("{:?}", edge.role),
        "geometry": geometry,
        "provenance": provenance,
    }))
}

fn validate_native_lifecycle_shape(manifest: &NativeParityManifest) -> Result<(), String> {
    let base = &manifest.stages[0];
    let created = &manifest.stages[1];
    if created.history_len != base.history_len + 1
        || created.history_cursor != base.history_cursor + 1
    {
        return Err(format!(
            "{} native create did not advance history exactly once",
            manifest.case_id
        ));
    }
    let (edited, undo, redo) = if manifest.lifecycle == "create-edit-undo-redo" {
        let edited = &manifest.stages[2];
        if edited.history_len != created.history_len + 1
            || edited.history_cursor != created.history_cursor + 1
        {
            return Err(format!(
                "{} native edit did not advance history exactly once",
                manifest.case_id
            ));
        }
        (Some(edited), &manifest.stages[3], &manifest.stages[4])
    } else {
        (None, &manifest.stages[2], &manifest.stages[3])
    };
    let terminal = edited.unwrap_or(created);
    let undo_expected = if edited.is_some() { created } else { base };
    if undo.history_len != terminal.history_len
        || undo.history_cursor + 1 != terminal.history_cursor
        || redo.history_len != terminal.history_len
        || redo.history_cursor != terminal.history_cursor
    {
        return Err(format!(
            "{} native Undo/Redo history cursor shape diverged",
            manifest.case_id
        ));
    }
    compare_document_json(
        &undo_expected.design_json,
        &undo.design_json,
        0.0,
        "native Undo design",
    )?;
    compare_document_json(
        &undo_expected.accepted_json,
        &undo.accepted_json,
        1.0e-12,
        "native Undo accepted geometry",
    )?;
    compare_document_json(
        &terminal.design_json,
        &redo.design_json,
        0.0,
        "native Redo design",
    )?;
    compare_document_json(
        &terminal.accepted_json,
        &redo.accepted_json,
        1.0e-12,
        "native Redo accepted geometry",
    )
}

fn reconciled(project: &CodeProject) -> Result<KeyedReconcileState, String> {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project)
                .map_err(|error| format!("generated inventory failed: {error}"))?,
            &BTreeSet::new(),
        )
        .map(|plan| plan.into_staged())
        .map_err(|error| format!("generated reconciliation failed: {error}"))
}

fn materialize(
    case_id: &str,
    stage_index: usize,
    project: &CodeProject,
    generated: &KeyedReconcileState,
    model_scale: f64,
) -> Result<MaterializedCodeProject, String> {
    let hash = u128::from(fnv1a64(case_id.as_bytes()));
    let raw = 0x91_0000_0000_0000_0000_0000_0000_0000_u128
        | (hash << 16)
        | u128::try_from(stage_index + 1).expect("stage index fits u128");
    materialize_code_project_cold(
        project,
        generated,
        IntentSessionId::from_raw(raw),
        DocumentId(PersistentId::from_u128(raw)),
        model_scale,
    )
    .map_err(|error| format!("managed materialization failed: {error}"))
}

fn validate_stage(
    manifest: &NativeParityManifest,
    stage: &NativeParityStage,
    stage_index: usize,
    native_design: &SketchDocument,
    _exported_source: &str,
    managed: &MaterializedCodeProject,
    diagnostic_directory: &Path,
) -> Result<(), String> {
    let accepted = managed
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| {
            format!(
                "{} `{}` has no managed accepted authority",
                manifest.case_id, stage.name
            )
        })?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(format!(
            "{} `{}` managed success lacks independent validation: {:?}",
            manifest.case_id, stage.name, accepted.validation
        ));
    }
    let managed_design = accepted.session.design_document();
    let roundtrip_source =
        export_sketch_document_to_managed_source(managed_design).map_err(|error| {
            format!(
                "{} `{}` managed design cannot reverse-project: {error}",
                manifest.case_id, stage.name
            )
        })?;
    let repeated_roundtrip = export_sketch_document_to_managed_source(managed_design)
        .map_err(|error| format!("managed repeat export failed: {error}"))?;
    if roundtrip_source != repeated_roundtrip {
        return Err(format!(
            "{} `{}` managed reverse projection is nondeterministic",
            manifest.case_id, stage.name,
        ));
    }
    let managed_design_json = managed_design
        .to_draft_v5_json()
        .map_err(|error| format!("managed design encoding failed: {error}"))?;
    let model_scale = native_design.model_scale();
    let (native_semantic_design, native_design_colors) = semantic_document(&stage.design_json)?;
    let (managed_semantic_design, managed_design_colors) = semantic_document(&managed_design_json)?;
    compare_values(&native_semantic_design, &managed_semantic_design, 0.0, "$").map_err(
        |detail| {
            let native_path = diagnostic_directory.join(format!("native-stage-{stage_index}.json"));
            let managed_path =
                diagnostic_directory.join(format!("managed-stage-{stage_index}.json"));
            let _ = fs::write(
                &native_path,
                serde_json::to_vec_pretty(&native_semantic_design).unwrap_or_default(),
            );
            let _ = fs::write(
                &managed_path,
                serde_json::to_vec_pretty(&managed_semantic_design).unwrap_or_default(),
            );
            format!(
                "{} `{}` design semantics differ: {detail} (native {}, managed {})",
                manifest.case_id,
                stage.name,
                native_path.display(),
                managed_path.display(),
            )
        },
    )?;
    let managed_accepted = accepted
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| {
            format!(
                "{} `{}` managed acceptance is not current",
                manifest.case_id, stage.name
            )
        })?;
    let managed_accepted_json = managed_accepted
        .document()
        .to_draft_v5_json()
        .map_err(|error| format!("managed accepted encoding failed: {error}"))?;
    let native_accepted =
        semantic_document_with_colors(&stage.accepted_json, &native_design_colors)?;
    let managed_accepted =
        semantic_document_with_colors(&managed_accepted_json, &managed_design_colors)?;
    compare_values(
        &native_accepted,
        &managed_accepted,
        1.0e-9 * model_scale.max(1.0),
        "$",
    )
    .map_err(|detail| {
        format!(
            "{} `{}` accepted geometry semantics differ: {detail}",
            manifest.case_id, stage.name,
        )
    })?;
    let managed_diagnostics = diagnostic_observation(&accepted.session)?;
    compare_values(
        &stage.diagnostics,
        &managed_diagnostics,
        1.0e-9,
        &format!("$.stages[{stage_index}].diagnostics"),
    )
    .map_err(|detail| {
        let native_path =
            diagnostic_directory.join(format!("native-diagnostics-{stage_index}.json"));
        let managed_path =
            diagnostic_directory.join(format!("managed-diagnostics-{stage_index}.json"));
        let _ = fs::write(
            &native_path,
            serde_json::to_vec_pretty(&stage.diagnostics).unwrap_or_default(),
        );
        let _ = fs::write(
            &managed_path,
            serde_json::to_vec_pretty(&managed_diagnostics).unwrap_or_default(),
        );
        format!(
            "{detail} (native {}, managed {})",
            native_path.display(),
            managed_path.display()
        )
    })
}

const DOCUMENT_NODE_ARRAYS: [&str; 6] = [
    "points",
    "scalars",
    "curves",
    "contacts",
    "constraints",
    "dimensions",
];

fn semantic_document(source: &str) -> Result<(Value, BTreeMap<String, String>), String> {
    let value: Value = serde_json::from_str(source)
        .map_err(|error| format!("semantic document JSON is invalid: {error}"))?;
    let colors = document_semantic_colors(&value)?;
    let normalized = normalize_semantic_document(value, &colors)?;
    Ok((normalized, colors))
}

fn semantic_document_with_colors(
    source: &str,
    colors: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let mut value: Value = serde_json::from_str(source)
        .map_err(|error| format!("accepted document JSON is invalid: {error}"))?;
    project_accepted_contact_parameters(source, &mut value)?;
    normalize_semantic_document(value, colors)
}

fn project_accepted_contact_parameters(source: &str, value: &mut Value) -> Result<(), String> {
    let document = SketchDocument::from_draft_v5_json(source)
        .map_err(|error| format!("accepted document cannot be evaluated: {error}"))?;
    let mut positions = BTreeMap::new();
    for contact in document.contacts() {
        let jet = document.evaluate_contact_jet(contact.id).map_err(|error| {
            format!(
                "accepted contact {} cannot be evaluated: {error}",
                contact.id.0
            )
        })?;
        jet.differential().map_err(|error| {
            format!(
                "accepted contact {} is not finite and regular: {error}",
                contact.id.0
            )
        })?;
        let position = [jet.position.x, jet.position.y];
        if position.iter().any(|coordinate| !coordinate.is_finite()) {
            return Err(format!(
                "accepted contact {} evaluated to a non-finite position",
                contact.id.0
            ));
        }
        if positions
            .insert(contact.parameter.0.to_string(), position)
            .is_some()
        {
            return Err(format!(
                "accepted contact parameter {} is owned by more than one contact",
                contact.parameter.0
            ));
        }
    }

    let scalars = value
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .and_then(|document| document.get_mut("scalars"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "accepted draft-v5 document has no `scalars` array".to_owned())?;
    for scalar in scalars {
        let Some(id) = scalar.get("id").and_then(Value::as_str) else {
            return Err("accepted scalar row has no persistent ID".into());
        };
        let Some(position) = positions.remove(id) else {
            continue;
        };
        scalar
            .as_object_mut()
            .expect("accepted scalar row with an ID is an object")
            .insert(
                "value".into(),
                json!({ "$evaluatedContactPosition": position }),
            );
    }
    if let Some((parameter, _)) = positions.into_iter().next() {
        return Err(format!(
            "accepted contact parameter {parameter} has no scalar row"
        ));
    }
    Ok(())
}

fn document_semantic_colors(value: &Value) -> Result<BTreeMap<String, String>, String> {
    let document = value
        .get("document")
        .and_then(Value::as_object)
        .ok_or_else(|| "draft-v5 document wrapper has no document object".to_owned())?;
    let mut nodes = BTreeMap::<String, (&str, &Value)>::new();
    for array_name in DOCUMENT_NODE_ARRAYS {
        let array = document
            .get(array_name)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("draft-v5 document has no `{array_name}` array"))?;
        for node in array {
            let id = node
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| is_persistent_id(id))
                .ok_or_else(|| format!("`{array_name}` row has no persistent ID"))?;
            if nodes.insert(id.to_owned(), (array_name, node)).is_some() {
                return Err(format!("persistent ID `{id}` is defined more than once"));
            }
        }
    }
    let id_kinds = nodes
        .iter()
        .map(|(id, (kind, _))| (id.clone(), (*kind).to_owned()))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<(String, String)>>::new();
    let mut incoming = BTreeMap::<String, Vec<(String, String)>>::new();
    for (id, (_, node)) in &nodes {
        let mut edges = Vec::new();
        collect_reference_edges(node, "$", &id_kinds, &mut edges);
        edges.sort();
        for (path, target) in &edges {
            incoming
                .entry(target.clone())
                .or_default()
                .push((path.clone(), id.clone()));
        }
        outgoing.insert(id.clone(), edges);
    }
    for edges in incoming.values_mut() {
        edges.sort();
    }

    let mut colors = BTreeMap::new();
    for (id, (kind, node)) in &nodes {
        let base = normalize_semantic_value((*node).clone(), &id_kinds, None, true, false);
        colors.insert(
            id.clone(),
            semantic_digest(&json!({ "kind": kind, "base": base }))?,
        );
    }
    for _ in 0..12 {
        let previous = colors.clone();
        for (id, (kind, node)) in &nodes {
            let base = normalize_semantic_value((*node).clone(), &id_kinds, None, true, false);
            let mut outgoing = outgoing
                .get(id)
                .into_iter()
                .flatten()
                .map(|(path, target)| {
                    json!({
                        "path": path,
                        "target": previous.get(target).expect("edge target was indexed"),
                    })
                })
                .collect::<Vec<_>>();
            outgoing.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());
            let mut incoming = incoming
                .get(id)
                .into_iter()
                .flatten()
                .map(|(path, source)| {
                    json!({
                        "path": path,
                        "source": previous.get(source).expect("edge source was indexed"),
                    })
                })
                .collect::<Vec<_>>();
            incoming.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());
            colors.insert(
                id.clone(),
                semantic_digest(&json!({
                    "kind": kind,
                    "base": base,
                    "outgoing": outgoing,
                    "incoming": incoming,
                }))?,
            );
        }
    }
    Ok(colors)
}

fn collect_reference_edges(
    value: &Value,
    path: &str,
    ids: &BTreeMap<String, String>,
    output: &mut Vec<(String, String)>,
) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(key.as_str(), "id" | "source_id") {
                    continue;
                }
                collect_reference_edges(child, &format!("{path}.{key}"), ids, output);
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                collect_reference_edges(child, &format!("{path}[{index}]"), ids, output);
            }
        }
        Value::String(id) if ids.contains_key(id) => output.push((path.to_owned(), id.clone())),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn semantic_digest(value: &Value) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| intent_content_digest(&bytes).to_string())
        .map_err(|error| format!("semantic snapshot cannot be encoded: {error}"))
}

fn normalize_semantic_document(
    mut value: Value,
    colors: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let document = value
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "draft-v5 document wrapper has no document object".to_owned())?;
    document.remove("id");
    document.remove("next_id");
    document.remove("source_order");
    for array_name in DOCUMENT_NODE_ARRAYS {
        let array = document
            .get_mut(array_name)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| format!("draft-v5 document has no `{array_name}` array"))?;
        for node in array.iter_mut() {
            let id = node
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("`{array_name}` row has no ID"))?;
            let semantic = colors
                .get(id)
                .ok_or_else(|| format!("accepted `{array_name}` row `{id}` has no design owner"))?
                .clone();
            *node = normalize_semantic_value(
                std::mem::take(node),
                &BTreeMap::new(),
                Some(colors),
                true,
                true,
            );
            node.as_object_mut()
                .expect("a document row remains an object")
                .insert("$semantic".to_owned(), Value::String(semantic));
        }
        array.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());
    }
    Ok(normalize_semantic_value(
        value,
        &BTreeMap::new(),
        Some(colors),
        false,
        true,
    ))
}

fn normalize_semantic_value(
    value: Value,
    reference_kinds: &BTreeMap<String, String>,
    colors: Option<&BTreeMap<String, String>>,
    _document_node: bool,
    retain_values: bool,
) -> Value {
    match value {
        Value::Object(object) => {
            let mut normalized = Map::new();
            for (key, child) in object {
                if matches!(
                    key.as_str(),
                    "id" | "label" | "source_id" | "next_id" | "source_order"
                ) {
                    continue;
                }
                let child = if !retain_values
                    && ((key == "position" && child.is_array()) || key == "value")
                {
                    Value::String("$variable".to_owned())
                } else {
                    normalize_semantic_value(child, reference_kinds, colors, false, retain_values)
                };
                normalized.insert(key, child);
            }
            Value::Object(normalized)
        }
        Value::Array(array) => {
            let mut normalized = array
                .into_iter()
                .map(|child| {
                    normalize_semantic_value(child, reference_kinds, colors, false, retain_values)
                })
                .collect::<Vec<_>>();
            if semantic_array_is_unordered(&normalized) {
                normalized.sort_by_cached_key(|row| serde_json::to_string(row).unwrap_or_default());
            }
            Value::Array(normalized)
        }
        Value::String(id) if is_persistent_id(&id) => {
            if let Some(color) = colors.and_then(|colors| colors.get(&id)) {
                json!({ "$ref": color })
            } else if let Some(kind) = reference_kinds.get(&id) {
                json!({ "$refKind": kind })
            } else {
                Value::String("$document-or-source".to_owned())
            }
        }
        other => other,
    }
}

fn diagnostic_observation(
    session: &geosolve_sketch::RetainedSketchDocumentSession,
) -> Result<Value, String> {
    let diagnostics = session.latest_attempt_diagnostics();
    let solve = diagnostics
        .solve
        .ok_or_else(|| "managed stage has no solve diagnostics".to_owned())?;
    let rank = diagnostics
        .rank
        .ok_or_else(|| "managed stage has no rank diagnostics".to_owned())?;
    let mobility = diagnostics
        .mobility
        .ok_or_else(|| "managed stage has no mobility diagnostics".to_owned())?;
    let mut bounds = diagnostics
        .bounds
        .iter()
        .map(|bound| {
            json!({
                "status": format!("{:?}", bound.status),
                "lower": bound.lower,
                "upper": bound.upper,
                "value": bound.value,
            })
        })
        .collect::<Vec<_>>();
    bounds.sort_by_cached_key(|bound| serde_json::to_string(bound).unwrap_or_default());
    Ok(json!({
        "solve": {
            "accepted": solve.accepted,
            "hardValidity": format!("{:?}", solve.hard_validity),
            "termination": format!("{:?}", solve.termination),
            "hardResidualsValidated": solve.hard_residuals_validated,
            "maximumNormalizedHardResidual": solve.maximum_normalized_hard_residual,
            "normalizedHardResidualL2": solve.normalized_hard_residual_l2,
        },
        "rank": {
            "numericalValid": rank.numerical_valid,
            "numericalRank": rank.numerical_rank,
            "numericalLeftNullity": rank.numerical_left_nullity,
            "numericalRightNullity": rank.numerical_right_nullity,
            "singular": rank.singular,
            "nearSingular": rank.near_singular,
            "structuralRank": rank.structural_rank,
            "structuralLeftNullity": rank.structural_left_nullity,
            "structuralRightNullity": rank.structural_right_nullity,
            "structuralClassification": format!("{:?}", rank.structural_classification),
        },
        "mobility": {
            "equalityDegreesOfFreedom": mobility.equality_degrees_of_freedom,
            "bidirectionalBoundedDegreesOfFreedom": mobility.bidirectional_bounded_degrees_of_freedom,
            "oneSided": format!("{:?}", mobility.one_sided),
        },
        "bounds": bounds,
    }))
}

fn semantic_array_is_unordered(array: &[Value]) -> bool {
    !array.is_empty()
        && array.iter().all(|row| {
            row.as_object().is_some_and(|object| {
                object.contains_key("$semantic")
                    || object.contains_key("element")
                    || object.contains_key("curve") && object.contains_key("role")
            })
        })
}

fn validate_code_lifecycle(
    manifest: &NativeParityManifest,
    projects: &[CodeProject],
    materialized: &[(KeyedReconcileState, MaterializedCodeProject)],
) -> Result<(), String> {
    let mut session = SketchCodeSession::new_project(
        projects[0].clone(),
        materialized[0].0.clone(),
        materialized[0].1.expansion.clone(),
        json!({ "stage": "base" }),
    )
    .map_err(|error| {
        format!(
            "{} code-session bootstrap failed: {error}",
            manifest.case_id
        )
    })?;
    if session.can_undo() || session.can_redo() {
        return Err(format!(
            "{} code-session bootstrap invented history",
            manifest.case_id
        ));
    }
    apply_code_stage(manifest, &mut session, &projects[1], 1)?;
    let terminal_index = if manifest.lifecycle == "create-edit-undo-redo" {
        apply_code_stage(manifest, &mut session, &projects[2], 2)?;
        2
    } else {
        1
    };
    if !session.can_undo() || session.can_redo() {
        return Err(format!(
            "{} code lifecycle has the wrong terminal history shape",
            manifest.case_id
        ));
    }
    session
        .undo()
        .map_err(|error| format!("{} code Undo failed: {error}", manifest.case_id))?
        .ok_or_else(|| format!("{} code Undo returned no receipt", manifest.case_id))?;
    let undo_index = usize::from(manifest.lifecycle == "create-edit-undo-redo");
    if session.snapshot().managed.source != projects[undo_index].managed.source
        || !session.can_redo()
    {
        return Err(format!(
            "{} code Undo did not restore the created source authority",
            manifest.case_id
        ));
    }
    session
        .redo()
        .map_err(|error| format!("{} code Redo failed: {error}", manifest.case_id))?
        .ok_or_else(|| format!("{} code Redo returned no receipt", manifest.case_id))?;
    if session.snapshot().managed.source != projects[terminal_index].managed.source
        || session.can_redo()
    {
        return Err(format!(
            "{} code Redo did not restore terminal source authority",
            manifest.case_id
        ));
    }
    let wire = session
        .to_canonical_json()
        .map_err(|error| format!("{} code history encoding failed: {error}", manifest.case_id))?;
    let restored = SketchCodeSession::from_json(&wire)
        .map_err(|error| format!("{} code history restore failed: {error}", manifest.case_id))?;
    if restored
        .to_canonical_json()
        .map_err(|error| error.to_string())?
        != wire
        || restored.snapshot().managed.source != projects[terminal_index].managed.source
    {
        return Err(format!(
            "{} code history did not round-trip exactly",
            manifest.case_id
        ));
    }
    Ok(())
}

fn apply_code_stage(
    manifest: &NativeParityManifest,
    session: &mut SketchCodeSession,
    project: &CodeProject,
    stage_index: usize,
) -> Result<(), String> {
    let desired = required_generated_members(project)
        .map_err(|error| format!("{} lifecycle inventory failed: {error}", manifest.case_id))?;
    let plan = session
        .plan_structural_reconciliation(session.identity(), desired, &BTreeSet::new())
        .map_err(|error| {
            format!(
                "{} lifecycle reconciliation failed: {error}",
                manifest.case_id
            )
        })?;
    let native = SketchDocument::from_draft_v5_json(&manifest.stages[stage_index].design_json)
        .map_err(|error| {
            format!(
                "{} lifecycle native stage failed: {error}",
                manifest.case_id
            )
        })?;
    let candidate = materialize(
        &format!("{}-lifecycle", manifest.case_id),
        stage_index,
        project,
        plan.staged(),
        native.model_scale(),
    )?;
    let prepared = session
        .prepare_project_edit_from_plan(
            session.identity(),
            project.clone(),
            plan,
            candidate.expansion,
            json!({ "stage": manifest.stages[stage_index].name }),
            format!("Parity {}", manifest.stages[stage_index].name),
        )
        .map_err(|error| {
            format!(
                "{} code stage preparation failed: {error}",
                manifest.case_id
            )
        })?;
    session.apply_prepared(prepared).map_err(|error| {
        format!(
            "{} code stage publication failed: {error}",
            manifest.case_id
        )
    })?;
    if session.snapshot().managed.source != project.managed.source {
        return Err(format!(
            "{} code stage published the wrong source",
            manifest.case_id
        ));
    }
    Ok(())
}

fn compare_document_json(
    expected: &str,
    actual: &str,
    tolerance: f64,
    context: &str,
) -> Result<(), String> {
    let mut expected: Value = serde_json::from_str(expected)
        .map_err(|error| format!("{context} expected JSON is invalid: {error}"))?;
    let mut actual: Value = serde_json::from_str(actual)
        .map_err(|error| format!("{context} actual JSON is invalid: {error}"))?;
    normalize_document_ids(&mut expected);
    normalize_document_ids(&mut actual);
    compare_values(&expected, &actual, tolerance, "$ ")
        .map_err(|detail| format!("{context} differs: {detail}"))
}

fn normalize_document_ids(value: &mut Value) {
    let mut ids = BTreeMap::new();
    collect_defined_ids(value, "$", &mut ids);
    replace_ids(value, "$", &mut ids);
}

fn collect_defined_ids(value: &Value, path: &str, ids: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for key in keys {
                if key == "next_id" {
                    continue;
                }
                let child = &object[key];
                let child_path = format!("{path}.{key}");
                if (key == "id" || key.ends_with("_id"))
                    && child.as_str().is_some_and(is_persistent_id)
                {
                    ids.entry(child.as_str().expect("checked string").to_owned())
                        .or_insert(child_path.clone());
                }
                collect_defined_ids(child, &child_path, ids);
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                collect_defined_ids(child, &format!("{path}[{index}]"), ids);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn replace_ids(value: &mut Value, path: &str, ids: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(object) => {
            object.remove("next_id");
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort_unstable();
            for key in keys {
                replace_ids(
                    object.get_mut(&key).expect("key came from object"),
                    &format!("{path}.{key}"),
                    ids,
                );
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter_mut().enumerate() {
                replace_ids(child, &format!("{path}[{index}]"), ids);
            }
        }
        Value::String(string) if is_persistent_id(string) => {
            let replacement = ids
                .entry(string.clone())
                .or_insert_with(|| format!("{path}:reference"))
                .clone();
            *string = replacement;
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_persistent_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
            if !expected.is_finite() || !actual.is_finite() {
                return Err(format!("{path}: non-finite number"));
            }
            let allowed = tolerance * expected.abs().max(actual.abs()).max(1.0);
            if (expected - actual).abs() <= allowed {
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
            if path.ends_with(".bounds") {
                return compare_unordered_values(expected, actual, tolerance, path);
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

fn compare_unordered_values(
    expected: &[Value],
    actual: &[Value],
    tolerance: f64,
    path: &str,
) -> Result<(), String> {
    let mut unmatched = (0..actual.len()).collect::<BTreeSet<_>>();
    for (expected_index, expected_value) in expected.iter().enumerate() {
        let matched = unmatched.iter().copied().find(|actual_index| {
            compare_values(
                expected_value,
                &actual[*actual_index],
                tolerance,
                &format!("{path}[{expected_index}]"),
            )
            .is_ok()
        });
        let Some(matched) = matched else {
            return Err(format!(
                "{path}[{expected_index}]: no semantically equal row in unordered multiset"
            ));
        };
        unmatched.remove(&matched);
    }
    Ok(())
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

#[cfg(test)]
mod canonicalizer_tests {
    use serde_json::{Value, json};

    use super::{compare_values, semantic_document, semantic_document_with_colors};

    const A_POINT: &str = "00000000000000000000000000000001";
    const B_POINT: &str = "00000000000000000000000000000002";
    const PARAMETER: &str = "00000000000000000000000000000003";
    const LINE: &str = "00000000000000000000000000000004";
    const CONTACT: &str = "00000000000000000000000000000005";

    fn fixture() -> Value {
        json!({
            "version": 5,
            "document": {
                "version": 4,
                "id": "00000000000000000000000000000010",
                "next_id": "00000000000000000000000000000011",
                "model_scale": 4.0,
                "points": [
                    { "id": A_POINT, "label": "start", "position": [0.0, 0.0] },
                    { "id": B_POINT, "label": "end", "position": [2.0, 0.0] }
                ],
                "scalars": [{
                    "id": PARAMETER,
                    "label": "contact parameter",
                    "value": 0.25,
                    "unit": "parameter",
                    "domain": { "kind": "bounded", "lower": 0.0, "upper": 1.0 }
                }],
                "curves": [{
                    "id": LINE,
                    "label": "edge",
                    "definition": {
                        "kind": "line",
                        "start": A_POINT,
                        "end": B_POINT,
                        "branch_direction": [1.0, 0.0]
                    }
                }],
                "contacts": [{
                    "id": CONTACT,
                    "label": "contact",
                    "curve": { "curve": LINE, "segment": 0 },
                    "parameter": PARAMETER,
                    "domain": { "kind": "bounded", "lower": 0.0, "upper": 1.0 },
                    "winding": 0,
                    "neighborhood": "interior",
                    "tangent_orientation": null
                }],
                "trim_views": [],
                "constraints": [],
                "dimensions": [],
                "source_order": []
            },
            "geometry_roles": [],
            "user_inactive_elements": [],
            "host_activation": null,
            "parameters": [],
            "parameter_bindings": [],
            "parameter_outputs": [],
            "external_bindings": [],
            "retained_planar_constraints": []
        })
    }

    fn canonical(value: &Value) -> Value {
        semantic_document(&serde_json::to_string(value).expect("fixture JSON"))
            .expect("fixture canonicalizes")
            .0
    }

    fn accepted_canonical(value: &Value) -> Result<Value, String> {
        let source = serde_json::to_string(value).expect("fixture JSON");
        let (_, colors) = semantic_document(&source)?;
        semantic_document_with_colors(&source, &colors)
    }

    fn assert_semantically_different(changed: &Value) {
        assert!(
            compare_values(&canonical(&fixture()), &canonical(changed), 0.0, "$").is_err(),
            "meaningful mutation must not canonicalize away"
        );
    }

    #[test]
    fn canonicalizer_ignores_backend_ids_labels_and_definition_order() {
        let mut equivalent = fixture();
        let replacements = [
            (A_POINT, "10000000000000000000000000000001"),
            (B_POINT, "10000000000000000000000000000002"),
            (PARAMETER, "10000000000000000000000000000003"),
            (LINE, "10000000000000000000000000000004"),
            (CONTACT, "10000000000000000000000000000005"),
        ];
        let mut encoded = serde_json::to_string(&equivalent).expect("fixture JSON");
        for (before, after) in replacements {
            encoded = encoded.replace(before, after);
        }
        equivalent = serde_json::from_str(&encoded).expect("rewritten fixture JSON");
        equivalent["document"]["points"]
            .as_array_mut()
            .expect("points")
            .reverse();
        equivalent["document"]["points"][0]["label"] = json!("renamed B");
        equivalent["document"]["points"][1]["label"] = json!("renamed A");
        equivalent["document"]["curves"][0]["label"] = json!("renamed curve");
        equivalent["document"]["contacts"][0]["label"] = json!("renamed contact");

        compare_values(&canonical(&fixture()), &canonical(&equivalent), 0.0, "$")
            .expect("backend identity, labels, and definition order are non-semantic");
    }

    #[test]
    fn canonicalizer_retains_topology_branch_contact_and_numeric_meaning() {
        let mut topology = fixture();
        topology["document"]["curves"][0]["definition"]["end"] = json!(A_POINT);
        assert_semantically_different(&topology);

        let mut branch = fixture();
        branch["document"]["curves"][0]["definition"]["branch_direction"] = json!([-1.0, 0.0]);
        assert_semantically_different(&branch);

        let mut contact = fixture();
        contact["document"]["contacts"][0]["neighborhood"] = json!("start");
        assert_semantically_different(&contact);

        let mut numeric = fixture();
        numeric["document"]["points"][1]["position"] = json!([3.0, 0.0]);
        assert_semantically_different(&numeric);
    }

    #[test]
    fn accepted_contact_parameter_comparison_is_curve_space_and_scale_sensitive() {
        let mut short_expected = fixture();
        short_expected["document"]["model_scale"] = json!(1.0);
        short_expected["document"]["points"][1]["position"] = json!([0.01, 0.0]);
        let mut short_actual = short_expected.clone();
        short_actual["document"]["scalars"][0]["value"] = json!(0.25000005);

        assert!(
            compare_values(
                &canonical(&short_expected),
                &canonical(&short_actual),
                0.0,
                "$",
            )
            .is_err(),
            "authored design parameters remain exact"
        );
        compare_values(
            &accepted_canonical(&short_expected).expect("short expected contact evaluates"),
            &accepted_canonical(&short_actual).expect("short actual contact evaluates"),
            1.0e-9,
            "$",
        )
        .expect("a sub-tolerance displacement on a short support is semantically equal");

        let mut long_expected = short_expected;
        long_expected["document"]["points"][1]["position"] = json!([1.0, 0.0]);
        let mut long_actual = long_expected.clone();
        long_actual["document"]["scalars"][0]["value"] = json!(0.25000005);
        assert!(
            compare_values(
                &accepted_canonical(&long_expected).expect("long expected contact evaluates"),
                &accepted_canonical(&long_actual).expect("long actual contact evaluates"),
                1.0e-9,
                "$",
            )
            .is_err(),
            "the same raw parameter delta must fail when it becomes a larger physical displacement"
        );
    }

    #[test]
    fn accepted_contact_parameter_projection_rejects_irregular_supports() {
        let mut irregular = fixture();
        irregular["document"]["points"][1]["position"] = json!([0.0, 0.0]);
        assert!(
            accepted_canonical(&irregular).is_err(),
            "accepted comparison must not project a contact through a degenerate support"
        );
    }
}
